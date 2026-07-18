use super::*;

use ori_types::{TypeCheckResult, TypeErrorKind, TypedModule};

use crate::db::{CompilerDb, Db};

// Cyclic-import regression: register_resolved_imports recursion into typed()

/// Regression: a genuine two-file mutual import cycle must produce a
/// `CircularImport` diagnostic, never an unhandled Salsa panic. Before the
/// upstream cycle guard, `typed()` recursed into itself via
/// `register_resolved_imports`'s `typed(db, sf)` calls with no cycle
/// detection, and Salsa's default `CycleRecoveryStrategy::Panic` aborted the
/// query with an opaque `Box<dyn Any>` panic reachable from `ori check`.
#[test]
fn cyclic_import_two_file_produces_diagnostic_not_panic() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let a_path = dir.path().join("a.ori");
    let b_path = dir.path().join("b.ori");
    std::fs::write(
        &a_path,
        "use \"./b\" { by };\npub @ax (x: int) -> int = by(x: x) + 1;\n",
    )
    .unwrap_or_else(|e| panic!("write a.ori: {e}"));
    std::fs::write(
        &b_path,
        "use \"./a\" { ax };\npub @by (x: int) -> int = ax(x: x) + 1;\n",
    )
    .unwrap_or_else(|e| panic!("write b.ori: {e}"));

    let db = CompilerDb::new();
    let file_a = db
        .load_file(&a_path)
        .unwrap_or_else(|| panic!("failed to load a.ori"));

    let result = crate::query::typed(&db, file_a);

    assert!(
        result.has_errors(),
        "cyclic import must produce a type-check error, not succeed silently"
    );
    let has_circular_import_error = result.errors().iter().any(|e| {
        matches!(
            e.kind,
            TypeErrorKind::ImportError {
                kind: ori_ir::ImportErrorKind::CircularImport,
                ..
            }
        )
    });
    assert!(
        has_circular_import_error,
        "expected a CircularImport diagnostic among: {:?}",
        result
            .errors()
            .iter()
            .map(ori_types::TypeCheckError::message)
            .collect::<Vec<_>>()
    );
}

/// Regression: a self-import (`a.ori` imports itself) is a 1-cycle and must
/// also produce a `CircularImport` diagnostic, not a panic.
#[test]
fn cyclic_import_self_import_produces_diagnostic_not_panic() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let a_path = dir.path().join("a.ori");
    std::fs::write(
        &a_path,
        "use \"./a\" { ax };\npub @ax (x: int) -> int = x + 1;\n",
    )
    .unwrap_or_else(|e| panic!("write a.ori: {e}"));

    let db = CompilerDb::new();
    let file_a = db
        .load_file(&a_path)
        .unwrap_or_else(|| panic!("failed to load a.ori"));

    let result = crate::query::typed(&db, file_a);

    assert!(
        result.has_errors(),
        "self-import must produce a type-check error, not succeed silently"
    );
    let has_circular_import_error = result.errors().iter().any(|e| {
        matches!(
            e.kind,
            TypeErrorKind::ImportError {
                kind: ori_ir::ImportErrorKind::CircularImport,
                ..
            }
        )
    });
    assert!(
        has_circular_import_error,
        "expected a CircularImport diagnostic among: {:?}",
        result
            .errors()
            .iter()
            .map(ori_types::TypeCheckError::message)
            .collect::<Vec<_>>()
    );
}

/// Regression: a diamond import graph (A imports B and C; B and C both
/// import D) is NOT a cycle and must type-check cleanly — a naive one-set
/// cycle guard (vs the two-set `loading_set/visited` discipline) would
/// false-positive on D being reached twice.
#[test]
fn diamond_import_graph_is_not_a_false_positive_cycle() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let a_path = dir.path().join("a.ori");
    let b_path = dir.path().join("b.ori");
    let c_path = dir.path().join("c.ori");
    let d_path = dir.path().join("d.ori");
    std::fs::write(
        &a_path,
        "use \"./b\" { bx };\nuse \"./c\" { cx };\npub @ax () -> int = bx() + cx();\n",
    )
    .unwrap_or_else(|e| panic!("write a.ori: {e}"));
    std::fs::write(
        &b_path,
        "use \"./d\" { dx };\npub @bx () -> int = dx() + 1;\n",
    )
    .unwrap_or_else(|e| panic!("write b.ori: {e}"));
    std::fs::write(
        &c_path,
        "use \"./d\" { dx };\npub @cx () -> int = dx() + 2;\n",
    )
    .unwrap_or_else(|e| panic!("write c.ori: {e}"));
    std::fs::write(&d_path, "pub @dx () -> int = 1;\n")
        .unwrap_or_else(|e| panic!("write d.ori: {e}"));

    let db = CompilerDb::new();
    let file_a = db
        .load_file(&a_path)
        .unwrap_or_else(|| panic!("failed to load a.ori"));

    let result = crate::query::typed(&db, file_a);

    assert!(
        !result.has_errors(),
        "diamond import graph is not a cycle and must type-check cleanly, got errors: {:?}",
        result
            .errors()
            .iter()
            .map(ori_types::TypeCheckError::message)
            .collect::<Vec<_>>()
    );
}

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

// collect_surfaces_from_results tests

/// Regression: exercises the exact collection path from
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

// collect_metadata_from_results tests

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

#[test]
fn selected_public_constant_is_typed_in_the_consumer_pool() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let provider_path = dir.path().join("provider.ori");
    let consumer_path = dir.path().join("consumer.ori");
    std::fs::write(&provider_path, "pub $answer = 30;\n")
        .unwrap_or_else(|e| panic!("write provider: {e}"));
    std::fs::write(
        &consumer_path,
        "use \"./provider\" { $answer };\npub @answer_value () -> int = $answer;\n",
    )
    .unwrap_or_else(|e| panic!("write consumer: {e}"));

    let db = CompilerDb::new();
    let consumer = db
        .load_file(&consumer_path)
        .unwrap_or_else(|| panic!("load consumer"));
    let result = crate::query::typed(&db, consumer);

    assert!(
        !result.has_errors(),
        "selected public constant must retain its provider-inferred type: {:?}",
        result
            .errors()
            .iter()
            .map(ori_types::TypeCheckError::message)
            .collect::<Vec<_>>()
    );
}

#[test]
fn missing_selected_constant_reports_constant_not_function() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let provider_path = dir.path().join("provider.ori");
    let consumer_path = dir.path().join("consumer.ori");
    std::fs::write(&provider_path, "pub $present = 30;\n")
        .unwrap_or_else(|e| panic!("write provider: {e}"));
    std::fs::write(
        &consumer_path,
        "use \"./provider\" { $missing };\n@main () -> int = $missing;\n",
    )
    .unwrap_or_else(|e| panic!("write consumer: {e}"));

    let db = CompilerDb::new();
    let consumer = db
        .load_file(&consumer_path)
        .unwrap_or_else(|| panic!("load consumer"));
    let result = crate::query::typed(&db, consumer);
    let messages = result
        .errors()
        .iter()
        .map(ori_types::TypeCheckError::message)
        .collect::<Vec<_>>();

    assert!(
        messages
            .iter()
            .any(|message| message.contains("constant '$missing' not found")),
        "missing selected constant must not be misreported as a function: {messages:?}"
    );
    assert!(messages
        .iter()
        .all(|message| !message.contains("function 'missing'")));
}

#[test]
fn private_selected_constant_requires_public_visibility() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let provider_path = dir.path().join("provider.ori");
    let consumer_path = dir.path().join("consumer.ori");
    std::fs::write(&provider_path, "$secret = 30;\n")
        .unwrap_or_else(|e| panic!("write provider: {e}"));
    std::fs::write(
        &consumer_path,
        "use \"./provider\" { $secret };\n@main () -> int = $secret;\n",
    )
    .unwrap_or_else(|e| panic!("write consumer: {e}"));

    let db = CompilerDb::new();
    let consumer = db
        .load_file(&consumer_path)
        .unwrap_or_else(|| panic!("load consumer"));
    let result = crate::query::typed(&db, consumer);
    let private = result.errors().iter().find(|error| {
        matches!(
            error.kind,
            TypeErrorKind::ImportError {
                kind: ori_ir::ImportErrorKind::PrivateAccess,
                ..
            }
        )
    });

    assert!(private.is_some(), "private constant import must fail");
    assert!(
        private
            .map(ori_types::TypeCheckError::message)
            .is_some_and(|message| message.contains("add `pub`")),
        "visibility error must name the fix"
    );
}

#[test]
fn parent_test_module_may_import_private_constant() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let test_dir = dir.path().join("_test");
    std::fs::create_dir(&test_dir).unwrap_or_else(|e| panic!("create _test: {e}"));
    let provider_path = dir.path().join("provider.ori");
    let consumer_path = test_dir.join("provider.test.ori");
    std::fs::write(&provider_path, "$secret = 30;\n")
        .unwrap_or_else(|e| panic!("write provider: {e}"));
    std::fs::write(
        &consumer_path,
        "use \"../provider\" { $secret };\n@test_private tests _ () -> void = { let x: int = $secret; };\n",
    )
    .unwrap_or_else(|e| panic!("write consumer: {e}"));

    let db = CompilerDb::new();
    let consumer = db
        .load_file(&consumer_path)
        .unwrap_or_else(|| panic!("load consumer"));
    let result = crate::query::typed(&db, consumer);

    assert!(
        !result.has_errors(),
        "parent test-module visibility exception must include constants: {:?}",
        result
            .errors()
            .iter()
            .map(ori_types::TypeCheckError::message)
            .collect::<Vec<_>>()
    );
}
