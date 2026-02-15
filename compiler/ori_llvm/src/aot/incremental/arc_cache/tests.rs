use super::*;
use ori_arc::{
    ArcBlock, ArcBlockId, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId, LitValue,
    Ownership,
};
use ori_ir::Name;
use ori_types::Idx;

fn sample_arc_function() -> ArcFunction {
    ArcFunction {
        name: Name::from_raw(1),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::INT, Idx::INT],
        spans: vec![vec![None]],
    }
}

#[test]
fn test_cached_arc_ir_roundtrip() {
    let funcs = vec![sample_arc_function()];

    let cached =
        CachedArcIr::from_arc_functions(&funcs).unwrap_or_else(|e| panic!("serialize failed: {e}"));

    let restored = cached
        .to_arc_functions()
        .unwrap_or_else(|e| panic!("deserialize failed: {e}"));

    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].name, funcs[0].name);
    assert_eq!(restored[0].blocks, funcs[0].blocks);
    // Spans are skipped in serialization
    assert!(restored[0].spans.is_empty());
}

#[test]
fn test_arc_cache_put_get() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
    let cache =
        ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

    let key = ArcIrCacheKey {
        function_hash: ContentHash::new(12345),
    };

    // Cache miss
    assert!(!cache.has(&key));
    assert!(cache.get(&key).is_none());

    // Put
    let cached = CachedArcIr::from_arc_functions(&[sample_arc_function()])
        .unwrap_or_else(|e| panic!("serialize failed: {e}"));
    cache
        .put(&key, &cached)
        .unwrap_or_else(|e| panic!("put failed: {e}"));

    // Cache hit
    assert!(cache.has(&key));
    let retrieved = cache
        .get(&key)
        .unwrap_or_else(|| panic!("cache should contain entry"));
    let funcs = retrieved
        .to_arc_functions()
        .unwrap_or_else(|e| panic!("deserialize failed: {e}"));
    assert_eq!(funcs.len(), 1);
    assert_eq!(funcs[0].name, Name::from_raw(1));
}

#[test]
fn test_arc_cache_miss_returns_none() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
    let cache =
        ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

    let key = ArcIrCacheKey {
        function_hash: ContentHash::new(99999),
    };

    assert!(cache.get(&key).is_none());
}

#[test]
fn test_arc_cache_clear() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
    let cache =
        ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

    let key = ArcIrCacheKey {
        function_hash: ContentHash::new(42),
    };
    let cached = CachedArcIr::from_arc_functions(&[sample_arc_function()])
        .unwrap_or_else(|e| panic!("serialize failed: {e}"));
    cache
        .put(&key, &cached)
        .unwrap_or_else(|e| panic!("put failed: {e}"));
    assert!(cache.has(&key));

    cache
        .clear()
        .unwrap_or_else(|e| panic!("clear failed: {e}"));
    assert!(!cache.has(&key));
}

#[test]
fn test_different_hash_different_entry() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("failed to create temp dir: {e}"));
    let cache =
        ArcIrCache::new(dir.path()).unwrap_or_else(|e| panic!("failed to create cache: {e}"));

    let key1 = ArcIrCacheKey {
        function_hash: ContentHash::new(100),
    };
    let key2 = ArcIrCacheKey {
        function_hash: ContentHash::new(200),
    };

    let cached = CachedArcIr::from_arc_functions(&[sample_arc_function()])
        .unwrap_or_else(|e| panic!("serialize failed: {e}"));
    cache
        .put(&key1, &cached)
        .unwrap_or_else(|e| panic!("put failed: {e}"));

    // key1 hits, key2 misses
    assert!(cache.has(&key1));
    assert!(!cache.has(&key2));
}
