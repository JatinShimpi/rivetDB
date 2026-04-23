use super::state::SharedState;
use super::value::estimate_value_size;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvictionPolicy {
    NoEviction,
    AllKeysLFU,
    AllKeysLRU,
}

impl EvictionPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            EvictionPolicy::NoEviction => "noeviction",
            EvictionPolicy::AllKeysLFU => "allkeys-lfu",
            EvictionPolicy::AllKeysLRU => "allkeys-lru",
        }
    }
}

/// Get current memory usage (works with DashMap)
pub fn current_memory_usage(state: &SharedState) -> usize {
    state.db.iter().map(|entry| estimate_value_size(entry.value())).sum()
}

/// Evict keys if over memory limit (DashMap version)
/// Protected key won't be evicted (usually the key just set)
pub fn evict_if_needed(state: &SharedState, protected: Option<&str>) {
    if state.eviction_policy == EvictionPolicy::NoEviction {
        return;
    }

    let mut used = current_memory_usage(state);

    while used > state.max_memory && !state.db.is_empty() {
        // Find victim key (least frequently used)
        let victim = match state.eviction_policy {
            EvictionPolicy::AllKeysLFU | EvictionPolicy::AllKeysLRU => {
                state.key_access_count
                    .iter()
                    .filter(|entry| Some(entry.key().as_str()) != protected)
                    .min_by_key(|entry| *entry.value())
                    .map(|entry| entry.key().clone())
            }
            EvictionPolicy::NoEviction => None,
        };

        let Some(key) = victim else { break };

        // Remove the key and update memory
        if let Some((_, val)) = state.db.remove(&key) {
            used -= estimate_value_size(&val);
        }

        // Also remove from access count
        state.key_access_count.remove(&key);
    }
}
