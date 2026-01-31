//! Test Definition Formatting
//!
//! Formatting for test function declarations.

use crate::formatter::Formatter;
use crate::width::ALWAYS_STACKED;
use ori_ir::ast::items::TestDef;
use ori_ir::{ExprId, StringLookup};

use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format a test definition including attributes and body.
    pub fn format_test(&mut self, test: &TestDef) {
        // Skip attribute
        if let Some(reason) = test.skip_reason {
            self.ctx.emit("#skip(\"");
            self.ctx.emit(self.interner.lookup(reason));
            self.ctx.emit("\")");
            self.ctx.emit_newline();
        }

        // Compile fail attribute
        if !test.expected_errors.is_empty() {
            self.ctx.emit("#compile_fail");
            // Only emit details if there's a message
            if let Some(first_err) = test.expected_errors.first() {
                if let Some(msg) = first_err.message {
                    self.ctx.emit("(\"");
                    self.ctx.emit(self.interner.lookup(msg));
                    self.ctx.emit("\")");
                }
            }
            self.ctx.emit_newline();
        }

        // Fail attribute
        if let Some(expected) = test.fail_expected {
            self.ctx.emit("#fail(\"");
            self.ctx.emit(self.interner.lookup(expected));
            self.ctx.emit("\")");
            self.ctx.emit_newline();
        }

        // Test name
        self.ctx.emit("@");
        self.ctx.emit(self.interner.lookup(test.name));

        // Targets (only if there are any - free-floating tests have no targets clause)
        if !test.targets.is_empty() {
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

    /// Format a test body, breaking to new line if it doesn't fit after `= `.
    fn format_test_body(&mut self, body: ExprId) {
        // Calculate body width to determine if it fits inline
        let body_width = self.width_calc.width(body);

        // Check if body fits after " = " on current line
        let space_after_eq = 3; // " = "
        let fits_inline =
            body_width != ALWAYS_STACKED && self.ctx.fits(space_after_eq + body_width);

        if fits_inline {
            // Inline: " = body"
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        } else {
            // Body doesn't fit - always-stacked constructs (run/try/match) stay on same line
            // and break internally. Other constructs also stay on same line.
            self.ctx.emit(" = ");
            let current_column = self.ctx.column();
            let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
                .with_starting_column(current_column);
            expr_formatter.format(body);
            let body_output = expr_formatter.ctx.as_str().trim_end();
            self.ctx.emit(body_output);
        }
    }
}
