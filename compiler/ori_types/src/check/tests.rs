use ori_ir::{ExprArena, Name, Span, StringInterner};
use rustc_hash::FxHashSet;

use crate::{Idx, Pool, TraitRegistry, TypeRegistry};

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
fn finish_with_pool_exports_monomorphized_list_collection_burden() {
    // Export-path pin: a burden registered against a monomorphized builtin
    // collection instance (`[str]`, which has no nominal `TypeEntry`) lands in
    // the `TypeRegistry::collection_burdens` side-table and MUST surface on
    // `TypedModule.collection_burdens` after `finish_with_pool`. The side-table
    // is excluded from `types` (sourced from `into_entries`), so without the
    // explicit export the collection-instance burden would never reach the ARC
    // codegen pipeline. Spec: Annex E §AIMS.
    use crate::registry::burden::{UserBurdenSpec, UserOwnedField};

    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Monomorphize `[str]` in the pool — a genuine builtin collection instance
    // with NO nominal `TypeEntry` (struct / enum / newtype / alias).
    let list_str = checker.pool_mut().list(Idx::STR);
    assert!(
        checker.type_registry().get_by_idx(list_str).is_none(),
        "Precondition: a monomorphized [str] instance has no nominal TypeEntry"
    );

    // The element-burden shape `compose_user_burden` produces for `[str]`: the
    // buffer owns its `str` elements.
    let spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: Some(Idx::STR),
        drop_operation: None,
        user_drop: None,
    };
    let canonical = checker
        .type_registry_mut()
        .register_user_burden(list_str, spec.clone());
    assert_eq!(
        canonical, list_str,
        "Side-table registration is canonical at the [str] instance Idx"
    );
    // Burden landed in the side-table, NOT on a nominal entry.
    assert!(
        checker.type_registry().get_by_idx(list_str).is_none(),
        "Burden for [str] lives in the collection_burdens side-table, not a TypeEntry"
    );

    let (result, _pool) = checker.finish_with_pool();

    // The export carries the side-table entry verbatim.
    assert_eq!(
        result.typed.collection_burdens.len(),
        1,
        "TypedModule.collection_burdens carries the [str] instance burden"
    );
    let (exported_idx, exported_spec) = &result.typed.collection_burdens[0];
    assert_eq!(
        *exported_idx, list_str,
        "Exported entry keyed by the [str] Idx"
    );
    assert_eq!(
        exported_spec, &spec,
        "Exported spec is structurally identical to the registered burden"
    );
}

#[test]
fn finish_with_pool_collection_burdens_sorted_and_excludes_nominal_entries() {
    // Pins two export-path invariants: (1) `collection_burdens` is sorted
    // ascending by `Idx` for Salsa-deterministic output; (2) nominal-type
    // burdens (struct / enum / newtype) stay on `types`, NEVER on the
    // side-table export. Spec: Annex E §AIMS.
    use crate::registry::burden::{UserBurdenSpec, UserOwnedField};
    use ori_ir::Span;

    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Two monomorphized collection instances: `[str]` and `{str: str}`. Their
    // pool Idx ordering is not guaranteed, so the export must sort.
    let list_str = checker.pool_mut().list(Idx::STR);
    let map_str_str = checker.pool_mut().map(Idx::STR, Idx::STR);

    let list_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: Some(Idx::STR),
        drop_operation: None,
        user_drop: None,
    };
    let map_spec = UserBurdenSpec {
        self_owned_identity: true,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: Some(Idx::STR),
        drop_operation: None,
        user_drop: None,
    };
    checker
        .type_registry_mut()
        .register_user_burden(map_str_str, map_spec);
    checker
        .type_registry_mut()
        .register_user_burden(list_str, list_spec);

    // A nominal struct WITH a burden — must surface on `types`, never on the
    // side-table export.
    let nominal_idx = Idx::from_raw(951);
    let nominal_name = Name::from_raw(0x20009);
    let nominal_spec = UserBurdenSpec {
        self_owned_identity: false,
        owned_fields: vec![UserOwnedField {
            field_path: vec![0],
            field_type: Idx::STR,
        }],
        borrowed_fields: vec![],
        variant_burdens: vec![],
        element_burden: None,
        drop_operation: None,
        user_drop: None,
    };
    checker.type_registry_mut().register_struct(
        nominal_name,
        nominal_idx,
        vec![],
        vec![],
        Span::DUMMY,
        crate::registry::Visibility::Public,
        0,
        None,
        Some(nominal_spec.clone()),
    );

    let (result, _pool) = checker.finish_with_pool();

    // Exactly the two collection instances, sorted ascending by Idx.
    let exported = &result.typed.collection_burdens;
    assert_eq!(
        exported.len(),
        2,
        "Both monomorphized collection instances export; nominal burden does not"
    );
    assert!(
        exported.windows(2).all(|w| w[0].0.raw() < w[1].0.raw()),
        "collection_burdens is sorted ascending by Idx for Salsa determinism"
    );
    let exported_idxs: Vec<Idx> = exported.iter().map(|(idx, _)| *idx).collect();
    assert!(
        exported_idxs.contains(&list_str) && exported_idxs.contains(&map_str_str),
        "Both [str] and {{str: str}} instance burdens are exported"
    );
    assert!(
        !exported_idxs.contains(&nominal_idx),
        "Nominal-struct burden stays on `types`, never the side-table export"
    );

    // The nominal burden rode out on `types`, not the side-table.
    let nominal_entry = result
        .typed
        .types
        .iter()
        .find(|te| te.idx == nominal_idx)
        .unwrap_or_else(|| panic!("nominal struct present in TypedModule.types"));
    assert_eq!(
        nominal_entry.burden.as_ref(),
        Some(&nominal_spec),
        "Nominal burden travels on its TypeEntry, not collection_burdens"
    );
}

// Transitive metadata forwarding tests

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

// Transitive collection-surface forwarding tests

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
    checker_a.set_imported_collection_surfaces(result_b.typed.exported_collection_surfaces);
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
    let mut combined = result_b.typed.exported_collection_surfaces;
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
