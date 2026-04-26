//! Method call type inference: `receiver.method(args)`.

use ori_diagnostic::Suggestion;
use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span};

use super::super::super::InferEngine;
use super::super::{infer_expr, range_method_requires_iteration, resolve_builtin_method};
use super::impl_lookup::{
    emit_into_not_implemented, lookup_impl_method, resolve_impl_signature, ImplMethodSig,
};
use crate::type_error::ErrorContext;
use crate::{ContextKind, Expected, ExpectedOrigin, Idx, Tag, TypeCheckError, TypeCheckWarning};

/// Infer the type of a method call expression: `receiver.method(args)`.
///
/// Resolution priority:
/// 1. Built-in methods on primitives/collections (len, `is_empty`, first, etc.)
/// 2. User-defined inherent methods (from `impl Type { ... }`)
/// 3. User-defined trait methods (from `impl Type: Trait { ... }`)
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
        ReceiverDispatch::Return {
            ret_ty,
            receiver_ty,
        } => {
            let arg_ids: Vec<ExprId> = arena.get_expr_list(args).to_vec();
            let arg_types: Vec<Idx> = arg_ids
                .iter()
                .map(|&arg_id| infer_expr(engine, arena, arg_id))
                .collect();
            let arg_spans: Vec<Span> = arg_ids
                .iter()
                .map(|&arg_id| arena.get_expr(arg_id).span)
                .collect();
            unify_higher_order_constraints(
                engine,
                method,
                ret_ty,
                receiver_ty,
                &arg_types,
                &arg_spans,
                span,
            );
            return ret_ty;
        }
        ReceiverDispatch::Continue { resolved } => resolved,
    };

    let arg_ids = arena.get_expr_list(args);
    let outcome = lookup_impl_method(engine, resolved, method);
    if let Some(Ok(sig)) = resolve_impl_signature(engine, outcome, method, arg_ids.len(), span) {
        return check_positional_args(engine, arena, arg_ids, &sig, span);
    }

    // Error or not found -- infer all args for side effects
    for &arg_id in arena.get_expr_list(args) {
        infer_expr(engine, arena, arg_id);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);

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
        ReceiverDispatch::Return {
            ret_ty,
            receiver_ty,
        } => {
            let call_args = arena.get_call_args(args);
            let arg_types: Vec<Idx> = call_args
                .iter()
                .map(|arg| infer_expr(engine, arena, arg.value))
                .collect();
            // Per Plan TPR finding A4: use arena.get_expr(arg.value).span
            // (value-only span) instead of arg.span (whole `name: value` range)
            // so diagnostics anchor at the closure body, not the keyword.
            let arg_spans: Vec<Span> = call_args
                .iter()
                .map(|arg| arena.get_expr(arg.value).span)
                .collect();
            unify_higher_order_constraints(
                engine,
                method,
                ret_ty,
                receiver_ty,
                &arg_types,
                &arg_spans,
                span,
            );
            return ret_ty;
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

    // Error or not found -- infer all args for side effects
    for arg in arena.get_call_args(args) {
        infer_expr(engine, arena, arg.value);
    }

    // Emit E2036 for unresolved `.into()` calls
    emit_into_not_implemented(engine, resolved, method, span);

    Idx::ERROR
}

/// Result of resolving a method receiver and checking builtin dispatch.
enum ReceiverDispatch {
    /// Return this type. Caller must infer all args first.
    /// `receiver_ty` is the resolved receiver, needed for higher-order constraint propagation.
    Return { ret_ty: Idx, receiver_ty: Idx },
    /// No builtin found. Proceed to impl lookup with this resolved receiver.
    Continue { resolved: Idx },
}

/// Type-check positional method call arguments against resolved param types.
fn check_positional_args(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    arg_ids: &[ExprId],
    sig: &ImplMethodSig,
    span: Span,
) -> Idx {
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
    sig.ret
}

/// Unify fresh type variables in builtin method return types with constraints
/// from closure arguments.
///
/// Higher-order iterator adapters (`map`, `flat_map`, `fold`) create fresh
/// type variables in their return types. After the closure arguments are
/// inferred, this function unifies those variables with the closure's return
/// type so they resolve to concrete types before codegen.
///
/// Also unifies closure **parameter** types with the source iterator's element
/// type, ensuring unannotated lambda params (e.g., `r` in `.map(r -> r.score)`)
/// resolve to the correct type rather than remaining as unresolved type variables.
fn unify_higher_order_constraints(
    engine: &mut InferEngine<'_>,
    method: Name,
    ret_ty: Idx,
    receiver_ty: Idx,
    arg_types: &[Idx],
    arg_spans: &[Span],
    call_span: Span,
) {
    debug_assert_eq!(
        arg_types.len(),
        arg_spans.len(),
        "arg_types and arg_spans must be parallel (one entry per arg)"
    );

    let Some(method_str) = engine.lookup_name(method) else {
        return;
    };

    match method_str {
        "map" => {
            // ret_ty is Iterator<var> or DEI<var>.
            // arg_types[0] is the closure (T) -> U. Unify var with U.
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            let resolved_ret = engine.resolve(ret_ty);
            if !engine.pool().tag(resolved_ret).is_iterator() {
                return;
            }
            let elem_var = engine.pool().iterator_elem(resolved_ret);
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                let closure_ret = engine.pool().function_return(resolved_closure);
                let _ = engine.unify().unify(elem_var, closure_ret);
                // Unify closure param with source iterator element
                unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);
            }
        }
        "flat_map" => {
            unify_flat_map_constraints(
                engine,
                ret_ty,
                receiver_ty,
                arg_types,
                arg_spans,
                call_span,
            );
        }
        // filter, any, all, find, for_each: closure (T) -> bool/void
        "filter" | "any" | "all" | "find" | "for_each" => {
            let Some(&closure_ty) = arg_types.first() else {
                return;
            };
            let resolved_closure = engine.resolve(closure_ty);
            if engine.pool().tag(resolved_closure) == Tag::Function {
                unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);
            }
        }
        "fold" | "rfold" => {
            // ret_ty is a fresh var. Unify with initial value and closure return.
            if let Some(&init_ty) = arg_types.first() {
                let _ = engine.unify().unify(ret_ty, init_ty);
            }
            if let Some(&closure_ty) = arg_types.get(1) {
                let resolved_closure = engine.resolve(closure_ty);
                if engine.pool().tag(resolved_closure) == Tag::Function {
                    let closure_ret = engine.pool().function_return(resolved_closure);
                    let _ = engine.unify().unify(ret_ty, closure_ret);
                    // fold/rfold closure is (Acc, T) -> Acc:
                    // first param is accumulator, second param is iterator elem
                    let params = engine.pool().function_params(resolved_closure);
                    if let Some(&first_param) = params.first() {
                        let _ = engine.unify().unify(first_param, ret_ty);
                    }
                    let resolved_recv = engine.resolve(receiver_ty);
                    let recv_tag = engine.pool().tag(resolved_recv);
                    let source_elem = if recv_tag.is_iterator() {
                        Some(engine.pool().iterator_elem(resolved_recv))
                    } else if recv_tag == Tag::Set {
                        Some(engine.pool().set_elem(resolved_recv))
                    } else if recv_tag == Tag::List {
                        Some(engine.pool().list_elem(resolved_recv))
                    } else {
                        None
                    };
                    if let Some(elem) = source_elem {
                        if let Some(&second_param) = params.get(1) {
                            let _ = engine.unify().unify(second_param, elem);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Unify a closure's first parameter with the source iterator's element type.
///
/// For adapters like `.map(r -> r.score)`, ensures that `r` is constrained to
/// the iterator's element type rather than remaining as an unresolved type variable.
fn unify_closure_param_with_iterator_elem(
    engine: &mut InferEngine<'_>,
    resolved_closure: Idx,
    receiver_ty: Idx,
) {
    let resolved_recv = engine.resolve(receiver_ty);
    if !engine.pool().tag(resolved_recv).is_iterator() {
        return;
    }
    let source_elem = engine.pool().iterator_elem(resolved_recv);
    let params = engine.pool().function_params(resolved_closure);
    if let Some(&first_param) = params.first() {
        let _ = engine.unify().unify(first_param, source_elem);
    }
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
        return ReceiverDispatch::Return {
            ret_ty: Idx::ERROR,
            receiver_ty: Idx::ERROR,
        };
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
        return ReceiverDispatch::Return {
            ret_ty: engine.pool_mut().fresh_var(),
            receiver_ty: resolved,
        };
    }

    let method_str = engine.lookup_name(method);

    // 1. Try built-in method resolution
    if let Some(name_str) = method_str {
        if let Some(ret) = resolve_builtin_method(engine, resolved, tag, name_str) {
            // 1a. Before returning, check for infinite iterator consumption
            if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
                check_infinite_iterator_consumed(engine, arena, receiver, name_str, span);
            }
            return ReceiverDispatch::Return {
                ret_ty: ret,
                receiver_ty: resolved,
            };
        }
    }

    // 1b. Reject DoubleEndedIterator methods on plain Iterator receivers
    if tag == Tag::Iterator {
        if let Some(name_str) = method_str {
            if ori_registry::is_dei_only(name_str) {
                engine.push_error(TypeCheckError::unsatisfied_bound(
                    span,
                    format!(
                        "`{name_str}` requires a DoubleEndedIterator, \
                         but this is an Iterator (use .iter() on a list, range, \
                         or string to get a DoubleEndedIterator)"
                    ),
                ));
                return ReceiverDispatch::Return {
                    ret_ty: Idx::ERROR,
                    receiver_ty: Idx::ERROR,
                };
            }
        }
    }

    // 1c. Reject ALL methods on Range<float> — ranges are int-only per spec.
    // Float range construction is now rejected in infer_range(), but this guard
    // is defense-in-depth in case Range<float> is constructed through other means.
    // Iteration methods get a specific diagnostic; other methods get a generic
    // "Range<float> not supported" error with a diagnostic (not silent).
    if tag == Tag::Range && engine.pool().range_elem(resolved) == Idx::FLOAT {
        if let Some(err) = check_range_float_iteration(engine, resolved, tag, method_str, span) {
            return ReceiverDispatch::Return {
                ret_ty: err,
                receiver_ty: resolved,
            };
        }
        // Non-iteration methods: emit diagnostic, then return error
        engine.push_error(TypeCheckError::range_float_not_constructible(span));
        return ReceiverDispatch::Return {
            ret_ty: Idx::ERROR,
            receiver_ty: resolved,
        };
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
    if !range_method_requires_iteration(name_str) {
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

/// Type-check `flat_map`'s closure-return shape requirement.
///
/// `flat_map` requires the closure to return `Iterator<U>` or
/// `DoubleEndedIterator<U>`. This is the only place that requirement is
/// enforceable at typecheck time — the registry signature carries a
/// fresh-var return because `U` is determined by the closure body, not
/// the signature.
///
/// **Step ordering** — load-bearing per `bug-tracker/plans/BUG-02-013/`
/// Plan TPR Round 0 finding A1: param-elem unification runs FIRST, then
/// we re-resolve the closure return, then we check the shape. The reverse
/// order would silently accept identity closures `x -> x` because the
/// shape check would see an unbound `Tag::Var`, defer, and only afterwards
/// bind the var to `receiver_elem` — never re-checking.
fn unify_flat_map_constraints(
    engine: &mut InferEngine<'_>,
    ret_ty: Idx,
    receiver_ty: Idx,
    arg_types: &[Idx],
    arg_spans: &[Span],
    call_span: Span,
) {
    let Some(&closure_ty) = arg_types.first() else {
        return;
    };
    let closure_span = arg_spans.first().copied().unwrap_or(call_span);
    let resolved_ret = engine.resolve(ret_ty);
    if !engine.pool().tag(resolved_ret).is_iterator() {
        return;
    }
    let elem_var = engine.pool().iterator_elem(resolved_ret);
    let resolved_closure = engine.resolve(closure_ty);
    if engine.pool().tag(resolved_closure) != Tag::Function {
        return;
    }

    // STEP 3a — Param-elem unification first (the REORDER per A1). For
    // `(α) -> α`, this binds α to receiver_elem so STEP 3b's re-resolve
    // sees a concrete type instead of an unbound var.
    unify_closure_param_with_iterator_elem(engine, resolved_closure, receiver_ty);

    // STEP 3b — Re-resolve the closure return now that the param-elem
    // binding may have transitively bound vars reachable from closure_ret.
    let closure_ret = engine.pool().function_return(resolved_closure);
    let resolved_inner = engine.resolve(closure_ret);
    let inner_tag = engine.pool().tag(resolved_inner);

    // STEP 3c-PRE — Compound-poison absorb (Plan TPR Round 1 finding R1-A).
    // The OUTER tag check below catches `Tag::Error` directly, but compound
    // returns like `[Error]`, `Option<Error>`, `Function<.., Error>` carry
    // `HAS_ERROR` propagated upward per `types.md §TF-3 PROPAGATE_MASK`
    // while keeping their outer tag intact. Without this absorb, the
    // diagnostic arm would fire E2001 on top of the upstream E2003 — a
    // `typeck.md §ER-4` cascade. The `return` is a SILENT defer: the
    // upstream `Tag::Error`-producing site already pushed E2003 (or
    // similar), and `typeck.md §UN-4` absorb semantics mean a downstream
    // unify of `elem_var` with `Idx::ERROR` would NOT bind `elem_var`
    // (Error absorbs without writing to the var). Leaving `elem_var`
    // unbound here is intentional: the result-iterator's element-type
    // ambiguity surfaces only via tests whose `.collect()` target is
    // unbound (e.g., `let _ = …collect()`), which is a separate inference
    // limitation; well-formed callers (`let _: [int] = …collect()`,
    // typed function returns) anchor `.collect()` and the chain resolves
    // cleanly.
    if engine.pool().flags(resolved_inner).has_errors() {
        return;
    }

    if inner_tag.is_iterator() {
        // Success path — propagate inner element to result iter.
        let inner_elem = engine.pool().iterator_elem(resolved_inner);
        let _ = engine.unify().unify(elem_var, inner_elem);
    } else if inner_tag == Tag::Never {
        // Divergent closure body (panic/unreachable/loop). Per
        // `typeck.md §UN-3`, `Never` absorbs unification with any type.
        // Bind elem_var to `Never` so the result iterator's element
        // resolves cleanly to `Never` (uninhabited iterator —
        // semantically consistent: a closure that never returns a value
        // yields a never-producing iterator). Without this bind,
        // `validate_body_types` fires E2005 on the unbound elem_var.
        let _ = engine.unify().unify(elem_var, Idx::NEVER);
    } else if matches!(inner_tag, Tag::Var)
        || inner_tag == Tag::Projection
        || inner_tag == Tag::Error
        || inner_tag == Tag::Alias
    {
        // Silent defer / absorb (no elem_var binding — that's the point of "defer"):
        //   Tag::Var       — STILL unbound after STEP 3a; validate_body_types
        //                    fires E2005 if it survives. matches!(inner_tag, Tag::Var)
        //                    NOT Tag::is_type_variable() — that includes
        //                    BoundVar/RigidVar which the validator silently skips
        //                    (different defer semantics).
        //   Tag::Projection — kept symbolic per typeck.md §UN-8; resolved later
        //                     when receiver concretizes.
        //   Tag::Error     — poison absorbed silently per §UN-4; compound-poison
        //                    case handled by §3c-PRE.
        //   Tag::Alias     — engine.resolve() chases VarState::Link only; per
        //                    types.md §TK-9 user-writable aliases are parsed as
        //                    Newtype, so this case is unreachable from user
        //                    surface today (defensive).
    } else {
        // BoundVar, RigidVar, primitives, structs, sums, tuples, List, Map,
        // Set, Option, Range, Str, Char, Byte, Duration, Size, Ordering, etc.
        // — concrete-enough non-iterator returns that warrant a clear
        // diagnostic now.
        //
        // Use `unsatisfied_bound` (TypeErrorKind::UnsatisfiedBound, E2001)
        // over `mismatch` for two reasons: (a) the precedent in this same
        // file (`resolve_receiver_and_builtin` line 360 for the DEI-required
        // diagnostic) uses the same constructor for an analogous "method
        // requires X-shaped argument" diagnostic; (b) the message renders
        // verbatim, no Pool dependency — the test harness's pool-less
        // `Idx::display_name()` path would render `Iterator<U>` as `<type>`
        // for the Mismatch variant, masking the diagnostic.
        let err = TypeCheckError::unsatisfied_bound(
            closure_span,
            "`flat_map` closure must return an iterator type (`Iterator<U>` or `DoubleEndedIterator<U>`)",
        )
        .with_context(ErrorContext::new(ContextKind::HigherOrderClosureReturn {
            adapter_name: "flat_map",
        }))
        .with_suggestion(suggest_iterator_fix(inner_tag));
        engine.push_error(err);

        // Absorb the result-iterator's element variable so it doesn't
        // surface as a downstream E2005 from `validate_body_types`. Per
        // `typeck.md §UN-4`, unifying with `Tag::Error` absorbs silently —
        // the user sees one precise error at the closure body, not a
        // cascade of confused E2005s pointing at the call site.
        let _ = engine.unify().unify(elem_var, Idx::ERROR);
    }
}

/// Tag-specialized fix suggestion for the `flat_map` closure-return diagnostic.
///
/// Queries `ori_registry` (the SSOT for builtin type behavior per
/// `impl-hygiene.md §SSOT`) to determine whether the actual closure-return
/// tag has a callable `.iter()` method that yields `Iterator<U>`. When yes,
/// the suggestion points users straight at that fix. When no (or when the
/// tag has no registry mapping — type variables, named types, projections,
/// etc.), the suggestion falls back to the generic "wrap in iterator" message.
///
/// Replaces the hardcoded tag set (`List | Set | Map | Str | Range | Option`)
/// flagged by Phase 5 Code TPR Round 0 (codex F4 + gemini F2 + opencode F3 —
/// LEAK:scattered-knowledge violation duplicating registry method knowledge).
pub(crate) fn suggest_iterator_fix(inner_tag: Tag) -> Suggestion {
    use super::super::registry_bridge::tag_to_type_tag;
    let has_iter = tag_to_type_tag(inner_tag)
        .and_then(|tt| ori_registry::find_method(tt, "iter"))
        .is_some();
    let text = if has_iter {
        "this type is not an Iterator; call `.iter()` on it (e.g., `[x, x * 10].iter()`)"
    } else {
        "this type is not an Iterator; `flat_map` requires the closure to return an iterator type"
    };
    Suggestion::text(text, 1)
}

/// Methods that consume an entire iterator and will never terminate on infinite sources.
const INFINITE_CONSUMING_METHODS: &[&str] = &["collect", "count", "fold", "for_each", "to_list"];

/// Methods that are transparent -- they wrap the source but don't bound it.
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
            // .take() bounds the chain -- safe
            if BOUNDING_METHODS.contains(&name) {
                return None;
            }
            // .cycle() is an infinite source
            if name == "cycle" {
                return Some("cycle()".into());
            }
            // Transparent adapters -- keep walking
            if TRANSPARENT_ADAPTERS.contains(&name) {
                return find_infinite_source(engine, arena, *receiver);
            }
            // Unknown method -- stop (conservative: don't warn)
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

        // Anything else -- stop (conservative: don't warn on unknowns)
        _ => None,
    }
}
