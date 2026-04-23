use ordered_float::NotNan;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Sorted Set (ZSet) - stores members with scores
/// Optimized for both score-based and member-based lookups
#[derive(Debug, Clone)]
pub struct ZSet {
    // Score -> Members mapping for range queries by score
    // Using BTreeMap for sorted access, HashSet for multiple members with same score
    by_score: BTreeMap<NotNan<f64>, HashSet<String>>,
    
    // Member -> Score mapping for O(1) member lookup
    scores: HashMap<String, NotNan<f64>>,
}

impl ZSet {
    /// Create a new empty sorted set
    pub fn new() -> Self {
        ZSet {
            by_score: BTreeMap::new(),
            scores: HashMap::new(),
        }
    }

    /// Add a member with a score, returns true if member was added (not just updated)
    pub fn add(&mut self, member: String, score: f64) -> bool {
        let score = NotNan::new(score).unwrap_or(NotNan::new(0.0).unwrap());
        
        // Check if member already exists
        let is_new = if let Some(&old_score) = self.scores.get(&member) {
            // Remove from old score bucket if score changed
            if old_score != score {
                if let Some(members) = self.by_score.get_mut(&old_score) {
                    members.remove(&member);
                    if members.is_empty() {
                        self.by_score.remove(&old_score);
                    }
                }
                true // Score changed, counts as modification
            } else {
                false // Same score, no change
            }
        } else {
            true // New member
        };

        // Add to new score bucket
        self.scores.insert(member.clone(), score);
        self.by_score
            .entry(score)
            .or_insert_with(HashSet::new)
            .insert(member);

        is_new
    }

    /// Remove a member, returns true if member existed
    pub fn remove(&mut self, member: &str) -> bool {
        if let Some(score) = self.scores.remove(member) {
            if let Some(members) = self.by_score.get_mut(&score) {
                members.remove(member);
                if members.is_empty() {
                    self.by_score.remove(&score);
                }
            }
            true
        } else {
            false
        }
    }

    /// Get score of a member
    pub fn score(&self, member: &str) -> Option<f64> {
        self.scores.get(member).map(|s| s.into_inner())
    }

    /// Get number of members
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    /// Get members by rank range (0-indexed)
    /// Supports negative indices from the end
    pub fn range_by_rank(&self, start: isize, stop: isize) -> Vec<(String, f64)> {
        let len = self.len() as isize;
        if len == 0 {
            return vec![];
        }

        let start_idx = if start < 0 { (len + start).max(0) } else { start.min(len) } as usize;
        let stop_idx = if stop < 0 { (len + stop).max(-1) } else { stop.min(len - 1) } as usize;

        if start_idx >= self.len() || stop_idx < start_idx {
            return vec![];
        }

        self.all_members()
            .into_iter()
            .skip(start_idx)
            .take(stop_idx - start_idx + 1)
            .collect()
    }

    /// Get members by score range
    pub fn range_by_score(&self, min: f64, max: f64, limit: Option<usize>) -> Vec<(String, f64)> {
        let min_score = NotNan::new(min).unwrap_or(NotNan::new(f64::NEG_INFINITY).unwrap());
        let max_score = NotNan::new(max).unwrap_or(NotNan::new(f64::INFINITY).unwrap());

        let mut result = Vec::new();
        
        for (&score, members) in self.by_score.range(min_score..=max_score) {
            for member in members {
                result.push((member.clone(), score.into_inner()));
                if let Some(lim) = limit {
                    if result.len() >= lim {
                        return result;
                    }
                }
            }
        }

        result
    }

    /// Get rank of a member (0-indexed, lower scores have lower rank)
    pub fn rank(&self, member: &str) -> Option<usize> {
        let target_score = self.scores.get(member)?;
        
        let mut rank = 0;
        for (&score, members) in &self.by_score {
            if score < *target_score {
                rank += members.len();
            } else if score == *target_score {
                // Count members with same score that are lexicographically before this one
                for m in members {
                    if m.as_str() < member {
                        rank += 1;
                    } else if m == member {
                        return Some(rank);
                    }
                }
            }
        }
        
        None
    }

    /// Get reverse rank (highest scores have rank 0)
    pub fn rev_rank(&self, member: &str) -> Option<usize> {
        self.rank(member).map(|r| self.len() - r - 1)
    }

    /// Get all members in score order
    fn all_members(&self) -> Vec<(String, f64)> {
        let mut result = Vec::new();
        for (&score, members) in &self.by_score {
            let mut sorted_members: Vec<_> = members.iter().cloned().collect();
            sorted_members.sort();
            for member in sorted_members {
                result.push((member, score.into_inner()));
            }
        }
        result
    }

    /// Increment score of a member
    pub fn increment(&mut self, member: String, delta: f64) -> f64 {
        let current_score = self.score(&member).unwrap_or(0.0);
        let new_score = current_score + delta;
        self.add(member, new_score);
        new_score
    }

    /// Pop minimum score member
    pub fn pop_min(&mut self) -> Option<(String, f64)> {
        let (&score, members) = self.by_score.iter_mut().next()?;
        let member = members.iter().next()?.clone();
        self.remove(&member);
        Some((member, score.into_inner()))
    }

    /// Pop maximum score member
    pub fn pop_max(&mut self) -> Option<(String, f64)> {
        let (&score, members) = self.by_score.iter_mut().next_back()?;
        let member = members.iter().next()?.clone();
        self.remove(&member);
        Some((member, score.into_inner()))
    }

    /// Count members within score range
    pub fn count(&self, min: f64, max: f64) -> usize {
        let min_score = NotNan::new(min).unwrap_or(NotNan::new(f64::NEG_INFINITY).unwrap());
        let max_score = NotNan::new(max).unwrap_or(NotNan::new(f64::INFINITY).unwrap());

        self.by_score
            .range(min_score..=max_score)
            .map(|(_, members)| members.len())
            .sum()
    }

    /// Remove members by rank range
    pub fn remove_range_by_rank(&mut self, start: isize, stop: isize) -> usize {
        let members_to_remove = self.range_by_rank(start, stop);
        let count = members_to_remove.len();
        
        for (member, _) in members_to_remove {
            self.remove(&member);
        }
        
        count
    }

    /// Remove members by score range
    pub fn remove_range_by_score(&mut self, min: f64, max: f64) -> usize {
        let members_to_remove = self.range_by_score(min, max, None);
        let count = members_to_remove.len();
        
        for (member, _) in members_to_remove {
            self.remove(&member);
        }
        
        count
    }

    /// Add with options (NX, XX, GT, LT)
    /// Returns (changed, new_score)
    pub fn add_with_options(
        &mut self,
        member: String,
        score: f64,
        nx: bool,  // Only add new elements
        xx: bool,  // Only update existing elements
        gt: bool,  // Only update if new score > current score
        lt: bool,  // Only update if new score < current score
    ) -> (bool, Option<f64>) {
        let current_score = self.score(&member);
        
        // NX: Only add if doesn't exist
        if nx && current_score.is_some() {
            return (false, current_score);
        }
        
        // XX: Only update if exists
        if xx && current_score.is_none() {
            return (false, None);
        }
        
        // GT: Only update if new score > current
        if gt {
            if let Some(curr) = current_score {
                if score <= curr {
                    return (false, Some(curr));
                }
            }
        }
        
        // LT: Only update if new score < current
        if lt {
            if let Some(curr) = current_score {
                if score >= curr {
                    return (false, Some(curr));
                }
            }
        }
        
        self.add(member, score);
        (true, Some(score))
    }

    /// Get all members with their scores (for set operations)
    pub fn all_with_scores(&self) -> HashMap<String, f64> {
        self.scores.iter().map(|(k, &v)| (k.clone(), v.into_inner())).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zset_add() {
        let mut zset = ZSet::new();
        assert!(zset.add("alice".into(), 100.0));
        assert!(zset.add("bob".into(), 200.0));
        assert_eq!(zset.len(), 2);
    }

    #[test]
    fn test_zset_score() {
        let mut zset = ZSet::new();
        zset.add("alice".into(), 100.0);
        assert_eq!(zset.score("alice"), Some(100.0));
        assert_eq!(zset.score("bob"), None);
    }

    #[test]
    fn test_zset_rank() {
        let mut zset = ZSet::new();
        zset.add("alice".into(), 100.0);
        zset.add("bob".into(), 200.0);
        zset.add("charlie".into(), 150.0);
        
        assert_eq!(zset.rank("alice"), Some(0));
        assert_eq!(zset.rank("charlie"), Some(1));
        assert_eq!(zset.rank("bob"), Some(2));
    }

    #[test]
    fn test_zset_range() {
        let mut zset = ZSet::new();
        zset.add("a".into(), 1.0);
        zset.add("b".into(), 2.0);
        zset.add("c".into(), 3.0);
        
        let range = zset.range_by_rank(0, 1);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].0, "a");
        assert_eq!(range[1].0, "b");
    }
}
