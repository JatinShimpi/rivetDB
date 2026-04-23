use std::collections::{VecDeque, BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use dashmap::DashMap;

use super::value::ValueObject;
use super::eviction::EvictionPolicy;

pub type ExpiryHeap = BinaryHeap<Reverse<(Instant, String)>>;

/// Shared state type - now uses lock-free DashMap for the main data store
pub type SharedState = Arc<ServerState>;

pub struct SlowLogEntry {
    pub command: String,
    pub duration_ns: u128,
    pub timestamp: Instant,
}

/// ServerState with DashMap for lock-free concurrent access
/// 
/// The main `db` field uses DashMap which provides:
/// - Lock-free concurrent reads and writes
/// - Fine-grained per-shard locking internally
/// - ~10x better performance under high concurrency vs Mutex<HashMap>
/// 
/// Other fields that need synchronized access use appropriate locks:
/// - expiries: Mutex for heap operations (not concurrent-safe)
/// - metrics: RwLock for mostly-read access patterns
pub struct ServerState {
    /// Main data store - lock-free concurrent HashMap
    pub db: DashMap<String, ValueObject>,
    
    /// Expiry heap - needs Mutex as BinaryHeap isn't concurrent
    pub expiries: Mutex<ExpiryHeap>,
    
    /// Expired key counter
    pub expired_count: std::sync::atomic::AtomicU64,
    
    /// Observability metrics - use DashMap for concurrent access
    pub command_count: DashMap<String, u64>,
    pub command_time_ns: DashMap<String, u128>,
    pub key_access_count: DashMap<String, u64>,
    
    /// Slowlog - needs Mutex for VecDeque operations
    pub slowlog: Mutex<VecDeque<SlowLogEntry>>,
    
    /// Configuration
    pub max_memory: usize,
    pub eviction_policy: EvictionPolicy,
}

impl ServerState {
    /// Create a new ServerState with default settings
    /// 
    /// Pre-allocates DashMap with 100,000 capacity to avoid rehashing
    /// during high-volume operations. This uses ~8-16MB initial memory
    /// but provides consistent performance under benchmark loads.
    pub fn new(max_memory: usize, eviction_policy: EvictionPolicy) -> Self {
        ServerState {
            // Pre-size for 100K keys to avoid rehashing during benchmarks
            db: DashMap::with_capacity(100_000),
            expiries: Mutex::new(BinaryHeap::new()),
            expired_count: std::sync::atomic::AtomicU64::new(0),
            command_count: DashMap::with_capacity(64),  // ~64 unique commands
            command_time_ns: DashMap::with_capacity(64),
            key_access_count: DashMap::with_capacity(10_000),  // Hot keys tracking
            slowlog: Mutex::new(VecDeque::with_capacity(128)),
            max_memory,
            eviction_policy,
        }
    }
    
    /// Increment command count atomically
    pub fn inc_command_count(&self, cmd: &str) {
        self.command_count
            .entry(cmd.to_string())
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }
    
    /// Add command timing
    pub fn add_command_time(&self, cmd: &str, time_ns: u128) {
        self.command_time_ns
            .entry(cmd.to_string())
            .and_modify(|t| *t += time_ns)
            .or_insert(time_ns);
    }
    
    /// Track key access
    pub fn track_key_access(&self, key: &str) {
        self.key_access_count
            .entry(key.to_string())
            .and_modify(|c| *c += 1)
            .or_insert(1);
    }
    
    /// Add to slow log
    pub fn add_slowlog(&self, entry: SlowLogEntry) {
        if let Ok(mut log) = self.slowlog.lock() {
            if log.len() >= 128 {
                log.pop_front();
            }
            log.push_back(entry);
        }
    }
    
    /// Check and remove expired key (call from expiry loop)
    pub fn check_and_expire(&self, key: &str) -> bool {
        if self.db.remove(key).is_some() {
            self.expired_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            true
        } else {
            false
        }
    }
    
    /// Get total key count
    pub fn key_count(&self) -> usize {
        self.db.len()
    }
    
    /// Get expired count
    pub fn get_expired_count(&self) -> u64 {
        self.expired_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}