//! Shared ARC IR lowering utilities.
//!
//! Provides the common per-function lowering step used by both the AOT path
//! (`compile_common.rs`) and the JIT test runner (`runner/mod.rs`). Both paths
//! need to: build a params Vec, look up the canon body root, and call
//! `ori_arc::lower_function_can`. This module eliminates that duplication.

use ori_arc::ArcFunction;
use ori_ir::canon::CanonResult;
use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, Idx, Pool};
use rustc_hash::FxHashMap;

/// Lower a single function to ARC IR.
///
/// Handles the common pattern shared across module functions, imported
/// functions, impl methods, and monomorphized functions:
/// 1. Build `(Name, Idx)` param list from the signature
/// 2. Resolve the canonical body root
/// 3. Call `lower_function_can` with the correct `is_fbip` flag
///
/// `body_name` is the name used for `canon.root_for()` lookup — usually the
/// same as `name`, except for monomorphized functions where `name` is the
/// mangled specialization and `body_name` is the original generic function.
#[expect(
    clippy::too_many_arguments,
    reason = "thin wrapper over lower_function_can — params mirror the underlying API"
)]
#[expect(
    clippy::implicit_hasher,
    reason = "downstream lower_function_can requires concrete FxHashMap — cannot generalize"
)]
pub fn lower_to_arc(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_to_arc_impl(
        name,
        sig,
        body_name,
        None,
        canon,
        interner,
        pool,
        arc_problems,
        type_subst,
    )
}

/// Lower a single impl method with ordinal-aware body lookup.
///
/// For types with multiple impls defining the same method name (e.g.,
/// `impl Index<int, V>` and `impl Index<str, V>`), `ordinal` selects
/// which body to use via `canon.method_root_for_nth()` (TPR-03-051).
#[expect(
    clippy::too_many_arguments,
    reason = "thin wrapper over lower_function_can — params mirror the underlying API"
)]
#[expect(
    clippy::implicit_hasher,
    reason = "downstream lower_function_can requires concrete FxHashMap — cannot generalize"
)]
pub fn lower_impl_method_to_arc_nth(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Name,
    ordinal: usize,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let params: Vec<(Name, Idx)> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&n, &t)| (n, t))
        .collect();
    let body_id = canon
        .method_root_for_nth(type_name, body_name, ordinal)
        .or_else(|| canon.method_root_for(type_name, body_name))
        .or_else(|| canon.root_for(body_name))
        .unwrap_or(canon.root);
    ori_arc::lower_function_can(
        name,
        &params,
        sig.return_type,
        body_id,
        canon,
        interner,
        pool,
        arc_problems,
        sig.is_fbip,
        type_subst,
    )
}

/// Lower a single impl method to ARC IR with correct method-root lookup.
///
/// Uses `canon.method_root_for(type_name, body_name)` to find the impl
/// method body (TPR-03-049).
#[expect(
    clippy::too_many_arguments,
    reason = "thin wrapper over lower_function_can — params mirror the underlying API"
)]
#[expect(
    clippy::implicit_hasher,
    reason = "downstream lower_function_can requires concrete FxHashMap — cannot generalize"
)]
pub fn lower_impl_method_to_arc(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Name,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_to_arc_impl(
        name,
        sig,
        body_name,
        Some(type_name),
        canon,
        interner,
        pool,
        arc_problems,
        type_subst,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "shared impl for lower_to_arc and lower_impl_method_to_arc"
)]
fn lower_to_arc_impl(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Option<Name>,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let params: Vec<(Name, Idx)> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&n, &t)| (n, t))
        .collect();
    // For impl methods, use method_root_for (searches method_roots).
    // For top-level functions, use root_for (searches roots).
    let body_id = if let Some(tn) = type_name {
        canon
            .method_root_for(tn, body_name)
            .or_else(|| canon.root_for(body_name))
            .unwrap_or(canon.root)
    } else {
        canon.root_for(body_name).unwrap_or(canon.root)
    };
    ori_arc::lower_function_can(
        name,
        &params,
        sig.return_type,
        body_id,
        canon,
        interner,
        pool,
        arc_problems,
        sig.is_fbip,
        type_subst,
    )
}
