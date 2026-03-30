//! Decision tracking types for the representation optimization pipeline.
//!
//! Each narrowing decision records its source pass, the affected type,
//! the chosen representation, and the justification — forming a
//! complete audit trail that can be dumped for debugging.

use ori_types::Idx;

use crate::range::ValueRange;
use crate::repr::{IntWidth, MachineRepr};

/// A single narrowing decision recorded in the `ReprPlan`.
#[derive(Debug, Clone)]
pub struct ReprDecision {
    /// Which analysis pass made this decision.
    pub source: DecisionSource,
    /// The semantic type this applies to.
    pub type_idx: Idx,
    /// The chosen machine representation.
    pub repr: MachineRepr,
    /// Why this representation was chosen (for tracing).
    pub reason: DecisionReason,
}

/// Which optimization pass produced a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecisionSource {
    /// Default: canonical representation (no optimization).
    Canonical,
    /// Transitive triviality analysis.
    Triviality,
    /// Value range analysis → integer narrowing.
    IntegerNarrowing,
    /// Precision analysis → float narrowing.
    FloatNarrowing,
    /// Struct field reordering.
    StructLayout,
    /// Enum niche/discriminant.
    EnumRepr,
    /// Escape analysis.
    EscapeAnalysis,
    /// ARC header compression.
    ArcHeader,
    /// Thread-local ARC.
    ThreadLocal,
    /// Collection specialization.
    CollectionSpec,
}

/// Reason for a narrowing decision — used in audit trail and debug tracing.
///
/// `ValueRange` is a placeholder populated by the canonical pass (replaced by
/// the real interval lattice in the range analysis pass).
#[derive(Debug, Clone)]
pub enum DecisionReason {
    /// Type is canonically this width (no narrowing applied).
    Canonical,
    /// Value range fits in a narrower type.
    RangeFits {
        /// The computed value range from range analysis.
        range: ValueRange,
        /// The narrowest `IntWidth` that covers the range.
        min_width: IntWidth,
    },
    /// All fields are trivial — no RC needed.
    TransitivelyTrivial,
    /// Value never escapes function scope (from escape analysis).
    DoesNotEscape,
    /// Sharing bound is within RC width (from ARC header compression).
    BoundedSharing {
        /// Maximum number of simultaneous references.
        max_refs: u32,
    },
    /// Niche available in field (from enum repr pass).
    NicheAvailable {
        /// Field index containing the niche.
        field: u32,
        /// Niche value (invalid bit pattern used as discriminant).
        niche: u64,
    },
    /// Custom reason (for tracing).
    Custom(String),
}
