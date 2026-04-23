//! Multi-Tenant Namespace Support for RivetDB
//!
//! Provides true multi-tenancy with unlimited namespaces and resource isolation.
//! Unlike Redis (which only has 16 databases 0-15), RivetDB supports:
//! - Unlimited named namespaces
//! - Per-namespace memory limits
//! - Per-namespace metrics
//! - Complete data isolation

use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};
use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use std::time::Instant;
use dashmap::DashMap;

use super::value::ValueObject;
use super::eviction::EvictionPolicy;
use super::state::ExpiryHeap;
use std::collections::{BinaryHeap, VecDeque};
use std::cmp::Reverse;

/// Individual namespace state - completely isolated from other namespaces
pub struct NamespaceState {
    /// Namespace name
    pub name: String,
    
    /// Data store - isolated per namespace
    pub db: DashMap<String, ValueObject>,
    
    /// Expiry heap for this namespace
    pub expiries: Mutex<ExpiryHeap>,
    
    /// Memory limit in bytes (0 = unlimited)
    pub max_memory: AtomicUsize,
    
    /// Current memory usage estimate
    pub current_memory: AtomicUsize,
    
    /// Key count
    pub key_count: AtomicUsize,
    
    /// Command count for this namespace
    pub command_count: AtomicU64,
    
    /// Created timestamp
    pub created_at: Instant,
    
    /// Eviction policy
    pub eviction_policy: EvictionPolicy,
}

impl NamespaceState {
    /// Create a new namespace with optional memory limit
    pub fn new(name: String, max_memory: usize) -> Self {
        NamespaceState {
            name,
            db: DashMap::new(),
            expiries: Mutex::new(BinaryHeap::new()),
            max_memory: AtomicUsize::new(max_memory),
            current_memory: AtomicUsize::new(0),
            key_count: AtomicUsize::new(0),
            command_count: AtomicU64::new(0),
            created_at: Instant::now(),
            eviction_policy: EvictionPolicy::AllKeysLRU,
        }
    }
    
    /// Get namespace info
    pub fn info(&self) -> NamespaceInfo {
        NamespaceInfo {
            name: self.name.clone(),
            key_count: self.db.len(),
            max_memory: self.max_memory.load(Ordering::Relaxed),
            current_memory: self.current_memory.load(Ordering::Relaxed),
            command_count: self.command_count.load(Ordering::Relaxed),
            uptime_seconds: self.created_at.elapsed().as_secs(),
        }
    }
    
    /// Increment command count
    pub fn inc_command_count(&self) {
        self.command_count.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Update memory usage estimate
    pub fn update_memory(&self, delta: i64) {
        if delta > 0 {
            self.current_memory.fetch_add(delta as usize, Ordering::Relaxed);
        } else {
            let abs_delta = (-delta) as usize;
            let current = self.current_memory.load(Ordering::Relaxed);
            if current >= abs_delta {
                self.current_memory.fetch_sub(abs_delta, Ordering::Relaxed);
            }
        }
    }
    
    /// Check if memory limit is exceeded
    pub fn is_memory_exceeded(&self) -> bool {
        let max = self.max_memory.load(Ordering::Relaxed);
        if max == 0 {
            return false; // Unlimited
        }
        self.current_memory.load(Ordering::Relaxed) >= max
    }
    
    /// Set memory limit
    pub fn set_max_memory(&self, limit: usize) {
        self.max_memory.store(limit, Ordering::Relaxed);
    }
}

/// Namespace info for stats
#[derive(Debug, Clone)]
pub struct NamespaceInfo {
    pub name: String,
    pub key_count: usize,
    pub max_memory: usize,
    pub current_memory: usize,
    pub command_count: u64,
    pub uptime_seconds: u64,
}

/// Multi-tenant state manager
pub struct MultiTenantState {
    /// All namespaces (protected by RwLock for rare write operations)
    namespaces: RwLock<HashMap<String, Arc<NamespaceState>>>,
    
    /// Default namespace (always available)
    pub default_namespace: Arc<NamespaceState>,
}

impl MultiTenantState {
    /// Create a new multi-tenant state manager
    pub fn new() -> Self {
        let default = Arc::new(NamespaceState::new("default".into(), 0));
        let mut namespaces = HashMap::new();
        namespaces.insert("default".into(), default.clone());
        
        MultiTenantState {
            namespaces: RwLock::new(namespaces),
            default_namespace: default,
        }
    }
    
    /// Create a new namespace
    pub fn create_namespace(&self, name: &str, max_memory: usize) -> Result<(), NamespaceError> {
        if name.is_empty() {
            return Err(NamespaceError::InvalidName("Name cannot be empty".into()));
        }
        
        if name.len() > 64 {
            return Err(NamespaceError::InvalidName("Name too long (max 64 chars)".into()));
        }
        
        // Check for invalid characters
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return Err(NamespaceError::InvalidName(
                "Name can only contain alphanumeric, underscore, and hyphen".into()
            ));
        }
        
        let mut namespaces = self.namespaces.write().unwrap();
        
        if namespaces.contains_key(name) {
            return Err(NamespaceError::AlreadyExists(name.to_string()));
        }
        
        let namespace = Arc::new(NamespaceState::new(name.to_string(), max_memory));
        namespaces.insert(name.to_string(), namespace);
        
        Ok(())
    }
    
    /// Get a namespace by name
    pub fn get_namespace(&self, name: &str) -> Option<Arc<NamespaceState>> {
        let namespaces = self.namespaces.read().unwrap();
        namespaces.get(name).cloned()
    }
    
    /// Delete a namespace (cannot delete default)
    pub fn delete_namespace(&self, name: &str) -> Result<(), NamespaceError> {
        if name == "default" {
            return Err(NamespaceError::CannotDeleteDefault);
        }
        
        let mut namespaces = self.namespaces.write().unwrap();
        
        if namespaces.remove(name).is_none() {
            return Err(NamespaceError::NotFound(name.to_string()));
        }
        
        Ok(())
    }
    
    /// List all namespaces
    pub fn list_namespaces(&self) -> Vec<NamespaceInfo> {
        let namespaces = self.namespaces.read().unwrap();
        namespaces.values().map(|ns| ns.info()).collect()
    }
    
    /// Get namespace count
    pub fn namespace_count(&self) -> usize {
        let namespaces = self.namespaces.read().unwrap();
        namespaces.len()
    }
    
    /// Check if namespace exists
    pub fn exists(&self, name: &str) -> bool {
        let namespaces = self.namespaces.read().unwrap();
        namespaces.contains_key(name)
    }
}

impl Default for MultiTenantState {
    fn default() -> Self {
        Self::new()
    }
}

/// Namespace errors
#[derive(Debug, Clone)]
pub enum NamespaceError {
    InvalidName(String),
    AlreadyExists(String),
    NotFound(String),
    CannotDeleteDefault,
    MemoryExceeded,
}

impl std::fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NamespaceError::InvalidName(s) => write!(f, "Invalid namespace name: {}", s),
            NamespaceError::AlreadyExists(s) => write!(f, "Namespace '{}' already exists", s),
            NamespaceError::NotFound(s) => write!(f, "Namespace '{}' not found", s),
            NamespaceError::CannotDeleteDefault => write!(f, "Cannot delete default namespace"),
            NamespaceError::MemoryExceeded => write!(f, "Namespace memory limit exceeded"),
        }
    }
}

/// Parse memory size string (e.g., "256MB", "1GB", "1024KB")
pub fn parse_memory_size(s: &str) -> Option<usize> {
    let s = s.trim().to_uppercase();
    
    if let Some(num) = s.strip_suffix("GB") {
        num.trim().parse::<usize>().ok().map(|n| n * 1024 * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("MB") {
        num.trim().parse::<usize>().ok().map(|n| n * 1024 * 1024)
    } else if let Some(num) = s.strip_suffix("KB") {
        num.trim().parse::<usize>().ok().map(|n| n * 1024)
    } else if let Some(num) = s.strip_suffix("B") {
        num.trim().parse::<usize>().ok()
    } else {
        s.parse::<usize>().ok()
    }
}

/// Format memory size for display
pub fn format_memory_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_namespace() {
        let mt = MultiTenantState::new();
        
        assert!(mt.create_namespace("tenant_a", 256 * 1024 * 1024).is_ok());
        assert!(mt.create_namespace("tenant_b", 512 * 1024 * 1024).is_ok());
        
        // Duplicate should fail
        assert!(mt.create_namespace("tenant_a", 0).is_err());
        
        // Empty name should fail
        assert!(mt.create_namespace("", 0).is_err());
    }

    #[test]
    fn test_get_namespace() {
        let mt = MultiTenantState::new();
        mt.create_namespace("test", 0).unwrap();
        
        let ns = mt.get_namespace("test");
        assert!(ns.is_some());
        assert_eq!(ns.unwrap().name, "test");
        
        assert!(mt.get_namespace("nonexistent").is_none());
    }

    #[test]
    fn test_delete_namespace() {
        let mt = MultiTenantState::new();
        mt.create_namespace("deleteme", 0).unwrap();
        
        assert!(mt.delete_namespace("deleteme").is_ok());
        assert!(mt.get_namespace("deleteme").is_none());
        
        // Can't delete default
        assert!(mt.delete_namespace("default").is_err());
    }

    #[test]
    fn test_parse_memory_size() {
        assert_eq!(parse_memory_size("256MB"), Some(256 * 1024 * 1024));
        assert_eq!(parse_memory_size("1GB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_memory_size("1024KB"), Some(1024 * 1024));
        assert_eq!(parse_memory_size("1024"), Some(1024));
    }
}
