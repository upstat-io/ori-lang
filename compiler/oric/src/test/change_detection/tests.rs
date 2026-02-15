use ori_ir::canon::{
    CanArena, CanExpr, CanNode, CanonResult, CanonRoot, ConstantPool, DecisionTreePool,
};
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
        });
    }

    CanonResult {
        arena,
        constants: ConstantPool::default(),
        decision_trees: DecisionTreePool::default(),
        root: ori_ir::canon::CanId::INVALID,
        roots: canon_roots,
        method_roots: vec![],
        problems: vec![],
    }
}

/// Build a Module with test definitions.
fn make_module(tests: &[(u32, &[u32])]) -> Module {
    let mut module = Module::new();
    for &(name_raw, target_raws) in tests {
        module.tests.push(TestDef {
            name: Name::from_raw(name_raw),
            targets: target_raws.iter().map(|&r| Name::from_raw(r)).collect(),
            params: ori_ir::ParamRange::default(),
            return_ty: None,
            body: ExprId::new(0),
            span: Span::DUMMY,
            skip_reason: None,
            fail_expected: None,
            expected_errors: vec![],
        });
    }
    module
}

// ── FunctionChangeMap ────────────────────────────────────────

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

// ── TestTargetIndex ──────────────────────────────────────────

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

// ── TestRunCache ─────────────────────────────────────────────

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
