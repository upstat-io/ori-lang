// Low-level output helpers for C code generation
// Handles output buffer, indentation, and line emission

use super::CodeGen;

impl CodeGen {
    #[allow(dead_code)] // May be used for future incremental code generation
    pub(super) fn emit(&mut self, s: &str) {
        self.output.push_str(s);
    }

    pub(super) fn emit_line(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    pub(super) fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    pub(super) fn indent(&mut self) {
        self.indent += 1;
    }

    pub(super) fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }
}
