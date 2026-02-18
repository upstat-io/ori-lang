//! Function call and method call inference.

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};

use super::super::InferEngine;
use super::methods::DEI_ONLY_METHODS;
use super::{infer_expr, resolve_builtin_method};
use crate::{
    ContextKind, Expected, ExpectedOrigin, Idx, MethodLookupResult, Pool, Tag, TypeCheckError,
    TypeCheckWarning,
};

/// Infer the type of a function call expression.
pub(crate) fn infer_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let arg_ids = arena.get_expr_list(args);

    // Extract function name for signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if arg_ids.len() < required_params || arg_ids.len() > params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            params.len(),
            arg_ids.len(),
            crate::ArityMismatchKind::Function,
        ));
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each provided argument
    for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: None,
                    arg_index: i,
                    param_name: None,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg_id);
        let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
    }

    ret
}

/// Infer the type of a named-argument function call.
pub(crate) fn infer_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    func: ExprId,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Idx {
    let func_ty = infer_expr(engine, arena, func);
    let resolved = engine.resolve(func_ty);

    if engine.pool().tag(resolved) != Tag::Function {
        if resolved != Idx::ERROR {
            engine.push_error(TypeCheckError::not_callable(span, resolved));
        }
        return Idx::ERROR;
    }

    let params = engine.pool().function_params(resolved);
    let ret = engine.pool().function_return(resolved);

    let call_args = arena.get_call_args(args);

    // Extract function name for error messages and signature lookup
    let func_name_id = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(name) | ExprKind::Ident(name) => Some(*name),
        _ => None,
    };

    // Look up required_params from function signature if available
    let required_params = func_name_id
        .and_then(|n| engine.get_signature(n))
        .map_or(params.len(), |sig| sig.required_params);

    // Check arity: allow fewer args if defaults fill the gap
    if call_args.len() < required_params || call_args.len() > params.len() {
        // Allocate func name string only on the error path
        let func_name = func_name_id.and_then(|n| engine.lookup_name(n).map(String::from));
        if let Some(name) = func_name {
            engine.push_error(TypeCheckError::arity_mismatch_named(
                span,
                name,
                params.len(),
                call_args.len(),
            ));
        } else {
            engine.push_error(TypeCheckError::arity_mismatch(
                span,
                params.len(),
                call_args.len(),
                crate::ArityMismatchKind::Function,
            ));
        }
        return Idx::ERROR;
    }

    // Validate capability requirements
    check_call_capabilities(engine, func_name_id, span);

    // Check each argument type by position
    for (i, (arg, &param_ty)) in call_args.iter().zip(params.iter()).enumerate() {
        let expected = Expected {
            ty: param_ty,
            origin: ExpectedOrigin::Context {
                span: arena.get_expr(func).span,
                kind: ContextKind::FunctionArgument {
                    func_name: func_name_id,
                    arg_index: i,
                    param_name: arg.name,
                },
            },
        };
        let arg_ty = infer_expr(engine, arena, arg.value);
        let _ = engine.check_type(arg_ty, &expected, arg.span);
    }

    // Validate where-clause constraints after argument type-checking.
    // At this point, generic type variables have been unified with concrete types.
    if let Some(func_name) = match &arena.get_expr(func).kind {
        ExprKind::FunctionRef(n) | ExprKind::Ident(n) => Some(*n),
        _ => None,
    } {
        check_where_clauses(engine, func_name, &params, span);
    }

    ret
}

/// Validate that required capabilities are available at a call site.
///
/// Looks up the callee's signature to find its `uses` capabilities,
/// then checks each one against the caller's declared + provided capabilities.
/// Emits `E2014 MissingCapability` for each missing capability.
pub(crate) fn check_call_capabilities(
    engine: &mut InferEngine<'_>,
    func_name: Option<Name>,
    span: Span,
) {
    let Some(name) = func_name else { return };
    let Some(sig) = engine.get_signature(name) else {
        return;
    };

    // Collect missing capabilities during immutable borrow
    let missing: Vec<Name> = sig
        .capabilities
        .iter()
        .copied()
        .filter(|&cap| !engine.has_capability(cap))
        .collect();

    if missing.is_empty() {
        return;
    }

    // Push errors in a separate mutable pass
    let available = engine.available_capabilities();
    for cap in missing {
        tracing::debug!(?cap, "missing capability at call site");
        engine.push_error(TypeCheckError::missing_capability(span, cap, &available));
    }
}

/// Validate where-clause constraints for a generic function call.
///
/// After argument type-checking has unified generic type variables with concrete
/// types, this checks constraints like `where C.Item: Eq` by:
/// 1. Resolving the concrete type for the generic param
/// 2. Finding the trait impl that defines the associated type
/// 3. Looking up the projected type
/// 4. Checking the projected type satisfies the required trait bound
///
/// Uses a three-phase approach to satisfy the borrow checker:
/// 1. Mutable phase: resolve types and create pool entries
/// 2. Immutable phase: check trait registry and collect violations
/// 3. Mutable phase: push collected errors
pub(crate) fn check_where_clauses(
    engine: &mut InferEngine<'_>,
    func_name: Name,
    params: &[Idx],
    call_span: Span,
) {
    struct PreparedCheck {
        concrete_type: Idx,
        projection: Option<Name>,
        bound_entries: Vec<(Name, Idx)>,
        trait_bound_entries: Vec<Idx>,
    }

    let Some(sig) = engine.get_signature(func_name) else {
        return;
    };

    if sig.where_clauses.is_empty() {
        return;
    }

    // Extract only the fields we need, avoiding a full FunctionSig clone
    let where_clauses = sig.where_clauses.clone();
    let type_params = sig.type_params.clone();
    let type_param_bounds = sig.type_param_bounds.clone();
    let generic_param_mapping = sig.generic_param_mapping.clone();

    // Phase 1 (mutable): Resolve concrete types and create named Idx entries

    let mut prepared = Vec::new();

    for wc in &where_clauses {
        let Some(tp_idx) = type_params.iter().position(|&n| n == wc.param) else {
            continue;
        };
        let Some(Some(param_idx)) = generic_param_mapping.get(tp_idx) else {
            continue;
        };
        let Some(&instantiated_param) = params.get(*param_idx) else {
            continue;
        };
        let concrete_type = engine.resolve(instantiated_param);
        if concrete_type == Idx::ERROR {
            continue;
        }

        // Pre-create named Idx for each bound (needs &mut pool)
        let bound_entries: Vec<(Name, Idx)> = wc
            .bounds
            .iter()
            .map(|&name| (name, engine.pool_mut().named(name)))
            .collect();

        // Pre-create named Idx for type param bounds (for projection lookup)
        let tp_bounds = type_param_bounds.get(tp_idx).cloned().unwrap_or_default();
        let trait_bound_entries: Vec<Idx> = tp_bounds
            .iter()
            .map(|&name| engine.pool_mut().named(name))
            .collect();

        prepared.push(PreparedCheck {
            concrete_type,
            projection: wc.projection,
            bound_entries,
            trait_bound_entries,
        });
    }

    // Phase 2 (immutable): Check trait registry and collect error messages
    let errors = {
        let Some(trait_registry) = engine.trait_registry() else {
            return;
        };
        let pool = engine.pool();

        let mut errors: Vec<String> = Vec::new();

        for check in &prepared {
            if let Some(projection) = check.projection {
                // Where-clause with projection: `where C.Item: Eq`
                for &trait_idx in &check.trait_bound_entries {
                    let Some((_, impl_entry)) =
                        trait_registry.find_impl(trait_idx, check.concrete_type)
                    else {
                        continue;
                    };
                    let Some(&projected_type) = impl_entry.assoc_types.get(&projection) else {
                        continue;
                    };
                    for &(bound_name, bound_idx) in &check.bound_entries {
                        let bound_str = engine.lookup_name(bound_name).unwrap_or("");
                        if !trait_registry.has_impl(bound_idx, projected_type)
                            && !type_satisfies_trait(projected_type, bound_str, pool)
                        {
                            errors.push(format!("does not satisfy trait bound `{bound_str}`",));
                        }
                    }
                }
            } else {
                // Direct bound: `where T: Clone`
                for &(bound_name, bound_idx) in &check.bound_entries {
                    let bound_str = engine.lookup_name(bound_name).unwrap_or("");
                    if !trait_registry.has_impl(bound_idx, check.concrete_type)
                        && !type_satisfies_trait(check.concrete_type, bound_str, pool)
                    {
                        errors.push(format!("does not satisfy trait bound `{bound_str}`",));
                    }
                }
            }
        }

        errors
    };

    // Phase 3 (mutable): Push collected errors
    for msg in errors {
        engine.push_error(TypeCheckError::unsatisfied_bound(call_span, msg));
    }
}

/// Check if a type inherently satisfies a trait without needing an explicit impl.
///
/// Mirrors V1's `primitive_implements_trait()` from `bound_checking.rs`.
/// Primitive and built-in types have known trait implementations that don't
/// require explicit `impl` blocks in the trait registry.
pub(crate) fn primitive_satisfies_trait(ty: Idx, trait_name: &str) -> bool {
    // Trait sets for each primitive type, matching V1's const arrays.
    const INT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "FloorDiv",
        "Rem",
        "Neg",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const FLOAT_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Neg",
    ];
    const BOOL_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Not",
    ];
    const STR_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Len",
        "IsEmpty",
        "Add",
    ];
    const CHAR_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable", "Printable"];
    const BYTE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Printable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "BitAnd",
        "BitOr",
        "BitXor",
        "BitNot",
        "Shl",
        "Shr",
    ];
    const UNIT_TRAITS: &[&str] = &["Eq", "Clone", "Default"];
    const DURATION_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
        "Neg",
    ];
    const SIZE_TRAITS: &[&str] = &[
        "Eq",
        "Comparable",
        "Clone",
        "Hashable",
        "Default",
        "Printable",
        "Sendable",
        "Add",
        "Sub",
        "Mul",
        "Div",
        "Rem",
    ];
    const ORDERING_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable", "Printable"];

    // Check primitive types by Idx constant
    if ty == Idx::INT {
        return INT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::FLOAT {
        return FLOAT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BOOL {
        return BOOL_TRAITS.contains(&trait_name);
    }
    if ty == Idx::STR {
        return STR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::CHAR {
        return CHAR_TRAITS.contains(&trait_name);
    }
    if ty == Idx::BYTE {
        return BYTE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::UNIT {
        return UNIT_TRAITS.contains(&trait_name);
    }
    if ty == Idx::DURATION {
        return DURATION_TRAITS.contains(&trait_name);
    }
    if ty == Idx::SIZE {
        return SIZE_TRAITS.contains(&trait_name);
    }
    if ty == Idx::ORDERING {
        return ORDERING_TRAITS.contains(&trait_name);
    }

    false
}

/// Extended trait satisfaction check that also handles compound types via Pool tags.
///
/// This extends `primitive_satisfies_trait` to handle List, Map, Option, Result,
/// Tuple, Set, and Range — types that aren't simple Idx constants but can be
/// identified by their Pool tag.
pub(crate) fn type_satisfies_trait(ty: Idx, trait_name: &str, pool: &Pool) -> bool {
    const COLLECTION_TRAITS: &[&str] = &["Eq", "Clone", "Hashable", "Len", "IsEmpty"];
    const WRAPPER_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable", "Default"];
    const RESULT_TRAITS: &[&str] = &["Eq", "Comparable", "Clone", "Hashable"];

    // First check primitives (no pool access needed)
    if primitive_satisfies_trait(ty, trait_name) {
        return true;
    }

    // Then check compound types by tag

    match pool.tag(ty) {
        Tag::List => {
            COLLECTION_TRAITS.contains(&trait_name)
                || trait_name == "Comparable"
                || trait_name == "Iterable"
        }
        Tag::Map | Tag::Set => COLLECTION_TRAITS.contains(&trait_name) || trait_name == "Iterable",
        Tag::Option => WRAPPER_TRAITS.contains(&trait_name),
        Tag::Result | Tag::Tuple => RESULT_TRAITS.contains(&trait_name),
        Tag::Range => matches!(trait_name, "Len" | "Iterable"),
        Tag::Str => trait_name == "Iterable",
        Tag::DoubleEndedIterator => trait_name == "Iterator" || trait_name == "DoubleEndedIterator",
        Tag::Iterator => trait_name == "Iterator",
        _ => false,
    }
}

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Trait for Type { ... }`)
///
/// For unresolved type variables, returns a fresh variable to defer resolution.
pub(crate) fn infer_method_call(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::ExprRange,
    span: Span,
) -> Idx {
    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
        ReceiverDispatch::Return(ty) => {
            for &arg_id in arena.get_expr_list(args) {
                infer_expr(engine, arena, arg_id);
            }
            return ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let arg_ids = arena.get_expr_list(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, arg_ids.len(), span) {
        for (i, (&arg_id, &param_ty)) in arg_ids.iter().zip(sig.params.iter()).enumerate() {
            let expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: None,
                        arg_index: i,
                        param_name: None,
                    },
                },
            };
            let arg_ty = infer_expr(engine, arena, arg_id);
            let _ = engine.check_type(arg_ty, &expected, arena.get_expr(arg_id).span);
        }
        return sig.ret;
    }

    // Error or not found — infer all args for side effects
    for &arg_id in arena.get_expr_list(args) {
        infer_expr(engine, arena, arg_id);
    }
    Idx::ERROR
}

/// Infer the type of a named-argument method call: `receiver.method(name: value)`.
pub(crate) fn infer_method_call_named(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    args: ori_ir::CallArgRange,
    span: Span,
) -> Idx {
    let resolved = match resolve_receiver_and_builtin(engine, arena, receiver, method, span) {
        ReceiverDispatch::Return(ty) => {
            for arg in arena.get_call_args(args) {
                infer_expr(engine, arena, arg.value);
            }
            return ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let call_args = arena.get_call_args(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, call_args.len(), span) {
        for (i, (arg, &param_ty)) in call_args.iter().zip(sig.params.iter()).enumerate() {
            let expected = Expected {
                ty: param_ty,
                origin: ExpectedOrigin::Context {
                    span,
                    kind: ContextKind::FunctionArgument {
                        func_name: None,
                        arg_index: i,
                        param_name: arg.name,
                    },
                },
            };
            let arg_ty = infer_expr(engine, arena, arg.value);
            let _ = engine.check_type(arg_ty, &expected, arg.span);
        }
        return sig.ret;
    }

    // Error or not found — infer all args for side effects
    for arg in arena.get_call_args(args) {
        infer_expr(engine, arena, arg.value);
    }
    Idx::ERROR
}

// ── Shared method dispatch helpers ───────────────────────────────────

/// Result of resolving a method receiver and checking builtin dispatch.
enum ReceiverDispatch {
    /// Return this type. Caller must infer all args first.
    Return(Idx),
    /// No builtin found. Proceed to impl lookup with this resolved receiver.
    Continue { resolved: Idx },
}

/// Resolve the receiver type and try builtin method dispatch.
///
/// Handles: receiver inference, error propagation, scheme instantiation,
/// type-variable deferral, builtin method lookup, `DoubleEndedIterator`
/// gating, and `Range<float>` iteration rejection.
///
/// Returns `Return(ty)` for early results (caller should infer all args
/// and return the type). Returns `Continue { resolved }` to proceed
/// with impl method lookup.
fn resolve_receiver_and_builtin(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: Name,
    span: Span,
) -> ReceiverDispatch {
    let receiver_ty = infer_expr(engine, arena, receiver);
    let resolved = engine.resolve(receiver_ty);

    // Propagate errors silently
    if resolved == Idx::ERROR {
        return ReceiverDispatch::Return(Idx::ERROR);
    }

    // If receiver is a scheme, instantiate it to get the concrete type
    let resolved = if engine.pool().tag(resolved) == Tag::Scheme {
        engine.instantiate(resolved)
    } else {
        resolved
    };

    // For unresolved type variables, defer resolution
    let tag = engine.pool().tag(resolved);
    if tag == Tag::Var {
        return ReceiverDispatch::Return(engine.pool_mut().fresh_var());
    }

    let method_str = engine.lookup_name(method);

    // 1. Try built-in method resolution
    if let Some(name_str) = method_str {
        if let Some(ret) = resolve_builtin_method(engine, resolved, tag, name_str) {
            // 1a. Before returning, check for infinite iterator consumption
            if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
                check_infinite_iterator_consumed(engine, arena, receiver, name_str, span);
            }
            return ReceiverDispatch::Return(ret);
        }
    }

    // 1b. Reject DoubleEndedIterator methods on plain Iterator receivers
    if tag == Tag::Iterator {
        if let Some(name_str) = method_str {
            if DEI_ONLY_METHODS.contains(&name_str) {
                engine.push_error(TypeCheckError::unsatisfied_bound(
                    span,
                    format!(
                        "`{name_str}` requires a DoubleEndedIterator, \
                         but this is an Iterator (use .iter() on a list, range, \
                         or string to get a DoubleEndedIterator)"
                    ),
                ));
                return ReceiverDispatch::Return(Idx::ERROR);
            }
        }
    }

    // 1c. Reject iteration methods on Range<float>
    if let Some(err) = check_range_float_iteration(engine, resolved, tag, method_str, span) {
        return ReceiverDispatch::Return(err);
    }

    ReceiverDispatch::Continue { resolved }
}

/// Check if a method call on a `Range<float>` is attempting iteration.
///
/// Returns `Some(Idx::ERROR)` with a diagnostic pushed if the method
/// is an iteration method and the range element type is `float`.
/// Returns `None` if the check doesn't apply.
fn check_range_float_iteration(
    engine: &mut InferEngine<'_>,
    resolved: Idx,
    tag: Tag,
    method_str: Option<&str>,
    span: Span,
) -> Option<Idx> {
    if tag != Tag::Range {
        return None;
    }
    let name_str = method_str?;
    if !matches!(name_str, "iter" | "collect" | "to_list") {
        return None;
    }
    let elem = engine.pool().range_elem(resolved);
    if elem != Idx::FLOAT {
        return None;
    }
    engine.push_error(TypeCheckError::range_float_not_iterable(
        span,
        "(0..10).iter().map((i) -> i.to_float() / 10.0)",
    ));
    Some(Idx::ERROR)
}

/// Methods that consume an entire iterator and will never terminate on infinite sources.
const INFINITE_CONSUMING_METHODS: &[&str] = &["collect", "count", "fold", "for_each", "to_list"];

/// Methods that are transparent — they wrap the source but don't bound it.
const TRANSPARENT_ADAPTERS: &[&str] = &[
    "map",
    "filter",
    "enumerate",
    "skip",
    "zip",
    "chain",
    "flatten",
    "flat_map",
    "rev",
    "iter",
];

/// Methods that bound an infinite iterator, making consumption safe.
const BOUNDING_METHODS: &[&str] = &["take"];

/// Check if a consuming method is called on an infinite iterator source.
///
/// Walks the receiver's AST chain backward looking for infinite sources
/// (`repeat()`, unbounded ranges `start..`, `.cycle()`) without an
/// intervening `.take()` that would bound the iteration.
///
/// Emits a warning (W2001) if an infinite pattern is detected.
fn check_infinite_iterator_consumed(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver: ExprId,
    method: &str,
    span: Span,
) {
    if !INFINITE_CONSUMING_METHODS.contains(&method) {
        return;
    }

    if let Some(source_desc) = find_infinite_source(engine, arena, receiver) {
        engine.push_warning(TypeCheckWarning::infinite_iterator_consumed(
            span,
            method,
            source_desc,
        ));
    }
}

/// Walk the AST chain from a receiver expression looking for an infinite source.
///
/// Returns `Some(description)` if an unbounded infinite source is found,
/// `None` if the chain is bounded or not infinite.
pub(crate) fn find_infinite_source(
    engine: &InferEngine<'_>,
    arena: &ExprArena,
    expr: ExprId,
) -> Option<String> {
    let node = arena.get_expr(expr);
    match &node.kind {
        // Method call chain: check the method name, then walk the receiver
        ExprKind::MethodCall {
            receiver, method, ..
        }
        | ExprKind::MethodCallNamed {
            receiver, method, ..
        } => {
            let name = engine.lookup_name(*method).unwrap_or("");
            // .take() bounds the chain — safe
            if BOUNDING_METHODS.contains(&name) {
                return None;
            }
            // .cycle() is an infinite source
            if name == "cycle" {
                return Some("cycle()".into());
            }
            // Transparent adapters — keep walking
            if TRANSPARENT_ADAPTERS.contains(&name) {
                return find_infinite_source(engine, arena, *receiver);
            }
            // Unknown method — stop (conservative: don't warn)
            None
        }

        // Function call: check if it's `repeat(...)`
        ExprKind::Call { func, .. } | ExprKind::CallNamed { func, .. } => {
            let func_node = arena.get_expr(*func);
            if let ExprKind::Ident(name) = &func_node.kind {
                let name_str = engine.lookup_name(*name).unwrap_or("");
                if name_str == "repeat" {
                    return Some("repeat()".into());
                }
            }
            None
        }

        // Range expression: check if end is unbounded
        ExprKind::Range { end, .. } => {
            if !end.is_valid() {
                return Some("unbounded range (start..)".into());
            }
            None
        }

        // Anything else — stop (conservative: don't warn on unknowns)
        _ => None,
    }
}

// ── Impl method resolution (TraitRegistry) ───────────────────────────

/// Result of looking up a method in the `TraitRegistry`.
enum LookupOutcome {
    Found { sig: Idx, has_self: bool },
    Ambiguous(Vec<ori_ir::Name>),
    NotFound,
}

/// Successfully resolved impl method signature.
struct ImplMethodSig {
    /// Method parameters (excluding `self`).
    params: Vec<Idx>,
    /// Return type.
    ret: Idx,
}

/// Perform the borrow-dance lookup for impl methods via `TraitRegistry`.
///
/// Scopes the immutable `trait_registry` borrow to extract data, so the
/// caller can use `engine` mutably afterwards.
fn lookup_impl_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> LookupOutcome {
    let trait_registry = engine.trait_registry();
    match trait_registry {
        None => LookupOutcome::NotFound,
        Some(reg) => match reg.lookup_method_checked(receiver_ty, method) {
            MethodLookupResult::Found(lookup) => LookupOutcome::Found {
                sig: lookup.method().signature,
                has_self: lookup.method().has_self,
            },
            MethodLookupResult::Ambiguous { candidates } => {
                LookupOutcome::Ambiguous(candidates.iter().map(|&(_, n)| n).collect())
            }
            MethodLookupResult::NotFound => LookupOutcome::NotFound,
        },
    }
}

/// After an impl method lookup, resolve the signature and validate arity.
///
/// Returns `Some(Ok(sig))` on success with params (excluding `self`) and
/// return type. Returns `Some(Err(()))` for errors (ambiguous, bad
/// signature, arity mismatch — diagnostic already pushed). Returns `None`
/// if the method was not found.
fn resolve_impl_signature(
    engine: &mut InferEngine<'_>,
    outcome: LookupOutcome,
    method: Name,
    arg_count: usize,
    span: Span,
) -> Option<Result<ImplMethodSig, ()>> {
    let (sig_ty, has_self) = match outcome {
        LookupOutcome::Found { sig, has_self } => (sig, has_self),
        LookupOutcome::Ambiguous(trait_names) => {
            engine.push_error(TypeCheckError::ambiguous_method(span, method, trait_names));
            return Some(Err(()));
        }
        LookupOutcome::NotFound => return None,
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Err(()));
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // For instance methods (has_self), skip the first `self` param
    let skip = usize::from(has_self);
    let method_params = params[skip..].to_vec();

    if arg_count != method_params.len() {
        engine.push_error(TypeCheckError::arity_mismatch(
            span,
            method_params.len(),
            arg_count,
            crate::ArityMismatchKind::Function,
        ));
        return Some(Err(()));
    }

    Some(Ok(ImplMethodSig {
        params: method_params,
        ret,
    }))
}
