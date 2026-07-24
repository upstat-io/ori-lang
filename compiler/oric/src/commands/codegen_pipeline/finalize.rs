//! Post-codegen finalization.
//!
//! `finalize_module` runs the LLVM-IR dump (if requested), the codegen audit
//! (if requested), and module verification. Returns the cloned module on
//! success or a diagnostic string on failure. Realized-artifact observation
//! lives at `dump_orchestrator::dump_realized_arc`, shared by every driver.

use ori_ir::StringInterner;
use ori_llvm::inkwell::module::Module;
use ori_llvm::SimpleCx;
use ori_types::Pool;

/// Post-codegen diagnostics-and-verify phase.
///
/// Runs the LLVM-IR dump (if `ORI_DUMP_AFTER_LLVM` / `ORI_DEBUG_LLVM` is set),
/// the codegen audit (if `ORI_AUDIT_CODEGEN` is set), and the explicit LLVM IR
/// verification. Returns the cloned module on success; `Err(msg)` when any
/// stage fails. The caller aborts AOT on `Err`.
///
/// IR dump runs BEFORE verify so the dump is emitted even when the module has
/// structural errors, matching the JIT path pattern.
pub(super) fn finalize_module<'ctx>(
    scx: &SimpleCx<'ctx>,
    codegen_errors: u32,
    codegen_descriptions: &[String],
    source_path: &str,
    pool: &Pool,
    interner: &StringInterner,
) -> Result<Module<'ctx>, String> {
    if codegen_errors > 0 {
        let details = if codegen_descriptions.is_empty() {
            String::new()
        } else {
            format!(":\n  - {}", codegen_descriptions.join("\n  - "))
        };
        return Err(format!(
            "LLVM codegen had {codegen_errors} error(s) — aborting AOT compilation{details}",
        ));
    }

    crate::dump_orchestrator::dump_llvm(pool, interner, source_path, || {
        scx.llmod.print_to_string().to_string_lossy().into_owned()
    });

    crate::dbg_do!(crate::debug_flags::ORI_AUDIT_CODEGEN, {
        let audit_report = ori_llvm::verify::audit_module(&scx.llmod);
        audit_report.emit_to_stderr();
        if audit_report.has_errors() {
            return Err(format!(
                "RC audit found {} error(s) — aborting",
                audit_report.error_count()
            ));
        }
    });

    if let Err(msg) = scx.llmod.verify() {
        return Err(format!("LLVM IR verification failed: {msg}"));
    }

    Ok(scx.llmod.clone())
}
