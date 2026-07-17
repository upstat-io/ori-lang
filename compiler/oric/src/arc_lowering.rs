//! Shared ARC IR lowering utilities.
//!
//! Provides the common per-function lowering step used by both the AOT path
//! (`compile_common.rs`) and the JIT test runner (`runner/mod.rs`). Both paths
//! need to: build a params Vec, look up the canon body root, and call
//! `ori_arc::lower_function_can`. This module eliminates that duplication.

use ori_arc::ArcFunction;
use ori_ir::canon::{CanonResult, MonoConstBinding};
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
        None,
    )
}

/// Lower one exact monomorphized source body with its solved const environment.
///
/// `type_name` selects an impl-method canonical root when present. Ordinary
/// source functions pass `None`. The const bindings are producer-issued mono
/// metadata; this wrapper never infers them from the body or mangled name.
#[expect(
    clippy::too_many_arguments,
    reason = "mono lowering adds exact const bindings to the shared ARC coordinates"
)]
#[expect(
    clippy::implicit_hasher,
    reason = "downstream lowering consumes the concrete FxHashMap body substitution"
)]
pub fn lower_mono_to_arc(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Option<Name>,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: &FxHashMap<Idx, Idx>,
    const_bindings: &[MonoConstBinding],
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_to_arc_impl(
        name,
        sig,
        body_name,
        type_name,
        canon,
        interner,
        pool,
        arc_problems,
        Some(type_subst),
        Some(const_bindings),
    )
}

/// Lower a single impl method with ordinal-aware body lookup.
///
/// For types with multiple impls defining the same method name (e.g.,
/// `impl Index<int, V>` and `impl Index<str, V>`), `ordinal` selects
/// which body to use via `canon.method_root_for_nth()`.
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
/// method body.
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
        None,
    )
}

/// Lower one impl-method specialization by its exact parse-level body.
#[expect(
    clippy::too_many_arguments,
    reason = "thin wrapper over lower_function_can — params mirror the underlying API"
)]
#[expect(
    clippy::implicit_hasher,
    reason = "downstream lower_function_can requires concrete FxHashMap — cannot generalize"
)]
pub fn lower_impl_method_to_arc_by_source(
    name: Name,
    sig: &FunctionSig,
    source_body: ori_ir::ExprId,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_impl_method_to_arc_by_source_impl(
        name,
        sig,
        source_body,
        canon,
        interner,
        pool,
        arc_problems,
        type_subst,
        None,
    )
}

/// Lower one exact monomorphized impl body with solved type and const bindings.
#[expect(
    clippy::too_many_arguments,
    reason = "mono impl lowering adds exact const bindings to the shared ARC coordinates"
)]
#[expect(
    clippy::implicit_hasher,
    reason = "downstream lowering consumes the concrete FxHashMap body substitution"
)]
pub fn lower_mono_impl_method_to_arc_by_source(
    name: Name,
    sig: &FunctionSig,
    source_body: ori_ir::ExprId,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: &FxHashMap<Idx, Idx>,
    const_bindings: &[MonoConstBinding],
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_impl_method_to_arc_by_source_impl(
        name,
        sig,
        source_body,
        canon,
        interner,
        pool,
        arc_problems,
        Some(type_subst),
        Some(const_bindings),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "shared exact-source lowering coordinates include optional mono substitutions"
)]
fn lower_impl_method_to_arc_by_source_impl(
    name: Name,
    sig: &FunctionSig,
    source_body: ori_ir::ExprId,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    arc_problems: &mut Vec<ori_arc::ArcProblem>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
    const_bindings: Option<&[MonoConstBinding]>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let params: Vec<(Name, Idx)> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&name, &ty)| (name, ty))
        .collect();
    let Some(body_id) = canon.method_root_for_source(source_body) else {
        arc_problems.push(ori_arc::ArcProblem::InternalError {
            message: format!(
                "impl-method specialization {name:?} has no canonical root for source body {source_body:?}"
            ),
            span: ori_ir::Span::DUMMY,
        });
        return (
            ori_arc::ArcFunction {
                name,
                return_type: sig.return_type,
                ..ori_arc::ArcFunction::default()
            },
            Vec::new(),
        );
    };
    ori_arc::lower_function_can_with_const_bindings(
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
        const_bindings,
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
    const_bindings: Option<&[MonoConstBinding]>,
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
    ori_arc::lower_function_can_with_const_bindings(
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
        const_bindings,
    )
}
