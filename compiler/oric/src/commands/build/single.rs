//! Single-file build pipeline (no imports).
//!
//! Compiles a single Ori source file through parse, type-check, LLVM codegen,
//! optimization, and linking into a native executable.

use std::path::Path;
use std::path::PathBuf;

use super::{
    build_optimization_config, configure_target, determine_output_path, link_and_finish,
    BuildOptions, EmitType,
};

/// Build a single Ori source file (no imports).
pub(super) fn build_file_single(
    path: &str,
    content: String,
    options: &BuildOptions,
    start: std::time::Instant,
) {
    use ori_llvm::aot::ObjectEmitter;
    use ori_llvm::inkwell::context::Context;
    use oric::{CompilerDb, SourceFile};
    use tempfile::TempDir;

    use crate::commands::build::build_imported_mono_state;
    use crate::commands::compile_common::{check_source, compile_to_llvm_with_imported_monos};
    use crate::problem::codegen::{report_codegen_error, CodegenProblem};

    if options.verbose {
        eprintln!("  Compiling {path}...");
    }

    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let Some((parse_result, type_result, pool, canon_result)) = check_source(&db, file, path)
    else {
        std::process::exit(1)
    };
    super::require_cli_entry(
        path,
        options,
        super::module_has_cli_entry(&parse_result, &type_result),
    );

    let target = configure_target(options).unwrap_or_else(|e| report_codegen_error(e));

    if options.verbose {
        eprintln!("  Target: {}", target.triple());
        eprintln!("  Optimization: {:?}", options.opt_level);
    }

    // Why: imported generic instances (stdlib or relative imports) need mono
    // state or LLVM verification fails with an unresolved mono instance.
    // INVARIANT: the merged pool outlives the codegen call — the returned LLVM
    // module's type references point into it.
    let imported_state =
        build_imported_mono_state(&db, Path::new(path), &parse_result, &type_result, &pool);

    // Salsa/ArtifactCache boundary: Salsa queries end here; codegen uses ArtifactCache.
    let context = Context::create();
    let llvm_module = compile_to_llvm_with_imported_monos(
        &context,
        &db,
        &parse_result,
        &type_result,
        &imported_state.merged_pool,
        imported_state.surfaces(),
        &canon_result,
        path,
        Some(target.triple()),
        options.narrowing_policy,
    )
    .unwrap_or_else(|e| report_codegen_error(CodegenProblem::VerificationFailed { message: e }));

    let emitter = ObjectEmitter::new(&target).unwrap_or_else(|e| report_codegen_error(e));

    if let Err(e) = emitter.configure_module(&llvm_module) {
        report_codegen_error(CodegenProblem::ModuleConfigFailed {
            message: e.to_string(),
        });
    }

    let opt_config = build_optimization_config(options);
    let output_path = determine_output_path(path, options);

    if let Some(emit_type) = options.emit {
        handle_emit_type(
            &llvm_module,
            &emitter,
            &output_path,
            emit_type,
            &opt_config,
            options,
            start,
        );
        return;
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
    let module_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let obj_path = temp_dir.path().join(format!("{module_name}.o"));

    if options.verbose {
        eprintln!("  Emitting object to {}", obj_path.display());
    }

    let hooks = super::ir_capture::build_hooks(path);
    if let Err(e) = emitter.verify_optimize_emit(
        &llvm_module,
        &opt_config,
        &obj_path,
        ori_llvm::aot::OutputFormat::Object,
        hooks,
    ) {
        report_codegen_error(e);
    }

    // INVARIANT: temp_dir stays alive until linking completes (cleaned on drop).
    link_and_finish(
        vec![obj_path],
        &output_path,
        &target,
        options,
        &opt_config.sanitizer,
        start,
    );
}

/// Handle `--emit` flag: optimize then emit in the requested format.
/// When sanitizers are enabled and emit type is Object, routes through Clang delegation.
fn handle_emit_type(
    llvm_module: &ori_llvm::inkwell::module::Module<'_>,
    emitter: &ori_llvm::aot::ObjectEmitter,
    output_path: &Path,
    emit_type: EmitType,
    opt_config: &ori_llvm::aot::OptimizationConfig,
    options: &BuildOptions,
    start: std::time::Instant,
) {
    use crate::problem::codegen::report_codegen_error;

    if let Err(e) = ori_llvm::aot::optimize_module(llvm_module, emitter.machine(), opt_config) {
        report_codegen_error(e);
    }
    if ori_llvm::verify::audit_requested() {
        let audit_opts = ori_llvm::verify::AuditOptions::from_env();
        let stats = ori_llvm::verify::audit_module_histogram_only(llvm_module, &audit_opts);
        stats.emit_to_stderr();
    }
    if opt_config.sanitizer.any_enabled() {
        if emit_type == EmitType::Object {
            emit_sanitized_object(
                llvm_module,
                emitter,
                output_path,
                opt_config,
                options,
                start,
            );
            return;
        }
        // Warn: --emit ir/asm/bc with sanitizers produces uninstrumented output.
        // Sanitizer passes are added by Clang during object compilation, not by LLVM.
        eprintln!(
            "warning: --emit {emit_type:?} with ORI_SANITIZE produces pre-sanitizer output. \
             Sanitizer instrumentation is only visible in --emit object."
        );
    }
    emit_and_finish(llvm_module, emitter, output_path, emit_type, options, start);
}

/// Emit a sanitizer-instrumented object via Clang delegation (used for --emit object with sanitizers).
fn emit_sanitized_object(
    llvm_module: &ori_llvm::inkwell::module::Module<'_>,
    emitter: &ori_llvm::aot::ObjectEmitter,
    output_path: &Path,
    opt_config: &ori_llvm::aot::OptimizationConfig,
    options: &BuildOptions,
    start: std::time::Instant,
) {
    use crate::problem::codegen::report_codegen_error;

    // Use the output path as-is (respects user's -o flag) with object extension
    let emit_path = if output_path.extension().is_some() {
        output_path.to_path_buf()
    } else {
        output_path.with_extension("o")
    };
    let target = emitter.config().triple().to_string();
    if let Err(e) = ori_llvm::aot::clang_sanitize_object(
        emitter,
        llvm_module,
        &emit_path,
        &opt_config.sanitizer,
        opt_config.level.as_clang_flag(),
        &target,
    ) {
        report_codegen_error(e);
    }
    if options.verbose {
        eprintln!(
            "  Finished {} (sanitized) in {:.2}s",
            emit_path.display(),
            start.elapsed().as_secs_f64()
        );
    }
}

/// Emit a module and finish (used for --emit flag).
fn emit_and_finish(
    llvm_module: &ori_llvm::inkwell::module::Module<'_>,
    emitter: &ori_llvm::aot::ObjectEmitter,
    output_path: &Path,
    emit_type: EmitType,
    options: &BuildOptions,
    start: std::time::Instant,
) {
    use ori_llvm::aot::OutputFormat;

    let emit_path = output_path.with_extension(emit_type.extension());

    if options.verbose {
        eprintln!("  Emitting {:?} to {}", emit_type, emit_path.display());
    }

    let format = match emit_type {
        EmitType::Object => OutputFormat::Object,
        EmitType::LlvmIr => OutputFormat::LlvmIr,
        EmitType::LlvmBc => OutputFormat::Bitcode,
        EmitType::Assembly => OutputFormat::Assembly,
    };

    if let Err(e) = emitter.emit(llvm_module, &emit_path, format) {
        crate::problem::codegen::report_codegen_error(e);
    }

    let elapsed = start.elapsed();
    if options.verbose {
        eprintln!("  Finished in {:.2}s", elapsed.as_secs_f64());
    }
}
