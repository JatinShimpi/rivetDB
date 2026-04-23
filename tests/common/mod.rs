use rivetdb::{ServerState, SharedState, EvictionPolicy};
use std::sync::Arc;

/// Create a fresh test state for unit testing (DashMap version)
pub fn create_test_state() -> SharedState {
    Arc::new(ServerState::new(
        64 * 1024 * 1024, // 64 MB default
        EvictionPolicy::AllKeysLFU,
    ))
}

/// Helper to get value count in database
pub fn db_size(state: &SharedState) -> usize {
    state.db.len()
}

/// Helper to check if key exists
pub fn key_exists(state: &SharedState, key: &str) -> bool {
    state.db.contains_key(key)
}

/// Helper to get string value
pub fn get_string_value(state: &SharedState, key: &str) -> Option<String> {
    state.db.get(key).and_then(|entry| {
        if let rivetdb::ValueObject::String(s) = entry.value() {
            Some(s.clone())
        } else {
            None
        }
    })
}
