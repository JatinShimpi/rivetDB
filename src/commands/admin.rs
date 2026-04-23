use crate::protocol::RespReply;
use crate::storage::{SharedState, estimate_value_size};
use super::ParsedCommand;
use super::expiry::is_expired;

/// STATS - Get server statistics
pub fn stats(state: &SharedState) -> RespReply {
    let mut arr = Vec::new();
    
    arr.push(RespReply::Bulk(Some("expired_keys".into())));
    arr.push(RespReply::Integer(state.get_expired_count() as i64));

    for entry in state.command_count.iter() {
        let cmd = entry.key();
        let count = *entry.value();
        let total_time = state.command_time_ns.get(cmd).map(|e| *e).unwrap_or(0);
        let avg = if count > 0 { total_time / (count as u128) } else { 0 };

        arr.push(RespReply::Bulk(Some(format!("cmd:{}:count", cmd))));
        arr.push(RespReply::Integer(count as i64));

        arr.push(RespReply::Bulk(Some(format!("cmd:{}:avg_ns", cmd))));
        arr.push(RespReply::Integer(avg as i64));
    }

    RespReply::Array(arr)
}

/// HOTKEYS - Get most accessed keys
pub fn hotkeys(state: &SharedState) -> RespReply {
    let mut keys: Vec<_> = state.key_access_count.iter()
        .map(|e| (e.key().clone(), *e.value()))
        .collect();
    keys.sort_by(|a, b| b.1.cmp(&a.1));

    let mut result = Vec::new();
    for (k, v) in keys.into_iter().take(5) {
        result.push(RespReply::Bulk(Some(k)));
        result.push(RespReply::Integer(v as i64));
    }

    RespReply::Array(result)
}

/// MEMORY key - Get memory usage of key
pub fn memory(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 1 {
        return RespReply::Error("MEMORY requires key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Integer(0);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => RespReply::Integer(estimate_value_size(entry.value()) as i64),
        None => RespReply::Integer(0),
    }
}

/// SLOWLOG - Get slow log entries
pub fn slowlog(state: &SharedState) -> RespReply {
    let mut result = Vec::new();

    if let Ok(log) = state.slowlog.lock() {
        for entry in log.iter().rev() {
            result.push(RespReply::Bulk(Some(entry.command.clone())));
            result.push(RespReply::Integer(entry.duration_ns as i64));
        }
    }

    RespReply::Array(result)
}

/// CONFIG GET|SET - Configuration commands
/// Note: CONFIG SET is limited since max_memory and eviction_policy are no longer mutable
pub fn config(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("CONFIG GET|SET".into());
    }

    match cmd.args[0].to_uppercase().as_str() {
        "GET" => {
            match cmd.args[1].to_lowercase().as_str() {
                "maxmemory" => RespReply::Integer(state.max_memory as i64),
                "eviction" => RespReply::Bulk(Some(state.eviction_policy.as_str().into())),
                _ => RespReply::Bulk(None),
            }
        }
        "SET" => {
            // With DashMap, max_memory and eviction_policy are not mutable at runtime
            // They are set at server startup
            RespReply::Error("CONFIG SET not supported with DashMap (set via config file)".into())
        }
        _ => RespReply::Error("CONFIG GET|SET".into()),
    }
}