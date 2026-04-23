use std::collections::{HashMap, HashSet, LinkedList};
use super::zset::ZSet;
use super::bloom::RivetBloomFilter;
use super::timeseries::TimeSeries;
use serde_json::Value as JsonValue;

pub enum ValueObject {
    String(String),
    List(LinkedList<String>),
    Set(HashSet<String>),
    ZSet(ZSet),
    Hash(HashMap<String, String>),
    Json(JsonValue),
    BloomFilter(RivetBloomFilter),
    TimeSeries(TimeSeries),
}

pub fn estimate_value_size(v: &ValueObject) -> usize {
    match v {
        ValueObject::String(s) => s.len(),
        ValueObject::List(l) => l.iter().map(|s| s.len()).sum(),
        ValueObject::Set(s) => s.iter().map(|s| s.len()).sum(),
        ValueObject::ZSet(z) => {
            // Estimate: number of members * (average member size + 8 bytes for f64 score)
            z.len() * 20  // Rough estimate
        }
        ValueObject::Hash(h) => {
            // Estimate: sum of key + value sizes
            h.iter().map(|(k, v)| k.len() + v.len()).sum()
        }
        ValueObject::Json(j) => {
            // Estimate: JSON string representation size
            serde_json::to_string(j).map(|s| s.len()).unwrap_or(64)
        }
        ValueObject::BloomFilter(bf) => {
            bf.memory_usage()
        }
        ValueObject::TimeSeries(ts) => {
            ts.memory_usage()
        }
    }
}
