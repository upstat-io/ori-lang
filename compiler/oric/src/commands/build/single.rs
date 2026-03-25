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

    use crate::commands::compile_common::{check_source, compile_to_llvm};
    use crate::problem::codegen::{report_codegen_error, CodegenProblem};

    // Step 1: Parse and type-check the source file
    if options.verbose {
        eprintln!("  Compiling {path}...");
    }

    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    // Check for parse and type errors
    let Some((parse_result, type_result, pool, canon_result)) = check_source(&db, file, path)
    else {
        std::process::exit(1)
    };

    // Step 2: Configure target
    let target = configure_target(options).unwrap_or_else(|e| report_codegen_error(e));

    if options.verbose {
        eprintln!("  Target: {}", target.triple());
        eprintln!("  Optimization: {:?}", options.opt_level);
    }

    // Step 3: Generate LLVM IR
    // This is the Salsa/ArtifactCache boundary: Salsa queries (tokens -> parsed ->
    // typed) are done; codegen uses ArtifactCache for incremental caching.
    let context = Context::create();
    let llvm_module = compile_to_llvm(
        &context,
        &db,
        &parse_result,
        &type_result,
        &pool,
        &canon_result,
        path,
        Some(target.triple()),
        options.narrowing_policy,
    )
    .unwrap_or_else(|e| report_codegen_error(CodegenProblem::VerificationFailed { message: e }));

    // Configure module for target
    let emitter = ObjectEmitter::new(&target).unwrap_or_else(|e| report_codegen_error(e));

    if let Err(e) = emitter.configure_module(&llvm_module) {
        report_codegen_error(CodegenProblem::ModuleConfigFailed {
            message: e.to_string(),
        });
    }

    // Step 4: Build optimization config
    let opt_config = build_optimization_config(options);

    // Step 5: Determine output path
    let output_path = determine_output_path(path, options);

    // Step 6: Emit based on emit type (--emit flag)
    // For --emit, we still verify+optimize first, then emit in the requested format.
    if let Some(emit_type) = options.emit {
        if let Err(e) = ori_llvm::aot::optimize_module(&llvm_module, emitter.machine(), &opt_config)
        {
            report_codegen_error(e);
        }
        emit_and_finish(
            &llvm_module,
            &emitter,
            &output_path,
            emit_type,
            options,
            start,
        );
        return;
    }

    // Step 7: Verify -> optimize -> emit object file via unified pipeline
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
    let module_name = Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");
    let obj_path = temp_dir.path().join(format!("{module_name}.o"));

    if options.verbose {
        eprintln!("  Emitting object to {}", obj_path.display());
    }

    if let Err(e) = emitter.verify_optimize_emit(
        &llvm_module,
        &opt_config,
        &obj_path,
        ori_llvm::aot::OutputFormat::Object,
    ) {
        report_codegen_error(e);
    }

    // Step 8: Link into executable
    // Note: temp_dir must stay alive until linking completes (auto-cleaned on drop)
    link_and_finish(vec![obj_path], &output_path, &target, options, start);
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
