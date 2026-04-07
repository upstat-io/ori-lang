//! Value representation and RC strategy for LLVM emission.
//!
//! [`ValueRepr`] classifies how a value is laid out at the machine level,
//! bridging the gap between ARC's three-way classification ([`ArcClass`])
//! and LLVM's concrete type system. [`RcStrategy`] refines `ValueRepr`
//! for RC operations, encoding exactly how to increment or decrement a
//! value's reference count.
//!
//! Both are computed once and embedded in the ARC IR, eliminating the
//! LLVM emitter's need to re-query the [`Pool`] at every use site.

use ori_types::{Idx, Pool, Tag};

use crate::{ArcClass, ArcClassification};

use super::ArcFunction;

/// Machine-level value representation.
///
/// Determines how a value is passed, stored, and RC-managed in generated
/// code. Each variant maps to a distinct LLVM lowering strategy:
///
/// - [`Scalar`](Self::Scalar) — fits in a register, no RC.
/// - [`RcPointer`](Self::RcPointer) — single heap pointer, RC via `ori_rc_inc`/`ori_rc_dec`.
/// - [`Aggregate`](Self::Aggregate) — multi-field value (tuple, struct, enum, Result, Option).
///   May be stack-allocated or heap-boxed depending on escape analysis.
/// - [`FatValue`](Self::FatValue) — two-word value: `str` (ptr + len) or closure (`fn_ptr` + `env_ptr`).
///   Needs RC on the pointer component but carries inline metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueRepr {
    /// Register-width scalar: int, float, bool, char, byte, unit, etc.
    Scalar,

    /// Single reference-counted heap pointer: list, map, set, channel, iterator.
    RcPointer,

    /// Multi-field aggregate: tuple, struct, enum, Result, Option.
    /// Fields may themselves be `Scalar`, `RcPointer`, `FatValue`, or nested `Aggregate`.
    Aggregate,

    /// Two-word fat value: str (ptr + len) or closure (`fn_ptr` + `env_ptr`).
    /// The pointer component is reference-counted.
    FatValue,
}

impl ValueRepr {
    /// Derive the value representation from an [`ArcClass`] and the type's
    /// [`Pool`] tag.
    ///
    /// For `Scalar` classes, the result is always `Scalar`. For ref-containing
    /// classes (`DefiniteRef`/`PossibleRef`), the Pool tag disambiguates between
    /// pointer, aggregate, and fat-value layouts.
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
            // Fat values: two-word layout (ptr + metadata).
            Tag::Str | Tag::Function => Self::FatValue,

            // Aggregates: multi-field compound types.
            Tag::Tuple | Tag::Struct | Tag::Enum | Tag::Result | Tag::Option => Self::Aggregate,

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
            | Tag::Range
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

/// How to perform an RC operation on a value.
///
/// Computed during RC insertion from [`ValueRepr`] + [`Pool`] structure.
/// Embedded in [`ArcInstr::RcInc`](super::ArcInstr::RcInc) and
/// [`ArcInstr::RcDec`](super::ArcInstr::RcDec) instructions so the LLVM
/// emitter can pattern-match directly — no Pool queries at emission time.
///
/// `ValueRepr` tells you *what* a value is. `RcStrategy` tells you *how*
/// to manage its reference count. The distinction matters within `Aggregate`
/// (struct vs enum) and `FatValue` (string vs closure).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum RcStrategy {
    /// Heap-allocated RC pointer (list, map, set, etc.).
    /// Inc: `ori_rc_inc(data_ptr)`. Dec: `ori_rc_dec(data_ptr, drop_fn)`.
    HeapPointer,

    /// Fat value with RC pointer half (str = `{len, data_ptr}`).
    /// Inc: extract field 1, `ori_rc_inc(data_ptr)`.
    /// Dec: extract field 1, `ori_rc_dec(data_ptr, drop_fn)`.
    FatPointer,

    /// Closure (`{fn_ptr, env_ptr}`). Env may be null (zero captures).
    /// Inc: extract `env_ptr`, null check, `ori_rc_inc(env_ptr)`.
    /// Dec: extract `env_ptr`, null check, load `drop_fn`, `ori_rc_dec(env_ptr, drop_fn)`.
    Closure,

    /// Stack aggregate with RC-typed fields (struct, tuple).
    /// Inc: traverse RC fields, Inc each recursively.
    /// Dec: call generated drop function that traverses fields.
    AggregateFields,

    /// Enum/Result/Option with potentially RC-typed variant payloads.
    /// Inc: **no-op** (stack-allocated container; inner fields managed at Dec).
    /// Dec: tag-switch, per-variant field Dec.
    InlineEnum,

    /// Iterator handle (`Tag::Iterator` / `Tag::DoubleEndedIterator`).
    ///
    /// Box-allocated state with no RC header. Iterators are effectively
    /// unique-owned (they are moved through `iter_next`, never copied),
    /// so `Inc` is a no-op. `Dec` calls `ori_iter_drop(ptr)` directly,
    /// bypassing the `ori_rc_dec` path which would dereference a
    /// non-existent refcount header.
    ///
    /// Spec: TPR-07-008 — previously iterators were classified as
    /// `Scalar` (no cleanup emitted at all), leaking the Box-allocated
    /// state in any container that held an unconsumed iterator.
    Iterator,
}

impl RcStrategy {
    /// Compute from a variable's [`ValueRepr`] and its [`Pool`] type.
    ///
    /// Called once during RC insertion; the result is embedded in the
    /// `RcInc`/`RcDec` instruction and never recomputed.
    ///
    /// # Panics
    ///
    /// Debug-panics if called on a `Scalar` repr (scalars never get RC ops).
    pub fn from_var(repr: ValueRepr, pool: &Pool, ty: Idx) -> Self {
        match repr {
            ValueRepr::Scalar => {
                debug_assert!(false, "RcStrategy::from_var called on Scalar repr");
                // Fallback for release builds: treat as heap pointer (conservative).
                Self::HeapPointer
            }
            ValueRepr::RcPointer => {
                // TPR-07-008: iterators map to `ValueRepr::RcPointer` but
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

#[cfg(test)]
mod tests;
