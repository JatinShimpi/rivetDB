use super::expiry::is_expired;
use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::evict_if_needed;
use crate::storage::{SharedState, ValueObject};
use std::collections::HashMap;

/// HSET key field value [field value ...] - Set hash field(s)
pub fn hset(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 || cmd.args.len() % 2 == 0 {
        return RespReply::Error("HSET requires key and field-value pairs".into());
    }

    let key = &cmd.args[0];
    let pairs = &cmd.args[1..];

    state.track_key_access(key);

    let added = {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::Hash(HashMap::new()));

        match entry.value_mut() {
            ValueObject::Hash(hash) => {
                let mut added = 0;
                for chunk in pairs.chunks(2) {
                    if hash.insert(chunk[0].clone(), chunk[1].clone()).is_none() {
                        added += 1;
                    }
                }
                added
            }
            _ => return RespReply::Error("WRONGTYPE".into()),
        }
    };

    evict_if_needed(state, Some(key));
    RespReply::Integer(added)
}

/// HSETNX key field value - Set hash field only if it doesn't exist
pub fn hsetnx(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("HSETNX requires key, field, and value".into());
    }

    let key = &cmd.args[0];
    let field = &cmd.args[1];
    let value = &cmd.args[2];

    state.track_key_access(key);

    let result = {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::Hash(HashMap::new()));

        match entry.value_mut() {
            ValueObject::Hash(hash) => {
                if hash.contains_key(field) {
                    0
                } else {
                    hash.insert(field.clone(), value.clone());
                    1
                }
            }
            _ => return RespReply::Error("WRONGTYPE".into()),
        }
    };

    evict_if_needed(state, Some(key));
    RespReply::Integer(result)
}

/// HMSET key field value [field value ...] - Set multiple hash fields
pub fn hmset(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 || cmd.args.len() % 2 == 0 {
        return RespReply::Error("HMSET requires key and field-value pairs".into());
    }

    let key = &cmd.args[0];
    let pairs = &cmd.args[1..];

    state.track_key_access(key);

    {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::Hash(HashMap::new()));

        match entry.value_mut() {
            ValueObject::Hash(hash) => {
                for chunk in pairs.chunks(2) {
                    hash.insert(chunk[0].clone(), chunk[1].clone());
                }
            }
            _ => return RespReply::Error("WRONGTYPE".into()),
        }
    }

    evict_if_needed(state, Some(key));
    RespReply::Simple("OK".into())
}

/// HGET key field - Get hash field value
pub fn hget(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("HGET requires key and field".into());
    }

    let key = &cmd.args[0];
    let field = &cmd.args[1];

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                match hash.get(field) {
                    Some(value) => RespReply::Bulk(Some(value.clone())),
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// HMGET key field [field ...] - Get multiple hash field values
pub fn hmget(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("HMGET requires key and at least one field".into());
    }

    let key = &cmd.args[0];
    let fields = &cmd.args[1..];

    if is_expired(state, key) {
        return RespReply::Array(fields.iter().map(|_| RespReply::Bulk(None)).collect());
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                let values: Vec<RespReply> = fields.iter()
                    .map(|f| match hash.get(f) {
                        Some(v) => RespReply::Bulk(Some(v.clone())),
                        None => RespReply::Bulk(None),
                    })
                    .collect();
                RespReply::Array(values)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(fields.iter().map(|_| RespReply::Bulk(None)).collect()),
    }
}

/// HDEL key field [field ...] - Delete hash fields
pub fn hdel(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("HDEL requires key and at least one field".into());
    }

    let key = &cmd.args[0];
    let fields = &cmd.args[1..];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::Hash(hash) => {
                let mut deleted = 0;
                for field in fields {
                    if hash.remove(field).is_some() {
                        deleted += 1;
                    }
                }
                RespReply::Integer(deleted)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// HEXISTS key field - Check if field exists
pub fn hexists(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("HEXISTS requires key and field".into());
    }

    let key = &cmd.args[0];
    let field = &cmd.args[1];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                if hash.contains_key(field) { RespReply::Integer(1) } else { RespReply::Integer(0) }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// HGETALL key - Get all fields and values
pub fn hgetall(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("HGETALL requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                let mut result = Vec::new();
                for (field, value) in hash {
                    result.push(RespReply::Bulk(Some(field.clone())));
                    result.push(RespReply::Bulk(Some(value.clone())));
                }
                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// HLEN key - Get number of fields
pub fn hlen(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("HLEN requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => RespReply::Integer(hash.len() as i64),
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// HINCRBY key field increment - Increment hash field
pub fn hincrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("HINCRBY requires key, field, and increment".into());
    }

    let key = &cmd.args[0];
    let field = &cmd.args[1];
    let increment: i64 = match cmd.args[2].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("value is not an integer".into()),
    };

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::Hash(HashMap::new()));

    match entry.value_mut() {
        ValueObject::Hash(hash) => {
            let current: i64 = hash.get(field)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            
            let new_val = current + increment;
            hash.insert(field.clone(), new_val.to_string());
            RespReply::Integer(new_val)
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// HINCRBYFLOAT key field increment - Increment hash field by float
pub fn hincrbyfloat(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("HINCRBYFLOAT requires key, field, and increment".into());
    }

    let key = &cmd.args[0];
    let field = &cmd.args[1];
    let increment: f64 = match cmd.args[2].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("value is not a valid float".into()),
    };

    state.track_key_access(key);

    let mut entry = state.db.entry(key.clone())
        .or_insert_with(|| ValueObject::Hash(HashMap::new()));

    match entry.value_mut() {
        ValueObject::Hash(hash) => {
            let current: f64 = hash.get(field)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0.0);
            
            let new_val = current + increment;
            hash.insert(field.clone(), new_val.to_string());
            RespReply::Bulk(Some(new_val.to_string()))
        }
        _ => RespReply::Error("WRONGTYPE".into()),
    }
}

/// HKEYS key - Get all fields
pub fn hkeys(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("HKEYS requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                let keys: Vec<RespReply> = hash.keys()
                    .map(|k| RespReply::Bulk(Some(k.clone())))
                    .collect();
                RespReply::Array(keys)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// HVALS key - Get all values
pub fn hvals(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("HVALS requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                let vals: Vec<RespReply> = hash.values()
                    .map(|v| RespReply::Bulk(Some(v.clone())))
                    .collect();
                RespReply::Array(vals)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// HSCAN key cursor [MATCH pattern] [COUNT count] - Iterate hash fields
pub fn hscan(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("HSCAN requires key and cursor".into());
    }

    let key = &cmd.args[0];
    let cursor: usize = cmd.args[1].parse().unwrap_or(0);
    let count = 10; // Default count

    if is_expired(state, key) {
        return RespReply::Array(vec![
            RespReply::Bulk(Some("0".to_string())),
            RespReply::Array(vec![]),
        ]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Hash(hash) => {
                let entries: Vec<(&String, &String)> = hash.iter().collect();
                let start = cursor.min(entries.len());
                let end = (start + count).min(entries.len());
                let next_cursor = if end < entries.len() { end } else { 0 };

                let mut result = Vec::new();
                for (field, value) in &entries[start..end] {
                    result.push(RespReply::Bulk(Some((*field).clone())));
                    result.push(RespReply::Bulk(Some((*value).clone())));
                }

                RespReply::Array(vec![
                    RespReply::Bulk(Some(next_cursor.to_string())),
                    RespReply::Array(result),
                ])
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![
            RespReply::Bulk(Some("0".to_string())),
            RespReply::Array(vec![]),
        ]),
    }
}
