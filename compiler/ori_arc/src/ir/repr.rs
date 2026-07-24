//! Executable value-shape and RC-adapter classifications.
//!
//! [`ValueRepr`] refines ARC's three-way classification ([`ArcClass`]) into
//! ownership-relevant shapes carried by the executable plan. [`RcStrategy`]
//! selects compiled adapter behavior. Pointer, fat-value, and atomicity choices
//! describe that physical adapter rather than backend-neutral AIMS facts.
//!
//! Both values are computed once and embedded in ARC IR, preventing repeated
//! [`Pool`] queries. No physical projection may re-derive AIMS policy from
//! these compiled shapes.

use ori_types::{Idx, Pool, Tag};

use crate::{ArcClass, ArcClassification};

use super::ArcFunction;

/// Ownership-relevant executable value shape.
///
/// Records the compiled shape embedded in ARC IR. Pointer-oriented variants
/// do not require every physical projection to use the same storage encoding.
///
/// - [`Scalar`](Self::Scalar) — scalar semantics, no RC.
/// - [`RcPointer`](Self::RcPointer) — one RC-managed reference.
/// - [`Aggregate`](Self::Aggregate) — compound fields (tuple, struct, enum,
///   `Result`, `Option`).
/// - [`FatValue`](Self::FatValue) — an RC-managed reference plus inline
///   metadata, such as a string length or closure entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueRepr {
    /// Scalar with no RC burden: int, float, bool, char, byte, unit, etc.
    Scalar,

    /// Single reference-counted handle: list, map, set, channel, iterator.
    RcPointer,

    /// Multi-field aggregate: tuple, struct, enum, Result, Option.
    /// Fields may themselves be `Scalar`, `RcPointer`, `FatValue`, or nested `Aggregate`.
    Aggregate,

    /// Reference-bearing value with inline metadata: string or closure.
    /// The reference component is reference-counted; each physical projection
    /// chooses its concrete field and slot encoding.
    FatValue,
}

impl ValueRepr {
    /// Derive the value representation from an [`ArcClass`] and the type's
    /// [`Pool`] tag.
    ///
    /// For `Scalar` classes, the result is always `Scalar`. For ref-containing
    /// classes (`DefiniteRef`/`PossibleRef`), the Pool tag disambiguates between
    /// reference, aggregate, and reference-plus-metadata shapes.
    pub fn from_arc_class(class: ArcClass, pool: &Pool, idx: Idx) -> Self {
        match class {
            ArcClass::Scalar => Self::Scalar,
            ArcClass::DefiniteRef | ArcClass::PossibleRef => Self::from_ref_tag(pool, idx),
        }
    }

    /// Classify a ref-containing type by its Pool tag.
    fn from_ref_tag(pool: &Pool, idx: Idx) -> Self {
        // Resolve through type aliases/applications to the concrete tag.
        let resolved = pool.resolve_fully(idx);

        // Sentinel: unresolvable → conservative RcPointer.
        if resolved == Idx::NONE {
            return Self::RcPointer;
        }

        match pool.tag(resolved) {
            // Reference-bearing values with inline metadata.
            Tag::Str | Tag::Function => Self::FatValue,

            // INVARIANT: Range is an inline scalar aggregate; it never carries one heap identity.
            Tag::Tuple | Tag::Struct | Tag::Enum | Tag::Result | Tag::Option | Tag::Range => {
                Self::Aggregate
            }

            // Primitives: shouldn't reach here (ArcClass would be Scalar),
            // but handle gracefully.
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => Self::Scalar,

            // Single heap pointer (collections, iterators), unresolved names,
            // type variables, and other conservative cases.
            Tag::List
            | Tag::Map
            | Tag::Set
            | Tag::Channel
            | Tag::Iterator
            | Tag::DoubleEndedIterator
            | Tag::Named
            | Tag::Applied
            | Tag::Alias
            | Tag::Var
            | Tag::BoundVar
            | Tag::RigidVar
            | Tag::Borrowed
            | Tag::Scheme
            | Tag::Projection
            | Tag::ModuleNs
            | Tag::Infer
            | Tag::SelfType => Self::RcPointer,
        }
    }
}

/// Transitional adapter classification for the shipped RC operation carriers.
///
/// The current pipeline derives this value from [`ValueRepr`] and [`Pool`]
/// structure, then embeds it in [`ArcInstr::RcInc`](super::ArcInstr::RcInc) and
/// [`ArcInstr::RcDec`](super::ArcInstr::RcDec). That prevents downstream Pool
/// queries today, but the enum's physical names are not AIMS facts. Production
/// binds each logical event to stable value-semantics and cleanup-plan IDs;
/// `VmLayoutPlan` and `CompiledLayoutPlan` then select a satisfying mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum RcStrategy {
    /// Legacy adapter name for one self-owned identity (list, map, set, etc.).
    /// The shipped compiled runtime normally realizes that identity as a heap
    /// reference handled by `ori_rc_inc` / `ori_rc_dec`.
    HeapPointer,

    /// Metadata-bearing value with one RC-managed reference (for example,
    /// string data plus length). Inc/Dec target the reference component; its
    /// physical field location belongs to the projection.
    FatPointer,

    /// Closure with an optional captured environment. Inc/Dec target the
    /// environment when present; the entry-point/environment encoding and
    /// drop implementation belong to the physical projection.
    Closure,

    /// Product with ownership-bearing fields (struct, tuple). The transitional
    /// adapter traverses those logical fields; inline storage is not an AIMS fact.
    AggregateFields,

    /// Sum with potentially ownership-bearing variant payloads. The
    /// transitional adapter selects the active logical variant and processes
    /// its fields; tag and payload layout remain projection-owned.
    InlineEnum,

    /// Iterator handle (`Tag::Iterator` / `Tag::DoubleEndedIterator`).
    ///
    /// Iterators are affine under the current semantic contract: they move
    /// through `iter_next` and cannot acquire another owner. The shipped
    /// compiled adapter uses boxed state without an RC header, so `Inc` is a
    /// no-op and `Dec` calls `ori_iter_drop(ptr)` directly.
    Iterator,

    /// Scalar-repr value whose type carries a user `@drop`.
    ///
    /// The type declares a `Drop` impl despite carrying no shared-identity
    /// credit. Its logical cleanup invokes `@drop` exactly once without a
    /// retain operation. The current compiled projection supplies any inline
    /// passing and unwind mechanics.
    /// Spec: Annex E §AIMS RL-DROP (`RLDROP_scalar_lifecycle_sound`).
    UserDrop,
}

/// Transitional physical arithmetic choice on the shipped RC carrier.
///
/// Carried on [`ArcInstr::RcInc`](super::ArcInstr::RcInc) and
/// [`ArcInstr::RcDec`](super::ArcInstr::RcDec) alongside [`RcStrategy`].
/// Every current construction site uses `Atomic` because the shipped compiled
/// runtime primitives are unconditionally atomic. AIMS freezes neutral thread
/// reachability; it does not choose this enum. Production moves the mechanism
/// into the selected physical plan and removes `RcAtomicity` from shared ARC IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum RcAtomicity {
    /// Atomic refcount arithmetic (`fetch_add`/`fetch_sub`) in the shipped
    /// compiled projection. This is the only value currently emitted.
    Atomic,

    /// Non-atomic refcount arithmetic (plain load/store). Not selected by any
    /// current construction site; a physical plan may admit it only when its
    /// capability satisfies the frozen thread-reachability fact.
    NonAtomic,
}

impl RcAtomicity {
    /// The compatibility default for the shipped compiled runtime.
    ///
    /// Returns [`RcAtomicity::Atomic`] — every realization-emitted `RcInc`/
    /// `RcDec` is atomic, reproducing the shipped unconditionally-atomic
    /// runtime RC primitives bit-for-bit. Production physical planners replace
    /// this shared default with a validated mechanism choice.
    #[must_use]
    #[inline]
    pub const fn default_atomic() -> Self {
        Self::Atomic
    }

    /// Whether this atomicity selects atomic refcount arithmetic.
    #[must_use]
    #[inline]
    pub const fn is_atomic(self) -> bool {
        matches!(self, Self::Atomic)
    }
}

impl RcStrategy {
    /// Compute from a variable's [`ValueRepr`] and its [`Pool`] type.
    ///
    /// Called once by the transitional carrier builder; the result is embedded
    /// in `RcInc`/`RcDec` and never recomputed by a physical consumer.
    ///
    /// # Panics
    ///
    /// Debug-panics if called on a `Scalar` repr (scalars never get RC ops).
    pub fn from_repr(repr: ValueRepr, pool: &Pool, ty: Idx) -> Self {
        match repr {
            ValueRepr::Scalar => {
                panic!("RcStrategy::from_repr cannot classify a scalar representation")
            }
            ValueRepr::RcPointer => {
                // iterators map to `ValueRepr::RcPointer` but
                // need `ori_iter_drop`, not `ori_rc_dec`. Route them
                // through the dedicated `Iterator` strategy before
                // defaulting to `HeapPointer`.
                let resolved = pool.resolve_fully(ty);
                match pool.tag(resolved) {
                    Tag::Iterator | Tag::DoubleEndedIterator => Self::Iterator,
                    _ => Self::HeapPointer,
                }
            }
            ValueRepr::FatValue => {
                let resolved = pool.resolve_fully(ty);
                if pool.tag(resolved) == Tag::Function {
                    Self::Closure
                } else {
                    Self::FatPointer
                }
            }
            ValueRepr::Aggregate => {
                let resolved = pool.resolve_fully(ty);
                match pool.tag(resolved) {
                    Tag::Result | Tag::Enum | Tag::Option => Self::InlineEnum,
                    _ => Self::AggregateFields,
                }
            }
        }
    }
}

/// Whether an [`RcStrategy`]'s drop walks its payload, transitively decrementing
/// inner RC slots.
///
/// Returns `true` for strategies whose drop function dec's payload allocations
/// (`Closure` walks captured env, `AggregateFields` walks RC fields, `InlineEnum`
/// switches on tag and dec's variant payloads, `HeapPointer` runs `elem_dec_fn`
/// over collection elements). Returns `false` for `FatPointer` (str — drop dec's
/// the data buffer ONLY, the `FatPointer` IS the leaf payload), `Iterator` (drop
/// calls `ori_iter_drop` directly without payload-walk), and `UserDrop` (scalar
/// value — the `@drop` call manages no transitive RC payload).
///
/// Pure function on the enum — no Pool query, no `AimsStateMap` query. The
/// `RcStrategy` value already carries the answer.
///
/// Used by AIMS PIN-6 for inter-class payload-of suppression: when
/// class A's allocation is `[own]`-consumed by an instruction constructing a
/// parent aggregate of class B, and class B's strategy is transitive-drop,
/// class B's drop covers class A's RC slot — class A's canonical dec is
/// suppressed to avoid double-free.
#[must_use]
#[inline]
pub fn is_transitive_drop_strategy(strategy: RcStrategy) -> bool {
    matches!(
        strategy,
        RcStrategy::Closure
            | RcStrategy::AggregateFields
            | RcStrategy::InlineEnum
            | RcStrategy::HeapPointer
    )
}

/// Compute value representations for all variables in a function.
///
/// Produces a parallel array indexed by [`ArcVarId::index()`](super::ArcVarId::index),
/// matching `func.var_types` element-for-element. Each entry is derived from the
/// variable's [`ArcClass`] (via the classifier) and its [`Pool`] tag.
///
/// Called once at the start of the ARC pipeline, after lowering and before
/// any optimization passes.
pub fn compute_var_reprs(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    pool: &Pool,
) -> Vec<ValueRepr> {
    func.var_types
        .iter()
        .map(|&ty| {
            let class = classifier.arc_class(ty);
            ValueRepr::from_arc_class(class, pool, ty)
        })
        .collect()
}

/// Compute cached RC strategies for all variables in a function.
///
/// Produces a parallel array indexed by [`ArcVarId::index()`](super::ArcVarId::index),
/// matching `func.var_reprs` element-for-element. Each entry is `Some(strategy)`
/// for non-scalar variables (heap pointers, fat values, aggregates, closures,
/// inline enums, iterators) and `None` for scalars.
///
/// Called once at the start of the ARC pipeline, immediately after
/// [`compute_var_reprs`] populates `func.var_reprs`. Caches the
/// [`RcStrategy::from_repr`] result so downstream pre-walk passes (the SSA
/// alias-class computation in `intraprocedural::ssa_alias_classes` + the
/// transitive-drop edge materialization in `intraprocedural::post_convergence`)
/// can classify a variable's strategy without holding a `&Pool` reference at
/// analyze-time.
///
/// # Preconditions
///
/// `func.var_reprs` MUST be ready (length equal to `func.var_types`). Calling
/// this while metadata is explicitly unrealized returns an empty vector.
#[must_use]
pub fn compute_var_rc_strategies(func: &ArcFunction, pool: &Pool) -> Vec<Option<RcStrategy>> {
    if func.var_metadata_state == super::VariableMetadataState::Unrealized {
        return Vec::new();
    }
    assert_eq!(
        func.var_reprs.len(),
        func.var_types.len(),
        "var_reprs and var_types length mismatch"
    );
    derive_var_rc_strategies(&func.var_reprs, &func.var_types, pool)
}

pub(crate) fn derive_var_rc_strategies(
    representations: &[ValueRepr],
    types: &[Idx],
    pool: &Pool,
) -> Vec<Option<RcStrategy>> {
    assert_eq!(
        representations.len(),
        types.len(),
        "representation and type tables must have identical lengths"
    );
    representations
        .iter()
        .zip(types)
        .map(|(&repr, &ty)| {
            if repr == ValueRepr::Scalar {
                None
            } else {
                Some(RcStrategy::from_repr(repr, pool, ty))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
