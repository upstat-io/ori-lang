//! Multi-file build pipeline (with imports).
//!
//! Builds all dependent modules in topological order, compiles each to an
//! object file (or bitcode for LTO), and links them into a single executable.

use std::path::{Path, PathBuf};

use super::multi_emission::{emit_module_artifact, lto_merge};
use super::multi_imports::build_import_infos;
use super::multi_imports::{collect_imported_collection_surfaces, collect_imported_type_metadata};
use super::multi_mono_state::build_imported_mono_state;
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

    let target = configure_target(options).unwrap_or_else(|e| report_codegen_error(e));

    if options.verbose {
        eprintln!("  Target: {}", target.triple());
        eprintln!("  Optimization: {:?}", options.opt_level);
    }

    // Why: tempfile gives a unique directory, avoiding races in parallel builds.
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
    };

    let module_count = dep_result.compilation_order.len();
    let mut compiled_modules: Vec<CompiledModuleInfo> = Vec::with_capacity(module_count);
    let mut object_files: Vec<PathBuf> = Vec::with_capacity(module_count);

    for source_path in &dep_result.compilation_order {
        match compile_single_module(&compile_ctx, source_path, &compiled_modules) {
            Some((obj_path, module_info)) => {
                compiled_modules.push(module_info);
                object_files.push(obj_path);
            }
            None => std::process::exit(1),
        }
    }

    super::require_cli_entry(
        path,
        options,
        compiled_modules.iter().any(|module| module.has_cli_entry),
    );

    let is_lto = !matches!(options.lto, LtoMode::Off);
    let final_object_files = if is_lto && object_files.len() > 1 {
        lto_merge(&object_files, &obj_dir, &target, &opt_config, options)
    } else {
        object_files
    };

    // INVARIANT: temp_dir stays alive until linking completes.
    let output_path = determine_output_path(path, options);
    link_and_finish(
        final_object_files,
        &output_path,
        &target,
        options,
        &opt_config.sanitizer,
        start,
    );

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
}

/// Producer export projected from the same closed artifact LLVM consumed.
pub(super) type ExportedFunctionInfo = crate::commands::codegen_pipeline::RealizedCallableExport;

/// Information about a compiled module, including its function signatures.
pub(super) struct CompiledModuleInfo {
    /// Path to the source file.
    pub(super) path: PathBuf,
    /// Whether this module declares the executable `@main` entry point.
    pub(super) has_cli_entry: bool,
    /// Public function signatures.
    pub(super) public_functions: Vec<ExportedFunctionInfo>,
    /// Exported type metadata (repr attrs + visibility) for cross-module repr
    /// plan construction. Imported modules' metadata is fed into `ReprPlan` so
    /// that `pub` and `#repr(...)` types are not incorrectly narrowed.
    pub(super) exported_type_metadata: Vec<ori_types::ExportedTypeMetadata>,
    /// Merkle hashes of collection types in public function signatures.
    /// Forwarded for downstream metadata only; does not suppress narrowing.
    pub(super) exported_collection_surfaces: Vec<u64>,
}

/// Compile a single module's typed + canonicalized IR to an LLVM module.
///
/// Wraps `compile_to_llvm_with_imports` with the `VerificationFailed`
/// diagnostic path so the caller can stay focused on orchestration. Returns
/// `None` on codegen error after emitting diagnostics.
#[expect(
    clippy::too_many_arguments,
    reason = "thin wrapper around compile_to_llvm_with_imports; structural cure would push 11+ args one level deeper"
)]
fn lower_module_to_llvm<'ctx>(
    context: &'ctx ori_llvm::inkwell::context::Context,
    ctx: &ModuleCompileContext<'_>,
    source_path_str: &str,
    module_name: &str,
    parse_result: &crate::parser::ParseOutput,
    type_result: &ori_types::TypeCheckResult,
    merged_pool: &'ctx ori_types::Pool,
    canon_result: &ori_ir::canon::CanonResult,
    imported_functions: &[crate::commands::compile_common::ImportedFunctionInfo],
    imported_type_metadata: &[ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &[u64],
    imported: crate::commands::ImportedSurfaces<'_>,
) -> Option<crate::commands::codegen_pipeline::LlvmCodegenOutput<'ctx>> {
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
        imported,
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

    let module_name = derive_module_name(source_path, Some(ctx.base_dir));

    let file = SourceFile::new(ctx.db, source_path.to_path_buf(), content);
    let (parse_result, type_result, pool, canon_result) =
        check_source(ctx.db, file, &source_path_str)?;

    let resolved_imports = crate::imports::resolve_imports(ctx.db, &parse_result, source_path);
    let imported_functions = {
        use oric::Db;
        build_import_infos(
            source_path,
            ctx.graph,
            compiled_modules,
            ctx.base_dir,
            ctx.mangler,
            &resolved_imports,
            ctx.db.interner(),
        )
    };

    // Why: imported `pub` and `#repr(...)` types must be exempted from integer
    // narrowing when the repr plan is constructed.
    let imported_type_metadata =
        collect_imported_type_metadata(source_path, ctx.graph, compiled_modules);
    let imported_collection_surfaces =
        collect_imported_collection_surfaces(source_path, ctx.graph, compiled_modules);

    let imported_state =
        build_imported_mono_state(ctx.db, source_path, &parse_result, &type_result, &pool);

    let context = Context::create();
    let llvm_output = lower_module_to_llvm(
        &context,
        ctx,
        &source_path_str,
        &module_name,
        &parse_result,
        &type_result,
        &imported_state.merged_pool,
        &canon_result,
        &imported_functions,
        &imported_type_metadata,
        &imported_collection_surfaces,
        imported_state.surfaces(),
    )?;

    let obj_path = emit_module_artifact(ctx, &llvm_output.module, &module_name, &source_path_str)?;

    let exported_type_metadata = type_result.typed.exported_type_metadata.clone();
    let exported_collection_surfaces = type_result.typed.exported_collection_surfaces.clone();
    let has_cli_entry = super::module_has_cli_entry(&parse_result, &type_result);
    let crate::commands::codegen_pipeline::LlvmCodegenOutput {
        module,
        exports: public_functions,
    } = llvm_output;
    drop(module);
    let module_info = CompiledModuleInfo {
        path: source_path.to_path_buf(),
        has_cli_entry,
        public_functions,
        exported_type_metadata,
        exported_collection_surfaces,
    };

    Some((obj_path, module_info))
}
