use std::cmp::Reverse;
use std::time::{Duration, Instant};
use crate::protocol::RespReply;
use crate::storage::SharedState;
use super::ParsedCommand;

/// Check if a key is expired (DashMap version)
/// Returns true if the key was expired and removed
pub fn is_expired(state: &SharedState, key: &str) -> bool {
    let now = Instant::now();
    
    // Check expiry heap
    let expired = {
        if let Ok(expiries) = state.expiries.lock() {
            expiries.iter().any(|Reverse((t, k))| k == key && *t <= now)
        } else {
            false
        }
    };

    if expired {
        state.db.remove(key);
        state.key_access_count.remove(key);
        state.expired_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    expired
}

/// EXPIRE key seconds - Set a timeout on key
pub fn expire(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("EXPIRE requires key and seconds".into());
    }

    let key = &cmd.args[0];
    let seconds: u64 = match cmd.args[1].parse() {
        Ok(s) => s,
        Err(_) => return RespReply::Error("invalid expire time".into()),
    };

    // Check if key exists (lock-free)
    if !state.db.contains_key(key) {
        return RespReply::Integer(0);
    }

    // Add to expiry heap (needs lock)
    let expire_at = Instant::now() + Duration::from_secs(seconds);
    if let Ok(mut expiries) = state.expiries.lock() {
        expiries.push(Reverse((expire_at, key.clone())));
    }

    RespReply::Integer(1)
}

/// TTL key - Get the time to live for a key
pub fn ttl(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("TTL requires one key".into());
    }

    let key = &cmd.args[0];

    // Check if key exists (lock-free)
    if !state.db.contains_key(key) {
        return RespReply::Integer(-2);
    }

    let now = Instant::now();
    
    // Find TTL in expiry heap
    let ttl = if let Ok(expiries) = state.expiries.lock() {
        let mut found_ttl = None;
        for Reverse((t, k)) in expiries.iter() {
            if k == key {
                if *t <= now {
                    // Key is expired - remove it
                    state.db.remove(key);
                    state.expired_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return RespReply::Integer(-2);
                }
                found_ttl = Some(t.saturating_duration_since(now).as_secs() as i64);
                break;
            }
        }
        found_ttl
    } else {
        None
    };

    match ttl {
        Some(v) => RespReply::Integer(v),
        None => RespReply::Integer(-1), // No expiry set
    }
}