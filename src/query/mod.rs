//! SQL-like Query Language for RivetDB
//!
//! A UNIQUE feature that Redis completely lacks!
//! Provides familiar SQL syntax for querying key-value data.
//!
//! Supported queries:
//! - SELECT * FROM keys WHERE type = 'string'
//! - SELECT key, type, memory FROM keys WHERE key LIKE 'user:*'
//! - SELECT COUNT(*) FROM keys
//! - DELETE FROM keys WHERE ttl < 60

pub mod executor;

pub use executor::{execute_query, QueryResult, QueryError};
