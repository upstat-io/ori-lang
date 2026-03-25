//! LLVM module emission helpers for multi-file builds.
//!
//! Extracted from `multi.rs` to keep the main pipeline under the
//! 500-line limit. Contains LTO merge and per-module artifact emission.

use std::path::{Path, PathBuf};

use super::BuildOptions;

use super::multi::ModuleCompileContext;

/// Merge bitcode files via LTO pipeline, returning a single object file path.
///
/// All error paths diverge via `report_codegen_error` (which exits the process).
pub(super) fn lto_merge(
    object_files: &[PathBuf],
    obj_dir: &Path,
    target: &ori_llvm::aot::TargetConfig,
    opt_config: &ori_llvm::aot::OptimizationConfig,
    options: &BuildOptions,
) -> Vec<PathBuf> {
    use ori_llvm::aot::ObjectEmitter;
    use ori_llvm::inkwell::context::Context;
    use ori_llvm::inkwell::module::Module;

    use crate::problem::codegen::{report_codegen_error, CodegenProblem};

    if options.verbose {
        eprintln!("  Running LTO merge ({} modules)...", object_files.len());
    }

    let lto_context = Context::create();
    // Load first bitcode as the base module
    let merged_module = Module::parse_bitcode_from_path(&object_files[0], &lto_context)
        .unwrap_or_else(|e| {
            report_codegen_error(CodegenProblem::EmissionFailed {
                format: "bitcode".into(),
                path: object_files[0].display().to_string(),
                message: format!("failed to load: {e}"),
            });
        });

    // Link remaining bitcode modules into the base
    for bc_path in &object_files[1..] {
        let other = Module::parse_bitcode_from_path(bc_path, &lto_context).unwrap_or_else(|e| {
            report_codegen_error(CodegenProblem::EmissionFailed {
                format: "bitcode".into(),
                path: bc_path.display().to_string(),
                message: format!("failed to load: {e}"),
            });
        });
        if let Err(e) = merged_module.link_in_module(other) {
            report_codegen_error(CodegenProblem::OptimizationFailed {
                pipeline: "LTO module linking".into(),
                message: e.to_string(),
            });
        }
    }

    // Configure merged module for target
    let emitter = ObjectEmitter::new(target).unwrap_or_else(|e| report_codegen_error(e));

    if let Err(e) = emitter.configure_module(&merged_module) {
        report_codegen_error(CodegenProblem::ModuleConfigFailed {
            message: e.to_string(),
        });
    }

    // Run LTO pipeline on merged module
    if let Err(e) = ori_llvm::aot::run_lto_pipeline(&merged_module, emitter.machine(), opt_config) {
        report_codegen_error(e);
    }

    // Emit final object
    let final_obj = obj_dir.join("merged_lto.o");
    if let Err(e) = emitter.emit_object(&merged_module, &final_obj) {
        report_codegen_error(CodegenProblem::EmissionFailed {
            format: "LTO object".into(),
            path: final_obj.display().to_string(),
            message: e.to_string(),
        });
    }

    if options.verbose {
        eprintln!("  LTO merge complete -> {}", final_obj.display());
    }

    vec![final_obj]
}

/// Configure, optimize, and emit a compiled LLVM module to an object or bitcode file.
///
/// Handles both LTO (pre-link + bitcode emit) and non-LTO (verify + optimize + emit)
/// pipelines. Returns the output file path on success.
pub(super) fn emit_module_artifact(
    ctx: &ModuleCompileContext<'_>,
    llvm_module: &ori_llvm::inkwell::module::Module<'_>,
    module_name: &str,
) -> Option<PathBuf> {
    use ori_llvm::aot::ObjectEmitter;

    use crate::problem::codegen::{emit_codegen_diagnostics, CodegenDiagnostics, CodegenProblem};

    let emitter = match ObjectEmitter::new(ctx.target) {
        Ok(e) => e,
        Err(e) => {
            let mut acc = CodegenDiagnostics::new();
            acc.push(e.into());
            emit_codegen_diagnostics(acc);
            return None;
        }
    };

    if let Err(e) = emitter.configure_module(llvm_module) {
        let mut acc = CodegenDiagnostics::new();
        acc.push(CodegenProblem::ModuleConfigFailed {
            message: e.to_string(),
        });
        emit_codegen_diagnostics(acc);
        return None;
    }

    let is_lto = !matches!(ctx.opt_config.lto, ori_llvm::aot::LtoMode::Off);
    let safe_name = module_name.replace('$', "_");

    if is_lto {
        // LTO: run pre-link pipeline and emit bitcode
        let bc_path = ctx.obj_dir.join(format!("{safe_name}.bc"));
        if ctx.verbose {
            eprintln!(
                "    Emitting bitcode to {} (LTO pre-link)",
                bc_path.display()
            );
        }
        if let Err(e) = ori_llvm::aot::prelink_and_emit_bitcode(
            llvm_module,
            emitter.machine(),
            ctx.opt_config,
            &bc_path,
        ) {
            let mut acc = CodegenDiagnostics::new();
            acc.push(e.into());
            emit_codegen_diagnostics(acc);
            return None;
        }
        return Some(bc_path);
    }

    // Non-LTO: verify, optimize, emit object
    let obj_path = ctx.obj_dir.join(format!("{safe_name}.o"));
    if ctx.verbose {
        eprintln!("    Emitting object to {}", obj_path.display());
    }

    if let Err(e) = emitter.verify_optimize_emit(
        llvm_module,
        ctx.opt_config,
        &obj_path,
        ori_llvm::aot::OutputFormat::Object,
    ) {
        let mut acc = CodegenDiagnostics::new();
        acc.push(e.into());
        emit_codegen_diagnostics(acc);
        return None;
    }

    Some(obj_path)
}
