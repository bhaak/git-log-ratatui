use crate::app::cache::CommitCache;

fn sample_vec() -> Vec<crate::view::CommitRow> {
    vec![]
}

#[test]
fn test_new_cache_is_empty() {
    let cache = CommitCache::new();
    assert!(cache.is_empty());
}

#[test]
fn test_default_cache_is_empty() {
    let cache = CommitCache::default();
    assert!(cache.is_empty());
}

#[test]
fn test_set_full_then_get_full() {
    let mut cache = CommitCache::new();
    cache.set(false, sample_vec());
    assert!(!cache.is_empty());
    assert!(cache.get(false).is_some());
    assert!(cache.is_empty());
}

#[test]
fn test_set_simplified_then_get_simplified() {
    let mut cache = CommitCache::new();
    cache.set(true, sample_vec());
    assert!(!cache.is_empty());
    assert!(cache.get(true).is_some());
    assert!(cache.is_empty());
}

#[test]
fn test_get_without_set_returns_none() {
    let mut cache = CommitCache::new();
    assert!(cache.get(false).is_none());
    assert!(cache.get(true).is_none());
}

#[test]
fn test_get_consumes_entry() {
    let mut cache = CommitCache::new();
    cache.set(false, sample_vec());
    let _ = cache.get(false);
    assert!(cache.get(false).is_none());
    assert!(cache.is_empty());
}

#[test]
fn test_set_same_mode_twice_overwrites() {
    let mut cache = CommitCache::new();
    cache.set(false, vec![]);
    let first = cache.get(false);
    assert!(first.is_some());

    cache.set(false, vec![]);
    let second = cache.get(false);
    assert!(second.is_some());
}

#[test]
fn test_both_modes_populated() {
    let mut cache = CommitCache::new();
    cache.set(false, sample_vec());
    cache.set(true, sample_vec());
    assert!(!cache.is_empty());
    assert!(cache.get(false).is_some());
    assert!(cache.get(true).is_some());
}

#[test]
fn test_invalidate_clears_both() {
    let mut cache = CommitCache::new();
    cache.set(false, sample_vec());
    cache.set(true, sample_vec());
    assert!(!cache.is_empty());
    cache.invalidate();
    assert!(cache.is_empty());
    assert!(cache.get(false).is_none());
}

#[test]
fn test_is_empty_after_partial_invalidation() {
    let mut cache = CommitCache::new();
    cache.set(false, sample_vec());
    cache.set(true, sample_vec());
    assert!(!cache.is_empty());
    let _ = cache.get(false);
    assert!(!cache.is_empty());
    let _ = cache.get(true);
    assert!(cache.is_empty());
}
