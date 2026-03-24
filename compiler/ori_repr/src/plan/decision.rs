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
    /// §02: Transitive triviality analysis.
    Triviality,
    /// §03/§04: Value range → integer narrowing.
    IntegerNarrowing,
    /// §03/§05: Precision analysis → float narrowing.
    FloatNarrowing,
    /// §06: Struct field reordering.
    StructLayout,
    /// §07: Enum niche/discriminant.
    EnumRepr,
    /// §08: Escape analysis.
    EscapeAnalysis,
    /// §09: ARC header compression.
    ArcHeader,
    /// §10: Thread-local ARC.
    ThreadLocal,
    /// §11: Collection specialization.
    CollectionSpec,
}

/// Reason for a narrowing decision — used in audit trail and debug tracing.
///
/// `ValueRange` is a placeholder in §01 (replaced by the real interval
/// lattice in §03).
#[derive(Debug, Clone)]
pub enum DecisionReason {
    /// Type is canonically this width (no narrowing applied).
    Canonical,
    /// Value range fits in a narrower type.
    RangeFits {
        /// The computed value range from §03.
        range: ValueRange,
        /// The narrowest `IntWidth` that covers the range.
        min_width: IntWidth,
    },
    /// All fields are trivial — no RC needed.
    TransitivelyTrivial,
    /// Value never escapes function scope (from §08).
    DoesNotEscape,
    /// Sharing bound is within RC width (from §09).
    BoundedSharing {
        /// Maximum number of simultaneous references.
        max_refs: u32,
    },
    /// Niche available in field (from §07).
    NicheAvailable {
        /// Field index containing the niche.
        field: u32,
        /// Niche value (invalid bit pattern used as discriminant).
        niche: u64,
    },
    /// Custom reason (for tracing).
    Custom(String),
}
