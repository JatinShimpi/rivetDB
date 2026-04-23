use super::expiry::is_expired;
use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::evict_if_needed;
use crate::storage::{SharedState, ValueObject};
use jsonpath_rust::JsonPath;
use serde_json::{json, Value as JsonValue};
use std::str::FromStr;

/// JSON.SET key path value [NX|XX] - Set JSON value at path
pub fn json_set(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("JSON.SET requires key, path, and value".into());
    }

    let key = &cmd.args[0];
    let path = &cmd.args[1];
    let json_str: String = cmd.args[2..].iter()
        .take_while(|s| !["NX", "XX"].contains(&s.to_uppercase().as_str()))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");

    let nx = cmd.args.iter().any(|s| s.to_uppercase() == "NX");
    let xx = cmd.args.iter().any(|s| s.to_uppercase() == "XX");

    if nx && xx {
        return RespReply::Error("NX and XX are mutually exclusive".into());
    }

    let new_value: JsonValue = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => return RespReply::Error(format!("Invalid JSON: {}", e)),
    };

    state.track_key_access(key);

    let key_exists = state.db.contains_key(key);
    if nx && key_exists { return RespReply::Bulk(None); }
    if xx && !key_exists { return RespReply::Bulk(None); }

    if path == "$" || path == "." {
        state.db.insert(key.clone(), ValueObject::Json(new_value));
        evict_if_needed(state, Some(key));
        return RespReply::Simple("OK".into());
    }

    let result = if let Some(mut entry) = state.db.get_mut(key) {
        match entry.value_mut() {
            ValueObject::Json(existing) => {
                if set_json_path(existing, path, new_value) {
                    Ok(())
                } else {
                    Err("Failed to set path".to_string())
                }
            }
            _ => Err("WRONGTYPE".to_string()),
        }
    } else {
        let mut root = json!({});
        if set_json_path(&mut root, path, new_value) {
            state.db.insert(key.clone(), ValueObject::Json(root));
            Ok(())
        } else {
            Err("Invalid JSON path".to_string())
        }
    };

    match result {
        Ok(()) => {
            evict_if_needed(state, Some(key));
            RespReply::Simple("OK".into())
        }
        Err(e) => RespReply::Error(e),
    }
}

fn set_json_path(root: &mut JsonValue, path: &str, value: JsonValue) -> bool {
    let path = path.trim_start_matches('$').trim_start_matches('.');
    
    if path.is_empty() {
        *root = value;
        return true;
    }

    let parts: Vec<&str> = path.split('.').collect();
    set_json_path_recursive(root, &parts, value)
}

fn set_json_path_recursive(current: &mut JsonValue, parts: &[&str], value: JsonValue) -> bool {
    if parts.is_empty() {
        *current = value;
        return true;
    }

    let part = parts[0];
    let rest = &parts[1..];

    if let Some(bracket_pos) = part.find('[') {
        let field_name = &part[..bracket_pos];
        let index_str = &part[bracket_pos + 1..part.len() - 1];
        let index: usize = match index_str.parse() {
            Ok(i) => i,
            Err(_) => return false,
        };

        if !field_name.is_empty() {
            if !current.is_object() {
                *current = json!({});
            }
            let obj = current.as_object_mut().unwrap();
            if !obj.contains_key(field_name) {
                obj.insert(field_name.to_string(), json!([]));
            }
            let arr_val = obj.get_mut(field_name).unwrap();
            if !arr_val.is_array() {
                *arr_val = json!([]);
            }
            let arr = arr_val.as_array_mut().unwrap();
            while arr.len() <= index {
                arr.push(JsonValue::Null);
            }
            return set_json_path_recursive(&mut arr[index], rest, value);
        } else {
            if !current.is_array() {
                *current = json!([]);
            }
            let arr = current.as_array_mut().unwrap();
            while arr.len() <= index {
                arr.push(JsonValue::Null);
            }
            return set_json_path_recursive(&mut arr[index], rest, value);
        }
    } else {
        if !current.is_object() {
            *current = json!({});
        }
        let obj = current.as_object_mut().unwrap();
        if rest.is_empty() {
            obj.insert(part.to_string(), value);
            true
        } else {
            if !obj.contains_key(part) {
                obj.insert(part.to_string(), json!({}));
            }
            set_json_path_recursive(obj.get_mut(part).unwrap(), rest, value)
        }
    }
}

/// JSON.GET key [path ...] - Get JSON value(s)
pub fn json_get(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("JSON.GET requires at least a key".into());
    }

    let key = &cmd.args[0];

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Json(json) => {
                if cmd.args.len() == 1 {
                    RespReply::Bulk(Some(json.to_string()))
                } else if cmd.args.len() == 2 {
                    let path = &cmd.args[1];
                    match query_json_path(json, path) {
                        Some(result) => RespReply::Bulk(Some(result.to_string())),
                        None => RespReply::Bulk(None),
                    }
                } else {
                    let mut results = Vec::new();
                    for path in &cmd.args[1..] {
                        match query_json_path(json, path) {
                            Some(result) => results.push(RespReply::Bulk(Some(result.to_string()))),
                            None => results.push(RespReply::Bulk(None)),
                        }
                    }
                    RespReply::Array(results)
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

fn query_json_path(json: &JsonValue, path: &str) -> Option<JsonValue> {
    if path == "$" || path == "." {
        return Some(json.clone());
    }

    match JsonPath::from_str(path) {
        Ok(json_path) => {
            let result = json_path.find(json);
            if result.is_null() {
                simple_path_query(json, path)
            } else if let Some(arr) = result.as_array() {
                if arr.is_empty() {
                    simple_path_query(json, path)
                } else if arr.len() == 1 {
                    Some(arr[0].clone())
                } else {
                    Some(result)
                }
            } else {
                Some(result)
            }
        }
        Err(_) => simple_path_query(json, path),
    }
}

fn simple_path_query(json: &JsonValue, path: &str) -> Option<JsonValue> {
    let path = path.trim_start_matches('$').trim_start_matches('.');
    
    if path.is_empty() {
        return Some(json.clone());
    }

    let mut current = json;
    for part in path.split('.') {
        if let Some(bracket_pos) = part.find('[') {
            let field_name = &part[..bracket_pos];
            let index_str = &part[bracket_pos + 1..part.len() - 1];
            let index: usize = index_str.parse().ok()?;

            if !field_name.is_empty() {
                current = current.get(field_name)?;
            }
            current = current.get(index)?;
        } else {
            current = current.get(part)?;
        }
    }

    Some(current.clone())
}

/// JSON.MGET key [key ...] path
pub fn json_mget(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("JSON.MGET requires key(s) and path".into());
    }

    let path = &cmd.args[cmd.args.len() - 1];
    let keys = &cmd.args[..cmd.args.len() - 1];

    let mut results = Vec::new();
    for key in keys {
        if is_expired(state, key) {
            results.push(RespReply::Bulk(None));
            continue;
        }

        state.track_key_access(key);

        match state.db.get(key) {
            Some(entry) => match entry.value() {
                ValueObject::Json(json) => {
                    match query_json_path(json, path) {
                        Some(result) => results.push(RespReply::Bulk(Some(result.to_string()))),
                        None => results.push(RespReply::Bulk(None)),
                    }
                }
                _ => results.push(RespReply::Bulk(None)),
            },
            None => results.push(RespReply::Bulk(None)),
        }
    }

    RespReply::Array(results)
}

/// JSON.DEL key [path]
pub fn json_del(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("JSON.DEL requires key".into());
    }

    let key = &cmd.args[0];
    let path = cmd.args.get(1).map(|s| s.as_str());

    state.track_key_access(key);

    match path {
        None | Some("$") | Some(".") => {
            if state.db.remove(key).is_some() {
                RespReply::Integer(1)
            } else {
                RespReply::Integer(0)
            }
        }
        Some(path) => {
            match state.db.get_mut(key) {
                Some(mut entry) => match entry.value_mut() {
                    ValueObject::Json(json) => {
                        if delete_json_path(json, path) {
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
    }
}

fn delete_json_path(root: &mut JsonValue, path: &str) -> bool {
    let path = path.trim_start_matches('$').trim_start_matches('.');
    let parts: Vec<&str> = path.split('.').collect();
    
    if parts.is_empty() {
        return false;
    }

    let mut current = root;
    for part in &parts[..parts.len() - 1] {
        if let Some(c) = current.get_mut(*part) {
            current = c;
        } else {
            return false;
        }
    }

    let last = parts[parts.len() - 1];
    if let Some(obj) = current.as_object_mut() {
        obj.remove(last).is_some()
    } else if let Some(arr) = current.as_array_mut() {
        if let Ok(idx) = last.parse::<usize>() {
            if idx < arr.len() {
                arr.remove(idx);
                true
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

/// JSON.TYPE key [path]
pub fn json_type(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("JSON.TYPE requires key".into());
    }

    let key = &cmd.args[0];
    let path = cmd.args.get(1).map(|s| s.as_str()).unwrap_or("$");

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Json(json) => {
                let target = if path == "$" || path == "." {
                    json.clone()
                } else {
                    query_json_path(json, path).unwrap_or(JsonValue::Null)
                };

                let type_str = match target {
                    JsonValue::Null => "null",
                    JsonValue::Bool(_) => "boolean",
                    JsonValue::Number(n) => if n.is_i64() { "integer" } else { "number" },
                    JsonValue::String(_) => "string",
                    JsonValue::Array(_) => "array",
                    JsonValue::Object(_) => "object",
                };

                RespReply::Bulk(Some(type_str.to_string()))
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// JSON.ARRAPPEND key path value [value ...]
pub fn json_arrappend(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("JSON.ARRAPPEND requires key, path, and value(s)".into());
    }

    let key = &cmd.args[0];
    let path = &cmd.args[1];
    let values = &cmd.args[2..];

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::Json(json) => {
                let target = if path == "$" || path == "." {
                    json
                } else {
                    match simple_path_get_mut(json, path) {
                        Some(t) => t,
                        None => return RespReply::Error("Path not found".into()),
                    }
                };

                if let Some(arr) = target.as_array_mut() {
                    for val_str in values {
                        match serde_json::from_str(val_str) {
                            Ok(val) => arr.push(val),
                            Err(_) => return RespReply::Error("Invalid JSON value".into()),
                        }
                    }
                    RespReply::Integer(arr.len() as i64)
                } else {
                    RespReply::Error("Value at path is not an array".into())
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

fn simple_path_get_mut<'a>(json: &'a mut JsonValue, path: &str) -> Option<&'a mut JsonValue> {
    let path = path.trim_start_matches('$').trim_start_matches('.');
    
    if path.is_empty() {
        return Some(json);
    }

    let mut current = json;
    for part in path.split('.') {
        current = current.get_mut(part)?;
    }
    Some(current)
}

/// JSON.ARRLEN key [path]
pub fn json_arrlen(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("JSON.ARRLEN requires key".into());
    }

    let key = &cmd.args[0];
    let path = cmd.args.get(1).map(|s| s.as_str()).unwrap_or("$");

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Json(json) => {
                let target = if path == "$" || path == "." {
                    json.clone()
                } else {
                    query_json_path(json, path).unwrap_or(JsonValue::Null)
                };

                match target.as_array() {
                    Some(arr) => RespReply::Integer(arr.len() as i64),
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// JSON.OBJKEYS key [path]
pub fn json_objkeys(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("JSON.OBJKEYS requires key".into());
    }

    let key = &cmd.args[0];
    let path = cmd.args.get(1).map(|s| s.as_str()).unwrap_or("$");

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Json(json) => {
                let target = if path == "$" || path == "." {
                    json.clone()
                } else {
                    query_json_path(json, path).unwrap_or(JsonValue::Null)
                };

                match target.as_object() {
                    Some(obj) => {
                        let keys: Vec<RespReply> = obj.keys()
                            .map(|k| RespReply::Bulk(Some(k.clone())))
                            .collect();
                        RespReply::Array(keys)
                    }
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// JSON.OBJLEN key [path]
pub fn json_objlen(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("JSON.OBJLEN requires key".into());
    }

    let key = &cmd.args[0];
    let path = cmd.args.get(1).map(|s| s.as_str()).unwrap_or("$");

    if is_expired(state, key) {
        return RespReply::Bulk(None);
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::Json(json) => {
                let target = if path == "$" || path == "." {
                    json.clone()
                } else {
                    query_json_path(json, path).unwrap_or(JsonValue::Null)
                };

                match target.as_object() {
                    Some(obj) => RespReply::Integer(obj.len() as i64),
                    None => RespReply::Bulk(None),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// JSON.NUMINCRBY key path value
pub fn json_numincrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 3 {
        return RespReply::Error("JSON.NUMINCRBY requires key, path, and value".into());
    }

    let key = &cmd.args[0];
    let path = &cmd.args[1];
    let increment: f64 = match cmd.args[2].parse() {
        Ok(v) => v,
        Err(_) => return RespReply::Error("Increment is not a valid number".into()),
    };

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::Json(json) => {
                let target = if path == "$" || path == "." {
                    json
                } else {
                    match simple_path_get_mut(json, path) {
                        Some(t) => t,
                        None => return RespReply::Error("Path not found".into()),
                    }
                };

                if let Some(n) = target.as_f64() {
                    let new_val = n + increment;
                    *target = json!(new_val);
                    RespReply::Bulk(Some(new_val.to_string()))
                } else if let Some(n) = target.as_i64() {
                    let new_val = (n as f64) + increment;
                    *target = json!(new_val);
                    RespReply::Bulk(Some(new_val.to_string()))
                } else {
                    RespReply::Error("Value at path is not a number".into())
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}
