//! SQL Query Executor for RivetDB
//!
//! Executes SQL-like queries against the key-value store.
//! A UNIQUE feature that Redis completely lacks!

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use sqlparser::ast::{
    BinaryOperator, Expr, Query, Select, SelectItem, SetExpr, Statement, TableFactor,
    Value as SqlValue,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use crate::storage::{SharedState, ValueObject, estimate_value_size};
use crate::commands::expiry::is_expired;

/// Query execution error
#[derive(Debug, Clone)]
pub enum QueryError {
    ParseError(String),
    UnsupportedQuery(String),
    InvalidTable(String),
    InvalidColumn(String),
    ExecutionError(String),
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::ParseError(s) => write!(f, "Parse error: {}", s),
            QueryError::UnsupportedQuery(s) => write!(f, "Unsupported query: {}", s),
            QueryError::InvalidTable(s) => write!(f, "Invalid table: {}", s),
            QueryError::InvalidColumn(s) => write!(f, "Invalid column: {}", s),
            QueryError::ExecutionError(s) => write!(f, "Execution error: {}", s),
        }
    }
}

/// A row in the query result
#[derive(Debug, Clone)]
pub struct QueryRow {
    pub columns: Vec<String>,
    pub values: Vec<String>,
}

/// Query result
#[derive(Debug, Clone)]
pub enum QueryResult {
    /// Rows returned from SELECT
    Rows {
        columns: Vec<String>,
        rows: Vec<QueryRow>,
    },
    /// Count result
    Count(usize),
    /// Number of rows affected (DELETE, UPDATE)
    Affected(usize),
    /// Error
    Error(QueryError),
}

/// Key metadata used during query execution
struct KeyInfo {
    key: String,
    type_name: String,
    memory: usize,
    ttl: i64,  // -1 = no expiry, -2 = expired/not exist
    length: usize,  // For lists, sets, etc.
}

/// Execute a SQL query against the database
pub fn execute_query(sql: &str, state: &SharedState) -> QueryResult {
    let dialect = GenericDialect {};
    
    // Parse the SQL
    let statements = match Parser::parse_sql(&dialect, sql) {
        Ok(stmts) => stmts,
        Err(e) => return QueryResult::Error(QueryError::ParseError(e.to_string())),
    };

    if statements.is_empty() {
        return QueryResult::Error(QueryError::ParseError("Empty query".into()));
    }

    if statements.len() > 1 {
        return QueryResult::Error(QueryError::UnsupportedQuery(
            "Only single statements are supported".into(),
        ));
    }

    match &statements[0] {
        Statement::Query(query) => execute_select(query, state),
        stmt @ Statement::Delete { .. } => execute_delete_stmt(stmt, state),
        _ => QueryResult::Error(QueryError::UnsupportedQuery(
            "Only SELECT and DELETE are supported".into(),
        )),
    }
}

/// Execute a SELECT query
fn execute_select(query: &Query, state: &SharedState) -> QueryResult {
    // Only support simple queries without CTEs, ORDER BY, LIMIT for now
    let Select {
        projection,
        from,
        selection,
        ..
    } = match query.body.as_ref() {
        SetExpr::Select(select) => select.as_ref(),
        _ => {
            return QueryResult::Error(QueryError::UnsupportedQuery(
                "Only simple SELECT is supported".into(),
            ))
        }
    };

    // Validate table name is "keys"
    if from.is_empty() {
        return QueryResult::Error(QueryError::InvalidTable("FROM clause is required".into()));
    }

    let table_name = match &from[0].relation {
        TableFactor::Table { name, .. } => name.to_string().to_lowercase(),
        _ => {
            return QueryResult::Error(QueryError::InvalidTable(
                "Only simple table names are supported".into(),
            ))
        }
    };

    if table_name != "keys" {
        return QueryResult::Error(QueryError::InvalidTable(format!(
            "Unknown table '{}'. Only 'keys' is supported",
            table_name
        )));
    }

    // Check for COUNT(*)
    let is_count = projection.iter().any(|item| {
        if let SelectItem::UnnamedExpr(Expr::Function(func)) = item {
            func.name.to_string().to_uppercase() == "COUNT"
        } else {
            false
        }
    });

    if is_count {
        // COUNT query
        let count = count_matching_keys(state, selection.as_ref());
        return QueryResult::Count(count);
    }

    // Determine columns to return
    let columns = extract_columns(projection);
    if columns.is_err() {
        return QueryResult::Error(columns.unwrap_err());
    }
    let columns = columns.unwrap();

    // Get matching rows
    let rows = get_matching_rows(state, selection.as_ref(), &columns);

    QueryResult::Rows { columns, rows }
}

/// Execute a DELETE query
fn execute_delete_stmt(delete: &Statement, state: &SharedState) -> QueryResult {
    // Extract delete info from Statement::Delete variant
    // sqlparser 0.39 wraps delete in a boxed struct
    let (from_tables, selection) = extract_delete_info(delete);
    
    if from_tables.is_empty() {
        return QueryResult::Error(QueryError::InvalidTable("FROM clause required".into()));
    }

    let table_name = from_tables[0].to_lowercase();

    if table_name != "keys" {
        return QueryResult::Error(QueryError::InvalidTable(format!(
            "Unknown table '{}'. Only 'keys' is supported",
            table_name
        )));
    }

    // Find keys to delete
    let keys_to_delete = find_matching_keys(state, selection.as_deref());
    let count = keys_to_delete.len();

    // Delete the keys
    for key in keys_to_delete {
        state.db.remove(&key);
    }

    QueryResult::Affected(count)
}

/// Extract delete information from Statement::Delete
fn extract_delete_info(stmt: &Statement) -> (Vec<String>, Option<Box<Expr>>) {
    // This is a workaround for sqlparser version differences
    // We'll just get the info we need by converting to string and re-parsing
    // This is not ideal but works for our use case
    let sql = stmt.to_string();
    
    // Parse out table name (after DELETE FROM)
    let mut tables = Vec::new();
    let mut selection = None;
    
    if let Some(from_idx) = sql.to_uppercase().find("FROM") {
        let after_from = &sql[from_idx + 4..].trim();
        let table_end = after_from.find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(after_from.len());
        tables.push(after_from[..table_end].to_string());
        
        // Parse WHERE clause
        if let Some(where_idx) = sql.to_uppercase().find("WHERE") {
            let after_where = &sql[where_idx + 5..].trim();
            let dialect = GenericDialect {};
            // Try to parse just the where clause as expression
            let where_sql = format!("SELECT * FROM t WHERE {}", after_where);
            if let Ok(stmts) = Parser::parse_sql(&dialect, &where_sql) {
                if let Some(Statement::Query(q)) = stmts.first() {
                    if let SetExpr::Select(sel) = q.body.as_ref() {
                        selection = sel.selection.clone().map(Box::new);
                    }
                }
            }
        }
    }
    
    (tables, selection)
}

/// Extract column names from SELECT projection
fn extract_columns(projection: &[SelectItem]) -> Result<Vec<String>, QueryError> {
    let mut columns = Vec::new();

    for item in projection {
        match item {
            SelectItem::Wildcard(_) => {
                // Return all columns
                return Ok(vec![
                    "key".into(),
                    "type".into(),
                    "memory".into(),
                    "ttl".into(),
                    "length".into(),
                ]);
            }
            SelectItem::UnnamedExpr(expr) => {
                let col = expr_to_column_name(expr)?;
                columns.push(col);
            }
            SelectItem::ExprWithAlias { expr, alias } => {
                let _ = expr_to_column_name(expr)?;
                columns.push(alias.value.clone());
            }
            _ => {
                return Err(QueryError::UnsupportedQuery(
                    "Unsupported projection type".into(),
                ))
            }
        }
    }

    Ok(columns)
}

/// Convert an expression to a column name
fn expr_to_column_name(expr: &Expr) -> Result<String, QueryError> {
    match expr {
        Expr::Identifier(ident) => {
            let name = ident.value.to_lowercase();
            match name.as_str() {
                "key" | "type" | "memory" | "ttl" | "length" | "value" => Ok(name),
                _ => Err(QueryError::InvalidColumn(format!("Unknown column: {}", name))),
            }
        }
        Expr::CompoundIdentifier(idents) => {
            if idents.len() == 2 && idents[0].value.to_lowercase() == "keys" {
                let col = idents[1].value.to_lowercase();
                Ok(col)
            } else {
                Err(QueryError::InvalidColumn("Invalid column reference".into()))
            }
        }
        _ => Err(QueryError::InvalidColumn("Expected column name".into())),
    }
}

/// Count matching keys
fn count_matching_keys(state: &SharedState, selection: Option<&Expr>) -> usize {
    let mut count = 0;

    for entry in state.db.iter() {
        let key = entry.key();
        
        // Skip expired keys
        if is_expired(state, key) {
            continue;
        }

        let key_info = build_key_info(state, key, entry.value());

        if matches_where_clause(&key_info, selection) {
            count += 1;
        }
    }

    count
}

/// Find keys matching the WHERE clause
fn find_matching_keys(state: &SharedState, selection: Option<&Expr>) -> Vec<String> {
    let mut keys = Vec::new();

    for entry in state.db.iter() {
        let key = entry.key();
        
        if is_expired(state, key) {
            continue;
        }

        let key_info = build_key_info(state, key, entry.value());

        if matches_where_clause(&key_info, selection) {
            keys.push(key.clone());
        }
    }

    keys
}

/// Get matching rows
fn get_matching_rows(
    state: &SharedState,
    selection: Option<&Expr>,
    columns: &[String],
) -> Vec<QueryRow> {
    let mut rows = Vec::new();

    for entry in state.db.iter() {
        let key = entry.key();
        
        if is_expired(state, key) {
            continue;
        }

        let key_info = build_key_info(state, key, entry.value());

        if matches_where_clause(&key_info, selection) {
            let values: Vec<String> = columns
                .iter()
                .map(|col| get_column_value(&key_info, col, entry.value()))
                .collect();

            rows.push(QueryRow {
                columns: columns.to_vec(),
                values,
            });
        }
    }

    rows
}

/// Build key info for a key
fn build_key_info(state: &SharedState, key: &str, value: &ValueObject) -> KeyInfo {
    let type_name = match value {
        ValueObject::String(_) => "string",
        ValueObject::List(_) => "list",
        ValueObject::Set(_) => "set",
        ValueObject::ZSet(_) => "zset",
        ValueObject::Hash(_) => "hash",
        ValueObject::Json(_) => "json",
        ValueObject::BloomFilter(_) => "bloom",
        ValueObject::TimeSeries(_) => "timeseries",
    };

    let memory = estimate_value_size(value);
    
    let ttl = get_ttl_for_key(state, key);
    
    let length = match value {
        ValueObject::String(s) => s.len(),
        ValueObject::List(l) => l.len(),
        ValueObject::Set(s) => s.len(),
        ValueObject::ZSet(z) => z.len(),
        ValueObject::Hash(h) => h.len(),
        ValueObject::Json(_) => 1,
        ValueObject::BloomFilter(bf) => bf.items_added(),
        ValueObject::TimeSeries(ts) => ts.len(),
    };

    KeyInfo {
        key: key.to_string(),
        type_name: type_name.to_string(),
        memory,
        ttl,
        length,
    }
}

/// Get TTL for a key in seconds
fn get_ttl_for_key(state: &SharedState, key: &str) -> i64 {
    use std::time::Instant;
    
    let now = Instant::now();

    // Check expiry heap - uses Reverse<(Instant, String)>
    let expiries = state.expiries.lock().unwrap();
    for entry in expiries.iter() {
        // entry is Reverse<(Instant, String)>
        // entry.0 is the (Instant, String) tuple
        let (expire_time, entry_key) = &entry.0;
        if entry_key == key {
            // Calculate TTL in seconds
            if *expire_time > now {
                let duration = expire_time.duration_since(now);
                return duration.as_secs() as i64;
            } else {
                return -2; // Already expired
            }
        }
    }
    -1 // No expiry
}

/// Get column value for a key
fn get_column_value(key_info: &KeyInfo, column: &str, value: &ValueObject) -> String {
    match column {
        "key" => key_info.key.clone(),
        "type" => key_info.type_name.clone(),
        "memory" => key_info.memory.to_string(),
        "ttl" => key_info.ttl.to_string(),
        "length" => key_info.length.to_string(),
        "value" => match value {
            ValueObject::String(s) => s.clone(),
            _ => format!("<{}>", key_info.type_name),
        },
        _ => "NULL".to_string(),
    }
}

/// Check if a key matches the WHERE clause
fn matches_where_clause(key_info: &KeyInfo, selection: Option<&Expr>) -> bool {
    match selection {
        None => true, // No WHERE clause = match all
        Some(expr) => evaluate_where_expr(key_info, expr),
    }
}

/// Evaluate a WHERE expression
fn evaluate_where_expr(key_info: &KeyInfo, expr: &Expr) -> bool {
    match expr {
        Expr::BinaryOp { left, op, right } => {
            match op {
                BinaryOperator::And => {
                    evaluate_where_expr(key_info, left) && evaluate_where_expr(key_info, right)
                }
                BinaryOperator::Or => {
                    evaluate_where_expr(key_info, left) || evaluate_where_expr(key_info, right)
                }
                BinaryOperator::Eq => {
                    let left_val = evaluate_expr_value(key_info, left);
                    let right_val = evaluate_expr_value(key_info, right);
                    left_val == right_val
                }
                BinaryOperator::NotEq => {
                    let left_val = evaluate_expr_value(key_info, left);
                    let right_val = evaluate_expr_value(key_info, right);
                    left_val != right_val
                }
                BinaryOperator::Lt => {
                    compare_values(key_info, left, right, |a, b| a < b)
                }
                BinaryOperator::LtEq => {
                    compare_values(key_info, left, right, |a, b| a <= b)
                }
                BinaryOperator::Gt => {
                    compare_values(key_info, left, right, |a, b| a > b)
                }
                BinaryOperator::GtEq => {
                    compare_values(key_info, left, right, |a, b| a >= b)
                }
                _ => true, // Unsupported operator = ignore
            }
        }
        Expr::Like { expr, pattern, .. } => {
            let val = evaluate_expr_value(key_info, expr);
            let pattern = evaluate_expr_value(key_info, pattern);
            matches_like_pattern(&val, &pattern)
        }
        Expr::Nested(inner) => evaluate_where_expr(key_info, inner),
        _ => true, // Unsupported expression = match
    }
}

/// Evaluate expression to a string value
fn evaluate_expr_value(key_info: &KeyInfo, expr: &Expr) -> String {
    match expr {
        Expr::Identifier(ident) => {
            let col = ident.value.to_lowercase();
            match col.as_str() {
                "key" => key_info.key.clone(),
                "type" => key_info.type_name.clone(),
                "memory" => key_info.memory.to_string(),
                "ttl" => key_info.ttl.to_string(),
                "length" => key_info.length.to_string(),
                _ => "NULL".to_string(),
            }
        }
        Expr::Value(val) => match val {
            SqlValue::SingleQuotedString(s) => s.clone(),
            SqlValue::DoubleQuotedString(s) => s.clone(),
            SqlValue::Number(n, _) => n.clone(),
            SqlValue::Boolean(b) => b.to_string(),
            SqlValue::Null => "NULL".to_string(),
            _ => "".to_string(),
        },
        Expr::Nested(inner) => evaluate_expr_value(key_info, inner),
        _ => "".to_string(),
    }
}

/// Compare numeric values
fn compare_values<F>(key_info: &KeyInfo, left: &Expr, right: &Expr, cmp: F) -> bool
where
    F: Fn(i64, i64) -> bool,
{
    let left_val = evaluate_expr_value(key_info, left);
    let right_val = evaluate_expr_value(key_info, right);

    match (left_val.parse::<i64>(), right_val.parse::<i64>()) {
        (Ok(l), Ok(r)) => cmp(l, r),
        _ => false,
    }
}

/// Match SQL LIKE pattern
/// Supports % (any characters) and _ (single character)
fn matches_like_pattern(value: &str, pattern: &str) -> bool {
    // Convert SQL LIKE pattern to regex-like matching
    let mut regex_pattern = String::new();
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '%' => regex_pattern.push_str(".*"),
            '_' => regex_pattern.push('.'),
            '\\' => {
                // Escape next character
                if let Some(next) = chars.next() {
                    regex_pattern.push('\\');
                    regex_pattern.push(next);
                }
            }
            c if c.is_alphanumeric() || c == ':' || c == '-' || c == '_' => {
                regex_pattern.push(c);
            }
            c => {
                regex_pattern.push('\\');
                regex_pattern.push(c);
            }
        }
    }

    // Simple glob-like matching without regex dependency
    simple_glob_match(value, &pattern)
}

/// Simple glob matching (% = *, _ = ?)
fn simple_glob_match(text: &str, pattern: &str) -> bool {
    let mut text_chars: Vec<char> = text.chars().collect();
    let mut pattern_chars: Vec<char> = pattern.chars().collect();
    
    fn match_helper(text: &[char], pattern: &[char]) -> bool {
        if pattern.is_empty() {
            return text.is_empty();
        }
        
        match pattern[0] {
            '%' => {
                // % matches zero or more characters
                // Try matching with 0, 1, 2, ... characters consumed from text
                for i in 0..=text.len() {
                    if match_helper(&text[i..], &pattern[1..]) {
                        return true;
                    }
                }
                false
            }
            '_' => {
                // _ matches exactly one character
                if text.is_empty() {
                    false
                } else {
                    match_helper(&text[1..], &pattern[1..])
                }
            }
            c => {
                if text.is_empty() {
                    false
                } else if text[0].to_ascii_lowercase() == c.to_ascii_lowercase() {
                    match_helper(&text[1..], &pattern[1..])
                } else {
                    false
                }
            }
        }
    }
    
    match_helper(&text_chars, &pattern_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_like_pattern() {
        assert!(simple_glob_match("user:123", "user:%"));
        assert!(simple_glob_match("user:456", "user:%"));
        assert!(!simple_glob_match("item:123", "user:%"));
        
        assert!(simple_glob_match("user:1", "user:_"));
        assert!(!simple_glob_match("user:12", "user:_"));
        
        assert!(simple_glob_match("hello", "%"));
        assert!(simple_glob_match("", "%"));
    }

    #[test]
    fn test_sql_parsing() {
        let dialect = GenericDialect {};
        
        // Test SELECT
        let sql = "SELECT * FROM keys WHERE type = 'string'";
        let result = Parser::parse_sql(&dialect, sql);
        assert!(result.is_ok());
        
        // Test DELETE
        let sql = "DELETE FROM keys WHERE ttl < 60";
        let result = Parser::parse_sql(&dialect, sql);
        assert!(result.is_ok());
        
        // Test COUNT
        let sql = "SELECT COUNT(*) FROM keys";
        let result = Parser::parse_sql(&dialect, sql);
        assert!(result.is_ok());
    }
}
