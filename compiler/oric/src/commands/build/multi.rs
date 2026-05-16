//! Multi-file build pipeline (with imports).
//!
//! Builds all dependent modules in topological order, compiles each to an
//! object file (or bitcode for LTO), and links them into a single executable.

use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap;

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
    link_and_finish(
        final_object_files,
        &output_path,
        &target,
        options,
        &opt_config.sanitizer,
        start,
    );

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
    pub(super) exported_type_metadata: Vec<ori_types::ExportedTypeMetadata>,
    /// Merkle hashes of collection types in public function signatures.
    /// Enables cross-module protection of collection element layouts from
    /// narrowing. See
    pub(super) exported_collection_surfaces: Vec<u64>,
    /// Type-check result retained for cross-module imported-generic mono
    /// dispatch via body-import linkage. The host
    /// module's `build_imported_mono_functions` consumes generic sigs +
    /// public-name metadata from each imported module's `TypeCheckResult`.
    #[allow(dead_code, reason = "consumer arrives at merged-pool re-interning")]
    pub(super) type_result: ori_types::TypeCheckResult,
    /// Canonical IR retained alongside `type_result`. `arc_lowering` uses
    /// this to specialize imported generic bodies via
    /// `lower_to_arc(mono_fn.mangled_name, &mono_fn.sig, source_body_name,
    /// source_canon, ...)`.
    #[allow(dead_code, reason = "consumer arrives at merged-pool re-interning")]
    pub(super) canon_result: ori_ir::canon::SharedCanonResult,
    /// Per-module pool retained alongside `type_result`. The host module's
    /// re-interner uses this when remapping imported generic sigs and canon
    /// IR into the host's merged pool.
    #[allow(dead_code, reason = "consumer arrives at merged-pool re-interning")]
    pub(super) pool: std::sync::Arc<ori_types::Pool>,
}

/// Compile a single module's typed + canonicalized IR to an LLVM module.
///
/// Wraps `compile_to_llvm_with_imports` with the source-path-keyed module
/// hash lookup + the `VerificationFailed` diagnostic path so the caller can
/// stay focused on orchestration. Returns `None` on codegen error after
/// emitting diagnostics.
#[allow(
    clippy::too_many_arguments,
    reason = "thin wrapper around compile_to_llvm_with_imports; structural cure would push 11+ args one level deeper"
)]
fn lower_module_to_llvm<'ctx>(
    context: &'ctx ori_llvm::inkwell::context::Context,
    ctx: &ModuleCompileContext<'_>,
    source_path: &Path,
    source_path_str: &str,
    module_name: &str,
    parse_result: &crate::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    merged_pool: &'ctx ori_types::Pool,
    canon_result: &ori_ir::canon::CanonResult,
    imported_functions: &[crate::commands::compile_common::ImportedFunctionInfo],
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
    imported_mono_fns: &[crate::commands::ImportedMonoFn],
    re_interned_canons: &[ori_ir::canon::CanonResult],
) -> Option<ori_llvm::inkwell::module::Module<'ctx>> {
    use crate::commands::compile_common::compile_to_llvm_with_imports;
    use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics, CodegenProblem};

    match compile_to_llvm_with_imports(
        context,
        ctx.db,
        parse_result,
        type_result,
        merged_pool,
        canon_result,
        source_path_str,
        module_name,
        imported_functions,
        imported_type_metadata,
        imported_collection_surfaces,
        imported_mono_fns,
        re_interned_canons,
        ctx.arc_cache.as_ref(),
        ctx.module_hash
            .as_ref()
            .and_then(|hashes| hashes.get(source_path).copied()),
        Some(ctx.target.triple()),
        ctx.narrowing_policy,
    ) {
        Ok(m) => Some(m),
        Err(e) => {
            let mut acc = CodegenDiagnostics::new();
            acc.push(CodegenProblem::VerificationFailed { message: e });
            emit_codegen_diagnostics(acc);
            None
        }
    }
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

    use crate::commands::compile_common::check_source;
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
    // correctly exempted from integer narrowing. See
    let imported_type_metadata =
        collect_imported_type_metadata(source_path, ctx.graph, compiled_modules);
    let imported_collection_surfaces =
        collect_imported_collection_surfaces(source_path, ctx.graph, compiled_modules);

    // Build imported_mono_fns + merged pool for cross-module imported-generic
    // body specialization via body-import linkage. The merged pool is the host
    // pool with imported types re-interned alongside; imported generic sigs +
    // imported canons are remapped to merged-pool coordinates so
    // `arc_lowering::lower_to_arc` can specialize the imported body locally
    // against the SOURCE module's canon with merged-pool Idx values.
    //
    // When the host has no imported generic instantiations (single-file
    // entry, or every imported function is non-generic), the helper short-
    // circuits — `merged_pool` ends up Idx-equivalent to the host pool, and
    // both `re_interned_canons` and `imported_mono_fns` are empty.
    let ImportedMonoState {
        merged_pool,
        re_interned_canons,
        imported_mono_fns,
    } = build_imported_mono_state(ctx.db, source_path, &parse_result, &type_result, &pool);

    // Compile to LLVM IR (with ARC cache if available).
    // Salsa/ArtifactCache boundary: typed() results flow into codegen via
    // function content hashes; ArcIrCache provides Layer 1 caching.
    let context = Context::create();
    let llvm_module = lower_module_to_llvm(
        &context,
        ctx,
        source_path,
        &source_path_str,
        &module_name,
        &parse_result,
        &type_result,
        &merged_pool,
        &canon_result,
        &imported_functions,
        &imported_type_metadata,
        &imported_collection_surfaces,
        &imported_mono_fns,
        &re_interned_canons,
    )?;

    // Configure target, optimize, and emit object/bitcode
    let obj_path = emit_module_artifact(ctx, &llvm_module, &module_name, &source_path_str)?;

    // Transitive metadata forwarding now happens in the type checker
    // (generate_exported_type_metadata merges imported metadata at finish_with_pool).
    // No post-check merge needed here.
    //
    // Retain type_result + canon_result + pool for the host module's
    // imported-generic mono dispatch via body-import linkage. Each
    // importer consumes these via `build_imported_mono_functions` +
    // `arc_lowering`'s `lower_to_arc` of the imported generic body. Pool
    // is `Arc<Pool>` so the clone is cheap.
    let exported_type_metadata = type_result.typed.exported_type_metadata.clone();
    let exported_collection_surfaces = type_result.typed.exported_collection_surfaces.clone();
    let retained_pool = std::sync::Arc::clone(&pool);
    drop(llvm_module);
    drop(pool);
    let module_info = CompiledModuleInfo {
        path: source_path.to_path_buf(),
        module_name,
        public_functions,
        exported_type_metadata,
        exported_collection_surfaces,
        type_result,
        canon_result,
        pool: retained_pool,
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

    // Match parsed functions with their type-checked signatures by name.
    //
    // Skip generic functions: their `param_types` / `return_type` carry
    // `Tag::BoundVar` leaves bound by an enclosing scheme, which are
    // meaningless when copied into another module's pool. Imported generic
    // call sites are handled by the mono-dispatch path
    // (`build_imported_mono_state` + `compile_to_llvm_with_imports`'s
    // `imported_mono_fns` parameter) which monomorphizes each instance
    // with the concrete type. Mirrors the JIT runner pattern at
    // `oric/src/test/runner/llvm_backend.rs:213`.
    for func in &parse_result.module.functions {
        if !func.visibility.is_public() {
            continue;
        }

        if let Some(func_sig) = sig_map.get(&func.name) {
            if func_sig.is_generic() {
                continue;
            }
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

/// Merge imported metadata into a module's own exported type metadata.
///
/// When module B imports module C, C's `ExportedTypeMetadata` (repr attrs,
/// public visibility) must be forwarded into B's exports. This ensures that
/// when module A imports only B, it still receives C's metadata transitively.
/// Collect exported type metadata from all modules this module imports.
///
/// Mirrors `build_import_infos()` but collects `ExportedTypeMetadata` instead
/// of function signatures. This metadata enables `ReprPlan` to correctly exempt
/// imported `pub` and `#repr(...)` types from integer narrowing.
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

/// Collect imported collection surface hashes from dependency modules.
///
/// Parallel to `collect_imported_type_metadata()` but collects merkle hashes
/// of collection types (List, Set) that appear in imported public function
/// signatures. Used for transitive forwarding metadata (A→B→C propagation).
/// After imported surfaces no longer suppress narrowing — they
/// are forwarded for downstream metadata only. See
pub(super) fn collect_imported_collection_surfaces(
    source_path: &Path,
    graph: &ori_llvm::aot::incremental::deps::DependencyGraph,
    compiled_modules: &[CompiledModuleInfo],
) -> Vec<u64> {
    let Some(imports) = graph.get_imports(source_path) else {
        return Vec::new();
    };

    let module_index: rustc_hash::FxHashMap<&Path, &CompiledModuleInfo> = compiled_modules
        .iter()
        .map(|m| (m.path.as_path(), m))
        .collect();

    let mut surfaces = Vec::new();
    for import_path in imports {
        if let Some(module_info) = module_index.get(import_path.as_path()) {
            surfaces.extend(module_info.exported_collection_surfaces.iter().copied());
        }
    }
    surfaces
}

/// State derived from cross-module imported-generic resolution.
///
/// Returned by `build_imported_mono_state` and consumed by
/// `compile_to_llvm_with_imports` for cross-module mono dispatch.
///
/// Carries merged-pool coordinates: every `Idx` in `imported_mono_fns.sig`
/// and `body_type_map`, plus every `TypeId` in entries of `re_interned_canons`,
/// is valid in `merged_pool`. The host module's own `canon_result` (built
/// against the original host pool) is ALSO valid in `merged_pool` because
/// the merged pool is cloned from the host pool — host `Idx` values are
/// preserved by clone.
pub(crate) struct ImportedMonoState {
    /// Merged pool — host pool with imported types re-interned alongside.
    pub(crate) merged_pool: ori_types::Pool,
    /// Per-imported-module re-interned canons, indexed by entry index in
    /// `resolved_imports.modules`. `imported_mono_fns[i].1` indexes this
    /// vector so `arc_lowering` can fetch the SOURCE canon for body
    /// specialization.
    pub(crate) re_interned_canons: Vec<ori_ir::canon::CanonResult>,
    /// `(MonoFunction, source_module_idx, source_body_name)` triples — one
    /// entry per unique imported mono instance reachable from the host's
    /// `MonoInstance` list. Empty when the host has no imported generic
    /// instantiations.
    pub(crate) imported_mono_fns: Vec<crate::commands::ImportedMonoFn>,
}

/// Build the merged-pool re-interning state for cross-module imported
/// generic body specialization via body-import linkage.
///
/// Mirrors the JIT-side dance at
/// `compiler/oric/src/test/runner/llvm_backend.rs:135-292`:
///
/// 1. Clone the host pool to obtain a mutable `merged_pool`. Host `Idx`
///    values remain valid in the clone.
/// 2. Resolve the host's imports via `resolve_imports` to get function-level
///    cross-references (`local_name`, `original_name`, `module_index`).
/// 3. For each imported module, locate the corresponding `CompiledModuleInfo`
///    by path, re-intern its `canon_result` arena into `merged_pool` (using
///    shared per-module cache + `var_remap` maps so subsequent sig re-intern
///    reuses them), and record the resulting `CanonResult`.
/// 4. For each `ImportedFunctionRef`, find its `FunctionSig` in the source
///    module's type-check result, skip non-generics, re-intern the sig into
///    `merged_pool`, and key by `local_name` per JIT comment at
///    `llvm_backend.rs:243-248` (`MonoInstance.fn_name` is the call-site
///    identifier).
/// 5. Call `build_imported_mono_functions` with the host's `MonoInstance`
///    list to produce one `ImportedMonoFn` triple per unique instance.
///
/// Short-circuits to host-pool clone + empty vectors when the host has no
/// imports or no imported function refs survive the generic filter. This
/// keeps single-file builds and host-only-generic builds zero-cost on the
/// cross-module path.
/// Type-check + canonicalize every imported module returned by
/// `resolve_imports`. Salsa memoizes — modules already processed during the
/// host module's compilation hit the cache; stdlib modules pick up cached
/// results from the prelude resolution path or are loaded on demand. Each
/// returned vector is index-aligned with `resolved_imports.modules`; modules
/// whose type-check returns `None` (internal Pool cache miss) get empty
/// placeholders to preserve index alignment with `imported_mono_fns[i]
/// .source_module_idx`.
fn type_check_and_canonicalize_imports(
    db: &oric::CompilerDb,
    resolved_imports: &crate::imports::ResolvedImports,
) -> (
    Vec<ori_types::TypeCheckResult>,
    Vec<std::sync::Arc<ori_types::Pool>>,
    Vec<ori_ir::canon::SharedCanonResult>,
) {
    let count = resolved_imports.modules.len();
    let mut imported_type_results: Vec<ori_types::TypeCheckResult> = Vec::with_capacity(count);
    let mut imported_pools: Vec<std::sync::Arc<ori_types::Pool>> = Vec::with_capacity(count);
    let mut imported_canon_results: Vec<ori_ir::canon::SharedCanonResult> =
        Vec::with_capacity(count);

    for imp_module in &resolved_imports.modules {
        let Some((imp_tc, imp_pool)) = oric::query::type_check_module(
            db,
            &imp_module.parse_output,
            &imp_module.module_path,
            imp_module.source_file,
        ) else {
            imported_type_results.push(ori_types::TypeCheckResult::ok(
                ori_types::TypedModule::default(),
            ));
            imported_pools.push(std::sync::Arc::new(ori_types::Pool::new()));
            imported_canon_results.push(ori_ir::canon::SharedCanonResult::new(
                ori_ir::canon::CanonResult::empty(),
            ));
            continue;
        };
        let imp_canon = oric::query::canonicalize_cached_by_path(
            db,
            &imp_module.module_path,
            &imp_module.parse_output,
            &imp_tc,
            &imp_pool,
        );
        imported_type_results.push(imp_tc);
        imported_pools.push(imp_pool);
        imported_canon_results.push(imp_canon);
    }

    (
        imported_type_results,
        imported_pools,
        imported_canon_results,
    )
}

pub(crate) fn build_imported_mono_state(
    db: &oric::CompilerDb,
    source_path: &Path,
    parse_result: &crate::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    host_pool: &std::sync::Arc<ori_types::Pool>,
) -> ImportedMonoState {
    use oric::Db;

    let interner = db.interner();

    // Clone the host pool — host Idx values are preserved by clone. All
    // imported types will be re-interned alongside via Pool::intern, which
    // never mutates existing entries.
    let mut merged_pool = (**host_pool).clone();

    // Resolve imports for this module (function-level cross-references).
    // resolve_imports populates resolved.modules from EVERY `use` statement,
    // including stdlib (`use std.testing { ... }`) — broader than the
    // multi-file build dependency graph (which filters stdlib out via
    // `extract_imports` in ori_llvm/aot/multi_file). We re-type-check +
    // re-canonicalize EACH imported module here via the shared Salsa-cached
    // query helpers; this works uniformly for relative-path imports AND
    // stdlib imports, mirroring the JIT runner's pattern at
    // `oric/src/test/runner/llvm_backend.rs:103-133`.
    let resolved_imports = crate::imports::resolve_imports(db, parse_result, source_path);

    if resolved_imports.modules.is_empty() {
        return ImportedMonoState {
            merged_pool,
            re_interned_canons: Vec::new(),
            imported_mono_fns: Vec::new(),
        };
    }

    // Type-check + canonicalize each imported module via the Salsa-cached
    // query path. Each entry is index-aligned with `resolved_imports.modules`
    // so `imported_mono_fns[i].source_module_idx` references the correct
    // re-interned canon.
    let (imported_type_results, imported_pools, imported_canon_results) =
        type_check_and_canonicalize_imports(db, &resolved_imports);

    // Per-module re-intern caches + var_remaps, indexed by
    // resolved_imports.modules entry index. SAME cache + var_remap shared
    // across canon arena re-intern + sig re-intern per JIT pattern at
    // `llvm_backend.rs:152-156` so scheme_var_ids + leaf Tag::Var ids stay
    // coherent.
    let mut per_module_caches: Vec<FxHashMap<ori_types::Idx, ori_types::Idx>> =
        vec![FxHashMap::default(); resolved_imports.modules.len()];
    let mut per_module_var_remaps: Vec<FxHashMap<u32, u32>> =
        vec![FxHashMap::default(); resolved_imports.modules.len()];

    // Re-intern each imported module's canon arena into merged_pool. Each
    // import's TypeIds must be remapped independently into the merged pool
    // so scheme_var_ids and leaf Tag::Var ids resolve coherently per import.
    let re_interned_canons: Vec<ori_ir::canon::CanonResult> = imported_canon_results
        .iter()
        .enumerate()
        .map(|(module_idx, shared_canon)| {
            let source_pool = &imported_pools[module_idx];
            let cache = &mut per_module_caches[module_idx];
            let var_remap = &mut per_module_var_remaps[module_idx];

            let mut remapped: ori_ir::canon::CanonResult = (**shared_canon).clone();
            remapped.arena.remap_types(|type_id| {
                let source_idx = ori_types::Idx::from_raw(type_id.raw());
                let target_idx = ori_types::re_intern_type_with_var_remap(
                    source_pool,
                    source_idx,
                    &mut merged_pool,
                    cache,
                    var_remap,
                );
                ori_ir::TypeId::from_raw(target_idx.raw())
            });
            remapped
        })
        .collect();

    // Build imported_generic_sigs: { local_name → (re_interned_sig,
    // module_idx, original_name) }. Keyed by LOCAL name per JIT comment at
    // `llvm_backend.rs:243-248`: MonoInstance.fn_name uses the call-site
    // identifier (which is the local/aliased name from the use statement).
    let mut imported_generic_sigs: FxHashMap<
        ori_ir::Name,
        (ori_types::FunctionSig, usize, ori_ir::Name),
    > = FxHashMap::default();
    for func_ref in &resolved_imports.imported_functions {
        if func_ref.is_module_alias {
            continue;
        }
        let Some(sig) = imported_type_results[func_ref.module_index]
            .typed
            .functions
            .iter()
            .find(|s| s.name == func_ref.original_name)
        else {
            continue;
        };
        if !sig.is_generic() {
            continue;
        }
        let source_pool = &imported_pools[func_ref.module_index];
        let cache = &mut per_module_caches[func_ref.module_index];
        let var_remap = &mut per_module_var_remaps[func_ref.module_index];
        let re_interned = ori_types::re_intern_sig_with_var_remap(
            sig,
            source_pool,
            &mut merged_pool,
            cache,
            var_remap,
        );
        imported_generic_sigs.insert(
            func_ref.local_name,
            (re_interned, func_ref.module_index, func_ref.original_name),
        );
    }

    // Build ImportedMonoFn triples from host MonoInstances filtered against
    // imported_generic_sigs. Delegates to the SSOT entry point — same
    // builder consumed by the JIT test runner via body-import linkage
    // (single monomorphization mechanism for JIT + AOT).
    let imported_mono_fns = crate::commands::build_imported_mono_functions_for_test_runner(
        type_result,
        &imported_generic_sigs,
        &per_module_caches,
        &mut merged_pool,
        interner,
    );

    ImportedMonoState {
        merged_pool,
        re_interned_canons,
        imported_mono_fns,
    }
}
