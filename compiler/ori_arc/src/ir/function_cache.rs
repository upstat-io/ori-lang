//! ARC function-cache traversal.

use ori_ir::Name;
use rustc_hash::FxHashMap;

use super::ArcFunction;

/// Flatten an ARC function cache into a single Vec (parents + lambdas).
///
/// The cache uses `FxHashMap`, so callers that require deterministic ordering
/// must sort the returned functions.
#[expect(
    clippy::implicit_hasher,
    reason = "internal function always called with FxHashMap"
)]
pub fn collect_all_arc_functions(
    arc_cache: &FxHashMap<Name, (ArcFunction, Vec<ArcFunction>)>,
) -> Vec<ArcFunction> {
    arc_cache
        .values()
        .flat_map(|(parent, lambdas)| std::iter::once(parent).chain(lambdas.iter()))
        .cloned()
        .collect()
}
