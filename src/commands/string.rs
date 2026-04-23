use super::expiry::is_expired;
use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::evict_if_needed;
use crate::storage::{SharedState, ValueObject};

/// SET key value - Set string value
pub fn set(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("SET requires key and value".into());
    }

    let key = &cmd.args[0];
    let value = &cmd.args[1];

    // Track access (lock-free)
    state.track_key_access(key);

    // Insert directly into DashMap (lock-free!)
    state.db.insert(key.clone(), ValueObject::String(value.clone()));

    // Check eviction
    evict_if_needed(state, Some(key));

    RespReply::Simple("OK".into())
}

/// GET key - Get string value
pub fn get(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("GET requires key".into());
    }

    let key = &cmd.args[0];

    // Check expiry
    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    // Track access
    state.track_key_access(key);

    // Get from DashMap (lock-free!)
    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::String(v) => RespReply::Bulk(Some(v.clone())),
            _ => RespReply::Error("WRONGTYPE Operation against wrong kind of value".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// INCR key - Increment integer value
pub fn incr(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("INCR requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        state.db.insert(key.clone(), ValueObject::String("0".into()));
    }

    state.track_key_access(key);

    // Use entry API for atomic update - need separate binding for lifetime
    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String("0".into()));
    let result = entry.value_mut();
    
    match result {
        ValueObject::String(s) => {
            match s.parse::<i64>() {
                Ok(n) => {
                    let new_val = n + 1;
                    *s = new_val.to_string();
                    RespReply::Integer(new_val)
                }
                Err(_) => RespReply::Error("value is not an integer".into()),
            }
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// DECR key - Decrement integer value
pub fn decr(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("DECR requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        state.db.insert(key.clone(), ValueObject::String("0".into()));
    }

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String("0".into()));
    let result = entry.value_mut();
    
    match result {
        ValueObject::String(s) => {
            match s.parse::<i64>() {
                Ok(n) => {
                    let new_val = n - 1;
                    *s = new_val.to_string();
                    RespReply::Integer(new_val)
                }
                Err(_) => RespReply::Error("value is not an integer".into()),
            }
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// INCRBY key increment - Increment by specific amount
pub fn incrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("INCRBY requires key and increment".into());
    }

    let key = &cmd.args[0];
    let increment: i64 = match cmd.args[1].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("value is not an integer".into()),
    };

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String("0".into()));
    let result = entry.value_mut();
    
    match result {
        ValueObject::String(s) => {
            match s.parse::<i64>() {
                Ok(n) => {
                    let new_val = n + increment;
                    *s = new_val.to_string();
                    RespReply::Integer(new_val)
                }
                Err(_) => RespReply::Error("value is not an integer".into()),
            }
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// DECRBY key decrement - Decrement by specific amount
pub fn decrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("DECRBY requires key and decrement".into());
    }

    let key = &cmd.args[0];
    let decrement: i64 = match cmd.args[1].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("value is not an integer".into()),
    };

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String("0".into()));
    let result = entry.value_mut();
    
    match result {
        ValueObject::String(s) => {
            match s.parse::<i64>() {
                Ok(n) => {
                    let new_val = n - decrement;
                    *s = new_val.to_string();
                    RespReply::Integer(new_val)
                }
                Err(_) => RespReply::Error("value is not an integer".into()),
            }
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// INCRBYFLOAT key increment - Increment by float
pub fn incrbyfloat(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("INCRBYFLOAT requires key and increment".into());
    }

    let key = &cmd.args[0];
    let increment: f64 = match cmd.args[1].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("value is not a valid float".into()),
    };

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String("0".into()));
    let result = entry.value_mut();
    
    match result {
        ValueObject::String(s) => {
            match s.parse::<f64>() {
                Ok(n) => {
                    let new_val = n + increment;
                    *s = new_val.to_string();
                    RespReply::Bulk(Some(new_val.to_string()))
                }
                Err(_) => RespReply::Error("value is not a valid float".into()),
            }
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// APPEND key value - Append to string
pub fn append(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("APPEND requires key and value".into());
    }

    let key = &cmd.args[0];
    let value = &cmd.args[1];

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String(String::new()));
    
    match entry.value_mut() {
        ValueObject::String(s) => {
            s.push_str(value);
            RespReply::Integer(s.len() as i64)
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// STRLEN key - Get string length
pub fn strlen(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("STRLEN requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::String(s) => RespReply::Integer(s.len() as i64),
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// GETRANGE key start end - Get substring
pub fn getrange(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("GETRANGE requires key, start, and end".into());
    }

    let key = &cmd.args[0];
    let start: i64 = match cmd.args[1].parse() {
        Ok(s) => s,
        Err(_) => return RespReply::Error("value is not an integer".into()),
    };
    let end: i64 = match cmd.args[2].parse() {
        Ok(e) => e,
        Err(_) => return RespReply::Error("value is not an integer".into()),
    };

    if is_expired(state, key) {
        return RespReply::Bulk(Some(String::new()));
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::String(s) => {
                let len = s.len() as i64;
                let start = if start < 0 { (len + start).max(0) as usize } else { start as usize };
                let end = if end < 0 { (len + end).max(0) as usize } else { (end as usize).min(s.len().saturating_sub(1)) };
                
                if start > end || start >= s.len() {
                    return RespReply::Bulk(Some(String::new()));
                }
                
                RespReply::Bulk(Some(s[start..=end].to_string()))
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(Some(String::new())),
    }
}

/// SETRANGE key offset value - Overwrite part of string
pub fn setrange(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("SETRANGE requires key, offset, and value".into());
    }

    let key = &cmd.args[0];
    let offset: usize = match cmd.args[1].parse() {
        Ok(o) if o >= 0 => o,
        _ => return RespReply::Error("offset is out of range".into()),
    };
    let value = &cmd.args[2];

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::String(String::new()));
    
    match entry.value_mut() {
        ValueObject::String(s) => {
            // Pad with zeros if needed
            while s.len() < offset {
                s.push('\0');
            }
            
            // Replace or append
            if offset < s.len() {
                let end = (offset + value.len()).min(s.len());
                s.replace_range(offset..end, &value[..end-offset]);
                if offset + value.len() > s.len() {
                    s.push_str(&value[end-offset..]);
                }
            } else {
                s.push_str(value);
            }
            
            RespReply::Integer(s.len() as i64)
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// MGET key [key ...] - Get multiple keys
pub fn mget(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("MGET requires at least one key".into());
    }

    let results: Vec<RespReply> = cmd.args.iter()
        .map(|key| {
            if is_expired(state, key) {
                return RespReply::Bulk(None);
            }
            
            state.track_key_access(key);
            
            match state.db.get(key) {
                Some(entry) => match entry.value() {
                    ValueObject::String(v) => RespReply::Bulk(Some(v.clone())),
                    _ => RespReply::Bulk(None),
                },
                None => RespReply::Bulk(None),
            }
        })
        .collect();

    RespReply::Array(results)
}

/// MSET key value [key value ...] - Set multiple keys
pub fn mset(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 || cmd.args.len() % 2 != 0 {
        return RespReply::Error("MSET requires pairs of key-value".into());
    }

    for chunk in cmd.args.chunks(2) {
        let key = &chunk[0];
        let value = &chunk[1];
        
        state.track_key_access(key);
        state.db.insert(key.clone(), ValueObject::String(value.clone()));
    }

    RespReply::Simple("OK".into())
}

/// SETNX key value - Set if not exists
pub fn setnx(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("SETNX requires key and value".into());
    }

    let key = &cmd.args[0];
    let value = &cmd.args[1];

    // Check if key exists
    if state.db.contains_key(key) && !is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);
    state.db.insert(key.clone(), ValueObject::String(value.clone()));
    
    RespReply::Integer(1)
}

/// SETEX key seconds value - Set with expiry
pub fn setex(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("SETEX requires key, seconds, and value".into());
    }

    let key = &cmd.args[0];
    let seconds: u64 = match cmd.args[1].parse() {
        Ok(s) => s,
        Err(_) => return RespReply::Error("invalid expire time".into()),
    };
    let value = &cmd.args[2];

    state.track_key_access(key);
    state.db.insert(key.clone(), ValueObject::String(value.clone()));

    // Set expiry
    use std::cmp::Reverse;
    use std::time::{Duration, Instant};
    
    let expire_at = Instant::now() + Duration::from_secs(seconds);
    if let Ok(mut expiries) = state.expiries.lock() {
        expiries.push(Reverse((expire_at, key.clone())));
    }

    RespReply::Simple("OK".into())
}

/// GETSET key value - Set and return old value (deprecated, use GETEX)
pub fn getset(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("GETSET requires key and value".into());
    }

    let key = &cmd.args[0];
    let value = &cmd.args[1];

    state.track_key_access(key);

    // Get old value and set new
    let old = state.db.insert(key.clone(), ValueObject::String(value.clone()));

    match old {
        Some(ValueObject::String(v)) => RespReply::Bulk(Some(v)),
        Some(_) => RespReply::Error("WRONGTYPE".into()),
        None => RespReply::Bulk(None),
    }
}
