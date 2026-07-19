//! Central gate for compiler IR dumps.
//!
//! [`DumpPhase`] and [`DumpView`] form the observation key. Each enabled phase
//! delegates rendering to one facet; `ORI_DUMP_TYPE_IDX` only decorates typeck.

use ori_ir::StringInterner;
use ori_parse::ParseOutput;
use ori_types::{Pool, TypedModule};

/// Identifies one observable compiler IR boundary.
///
/// `Parse` / `Typeck` fire on `ori check`; `Arc` / `Llvm` on `ori build`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DumpPhase {
    Parse,
    Typeck,
    Arc,
    Llvm,
}

/// Type-filter settings attached to a requested dump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DumpView {
    /// Enables resolved `Idx` annotations for type-check dumps.
    with_idx: bool,
}

/// Resolves the environment-controlled view for `phase`.
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

/// Emits the parsed AST when the parse dump gate is enabled.
pub(crate) fn dump_parse(parse_result: &ParseOutput, interner: &StringInterner, path: &str) {
    if requested(DumpPhase::Parse).is_none() {
        return;
    }
    crate::ast_dump::dump_ast(parse_result, interner, path);
}

/// Emits typed IR with optional resolved-`Idx` annotations when enabled.
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

/// Emits the realized ARC artifact when enabled.
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

/// Emits LLVM IR when enabled and delays `ir_text` until after the gate.
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
