//! Simple vs complex item detection.
//!
//! Simple items (literals, identifiers) can pack multiple per line.
//! Complex items (structs, calls, nested) go one per line.

use ori_ir::{ExprArena, ExprId, ExprKind};

/// Check if an expression is a "simple" item for packing purposes.
///
/// Simple items can pack multiple per line when a list breaks.
/// Complex items always go one per line.
///
/// # Spec Reference
///
/// Lines 225-242: Simple = literals, identifiers
///
/// # Simple Items
///
/// - Integer literals: `42`, `1_000`
/// - Float literals: `3.14`
/// - String literals: `"hello"`
/// - Char literals: `'a'`
/// - Boolean literals: `true`, `false`
/// - Duration literals: `100ms`
/// - Size literals: `4kb`
/// - Identifiers: `foo`, `bar`
/// - None/void: `None`, `void`
///
/// # Complex Items
///
/// - Function calls: `foo()`
/// - Method calls: `x.method()`
/// - Struct literals: `Point { x: 1, y: 2 }`
/// - Nested collections: `[[1, 2], [3, 4]]`
/// - Binary expressions: `a + b`
/// - Any other compound expression
pub fn is_simple_item(arena: &ExprArena, expr_id: ExprId) -> bool {
    let expr = arena.get_expr(expr_id);

    match &expr.kind {
        // Simple items: literals, identifiers, none, unit
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::String(_)
        | ExprKind::Char(_)
        | ExprKind::Bool(_)
        | ExprKind::Duration { .. }
        | ExprKind::Size { .. }
        | ExprKind::Ident(_)
        | ExprKind::None
        | ExprKind::Unit => true,

        // Unit tuple is simple (empty tuple)
        ExprKind::Tuple(elements) if elements.is_empty() => true,

        // Everything else is complex
        _ => false,
    }
}

/// Check if all items in a list are simple.
///
/// Used to determine if a list can use `FitOrPackMultiple` packing.
pub fn all_items_simple(arena: &ExprArena, items: &[ExprId]) -> bool {
    items.iter().all(|&id| is_simple_item(arena, id))
}

/// Determine if a list should use simple or complex packing.
///
/// Returns `ConstructKind::ListSimple` if all items are simple,
/// otherwise `ConstructKind::ListComplex`.
pub fn list_construct_kind(arena: &ExprArena, items: &[ExprId]) -> super::ConstructKind {
    if all_items_simple(arena, items) {
        super::ConstructKind::ListSimple
    } else {
        super::ConstructKind::ListComplex
    }
}
