//! Test Definition Formatting
//!
//! Formatting for test function declarations.

use crate::formatter::emit_escaped_str;
use ori_ir::ast::items::{ExpectedError, TestBackend, TestDef};
use ori_ir::{ExprId, StringLookup};

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

        // One #skip(backend: ..., reason: ...) attribute per backend-conditional skip.
        for skip in &test.skip_backends {
            self.ctx.emit("#skip(backend: \"");
            self.ctx.emit(match skip.backend {
                TestBackend::Interpreter => "interpreter",
                TestBackend::Llvm => "llvm",
            });
            self.ctx.emit("\", reason: ");
            let s = self.interner.lookup(skip.reason);
            emit_escaped_str(&mut self.ctx, s);
            self.ctx.emit(")");
            self.ctx.emit_newline();
        }

        // One #compile_fail attribute per ExpectedError (each entry parsed one).
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
        self.ctx.emit("#compile_fail(");
        if let (Some(msg), None, None, None) = (err.message, err.code, err.line, err.column) {
            let s = self.interner.lookup(msg);
            emit_escaped_str(&mut self.ctx, s);
            self.ctx.emit(")");
            return;
        }
        let mut first = true;
        if let Some(code) = err.code {
            self.emit_join_sep(&mut first);
            self.ctx.emit("code: ");
            let s = self.interner.lookup(code);
            emit_escaped_str(&mut self.ctx, s);
        }
        if let Some(msg) = err.message {
            self.emit_join_sep(&mut first);
            self.ctx.emit("message: ");
            let s = self.interner.lookup(msg);
            emit_escaped_str(&mut self.ctx, s);
        }
        if let Some(line) = err.line {
            self.emit_join_sep(&mut first);
            self.ctx.emit(format!("line: {line}"));
        }
        if let Some(column) = err.column {
            self.emit_join_sep(&mut first);
            self.ctx.emit(format!("column: {column}"));
        }
        self.ctx.emit(")");
    }

    /// Format a test body. Delegates the inline-vs-newline-vs-internal-break
    /// decision to the shared `emit_expr_body` (SSOT in `function_body`).
    /// `allow_force_newline = false`: test bodies keep their existing layout
    /// (no if/for force-newline) and gain ONLY the over-width-head break.
    fn format_test_body(&mut self, body: ExprId) {
        self.emit_expr_body(body, false);
    }
}
