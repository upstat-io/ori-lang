//! Test Definition Formatting
//!
//! Formatting for test function declarations.

use crate::formatter::{emit_escaped_str, Formatter};
use crate::width::ALWAYS_STACKED;
use ori_ir::ast::items::{ExpectedError, TestDef};
use ori_ir::{ExprId, StringLookup};

use super::function_body;
use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a test definition including attributes and body.
    pub fn format_test(&mut self, test: &TestDef) {
        // Skip attribute
        if let Some(reason) = test.skip_reason {
            self.ctx.emit("#skip(");
            let s = self.interner.lookup(reason);
            emit_escaped_str(&mut self.ctx, s);
            self.ctx.emit(")");
            self.ctx.emit_newline();
        }

        // Compile-fail attributes — one per ExpectedError, faithfully
        // round-tripping the keyed fields (each entry parsed one attribute).
        for err in &test.expected_errors {
            self.emit_compile_fail_attr(err);
            self.ctx.emit_newline();
        }

        // Fail attribute
        if let Some(expected) = test.fail_expected {
            self.ctx.emit("#fail(");
            let s = self.interner.lookup(expected);
            emit_escaped_str(&mut self.ctx, s);
            self.ctx.emit(")");
            self.ctx.emit_newline();
        }

        // Test name
        self.ctx.emit("@");
        self.ctx.emit(self.interner.lookup(test.name));

        // Targets: attached tests list specific targets, floating tests use `_`
        if test.targets.is_empty() {
            self.ctx.emit(" tests _");
        } else {
            for target in &test.targets {
                self.ctx.emit(" tests @");
                self.ctx.emit(self.interner.lookup(*target));
            }
        }

        // Parameters
        self.ctx.emit(" ");
        self.format_params(test.params);

        // Return type
        if let Some(ref ret_ty) = test.return_ty {
            self.ctx.emit(" -> ");
            format_parsed_type(ret_ty, self.arena, self.interner, &mut self.ctx);
        }

        // Body - use similar logic to format_function_body
        self.format_test_body(test.body);
    }

    /// Emit a single `#compile_fail(...)` attribute, faithfully round-tripping
    /// the keyed `ExpectedError` fields. Shorthand `#compile_fail("msg")` is used
    /// only when `message` is the sole set field; otherwise the keyed form in the
    /// canonical order `code, message, line, column` (only set fields emitted).
    fn emit_compile_fail_attr(&mut self, err: &ExpectedError) {
        let message_only = err.message.is_some()
            && err.code.is_none()
            && err.line.is_none()
            && err.column.is_none();
        self.ctx.emit("#compile_fail(");
        if message_only {
            if let Some(msg) = err.message {
                let s = self.interner.lookup(msg);
                emit_escaped_str(&mut self.ctx, s);
            }
            self.ctx.emit(")");
            return;
        }
        let mut first = true;
        if let Some(code) = err.code {
            self.ctx.emit("code: ");
            let s = self.interner.lookup(code);
            emit_escaped_str(&mut self.ctx, s);
            first = false;
        }
        if let Some(msg) = err.message {
            if !first {
                self.ctx.emit(", ");
            }
            self.ctx.emit("message: ");
            let s = self.interner.lookup(msg);
            emit_escaped_str(&mut self.ctx, s);
            first = false;
        }
        if let Some(line) = err.line {
            if !first {
                self.ctx.emit(", ");
            }
            self.ctx.emit(&format!("line: {line}"));
            first = false;
        }
        if let Some(column) = err.column {
            if !first {
                self.ctx.emit(", ");
            }
            self.ctx.emit(&format!("column: {column}"));
        }
        self.ctx.emit(")");
    }

    /// Format a test body, breaking to new line if it doesn't fit after `= `.
    ///
    /// Per grammar: expression bodies require trailing `;` but block bodies
    /// ending with `}` do not.
    fn format_test_body(&mut self, body: ExprId) {
        // Calculate body width to determine if it fits inline
        let body_width = self.width_calc.width(body);

        // Check if body fits after " = " on current line
        let space_after_eq = 3; // " = "
        let fits_inline =
            body_width != ALWAYS_STACKED && self.ctx.fits(space_after_eq + body_width);

        let ends_with_brace;

        if fits_inline {
            // Inline: " = body"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter =
                Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                    .with_starting_column(current_column);
            // Spec: annex-d-formatting.md §671 — function-body blocks always stacked
            function_body::emit_function_block_body_stacked(&mut expr_formatter, self.arena, body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            ends_with_brace = body_output.ends_with('}');
            self.ctx.emit(body_output);
        } else {
            // Body doesn't fit - always-stacked constructs (run/try/match) stay on same line
            // and break internally. Other constructs also stay on same line.
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter =
                Formatter::with_config(self.arena, self.interner, *self.ctx.config())
                    .with_starting_column(current_column);
            // Spec: annex-d-formatting.md §671 — function-body blocks always stacked
            function_body::emit_function_block_body_stacked(&mut expr_formatter, self.arena, body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            ends_with_brace = body_output.ends_with('}');
            self.ctx.emit(body_output);
        }

        // Trailing semicolon for non-block expression bodies
        if !ends_with_brace {
            self.ctx.emit(";");
        }
    }
}
