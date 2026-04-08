//! Transitive triviality classification for type-level ARC elision.
//!
//! A type is *trivial* when it (and all its transitive children) contain
//! no heap references requiring ARC operations. Trivial types can skip
//! all `ori_rc_inc`/`ori_rc_dec` calls in generated code.
//!
//! Single source of truth: both `ori_arc::ArcClassifier` and
//! `ori_llvm::TypeInfoStore` delegate to [`classify_triviality`].
//!
//! # Classification Rules
//!
//! - **Trivial**: primitives (`int`, `float`, `bool`, `char`, `byte`, `void`,
//!   `Never`, `Duration`, `Size`, `Ordering`), `Error` sentinel, and
//!   compound types whose transitive children are all trivial
//! - **`NonTrivial`**: heap-allocated with RC (`str`, `[T]`, `{K: V}`, `Set<T>`,
//!   `Channel<T>`), closures (`Function`), iterators (Box-allocated — no RC
//!   header, but `ori_iter_drop` must still run at scope exit), recursive
//!   types, and compounds containing any non-trivial child
//! - **`Unknown`**: unresolved type variables, unresolvable named types, and
//!   internal compiler types that should not reach codegen

use rustc_hash::FxHashSet;

use crate::{Idx, Pool, Tag};

/// Triviality classification for a type in the Pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Triviality {
    /// No heap references anywhere in the type tree.
    Trivial,
    /// Contains at least one heap reference (str, [T], etc.).
    NonTrivial,
    /// Contains unresolved type variables — must assume non-trivial.
    Unknown,
}

/// Classify whether `idx` is trivial (no ARC needed) in the given Pool.
///
/// Returns [`Triviality::Trivial`] when the type and all its transitive
/// children are scalar (no heap references). This is the single source of
/// truth for triviality — all consumers (`ori_arc::ArcClassifier`,
/// `ori_repr::ReprPlan`, `ori_llvm::TypeInfoStore`) must delegate here.
pub fn classify_triviality(idx: Idx, pool: &Pool) -> Triviality {
    // Sentinel: Idx::NONE is used as a placeholder in various data structures.
    // ArcClassifier treats it as Scalar; we match that behavior.
    if idx.is_none() {
        return Triviality::Trivial;
    }

    // Guard: out-of-bounds indices cannot be classified.
    if idx.raw() as usize >= pool.len() {
        return Triviality::Unknown;
    }

    let mut visiting = FxHashSet::default();
    classify_recursive(idx, pool, &mut visiting)
}

/// Recursive classification with cycle detection.
///
/// Uses `visiting` set to detect recursive types (which must be
/// heap-allocated and are therefore non-trivial).
fn classify_recursive(idx: Idx, pool: &Pool, visiting: &mut FxHashSet<Idx>) -> Triviality {
    let resolved = pool.resolve_fully(idx);
    let tag = pool.tag(resolved);

    // Fast path: primitives and known leaf types
    match tag {
        // Scalar primitives — no heap allocation.
        // Error placeholder: propagates silently, classified as Scalar
        // by ArcClassifier (Idx::ERROR is a pre-interned primitive).
        Tag::Int
        | Tag::Float
        | Tag::Bool
        | Tag::Char
        | Tag::Byte
        | Tag::Unit
        | Tag::Never
        | Tag::Duration
        | Tag::Size
        | Tag::Ordering
        | Tag::Error => return Triviality::Trivial,

        // Always heap-allocated with RC headers.
        //
        // Iterators (Box-allocated, no RC header) are non-trivial because
        // `ori_iter_drop` must run at scope exit — even though they have
        // no refcount to decrement. The ARC emitter routes them through
        // `RcStrategy::Iterator` / the `Tag::Iterator` arm of
        // `dec_value_rc_inner`, which emits `ori_iter_drop` instead of
        // `ori_rc_dec`. See
        Tag::Str
        | Tag::List
        | Tag::Map
        | Tag::Set
        | Tag::Channel
        | Tag::Iterator
        | Tag::DoubleEndedIterator => return Triviality::NonTrivial,

        // Unresolved type variables, reserved types, internal compiler types
        // — conservative fallback
        Tag::Var
        | Tag::BoundVar
        | Tag::RigidVar
        | Tag::Borrowed
        | Tag::Scheme
        | Tag::Projection
        | Tag::ModuleNs
        | Tag::Infer
        | Tag::SelfType => return Triviality::Unknown,

        // Named/Applied/Alias: resolve and re-classify.
        // Newtypes (e.g., `type UserId = int`) use Tag::Named and
        // resolve_fully() follows the chain transparently.
        Tag::Named | Tag::Applied | Tag::Alias => {
            let inner = pool.resolve_fully(resolved);
            if inner == resolved {
                return Triviality::Unknown; // unresolvable
            }
            return classify_recursive(inner, pool, visiting);
        }

        _ => {} // compound types — fall through to recurse
    }

    // Cycle detection: if we're already visiting this type, it's recursive
    // and must be heap-allocated (requires indirection) → non-trivial.
    if !visiting.insert(resolved) {
        return Triviality::NonTrivial;
    }

    let result = match tag {
        Tag::Option => classify_recursive(pool.option_inner(resolved), pool, visiting),
        Tag::Result => {
            let ok = classify_recursive(pool.result_ok(resolved), pool, visiting);
            let err = classify_recursive(pool.result_err(resolved), pool, visiting);
            merge_triviality(ok, err)
        }
        Tag::Tuple => {
            let elems = pool.tuple_elems(resolved);
            let mut result = Triviality::Trivial;
            for elem in &elems {
                result = merge_triviality(result, classify_recursive(*elem, pool, visiting));
                if result == Triviality::NonTrivial {
                    break;
                }
            }
            result
        }
        Tag::Struct => {
            let fields = pool.struct_fields(resolved);
            let mut result = Triviality::Trivial;
            for (_, field_ty) in &fields {
                result = merge_triviality(result, classify_recursive(*field_ty, pool, visiting));
                if result == Triviality::NonTrivial {
                    break;
                }
            }
            result
        }
        Tag::Enum => {
            let variants = pool.enum_variants(resolved);
            let mut result = Triviality::Trivial;
            'outer: for (_, field_types) in &variants {
                for field_ty in field_types {
                    result =
                        merge_triviality(result, classify_recursive(*field_ty, pool, visiting));
                    if result == Triviality::NonTrivial {
                        break 'outer;
                    }
                }
            }
            result
        }
        // Closures capture heap refs — always non-trivial
        Tag::Function => Triviality::NonTrivial,
        // Range element determines triviality
        Tag::Range => classify_recursive(pool.range_elem(resolved), pool, visiting),
        // All other tags handled above; unreachable after exhaustive early returns.
        _ => {
            debug_assert!(
                false,
                "unexpected tag in triviality classification: {tag:?}"
            );
            Triviality::Unknown
        }
    };

    visiting.remove(&resolved);
    result
}

/// Lattice merge: `NonTrivial` > `Unknown` > `Trivial`.
fn merge_triviality(a: Triviality, b: Triviality) -> Triviality {
    match (a, b) {
        (Triviality::NonTrivial, _) | (_, Triviality::NonTrivial) => Triviality::NonTrivial,
        (Triviality::Unknown, _) | (_, Triviality::Unknown) => Triviality::Unknown,
        _ => Triviality::Trivial,
    }
}

#[cfg(test)]
mod tests;
