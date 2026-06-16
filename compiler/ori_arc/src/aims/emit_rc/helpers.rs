//! Shared RC-emission helper predicates.

use crate::aims::intraprocedural::state_map::{AimsStateMap, ApplyAliasSource};
use crate::ir::ArcVarId;

/// Whether the scope-exit `RcDec` for `var` should be suppressed because
/// `var` was consumed by an Apply/Invoke whose dst aliases `var` (caller-side
/// ownership transfer detected via `apply_result_aliases`).
///
/// Conditions for suppression:
/// 1. The block is NOT an unwind block (unwind paths always emit cleanup
///    decs per RL-4). Caller passes `is_unwind_succ` from explicit Invoke
///    unwind-successor distinction OR from inline Resume detection on the
///    successor block.
/// 2. `var` appears as a consumed-arg source in some entry of
///    `state_map.apply_result_aliases`:
///    - `Direct(arg)` with `arg == var`
///    - `Project { arg, .. }` with `arg == var`
///    - `Wrapped(arg)` with `arg == var`
///    - `Conditional { candidates }` with `var ∈ candidates`
///
/// Reverse lookup is acceptable: `apply_result_aliases` is sparse (entries
/// only when callees transfer ownership through return AND the consumed arg
/// is a non-Let-alias root), so the linear scan is bounded by the small
/// number of in-flight ownership-transfer Apply sites in the function.
pub(crate) fn should_suppress_apply_aliased_dec(
    state_map: &AimsStateMap,
    var: ArcVarId,
    is_unwind_block: bool,
) -> bool {
    if is_unwind_block {
        return false;
    }
    state_map
        .apply_result_aliases()
        .values()
        .any(|source| match source {
            // Wrapped behaves like Direct/Project for dec-suppression
            // purposes (suppress arg's caller-side canonical dec because
            // arg's ownership transferred into dst's payload via the
            // wrapping construct). The class-union semantic differs
            // (Wrapped does NOT union per `ssa_alias_classes.rs`); only
            // the suppression trigger fires.
            ApplyAliasSource::Direct(arg)
            | ApplyAliasSource::Project { arg, .. }
            | ApplyAliasSource::Wrapped(arg) => *arg == var,
            ApplyAliasSource::Conditional { candidates } => candidates.contains(&var),
        })
}
