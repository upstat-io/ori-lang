//! Formatting Context
//!
//! Tracks state during formatting: column position, indentation level, and output.
//! Provides methods for emitting text while maintaining state.

use crate::emitter::{Emitter, StringEmitter};

/// Maximum line width before breaking.
pub const MAX_LINE_WIDTH: usize = 100;

/// Spaces per indentation level.
pub const INDENT_WIDTH: usize = 4;

/// Formatting context that tracks state during output.
///
/// This struct wraps an emitter and maintains:
/// - Current column position (0-indexed)
/// - Current indentation level
///
/// All emit operations update the column position automatically.
pub struct FormatContext<E: Emitter = StringEmitter> {
    emitter: E,
    column: usize,
    indent_level: usize,
}

impl FormatContext<StringEmitter> {
    /// Create a new format context with a string emitter.
    pub fn new() -> Self {
        Self::with_emitter(StringEmitter::new())
    }

    /// Create with pre-allocated capacity for the output buffer.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_emitter(StringEmitter::with_capacity(capacity))
    }
}

impl Default for FormatContext<StringEmitter> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Emitter> FormatContext<E> {
    /// Create a format context with a specific emitter.
    pub fn with_emitter(emitter: E) -> Self {
        Self {
            emitter,
            column: 0,
            indent_level: 0,
        }
    }

    /// Get the current column position (0-indexed).
    pub fn column(&self) -> usize {
        self.column
    }

    /// Get the current indentation level.
    pub fn indent_level(&self) -> usize {
        self.indent_level
    }

    /// Get the current indentation width in spaces.
    pub fn indent_width(&self) -> usize {
        self.indent_level * INDENT_WIDTH
    }

    /// Set the current column position.
    ///
    /// Used when formatting sub-expressions that continue on the same line
    /// as previous content (e.g., function body after `= `).
    pub fn set_column(&mut self, column: usize) {
        self.column = column;
    }

    /// Check if adding `width` characters would exceed the line limit.
    pub fn would_exceed_limit(&self, width: usize) -> bool {
        self.column + width > MAX_LINE_WIDTH
    }

    /// Check if content of `width` would fit on the current line.
    pub fn fits(&self, width: usize) -> bool {
        self.column + width <= MAX_LINE_WIDTH
    }

    /// Emit a text fragment.
    pub fn emit(&mut self, text: &str) {
        self.emitter.emit(text);
        self.column += text.len();
    }

    /// Emit a single space.
    pub fn emit_space(&mut self) {
        self.emitter.emit_space();
        self.column += 1;
    }

    /// Emit a newline and reset column to 0.
    pub fn emit_newline(&mut self) {
        self.emitter.emit_newline();
        self.column = 0;
    }

    /// Emit indentation at the current level and update column.
    pub fn emit_indent(&mut self) {
        self.emitter.emit_indent(self.indent_level);
        self.column = self.indent_level * INDENT_WIDTH;
    }

    /// Emit a newline followed by indentation.
    pub fn emit_newline_indent(&mut self) {
        self.emit_newline();
        self.emit_indent();
    }

    /// Increment indentation level.
    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    /// Decrement indentation level.
    pub fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
    }

    /// Execute a closure with increased indentation.
    ///
    /// Indentation is restored after the closure completes.
    pub fn with_indent<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.indent();
        let result = f(self);
        self.dedent();
        result
    }

    /// Get the underlying emitter.
    pub fn into_emitter(self) -> E {
        self.emitter
    }

    /// Get a reference to the underlying emitter.
    pub fn emitter(&self) -> &E {
        &self.emitter
    }

    /// Get a mutable reference to the underlying emitter.
    pub fn emitter_mut(&mut self) -> &mut E {
        &mut self.emitter
    }
}

impl FormatContext<StringEmitter> {
    /// Get the formatted output.
    pub fn output(self) -> String {
        self.emitter.output()
    }

    /// Get the current output without consuming.
    pub fn as_str(&self) -> &str {
        self.emitter.as_str()
    }

    /// Finalize output with trailing newline handling.
    ///
    /// Trims trailing blank lines and ensures exactly one trailing newline.
    pub fn finalize(mut self) -> String {
        self.emitter.trim_trailing_blank_lines();
        self.emitter.ensure_trailing_newline();
        self.emitter.output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_basic_emit() {
        let mut ctx = FormatContext::new();
        ctx.emit("hello");
        assert_eq!(ctx.column(), 5);
        ctx.emit_space();
        assert_eq!(ctx.column(), 6);
        ctx.emit("world");
        assert_eq!(ctx.column(), 11);
        assert_eq!(ctx.output(), "hello world");
    }

    #[test]
    fn context_newline_resets_column() {
        let mut ctx = FormatContext::new();
        ctx.emit("line1");
        assert_eq!(ctx.column(), 5);
        ctx.emit_newline();
        assert_eq!(ctx.column(), 0);
        ctx.emit("line2");
        assert_eq!(ctx.column(), 5);
        assert_eq!(ctx.output(), "line1\nline2");
    }

    #[test]
    fn context_indentation() {
        let mut ctx = FormatContext::new();
        ctx.emit("level0");
        ctx.emit_newline();

        ctx.indent();
        ctx.emit_indent();
        assert_eq!(ctx.column(), 4);
        ctx.emit("level1");
        ctx.emit_newline();

        ctx.indent();
        ctx.emit_indent();
        assert_eq!(ctx.column(), 8);
        ctx.emit("level2");

        assert_eq!(ctx.output(), "level0\n    level1\n        level2");
    }

    #[test]
    fn context_with_indent_scope() {
        let mut ctx = FormatContext::new();
        assert_eq!(ctx.indent_level(), 0);

        ctx.with_indent(|ctx| {
            assert_eq!(ctx.indent_level(), 1);
            ctx.with_indent(|ctx| {
                assert_eq!(ctx.indent_level(), 2);
            });
            assert_eq!(ctx.indent_level(), 1);
        });

        assert_eq!(ctx.indent_level(), 0);
    }

    #[test]
    fn context_fits_check() {
        let mut ctx = FormatContext::new();
        ctx.emit("x".repeat(90).as_str());

        assert!(ctx.fits(10)); // 90 + 10 = 100
        assert!(!ctx.fits(11)); // 90 + 11 = 101 > 100
    }

    #[test]
    fn context_would_exceed_limit() {
        let mut ctx = FormatContext::new();
        ctx.emit("x".repeat(50).as_str());

        assert!(!ctx.would_exceed_limit(50)); // 50 + 50 = 100
        assert!(ctx.would_exceed_limit(51)); // 50 + 51 = 101
    }

    #[test]
    fn context_finalize() {
        let mut ctx = FormatContext::new();
        ctx.emit("content");
        ctx.emit_newline();
        ctx.emit_newline();
        ctx.emit_newline();

        let output = ctx.finalize();
        assert_eq!(output, "content\n");
    }

    #[test]
    fn context_indent_width() {
        let mut ctx = FormatContext::new();
        assert_eq!(ctx.indent_width(), 0);
        ctx.indent();
        assert_eq!(ctx.indent_width(), 4);
        ctx.indent();
        assert_eq!(ctx.indent_width(), 8);
    }

    #[test]
    fn context_newline_indent() {
        let mut ctx = FormatContext::new();
        ctx.indent();
        ctx.emit("first");
        ctx.emit_newline_indent();
        ctx.emit("second");

        assert_eq!(ctx.output(), "first\n    second");
    }
}
