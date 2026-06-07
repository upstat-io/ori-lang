//! Infinite-iterator consumption detection (W2001).

use ori_ir::{ExprArena, ExprId, ExprKind, Span};

use super::super::super::InferEngine;
use crate::TypeCheckWarning;

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
pub(super) fn check_infinite_iterator_consumed(
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
