use super::*;

use ori_types::{TypeCheckResult, TypedModule};

/// Helper to create a `TypeCheckResult` with specified collection surfaces.
fn result_with_surfaces(surfaces: Vec<u64>) -> TypeCheckResult {
    TypeCheckResult {
        typed: TypedModule {
            exported_collection_surfaces: surfaces,
            ..TypedModule::default()
        },
        error_guarantee: None,
    }
}

// --- collect_surfaces_from_results tests (TPR-04-041) ---

/// Regression: TPR-04-041 — exercises the exact collection path from
/// `register_resolved_imports()` that gathers collection surface hashes
/// from imported `TypeCheckResult` objects.
#[test]
fn collect_surfaces_from_single_module() {
    let result = result_with_surfaces(vec![0xAAAA, 0xBBBB]);
    let results = vec![Some(result)];

    let surfaces = collect_surfaces_from_results(None, &results);
    assert_eq!(surfaces, vec![0xAAAA, 0xBBBB]);
}

#[test]
fn collect_surfaces_from_multiple_modules() {
    let r1 = result_with_surfaces(vec![0x1111]);
    let r2 = result_with_surfaces(vec![0x2222, 0x3333]);
    let results = vec![Some(r1), Some(r2)];

    let surfaces = collect_surfaces_from_results(None, &results);
    assert_eq!(surfaces, vec![0x1111, 0x2222, 0x3333]);
}

#[test]
fn collect_surfaces_skips_none_results() {
    let r1 = result_with_surfaces(vec![0xAAAA]);
    let results = vec![None, Some(r1), None];

    let surfaces = collect_surfaces_from_results(None, &results);
    assert_eq!(surfaces, vec![0xAAAA]);
}

#[test]
fn collect_surfaces_includes_prelude() {
    let prelude = result_with_surfaces(vec![0xBEEF]);
    let r1 = result_with_surfaces(vec![0xAAAA]);
    let results = vec![Some(r1)];

    let surfaces = collect_surfaces_from_results(Some(&prelude), &results);
    assert_eq!(surfaces.len(), 2);
    assert!(surfaces.contains(&0xBEEF));
    assert!(surfaces.contains(&0xAAAA));
}

#[test]
fn collect_surfaces_empty_modules() {
    let results: Vec<Option<TypeCheckResult>> = vec![];
    let surfaces = collect_surfaces_from_results(None, &results);
    assert!(surfaces.is_empty());
}

/// A→B→C transitive forwarding at the `register_resolved_imports()` level:
/// C's surfaces are in B's `TypeCheckResult`, B's surfaces are in A's input.
#[test]
fn collect_surfaces_transitive_a_b_c() {
    let c_hash: u64 = 0xC0DE;

    // Module C exports surfaces [c_hash].
    // Module B imports C, so B's TypeCheckResult includes C's hash
    // (forwarded by generate_exported_collection_surfaces in B's type checker).
    let module_b = result_with_surfaces(vec![c_hash]);

    // Module A imports B.
    let a_results = vec![Some(module_b)];
    let surfaces = collect_surfaces_from_results(None, &a_results);

    // A should see C's hash via B's forwarded surfaces.
    assert!(
        surfaces.contains(&c_hash),
        "A must see C's collection surface hash transitively through B"
    );
}

// --- collect_metadata_from_results tests ---

#[test]
fn collect_metadata_from_single_module() {
    let result = TypeCheckResult {
        typed: TypedModule {
            exported_type_metadata: vec![ori_types::ExportedTypeMetadata {
                merkle_hash: 0xABCD,
                repr: None,
                is_public: true,
            }],
            ..TypedModule::default()
        },
        error_guarantee: None,
    };
    let results = vec![Some(result)];

    let metadata = collect_metadata_from_results(None, &results);
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].merkle_hash, 0xABCD);
}

#[test]
fn collect_metadata_empty_modules() {
    let results: Vec<Option<TypeCheckResult>> = vec![];
    let metadata = collect_metadata_from_results(None, &results);
    assert!(metadata.is_empty());
}
