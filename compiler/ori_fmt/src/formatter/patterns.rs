//! Pattern Formatting
//!
//! Methods for emitting match patterns and binding patterns.

use ori_ir::{BindingPattern, MatchPattern, StringLookup};

use super::Formatter;

impl<I: StringLookup> Formatter<'_, I> {
    /// Emit a match pattern.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive MatchPattern formatting dispatch"
    )]
    pub(super) fn emit_match_pattern(&mut self, pattern: &MatchPattern) {
        match pattern {
            MatchPattern::Wildcard => self.ctx.emit("_"),
            MatchPattern::Binding(name) => {
                self.ctx.emit(self.interner.lookup(*name));
            }
            MatchPattern::Literal(expr_id) => {
                self.emit_inline(*expr_id);
            }
            MatchPattern::Variant { name, inner } => {
                self.ctx.emit(self.interner.lookup(*name));
                let inner_list = self.arena.get_match_pattern_list(*inner);
                if !inner_list.is_empty() {
                    self.ctx.emit("(");
                    for (i, pat_id) in inner_list.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        let pat = self.arena.get_match_pattern(*pat_id);
                        self.emit_match_pattern(pat);
                    }
                    self.ctx.emit(")");
                }
            }
            MatchPattern::Struct { fields, rest } => {
                self.ctx.emit("{ ");
                for (i, (field_name, pat_opt)) in fields.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit(self.interner.lookup(*field_name));
                    if let Some(pat_id) = pat_opt {
                        self.ctx.emit(": ");
                        let pat = self.arena.get_match_pattern(*pat_id);
                        self.emit_match_pattern(pat);
                    }
                }
                if *rest {
                    if !fields.is_empty() {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("..");
                }
                self.ctx.emit(" }");
            }
            MatchPattern::Tuple(items) => {
                let items_list = self.arena.get_match_pattern_list(*items);
                self.ctx.emit("(");
                for (i, pat_id) in items_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let pat = self.arena.get_match_pattern(*pat_id);
                    self.emit_match_pattern(pat);
                }
                // Single-element tuples need trailing comma: (x,) vs (x)
                if items_list.len() == 1 {
                    self.ctx.emit(",");
                }
                self.ctx.emit(")");
            }
            MatchPattern::List { elements, rest } => {
                let elements_list = self.arena.get_match_pattern_list(*elements);
                self.ctx.emit("[");
                for (i, pat_id) in elements_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    let pat = self.arena.get_match_pattern(*pat_id);
                    self.emit_match_pattern(pat);
                }
                if let Some(rest_name) = rest {
                    if !elements_list.is_empty() {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("..");
                    self.ctx.emit(self.interner.lookup(*rest_name));
                }
                self.ctx.emit("]");
            }
            MatchPattern::Range {
                start,
                end,
                inclusive,
            } => {
                if let Some(s) = start {
                    self.emit_inline(*s);
                }
                if *inclusive {
                    self.ctx.emit("..=");
                } else {
                    self.ctx.emit("..");
                }
                if let Some(e) = end {
                    self.emit_inline(*e);
                }
            }
            MatchPattern::Or(patterns) => {
                let patterns_list = self.arena.get_match_pattern_list(*patterns);
                for (i, pat_id) in patterns_list.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(" | ");
                    }
                    let pat = self.arena.get_match_pattern(*pat_id);
                    self.emit_match_pattern(pat);
                }
            }
            MatchPattern::At { name, pattern } => {
                self.ctx.emit(self.interner.lookup(*name));
                self.ctx.emit(" @ ");
                let pat = self.arena.get_match_pattern(*pattern);
                self.emit_match_pattern(pat);
            }
        }
    }

    /// Emit a binding pattern (for `let` bindings — emits `$` for immutable names).
    pub(super) fn emit_binding_pattern(&mut self, pattern: &BindingPattern) {
        self.emit_binding_pattern_inner(pattern, false);
    }

    /// Emit a for-loop binding pattern — suppresses `$` prefix.
    ///
    /// Per spec (§05-variables.md), for-loop variables are inherently immutable.
    /// The `$` prefix is a user annotation only meaningful in `let` context, so
    /// the formatter never emits it for for-loop patterns.
    pub(super) fn emit_for_binding_pattern_id(&mut self, id: ori_ir::BindingPatternId) {
        let pattern = self.arena.get_binding_pattern(id);
        self.emit_binding_pattern_inner(pattern, true);
    }

    /// Shared binding pattern emitter.
    ///
    /// When `suppress_dollar` is true, `$` is not emitted for immutable names
    /// (used for for-loop patterns where immutability is structural).
    fn emit_binding_pattern_inner(&mut self, pattern: &BindingPattern, suppress_dollar: bool) {
        match pattern {
            BindingPattern::Name { name, mutable } => {
                if !suppress_dollar && mutable.is_immutable() {
                    self.ctx.emit("$");
                }
                self.ctx.emit(self.interner.lookup(*name));
            }
            BindingPattern::Tuple(items) => {
                self.ctx.emit("(");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_binding_pattern_inner(item, suppress_dollar);
                }
                // Single-element tuples need trailing comma: (x,) vs (x)
                if items.len() == 1 {
                    self.ctx.emit(",");
                }
                self.ctx.emit(")");
            }
            BindingPattern::Struct { fields } => {
                self.ctx.emit("{ ");
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    // Shorthand with $ prefix: { $x }
                    if !suppress_dollar && field.mutable.is_immutable() && field.pattern.is_none() {
                        self.ctx.emit("$");
                    }
                    self.ctx.emit(self.interner.lookup(field.name));
                    if let Some(pat) = &field.pattern {
                        self.ctx.emit(": ");
                        self.emit_binding_pattern_inner(pat, suppress_dollar);
                    }
                }
                self.ctx.emit(" }");
            }
            BindingPattern::List { elements, rest } => {
                self.ctx.emit("[");
                for (i, item) in elements.iter().enumerate() {
                    if i > 0 {
                        self.ctx.emit(", ");
                    }
                    self.emit_binding_pattern_inner(item, suppress_dollar);
                }
                if let Some((rest_name, rest_mut)) = rest {
                    if !elements.is_empty() {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("..");
                    if !suppress_dollar && rest_mut.is_immutable() {
                        self.ctx.emit("$");
                    }
                    self.ctx.emit(self.interner.lookup(*rest_name));
                }
                self.ctx.emit("]");
            }
            BindingPattern::Wildcard => {
                self.ctx.emit("_");
            }
        }
    }
}
