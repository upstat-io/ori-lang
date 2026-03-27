//! Multi-file build pipeline (with imports).
//!
//! Builds all dependent modules in topological order, compiles each to an
//! object file (or bitcode for LTO), and links them into a single executable.

use std::path::{Path, PathBuf};

use super::multi_emission::{emit_module_artifact, lto_merge};
use super::{
    build_optimization_config, configure_target, determine_output_path, link_and_finish,
    BuildOptions, LtoMode,
};

/// Build a multi-file Ori program (with imports).
///
/// This builds all dependent modules in topological order and links them together.
#[expect(
    clippy::too_many_lines,
    reason = "multi-module build pipeline — splitting would fragment the build flow"
)]
pub(super) fn build_file_multi(path: &str, options: &BuildOptions, start: std::time::Instant) {
    use ori_llvm::aot::{build_dependency_graph, Mangler};
    use oric::CompilerDb;
    use tempfile::TempDir;

    use crate::problem::codegen::{report_codegen_error, CodegenProblem};

    // Step 1: Build dependency graph
    if options.verbose {
        eprintln!("  Building dependency graph...");
    }

    let entry_path = Path::new(path);
    let entry_canonical = entry_path
        .canonicalize()
        .unwrap_or_else(|_| entry_path.to_path_buf());

    // Import resolver that converts relative paths to absolute paths
    let resolve_import = |current: &Path, import: &str| -> Result<PathBuf, String> {
        let dir = current.parent().unwrap_or(Path::new("."));
        let resolved = dir.join(import);
        let with_ext = if resolved.extension().is_none() {
            resolved.with_extension("ori")
        } else {
            resolved
        };

        if with_ext.exists() {
            Ok(with_ext)
        } else {
            Err(format!(
                "cannot find '{}' at '{}'",
                import,
                with_ext.display()
            ))
        }
    };

    let dep_result = match build_dependency_graph(&entry_canonical, resolve_import) {
        Ok(result) => result,
        Err(e) => {
            report_codegen_error(CodegenProblem::ModuleConfigFailed {
                message: e.to_string(),
            });
        }
    };

    if options.verbose {
        eprintln!(
            "  Found {} files to compile",
            dep_result.compilation_order.len()
        );
        for (i, p) in dep_result.compilation_order.iter().enumerate() {
            eprintln!("    {}: {}", i + 1, p.display());
        }
    }

    // Step 2: Configure target
    let target = configure_target(options).unwrap_or_else(|e| report_codegen_error(e));

    if options.verbose {
        eprintln!("  Target: {}", target.triple());
        eprintln!("  Optimization: {:?}", options.opt_level);
    }

    // Step 3: Compile each module in topological order
    // Use tempfile for unique directory to avoid race conditions in parallel builds
    let temp_dir = match TempDir::new() {
        Ok(dir) => dir,
        Err(e) => {
            report_codegen_error(CodegenProblem::EmissionFailed {
                format: "object".into(),
                path: String::new(),
                message: format!("failed to create temp directory: {e}"),
            });
        }
    };
    let obj_dir = temp_dir.path().to_path_buf();

    let db = CompilerDb::new();
    let mangler = Mangler::new();
    let opt_config = build_optimization_config(options);

    // Set up ARC IR cache for incremental compilation
    let arc_cache = {
        let cache_dir = obj_dir.join("arc_cache");
        match ori_llvm::aot::incremental::ArcIrCache::new(&cache_dir) {
            Ok(cache) => {
                if options.verbose {
                    eprintln!("  ARC IR cache enabled at {}", cache_dir.display());
                }
                Some(cache)
            }
            Err(e) => {
                if options.verbose {
                    eprintln!("  ARC IR cache disabled: {e}");
                }
                None
            }
        }
    };

    // Create compilation context (avoids passing many parameters to helper)
    let compile_ctx = ModuleCompileContext {
        db: &db,
        target: &target,
        opt_config: &opt_config,
        mangler: &mangler,
        graph: &dep_result.graph,
        base_dir: &dep_result.base_dir,
        obj_dir: &obj_dir,
        verbose: options.verbose,
        narrowing_policy: options.narrowing_policy,
        arc_cache,
        module_hash: None, // Per-module hashes computed below if needed
    };

    // Pre-allocate vectors with known capacity to avoid reallocation
    let module_count = dep_result.compilation_order.len();
    let mut compiled_modules: Vec<CompiledModuleInfo> = Vec::with_capacity(module_count);
    let mut object_files: Vec<PathBuf> = Vec::with_capacity(module_count);

    // Compile each module in topological order
    for source_path in &dep_result.compilation_order {
        match compile_single_module(&compile_ctx, source_path, &compiled_modules) {
            Some((obj_path, module_info)) => {
                compiled_modules.push(module_info);
                object_files.push(obj_path);
            }
            None => std::process::exit(1),
        }
    }

    // Drop compile_ctx to release borrows on opt_config, target, etc.
    // before they're reused in the LTO merge step below.
    drop(compile_ctx);

    // Step 4: LTO merge (if enabled) or direct linking
    let is_lto = !matches!(options.lto, LtoMode::Off);
    let final_object_files = if is_lto && object_files.len() > 1 {
        lto_merge(&object_files, &obj_dir, &target, &opt_config, options)
    } else {
        object_files
    };

    // Note: temp_dir must stay alive until linking completes (auto-cleaned on drop)
    let output_path = determine_output_path(path, options);
    link_and_finish(final_object_files, &output_path, &target, options, start);

    // temp_dir automatically cleans up when it goes out of scope
    drop(temp_dir);
}

/// Context for compiling a single module in multi-file compilation.
pub(super) struct ModuleCompileContext<'a> {
    pub(super) db: &'a oric::CompilerDb,
    pub(super) target: &'a ori_llvm::aot::TargetConfig,
    pub(super) opt_config: &'a ori_llvm::aot::OptimizationConfig,
    pub(super) mangler: &'a ori_llvm::aot::Mangler,
    pub(super) graph: &'a ori_llvm::aot::incremental::deps::DependencyGraph,
    pub(super) base_dir: &'a Path,
    pub(super) obj_dir: &'a Path,
    pub(super) verbose: bool,
    /// Representation optimization policy.
    pub(super) narrowing_policy: ori_repr::NarrowingPolicy,
    /// Optional ARC IR cache for incremental compilation.
    pub(super) arc_cache: Option<ori_llvm::aot::incremental::ArcIrCache>,
    /// Per-module content hashes for ARC cache keying.
    pub(super) module_hash:
        Option<rustc_hash::FxHashMap<PathBuf, ori_llvm::aot::incremental::ContentHash>>,
}

/// Information about a compiled module, including its function signatures.
pub(super) struct CompiledModuleInfo {
    /// Path to the source file.
    pub(super) path: PathBuf,
    /// Module name for mangling.
    #[allow(dead_code, reason = "kept for debugging and potential future use")]
    pub(super) module_name: String,
    /// Public function signatures (`mangled_name`, `param_types`, `return_type`).
    /// These are the actual types from type checking, not defaults.
    /// The mangled name is pre-computed to avoid needing the interner later.
    pub(super) public_functions: Vec<(String, Vec<ori_types::Idx>, ori_types::Idx)>,
    /// Exported type metadata (repr attrs + visibility) for cross-module repr
    /// plan construction. Imported modules' metadata is fed into `ReprPlan` so
    /// that `pub` and `#repr(...)` types are not incorrectly narrowed.
    /// See CROSS-04-014 and CROSS-04-015.
    pub(super) exported_type_metadata: Vec<ori_types::ExportedTypeMetadata>,
}

/// Compile a single module to an object file.
///
/// Returns (`object_path`, `CompiledModuleInfo`) on success.
fn compile_single_module(
    ctx: &ModuleCompileContext<'_>,
    source_path: &Path,
    compiled_modules: &[CompiledModuleInfo],
) -> Option<(PathBuf, CompiledModuleInfo)> {
    use ori_llvm::aot::derive_module_name;
    use ori_llvm::inkwell::context::Context;
    use oric::SourceFile;

    use crate::commands::compile_common::{check_source, compile_to_llvm_with_imports};
    use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics, CodegenProblem};

    let source_path_str = source_path.to_string_lossy();

    if ctx.verbose {
        eprintln!("  Compiling {}...", source_path.display());
    }

    // Read source content
    let content = match std::fs::read_to_string(source_path) {
        Ok(c) => c,
        Err(e) => {
            let diag = CodegenProblem::EmissionFailed {
                format: "source".into(),
                path: source_path.display().to_string(),
                message: format!("failed to read: {e}"),
            };
            let mut acc = CodegenDiagnostics::new();
            acc.push(diag);
            emit_codegen_diagnostics(acc);
            return None;
        }
    };

    // Derive module name
    let module_name = derive_module_name(source_path, Some(ctx.base_dir));

    // Load and check the source
    let file = SourceFile::new(ctx.db, source_path.to_path_buf(), content);
    let (parse_result, type_result, pool, canon_result) =
        check_source(ctx.db, file, &source_path_str)?;

    // Extract public function signatures with actual types from type checking
    let public_functions = extract_public_function_types(
        &parse_result,
        &type_result,
        &module_name,
        ctx.mangler,
        ctx.db,
    );

    // Build list of imported functions for this module
    let imported_functions = build_import_infos(
        source_path,
        ctx.graph,
        compiled_modules,
        ctx.base_dir,
        ctx.mangler,
    );

    // Collect exported type metadata from imported modules for repr plan
    // construction. This ensures imported `pub` and `#repr(...)` types are
    // correctly exempted from integer narrowing. See CROSS-04-015.
    let imported_type_metadata =
        collect_imported_type_metadata(source_path, ctx.graph, compiled_modules);

    // Compile to LLVM IR (with ARC cache if available).
    // Salsa/ArtifactCache boundary: typed() results flow into codegen via
    // function content hashes; ArcIrCache provides Layer 1 caching.
    let context = Context::create();
    let llvm_module = match compile_to_llvm_with_imports(
        &context,
        ctx.db,
        &parse_result,
        &type_result,
        &pool,
        &canon_result,
        &source_path_str,
        &module_name,
        &imported_functions,
        &imported_type_metadata,
        ctx.arc_cache.as_ref(),
        ctx.module_hash
            .as_ref()
            .and_then(|hashes| hashes.get(source_path).copied()),
        Some(ctx.target.triple()),
        ctx.narrowing_policy,
    ) {
        Ok(m) => m,
        Err(e) => {
            let mut acc = CodegenDiagnostics::new();
            acc.push(CodegenProblem::VerificationFailed { message: e });
            emit_codegen_diagnostics(acc);
            return None;
        }
    };

    // Configure target, optimize, and emit object/bitcode
    let obj_path = emit_module_artifact(ctx, &llvm_module, &module_name)?;

    let module_info = CompiledModuleInfo {
        path: source_path.to_path_buf(),
        module_name,
        public_functions,
        exported_type_metadata: type_result.typed.exported_type_metadata.clone(),
    };

    Some((obj_path, module_info))
}

/// Extract public function signatures with actual types from a type-checked module.
///
/// Returns tuples of (`mangled_name`, `param_types`, `return_type`).
/// The mangled name is pre-computed to avoid needing the interner later.
fn extract_public_function_types(
    parse_result: &crate::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    module_name: &str,
    mangler: &ori_llvm::aot::Mangler,
    db: &oric::CompilerDb,
) -> Vec<(String, Vec<ori_types::Idx>, ori_types::Idx)> {
    use oric::Db; // For interner() method

    let interner = db.interner();
    let mut public_functions = Vec::new();

    // Build a name-based lookup map because typed.functions is sorted by name
    // (for Salsa determinism) while module.functions is in source order.
    let sig_map: rustc_hash::FxHashMap<oric::ir::Name, &ori_types::FunctionSig> = type_result
        .typed
        .functions
        .iter()
        .map(|ft| (ft.name, ft))
        .collect();

    // Match parsed functions with their type-checked signatures by name
    for func in &parse_result.module.functions {
        if !func.visibility.is_public() {
            continue;
        }

        if let Some(func_sig) = sig_map.get(&func.name) {
            let func_name_str = interner.lookup(func.name);
            let mangled_name = mangler.mangle_function(module_name, func_name_str);

            public_functions.push((
                mangled_name,
                func_sig.param_types.clone(),
                func_sig.return_type,
            ));
        }
    }

    public_functions
}

/// Build import information for a module based on its dependencies.
///
/// Uses actual type information from already-compiled modules rather than
/// defaulting to INT. This ensures correct calling conventions for cross-module calls.
fn build_import_infos(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
    _base_dir: &Path,
    _mangler: &ori_llvm::aot::Mangler,
) -> Vec<crate::commands::compile_common::ImportedFunctionInfo> {
    let mut imported_functions = Vec::new();

    // Get the direct imports of this module
    let Some(imports) = graph.get_imports(source_path) else {
        return imported_functions;
    };

    // Build index once for O(1) lookups instead of O(n) linear scan per import
    let module_index: rustc_hash::FxHashMap<&Path, &CompiledModuleInfo> = compiled_modules
        .iter()
        .map(|m| (m.path.as_path(), m))
        .collect();

    for import_path in imports {
        // O(1) lookup using the index
        let Some(module_info) = module_index.get(import_path.as_path()) else {
            // Module not yet compiled - shouldn't happen in topological order
            eprintln!(
                "warning: import '{}' not found in compiled modules",
                import_path.display()
            );
            continue;
        };
        let module_info = *module_info;

        // Add each public function using the actual types from type checking
        // The mangled names are pre-computed when the module was compiled
        // Pre-allocate to avoid reallocations in the inner loop
        imported_functions.reserve(module_info.public_functions.len());
        for (mangled_name, param_types, return_type) in &module_info.public_functions {
            imported_functions.push(crate::commands::compile_common::ImportedFunctionInfo {
                mangled_name: mangled_name.clone(),
                param_types: param_types.clone(),
                return_type: *return_type,
            });
        }
    }

    imported_functions
}

/// Collect exported type metadata from all modules this module imports.
///
/// Mirrors `build_import_infos()` but collects `ExportedTypeMetadata` instead
/// of function signatures. This metadata enables `ReprPlan` to correctly exempt
/// imported `pub` and `#repr(...)` types from integer narrowing.
/// See CROSS-04-014 and CROSS-04-015.
pub(super) fn collect_imported_type_metadata(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
) -> Vec<ori_types::ExportedTypeMetadata> {
    let Some(imports) = graph.get_imports(source_path) else {
        return Vec::new();
    };

    let module_index: rustc_hash::FxHashMap<&Path, &CompiledModuleInfo> = compiled_modules
        .iter()
        .map(|m| (m.path.as_path(), m))
        .collect();

    let mut metadata = Vec::new();
    for import_path in imports {
        if let Some(module_info) = module_index.get(import_path.as_path()) {
            metadata.extend(module_info.exported_type_metadata.iter().cloned());
        }
    }
    metadata
}
