//! Time-Series commands for RivetDB
//!
//! Built-in time-series support - no external module needed (unlike Redis!)
//!
//! Commands:
//! - TS.CREATE key [RETENTION ms] [LABELS label value ...]
//! - TS.ADD key timestamp value
//! - TS.GET key - get latest value
//! - TS.RANGE key fromTimestamp toTimestamp [AGGREGATION type timeBucket]
//! - TS.MRANGE fromTimestamp toTimestamp FILTER filter...
//! - TS.INFO key - get time series info
//! - TS.DEL key fromTimestamp toTimestamp

use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::{SharedState, ValueObject, TimeSeries};
use crate::storage::timeseries::{parse_timestamp, Aggregation};

/// TS.CREATE key [RETENTION ms] [LABELS label value ...]
/// Create a new time series
pub fn ts_create(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("TS.CREATE requires key".into());
    }

    let key = &cmd.args[0];
    
    // Check if key already exists
    if state.db.contains_key(key) {
        return RespReply::Error("key already exists".into());
    }

    let mut ts = TimeSeries::new();
    
    // Parse options
    let mut i = 1;
    while i < cmd.args.len() {
        match cmd.args[i].to_uppercase().as_str() {
            "RETENTION" => {
                if i + 1 >= cmd.args.len() {
                    return RespReply::Error("RETENTION requires a value".into());
                }
                match cmd.args[i + 1].parse::<i64>() {
                    Ok(ms) if ms > 0 => ts.set_retention(Some(ms)),
                    _ => return RespReply::Error("RETENTION must be a positive integer".into()),
                }
                i += 2;
            }
            "LABELS" => {
                i += 1;
                while i + 1 < cmd.args.len() {
                    // Check if we hit another keyword
                    if cmd.args[i].to_uppercase() == "RETENTION" {
                        break;
                    }
                    let label_key = cmd.args[i].clone();
                    let label_value = cmd.args[i + 1].clone();
                    ts.add_label(label_key, label_value);
                    i += 2;
                }
            }
            _ => {
                return RespReply::Error(format!("Unknown option: {}", cmd.args[i]));
            }
        }
    }

    state.db.insert(key.clone(), ValueObject::TimeSeries(ts));
    RespReply::Simple("OK".into())
}

/// TS.ADD key timestamp value
/// Add a data point to a time series
/// timestamp can be "*" for auto-timestamp, or a specific ms timestamp
pub fn ts_add(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("TS.ADD requires key, timestamp, and value".into());
    }

    let key = &cmd.args[0];
    
    let timestamp = match parse_timestamp(&cmd.args[1]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid timestamp".into()),
    };
    
    // For "*", use current time
    let actual_timestamp = if cmd.args[1] == "*" {
        TimeSeries::current_timestamp()
    } else {
        timestamp
    };

    let value: f64 = match cmd.args[2].parse() {
        Ok(v) => v,
        Err(_) => return RespReply::Error("Value must be a number".into()),
    };

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::TimeSeries(ts) => {
                let ts_used = ts.add(actual_timestamp, value);
                RespReply::Integer(ts_used)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => {
            // Auto-create time series
            let mut ts = TimeSeries::new();
            let ts_used = ts.add(actual_timestamp, value);
            state.db.insert(key.clone(), ValueObject::TimeSeries(ts));
            RespReply::Integer(ts_used)
        }
    }
}

/// TS.MADD key timestamp value [key timestamp value ...]
/// Add multiple data points to multiple time series
pub fn ts_madd(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 || cmd.args.len() % 3 != 0 {
        return RespReply::Error("TS.MADD requires key timestamp value triplets".into());
    }

    let mut results = Vec::new();

    for chunk in cmd.args.chunks(3) {
        let key = &chunk[0];
        
        let timestamp = match parse_timestamp(&chunk[1]) {
            Some(ts) => ts,
            None => {
                results.push(RespReply::Error("Invalid timestamp".into()));
                continue;
            }
        };
        
        let actual_timestamp = if chunk[1] == "*" {
            TimeSeries::current_timestamp()
        } else {
            timestamp
        };

        let value: f64 = match chunk[2].parse() {
            Ok(v) => v,
            Err(_) => {
                results.push(RespReply::Error("Value must be a number".into()));
                continue;
            }
        };

        state.track_key_access(key);

        match state.db.get_mut(key) {
            Some(mut entry) => match entry.value_mut() {
                ValueObject::TimeSeries(ts) => {
                    let ts_used = ts.add(actual_timestamp, value);
                    results.push(RespReply::Integer(ts_used));
                }
                _ => results.push(RespReply::Error("WRONGTYPE".into())),
            },
            None => {
                let mut ts = TimeSeries::new();
                let ts_used = ts.add(actual_timestamp, value);
                state.db.insert(key.clone(), ValueObject::TimeSeries(ts));
                results.push(RespReply::Integer(ts_used));
            }
        }
    }

    RespReply::Array(results)
}

/// TS.GET key
/// Get the latest data point from a time series
pub fn ts_get(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("TS.GET requires key".into());
    }

    let key = &cmd.args[0];
    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::TimeSeries(ts) => {
                match ts.get_latest() {
                    Some(dp) => RespReply::Array(vec![
                        RespReply::Integer(dp.timestamp),
                        RespReply::Bulk(Some(dp.value.to_string())),
                    ]),
                    None => RespReply::Array(vec![]),
                }
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// TS.RANGE key fromTimestamp toTimestamp [AGGREGATION aggregationType timeBucket]
/// Query a range of data points
pub fn ts_range(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("TS.RANGE requires key, from, and to timestamps".into());
    }

    let key = &cmd.args[0];
    
    let from = match parse_timestamp(&cmd.args[1]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid from timestamp".into()),
    };
    
    let to = match parse_timestamp(&cmd.args[2]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid to timestamp".into()),
    };

    // Parse optional aggregation
    let mut aggregation: Option<(Aggregation, i64)> = None;
    let mut i = 3;
    while i < cmd.args.len() {
        match cmd.args[i].to_uppercase().as_str() {
            "AGGREGATION" => {
                if i + 2 >= cmd.args.len() {
                    return RespReply::Error("AGGREGATION requires type and timeBucket".into());
                }
                let agg_type = match Aggregation::from_str(&cmd.args[i + 1]) {
                    Some(a) => a,
                    None => return RespReply::Error(format!("Unknown aggregation type: {}", cmd.args[i + 1])),
                };
                let bucket: i64 = match cmd.args[i + 2].parse() {
                    Ok(b) if b > 0 => b,
                    _ => return RespReply::Error("timeBucket must be a positive integer".into()),
                };
                aggregation = Some((agg_type, bucket));
                i += 3;
            }
            _ => i += 1,
        }
    }

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::TimeSeries(ts) => {
                let data_points = if let Some((agg, bucket)) = aggregation {
                    ts.aggregate(from, to, bucket, agg)
                } else {
                    ts.range(from, to)
                };

                let result: Vec<RespReply> = data_points
                    .iter()
                    .map(|dp| RespReply::Array(vec![
                        RespReply::Integer(dp.timestamp),
                        RespReply::Bulk(Some(dp.value.to_string())),
                    ]))
                    .collect();

                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Array(vec![]),
    }
}

/// TS.MRANGE fromTimestamp toTimestamp FILTER filter...
/// Query multiple time series by filter
pub fn ts_mrange(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 4 {
        return RespReply::Error("TS.MRANGE requires from, to, FILTER, and filter expression".into());
    }

    let from = match parse_timestamp(&cmd.args[0]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid from timestamp".into()),
    };
    
    let to = match parse_timestamp(&cmd.args[1]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid to timestamp".into()),
    };

    // Find FILTER keyword
    let filter_idx = cmd.args.iter()
        .position(|s| s.to_uppercase() == "FILTER")
        .unwrap_or(cmd.args.len());

    if filter_idx >= cmd.args.len() - 1 {
        return RespReply::Error("FILTER requires at least one filter expression".into());
    }

    // Parse filters (format: key=value)
    let mut filters: Vec<(&str, &str)> = Vec::new();
    for filter in &cmd.args[filter_idx + 1..] {
        if let Some(idx) = filter.find('=') {
            let (k, v) = filter.split_at(idx);
            filters.push((k, &v[1..])); // Skip the '='
        }
    }

    if filters.is_empty() {
        return RespReply::Error("At least one filter is required".into());
    }

    // Find matching time series
    let mut results = Vec::new();

    for entry in state.db.iter() {
        if let ValueObject::TimeSeries(ts) = entry.value() {
            // Check if all filters match
            let matches = filters.iter().all(|(k, v)| ts.matches_filter(k, v));
            
            if matches {
                let key = entry.key().clone();
                let data_points = ts.range(from, to);
                
                let samples: Vec<RespReply> = data_points
                    .iter()
                    .map(|dp| RespReply::Array(vec![
                        RespReply::Integer(dp.timestamp),
                        RespReply::Bulk(Some(dp.value.to_string())),
                    ]))
                    .collect();

                results.push(RespReply::Array(vec![
                    RespReply::Bulk(Some(key)),
                    RespReply::Array(vec![]), // Labels placeholder
                    RespReply::Array(samples),
                ]));
            }
        }
    }

    RespReply::Array(results)
}

/// TS.INFO key
/// Get information about a time series
pub fn ts_info(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("TS.INFO requires key".into());
    }

    let key = &cmd.args[0];

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::TimeSeries(ts) => {
                let info = ts.info();
                let mut result: Vec<RespReply> = Vec::with_capacity(info.len() * 2);
                for (k, v) in info {
                    result.push(RespReply::Bulk(Some(k)));
                    result.push(RespReply::Bulk(Some(v)));
                }
                RespReply::Array(result)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Bulk(None),
    }
}

/// TS.DEL key fromTimestamp toTimestamp
/// Delete samples in a range
pub fn ts_del(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 3 {
        return RespReply::Error("TS.DEL requires key, from, and to timestamps".into());
    }

    let key = &cmd.args[0];
    
    let from = match parse_timestamp(&cmd.args[1]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid from timestamp".into()),
    };
    
    let to = match parse_timestamp(&cmd.args[2]) {
        Some(ts) => ts,
        None => return RespReply::Error("Invalid to timestamp".into()),
    };

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::TimeSeries(ts) => {
                let deleted = ts.delete_range(from, to);
                RespReply::Integer(deleted as i64)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}

/// TS.ALTER key [RETENTION ms] [LABELS label value ...]
/// Alter an existing time series
pub fn ts_alter(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("TS.ALTER requires key".into());
    }

    let key = &cmd.args[0];

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::TimeSeries(ts) => {
                let mut i = 1;
                while i < cmd.args.len() {
                    match cmd.args[i].to_uppercase().as_str() {
                        "RETENTION" => {
                            if i + 1 >= cmd.args.len() {
                                return RespReply::Error("RETENTION requires a value".into());
                            }
                            match cmd.args[i + 1].parse::<i64>() {
                                Ok(0) => ts.set_retention(None),
                                Ok(ms) if ms > 0 => ts.set_retention(Some(ms)),
                                _ => return RespReply::Error("RETENTION must be 0 or positive integer".into()),
                            }
                            i += 2;
                        }
                        "LABELS" => {
                            i += 1;
                            while i + 1 < cmd.args.len() {
                                if cmd.args[i].to_uppercase() == "RETENTION" {
                                    break;
                                }
                                ts.add_label(cmd.args[i].clone(), cmd.args[i + 1].clone());
                                i += 2;
                            }
                        }
                        _ => i += 1,
                    }
                }
                RespReply::Simple("OK".into())
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Error("key does not exist".into()),
    }
}

/// TS.INCRBY key value [TIMESTAMP timestamp]
/// Increment the value of the latest sample, or create a new sample
pub fn ts_incrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("TS.INCRBY requires key and value".into());
    }

    let key = &cmd.args[0];
    let increment: f64 = match cmd.args[1].parse() {
        Ok(v) => v,
        Err(_) => return RespReply::Error("Value must be a number".into()),
    };

    // Parse optional timestamp
    let mut timestamp = TimeSeries::current_timestamp();
    let mut i = 2;
    while i < cmd.args.len() {
        if cmd.args[i].to_uppercase() == "TIMESTAMP" && i + 1 < cmd.args.len() {
            match parse_timestamp(&cmd.args[i + 1]) {
                Some(ts) => timestamp = if cmd.args[i + 1] == "*" { TimeSeries::current_timestamp() } else { ts },
                None => return RespReply::Error("Invalid timestamp".into()),
            }
            break;
        }
        i += 1;
    }

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::TimeSeries(ts) => {
                let current_value = ts.get_latest().map(|dp| dp.value).unwrap_or(0.0);
                let new_value = current_value + increment;
                ts.add(timestamp, new_value);
                RespReply::Integer(timestamp)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => {
            let mut ts = TimeSeries::new();
            ts.add(timestamp, increment);
            state.db.insert(key.clone(), ValueObject::TimeSeries(ts));
            RespReply::Integer(timestamp)
        }
    }
}

/// TS.DECRBY key value [TIMESTAMP timestamp]
/// Decrement the value of the latest sample
pub fn ts_decrby(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("TS.DECRBY requires key and value".into());
    }

    let key = &cmd.args[0];
    let decrement: f64 = match cmd.args[1].parse() {
        Ok(v) => v,
        Err(_) => return RespReply::Error("Value must be a number".into()),
    };

    // Use ts_incrby with negative value
    let mut new_cmd = cmd.clone();
    new_cmd.args[1] = (-decrement).to_string();
    ts_incrby(new_cmd, state)
}
