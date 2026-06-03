//! Shared body-emit dispatch for function-shaped declarations.
//!
//! Single canonical home for the Annex D §671 "block bodies always stack"
//! rule AND the function/test expression-body layout decision. All declaration
//! kinds that emit a function-shaped body (top-level `@fn`, impl methods,
//! def-impl methods, extension methods, default trait methods, test bodies) emit
//! through this module so the inline-vs-newline-vs-internal-break decision lives
//! in exactly one place.
//!
//! Spec: annex-d-formatting.md §180,199,671 (block bodies always stacked) +
//! §Functions/§Return Type (expression body breaks to the next line when the
//! full declaration exceeds the width).

use ori_ir::{ExprArena, ExprId, ExprKind, StringLookup};

use crate::formatter::Formatter;
use crate::width::ALWAYS_STACKED;

use super::ModuleFormatter;

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

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Emit an expression body after the signature, choosing the layout. SSOT
    /// consumed by both `format_function_body` and `format_test_body`.
    ///
    /// Layout precedence:
    /// 1. inline-head `= body` when the body fits after `= `.
    /// 2. break-to-newline `=\n    body` when EITHER `allow_force_newline` and the
    ///    body is a force-newline kind (`should_break_body_to_newline`) OR the
    ///    inline-head first line overflows the width and the newline layout fits.
    /// 3. inline-head with internal breaking otherwise.
    ///
    /// Emits the trailing `;` for non-block expression bodies.
    pub(crate) fn emit_expr_body(&mut self, body: ExprId, allow_force_newline: bool) {
        let body_width = self.width_calc.width(body);
        // " = "
        let space_after_eq = 3;
        let fits_inline =
            body_width != ALWAYS_STACKED && self.ctx.fits(space_after_eq + body_width);

        let ends_with_brace;

        if fits_inline {
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter =
                Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                    .with_starting_column(current_column);
            emit_function_block_body_stacked(&mut expr_formatter, self.arena, body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            ends_with_brace = body_output.ends_with('}');
            self.ctx.emit(body_output);
        } else {
            let force_newline =
                allow_force_newline && self.should_break_body_to_newline(body, body_width);
            let break_to_newline = force_newline || self.inline_head_overflows(body);

            if break_to_newline {
                self.ctx.emit(" =");
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                        .with_indent_level(1)
                        .with_starting_column(self.ctx.column());
                expr_formatter.format_broken(body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                ends_with_brace = body_output.ends_with('}');
                self.ctx.emit(body_output);
                self.ctx.dedent();
            } else {
                self.ctx.emit(" = ");
                let current_column = self.ctx.column();
                let mut expr_formatter =
                    Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                        .with_starting_column(current_column);
                emit_function_block_body_stacked(&mut expr_formatter, self.arena, body);
                let body_output = expr_formatter.ctx.as_str().trim_end();
                ends_with_brace = body_output.ends_with('}');
                self.ctx.emit(body_output);
            }
        }

        if !ends_with_brace {
            self.ctx.emit(";");
        }
    }

    /// Whether emitting `body` inline-head would overflow the line width on its
    /// first line AND breaking the body to its own indented line makes that
    /// first line fit. Returns false for bodies whose first line overflows even
    /// at indent level 1 (breaking would not help) — they stay inline-head.
    // Spec: annex-d-formatting.md §Return Type.
    fn inline_head_overflows(&self, body: ExprId) -> bool {
        // Block bodies keep their `= {` K&R brace on the signature line per Annex D
        // §671; breaking the brace to its own line is never correct.
        if matches!(self.arena.get_expr(body).kind, ExprKind::Block { .. }) {
            return false;
        }

        let max_width = self.ctx.config().max_width;
        let signature_column = self.ctx.column();
        // Breaking the body off leaves the signature on its own line as `<sig> =`.
        // If THAT already overflows (e.g. an intrinsically-wide return type),
        // breaking the body does not resolve the overflow — keep it inline.
        if signature_column + 2 > max_width {
            return false;
        }
        // " = "
        let projected_column = signature_column + 3;

        let mut inline_formatter =
            Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                .with_starting_column(projected_column);
        emit_function_block_body_stacked(&mut inline_formatter, self.arena, body);
        let inline_first_line = first_line_width(inline_formatter.ctx.as_str());
        if projected_column + inline_first_line <= max_width {
            return false;
        }

        let indent_width = self.ctx.config().indent_size;
        let mut newline_formatter =
            Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                .with_indent_level(1)
                .with_starting_column(indent_width);
        newline_formatter.format_broken(body);
        let newline_first_line = first_line_width(newline_formatter.ctx.as_str());
        indent_width + newline_first_line <= max_width
    }
}

/// Character width of the first line of `s` (0 when empty).
fn first_line_width(s: &str) -> usize {
    s.lines().next().map_or(0, str::len)
}
