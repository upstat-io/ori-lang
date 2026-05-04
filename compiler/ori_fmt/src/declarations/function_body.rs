//! Shared body-emit dispatch for function-shaped declarations.
//!
//! Single canonical home for the Annex D §671 "block bodies always stack"
//! rule. All declaration kinds that emit a function-shaped body (top-level
//! `@fn`, impl methods, def-impl methods, extension methods, default trait
//! methods, test bodies) call this helper instead of `Formatter::format`
//! directly, so the always-stacked rule lives in exactly one place.
//!
//! Spec: annex-d-formatting.md §180,199,671 — function-body blocks are
//! ALWAYS stacked regardless of measured width AND regardless of declaration
//! nesting depth. Nested non-function blocks (inside let/for/if) follow
//! normal width-based breaking and are not the helper's concern.

use ori_ir::{ExprArena, ExprId, ExprKind, StringLookup};

use crate::formatter::Formatter;

/// Emit a function-declaration body. Block bodies are FORCED to broken
/// (stacked) form per Annex D §671; all other body shapes delegate to
/// `Formatter::format` which preserves the existing inline-vs-broken decision
/// tree (width fit, always-stacked-construct list, etc.).
///
/// `fmt` is the per-body sub-formatter that the caller has already prepared
/// via `Formatter::with_config(...).with_indent_level(...).with_starting_column(...)`.
/// `arena` is passed explicitly because `Formatter` keeps its `arena` field
/// private; the caller already holds an `&ExprArena`, so passing it adds zero
/// API surface to `Formatter`.
///
/// The caller is responsible for emitting the `=` separator BEFORE invoking
/// this helper and for extracting + re-emitting `expr_formatter.ctx.as_str()`
/// AFTER. The helper subsumes ONLY the "is this an always-stacked block
/// body?" decision.
pub(crate) fn emit_function_block_body_stacked<I: StringLookup>(
    fmt: &mut Formatter<'_, I>,
    arena: &ExprArena,
    body: ExprId,
) {
    let body_kind = &arena.get_expr(body).kind;
    if matches!(body_kind, ExprKind::Block { .. }) {
        fmt.format_broken(body);
    } else {
        fmt.format(body);
    }
}
