//! Immortal object detection for the AIMS pipeline.
//!
//! Immortal objects have their reference count permanently set to `MAX_REFCOUNT`,
//! causing all RC operations to be no-ops. This eliminates RC traffic for
//! heap-allocated literals that are never freed (e.g., empty string `""`).
//!
//! Variables marked as immortal are excluded from RC emission, COW annotation,
//! reuse detection, and drop hints — the same treatment as `SCALAR` variables,
//! but for heap-allocated values that happen to be statically known constants.
//!
//! # Relationship to Scalars
//!
//! Scalar variables (`ArcClass::Scalar`) never have heap allocations at all.
//! Immortal variables DO have heap allocations, but those allocations use
//! pre-allocated singletons with `MAX_REFCOUNT` — so RC operations are
//! unnecessary. Both are excluded from the same emission phases, but for
//! different reasons.

#[cfg(test)]
mod tests;

use ori_ir::{Name, StringInterner};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, LitValue};

/// Detect immortal variables in a function.
///
/// Scans the function's instructions for `Let` bindings to immortal-eligible
/// literal values. Returns a parallel bitvector indexed by `ArcVarId::index()`.
///
/// # Immortal-Eligible Values (Stage 1)
///
/// - Empty string literal `""` — heap-allocated `DefiniteRef` with zero content
///
/// Boolean/int/unit/char literals are already `ArcClass::Scalar` and skip RC
/// via the scalar fast path. Immortal detection focuses on heap-allocated
/// literals that would otherwise incur RC operations.
pub fn detect_immortals(func: &ArcFunction, interner: &StringInterner) -> Vec<bool> {
    let num_vars = func.var_types.len();
    let mut immortals = vec![false; num_vars];

    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Literal(lit),
                ..
            } = instr
            {
                if is_immortal_literal(lit, interner) {
                    if let Some(entry) = immortals.get_mut(dst.index()) {
                        *entry = true;
                    }
                }
            }
        }
    }

    immortals
}

/// Whether a literal value qualifies as immortal.
///
/// Stage 1: only empty string `""`. Future stages may add empty list `[]`,
/// empty map `{}`, and static string constants.
fn is_immortal_literal(lit: &LitValue, interner: &StringInterner) -> bool {
    match lit {
        LitValue::String(name) => is_empty_string(*name, interner),
        // Int, Float, Bool, Char, Duration, Size, Unit are all Scalar —
        // already handled by the scalar fast path, no benefit from immortal.
        _ => false,
    }
}

/// Check whether an interned string name represents the empty string `""`.
fn is_empty_string(name: Name, interner: &StringInterner) -> bool {
    interner.lookup(name).is_empty()
}

/// Count of immortal variables detected (for tracing).
pub fn count_immortals(immortals: &[bool]) -> usize {
    immortals.iter().filter(|&&v| v).count()
}
