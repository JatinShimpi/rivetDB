// Module declarations
pub mod protocol;
pub mod storage;
pub mod commands;
pub mod utils;
pub mod config;
pub mod persistence;
pub mod schema;
pub mod metrics;
pub mod query;

// Re-export commonly used types
pub use protocol::{RespFrame, RespReply};
pub use storage::{ValueObject, ServerState, SharedState, EvictionPolicy, estimate_value_size};
pub use commands::ParsedCommand;
pub use config::Config;
pub use persistence::{AofWriter, AofFsyncPolicy, load_aof, rewrite_aof};
pub use schema::{Schema, FieldType, TypedValue};
pub use metrics::start_metrics_server;
pub use query::{execute_query, QueryResult, QueryError};