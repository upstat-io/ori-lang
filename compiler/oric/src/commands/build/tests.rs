//! Tests for the multi-file build pipeline.
//!
//! Verifies cross-module metadata collection for repr optimization (CROSS-04-015).

use std::path::PathBuf;

use ori_ir::ReprAttrKind;
use ori_llvm::aot::incremental::deps::DependencyGraph;
use ori_llvm::aot::incremental::ContentHash;
use ori_types::ExportedTypeMetadata;

use super::multi::{collect_imported_type_metadata, merge_forwarded_metadata, CompiledModuleInfo};

/// Helper to create a `CompiledModuleInfo` with specified metadata.
fn module_info(path: &str, metadata: Vec<ExportedTypeMetadata>) -> CompiledModuleInfo {
    CompiledModuleInfo {
        path: PathBuf::from(path),
        module_name: path.to_string(),
        public_functions: Vec::new(),
        exported_type_metadata: metadata,
    }
}

/// Helper to create a dependency graph where `importer` imports `dependencies`.
fn test_graph(importer: &str, dependencies: &[&str]) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    let imports: Vec<PathBuf> = dependencies.iter().map(PathBuf::from).collect();
    graph.add_file(PathBuf::from(importer), ContentHash::new(0), imports);
    graph
}

/// Semantic pin (CROSS-04-015): `collect_imported_type_metadata` collects
/// metadata from compiled dependency modules. Without this fix, the AOT
/// pipeline always passed `&[]` for imported metadata.
#[test]
fn collect_metadata_from_single_dependency() {
    let dep_metadata = vec![ExportedTypeMetadata {
        merkle_hash: 12345,
        repr: None,
        is_public: true,
    }];
    let compiled = vec![module_info("/src/lib.ori", dep_metadata)];
    let graph = test_graph("/src/main.ori", &["/src/lib.ori"]);

    let result = collect_imported_type_metadata(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].merkle_hash, 12345);
    assert!(result[0].is_public);
    assert!(result[0].repr.is_none());
}

/// Collect metadata from multiple dependencies — metadata is aggregated.
#[test]
fn collect_metadata_from_multiple_dependencies() {
    let compiled = vec![
        module_info(
            "/src/math.ori",
            vec![ExportedTypeMetadata {
                merkle_hash: 111,
                repr: None,
                is_public: true,
            }],
        ),
        module_info(
            "/src/types.ori",
            vec![
                ExportedTypeMetadata {
                    merkle_hash: 222,
                    repr: Some(ReprAttrKind::C),
                    is_public: true,
                },
                ExportedTypeMetadata {
                    merkle_hash: 333,
                    repr: None,
                    is_public: false,
                },
            ],
        ),
    ];
    let graph = test_graph("/src/main.ori", &["/src/math.ori", "/src/types.ori"]);

    let result = collect_imported_type_metadata(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert_eq!(result.len(), 3);

    // Verify all metadata is present (order depends on graph iteration)
    let hashes: Vec<u64> = result.iter().map(|m| m.merkle_hash).collect();
    assert!(hashes.contains(&111));
    assert!(hashes.contains(&222));
    assert!(hashes.contains(&333));

    // Verify repr("c") metadata is preserved
    assert!(
        result
            .iter()
            .any(|m| m.merkle_hash == 222 && m.repr == Some(ReprAttrKind::C)),
        "expected repr(c) metadata with hash 222"
    );
}

/// No imports → empty metadata.
#[test]
fn collect_metadata_no_imports() {
    let compiled = vec![module_info(
        "/src/lib.ori",
        vec![ExportedTypeMetadata {
            merkle_hash: 999,
            repr: None,
            is_public: true,
        }],
    )];
    let graph = DependencyGraph::new();

    let result = collect_imported_type_metadata(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert!(result.is_empty());
}

/// Dependency exists in graph but not yet compiled → skip gracefully.
#[test]
fn collect_metadata_missing_compiled_module() {
    let compiled: Vec<CompiledModuleInfo> = Vec::new();
    let graph = test_graph("/src/main.ori", &["/src/missing.ori"]);

    let result = collect_imported_type_metadata(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert!(result.is_empty());
}

/// Dependency with no exported types → empty contribution.
#[test]
fn collect_metadata_dependency_with_no_types() {
    let compiled = vec![module_info("/src/utils.ori", Vec::new())];
    let graph = test_graph("/src/main.ori", &["/src/utils.ori"]);

    let result = collect_imported_type_metadata(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert!(result.is_empty());
}

// --- Transitive metadata forwarding tests (TPR-04-016) ---
//
// These test `merge_forwarded_metadata()`, which merges imported metadata into
// a module's own exports so that re-exported `pub` / `#repr(...)` types
// propagate transitively through module chains (A → B → C).

/// Semantic pin (TPR-04-016): merge adds imported metadata to local exports.
///
/// B has one local type. It imports C which has `#repr("c")` type.
/// After merging, B's exports must include both its own type AND C's.
/// Without this merge, A (which imports only B) would never see C's metadata.
#[test]
fn merge_forwards_imported_repr_c_metadata() {
    let b_local = vec![ExportedTypeMetadata {
        merkle_hash: 0xB001,
        repr: None,
        is_public: true,
    }];
    let c_imported = vec![ExportedTypeMetadata {
        merkle_hash: 0xC001,
        repr: Some(ReprAttrKind::C),
        is_public: true,
    }];

    let result = merge_forwarded_metadata(b_local, &c_imported);

    assert_eq!(result.len(), 2, "expected B's own + C's forwarded");
    assert!(
        result.iter().any(|m| m.merkle_hash == 0xB001),
        "B's own metadata must be preserved"
    );
    assert!(
        result
            .iter()
            .any(|m| m.merkle_hash == 0xC001 && m.repr == Some(ReprAttrKind::C)),
        "C's #repr(c) metadata must be forwarded into B's exports"
    );
}

/// Semantic pin (TPR-04-016): merge forwards `pub` visibility metadata.
#[test]
fn merge_forwards_imported_pub_metadata() {
    let local = Vec::new(); // module has no local types
    let imported = vec![ExportedTypeMetadata {
        merkle_hash: 0xC002,
        repr: None,
        is_public: true,
    }];

    let result = merge_forwarded_metadata(local, &imported);

    assert_eq!(result.len(), 1);
    assert!(result[0].is_public);
    assert_eq!(result[0].merkle_hash, 0xC002);
}

/// Deduplication: if local and imported have the same `merkle_hash`, keep one.
///
/// This happens when a module both declares a type and imports it (shouldn't
/// normally occur, but the function must be robust).
#[test]
fn merge_deduplicates_by_merkle_hash() {
    let local = vec![ExportedTypeMetadata {
        merkle_hash: 0xABC,
        repr: Some(ReprAttrKind::C),
        is_public: true,
    }];
    let imported = vec![ExportedTypeMetadata {
        merkle_hash: 0xABC,
        repr: Some(ReprAttrKind::C),
        is_public: true,
    }];

    let result = merge_forwarded_metadata(local, &imported);

    assert_eq!(
        result.len(),
        1,
        "duplicate merkle_hash should be deduped to one entry"
    );
    assert_eq!(result[0].merkle_hash, 0xABC);
}

/// Diamond deps: imported metadata from two sources with overlapping hashes.
///
/// When B imports both C and D, and both export the same type from E,
/// the imported slice may contain duplicates. Merge should dedup.
#[test]
fn merge_deduplicates_diamond_imports() {
    let local = vec![ExportedTypeMetadata {
        merkle_hash: 0xB001,
        repr: None,
        is_public: true,
    }];
    // Imported metadata has two copies of E's type (from C and D paths)
    let imported = vec![
        ExportedTypeMetadata {
            merkle_hash: 0xE001,
            repr: Some(ReprAttrKind::C),
            is_public: true,
        },
        ExportedTypeMetadata {
            merkle_hash: 0xE001,
            repr: Some(ReprAttrKind::C),
            is_public: true,
        },
    ];

    let result = merge_forwarded_metadata(local, &imported);

    assert_eq!(result.len(), 2, "B's own + one copy of E's metadata");
    let e_count = result.iter().filter(|m| m.merkle_hash == 0xE001).count();
    assert_eq!(e_count, 1, "E's metadata should appear exactly once");
}

/// Empty imports: merge with no imported metadata returns local unchanged.
#[test]
fn merge_with_no_imports_returns_local() {
    let local = vec![ExportedTypeMetadata {
        merkle_hash: 0xA001,
        repr: None,
        is_public: true,
    }];

    let result = merge_forwarded_metadata(local, &[]);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].merkle_hash, 0xA001);
}

/// Empty local: merge with no local metadata returns forwarded imports.
#[test]
fn merge_with_no_local_returns_imported() {
    let imported = vec![ExportedTypeMetadata {
        merkle_hash: 0xC001,
        repr: Some(ReprAttrKind::Packed),
        is_public: true,
    }];

    let result = merge_forwarded_metadata(Vec::new(), &imported);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].repr, Some(ReprAttrKind::Packed));
}
