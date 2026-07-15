//! Dump orchestrator -- the single observation surface for phase IR dumps.
//!
//! The four phase-dump drivers (`ast_dump`, `ir_dump`, `arc_dump`, `llvm_dump`)
//! are facet renderers behind this orchestrator. The orchestrator owns the gate
//! (which phase is requested, via the `ORI_DUMP_AFTER_*` env flags) and the
//! per-phase view (the typeck idx-filter, folded in from `ORI_DUMP_TYPE_IDX`),
//! then dispatches to the facet renderer. One observation surface keyed on
//! `(phase, idx-filter)`; facet renderers behind it -- the `--print-ir-after
//! <phase>` / `-Z dump-mir` shape, never a parallel system.

use ori_ir::StringInterner;
use ori_parse::ParseOutput;
use ori_types::{Pool, TypedModule};

/// A compiler phase whose IR the orchestrator can dump.
///
/// `Parse` / `Typeck` fire on `ori check`; `Arc` / `Llvm` on `ori build`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DumpPhase {
    Parse,
    Typeck,
    Arc,
    Llvm,
}

/// The per-phase view component of the `(phase, idx-filter)` key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DumpView {
    /// Typeck `--with-idx`: annotate every type with its resolved pool `Idx`
    /// (the `ORI_DUMP_TYPE_IDX` capability). Always `false` for non-typeck phases.
    with_idx: bool,
}

/// The `(phase, idx-filter)` gate: `Some(view)` when the phase dump is requested
/// via its env flag(s), `None` otherwise. SSOT mapping each phase to its
/// `ORI_DUMP_AFTER_*` flag and (for typeck) the `ORI_DUMP_TYPE_IDX` idx-filter.
fn requested(phase: DumpPhase) -> Option<DumpView> {
    let gated = match phase {
        DumpPhase::Parse => crate::dbg_set!(crate::debug_flags::ORI_DUMP_AFTER_PARSE),
        DumpPhase::Typeck => crate::dbg_set!(crate::debug_flags::ORI_DUMP_AFTER_TYPECK),
        DumpPhase::Arc => crate::dbg_set!(crate::debug_flags::ORI_DUMP_AFTER_ARC),
        DumpPhase::Llvm => {
            crate::dbg_set!(crate::debug_flags::ORI_DUMP_AFTER_LLVM)
                || crate::dbg_set!(crate::debug_flags::ORI_DEBUG_LLVM)
        }
    };
    if !gated {
        return None;
    }
    let with_idx = matches!(phase, DumpPhase::Typeck)
        && crate::dbg_set!(crate::debug_flags::ORI_DUMP_TYPE_IDX);
    Some(DumpView { with_idx })
}

/// Dump the parsed AST (PARSE phase) to stderr when requested.
pub(crate) fn dump_parse(parse_result: &ParseOutput, interner: &StringInterner, path: &str) {
    if requested(DumpPhase::Parse).is_none() {
        return;
    }
    crate::ast_dump::dump_ast(parse_result, interner, path);
}

/// Dump the typed IR (TYPECK phase) to stderr when requested, applying the
/// idx-filter view (annotate every type with its resolved pool `Idx`).
pub(crate) fn dump_typeck(
    parse_result: &ParseOutput,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
    path: &str,
) {
    let Some(view) = requested(DumpPhase::Typeck) else {
        return;
    };
    crate::ir_dump::dump_typed_ir(parse_result, typed, pool, interner, path, view.with_idx);
}

/// Dump the already-realized ARC artifact when requested.
#[cfg(feature = "llvm")]
pub(crate) fn dump_arc(
    executable: &ori_repr::executable::ExecutableProgram,
    interner: &StringInterner,
    path: &str,
) {
    if requested(DumpPhase::Arc).is_none() {
        return;
    }
    crate::arc_dump::dump_arc_ir(executable.functions(), executable.pool(), interner, path);
}

/// Dump the annotated LLVM IR (LLVM phase) to stderr when requested. `ir_text`
/// is a thunk so the expensive module-to-string print only runs when gated on.
#[cfg(feature = "llvm")]
pub(crate) fn dump_llvm(
    pool: &Pool,
    interner: &StringInterner,
    path: &str,
    ir_text: impl FnOnce() -> String,
) {
    if requested(DumpPhase::Llvm).is_none() {
        return;
    }
    crate::llvm_dump::dump_llvm_ir(&ir_text(), pool, interner, path);
}
