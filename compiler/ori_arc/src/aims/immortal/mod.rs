//! Immortal object detection for the AIMS pipeline.
//!
//! Immortal values have process-wide logical lifetime and therefore carry no
//! per-use ownership-count or cleanup obligation. A physical plan may realize
//! that contract with a saturated counter, static storage, an immediate, or any
//! other validated mechanism; none of those encodings is an AIMS fact.
//!
//! Variables marked as immortal are excluded from RC emission, COW annotation,
//! reuse detection, and drop hints — the same treatment as `SCALAR` variables,
//! but for non-scalar values whose stable identity is known to be immortal.
//!
//! # Relationship to Scalars
//!
//! Scalar variables (`ArcClass::Scalar`) carry no logical allocation identity.
//! Immortal variables may carry one, but its process-wide lifetime makes
//! ownership-count and cleanup events unnecessary. Both are excluded from the
//! same emission phases for distinct backend-neutral reasons.

#[cfg(test)]
mod tests;

use ori_ir::{Name, StringInterner};

use crate::ir::{ArcFunction, ArcInstr, ArcValue, LitValue};

/// Detect immortal variables in a function.
///
/// Scans the function's instructions for `Let` bindings to immortal-eligible
/// literal values. Returns a parallel bitvector indexed by `ArcVarId::index()`.
///
/// # Immortal-Eligible Values (v1)
///
/// - Empty string literal `""` — registry-defined immortal string identity
///
/// Boolean/int/unit/char literals are already `ArcClass::Scalar` and skip RC
/// via the scalar fast path. Immortal detection focuses on non-scalar literals
/// whose logical lifetime makes ownership-count events unnecessary.
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
/// Currently only empty string `""`. Future versions may add empty list `[]`,
/// empty map `{}`, and static string constants.
fn is_immortal_literal(lit: &LitValue, interner: &StringInterner) -> bool {
    match lit {
        LitValue::String(name) => is_empty_string(*name, interner),
        // Why: Int, Float, Bool, Char, Duration, Size, Unit are all Scalar, which
        // bypass RC insertion via the scalar path rather than immortal optimization.
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
