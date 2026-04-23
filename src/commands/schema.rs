use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::schema::{Schema, TypedValue, validator};
use crate::storage::{SharedState, ValueObject};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Global schema registry
pub type SchemaRegistry = Arc<Mutex<HashMap<String, Schema>>>;

/// Create a new schema registry
pub fn create_schema_registry() -> SchemaRegistry {
    Arc::new(Mutex::new(HashMap::new()))
}

/// SCHEMA DEFINE name {definition}
pub fn schema_define(cmd: ParsedCommand, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("SCHEMA DEFINE requires name and definition".into());
    }

    if cmd.args[0].to_uppercase() != "DEFINE" {
        return RespReply::Error(format!("Unknown SCHEMA subcommand: {}", cmd.args[0]));
    }

    let schema_name = &cmd.args[1];
    let definition: String = cmd.args[2..].join(" ");

    match Schema::from_definition(schema_name, &definition) {
        Ok(schema) => {
            let mut guard = registry.lock().unwrap();
            let existed = guard.insert(schema_name.clone(), schema).is_some();
            
            if existed {
                RespReply::Simple(format!("OK (updated schema '{}')", schema_name))
            } else {
                RespReply::Simple(format!("OK (created schema '{}')", schema_name))
            }
        }
        Err(e) => RespReply::Error(format!("Schema definition error: {}", e)),
    }
}

/// SCHEMA LIST
pub fn schema_list(registry: &SchemaRegistry) -> RespReply {
    let guard = registry.lock().unwrap();
    let schemas: Vec<RespReply> = guard.keys()
        .map(|name| RespReply::Bulk(Some(name.clone())))
        .collect();
    RespReply::Array(schemas)
}

/// SCHEMA GET name
pub fn schema_get(cmd: ParsedCommand, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("SCHEMA GET requires schema name".into());
    }

    if cmd.args[0].to_uppercase() != "GET" {
        return RespReply::Error(format!("Unknown SCHEMA subcommand: {}", cmd.args[0]));
    }

    let schema_name = &cmd.args[1];
    let guard = registry.lock().unwrap();

    match guard.get(schema_name) {
        Some(schema) => RespReply::Bulk(Some(schema.to_definition())),
        None => RespReply::Bulk(None),
    }
}

/// SCHEMA DROP name
pub fn schema_drop(cmd: ParsedCommand, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("SCHEMA DROP requires schema name".into());
    }

    if cmd.args[0].to_uppercase() != "DROP" {
        return RespReply::Error(format!("Unknown SCHEMA subcommand: {}", cmd.args[0]));
    }

    let schema_name = &cmd.args[1];
    let mut guard = registry.lock().unwrap();

    if guard.remove(schema_name).is_some() {
        RespReply::Integer(1)
    } else {
        RespReply::Integer(0)
    }
}

/// Route SCHEMA subcommands
pub fn schema_cmd(cmd: ParsedCommand, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("SCHEMA requires: DEFINE, LIST, GET, DROP".into());
    }

    match cmd.args[0].to_uppercase().as_str() {
        "DEFINE" => schema_define(cmd, registry),
        "LIST" => schema_list(registry),
        "GET" => schema_get(cmd, registry),
        "DROP" => schema_drop(cmd, registry),
        _ => RespReply::Error(format!("Unknown SCHEMA subcommand: {}", cmd.args[0])),
    }
}

/// TSET key json_value - Set typed value (validates against schema)
pub fn tset(cmd: ParsedCommand, state: &SharedState, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("TSET requires key and JSON value".into());
    }

    let key = &cmd.args[0];
    let json_str: String = cmd.args[1..].join(" ");

    let schema_name = match key.split(':').next() {
        Some(name) => name,
        None => return RespReply::Error("Key must have format schema:id".into()),
    };

    let schema = {
        let guard = registry.lock().unwrap();
        match guard.get(schema_name) {
            Some(s) => s.clone(),
            None => return RespReply::Error(format!("Schema '{}' not defined", schema_name)),
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => return RespReply::Error(format!("Invalid JSON: {}", e)),
    };

    if let Err(errors) = validator::validate(&value, &schema) {
        let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        return RespReply::Error(format!("Validation failed: {}", error_msgs.join("; ")));
    }

    let typed_value = TypedValue::new(schema_name.to_string(), value);
    let json_store = serde_json::to_string(&typed_value).unwrap_or_default();

    state.track_key_access(key);
    state.db.insert(key.clone(), ValueObject::String(json_store));

    RespReply::Simple("OK".into())
}

/// TGET key [field] - Get typed value
pub fn tget(cmd: ParsedCommand, state: &SharedState, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("TGET requires key".into());
    }

    let key = &cmd.args[0];
    let field = cmd.args.get(1);

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::String(json_str) => {
                let typed_value: TypedValue = match serde_json::from_str(json_str) {
                    Ok(v) => v,
                    Err(_) => return RespReply::Bulk(Some(json_str.clone())),
                };

                {
                    let reg_guard = registry.lock().unwrap();
                    if !reg_guard.contains_key(&typed_value.schema_name) {
                        return RespReply::Error(format!(
                            "Schema '{}' no longer exists",
                            typed_value.schema_name
                        ));
                    }
                }

                if let Some(field_name) = field {
                    match typed_value.get(field_name) {
                        Some(val) => RespReply::Bulk(Some(val.to_string())),
                        None => RespReply::Bulk(None),
                    }
                } else {
                    RespReply::Bulk(Some(typed_value.to_json()))
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// TVALIDATE schema_name json_value
pub fn tvalidate(cmd: ParsedCommand, registry: &SchemaRegistry) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("TVALIDATE requires schema name and JSON".into());
    }

    let schema_name = &cmd.args[0];
    let json_str: String = cmd.args[1..].join(" ");

    let schema = {
        let guard = registry.lock().unwrap();
        match guard.get(schema_name) {
            Some(s) => s.clone(),
            None => return RespReply::Error(format!("Schema '{}' not defined", schema_name)),
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => return RespReply::Error(format!("Invalid JSON: {}", e)),
    };

    match validator::validate(&value, &schema) {
        Ok(()) => RespReply::Simple("OK".into()),
        Err(errors) => {
            let error_msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            RespReply::Error(format!("Validation failed: {}", error_msgs.join("; ")))
        }
    }
}

/// TKEYS pattern - Get typed keys
pub fn tkeys(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    let pattern = cmd.args.get(0).map(|s| s.as_str()).unwrap_or("*");
    
    let mut keys: Vec<RespReply> = Vec::new();
    
    for entry in state.db.iter() {
        let key = entry.key();
        if let ValueObject::String(json_str) = entry.value() {
            if serde_json::from_str::<TypedValue>(json_str).is_ok() {
                if pattern == "*" || key.starts_with(pattern.trim_end_matches('*')) {
                    keys.push(RespReply::Bulk(Some(key.clone())));
                }
            }
        }
    }
    
    RespReply::Array(keys)
}
