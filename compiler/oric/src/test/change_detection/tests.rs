use ori_ir::canon::{CanArena, CanExpr, CanNode, CanonResult, CanonRoot};
use ori_ir::{ExprId, Name, Span, TypeId};

use super::*;

/// Build a `CanonResult` with named roots at given body hashes.
fn make_canon(roots: &[(u32, i64)]) -> CanonResult {
    let mut arena = CanArena::new();
    let mut canon_roots = Vec::new();

    for &(name_raw, body_value) in roots {
        let body = arena.push(CanNode::new(
            CanExpr::Int(body_value),
            Span::DUMMY,
            TypeId::INT,
        ));
        canon_roots.push(CanonRoot {
            name: Name::from_raw(name_raw),
            body,
            defaults: vec![],
            param_names: vec![],
        });
    }

    CanonResult {
        arena,
        roots: canon_roots,
        ..CanonResult::empty()
    }
}

/// Build a Module with test definitions.
fn make_module(tests: &[(u32, &[u32])]) -> Module {
    let mut module = Module::new();
    for &(name_raw, target_raws) in tests {
        let Ok(test_id) = u32::try_from(module.tests.len()) else {
            panic!("test fixture count must fit u32")
        };
        module.tests.push(TestDef {
            id: ori_ir::TestId::new(test_id),
            name: Name::from_raw(name_raw),
            display_name: Name::from_raw(name_raw),
            targets: target_raws.iter().map(|&r| Name::from_raw(r)).collect(),
            params: ori_ir::ParamRange::default(),
            return_ty: None,
            body: ExprId::new(0),
            span: Span::DUMMY,
            skip_reason: None,
            skip_backends: vec![],
            fail_expected: None,
            expected_errors: vec![],
        });
    }
    module
}

// FunctionChangeMap

#[test]
fn change_map_from_canon() {
    let canon = make_canon(&[(1, 42), (2, 99)]);
    let map = FunctionChangeMap::from_canon(&canon);
    assert_eq!(map.len(), 2);
    assert!(map.get(Name::from_raw(1)).is_some());
    assert!(map.get(Name::from_raw(2)).is_some());
}

#[test]
fn no_changes_detected_for_identical_canons() {
    let canon1 = make_canon(&[(1, 42), (2, 99)]);
    let canon2 = make_canon(&[(1, 42), (2, 99)]);
    let map1 = FunctionChangeMap::from_canon(&canon1);
    let map2 = FunctionChangeMap::from_canon(&canon2);
    let changed = map2.changed_since(&map1);
    assert!(
        changed.is_empty(),
        "identical canons should have no changes"
    );
}

#[test]
fn body_change_detected() {
    let canon1 = make_canon(&[(1, 42), (2, 99)]);
    let canon2 = make_canon(&[(1, 42), (2, 100)]); // function 2 body changed
    let map1 = FunctionChangeMap::from_canon(&canon1);
    let map2 = FunctionChangeMap::from_canon(&canon2);
    let changed = map2.changed_since(&map1);

    assert_eq!(changed.len(), 1);
    assert!(changed.contains(&Name::from_raw(2)));
}

#[test]
fn new_function_detected_as_changed() {
    let canon1 = make_canon(&[(1, 42)]);
    let canon2 = make_canon(&[(1, 42), (2, 99)]); // function 2 is new
    let map1 = FunctionChangeMap::from_canon(&canon1);
    let map2 = FunctionChangeMap::from_canon(&canon2);
    let changed = map2.changed_since(&map1);

    assert!(changed.contains(&Name::from_raw(2)));
}

#[test]
fn deleted_function_detected_as_changed() {
    let canon1 = make_canon(&[(1, 42), (2, 99)]);
    let canon2 = make_canon(&[(1, 42)]); // function 2 deleted
    let map1 = FunctionChangeMap::from_canon(&canon1);
    let map2 = FunctionChangeMap::from_canon(&canon2);
    let changed = map2.changed_since(&map1);

    assert!(changed.contains(&Name::from_raw(2)));
}

// TestTargetIndex

#[test]
fn index_bidirectional_mapping() {
    // test 100 targets functions 1, 2
    // test 101 targets function 2
    let module = make_module(&[(100, &[1, 2]), (101, &[2])]);
    let index = TestTargetIndex::from_module(&module);

    // Forward: function 1 → test 100
    assert_eq!(index.tests_for(Name::from_raw(1)).len(), 1);

    // Forward: function 2 → tests 100, 101
    assert_eq!(index.tests_for(Name::from_raw(2)).len(), 2);

    // Reverse: test 100 → functions 1, 2
    assert_eq!(index.targets_for(Name::from_raw(100)).len(), 2);

    // Reverse: test 101 → function 2
    assert_eq!(index.targets_for(Name::from_raw(101)).len(), 1);
}

#[test]
fn tests_for_changed_functions() {
    let module = make_module(&[(100, &[1, 2]), (101, &[2]), (102, &[3])]);
    let index = TestTargetIndex::from_module(&module);

    let mut changed = FxHashSet::default();
    changed.insert(Name::from_raw(2)); // function 2 changed

    let affected = index.tests_for_changed(&changed);
    // Tests 100 and 101 target function 2
    assert!(affected.contains(&Name::from_raw(100)));
    assert!(affected.contains(&Name::from_raw(101)));
    // Test 102 targets function 3 (unchanged)
    assert!(!affected.contains(&Name::from_raw(102)));
}

#[test]
fn floating_tests_never_skipped() {
    // test 100 has no targets (floating)
    let module = make_module(&[(100, &[])]);
    let index = TestTargetIndex::from_module(&module);
    let changed = FxHashSet::default(); // nothing changed

    let test_refs: Vec<&TestDef> = module.tests.iter().collect();
    let skippable = index.skippable_tests(&changed, &test_refs);
    assert!(
        skippable.is_empty(),
        "floating tests should never be skipped"
    );
}

#[test]
fn targeted_tests_skipped_when_targets_unchanged() {
    let module = make_module(&[(100, &[1]), (101, &[2])]);
    let index = TestTargetIndex::from_module(&module);

    let mut changed = FxHashSet::default();
    changed.insert(Name::from_raw(1)); // only function 1 changed

    let test_refs: Vec<&TestDef> = module.tests.iter().collect();
    let skippable = index.skippable_tests(&changed, &test_refs);

    // Test 101 (targets function 2) can be skipped
    assert!(skippable.contains(&Name::from_raw(101)));
    // Test 100 (targets function 1) must re-run
    assert!(!skippable.contains(&Name::from_raw(100)));
}

#[test]
fn test_body_change_prevents_skip() {
    let module = make_module(&[(100, &[1])]);
    let index = TestTargetIndex::from_module(&module);

    let mut changed = FxHashSet::default();
    // Function 1 unchanged, but test 100's own body changed
    changed.insert(Name::from_raw(100));

    let test_refs: Vec<&TestDef> = module.tests.iter().collect();
    let skippable = index.skippable_tests(&changed, &test_refs);
    assert!(
        skippable.is_empty(),
        "test with changed body should not be skipped",
    );
}

// TestRunCache

#[test]
fn cache_insert_and_get() {
    let mut cache = TestRunCache::new();
    assert!(cache.is_empty());

    let canon = make_canon(&[(1, 42)]);
    let map = FunctionChangeMap::from_canon(&canon);
    cache.insert(PathBuf::from("/test.ori"), map);

    assert_eq!(cache.len(), 1);
    assert!(cache.get(Path::new("/test.ori")).is_some());
    assert!(cache.get(Path::new("/other.ori")).is_none());
}

// compute_skippable_and_update

#[test]
fn test_compute_skippable_first_sight_skips_nothing() {
    let cache = parking_lot::Mutex::new(TestRunCache::new());
    let canon = make_canon(&[(1, 42), (100, 7)]);
    let module = make_module(&[(100, &[1])]);

    let skippable = compute_skippable_and_update(&cache, Path::new("/test.ori"), &canon, &module);
    assert!(
        skippable.is_empty(),
        "first sight of a file has no previous snapshot — nothing skippable"
    );
    assert_eq!(cache.lock().len(), 1, "the fresh snapshot must be stored");
}

#[test]
fn test_compute_skippable_unchanged_rerun_skips_targeted_test() {
    let cache = parking_lot::Mutex::new(TestRunCache::new());
    let canon = make_canon(&[(1, 42), (100, 7)]);
    let module = make_module(&[(100, &[1])]);

    let _ = compute_skippable_and_update(&cache, Path::new("/test.ori"), &canon, &module);
    let skippable = compute_skippable_and_update(&cache, Path::new("/test.ori"), &canon, &module);
    assert!(
        skippable.contains(&Name::from_raw(100)),
        "an unchanged targeted test must be skippable on the second run"
    );
}

#[test]
fn test_compute_skippable_changed_target_reruns_test() {
    let cache = parking_lot::Mutex::new(TestRunCache::new());
    let module = make_module(&[(100, &[1])]);

    let canon_v1 = make_canon(&[(1, 42), (100, 7)]);
    let _ = compute_skippable_and_update(&cache, Path::new("/test.ori"), &canon_v1, &module);

    // Target function 1's body changed between runs.
    let canon_v2 = make_canon(&[(1, 43), (100, 7)]);
    let skippable =
        compute_skippable_and_update(&cache, Path::new("/test.ori"), &canon_v2, &module);
    assert!(
        !skippable.contains(&Name::from_raw(100)),
        "a test whose target changed must re-run"
    );
}

// Cache persistence (save_to / load_from)

#[test]
fn test_cache_save_load_roundtrip_preserves_hashes_across_interners() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let cache_path = dir.path().join("cache");

    // Names must round-trip by STRING (interned ids are process-local).
    let writer_interner = crate::ir::StringInterner::new();
    let double = writer_interner.intern("double");
    let triple = writer_interner.intern("triple");
    let mut map = FunctionChangeMap::default();
    map.insert(double, 0xDEAD_BEEF_0000_0001);
    map.insert(triple, 0x0000_0000_0000_0002);
    let mut cache = TestRunCache::new();
    cache.insert(PathBuf::from("/some dir/test.ori"), map);

    cache
        .save_to(&cache_path, &writer_interner)
        .unwrap_or_else(|e| panic!("save failed: {e}"));

    // A fresh interner models a fresh process: ids differ, strings match.
    let reader_interner = crate::ir::StringInterner::new();
    reader_interner.intern("unrelated-padding");
    let loaded = TestRunCache::load_from(&cache_path, &reader_interner);

    let Some(loaded_map) = loaded.get(Path::new("/some dir/test.ori")) else {
        panic!("loaded cache must carry the saved file entry")
    };
    assert_eq!(
        loaded_map.get(reader_interner.intern("double")),
        Some(0xDEAD_BEEF_0000_0001),
        "hash must round-trip keyed by name string"
    );
    assert_eq!(
        loaded_map.get(reader_interner.intern("triple")),
        Some(0x0000_0000_0000_0002)
    );
}

#[test]
fn test_cache_load_missing_file_returns_empty_cache() {
    let interner = crate::ir::StringInterner::new();
    let loaded = TestRunCache::load_from(Path::new("/nonexistent/ori-cache"), &interner);
    assert!(loaded.is_empty(), "a missing cache file starts cold");
}

#[test]
fn test_cache_load_unrecognized_header_returns_empty_cache() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let cache_path = dir.path().join("cache");
    std::fs::write(&cache_path, "some-other-format v9\nfile /a.ori\n01 f\n")
        .unwrap_or_else(|e| panic!("write failed: {e}"));

    let interner = crate::ir::StringInterner::new();
    let loaded = TestRunCache::load_from(&cache_path, &interner);
    assert!(
        loaded.is_empty(),
        "a version/format mismatch must be treated as an empty cache"
    );
}
