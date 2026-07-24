//! Concrete `Applied`-type resolution for monomorphized instances.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::{Idx, Pool};

/// Register pool resolutions for concrete Applied types produced by monomorphization.
///
/// When a generic struct like `Pair<A, B>` is instantiated as `Pair<int, int>`,
/// the `body_type_map` contains `Applied(Pair, [Var(A), Var(B)]) -> Applied(Pair, [int, int])`.
/// The LLVM `TypeInfoStore` needs to resolve that concrete Applied to a concrete Struct
/// to compute field layout. This function creates those resolutions.
///
/// Handles nested generics: if `Wrapper<T>` is instantiated with `T = Pair<int, bool>`,
/// the concrete struct field `inner: Applied(Pair, [int, bool])` is also registered.
pub(crate) fn register_concrete_applied_resolutions(
    pool: &mut Pool,
    body_type_map: &[(Idx, Idx)],
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    crate::pool::substitute::register_concrete_applied_resolutions(
        pool,
        body_type_map,
        generic_type_params,
    );
}

/// Resolve a single concrete Applied type to its concrete composite body in the
/// pool, covering BOTH `Tag::Struct` and `Tag::Enum`. Delegates to
/// the SSOT `materialize_applied_body` helper in `pool::substitute` (which
/// substitutes the generic field/payload types via the canonical name-keyed
/// walker `substitute_named_in_pool`, interns the concrete `Struct`/`Enum`,
/// records `set_resolution`, and recurses into nested generic fields under an
/// `in_progress` guard). Threads the registry's `name → param names` map and
/// pre-resolves the field/param list at the call site rather than plumbing a
/// `&TypeRegistry` through the generic substitution path.
pub(super) fn resolve_applied_type(
    pool: &mut Pool,
    applied_idx: Idx,
    generic_type_params: &FxHashMap<Name, Vec<Name>>,
) {
    let mut in_progress = rustc_hash::FxHashSet::default();
    crate::pool::substitute::materialize_applied_body(
        pool,
        applied_idx,
        generic_type_params,
        &mut in_progress,
    );
}
