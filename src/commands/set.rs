use std::collections::HashSet;
use crate::protocol::RespReply;
use crate::storage::{SharedState, ValueObject};
use crate::storage::evict_if_needed;
use super::ParsedCommand;
use super::expiry::is_expired;

/// SADD key member [member ...] - Add members to set
pub fn sadd(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("SADD requires key and members".into());
    }

    let key = &cmd.args[0];
    let members = &cmd.args[1..];

    if is_expired(state, key) {
        state.db.remove(key);
    }

    state.track_key_access(key);

    let added = {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::Set(HashSet::new()));

        match entry.value_mut() {
            ValueObject::Set(s) => {
                let mut added = 0;
                for m in members {
                    if s.insert(m.clone()) {
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

/// SREM key member [member ...] - Remove members
pub fn srem(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("SREM requires key and members".into());
    }

    let key = &cmd.args[0];
    let members = &cmd.args[1..];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::Set(s) => {
                let mut removed = 0;
                for m in members {
                    if s.remove(m) {
                        removed += 1;
                    }
                }
                RespReply::Integer(removed)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// SMEMBERS key - Get all members
pub fn smembers(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("SMEMBERS requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Set(s) => {
                let members = s.iter().map(|v| RespReply::Bulk(Some(v.clone()))).collect();
                RespReply::Array(members)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// SISMEMBER key member - Test if member exists
pub fn sismember(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("SISMEMBER requires key and member".into());
    }

    let key = &cmd.args[0];
    let member = &cmd.args[1];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Set(s) => {
                if s.contains(member) {
                    RespReply::Integer(1)
                } else {
                    RespReply::Integer(0)
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// SCARD key - Get set cardinality
pub fn scard(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("SCARD requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Set(s) => RespReply::Integer(s.len() as i64),
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// SUNION key [key ...] - Union of sets
pub fn sunion(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("SUNION requires at least one key".into());
    }

    let mut result_set = HashSet::new();

    for key in &cmd.args {
        if is_expired(state, key) {
            continue;
        }

        state.track_key_access(key);

        if let Some(entry) = state.db.get(key) {
            if let ValueObject::Set(s) = entry.value() {
                result_set.extend(s.iter().cloned());
            }
        }
    }

    let members = result_set.iter().map(|v| RespReply::Bulk(Some(v.clone()))).collect();
    RespReply::Array(members)
}

/// SINTER key [key ...] - Intersection of sets
pub fn sinter(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("SINTER requires at least one key".into());
    }

    let first_key = &cmd.args[0];
    if is_expired(state, first_key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(first_key);

    let mut result_set: HashSet<String> = match state.db.get(first_key) {
        Some(entry) => match entry.value() {
            ValueObject::Set(s) => s.clone(),
            _ => return RespReply::Error("WRONGTYPE".into()),
        },
        None => return RespReply::Array(vec![]),
    };

    for key in &cmd.args[1..] {
        if is_expired(state, key) {
            return RespReply::Array(vec![]);
        }

        state.track_key_access(key);

        match state.db.get(key) {
            Some(entry) => match entry.value() {
                ValueObject::Set(s) => {
                    result_set.retain(|e| s.contains(e));
                }
                _ => return RespReply::Error("WRONGTYPE".into()),
            },
            None => return RespReply::Array(vec![]),
        }
    }

    let members = result_set.iter().map(|v| RespReply::Bulk(Some(v.clone()))).collect();
    RespReply::Array(members)
}

/// SDIFF key [key ...] - Difference of sets
pub fn sdiff(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("SDIFF requires at least one key".into());
    }

    let first_key = &cmd.args[0];
    if is_expired(state, first_key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(first_key);

    let mut result_set: HashSet<String> = match state.db.get(first_key) {
        Some(entry) => match entry.value() {
            ValueObject::Set(s) => s.clone(),
            _ => return RespReply::Error("WRONGTYPE".into()),
        },
        None => return RespReply::Array(vec![]),
    };

    for key in &cmd.args[1..] {
        if is_expired(state, key) {
            continue;
        }

        state.track_key_access(key);

        if let Some(entry) = state.db.get(key) {
            if let ValueObject::Set(s) = entry.value() {
                for elem in s.iter() {
                    result_set.remove(elem);
                }
            }
        }
    }

    let members = result_set.iter().map(|v| RespReply::Bulk(Some(v.clone()))).collect();
    RespReply::Array(members)
}