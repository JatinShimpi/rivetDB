//! Namespace commands for RivetDB Multi-Tenancy
//!
//! Provides true multi-tenancy with unlimited named namespaces.
//! Unlike Redis (16 databases 0-15), RivetDB supports:
//! - Unlimited named namespaces
//! - Per-namespace memory limits
//! - Per-namespace metrics
//! - Complete data isolation
//!
//! Commands:
//! - NAMESPACE CREATE name [MAXMEMORY size]
//! - NAMESPACE USE name
//! - NAMESPACE DELETE name
//! - NAMESPACE LIST
//! - NAMESPACE STATS [name]
//! - NAMESPACE CURRENT

use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::namespace::{MultiTenantState, parse_memory_size, format_memory_size, NamespaceError};
use std::sync::Arc;

/// NAMESPACE CREATE name [MAXMEMORY size]
/// Create a new namespace with optional memory limit
pub fn namespace_create(cmd: ParsedCommand, mt_state: &MultiTenantState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("NAMESPACE CREATE requires a name".into());
    }

    let name = &cmd.args[0];
    
    // Parse optional MAXMEMORY
    let mut max_memory = 0usize;
    let mut i = 1;
    while i < cmd.args.len() {
        match cmd.args[i].to_uppercase().as_str() {
            "MAXMEMORY" => {
                if i + 1 >= cmd.args.len() {
                    return RespReply::Error("MAXMEMORY requires a value".into());
                }
                match parse_memory_size(&cmd.args[i + 1]) {
                    Some(size) => max_memory = size,
                    None => return RespReply::Error("Invalid MAXMEMORY value".into()),
                }
                i += 2;
            }
            _ => i += 1,
        }
    }

    match mt_state.create_namespace(name, max_memory) {
        Ok(()) => RespReply::Simple("OK".into()),
        Err(e) => RespReply::Error(e.to_string()),
    }
}

/// NAMESPACE USE name
/// Switch to a namespace (returns namespace reference for connection tracking)
pub fn namespace_use(cmd: ParsedCommand, mt_state: &MultiTenantState) -> (RespReply, Option<String>) {
    if cmd.args.is_empty() {
        return (RespReply::Error("NAMESPACE USE requires a name".into()), None);
    }

    let name = &cmd.args[0];

    match mt_state.get_namespace(name) {
        Some(_ns) => (RespReply::Simple("OK".into()), Some(name.clone())),
        None => (RespReply::Error(format!("Namespace '{}' not found", name)), None),
    }
}

/// NAMESPACE DELETE name
/// Delete a namespace
pub fn namespace_delete(cmd: ParsedCommand, mt_state: &MultiTenantState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("NAMESPACE DELETE requires a name".into());
    }

    let name = &cmd.args[0];

    match mt_state.delete_namespace(name) {
        Ok(()) => RespReply::Simple("OK".into()),
        Err(e) => RespReply::Error(e.to_string()),
    }
}

/// NAMESPACE LIST
/// List all namespaces with their info
pub fn namespace_list(mt_state: &MultiTenantState) -> RespReply {
    let namespaces = mt_state.list_namespaces();
    
    let mut result = Vec::new();
    
    for ns in namespaces {
        let info = vec![
            RespReply::Bulk(Some("name".into())),
            RespReply::Bulk(Some(ns.name)),
            RespReply::Bulk(Some("keys".into())),
            RespReply::Integer(ns.key_count as i64),
            RespReply::Bulk(Some("max_memory".into())),
            RespReply::Bulk(Some(if ns.max_memory > 0 { 
                format_memory_size(ns.max_memory) 
            } else { 
                "unlimited".into() 
            })),
            RespReply::Bulk(Some("used_memory".into())),
            RespReply::Bulk(Some(format_memory_size(ns.current_memory))),
            RespReply::Bulk(Some("commands".into())),
            RespReply::Integer(ns.command_count as i64),
            RespReply::Bulk(Some("uptime".into())),
            RespReply::Integer(ns.uptime_seconds as i64),
        ];
        result.push(RespReply::Array(info));
    }
    
    RespReply::Array(result)
}

/// NAMESPACE STATS [name]
/// Get detailed stats for a specific namespace or current
pub fn namespace_stats(cmd: ParsedCommand, mt_state: &MultiTenantState, current_ns: &str) -> RespReply {
    let name = if cmd.args.is_empty() {
        current_ns
    } else {
        &cmd.args[0]
    };

    match mt_state.get_namespace(name) {
        Some(ns) => {
            let info = ns.info();
            
            let result = vec![
                RespReply::Bulk(Some("name".into())),
                RespReply::Bulk(Some(info.name)),
                RespReply::Bulk(Some("keys".into())),
                RespReply::Integer(info.key_count as i64),
                RespReply::Bulk(Some("max_memory".into())),
                RespReply::Bulk(Some(if info.max_memory > 0 { 
                    format_memory_size(info.max_memory) 
                } else { 
                    "unlimited".into() 
                })),
                RespReply::Bulk(Some("used_memory".into())),
                RespReply::Bulk(Some(format_memory_size(info.current_memory))),
                RespReply::Bulk(Some("memory_usage_percent".into())),
                RespReply::Bulk(Some(if info.max_memory > 0 {
                    format!("{:.2}%", (info.current_memory as f64 / info.max_memory as f64) * 100.0)
                } else {
                    "N/A".into()
                })),
                RespReply::Bulk(Some("commands".into())),
                RespReply::Integer(info.command_count as i64),
                RespReply::Bulk(Some("uptime_seconds".into())),
                RespReply::Integer(info.uptime_seconds as i64),
            ];
            
            RespReply::Array(result)
        }
        None => RespReply::Error(format!("Namespace '{}' not found", name)),
    }
}

/// NAMESPACE CURRENT
/// Show the current namespace
pub fn namespace_current(current_ns: &str) -> RespReply {
    RespReply::Bulk(Some(current_ns.to_string()))
}

/// NAMESPACE SETMEMORY name size
/// Set memory limit for a namespace
pub fn namespace_setmemory(cmd: ParsedCommand, mt_state: &MultiTenantState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("NAMESPACE SETMEMORY requires name and size".into());
    }

    let name = &cmd.args[0];
    let size = match parse_memory_size(&cmd.args[1]) {
        Some(s) => s,
        None => return RespReply::Error("Invalid memory size".into()),
    };

    match mt_state.get_namespace(name) {
        Some(ns) => {
            ns.set_max_memory(size);
            RespReply::Simple("OK".into())
        }
        None => RespReply::Error(format!("Namespace '{}' not found", name)),
    }
}

/// Route NAMESPACE subcommands
pub fn namespace_cmd(cmd: ParsedCommand, mt_state: &MultiTenantState, current_ns: &str) -> (RespReply, Option<String>) {
    if cmd.args.is_empty() {
        return (RespReply::Error("NAMESPACE requires a subcommand: CREATE, USE, DELETE, LIST, STATS, CURRENT".into()), None);
    }

    let subcmd = cmd.args[0].to_uppercase();
    
    // Create a new ParsedCommand with args shifted
    let sub_cmd = ParsedCommand {
        name: subcmd.clone(),
        args: cmd.args[1..].to_vec(),
    };

    match subcmd.as_str() {
        "CREATE" => (namespace_create(sub_cmd, mt_state), None),
        "USE" => namespace_use(sub_cmd, mt_state),
        "DELETE" => (namespace_delete(sub_cmd, mt_state), None),
        "LIST" => (namespace_list(mt_state), None),
        "STATS" => (namespace_stats(sub_cmd, mt_state, current_ns), None),
        "CURRENT" => (namespace_current(current_ns), None),
        "SETMEMORY" => (namespace_setmemory(sub_cmd, mt_state), None),
        _ => (RespReply::Error(format!("Unknown NAMESPACE subcommand: {}", subcmd)), None),
    }
}
