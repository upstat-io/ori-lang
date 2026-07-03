//! Broken-Format Control-Flow Constructs
//!
//! `if`/`let`/lambda/`with`/`for`/`while` rendering for [`super::Formatter::emit_broken`],
//! split out of `broken/mod.rs` to keep the dispatch match's sibling file
//! under the workspace file-size limit.

use ori_ir::{BindingPatternId, ExprId, Name, ParamRange, ParsedTypeId, StringLookup};

use super::super::Formatter;
use crate::width::ALWAYS_STACKED;

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit `if` in broken format, breaking at `else` while keeping
    /// `else if` chains flat. Checks whether the initial `if cond then
    /// branch` segment fits on the current line before deciding whether to
    /// keep it inline or break the `then` branch onto its own line.
    pub(super) fn emit_broken_if(
        &mut self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
    ) {
        // Calculate width of initial segment: "if " + cond + " then " + branch
        let cond_width = self.width_calc.width(cond);
        let then_width = self.width_calc.width(then_branch);

        // Check if the initial segment fits
        // 3 = "if ", 6 = " then "
        let initial_fits = cond_width != ALWAYS_STACKED
            && then_width != ALWAYS_STACKED
            && self.ctx.fits(3 + cond_width + 6 + then_width);

        if initial_fits {
            // Emit "if cond then branch" inline, then break for else
            self.ctx.emit("if ");
            self.emit_inline(cond);
            self.ctx.emit(" then ");
            self.emit_inline(then_branch);
        } else {
            // Initial segment is too long, break the then_branch to new line
            self.ctx.emit("if ");
            self.format(cond);
            self.ctx.emit(" then");
            self.ctx.emit_newline();
            self.ctx.indent();
            self.ctx.emit_indent();
            self.format(then_branch);
            self.ctx.dedent();
        }

        if else_branch.is_present() {
            self.emit_else_branch(else_branch);
        }
    }

    /// Emit an else branch, handling else-if chains with proper line breaking.
    ///
    /// For chained else-if, each else clause goes on a new line, with the
    /// `else if cond then branch` together on that line:
    /// ```text
    /// if cond1 then branch1
    /// else if cond2 then branch2
    /// else if cond3 then branch3
    /// else branch4
    /// ```
    fn emit_else_branch(&mut self, else_id: ExprId) {
        self.ctx.emit_newline_indent();
        self.ctx.emit("else ");

        let else_expr = self.arena.get_expr(else_id);
        if let ori_ir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } = &else_expr.kind
        {
            // else-if chain: check if "if cond then branch" fits on this line
            let cond_width = self.width_calc.width(*cond);
            let then_width = self.width_calc.width(*then_branch);

            // Check if the segment fits: "if " + cond + " then " + branch
            let segment_fits = cond_width != ALWAYS_STACKED
                && then_width != ALWAYS_STACKED
                && self.ctx.fits(3 + cond_width + 6 + then_width);

            if segment_fits {
                // Emit "if cond then branch" inline
                self.ctx.emit("if ");
                self.emit_inline(*cond);
                self.ctx.emit(" then ");
                self.emit_inline(*then_branch);
            } else {
                // Segment too long, break the then_branch to new line
                self.ctx.emit("if ");
                self.format(*cond);
                self.ctx.emit(" then");
                self.ctx.emit_newline();
                self.ctx.indent();
                self.ctx.emit_indent();
                self.format(*then_branch);
                self.ctx.dedent();
            }

            if else_branch.is_present() {
                self.emit_else_branch(*else_branch);
            }
        } else {
            // Final else branch
            self.format(else_id);
        }
    }

    /// Emit `let` in broken format — preserve type annotation per Annex D.
    ///
    /// Per spec: mutable is default, `$` prefix for immutable. The `$`
    /// prefix is emitted by `emit_binding_pattern()`, not here.
    pub(super) fn emit_broken_let(
        &mut self,
        pattern: BindingPatternId,
        ty: ParsedTypeId,
        init: ExprId,
    ) {
        self.ctx.emit("let ");
        let pat = self.arena.get_binding_pattern(pattern);
        self.emit_binding_pattern(pat);
        if ty.is_valid() {
            self.ctx.emit(": ");
            self.emit_type(self.arena.get_parsed_type(ty));
        }
        self.ctx.emit(" =");
        self.ctx.emit_newline();
        self.ctx.indent();
        self.ctx.emit_indent();
        self.format(init);
        self.ctx.dedent();
    }

    /// Emit a lambda with its body on a new line — render through the
    /// lambda emit-shape SSOT `width::lambda::needs_parens_for_lambda`.
    /// Param/`ret_ty` type annotations preserved per Annex D `typed_lambda`.
    pub(super) fn emit_broken_lambda(
        &mut self,
        params: ParamRange,
        ret_ty: ParsedTypeId,
        body: ExprId,
    ) {
        let params_list = self.arena.get_params(params);
        let needs_parens = crate::width::lambda::needs_parens_for_lambda(params_list, ret_ty);

        if needs_parens {
            self.ctx.emit("(");
        }
        for (i, param) in params_list.iter().enumerate() {
            if i > 0 {
                self.ctx.emit(", ");
            }
            self.ctx.emit(self.interner.lookup(param.name));
            if let Some(ref ty) = param.ty {
                self.ctx.emit(": ");
                self.emit_type(ty);
            }
        }
        if needs_parens {
            self.ctx.emit(")");
        }

        self.ctx.emit(" ->");
        if ret_ty.is_valid() {
            self.ctx.emit(" ");
            self.emit_type(self.arena.get_parsed_type(ret_ty));
            self.ctx.emit(" =");
        }
        self.ctx.emit_newline();
        self.ctx.indent();
        self.ctx.emit_indent();
        self.format(body);
        self.ctx.dedent();
    }

    /// Emit `with Capability = provider in body` in broken format — body on
    /// a new line.
    pub(super) fn emit_broken_with_capability(
        &mut self,
        capability: Name,
        provider: ExprId,
        body: ExprId,
    ) {
        self.ctx.emit("with ");
        self.ctx.emit(self.interner.lookup(capability));
        self.ctx.emit(" = ");
        self.format(provider);
        self.ctx.emit(" in");
        self.ctx.emit_newline();
        self.ctx.indent();
        self.ctx.emit_indent();
        self.format(body);
        self.ctx.dedent();
    }

    /// Emit a `for` loop in broken format — body on a new line if needed.
    pub(super) fn emit_broken_for(
        &mut self,
        label: Name,
        pattern: BindingPatternId,
        iter: ExprId,
        guard: ExprId,
        body: ExprId,
        is_yield: bool,
    ) {
        self.ctx.emit("for");
        if label != Name::EMPTY {
            self.ctx.emit(":");
            self.ctx.emit(self.interner.lookup(label));
        }
        self.ctx.emit(" ");
        self.emit_for_binding_pattern_id(pattern);
        self.ctx.emit(" in ");
        self.format_iter(iter);
        if guard.is_present() {
            self.ctx.emit(" if ");
            self.format(guard);
        }
        if is_yield {
            self.ctx.emit(" yield");
        } else {
            self.ctx.emit(" do");
        }
        self.ctx.emit_newline();
        self.ctx.indent();
        self.ctx.emit_indent();
        self.format(body);
        self.ctx.dedent();
    }

    /// Emit `while` in broken format. A block body opens inline after `do`;
    /// a non-block body goes on a new line.
    pub(super) fn emit_broken_while(&mut self, label: Name, cond: ExprId, body: ExprId) {
        self.ctx.emit("while");
        if label != Name::EMPTY {
            self.ctx.emit(":");
            self.ctx.emit(self.interner.lookup(label));
        }
        self.ctx.emit(" ");
        self.format(cond);
        self.ctx.emit(" do");
        if matches!(
            self.arena.get_expr(body).kind,
            ori_ir::ExprKind::Block { .. }
        ) {
            // `do { ... }` — block renders its own brace inline + single indent,
            // matching the canonical `while c do { ... }` source form. Avoids the
            // double-indent that a forced newline + indent would introduce.
            self.ctx.emit(" ");
            self.format(body);
        } else {
            self.ctx.emit_newline();
            self.ctx.indent();
            self.ctx.emit_indent();
            self.format(body);
            self.ctx.dedent();
        }
    }
}
