//! SQL Query command for RivetDB
//!
//! Provides SQL-like query interface - a UNIQUE feature Redis lacks!
//!
//! Commands:
//! - QUERY "SELECT * FROM keys WHERE type = 'string'"
//! - QUERY "SELECT key, type, memory FROM keys WHERE key LIKE 'user:%'"
//! - QUERY "SELECT COUNT(*) FROM keys WHERE type = 'zset'"
//! - QUERY "DELETE FROM keys WHERE ttl < 60"

use super::ParsedCommand;
use crate::protocol::RespReply;
use crate::storage::SharedState;
use crate::query::{execute_query, QueryResult};

/// QUERY "sql"
/// Execute a SQL query against the database
pub fn query(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("QUERY requires a SQL statement".into());
    }

    let sql = &cmd.args[0];
    
    let result = execute_query(sql, state);
    
    match result {
        QueryResult::Rows { columns, rows } => {
            // Return as array of arrays
            let mut result_array = Vec::new();
            
            // Add column headers as first row
            let header: Vec<RespReply> = columns
                .iter()
                .map(|c| RespReply::Bulk(Some(c.clone())))
                .collect();
            result_array.push(RespReply::Array(header));
            
            // Add data rows
            for row in rows {
                let row_data: Vec<RespReply> = row.values
                    .iter()
                    .map(|v| RespReply::Bulk(Some(v.clone())))
                    .collect();
                result_array.push(RespReply::Array(row_data));
            }
            
            RespReply::Array(result_array)
        }
        QueryResult::Count(count) => {
            RespReply::Integer(count as i64)
        }
        QueryResult::Affected(count) => {
            RespReply::Integer(count as i64)
        }
        QueryResult::Error(err) => {
            RespReply::Error(err.to_string())
        }
    }
}

/// EXPLAIN "sql"
/// Show query execution plan (for debugging)
pub fn explain(cmd: ParsedCommand, state: &SharedState) -> RespReply {
    if cmd.args.is_empty() {
        return RespReply::Error("EXPLAIN requires a SQL statement".into());
    }

    let sql = &cmd.args[0];
    
    use sqlparser::dialect::GenericDialect;
    use sqlparser::parser::Parser;
    
    let dialect = GenericDialect {};
    match Parser::parse_sql(&dialect, sql) {
        Ok(statements) => {
            let mut info = Vec::new();
            info.push(RespReply::Bulk(Some("Parsed AST:".into())));
            for (i, stmt) in statements.iter().enumerate() {
                info.push(RespReply::Bulk(Some(format!("Statement {}: {:?}", i + 1, stmt))));
            }
            
            // Add key count estimate
            let key_count = state.db.len();
            info.push(RespReply::Bulk(Some(format!("Estimated scan: {} keys", key_count))));
            
            RespReply::Array(info)
        }
        Err(e) => {
            RespReply::Error(format!("Parse error: {}", e))
        }
    }
}
