use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;
use std::collections::HashMap;
use tracing::{info, warn, error, debug};

use crate::storage::{ServerState, ValueObject, ZSet};
use crate::commands::ParsedCommand;
use crate::protocol::{parse_frame, frame_to_command};

/// AOF fsync policy
#[derive(Debug, Clone, PartialEq)]
pub enum AofFsyncPolicy {
    /// Fsync after every write - safest, slowest
    Always,
    /// Fsync once per second - good balance
    Everysec,
    /// Let OS decide - fastest, least safe
    No,
}

impl Default for AofFsyncPolicy {
    fn default() -> Self {
        AofFsyncPolicy::Everysec
    }
}

impl From<&str> for AofFsyncPolicy {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "always" => AofFsyncPolicy::Always,
            "everysec" => AofFsyncPolicy::Everysec,
            "no" => AofFsyncPolicy::No,
            _ => AofFsyncPolicy::default(),
        }
    }
}

/// Commands that modify state and should be logged to AOF
const WRITE_COMMANDS: &[&str] = &[
    // String
    "SET", "MSET", "APPEND", "SETRANGE", "INCR", "DECR",
    // List  
    "LPUSH", "RPUSH", "LPOP", "RPOP", "LSET", "LTRIM",
    // Set
    "SADD", "SREM",
    // Sorted Set
    "ZADD", "ZREM", "ZINCRBY", "ZPOPMIN", "ZPOPMAX",
    "ZREMRANGEBYRANK", "ZREMRANGEBYSCORE",
    // Hash
    "HSET", "HSETNX", "HMSET", "HDEL", "HINCRBY", "HINCRBYFLOAT",
    // Key
    "DEL", "RENAME", "RENAMENX", "EXPIRE",
];

/// Check if a command modifies state
pub fn is_write_command(cmd_name: &str) -> bool {
    WRITE_COMMANDS.contains(&cmd_name.to_uppercase().as_str())
}

/// AOF Writer - handles command logging with buffering and fsync
pub struct AofWriter {
    writer: BufWriter<File>,
    path: String,
    fsync_policy: AofFsyncPolicy,
    last_fsync: Instant,
    pending_writes: usize,
}

impl AofWriter {
    /// Create new AOF writer with optimized 64KB buffer
    pub fn new(path: &str, fsync_policy: AofFsyncPolicy) -> io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        // Use 64KB buffer instead of default 8KB for better throughput
        // This reduces syscall overhead during high-volume pipelined writes
        Ok(AofWriter {
            writer: BufWriter::with_capacity(64 * 1024, file),
            path: path.to_string(),
            fsync_policy,
            last_fsync: Instant::now(),
            pending_writes: 0,
        })
    }

    /// Log a command to AOF
    pub fn log_command(&mut self, cmd: &ParsedCommand) -> io::Result<()> {
        // Only log write commands
        if !is_write_command(&cmd.name) {
            return Ok(());
        }

        // Format as RESP array
        let resp = self.format_as_resp_array(cmd);
        self.writer.write_all(resp.as_bytes())?;
        self.pending_writes += 1;

        // Handle fsync based on policy
        // Optimized thresholds for high-throughput pipelining
        match self.fsync_policy {
            AofFsyncPolicy::Always => {
                self.writer.flush()?;
                self.writer.get_ref().sync_all()?;
                self.pending_writes = 0;
            }
            AofFsyncPolicy::Everysec => {
                // Batch more writes before flushing - 1000 instead of 100
                // This dramatically improves pipelined write performance
                if self.pending_writes >= 1000 {
                    self.writer.flush()?;
                    self.pending_writes = 0;
                }
            }
            AofFsyncPolicy::No => {
                // Flush much less frequently for maximum throughput
                if self.pending_writes >= 10000 {
                    self.writer.flush()?;
                    self.pending_writes = 0;
                }
            }
        }

        Ok(())
    }

    /// Force fsync - called by background thread for everysec policy
    pub fn fsync(&mut self) -> io::Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_all()?;
        self.last_fsync = Instant::now();
        self.pending_writes = 0;
        Ok(())
    }

    /// Format command as RESP array
    fn format_as_resp_array(&self, cmd: &ParsedCommand) -> String {
        let total_elements = 1 + cmd.args.len();
        let mut resp = format!("*{}\r\n", total_elements);
        
        // Command name
        resp.push_str(&format!("${}\r\n{}\r\n", cmd.name.len(), cmd.name));
        
        // Arguments
        for arg in &cmd.args {
            resp.push_str(&format!("${}\r\n{}\r\n", arg.len(), arg));
        }
        
        resp
    }

    /// Get path
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Thread-safe AOF writer wrapper
pub type SharedAofWriter = Arc<Mutex<Option<AofWriter>>>;

/// Create a new shared AOF writer
pub fn create_aof_writer(path: &str, fsync_policy: AofFsyncPolicy) -> io::Result<SharedAofWriter> {
    let writer = AofWriter::new(path, fsync_policy)?;
    Ok(Arc::new(Mutex::new(Some(writer))))
}

/// Start background fsync thread for everysec policy
pub fn start_fsync_thread(aof: SharedAofWriter, interval_ms: u64) {
    thread::spawn(move || {
        let interval = Duration::from_millis(interval_ms);
        loop {
            thread::sleep(interval);
            
            if let Ok(mut guard) = aof.lock() {
                if let Some(ref mut writer) = *guard {
                    if let Err(e) = writer.fsync() {
                        error!("AOF fsync error: {}", e);
                    } else {
                        debug!("AOF fsync completed");
                    }
                }
            }
        }
    });
}

/// Load AOF file and replay commands into state
pub fn load_aof(path: &Path, state: &ServerState) -> io::Result<usize> {
    if !path.exists() {
        info!("No AOF file found at {:?}, starting fresh", path);
        return Ok(0);
    }

    let file = File::open(path)?;
    let file_size = file.metadata()?.len();
    info!("Loading AOF file: {:?} ({} bytes)", path, file_size);

    let mut reader = BufReader::new(file);
    let mut commands_loaded = 0;
    let mut errors = 0;

    loop {
        match parse_frame(&mut reader) {
            Ok(frame) => {
                match frame_to_command(frame) {
                    Ok(cmd) => {
                        // Replay command (without logging to AOF again!)
                        replay_command(cmd, state);
                        commands_loaded += 1;
                        
                        if commands_loaded % 10000 == 0 {
                            debug!("Loaded {} commands from AOF", commands_loaded);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse command from AOF: {}", e);
                        errors += 1;
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                // Normal end of file
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::InvalidData => {
                // Possibly truncated command - skip to next
                warn!("Invalid data in AOF, skipping: {}", e);
                errors += 1;
                // Try to recover by finding next command
                if !try_recover_aof_position(&mut reader)? {
                    break;
                }
            }
            Err(e) => {
                error!("Error reading AOF: {}", e);
                return Err(e);
            }
        }
    }

    info!(
        "AOF loaded: {} commands replayed, {} errors",
        commands_loaded, errors
    );

    Ok(commands_loaded)
}

/// Try to recover AOF position after error by finding next * (array start)
fn try_recover_aof_position<R: BufRead>(reader: &mut R) -> io::Result<bool> {
    let mut buf = [0u8; 1];
    loop {
        match reader.read_exact(&mut buf) {
            Ok(()) => {
                if buf[0] == b'*' {
                    // Found start of next command, but we consumed the *
                    // This is a simplified recovery - real implementation would unread
                    return Ok(true);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(false);
            }
            Err(e) => return Err(e),
        }
    }
}

/// Replay a single command into state (used during AOF load)
/// Note: This works with DashMap directly - no SharedState wrapper
fn replay_command(cmd: ParsedCommand, state: &ServerState) {
    use std::collections::{HashSet, LinkedList};
    use crate::storage::ZSet;

    match cmd.name.to_uppercase().as_str() {
        // String commands
        "SET" if cmd.args.len() >= 2 => {
            state.db.insert(cmd.args[0].clone(), ValueObject::String(cmd.args[1].clone()));
        }
        "MSET" if cmd.args.len() >= 2 && cmd.args.len() % 2 == 0 => {
            for chunk in cmd.args.chunks(2) {
                state.db.insert(chunk[0].clone(), ValueObject::String(chunk[1].clone()));
            }
        }
        "DEL" => {
            for key in &cmd.args {
                state.db.remove(key);
            }
        }
        
        // List commands
        "LPUSH" if cmd.args.len() >= 2 => {
            let key = &cmd.args[0];
            let mut entry = state.db.entry(key.clone())
                .or_insert_with(|| ValueObject::List(LinkedList::new()));
            if let ValueObject::List(list) = entry.value_mut() {
                for value in &cmd.args[1..] {
                    list.push_front(value.clone());
                }
            }
        }
        "RPUSH" if cmd.args.len() >= 2 => {
            let key = &cmd.args[0];
            let mut entry = state.db.entry(key.clone())
                .or_insert_with(|| ValueObject::List(LinkedList::new()));
            if let ValueObject::List(list) = entry.value_mut() {
                for value in &cmd.args[1..] {
                    list.push_back(value.clone());
                }
            }
        }
        
        // Set commands
        "SADD" if cmd.args.len() >= 2 => {
            let key = &cmd.args[0];
            let mut entry = state.db.entry(key.clone())
                .or_insert_with(|| ValueObject::Set(HashSet::new()));
            if let ValueObject::Set(set) = entry.value_mut() {
                for member in &cmd.args[1..] {
                    set.insert(member.clone());
                }
            }
        }
        "SREM" if cmd.args.len() >= 2 => {
            let key = &cmd.args[0];
            if let Some(mut entry) = state.db.get_mut(key) {
                if let ValueObject::Set(set) = entry.value_mut() {
                    for member in &cmd.args[1..] {
                        set.remove(member);
                    }
                }
            }
        }
        
        // Sorted Set commands
        "ZADD" if cmd.args.len() >= 3 => {
            let key = &cmd.args[0];
            let mut entry = state.db.entry(key.clone())
                .or_insert_with(|| ValueObject::ZSet(ZSet::new()));
            if let ValueObject::ZSet(zset) = entry.value_mut() {
                // Find where score-member pairs start (skip options)
                let mut start = 1;
                while start < cmd.args.len() {
                    match cmd.args[start].to_uppercase().as_str() {
                        "NX" | "XX" | "GT" | "LT" | "CH" => start += 1,
                        _ => break,
                    }
                }
                for chunk in cmd.args[start..].chunks(2) {
                    if let Ok(score) = chunk[0].parse::<f64>() {
                        zset.add(chunk[1].clone(), score);
                    }
                }
            }
        }
        "ZREM" if cmd.args.len() >= 2 => {
            let key = &cmd.args[0];
            if let Some(mut entry) = state.db.get_mut(key) {
                if let ValueObject::ZSet(zset) = entry.value_mut() {
                    for member in &cmd.args[1..] {
                        zset.remove(member);
                    }
                }
            }
        }
        
        // Hash commands
        "HSET" | "HMSET" if cmd.args.len() >= 3 => {
            let key = &cmd.args[0];
            let mut entry = state.db.entry(key.clone())
                .or_insert_with(|| ValueObject::Hash(HashMap::new()));
            if let ValueObject::Hash(hash) = entry.value_mut() {
                for chunk in cmd.args[1..].chunks(2) {
                    if chunk.len() == 2 {
                        hash.insert(chunk[0].clone(), chunk[1].clone());
                    }
                }
            }
        }
        "HDEL" if cmd.args.len() >= 2 => {
            let key = &cmd.args[0];
            if let Some(mut entry) = state.db.get_mut(key) {
                if let ValueObject::Hash(hash) = entry.value_mut() {
                    for field in &cmd.args[1..] {
                        hash.remove(field);
                    }
                }
            }
        }
        
        // Rename - DashMap remove returns (key, value) tuple
        "RENAME" if cmd.args.len() == 2 => {
            if let Some((_, value)) = state.db.remove(&cmd.args[0]) {
                state.db.insert(cmd.args[1].clone(), value);
            }
        }
        
        _ => {
            // Other commands - log but don't fail
            debug!("Skipping unknown command during AOF replay: {}", cmd.name);
        }
    }

    // Note: evict_if_needed not called during replay to avoid Arc requirement
}

/// Rewrite AOF file with current state (compaction)
pub fn rewrite_aof(state: &ServerState, path: &str) -> io::Result<()> {
    let temp_path = format!("{}.tmp", path);
    
    info!("Starting AOF rewrite to {}", temp_path);
    
    let file = File::create(&temp_path)?;
    let mut writer = BufWriter::new(file);
    let mut commands_written = 0;

    // Iterate with DashMap - returns RefMulti with key() and value() methods
    for entry in state.db.iter() {
        let key = entry.key();
        let value = entry.value();
        match value {
            ValueObject::String(s) => {
                write_set_command(&mut writer, key, s)?;
                commands_written += 1;
            }
            ValueObject::List(list) => {
                if !list.is_empty() {
                    write_rpush_command(&mut writer, key, list)?;
                    commands_written += 1;
                }
            }
            ValueObject::Set(set) => {
                if !set.is_empty() {
                    write_sadd_command(&mut writer, key, set)?;
                    commands_written += 1;
                }
            }
            ValueObject::ZSet(zset) => {
                if !zset.is_empty() {
                    write_zadd_command(&mut writer, key, zset)?;
                    commands_written += 1;
                }
            }
            ValueObject::Hash(hash) => {
                if !hash.is_empty() {
                    write_hset_command(&mut writer, key, hash)?;
                    commands_written += 1;
                }
            }
            ValueObject::Json(json) => {
                // Write JSON as JSON.SET command
                let json_str = serde_json::to_string(json).unwrap_or_default();
                write_json_set_command(&mut writer, key, &json_str)?;
                commands_written += 1;
            }
            ValueObject::BloomFilter(_bf) => {
                // Bloom filters are not persisted in AOF
                // They should be re-created with BF.RESERVE on restore
                // TODO: Consider adding BF.RESERVE + BF.ADD commands for persistence
            }
            ValueObject::TimeSeries(_ts) => {
                // Time series are not persisted in AOF
                // They should be re-created with TS.CREATE + TS.ADD on restore
                // TODO: Consider adding TS.CREATE + TS.ADD commands for persistence
            }
        }
    }

    // Ensure all data is written and synced
    writer.flush()?;
    writer.get_ref().sync_all()?;
    
    // Atomic rename
    fs::rename(&temp_path, path)?;
    
    info!("AOF rewrite complete: {} commands written", commands_written);
    
    Ok(())
}

// Helper functions for writing commands to AOF

fn write_resp_array<W: Write>(writer: &mut W, elements: &[&str]) -> io::Result<()> {
    write!(writer, "*{}\r\n", elements.len())?;
    for elem in elements {
        write!(writer, "${}\r\n{}\r\n", elem.len(), elem)?;
    }
    Ok(())
}

fn write_set_command<W: Write>(writer: &mut W, key: &str, value: &str) -> io::Result<()> {
    write_resp_array(writer, &["SET", key, value])
}

fn write_rpush_command<W: Write>(writer: &mut W, key: &str, list: &std::collections::LinkedList<String>) -> io::Result<()> {
    let mut elements: Vec<&str> = vec!["RPUSH", key];
    let vals: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
    elements.extend(vals);
    
    write!(writer, "*{}\r\n", elements.len())?;
    for elem in elements {
        write!(writer, "${}\r\n{}\r\n", elem.len(), elem)?;
    }
    Ok(())
}

fn write_sadd_command<W: Write>(writer: &mut W, key: &str, set: &std::collections::HashSet<String>) -> io::Result<()> {
    let mut elements: Vec<&str> = vec!["SADD", key];
    let members: Vec<&str> = set.iter().map(|s| s.as_str()).collect();
    elements.extend(members);
    
    write!(writer, "*{}\r\n", elements.len())?;
    for elem in elements {
        write!(writer, "${}\r\n{}\r\n", elem.len(), elem)?;
    }
    Ok(())
}

fn write_zadd_command<W: Write>(writer: &mut W, key: &str, zset: &ZSet) -> io::Result<()> {
    let scores = zset.all_with_scores();
    let mut parts: Vec<String> = vec!["ZADD".to_string(), key.to_string()];
    
    for (member, score) in scores {
        parts.push(score.to_string());
        parts.push(member);
    }
    
    write!(writer, "*{}\r\n", parts.len())?;
    for part in &parts {
        write!(writer, "${}\r\n{}\r\n", part.len(), part)?;
    }
    Ok(())
}

fn write_hset_command<W: Write>(writer: &mut W, key: &str, hash: &HashMap<String, String>) -> io::Result<()> {
    let mut parts: Vec<String> = vec!["HSET".to_string(), key.to_string()];
    
    for (field, value) in hash {
        parts.push(field.clone());
        parts.push(value.clone());
    }
    
    write!(writer, "*{}\r\n", parts.len())?;
    for part in &parts {
        write!(writer, "${}\r\n{}\r\n", part.len(), part)?;
    }
    Ok(())
}

fn write_json_set_command<W: Write>(writer: &mut W, key: &str, json_str: &str) -> io::Result<()> {
    // JSON.SET key $ value
    let parts = vec!["JSON.SET", key, "$", json_str];
    write!(writer, "*{}\r\n", parts.len())?;
    for part in parts {
        write!(writer, "${}\r\n{}\r\n", part.len(), part)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_is_write_command() {
        assert!(is_write_command("SET"));
        assert!(is_write_command("set"));
        assert!(is_write_command("ZADD"));
        assert!(is_write_command("hset"));
        assert!(!is_write_command("GET"));
        assert!(!is_write_command("ZRANGE"));
    }

    #[test]
    fn test_aof_fsync_policy_from_str() {
        assert_eq!(AofFsyncPolicy::from("always"), AofFsyncPolicy::Always);
        assert_eq!(AofFsyncPolicy::from("everysec"), AofFsyncPolicy::Everysec);
        assert_eq!(AofFsyncPolicy::from("no"), AofFsyncPolicy::No);
        assert_eq!(AofFsyncPolicy::from("invalid"), AofFsyncPolicy::Everysec);
    }

    #[test]
    fn test_format_resp_array() {
        let mut buf = Vec::new();
        write_resp_array(&mut buf, &["SET", "key", "value"]).unwrap();
        let result = String::from_utf8(buf).unwrap();
        assert_eq!(result, "*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n");
    }
}
