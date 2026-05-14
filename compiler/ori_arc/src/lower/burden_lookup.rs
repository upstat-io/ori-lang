//! Phase 5 burden lookup — bridges builtin (`BURDEN_TABLE`) and user-defined
//! (`TypeRegistry`) burden sides behind a single dispatch surface.
//!
//! Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding the
//! lattice-driven analysis. The builtin-vs-user partition is a purity boundary
//! (`ori_registry` is heap-free; `ori_types` is heap-backed) — `lookup_burden`
//! hides it so Phase 5 emission consumes burden specs uniformly via the
//! `Burden` trait on `BurdenRef`.

use ori_registry::burden::table::{BurdenRegistry, TYPE_PARAM_E};
use ori_types::TypeRegistry;

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

#[cfg(test)]
mod tests;
