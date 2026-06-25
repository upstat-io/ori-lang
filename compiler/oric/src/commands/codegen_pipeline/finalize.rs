//! Phase-dump + post-codegen finalization helpers.
//!
//! - `dump_arc_phases`: the `ORI_DUMP_AFTER_ARC` + `ORI_EMIT_ARC_DOT` phase-dump
//!   invocations. Both are `dbg_do!`-gated — no-op when the env vars are unset.
//! - `finalize_module`: the post-codegen diagnostics-and-verify phase. Runs
//!   LLVM-IR dump (if requested), codegen audit (if requested), and module
//!   verification. Returns the cloned module on success or a diagnostic string
//!   on failure.

#[cfg(feature = "llvm")]
use ori_ir::StringInterner;
#[cfg(feature = "llvm")]
use ori_llvm::inkwell::module::Module;
#[cfg(feature = "llvm")]
use ori_llvm::SimpleCx;
#[cfg(feature = "llvm")]
use ori_types::Pool;
#[cfg(feature = "llvm")]
use rustc_hash::FxHashMap;

/// Emit ARC-IR phase dumps when the corresponding env-var gates are set.
///
/// Both dumps are gated via `crate::dbg_do!`; the function is a single call
/// site for both invocations so the caller doesn't carry the `dbg_do!` noise
/// inline.
#[cfg(feature = "llvm")]
pub(super) fn dump_arc_phases(
    arc_cache: &FxHashMap<ori_ir::Name, (ori_arc::ArcFunction, Vec<ori_arc::ArcFunction>)>,
    annotated_sigs: &FxHashMap<ori_ir::Name, ori_arc::AnnotatedSig>,
    classifier: &ori_arc::ArcClassifier,
    pool: &Pool,
    interner: &StringInterner,
    type_registry: &ori_types::TypeRegistry,
    source_path: &str,
) {
    crate::dbg_do!(crate::debug_flags::ORI_DUMP_AFTER_ARC, {
        crate::arc_dump::dump_arc_ir(
            arc_cache,
            annotated_sigs,
            classifier,
            pool,
            interner,
            type_registry,
            source_path,
        );
    });
    crate::dbg_do!(crate::debug_flags::ORI_EMIT_ARC_DOT, {
        crate::arc_dot::emit_arc_dot(
            arc_cache,
            annotated_sigs,
            classifier,
            pool,
            interner,
            type_registry,
        );
    });
}

/// Post-codegen diagnostics-and-verify phase.
///
/// Runs the LLVM-IR dump (if `ORI_DUMP_AFTER_LLVM` / `ORI_DEBUG_LLVM` is set),
/// the codegen audit (if `ORI_AUDIT_CODEGEN` is set), and the explicit LLVM IR
/// verification. Returns the cloned module on success; `Err(msg)` when any
/// stage fails. The caller aborts AOT on `Err`.
///
/// Order mirrors the in-line code it replaces — IR dump BEFORE verify so the
/// dump is emitted even when the module has structural errors, matching the
/// JIT path pattern.
#[cfg(feature = "llvm")]
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

    if crate::llvm_dump::llvm_dump_requested() {
        crate::llvm_dump::dump_llvm_ir(
            &scx.llmod.print_to_string().to_string_lossy(),
            pool,
            interner,
            source_path,
        );
    }

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
