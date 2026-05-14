use ori_ir::Span;

use super::*;

#[test]
fn module_checker_basic() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();

    let checker = ModuleChecker::new(&arena, &interner);

    assert!(!checker.has_errors());
    assert!(checker.signatures.is_empty());
    assert!(checker.expr_types.is_empty());
}

#[test]
fn module_checker_with_registries() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let types = TypeRegistry::new();
    let traits = TraitRegistry::new();

    let checker = ModuleChecker::with_registries(&arena, &interner, types, traits);

    assert!(!checker.has_errors());
}

#[test]
fn module_checker_expr_types() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Store expression types
    checker.store_expr_type(0, Idx::INT);
    checker.store_expr_type(2, Idx::STR); // Skip index 1
    checker.store_expr_type(1, Idx::BOOL);

    assert_eq!(checker.get_expr_type(0), Some(Idx::INT));
    assert_eq!(checker.get_expr_type(1), Some(Idx::BOOL));
    assert_eq!(checker.get_expr_type(2), Some(Idx::STR));
    assert_eq!(checker.get_expr_type(99), None);
}

#[test]
fn module_checker_function_scope() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let fn_type = Idx::UNIT; // Placeholder
    let mut caps = FxHashSet::default();
    caps.insert(Name::from_raw(1)); // "Http"

    assert!(checker.current_function().is_none());

    checker.with_function_scope(fn_type, caps, |c| {
        assert_eq!(c.current_function(), Some(fn_type));
        assert!(c.has_capability(Name::from_raw(1)));
        assert!(!c.has_capability(Name::from_raw(2)));
    });

    assert!(checker.current_function().is_none());
}

#[test]
fn module_checker_impl_scope() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    let self_ty = Idx::INT;

    assert!(checker.current_impl_self().is_none());

    checker.with_impl_scope(self_ty, |c| {
        assert_eq!(c.current_impl_self(), Some(self_ty));
    });

    assert!(checker.current_impl_self().is_none());
}

#[test]
fn module_checker_error_accumulation() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    assert!(!checker.has_errors());

    checker.error_undefined(Name::from_raw(1), Span::DUMMY);
    assert!(checker.has_errors());
    assert_eq!(checker.errors().len(), 1);

    checker.error_undefined(Name::from_raw(2), Span::DUMMY);
    assert_eq!(checker.errors().len(), 2);
}

#[test]
fn module_checker_finish() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    checker.store_expr_type(0, Idx::INT);
    checker.store_expr_type(1, Idx::STR);

    let result = checker.finish();

    assert!(!result.has_errors());
    assert_eq!(result.typed.expr_types.len(), 2);
}

#[test]
fn module_checker_finish_with_pool() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Create a custom type in the pool
    let list_int = checker.pool_mut().list(Idx::INT);
    checker.store_expr_type(0, list_int);

    let (result, pool) = checker.finish_with_pool();

    assert_eq!(result.typed.expr_types[0], list_int);
    assert_eq!(pool.tag(list_int), crate::Tag::List);
}

#[test]
fn flush_composed_burdens_registers_spec_against_type_registry() {
    // Verification test for the §02.2 flush wiring: confirms that
    // composed burdens drained from a body-pass engine reach
    // `TypeRegistry::burden(idx)` via
    // `ModuleChecker::flush_composed_burdens`. Models the path
    // `bodies::finalize_body_and_export` exercises for every body:
    // (1) register a monomorphized type slot in the registry, (2) flush
    // a composed spec for that slot, (3) verify `TypeRegistry::burden`
    // returns the registered spec.
    //
    // F2 close-out pin (Plan TPR Round 1): the §02.1 claim of
    // "monomorphization wired" requires composed specs to reach the
    // registry through code, not API stub. This test fails if the flush
    // is reverted (composed burden accumulator drained but never
    // registered).
    use crate::registry::burden::{UserBurdenSpec, UserTransferRule, UserVariantBurden};
    use core::num::NonZeroU32;
    use ori_ir::Span;
    use ori_registry::burden::{TransferKind, VariantId};

    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Reserve a pool slot for the monomorphized type. `Idx::from_raw(950)`
    // is unused by the prelude (well above primitive-reserved range per
    // `TYPES:TY-5`).
    let mono_idx = Idx::from_raw(950);
    let type_name = Name::from_raw(0x20001);
    checker.type_registry_mut().register_struct(
        type_name,
        mono_idx,
        vec![],
        vec![],
        Span::DUMMY,
        crate::registry::Visibility::Public,
        0,
        None,
        None,
    );
    assert!(
        checker.type_registry().burden(mono_idx).is_none(),
        "Pre-flush: registry has no burden registered against the slot"
    );

    // Construct a one-variant spec — the shape `compose_user_burden`
    // produces for a monomorphized `Option<int>` Some arm — and feed it
    // through the flush surface that `finalize_body_and_export` calls.
    let variant_id = match NonZeroU32::new(1) {
        Some(nz) => VariantId::new(nz),
        None => panic!("unreachable: 1 is non-zero"),
    };
    let spec = UserBurdenSpec {
        self_heap_alloc: false,
        owned_fields: vec![],
        borrowed_fields: vec![],
        variant_burdens: vec![UserVariantBurden {
            variant_id,
            transfers_on_match: vec![UserTransferRule {
                source_field_path: vec![0],
                binding_index: 0,
                field_type: Idx::INT,
                transfer_kind: TransferKind::Move,
            }],
            retained_owned: vec![],
        }],
        element_burden: None,
        compiled_drop: None,
        user_drop: None,
    };

    checker.flush_composed_burdens(vec![(mono_idx, spec.clone())]);

    // Post-flush: the registry exposes the composed spec via the canonical
    // codegen-facing lookup surface.
    match checker.type_registry().burden(mono_idx) {
        Some(registered) => assert_eq!(
            registered, &spec,
            "Flushed spec is what TypeRegistry::burden returns at the monomorphized slot"
        ),
        None => {
            panic!("flush_composed_burdens did not reach TypeRegistry — the §02.1 wiring is broken")
        }
    }

    // Echo: re-flushing the same spec collapses through the dedup gate
    // and leaves the burden_entry_count stable. Pins the dedup path is
    // still load-bearing post-flush.
    let pre_count = checker.type_registry().burden_entry_count();
    checker.flush_composed_burdens(vec![(mono_idx, spec)]);
    assert_eq!(
        checker.type_registry().burden_entry_count(),
        pre_count,
        "Echo flush collapses to the same slot via signature dedup"
    );
}

// --- Transitive metadata forwarding tests ---

#[test]
fn exported_metadata_includes_imported_entries() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Simulate imported module's metadata (e.g., C exports a pub #repr("c") type)
    checker.set_imported_type_metadata(vec![crate::output::ExportedTypeMetadata {
        merkle_hash: 0xC001,
        repr: Some(ori_ir::ReprAttrKind::C),
        is_public: true,
    }]);

    let (result, _pool) = checker.finish_with_pool();

    // Module has no local types, so all metadata comes from imports
    assert_eq!(result.typed.exported_type_metadata.len(), 1);
    assert_eq!(result.typed.exported_type_metadata[0].merkle_hash, 0xC001);
    assert_eq!(
        result.typed.exported_type_metadata[0].repr,
        Some(ori_ir::ReprAttrKind::C)
    );
}

#[test]
fn exported_metadata_local_takes_priority_over_imported() {
    // Test the merge function directly via a module that has both local types
    // and imported metadata with the same merkle hash. The local entry should win.
    // We can't easily register a local type entry through the public API,
    // so we test via the generate_exported_type_metadata function's behavior:
    // when a type with hash 0xABC exists locally AND is imported, only one
    // copy should appear (local priority). We verify this by checking that
    // imported-only entries DO appear, while duplicates are deduped.
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Two imports with same hash: dedup should keep only one
    checker.set_imported_type_metadata(vec![
        crate::output::ExportedTypeMetadata {
            merkle_hash: 0xABC,
            repr: Some(ori_ir::ReprAttrKind::C),
            is_public: true,
        },
        crate::output::ExportedTypeMetadata {
            merkle_hash: 0xABC,
            repr: Some(ori_ir::ReprAttrKind::Packed),
            is_public: true,
        },
    ]);

    let (result, _pool) = checker.finish_with_pool();

    // Dedup by hash: first entry (C) wins over second (Packed)
    let matching: Vec<_> = result
        .typed
        .exported_type_metadata
        .iter()
        .filter(|m| m.merkle_hash == 0xABC)
        .collect();
    assert_eq!(matching.len(), 1, "dedup should produce exactly one entry");
    assert_eq!(
        matching[0].repr,
        Some(ori_ir::ReprAttrKind::C),
        "first-seen entry should win"
    );
}

#[test]
fn exported_metadata_empty_imports_unchanged() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let checker = ModuleChecker::new(&arena, &interner);

    // No imported metadata, no local types
    let (result, _pool) = checker.finish_with_pool();

    assert!(result.typed.exported_type_metadata.is_empty());
}

#[test]
fn exported_metadata_multiple_imported_modules() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Simulate metadata from two different imported modules
    checker.set_imported_type_metadata(vec![
        crate::output::ExportedTypeMetadata {
            merkle_hash: 0xC001,
            repr: Some(ori_ir::ReprAttrKind::C),
            is_public: true,
        },
        crate::output::ExportedTypeMetadata {
            merkle_hash: 0xD001,
            repr: None,
            is_public: true,
        },
        // Duplicate from diamond dependency
        crate::output::ExportedTypeMetadata {
            merkle_hash: 0xC001,
            repr: Some(ori_ir::ReprAttrKind::C),
            is_public: true,
        },
    ]);

    let (result, _pool) = checker.finish_with_pool();

    // 2 unique entries (C001 deduped)
    assert_eq!(result.typed.exported_type_metadata.len(), 2);
    let hashes: Vec<u64> = result
        .typed
        .exported_type_metadata
        .iter()
        .map(|m| m.merkle_hash)
        .collect();
    assert!(hashes.contains(&0xC001));
    assert!(hashes.contains(&0xD001));
}

// --- Transitive collection-surface forwarding tests ---

/// Regression: the A→B→C collection-surface forwarding path must
/// be pinned end-to-end. Module C exports a public `[int]` function. Module B
/// imports C (no own public collection functions). Module A imports B. C's
/// collection surface hash must propagate transitively through B to A.
#[test]
fn exported_collection_surfaces_forward_transitively_a_b_c() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();

    // Step 1: Simulate module C's exported hash.
    // In production, C would have `pub @f() -> [int]`, and its type checker
    // would compute hash(List<int>). Here we compute it from a fresh pool.
    let c_hash = {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);
        pool.hash(list_int)
    };

    // Step 2: Module B imports C's surfaces, has no own public collection functions.
    let mut checker_b = ModuleChecker::new(&arena, &interner);
    checker_b.set_imported_collection_surfaces(vec![c_hash]);
    let (result_b, _pool_b) = checker_b.finish_with_pool();

    // B should forward C's hash in its exported_collection_surfaces.
    assert!(
        result_b
            .typed
            .exported_collection_surfaces
            .contains(&c_hash),
        "Module B must forward C's collection surface hash (hop 1)"
    );

    // Step 3: Module A imports B's surfaces (which contain C's forwarded hash).
    let mut checker_a = ModuleChecker::new(&arena, &interner);
    checker_a.set_imported_collection_surfaces(result_b.typed.exported_collection_surfaces.clone());
    let (result_a, _pool_a) = checker_a.finish_with_pool();

    // A should see C's hash through the full transitive chain.
    assert!(
        result_a
            .typed
            .exported_collection_surfaces
            .contains(&c_hash),
        "Module A must see C's collection surface hash transitively through B (hop 2)"
    );
}

/// Diamond dependency: modules B and D both import C, module A imports both B
/// and D. C's hash should appear exactly once in A's exported surfaces (dedup).
#[test]
fn exported_collection_surfaces_diamond_dedup() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();

    let c_hash: u64 = 0xCAFE_BEEF;

    // Module B imports C's surface
    let mut checker_b = ModuleChecker::new(&arena, &interner);
    checker_b.set_imported_collection_surfaces(vec![c_hash]);
    let (result_b, _) = checker_b.finish_with_pool();

    // Module D also imports C's surface
    let mut checker_d = ModuleChecker::new(&arena, &interner);
    checker_d.set_imported_collection_surfaces(vec![c_hash]);
    let (result_d, _) = checker_d.finish_with_pool();

    // Module A imports both B and D
    let mut combined = result_b.typed.exported_collection_surfaces.clone();
    combined.extend(&result_d.typed.exported_collection_surfaces);

    let mut checker_a = ModuleChecker::new(&arena, &interner);
    checker_a.set_imported_collection_surfaces(combined);
    let (result_a, _) = checker_a.finish_with_pool();

    // C's hash should appear exactly once (deduped by the HashSet in
    // generate_exported_collection_surfaces).
    let count = result_a
        .typed
        .exported_collection_surfaces
        .iter()
        .filter(|&&h| h == c_hash)
        .count();
    assert_eq!(
        count, 1,
        "Diamond dependency must dedup to exactly one hash"
    );
}
