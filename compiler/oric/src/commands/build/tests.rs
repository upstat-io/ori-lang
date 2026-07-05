//! Tests for the multi-file build pipeline.
//!
//! Verifies cross-module metadata collection for repr optimization.

use std::path::PathBuf;

use ori_ir::ReprAttrKind;
use ori_llvm::aot::incremental::deps::DependencyGraph;
use ori_llvm::aot::incremental::ContentHash;
use ori_llvm::aot::WasmOptLevel;
use ori_types::ExportedTypeMetadata;

use super::multi::{
    collect_imported_collection_surfaces, collect_imported_type_metadata, CompiledModuleInfo,
};
use super::{wasm_opt_runner, BuildOptions, OptLevel};

/// Helper to create a `CompiledModuleInfo` with specified metadata.
fn module_info(path: &str, metadata: Vec<ExportedTypeMetadata>) -> CompiledModuleInfo {
    CompiledModuleInfo {
        path: PathBuf::from(path),
        module_name: path.to_string(),
        public_functions: Vec::new(),
        exported_type_metadata: metadata,
        exported_collection_surfaces: Vec::new(),
        type_result: ori_types::TypeCheckResult::ok(ori_types::TypedModule::default()),
        canon_result: ori_ir::canon::SharedCanonResult::new(ori_ir::canon::CanonResult::empty()),
        pool: std::sync::Arc::new(ori_types::Pool::new()),
    }
}

/// Helper to create a `CompiledModuleInfo` with both type metadata and collection surfaces.
fn module_info_with_surfaces(
    path: &str,
    metadata: Vec<ExportedTypeMetadata>,
    surfaces: Vec<u64>,
) -> CompiledModuleInfo {
    CompiledModuleInfo {
        path: PathBuf::from(path),
        module_name: path.to_string(),
        public_functions: Vec::new(),
        exported_type_metadata: metadata,
        exported_collection_surfaces: surfaces,
        type_result: ori_types::TypeCheckResult::ok(ori_types::TypedModule::default()),
        canon_result: ori_ir::canon::SharedCanonResult::new(ori_ir::canon::CanonResult::empty()),
        pool: std::sync::Arc::new(ori_types::Pool::new()),
    }
}

/// Helper to create a dependency graph where `importer` imports `dependencies`.
fn test_graph(importer: &str, dependencies: &[&str]) -> DependencyGraph {
    let mut graph = DependencyGraph::new();
    let imports: Vec<PathBuf> = dependencies.iter().map(PathBuf::from).collect();
    graph.add_file(PathBuf::from(importer), ContentHash::new(0), imports);
    graph
}

/// Semantic pin: `collect_imported_type_metadata` collects
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

// Collection surface collection

/// Semantic pin: `collect_imported_collection_surfaces` gathers hashes from
/// compiled dependency modules.
#[test]
fn collect_collection_surfaces_from_single_dependency() {
    let compiled = vec![module_info_with_surfaces(
        "/src/lib.ori",
        Vec::new(),
        vec![0xABCD_1234, 0xDEAD_BEEF],
    )];
    let graph = test_graph("/src/main.ori", &["/src/lib.ori"]);

    let result =
        collect_imported_collection_surfaces(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert_eq!(result.len(), 2);
    assert!(result.contains(&0xABCD_1234));
    assert!(result.contains(&0xDEAD_BEEF));
}

/// Multiple dependencies — collection surfaces are aggregated.
#[test]
fn collect_collection_surfaces_from_multiple_dependencies() {
    let compiled = vec![
        module_info_with_surfaces("/src/a.ori", Vec::new(), vec![0x1111]),
        module_info_with_surfaces("/src/b.ori", Vec::new(), vec![0x2222, 0x3333]),
    ];
    let graph = test_graph("/src/main.ori", &["/src/a.ori", "/src/b.ori"]);

    let result =
        collect_imported_collection_surfaces(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert_eq!(result.len(), 3);
    assert!(result.contains(&0x1111));
    assert!(result.contains(&0x2222));
    assert!(result.contains(&0x3333));
}

/// No imports → empty collection surfaces.
#[test]
fn collect_collection_surfaces_no_imports() {
    let compiled = vec![module_info_with_surfaces(
        "/src/lib.ori",
        Vec::new(),
        vec![0xFFFF],
    )];
    let graph = DependencyGraph::new();

    let result =
        collect_imported_collection_surfaces(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert!(result.is_empty());
}

/// Dependency with empty collection surfaces → no contribution.
#[test]
fn collect_collection_surfaces_dependency_with_none() {
    let compiled = vec![module_info_with_surfaces(
        "/src/lib.ori",
        Vec::new(),
        Vec::new(),
    )];
    let graph = test_graph("/src/main.ori", &["/src/lib.ori"]);

    let result =
        collect_imported_collection_surfaces(&PathBuf::from("/src/main.ori"), &graph, &compiled);

    assert!(result.is_empty());
}

// wasm-opt post-processor wiring

/// Semantic pin: `--wasm-opt` on a wasm target constructs the post-processor
/// with the level mapped from `--opt`.
#[test]
fn wasm_opt_flag_on_wasm_target_builds_runner_with_mapped_level() {
    let cases = [
        (OptLevel::O0, WasmOptLevel::O0),
        (OptLevel::O1, WasmOptLevel::O1),
        (OptLevel::O2, WasmOptLevel::O2),
        (OptLevel::O3, WasmOptLevel::O3),
        (OptLevel::Os, WasmOptLevel::Os),
        (OptLevel::Oz, WasmOptLevel::Oz),
    ];

    let mut covered = 0;
    for (opt_level, expected) in cases {
        let options = BuildOptions {
            wasm_opt: true,
            opt_level,
            ..BuildOptions::default()
        };

        let Some(runner) = wasm_opt_runner(&options, true) else {
            panic!("--wasm-opt on a wasm target must build a runner ({opt_level:?})")
        };
        assert_eq!(runner.level, expected, "level mismatch for {opt_level:?}");
        covered += 1;
    }
    assert_eq!(covered, cases.len());
}

/// Negative pin: without `--wasm-opt`, no post-processor runs even on wasm.
#[test]
fn wasm_opt_flag_absent_skips_post_processor() {
    let options = BuildOptions::default();

    assert!(wasm_opt_runner(&options, true).is_none());
}

/// Negative pin: `--wasm-opt` on a non-wasm target never runs the
/// post-processor (wasm-opt only understands WebAssembly binaries).
#[test]
fn wasm_opt_flag_on_native_target_skips_post_processor() {
    let options = BuildOptions {
        wasm_opt: true,
        ..BuildOptions::default()
    };

    assert!(wasm_opt_runner(&options, false).is_none());
}
