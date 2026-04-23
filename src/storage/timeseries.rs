//! Time-Series data structure for RivetDB
//!
//! Provides native time-series support for IoT, monitoring, and analytics.
//! Unlike Redis (which requires RedisTimeSeries module), RivetDB has built-in support.

use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

/// Aggregation types for time-series queries
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Aggregation {
    Avg,
    Sum,
    Min,
    Max,
    Count,
    First,
    Last,
    Range,
    StdDev,
}

impl Aggregation {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "AVG" | "AVERAGE" => Some(Aggregation::Avg),
            "SUM" => Some(Aggregation::Sum),
            "MIN" => Some(Aggregation::Min),
            "MAX" => Some(Aggregation::Max),
            "COUNT" => Some(Aggregation::Count),
            "FIRST" => Some(Aggregation::First),
            "LAST" => Some(Aggregation::Last),
            "RANGE" => Some(Aggregation::Range),
            "STDDEV" | "STD.P" => Some(Aggregation::StdDev),
            _ => None,
        }
    }
}

/// A time-series data point
#[derive(Debug, Clone, Copy)]
pub struct DataPoint {
    pub timestamp: i64,
    pub value: f64,
}

/// Time-Series data structure
/// Stores timestamped values with optional retention and labels
#[derive(Debug, Clone)]
pub struct TimeSeries {
    /// Data points sorted by timestamp (BTreeMap for efficient range queries)
    data: BTreeMap<i64, f64>,
    /// Retention period in milliseconds (None = infinite)
    retention_ms: Option<i64>,
    /// Labels for filtering and indexing
    labels: HashMap<String, String>,
    /// Total number of samples ever added
    total_samples: u64,
    /// First timestamp in series
    first_timestamp: Option<i64>,
    /// Last timestamp in series
    last_timestamp: Option<i64>,
}

impl TimeSeries {
    /// Create a new empty time series
    pub fn new() -> Self {
        TimeSeries {
            data: BTreeMap::new(),
            retention_ms: None,
            labels: HashMap::new(),
            total_samples: 0,
            first_timestamp: None,
            last_timestamp: None,
        }
    }

    /// Create a time series with retention period
    pub fn with_retention(retention_ms: i64) -> Self {
        let mut ts = Self::new();
        ts.retention_ms = Some(retention_ms);
        ts
    }

    /// Set retention period in milliseconds
    pub fn set_retention(&mut self, retention_ms: Option<i64>) {
        self.retention_ms = retention_ms;
    }

    /// Get retention period
    pub fn retention(&self) -> Option<i64> {
        self.retention_ms
    }

    /// Add a label
    pub fn add_label(&mut self, key: String, value: String) {
        self.labels.insert(key, value);
    }

    /// Get all labels
    pub fn labels(&self) -> &HashMap<String, String> {
        &self.labels
    }

    /// Check if labels match a filter (key=value)
    pub fn matches_filter(&self, key: &str, value: &str) -> bool {
        self.labels.get(key).map(|v| v == value).unwrap_or(false)
    }

    /// Get current timestamp in milliseconds
    pub fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    /// Add a data point
    /// Returns the timestamp used (useful when auto-timestamp is used)
    pub fn add(&mut self, timestamp: i64, value: f64) -> i64 {
        // Apply retention if configured
        self.apply_retention(timestamp);

        self.data.insert(timestamp, value);
        self.total_samples += 1;

        // Update first/last timestamps
        if self.first_timestamp.is_none() || timestamp < self.first_timestamp.unwrap() {
            self.first_timestamp = Some(timestamp);
        }
        if self.last_timestamp.is_none() || timestamp > self.last_timestamp.unwrap() {
            self.last_timestamp = Some(timestamp);
        }

        timestamp
    }

    /// Apply retention policy - remove old data points
    fn apply_retention(&mut self, current_time: i64) {
        if let Some(retention) = self.retention_ms {
            let cutoff = current_time - retention;
            // BTreeMap allows efficient range operations
            let keys_to_remove: Vec<i64> = self.data
                .range(..cutoff)
                .map(|(k, _)| *k)
                .collect();
            for key in keys_to_remove {
                self.data.remove(&key);
            }
            // Update first_timestamp if data was removed
            self.first_timestamp = self.data.keys().next().copied();
        }
    }

    /// Get the latest data point
    pub fn get_latest(&self) -> Option<DataPoint> {
        self.data.iter().next_back().map(|(ts, val)| DataPoint {
            timestamp: *ts,
            value: *val,
        })
    }

    /// Get a specific data point by timestamp
    pub fn get(&self, timestamp: i64) -> Option<f64> {
        self.data.get(&timestamp).copied()
    }

    /// Get data points in a range (inclusive)
    /// Use i64::MIN for "-" (oldest) and i64::MAX for "+" (newest)
    pub fn range(&self, from: i64, to: i64) -> Vec<DataPoint> {
        self.data
            .range(from..=to)
            .map(|(ts, val)| DataPoint {
                timestamp: *ts,
                value: *val,
            })
            .collect()
    }

    /// Get aggregated data for a time range with bucketing
    pub fn aggregate(&self, from: i64, to: i64, bucket_size: i64, agg: Aggregation) -> Vec<DataPoint> {
        if bucket_size <= 0 {
            return vec![];
        }

        let mut result = Vec::new();
        let mut bucket_start = from;

        while bucket_start <= to {
            let bucket_end = bucket_start + bucket_size - 1;
            let values: Vec<f64> = self.data
                .range(bucket_start..=bucket_end.min(to))
                .map(|(_, v)| *v)
                .collect();

            if !values.is_empty() {
                let agg_value = Self::compute_aggregation(&values, agg);
                result.push(DataPoint {
                    timestamp: bucket_start,
                    value: agg_value,
                });
            }

            bucket_start += bucket_size;
        }

        result
    }

    /// Compute aggregation for a set of values
    fn compute_aggregation(values: &[f64], agg: Aggregation) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        match agg {
            Aggregation::Avg => values.iter().sum::<f64>() / values.len() as f64,
            Aggregation::Sum => values.iter().sum(),
            Aggregation::Min => values.iter().cloned().fold(f64::INFINITY, f64::min),
            Aggregation::Max => values.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            Aggregation::Count => values.len() as f64,
            Aggregation::First => values[0],
            Aggregation::Last => values[values.len() - 1],
            Aggregation::Range => {
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                max - min
            }
            Aggregation::StdDev => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                variance.sqrt()
            }
        }
    }

    /// Delete data points in a range
    pub fn delete_range(&mut self, from: i64, to: i64) -> usize {
        let keys_to_remove: Vec<i64> = self.data
            .range(from..=to)
            .map(|(k, _)| *k)
            .collect();
        let count = keys_to_remove.len();
        for key in keys_to_remove {
            self.data.remove(&key);
        }
        // Update timestamps
        self.first_timestamp = self.data.keys().next().copied();
        self.last_timestamp = self.data.keys().next_back().copied();
        count
    }

    /// Get the number of data points
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get total samples ever added
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    /// Get first timestamp
    pub fn first_timestamp(&self) -> Option<i64> {
        self.first_timestamp
    }

    /// Get last timestamp
    pub fn last_timestamp(&self) -> Option<i64> {
        self.last_timestamp
    }

    /// Estimate memory usage
    pub fn memory_usage(&self) -> usize {
        // Each entry: i64 key (8 bytes) + f64 value (8 bytes) + BTreeMap overhead (~48 bytes per entry)
        let data_size = self.data.len() * 64;
        // Labels: estimate
        let labels_size: usize = self.labels.iter()
            .map(|(k, v)| k.len() + v.len() + 48)
            .sum();
        data_size + labels_size + std::mem::size_of::<Self>()
    }

    /// Get info about this time series
    pub fn info(&self) -> Vec<(String, String)> {
        let mut info = vec![
            ("totalSamples".into(), self.total_samples.to_string()),
            ("memoryUsage".into(), format!("{} bytes", self.memory_usage())),
            ("retentionTime".into(), self.retention_ms.map(|r| r.to_string()).unwrap_or("unlimited".into())),
            ("labels".into(), format!("{:?}", self.labels)),
            ("firstTimestamp".into(), self.first_timestamp.map(|t| t.to_string()).unwrap_or("N/A".into())),
            ("lastTimestamp".into(), self.last_timestamp.map(|t| t.to_string()).unwrap_or("N/A".into())),
            ("currentSamples".into(), self.len().to_string()),
        ];
        info
    }
}

impl Default for TimeSeries {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse timestamp string
/// - "*" = current time
/// - "-" = minimum (oldest)
/// - "+" = maximum (newest)
/// - number = specific timestamp in ms
pub fn parse_timestamp(s: &str) -> Option<i64> {
    match s.trim() {
        "*" => Some(TimeSeries::current_timestamp()),
        "-" => Some(i64::MIN),
        "+" => Some(i64::MAX),
        num => num.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeseries_basic() {
        let mut ts = TimeSeries::new();
        
        ts.add(1000, 25.5);
        ts.add(2000, 26.0);
        ts.add(3000, 24.5);
        
        assert_eq!(ts.len(), 3);
        assert_eq!(ts.get(2000), Some(26.0));
    }

    #[test]
    fn test_timeseries_range() {
        let mut ts = TimeSeries::new();
        
        ts.add(1000, 10.0);
        ts.add(2000, 20.0);
        ts.add(3000, 30.0);
        ts.add(4000, 40.0);
        
        let range = ts.range(1500, 3500);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].value, 20.0);
        assert_eq!(range[1].value, 30.0);
    }

    #[test]
    fn test_timeseries_aggregation() {
        let mut ts = TimeSeries::new();
        
        // Add hourly data points
        ts.add(0, 10.0);
        ts.add(1000, 20.0);
        ts.add(2000, 30.0);
        ts.add(3000, 40.0);
        
        // Aggregate with bucket size 2000
        let agg = ts.aggregate(0, 4000, 2000, Aggregation::Avg);
        assert_eq!(agg.len(), 2);
        assert_eq!(agg[0].value, 15.0); // avg of 10, 20
        assert_eq!(agg[1].value, 35.0); // avg of 30, 40
    }

    #[test]
    fn test_timeseries_labels() {
        let mut ts = TimeSeries::new();
        ts.add_label("sensor_id".into(), "1".into());
        ts.add_label("location".into(), "room1".into());
        
        assert!(ts.matches_filter("sensor_id", "1"));
        assert!(ts.matches_filter("location", "room1"));
        assert!(!ts.matches_filter("sensor_id", "2"));
    }

    #[test]
    fn test_timeseries_retention() {
        let mut ts = TimeSeries::with_retention(1000);
        
        ts.add(100, 1.0);
        ts.add(500, 2.0);
        ts.add(900, 3.0);
        
        // Adding a point at 1500 should remove points before 500
        ts.add(1500, 4.0);
        
        assert!(ts.get(100).is_none());
        assert_eq!(ts.get(500), Some(2.0));
        assert_eq!(ts.get(900), Some(3.0));
        assert_eq!(ts.get(1500), Some(4.0));
    }
}
