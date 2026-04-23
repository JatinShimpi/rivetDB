use std::collections::LinkedList;
use crate::protocol::RespReply;
use crate::storage::{SharedState, ValueObject};
use crate::storage::evict_if_needed;
use super::ParsedCommand;
use super::expiry::is_expired;

/// LPUSH key value [value ...] - Push to head of list
pub fn lpush(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("LPUSH requires key and value".into());
    }

    let key = &cmd.args[0];
    state.track_key_access(key);

    let new_len = {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::List(LinkedList::new()));

        match entry.value_mut() {
            ValueObject::List(list) => {
                for value in &cmd.args[1..] {
                    list.push_front(value.clone());
                }
                list.len() as i64
            }
            _ => return RespReply::Error("WRONGTYPE Operation against wrong kind of value".into()),
        }
    };

    evict_if_needed(state, Some(key));
    RespReply::Integer(new_len)
}

/// RPUSH key value [value ...] - Push to tail of list
pub fn rpush(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("RPUSH requires key and value".into());
    }

    let key = &cmd.args[0];
    state.track_key_access(key);

    let new_len = {
        let mut entry = state.db.entry(key.clone())
            .or_insert_with(|| ValueObject::List(LinkedList::new()));

        match entry.value_mut() {
            ValueObject::List(list) => {
                for value in &cmd.args[1..] {
                    list.push_back(value.clone());
                }
                list.len() as i64
            }
            _ => return RespReply::Error("WRONGTYPE".into()),
        }
    };

    evict_if_needed(state, Some(key));
    RespReply::Integer(new_len)
}

/// LPOP key [count] - Pop from head
pub fn lpop(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("LPOP requires key".into());
    }

    let key = &cmd.args[0];
    let count: usize = cmd.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    let mut result = Vec::new();
    
    if let Some(mut entry) = state.db.get_mut(key) {
        if let ValueObject::List(list) = entry.value_mut() {
            for _ in 0..count {
                if let Some(val) = list.pop_front() {
                    result.push(RespReply::Bulk(Some(val)));
                } else {
                    break;
                }
            }
        }
    }

    if result.is_empty() {
        RespReply::Bulk(None)
    } else if result.len() == 1 {
        result.pop().unwrap()
    } else {
        RespReply::Array(result)
    }
}

/// RPOP key [count] - Pop from tail
pub fn rpop(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("RPOP requires key".into());
    }

    let key = &cmd.args[0];
    let count: usize = cmd.args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    let mut result = Vec::new();
    
    if let Some(mut entry) = state.db.get_mut(key) {
        if let ValueObject::List(list) = entry.value_mut() {
            for _ in 0..count {
                if let Some(val) = list.pop_back() {
                    result.push(RespReply::Bulk(Some(val)));
                } else {
                    break;
                }
            }
        }
    }

    if result.is_empty() {
        RespReply::Bulk(None)
    } else if result.len() == 1 {
        result.pop().unwrap()
    } else {
        RespReply::Array(result)
    }
}

/// LLEN key - Get list length
pub fn llen(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("LLEN requires one key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::List(list) => RespReply::Integer(list.len() as i64),
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// LRANGE key start stop - Get range of elements
pub fn lrange(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("LRANGE requires key start stop".into());
    }

    let key = &cmd.args[0];
    let start: isize = cmd.args[1].parse().unwrap_or(0);
    let stop: isize = cmd.args[2].parse().unwrap_or(-1);

    if is_expired(state, key) {
        return RespReply::Array(vec![]);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::List(list) => {
                let len = list.len() as isize;
                let s = if start < 0 { len + start } else { start }.max(0);
                let e = if stop < 0 { len + stop } else { stop }.min(len - 1);

                let mut result = Vec::new();
                for (i, val) in list.iter().enumerate() {
                    let i = i as isize;
                    if i >= s && i <= e {
                        result.push(RespReply::Bulk(Some(val.clone())));
                    }
                }
                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// LINDEX key index - Get element by index
pub fn lindex(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("LINDEX requires key and index".into());
    }

    let key = &cmd.args[0];
    let index: isize = match cmd.args[1].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("index is not an integer".into()),
    };

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::List(list) => {
                let len = list.len() as isize;
                let idx = if index < 0 { len + index } else { index };
                
                if idx < 0 || idx >= len {
                    return RespReply::Bulk(None);
                }
                
                match list.iter().nth(idx as usize) {
                    Some(val) => RespReply::Bulk(Some(val.clone())),
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// LSET key index value - Set element at index
pub fn lset(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("LSET requires key, index, and value".into());
    }

    let key = &cmd.args[0];
    let index: isize = match cmd.args[1].parse() {
        Ok(i) => i,
        Err(_) => return RespReply::Error("index is not an integer".into()),
    };
    let value = &cmd.args[2];

    if is_expired(state, key) {
        return RespReply::Error("no such key".into());
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::List(list) => {
                let len = list.len() as isize;
                let idx = if index < 0 { len + index } else { index };
                
                if idx < 0 || idx >= len {
                    return RespReply::Error("index out of range".into());
                }
                
                if let Some(elem) = list.iter_mut().nth(idx as usize) {
                    *elem = value.clone();
                    RespReply::Simple("OK".into())
                } else {
                    RespReply::Error("index out of range".into())
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Error("no such key".into()),
    }
}

/// LREM key count value - Remove elements
pub fn lrem(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("LREM requires key, count, and value".into());
    }

    let key = &cmd.args[0];
    let count: i64 = match cmd.args[1].parse() {
        Ok(c) => c,
        Err(_) => return RespReply::Error("count is not an integer".into()),
    };
    let value = &cmd.args[2];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::List(list) => {
                let mut removed = 0i64;
                let mut new_list = LinkedList::new();
                
                if count == 0 {
                    // Remove all occurrences
                    for elem in list.iter() {
                        if elem != value {
                            new_list.push_back(elem.clone());
                        } else {
                            removed += 1;
                        }
                    }
                } else if count > 0 {
                    // Remove from head
                    for elem in list.iter() {
                        if elem == value && removed < count {
                            removed += 1;
                        } else {
                            new_list.push_back(elem.clone());
                        }
                    }
                } else {
                    // Remove from tail (negative count)
                    let abs_count = count.abs();
                    for elem in list.iter().rev() {
                        if elem == value && removed < abs_count {
                            removed += 1;
                        } else {
                            new_list.push_front(elem.clone());
                        }
                    }
                }
                
                *list = new_list;
                RespReply::Integer(removed)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// LTRIM key start stop - Trim list
pub fn ltrim(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("LTRIM requires key, start, and stop".into());
    }

    let key = &cmd.args[0];
    let start: isize = cmd.args[1].parse().unwrap_or(0);
    let stop: isize = cmd.args[2].parse().unwrap_or(-1);

    if is_expired(state, key) {
        return RespReply::Simple("OK".into());
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::List(list) => {
                let len = list.len() as isize;
                let s = if start < 0 { len + start } else { start }.max(0) as usize;
                let e = if stop < 0 { len + stop } else { stop }.min(len - 1) as usize;
                
                let new_list: LinkedList<String> = list.iter()
                    .enumerate()
                    .filter(|(i, _)| *i >= s && *i <= e)
                    .map(|(_, v)| v.clone())
                    .collect();
                
                *list = new_list;
                RespReply::Simple("OK".into())
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Simple("OK".into()),
    }
}

/// LINSERT key BEFORE|AFTER pivot value - Insert element
pub fn linsert(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 4 {
        return RespReply::Error("LINSERT requires key, BEFORE|AFTER, pivot, and value".into());
    }

    let key = &cmd.args[0];
    let position = cmd.args[1].to_uppercase();
    let pivot = &cmd.args[2];
    let value = &cmd.args[3];

    if position != "BEFORE" && position != "AFTER" {
        return RespReply::Error("position must be BEFORE or AFTER".into());
    }

    if is_expired(state, key) {
        return RespReply::Integer(-1);
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::List(list) => {
                let mut new_list = LinkedList::new();
                let mut inserted = false;
                
                for elem in list.iter() {
                    if elem == pivot && !inserted {
                        if position == "BEFORE" {
                            new_list.push_back(value.clone());
                            new_list.push_back(elem.clone());
                        } else {
                            new_list.push_back(elem.clone());
                            new_list.push_back(value.clone());
                        }
                        inserted = true;
                    } else {
                        new_list.push_back(elem.clone());
                    }
                }
                
                if inserted {
                    *list = new_list;
                    RespReply::Integer(list.len() as i64)
                } else {
                    RespReply::Integer(-1)
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// LPOS key element [RANK rank] [COUNT count] [MAXLEN maxlen] - Find position
pub fn lpos(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("LPOS requires key and element".into());
    }

    let key = &cmd.args[0];
    let element = &cmd.args[1];

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::List(list) => {
                for (i, val) in list.iter().enumerate() {
                    if val == element {
                        return RespReply::Integer(i as i64);
                    }
                }
                RespReply::Bulk(None)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}