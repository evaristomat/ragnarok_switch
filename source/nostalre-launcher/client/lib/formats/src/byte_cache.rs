//! Byte-bounded LRU for immutable archive entries, not whole GRFs or GPU textures.
use std::collections::HashMap;
pub(crate) struct ByteCache {
    entries: HashMap<String, (Vec<u8>, u64)>, used: usize, clock: u64,
    budget: usize, hits: u64,
}
impl ByteCache {
    pub(crate) fn new(budget: usize) -> Self {
        Self { entries: HashMap::new(), used: 0, clock: 0, budget, hits: 0 }
    }
    pub(crate) fn get(&mut self, name: &str) -> Option<Vec<u8>> {
        let entry = self.entries.get_mut(name)?;
        self.clock = self.clock.wrapping_add(1); entry.1 = self.clock; self.hits += 1;
        Some(entry.0.clone())
    }
    pub(crate) fn insert(&mut self, name: String, bytes: &[u8]) {
        if self.budget == 0 || bytes.len() > self.budget || bytes.len() > 2 * 1024 * 1024 { return; }
        if let Some((old, _)) = self.entries.remove(&name) { self.used -= old.len(); }
        while self.used + bytes.len() > self.budget || self.entries.len() >= 2048 {
            let Some(oldest) = self.entries.iter().min_by_key(|(_, (_, stamp))| *stamp).map(|(key, _)| key.clone()) else { break; };
            if let Some((old, _)) = self.entries.remove(&oldest) { self.used -= old.len(); }
        }
        self.clock = self.clock.wrapping_add(1); self.used += bytes.len();
        self.entries.insert(name, (bytes.to_vec(), self.clock));
    }
    #[cfg(test)] pub(crate) fn hits(&self) -> u64 { self.hits }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn evicts_least_recently_used_and_never_exceeds_byte_budget() {
        let mut cache = ByteCache::new(8);
        cache.insert("a".into(), &[1;4]); cache.insert("b".into(), &[2;4]);
        assert_eq!(cache.get("a"), Some(vec![1;4]));
        cache.insert("c".into(), &[3;4]);
        assert_eq!(cache.get("b"), None); assert_eq!(cache.used,8);
    }
    #[test] fn replacing_a_key_and_rejecting_oversized_entries_keeps_accounting() {
        let mut cache = ByteCache::new(8);
        cache.insert("a".into(), &[1;4]); cache.insert("a".into(), &[2;2]);
        cache.insert("oversized".into(), &[3;9]);
        assert_eq!(cache.used,2); assert_eq!(cache.get("a"),Some(vec![2;2]));
    }
    #[test] fn caller_cannot_mutate_cached_bytes() {
        let mut cache = ByteCache::new(8); cache.insert("a".into(), &[1;4]);
        cache.get("a").unwrap()[0]=9;
        assert_eq!(cache.get("a"),Some(vec![1;4])); assert_eq!(cache.hits(),2);
    }
}

#[cfg(test)]mod limits {
 use super::*;
 #[test]fn zero_length_resources_are_bounded_by_entry_count(){
  let mut c=ByteCache::new(1024);for i in 0..3000{c.insert(i.to_string(),&[]);}
  assert_eq!(c.used,0);assert_eq!(c.entries.len(),2048);
 }
 #[test]fn large_resources_and_disabled_cache_do_not_retain_allocations(){
  let mut c=ByteCache::new(16*1024*1024);c.insert("large".into(),&vec![1;2*1024*1024+1]);
  assert_eq!(c.used,0);assert!(c.entries.is_empty());
  let mut disabled=ByteCache::new(0);disabled.insert("empty".into(),&[]);assert!(disabled.entries.is_empty());
 }
}
