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

fn resolve_import_path(current: &Path, import: &str) -> Result<PathBuf, String> {
    let dir = current.parent().unwrap_or_else(|| Path::new("."));
    let resolved = dir.join(import);
    let path = if resolved.extension().is_none() {
        resolved.with_extension("ori")
    } else {
        resolved
    };
    path.exists()
        .then_some(path.clone())
        .ok_or_else(|| format!("cannot find '{import}' at '{}'", path.display()))
}

fn load_dependency_graph(path: &str, verbose: bool) -> ori_llvm::aot::DependencyBuildResult {
    use crate::problem::codegen::{report_codegen_error, CodegenProblem};

    if verbose {
        eprintln!("  Building dependency graph...");
    }
    let entry = Path::new(path)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(path));
    let result = ori_llvm::aot::build_dependency_graph(&entry, resolve_import_path).unwrap_or_else(
        |error| {
            report_codegen_error(CodegenProblem::ModuleConfigFailed {
                message: error.to_string(),
            })
        },
    );
    if verbose {
        eprintln!(
            "  Found {} files to compile",
            result.compilation_order.len()
        );
        for (index, path) in result.compilation_order.iter().enumerate() {
            eprintln!("    {}: {}", index + 1, path.display());
        }
    }
    result
}

fn compile_modules(
    context: &ModuleCompileContext<'_>,
    order: &[PathBuf],
) -> (Vec<PathBuf>, Vec<CompiledModuleInfo>) {
    let mut modules = Vec::with_capacity(order.len());
    let mut objects = Vec::with_capacity(order.len());
    for source_path in order {
        let Some((object, module)) = compile_single_module(context, source_path, &modules) else {
            std::process::exit(1);
        };
        modules.push(module);
        objects.push(object);
    }
    (objects, modules)
}

/// Build a multi-file Ori program (with imports).
///
/// This builds all dependent modules in topological order and links them together.
pub(super) fn build_file_multi(path: &str, options: &BuildOptions, start: std::time::Instant) {
    use oric::CompilerDb;
    use tempfile::TempDir;

    use crate::problem::codegen::{report_codegen_error, CodegenProblem};

    let dep_result = load_dependency_graph(path, options.verbose);

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
    let opt_config = build_optimization_config(options);

    // Create compilation context (avoids passing many parameters to helper)
    let compile_ctx = ModuleCompileContext {
        db: &db,
        target: &target,
        opt_config: &opt_config,
        graph: &dep_result.graph,
        base_dir: &dep_result.base_dir,
        obj_dir: &obj_dir,
        verbose: options.verbose,
        narrowing_policy: options.narrowing_policy,
    };

    let (object_files, compiled_modules) =
        compile_modules(&compile_ctx, &dep_result.compilation_order);

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
#[derive(Clone, Copy)]
struct ModuleLoweringInput<'ctx, 'a> {
    context: &'ctx ori_llvm::inkwell::context::Context,
    build: &'a ModuleCompileContext<'a>,
    source_path: &'a str,
    module_name: &'a str,
    parse: &'a crate::parser::ParseOutput,
    typed: &'a ori_types::TypeCheckResult,
    pool: &'ctx ori_types::Pool,
    canon: &'a ori_ir::canon::CanonResult,
    imported_functions: &'a [crate::commands::compile_common::ImportedFunctionInfo],
    imported_type_metadata: &'a [ori_types::ExportedTypeMetadata],
    imported_collection_surfaces: &'a [u64],
    imported: crate::commands::ImportedSurfaces<'a>,
}

fn lower_module_to_llvm<'ctx>(
    input: ModuleLoweringInput<'ctx, '_>,
) -> Option<crate::commands::codegen_pipeline::LlvmCodegenOutput<'ctx>> {
    use crate::commands::compile_common::compile_to_llvm_with_imports;
    use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics, CodegenProblem};

    let ModuleLoweringInput {
        context,
        build: ctx,
        source_path: source_path_str,
        module_name,
        parse: parse_result,
        typed: type_result,
        pool: merged_pool,
        canon: canon_result,
        imported_functions,
        imported_type_metadata,
        imported_collection_surfaces,
        imported,
    } = input;

    match compile_to_llvm_with_imports(crate::commands::compile_common::ImportedModuleCompilation {
        context,
        db: ctx.db,
        parse: parse_result,
        typed: type_result,
        pool: merged_pool,
        canon: canon_result,
        source_path: source_path_str,
        module_name,
        imported_functions,
        imported_type_metadata,
        imported_collection_surfaces,
        imported,
        target_triple: Some(ctx.target.triple()),
        narrowing_policy: ctx.narrowing_policy,
    }) {
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
    use oric::{Db, SourceFile};

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
    let imported_state =
        build_imported_mono_state(ctx.db, source_path, &parse_result, &type_result, &pool);
    let imported_functions = match build_import_infos(
        source_path,
        ctx.graph,
        compiled_modules,
        &resolved_imports,
        &imported_state.re_interned_function_sigs,
        ctx.db.interner(),
    ) {
        Ok(imported_functions) => imported_functions,
        Err(message) => {
            let mut diagnostics = CodegenDiagnostics::new();
            diagnostics.push(CodegenProblem::VerificationFailed { message });
            emit_codegen_diagnostics(diagnostics);
            return None;
        }
    };

    // Why: imported `pub` and `#repr(...)` types must be exempted from integer
    // narrowing when the repr plan is constructed.
    let imported_type_metadata =
        collect_imported_type_metadata(source_path, ctx.graph, compiled_modules);
    let imported_collection_surfaces =
        collect_imported_collection_surfaces(source_path, ctx.graph, compiled_modules);

    let context = Context::create();
    let llvm_output = lower_module_to_llvm(ModuleLoweringInput {
        context: &context,
        build: ctx,
        source_path: &source_path_str,
        module_name: &module_name,
        parse: &parse_result,
        typed: &type_result,
        pool: &imported_state.merged_pool,
        canon: &canon_result,
        imported_functions: &imported_functions,
        imported_type_metadata: &imported_type_metadata,
        imported_collection_surfaces: &imported_collection_surfaces,
        imported: imported_state.surfaces(),
    })?;

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
