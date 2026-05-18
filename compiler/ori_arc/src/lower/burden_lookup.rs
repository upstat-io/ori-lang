//! Phase 5 burden lookup — bridges builtin (`BURDEN_TABLE`) and user-defined
//! (`TypeRegistry`) burden sides behind a single dispatch surface.
//!
//! Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding the
//! lattice-driven analysis. The builtin-vs-user partition is a purity boundary
//! (`ori_registry` is heap-free; `ori_types` is heap-backed) — `lookup_burden`
//! hides it so Phase 5 emission consumes burden specs uniformly via the
//! `Burden` trait on `BurdenRef`.

use ori_registry::burden::table::{burden_type_id, BurdenRegistry, TYPE_PARAM_E};
use ori_registry::TypeTag;
use ori_types::{Idx, TypeRegistry};

use super::burden::{BurdenRef, TypeRef};

/// Reserved sentinel raw value for `TYPE_PARAM_E` (`u32::MAX - 1`). User pool
/// indices MUST stay strictly below this to avoid collision when
/// boundary-translating into the `BurdenTypeId` namespace. Extracted from the
/// shared SSOT constant in `ori_registry::burden::table` rather than mirrored
/// as a literal.
const TYPE_PARAM_E_RAW: u32 = TYPE_PARAM_E.get().get();

/// Returns the burden spec for `ty`, dispatching across the builtin
/// (`BURDEN_TABLE`) and user (`TypeRegistry`) partitions.
///
/// - `TypeRef::Builtin(type_id)` → `BurdenRegistry::lookup_builtin(type_id)`,
///   yielding `BurdenRef::Builtin(&'static BuiltinBurdenSpec)`. The
///   `&'static` borrow widens transparently to any caller-requested `'a`.
/// - `TypeRef::User(idx)` → `type_registry.burden(idx)`, yielding
///   `BurdenRef::User(&'a UserBurdenSpec)` tied to `type_registry`'s borrow.
///
/// Returns `None` when the type carries no burden — empty / scalar / opaque
/// FFI per proposal Q12 (caller-managed lifetime; Ori emits no drops).
#[must_use]
pub fn lookup_burden(ty: TypeRef, type_registry: &TypeRegistry) -> Option<BurdenRef<'_>> {
    match ty {
        TypeRef::Builtin(type_id) => {
            BurdenRegistry::lookup_builtin(type_id).map(BurdenRef::Builtin)
        }
        TypeRef::User(idx) => {
            // `Idx::NONE` (u32::MAX) is the "no type information" sentinel
            // used by test fixtures and untyped placeholder slots. It carries
            // no burden by construction; treat as "no burden" rather than
            // tripping the TYPE_PARAM debug_assert.
            if idx.is_none() {
                return None;
            }
            debug_assert!(
                idx.raw() < TYPE_PARAM_E_RAW,
                "user type pool would collide with TYPE_PARAM sentinel space \
                 (idx={}, TYPE_PARAM_E={})",
                idx.raw(),
                TYPE_PARAM_E_RAW,
            );
            type_registry.burden(idx).map(BurdenRef::User)
        }
    }
}

/// Classify a raw `Idx` into a `TypeRef` for `lookup_burden` dispatch.
///
/// Pre-interned primitive `Idx` values (per `types.md §TY-5` Appendix A) map to
/// `TypeRef::Builtin(burden_type_id(tag))` via the `Idx → TypeTag` translation
/// chain. The two namespaces have DIFFERENT orderings (e.g., `Idx::STR` is at
/// raw 3 but `TypeTag::Str` is at discriminant 10) — naive raw-value punning
/// would silently miscompile by dispatching the wrong burden entry. The match
/// below performs the explicit per-primitive translation.
///
/// Non-primitive `Idx` values (everything `>= FIRST_DYNAMIC = 64` plus the
/// `Idx::ERROR` slot at raw 8) fall through to `TypeRef::User(idx)` for
/// `TypeRegistry::burden(idx)` lookup.
#[must_use]
pub fn idx_to_type_ref(idx: Idx, _type_registry: &TypeRegistry) -> TypeRef {
    if !idx.is_primitive() {
        return TypeRef::User(idx);
    }
    let tag = match idx {
        Idx::INT => TypeTag::Int,
        Idx::FLOAT => TypeTag::Float,
        Idx::BOOL => TypeTag::Bool,
        Idx::STR => TypeTag::Str,
        Idx::CHAR => TypeTag::Char,
        Idx::BYTE => TypeTag::Byte,
        Idx::UNIT => TypeTag::Unit,
        Idx::NEVER => TypeTag::Never,
        Idx::DURATION => TypeTag::Duration,
        Idx::SIZE => TypeTag::Size,
        Idx::ORDERING => TypeTag::Ordering,
        // Idx::ERROR (raw 8) is poison per types.md §TK-3 — no burden entry;
        // reserved primitive slots 12..=63 (per types.md §TY-5) likewise.
        _ => return TypeRef::User(idx),
    };
    TypeRef::Builtin(burden_type_id(tag))
}

#[cfg(test)]
mod tests;
