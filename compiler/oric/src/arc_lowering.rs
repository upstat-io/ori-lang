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

/// Shared canonicalization and type state for one ARC lowering sequence.
pub(crate) struct ArcLoweringContext<'a> {
    pub(crate) canon: &'a CanonResult,
    pub(crate) interner: &'a StringInterner,
    pub(crate) pool: &'a Pool,
    pub(crate) problems: &'a mut Vec<ori_arc::ArcProblem>,
}

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
pub(crate) fn lower_to_arc(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    context: &mut ArcLoweringContext<'_>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_to_arc_impl(name, sig, body_name, None, context, type_subst, None)
}

/// Lower one exact monomorphized source body with its solved const environment.
///
/// `type_name` selects an impl-method canonical root when present. Ordinary
/// source functions pass `None`. The const bindings are producer-issued mono
/// metadata; this wrapper never infers them from the body or mangled name.
pub(crate) fn lower_mono_to_arc(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Option<Name>,
    context: &mut ArcLoweringContext<'_>,
    type_subst: &FxHashMap<Idx, Idx>,
    const_bindings: &[MonoConstBinding],
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_to_arc_impl(
        name,
        sig,
        body_name,
        type_name,
        context,
        Some(type_subst),
        Some(const_bindings),
    )
}

/// Lower a single impl method with ordinal-aware body lookup.
///
/// For types with multiple impls defining the same method name (e.g.,
/// `impl Index<int, V>` and `impl Index<str, V>`), `ordinal` selects
/// which body to use via `canon.method_root_for_nth()`.
pub(crate) fn lower_impl_method_to_arc_nth(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Name,
    ordinal: usize,
    context: &mut ArcLoweringContext<'_>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let params: Vec<(Name, Idx)> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&n, &t)| (n, t))
        .collect();
    let body_id = context
        .canon
        .method_root_for_nth(type_name, body_name, ordinal)
        .or_else(|| context.canon.method_root_for(type_name, body_name))
        .or_else(|| context.canon.root_for(body_name))
        .unwrap_or(context.canon.root);
    ori_arc::lower_function_can(
        ori_arc::ArcLoweringInput {
            name,
            params: &params,
            return_type: sig.return_type,
            body: body_id,
            canon: context.canon,
            interner: context.interner,
            pool: context.pool,
            type_subst,
            const_bindings: None,
            is_fbip: sig.is_fbip,
        },
        context.problems,
    )
}

/// Lower one impl-method specialization by its exact parse-level body.
pub(crate) fn lower_impl_method_to_arc_by_source(
    name: Name,
    sig: &FunctionSig,
    source_body: ori_ir::ExprId,
    context: &mut ArcLoweringContext<'_>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_impl_method_to_arc_by_source_impl(name, sig, source_body, context, type_subst, None)
}

/// Lower one exact monomorphized impl body with solved type and const bindings.
pub(crate) fn lower_mono_impl_method_to_arc_by_source(
    name: Name,
    sig: &FunctionSig,
    source_body: ori_ir::ExprId,
    context: &mut ArcLoweringContext<'_>,
    type_subst: &FxHashMap<Idx, Idx>,
    const_bindings: &[MonoConstBinding],
) -> (ArcFunction, Vec<ArcFunction>) {
    lower_impl_method_to_arc_by_source_impl(
        name,
        sig,
        source_body,
        context,
        Some(type_subst),
        Some(const_bindings),
    )
}

fn lower_impl_method_to_arc_by_source_impl(
    name: Name,
    sig: &FunctionSig,
    source_body: ori_ir::ExprId,
    context: &mut ArcLoweringContext<'_>,
    type_subst: Option<&FxHashMap<Idx, Idx>>,
    const_bindings: Option<&[MonoConstBinding]>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let params: Vec<(Name, Idx)> = sig
        .param_names
        .iter()
        .zip(sig.param_types.iter())
        .map(|(&name, &ty)| (name, ty))
        .collect();
    let Some(body_id) = context.canon.method_root_for_source(source_body) else {
        context.problems.push(ori_arc::ArcProblem::InternalError {
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
    ori_arc::lower_function_can(
        ori_arc::ArcLoweringInput {
            name,
            params: &params,
            return_type: sig.return_type,
            body: body_id,
            canon: context.canon,
            interner: context.interner,
            pool: context.pool,
            type_subst,
            const_bindings,
            is_fbip: sig.is_fbip,
        },
        context.problems,
    )
}

fn lower_to_arc_impl(
    name: Name,
    sig: &FunctionSig,
    body_name: Name,
    type_name: Option<Name>,
    context: &mut ArcLoweringContext<'_>,
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
        context
            .canon
            .method_root_for(tn, body_name)
            .or_else(|| context.canon.root_for(body_name))
            .unwrap_or(context.canon.root)
    } else {
        context
            .canon
            .root_for(body_name)
            .unwrap_or(context.canon.root)
    };
    ori_arc::lower_function_can(
        ori_arc::ArcLoweringInput {
            name,
            params: &params,
            return_type: sig.return_type,
            body: body_id,
            canon: context.canon,
            interner: context.interner,
            pool: context.pool,
            type_subst,
            const_bindings,
            is_fbip: sig.is_fbip,
        },
        context.problems,
    )
}
