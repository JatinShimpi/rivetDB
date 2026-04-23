use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tracing::{info, error};

use crate::storage::{SharedState, ValueObject, estimate_value_size};

/// Metrics server state
pub struct MetricsState {
    pub db_state: SharedState,
    pub start_time: Instant,
}

/// Create and start the metrics HTTP server
pub async fn start_metrics_server(db_state: SharedState, port: u16) {
    let metrics_state = Arc::new(MetricsState {
        db_state,
        start_time: Instant::now(),
    });

    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/health", get(health_handler))
        .route("/info", get(info_handler))
        .with_state(metrics_state);

    let addr = format!("0.0.0.0:{}", port);
    
    match TcpListener::bind(&addr).await {
        Ok(listener) => {
            info!(addr = %addr, "Metrics server listening");
            if let Err(e) = axum::serve(listener, app).await {
                error!(error = %e, "Metrics server error");
            }
        }
        Err(e) => {
            error!(error = %e, addr = %addr, "Failed to bind metrics server");
        }
    }
}

/// Prometheus metrics handler (DashMap version)
async fn metrics_handler(State(state): State<Arc<MetricsState>>) -> impl IntoResponse {
    let db_state = &state.db_state;
    
    let mut output = String::new();
    
    output.push_str("# RivetDB Prometheus Metrics\n\n");
    
    // Command metrics (iterate DashMap)
    output.push_str("# HELP rivetdb_commands_total Total number of commands executed\n");
    output.push_str("# TYPE rivetdb_commands_total counter\n");
    
    for entry in db_state.command_count.iter() {
        output.push_str(&format!(
            "rivetdb_commands_total{{command=\"{}\"}} {}\n",
            entry.key(), entry.value()
        ));
    }
    output.push('\n');
    
    // Total commands
    let total_commands: u64 = db_state.command_count.iter().map(|e| *e.value()).sum();
    output.push_str("# HELP rivetdb_commands_processed_total Total commands processed\n");
    output.push_str("# TYPE rivetdb_commands_processed_total counter\n");
    output.push_str(&format!("rivetdb_commands_processed_total {}\n\n", total_commands));
    
    // Latency metrics
    output.push_str("# HELP rivetdb_command_duration_nanoseconds_total Total time spent\n");
    output.push_str("# TYPE rivetdb_command_duration_nanoseconds_total counter\n");
    
    for entry in db_state.command_time_ns.iter() {
        output.push_str(&format!(
            "rivetdb_command_duration_nanoseconds_total{{command=\"{}\"}} {}\n",
            entry.key(), entry.value()
        ));
    }
    output.push('\n');
    
    // Memory metrics
    let memory_used: usize = db_state.db.iter()
        .map(|entry| estimate_value_size(entry.value()))
        .sum();
    
    output.push_str("# HELP rivetdb_memory_used_bytes Estimated memory used\n");
    output.push_str("# TYPE rivetdb_memory_used_bytes gauge\n");
    output.push_str(&format!("rivetdb_memory_used_bytes {}\n\n", memory_used));
    
    output.push_str("# HELP rivetdb_memory_max_bytes Maximum memory limit\n");
    output.push_str("# TYPE rivetdb_memory_max_bytes gauge\n");
    output.push_str(&format!("rivetdb_memory_max_bytes {}\n\n", db_state.max_memory));
    
    // Memory utilization
    let memory_pct = if db_state.max_memory > 0 {
        (memory_used as f64 / db_state.max_memory as f64) * 100.0
    } else {
        0.0
    };
    output.push_str("# HELP rivetdb_memory_utilization_percent Memory utilization\n");
    output.push_str("# TYPE rivetdb_memory_utilization_percent gauge\n");
    output.push_str(&format!("rivetdb_memory_utilization_percent {:.2}\n\n", memory_pct));
    
    // Key metrics
    output.push_str("# HELP rivetdb_keys_total Total number of keys\n");
    output.push_str("# TYPE rivetdb_keys_total gauge\n");
    output.push_str(&format!("rivetdb_keys_total {}\n\n", db_state.db.len()));
    
    // Keys by type
    let mut string_count = 0usize;
    let mut list_count = 0usize;
    let mut set_count = 0usize;
    let mut zset_count = 0usize;
    let mut hash_count = 0usize;
    let mut json_count = 0usize;
    let mut bloom_count = 0usize;
    let mut ts_count = 0usize;
    
    for entry in db_state.db.iter() {
        match entry.value() {
            ValueObject::String(_) => string_count += 1,
            ValueObject::List(_) => list_count += 1,
            ValueObject::Set(_) => set_count += 1,
            ValueObject::ZSet(_) => zset_count += 1,
            ValueObject::Hash(_) => hash_count += 1,
            ValueObject::Json(_) => json_count += 1,
            ValueObject::BloomFilter(_) => bloom_count += 1,
            ValueObject::TimeSeries(_) => ts_count += 1,
        }
    }
    
    output.push_str("# HELP rivetdb_keys_by_type Number of keys by type\n");
    output.push_str("# TYPE rivetdb_keys_by_type gauge\n");
    output.push_str(&format!("rivetdb_keys_by_type{{type=\"string\"}} {}\n", string_count));
    output.push_str(&format!("rivetdb_keys_by_type{{type=\"list\"}} {}\n", list_count));
    output.push_str(&format!("rivetdb_keys_by_type{{type=\"set\"}} {}\n", set_count));
    output.push_str(&format!("rivetdb_keys_by_type{{type=\"zset\"}} {}\n", zset_count));
    output.push_str(&format!("rivetdb_keys_by_type{{type=\"hash\"}} {}\n", hash_count));
    output.push_str(&format!("rivetdb_keys_by_type{{type=\"json\"}} {}\n\n", json_count));
    
    // Expiry metrics
    output.push_str("# HELP rivetdb_expired_keys_total Total expired keys\n");
    output.push_str("# TYPE rivetdb_expired_keys_total counter\n");
    output.push_str(&format!("rivetdb_expired_keys_total {}\n\n", db_state.get_expired_count()));
    
    let expiry_count = db_state.expiries.lock()
        .map(|e| e.len())
        .unwrap_or(0);
    output.push_str("# HELP rivetdb_expiring_keys_pending Keys with expiry pending\n");
    output.push_str("# TYPE rivetdb_expiring_keys_pending gauge\n");
    output.push_str(&format!("rivetdb_expiring_keys_pending {}\n\n", expiry_count));
    
    // Uptime
    let uptime_secs = state.start_time.elapsed().as_secs();
    output.push_str("# HELP rivetdb_uptime_seconds Server uptime\n");
    output.push_str("# TYPE rivetdb_uptime_seconds counter\n");
    output.push_str(&format!("rivetdb_uptime_seconds {}\n\n", uptime_secs));
    
    // Slowlog count
    let slowlog_count = db_state.slowlog.lock()
        .map(|s| s.len())
        .unwrap_or(0);
    output.push_str("# HELP rivetdb_slowlog_length Number of entries in slow log\n");
    output.push_str("# TYPE rivetdb_slowlog_length gauge\n");
    output.push_str(&format!("rivetdb_slowlog_length {}\n\n", slowlog_count));
    
    // Hot keys (top 10)
    output.push_str("# HELP rivetdb_key_access_count Access count per key (top 10)\n");
    output.push_str("# TYPE rivetdb_key_access_count gauge\n");
    
    let mut key_counts: Vec<_> = db_state.key_access_count.iter()
        .map(|e| (e.key().clone(), *e.value()))
        .collect();
    key_counts.sort_by(|a, b| b.1.cmp(&a.1));
    
    for (key, count) in key_counts.into_iter().take(10) {
        let escaped_key = key.replace('\\', "\\\\").replace('"', "\\\"");
        output.push_str(&format!(
            "rivetdb_key_access_count{{key=\"{}\"}} {}\n",
            escaped_key, count
        ));
    }
    
    (StatusCode::OK, output)
}

/// Health check endpoint
async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

/// Server info endpoint (JSON format) - DashMap version
async fn info_handler(State(state): State<Arc<MetricsState>>) -> impl IntoResponse {
    let db_state = &state.db_state;
    let uptime_secs = state.start_time.elapsed().as_secs();
    
    let memory_used: usize = db_state.db.iter()
        .map(|entry| estimate_value_size(entry.value()))
        .sum();
    
    let total_commands: u64 = db_state.command_count.iter()
        .map(|e| *e.value())
        .sum();
    
    let info = format!(r#"{{
  "server": "RivetDB",
  "version": "{}",
  "uptime_seconds": {},
  "keys": {},
  "memory_used_bytes": {},
  "memory_max_bytes": {},
  "commands_processed": {},
  "expired_keys": {},
  "eviction_policy": "{:?}",
  "concurrency_mode": "DashMap (lock-free)"
}}"#,
        env!("CARGO_PKG_VERSION"),
        uptime_secs,
        db_state.db.len(),
        memory_used,
        db_state.max_memory,
        total_commands,
        db_state.get_expired_count(),
        db_state.eviction_policy
    );
    
    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        info
    )
}
