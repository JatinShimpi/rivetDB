use crate::protocol::RespReply;
use crate::storage::{SharedState, ValueObject};
use super::ParsedCommand;
use super::expiry::is_expired;

/// TYPE key - Get value type
pub fn type_cmd(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("TYPE requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Simple("none".into());
    }

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::String(_) => RespReply::Simple("string".into()),
            ValueObject::List(_) => RespReply::Simple("list".into()),
            ValueObject::Set(_) => RespReply::Simple("set".into()),
            ValueObject::ZSet(_) => RespReply::Simple("zset".into()),
            ValueObject::Hash(_) => RespReply::Simple("hash".into()),
            ValueObject::Json(_) => RespReply::Simple("json".into()),
            ValueObject::BloomFilter(_) => RespReply::Simple("bloom".into()),
            ValueObject::TimeSeries(_) => RespReply::Simple("timeseries".into()),
        },
        None => RespReply::Simple("none".into()),
    }
}

/// RENAME key newkey - Rename a key
pub fn rename(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("RENAME requires old key and new key".into());
    }

    let old_key = &cmd.args[0];
    let new_key = &cmd.args[1];

    if is_expired(state, old_key) {
        return RespReply::Error("no such key".into());
    }

    match state.db.remove(old_key) {
        Some((_, value)) => {
            state.db.insert(new_key.clone(), value);
            state.key_access_count.remove(old_key);
            state.track_key_access(new_key);
            RespReply::Simple("OK".into())
        }
        None => RespReply::Error("no such key".into()),
    }
}

/// RENAMENX key newkey - Rename key only if new key doesn't exist
pub fn renamenx(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("RENAMENX requires old key and new key".into());
    }

    let old_key = &cmd.args[0];
    let new_key = &cmd.args[1];

    if is_expired(state, old_key) {
        return RespReply::Error("no such key".into());
    }

    if state.db.contains_key(new_key) {
        return RespReply::Integer(0);
    }

    match state.db.remove(old_key) {
        Some((_, value)) => {
            state.db.insert(new_key.clone(), value);
            state.key_access_count.remove(old_key);
            state.track_key_access(new_key);
            RespReply::Integer(1)
        }
        None => RespReply::Error("no such key".into()),
    }
}

/// KEYS pattern - Find keys matching pattern
pub fn keys(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("KEYS requires pattern".into());
    }

    let pattern = &cmd.args[0];

    let mut result = Vec::new();
    for entry in state.db.iter() {
        let key = entry.key();
        if !is_expired(state, key) && simple_match(pattern, key) {
            result.push(RespReply::Bulk(Some(key.clone())));
        }
    }

    RespReply::Array(result)
}

/// Simple pattern matching (supports * wildcard)
fn simple_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            
            if prefix.is_empty() {
                return text.ends_with(suffix);
            } else if suffix.is_empty() {
                return text.starts_with(prefix);
            } else {
                return text.starts_with(prefix) && text.ends_with(suffix) && text.len() >= prefix.len() + suffix.len();
            }
        }
    }

    pattern == text
}

/// SCAN cursor [MATCH pattern] [COUNT count] - Iterate keys
pub fn scan(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("SCAN requires cursor".into());
    }

    let cursor: usize = cmd.args[0].parse().unwrap_or(0);
    
    let mut pattern = "*".to_string();
    let mut count = 10;

    let mut i = 1;
    while i < cmd.args.len() {
        match cmd.args[i].to_uppercase().as_str() {
            "MATCH" if i + 1 < cmd.args.len() => {
                pattern = cmd.args[i + 1].clone();
                i += 2;
            }
            "COUNT" if i + 1 < cmd.args.len() => {
                count = cmd.args[i + 1].parse().unwrap_or(10);
                i += 2;
            }
            _ => i += 1,
        }
    }

    // Collect keys (avoiding expired ones)
    let mut keys: Vec<String> = state.db.iter()
        .filter(|entry| !is_expired(state, entry.key()))
        .map(|entry| entry.key().clone())
        .collect();

    keys.sort();

    let start = cursor.min(keys.len());
    let end = (start + count).min(keys.len());
    let next_cursor = if end < keys.len() { end } else { 0 };

    let mut result_keys = Vec::new();
    for key in &keys[start..end] {
        if simple_match(&pattern, key) {
            result_keys.push(RespReply::Bulk(Some(key.clone())));
        }
    }

    RespReply::Array(vec![
        RespReply::Bulk(Some(next_cursor.to_string())),
        RespReply::Array(result_keys),
    ])
}
