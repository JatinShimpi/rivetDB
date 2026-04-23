//! Bloom Filter commands for RivetDB
//! 
//! Built-in Bloom filter support - no external module needed (unlike Redis!)
//! 
//! Commands:
//! - BF.RESERVE key capacity error_rate - Create a new Bloom filter
//! - BF.ADD key item - Add an item to the filter
//! - BF.MADD key item [item ...] - Add multiple items
//! - BF.EXISTS key item - Check if item may exist
//! - BF.MEXISTS key item [item ...] - Check multiple items
//! - BF.INFO key - Get filter info

use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::{SharedState, ValueObject, RivetBloomFilter};

/// BF.RESERVE key capacity [error_rate]
/// Create a new Bloom filter with specified capacity and optional error rate
/// 
/// Default error rate: 0.01 (1%)
pub fn bf_reserve(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("BF.RESERVE requires key and capacity".into());
    }

    let key = &cmd.args[0];
    
    // Check if key already exists
    if state.db.contains_key(key) {
        return RespReply::Error("key already exists".into());
    }

    let capacity: usize = match cmd.args[1].parse() {
        Ok(c) if c > 0 => c,
        _ => return RespReply::Error("capacity must be a positive integer".into()),
    };

    let error_rate: f64 = if cmd.args.len() >= 3 {
        match cmd.args[2].parse() {
            Ok(e) if e > 0.0 && e < 1.0 => e,
            _ => return RespReply::Error("error rate must be between 0 and 1".into()),
        }
    } else {
        0.01 // Default 1% false positive rate
    };

    let bloom = RivetBloomFilter::new(capacity, error_rate);
    state.db.insert(key.clone(), ValueObject::BloomFilter(bloom));

    RespReply::Simple("OK".into())
}

/// BF.ADD key item
/// Add a single item to the Bloom filter
/// Returns 1 if item was (probably) new, 0 if it already existed
pub fn bf_add(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("BF.ADD requires key and item".into());
    }

    let key = &cmd.args[0];
    let item = &cmd.args[1];

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::BloomFilter(bf) => {
                let was_new = bf.add(item);
                RespReply::Integer(if was_new { 1 } else { 0 })
            }
            _ => RespReply::Error("WRONGTYPE Operation against a key holding the wrong kind of value".into()),
        },
        None => {
            // Auto-create filter with default settings (1M capacity, 1% FP rate)
            let mut bf = RivetBloomFilter::new(1_000_000, 0.01);
            let was_new = bf.add(item);
            state.db.insert(key.clone(), ValueObject::BloomFilter(bf));
            RespReply::Integer(if was_new { 1 } else { 0 })
        }
    }
}

/// BF.MADD key item [item ...]
/// Add multiple items to the Bloom filter
/// Returns array of 1s and 0s indicating if each item was new
pub fn bf_madd(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("BF.MADD requires key and at least one item".into());
    }

    let key = &cmd.args[0];
    let items: Vec<String> = cmd.args[1..].to_vec();

    state.track_key_access(key);

    match state.db.get_mut(key) {
        Some(mut entry) => match entry.value_mut() {
            ValueObject::BloomFilter(bf) => {
                let results: Vec<RespReply> = items
                    .iter()
                    .map(|item| {
                        let was_new = bf.add(item);
                        RespReply::Integer(if was_new { 1 } else { 0 })
                    })
                    .collect();
                RespReply::Array(results)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => {
            // Auto-create filter
            let mut bf = RivetBloomFilter::new(1_000_000, 0.01);
            let results: Vec<RespReply> = items
                .iter()
                .map(|item| {
                    let was_new = bf.add(item);
                    RespReply::Integer(if was_new { 1 } else { 0 })
                })
                .collect();
            state.db.insert(key.clone(), ValueObject::BloomFilter(bf));
            RespReply::Array(results)
        }
    }
}

/// BF.EXISTS key item
/// Check if an item may exist in the filter
/// Returns 1 if it may exist, 0 if it definitely does not
pub fn bf_exists(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() != 2 {
        return RespReply::Error("BF.EXISTS requires key and item".into());
    }

    let key = &cmd.args[0];
    let item = &cmd.args[1];

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::BloomFilter(bf) => {
                RespReply::Integer(if bf.exists(item) { 1 } else { 0 })
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0), // Non-existent key = item doesn't exist
    }
}

/// BF.MEXISTS key item [item ...]
/// Check if multiple items may exist in the filter
/// Returns array of 1s and 0s
pub fn bf_mexists(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.len() < 2 {
        return RespReply::Error("BF.MEXISTS requires key and at least one item".into());
    }

    let key = &cmd.args[0];
    let items: Vec<String> = cmd.args[1..].to_vec();

    state.track_key_access(key);

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::BloomFilter(bf) => {
                let results = bf.mexists(&items);
                let replies: Vec<RespReply> = results
                    .iter()
                    .map(|&exists| RespReply::Integer(if exists { 1 } else { 0 }))
                    .collect();
                RespReply::Array(replies)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => {
            // Non-existent key = all items don't exist
            let replies: Vec<RespReply> = items.iter().map(|_| RespReply::Integer(0)).collect();
            RespReply::Array(replies)
        }
    }
}

/// BF.INFO key
/// Get information about a Bloom filter
pub fn bf_info(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("BF.INFO requires key".into());
    }

    let key = &cmd.args[0];

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::BloomFilter(bf) => {
                let info = bf.info();
                let mut result = Vec::with_capacity(info.len() * 2);
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

/// BF.CARD key
/// Get the cardinality (number of items added) of a Bloom filter
pub fn bf_card(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("BF.CARD requires key".into());
    }

    let key = &cmd.args[0];

    match state.db.get(key) {
        Some(entry) => match entry.value() {
            ValueObject::BloomFilter(bf) => {
                RespReply::Integer(bf.items_added() as i64)
            }
            _ => RespReply::Error("WRONGTYPE".into()),
        },
        None => RespReply::Integer(0),
    }
}
