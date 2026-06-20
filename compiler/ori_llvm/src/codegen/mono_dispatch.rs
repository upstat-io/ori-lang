//! Shared mono-dispatch discriminators for the two call-target resolution
//! layers: ARC-IR rewrite (`function_compiler::nounwind::prepare`) and LLVM
//! emission (`arc_emitter::apply`).
//!
//! The name+arg-types mono fallback (taken when a call site carries no precise
//! `mono_instance_id`) cannot, on its own, tell a builtin method dispatch apart
//! from an enclosing free function that shares the method's interned name. ARC
//! IR is provenance-erased: `left.compare(other:)` and a free-fn self-call
//! lower to the same `Apply { func, args }`. `callee_shadows_builtin_method`
//! reconstructs the erased provenance from `ori_registry` — when the receiver
//! type carries a builtin method named `callee`, the `Apply` is a builtin-method
//! dispatch (emission belongs to `try_emit_builtin_method`), not a
//! free-function self-call, so the self-targeting mono candidate is skipped.

use ori_arc::{ArcFunction, ArcVarId};
use ori_ir::{Name, StringInterner};
use ori_types::Pool;

/// True when `callee` names a builtin method on the receiver type (`args[0]`).
/// Empty `args` (no receiver) or a non-builtin receiver type yield `false`.
#[must_use]
pub(crate) fn callee_shadows_builtin_method(
    pool: &Pool,
    interner: &StringInterner,
    callee: Name,
    args: &[ArcVarId],
    func: &ArcFunction,
) -> bool {
    let Some(&receiver) = args.first() else {
        return false;
    };
    let recv_ty = pool.resolve_fully(func.var_type(receiver));
    let Some(tag) = pool.builtin_type_tag(recv_ty) else {
        return false;
    };
    ori_registry::has_method(tag, interner.lookup(callee))
}
