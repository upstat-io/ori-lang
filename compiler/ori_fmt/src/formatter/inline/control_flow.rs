//! Inline-Format Control-Flow Constructs
//!
//! `if`/`let`/lambda/`with`/`for`/`loop`/`while` rendering for
//! [`super::Formatter::emit_inline`].

use ori_ir::{BindingPatternId, ExprId, Name, ParamRange, ParsedTypeId, StringLookup};

use super::super::Formatter;

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit `if cond then branch [else branch]` inline.
    pub(super) fn emit_inline_if(
        &mut self,
        cond: ExprId,
        then_branch: ExprId,
        else_branch: ExprId,
    ) {
        self.ctx.emit("if ");
        self.emit_inline(cond);
        self.ctx.emit(" then ");
        self.emit_inline(then_branch);
        if else_branch.is_present() {
            self.ctx.emit(" else ");
            self.emit_inline(else_branch);
        }
    }

    /// Emit `let pattern[: ty] = init` inline — preserve type annotation
    /// per Annex D. Per spec: mutable is default, `$` prefix for immutable.
    /// The `$` prefix is emitted by `emit_binding_pattern()`, not here.
    pub(super) fn emit_inline_let(
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
        self.ctx.emit(" = ");
        self.emit_inline(init);
    }

    /// Emit a lambda inline — render through the lambda emit-shape SSOT
    /// `width::lambda::needs_parens_for_lambda`. Param/`ret_ty` type
    /// annotations preserved per Annex D `typed_lambda`.
    pub(super) fn emit_inline_lambda(
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

        self.ctx.emit(" -> ");
        if ret_ty.is_valid() {
            self.emit_type(self.arena.get_parsed_type(ret_ty));
            self.ctx.emit(" = ");
        }
        self.emit_inline(body);
    }

    /// Emit `with Capability = provider in body` inline.
    pub(super) fn emit_inline_with_capability(
        &mut self,
        capability: Name,
        provider: ExprId,
        body: ExprId,
    ) {
        self.ctx.emit("with ");
        self.ctx.emit(self.interner.lookup(capability));
        self.ctx.emit(" = ");
        self.emit_inline(provider);
        self.ctx.emit(" in ");
        self.emit_inline(body);
    }

    /// Emit a `for` loop inline.
    pub(super) fn emit_inline_for(
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
        self.emit_iter_inline(iter);
        if guard.is_present() {
            self.ctx.emit(" if ");
            self.emit_inline(guard);
        }
        if is_yield {
            self.ctx.emit(" yield ");
        } else {
            self.ctx.emit(" do ");
        }
        self.emit_inline(body);
    }

    /// Emit `loop(body)` / `loop:label(body)` inline.
    pub(super) fn emit_inline_loop(&mut self, label: Name, body: ExprId) {
        self.ctx.emit("loop");
        if label != Name::EMPTY {
            self.ctx.emit(":");
            self.ctx.emit(self.interner.lookup(label));
        }
        self.ctx.emit(" ");
        self.emit_inline(body);
    }

    /// Emit `while cond do body` inline.
    pub(super) fn emit_inline_while(&mut self, label: Name, cond: ExprId, body: ExprId) {
        self.ctx.emit("while");
        if label != Name::EMPTY {
            self.ctx.emit(":");
            self.ctx.emit(self.interner.lookup(label));
        }
        self.ctx.emit(" ");
        self.emit_inline(cond);
        self.ctx.emit(" do ");
        self.emit_inline(body);
    }
}
