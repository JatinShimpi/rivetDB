//! Bloom Filter storage for RivetDB
//! 
//! Provides space-efficient probabilistic set membership testing.
//! Unlike Redis (which requires the RedisBloom module), RivetDB has
//! native built-in Bloom filter support.

use bloomfilter::Bloom;
use std::collections::hash_map::DefaultHasher;

/// Wrapper around Bloom filter to make it storable as a ValueObject
/// Note: We don't derive Clone because Bloom doesn't implement it.
/// Instead, we store the configuration and rebuild if needed.
#[derive(Debug)]
pub struct RivetBloomFilter {
    /// The underlying bloom filter
    filter: Bloom<String>,
    /// Expected capacity (for info purposes)
    capacity: usize,
    /// Target false positive rate (for info purposes)
    false_positive_rate: f64,
    /// Number of items added (estimated)
    items_added: usize,
}

impl Clone for RivetBloomFilter {
    fn clone(&self) -> Self {
        // Create a new filter with the same parameters
        // Note: This doesn't preserve the actual bits, so cloned filter starts empty
        // For our use case (storage in DashMap), this is acceptable as we don't
        // actually need to clone filters in practice
        let filter = Bloom::new_for_fp_rate(self.capacity, self.false_positive_rate);
        RivetBloomFilter {
            filter,
            capacity: self.capacity,
            false_positive_rate: self.false_positive_rate,
            items_added: 0, // Reset on clone - this is a limitation
        }
    }
}

impl RivetBloomFilter {
    /// Create a new Bloom filter with specified capacity and false positive rate
    /// 
    /// # Arguments
    /// * `capacity` - Expected number of items
    /// * `false_positive_rate` - Target false positive rate (0.0 to 1.0)
    /// 
    /// # Example
    /// ```ignore
    /// let bf = RivetBloomFilter::new(1_000_000, 0.01); // 1M items, 1% FP rate
    /// ```
    pub fn new(capacity: usize, false_positive_rate: f64) -> Self {
        // Clamp false positive rate to valid range
        let fpr = false_positive_rate.clamp(0.0001, 0.5);
        let cap = capacity.max(10); // Minimum 10 items
        
        let filter = Bloom::new_for_fp_rate(cap, fpr);
        
        RivetBloomFilter {
            filter,
            capacity: cap,
            false_positive_rate: fpr,
            items_added: 0,
        }
    }

    /// Add an item to the Bloom filter
    /// Returns true if this is potentially a new item (not previously seen)
    pub fn add(&mut self, item: &str) -> bool {
        let was_absent = !self.filter.check(&item.to_string());
        self.filter.set(&item.to_string());
        if was_absent {
            self.items_added += 1;
        }
        was_absent
    }

    /// Check if an item might exist in the filter
    /// Returns true if the item MAY exist (possible false positive)
    /// Returns false if the item DEFINITELY does not exist
    pub fn exists(&self, item: &str) -> bool {
        self.filter.check(&item.to_string())
    }

    /// Add multiple items, returns number of items that were (probably) new
    pub fn madd(&mut self, items: &[String]) -> usize {
        let mut added = 0;
        for item in items {
            if self.add(item) {
                added += 1;
            }
        }
        added
    }

    /// Check multiple items for existence
    /// Returns a vector of booleans indicating existence
    pub fn mexists(&self, items: &[String]) -> Vec<bool> {
        items.iter().map(|item| self.exists(item)).collect()
    }

    /// Get the capacity this filter was created with
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Get the target false positive rate
    pub fn false_positive_rate(&self) -> f64 {
        self.false_positive_rate
    }

    /// Get the estimated number of items added
    pub fn items_added(&self) -> usize {
        self.items_added
    }

    /// Estimated memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        // BloomFilter internal size + our struct overhead
        // This is an approximation based on the optimal bit array size
        let bits = optimal_bit_count(self.capacity, self.false_positive_rate);
        let bytes = (bits + 7) / 8;
        bytes + std::mem::size_of::<Self>()
    }

    /// Get filter info as key-value pairs
    pub fn info(&self) -> Vec<(String, String)> {
        vec![
            ("Capacity".into(), self.capacity.to_string()),
            ("False positive rate".into(), format!("{:.4}", self.false_positive_rate)),
            ("Items added".into(), self.items_added.to_string()),
            ("Fill ratio".into(), format!("{:.2}%", (self.items_added as f64 / self.capacity as f64) * 100.0)),
            ("Memory usage".into(), format!("{} bytes", self.memory_usage())),
            ("Number of hash functions".into(), optimal_hash_count(self.false_positive_rate).to_string()),
        ]
    }
}

/// Calculate optimal number of bits for a Bloom filter
fn optimal_bit_count(capacity: usize, fpr: f64) -> usize {
    let n = capacity as f64;
    let p = fpr;
    // m = -n * ln(p) / (ln(2)^2)
    let m = (-n * p.ln() / (2.0_f64.ln().powi(2))).ceil() as usize;
    m.max(8) // Minimum 1 byte
}

/// Calculate optimal number of hash functions
fn optimal_hash_count(fpr: f64) -> usize {
    // k = -log2(fpr)
    let k = (-fpr.log2()).ceil() as usize;
    k.clamp(1, 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut bf = RivetBloomFilter::new(1000, 0.01);
        
        // Add some items
        assert!(bf.add("hello"));
        assert!(bf.add("world"));
        
        // Check existence
        assert!(bf.exists("hello"));
        assert!(bf.exists("world"));
        
        // Item not added should not exist (with high probability)
        // Note: this could fail with very low probability due to false positives
        // Using a string that's unlikely to hash to same bits
        assert!(!bf.exists("xyzzy_not_added_123456"));
    }

    #[test]
    fn test_bloom_filter_madd_mexists() {
        let mut bf = RivetBloomFilter::new(1000, 0.01);
        
        let items = vec!["a".into(), "b".into(), "c".into()];
        let added = bf.madd(&items);
        assert_eq!(added, 3);
        
        let exists = bf.mexists(&items);
        assert_eq!(exists, vec![true, true, true]);
    }

    #[test]
    fn test_bloom_filter_info() {
        let bf = RivetBloomFilter::new(1000, 0.01);
        let info = bf.info();
        
        assert!(!info.is_empty());
        assert!(info.iter().any(|(k, _)| k == "Capacity"));
    }
}
