//! Formatting Context (Layer 1 & 3 Integration)
//!
//! Tracks state during formatting: column position, indentation level, and output.
//! Provides methods for emitting text while maintaining state.
//!
//! # Layer Integration
//!
//! This module integrates with:
//! - **Layer 1 (Spacing)**: Token-aware emission via `emit_token()` and `spacing_for()`
//! - **Layer 3 (Shape)**: Width tracking via internal `Shape` struct
//!
//! The `FormatContext` uses `Shape` internally to track available width and
//! make breaking decisions, and can use `spacing::lookup_spacing()` for
//! determining inter-token spacing.

use crate::emitter::{Emitter, StringEmitter};
use crate::shape::Shape;
use crate::spacing::{lookup_spacing, SpaceAction, TokenCategory};

/// Default maximum line width before breaking.
pub const MAX_LINE_WIDTH: usize = 100;

/// Spaces per indentation level.
pub const INDENT_WIDTH: usize = 4;

/// Configuration for the formatter.
///
/// Controls formatting behavior like line width, indentation, and trailing commas.
/// This is the unified configuration type used throughout the formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatConfig {
    /// Maximum line width before breaking to multiple lines.
    /// Defaults to 100 characters (Spec line 19).
    pub max_width: usize,

    /// Indentation size in spaces.
    /// Defaults to 4 spaces (Spec line 18).
    pub indent_size: usize,

    /// Whether to add trailing commas in multi-line lists.
    /// Defaults to `Always` (Spec line 20).
    pub trailing_commas: TrailingCommas,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            max_width: MAX_LINE_WIDTH,
            indent_size: INDENT_WIDTH,
            trailing_commas: TrailingCommas::Always,
        }
    }
}

impl FormatConfig {
    /// Create a new config with the specified max width.
    pub fn with_max_width(max_width: usize) -> Self {
        Self {
            max_width,
            ..Default::default()
        }
    }

    /// Create a new config with the specified indent size.
    pub fn with_indent_size(indent_size: usize) -> Self {
        Self {
            indent_size,
            ..Default::default()
        }
    }

    /// Check if trailing commas should be added in multi-line context.
    #[inline]
    pub fn add_trailing_comma(&self, is_multiline: bool, had_trailing: bool) -> bool {
        match self.trailing_commas {
            TrailingCommas::Always => is_multiline,
            TrailingCommas::Never => false,
            TrailingCommas::Preserve => had_trailing && is_multiline,
        }
    }
}

/// Trailing comma behavior for multi-line lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TrailingCommas {
    /// Always add trailing commas in multi-line (default).
    ///
    /// Spec line 20: "Trailing commas required in multi-line"
    #[default]
    Always,

    /// Never add trailing commas.
    Never,

    /// Preserve user's choice (keep if present, don't add if absent).
    Preserve,
}

impl TrailingCommas {
    /// Check if this setting always adds trailing commas.
    #[inline]
    pub fn is_always(self) -> bool {
        matches!(self, TrailingCommas::Always)
    }

    /// Check if this setting never adds trailing commas.
    #[inline]
    pub fn is_never(self) -> bool {
        matches!(self, TrailingCommas::Never)
    }

    /// Check if this setting preserves user choice.
    #[inline]
    pub fn is_preserve(self) -> bool {
        matches!(self, TrailingCommas::Preserve)
    }
}

/// Formatting context that tracks state during output.
///
/// This struct wraps an emitter and maintains:
/// - Current column position (0-indexed)
/// - Current indentation level
/// - Configuration (max width, etc.)
/// - Shape for width tracking (Layer 3)
/// - Last token category for spacing (Layer 1)
///
/// All emit operations update the column position and shape automatically.
///
/// # Layer Integration
///
/// - **Layer 1 (Spacing)**: Tracks last token category for `spacing_for()` lookups
/// - **Layer 3 (Shape)**: Uses `Shape` internally for `fits()` width decisions
pub struct FormatContext<E: Emitter = StringEmitter> {
    emitter: E,
    column: usize,
    indent_level: usize,
    config: FormatConfig,
    /// Shape for Layer 3 width tracking
    shape: Shape,
    /// Last token category for Layer 1 spacing decisions
    last_token: Option<TokenCategory>,
}

impl FormatContext<StringEmitter> {
    /// Create a new format context with a string emitter and default config.
    pub fn new() -> Self {
        Self::with_emitter(StringEmitter::new())
    }

    /// Create a new format context with a string emitter and custom config.
    pub fn with_config(config: FormatConfig) -> Self {
        Self::with_emitter_and_config(StringEmitter::new(), config)
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
    /// Create a format context with a specific emitter and default config.
    pub fn with_emitter(emitter: E) -> Self {
        Self::with_emitter_and_config(emitter, FormatConfig::default())
    }

    /// Create a format context with a specific emitter and config.
    pub fn with_emitter_and_config(emitter: E, config: FormatConfig) -> Self {
        Self {
            emitter,
            column: 0,
            indent_level: 0,
            shape: Shape::new(config.max_width),
            config,
            last_token: None,
        }
    }

    /// Get the current configuration.
    pub fn config(&self) -> &FormatConfig {
        &self.config
    }

    /// Get the maximum line width.
    pub fn max_width(&self) -> usize {
        self.config.max_width
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
    ///
    /// # Layer 3 Integration
    ///
    /// Updates both the column AND the shape's available width to ensure
    /// `fits()` returns correct results for the current position.
    pub fn set_column(&mut self, column: usize) {
        self.column = column;
        // Sync shape: reduce available width by the column offset
        self.shape = Shape {
            width: self.config.max_width.saturating_sub(column),
            offset: column,
            indent: self.shape.indent,
        };
    }

    /// Check if adding `width` characters would exceed the line limit.
    pub fn would_exceed_limit(&self, width: usize) -> bool {
        self.column + width > self.config.max_width
    }

    /// Check if content of `width` would fit on the current line.
    ///
    /// # Layer 3 Integration
    ///
    /// Delegates to `Shape::fits()` for consistent width-based decisions.
    pub fn fits(&self, width: usize) -> bool {
        self.shape.fits(width)
    }

    /// Get the current shape for width tracking.
    ///
    /// # Layer 3 Integration
    ///
    /// Returns the internal `Shape` used for width-based breaking decisions.
    pub fn shape(&self) -> &Shape {
        &self.shape
    }

    /// Emit a text fragment.
    pub fn emit(&mut self, text: &str) {
        self.emitter.emit(text);
        self.column += text.len();
        self.shape = self.shape.consume(text.len());
    }

    /// Emit a single space.
    pub fn emit_space(&mut self) {
        self.emitter.emit_space();
        self.column += 1;
        self.shape = self.shape.consume(1);
    }

    // ========================================================================
    // Layer 1 (Spacing) Integration
    // ========================================================================

    /// Get the spacing action required between the last emitted token and a new token.
    ///
    /// # Layer 1 Integration
    ///
    /// Uses `spacing::lookup_spacing()` to determine the appropriate spacing
    /// action based on the declarative rules in `spacing/rules.rs`.
    ///
    /// Returns `None` if no previous token was recorded.
    pub fn spacing_for(&self, next_token: TokenCategory) -> Option<SpaceAction> {
        self.last_token.map(|last| lookup_spacing(last, next_token))
    }

    /// Emit a token with automatic spacing based on Layer 1 rules.
    ///
    /// # Layer 1 Integration
    ///
    /// This method:
    /// 1. Looks up spacing between last token and this token
    /// 2. Emits appropriate spacing (space, newline, or nothing)
    /// 3. Emits the token text
    /// 4. Updates the last token for future spacing decisions
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.emit_token(TokenCategory::Ident, "foo");
    /// ctx.emit_token(TokenCategory::Plus, "+");  // Auto-adds space before
    /// ctx.emit_token(TokenCategory::Ident, "bar"); // Auto-adds space before
    /// // Result: "foo + bar"
    /// ```
    pub fn emit_token(&mut self, category: TokenCategory, text: &str) {
        // Check if we need spacing before this token
        if let Some(action) = self.spacing_for(category) {
            match action {
                SpaceAction::Space => self.emit_space(),
                SpaceAction::Newline => self.emit_newline_indent(),
                SpaceAction::None | SpaceAction::Preserve => {}
            }
        }

        // Emit the token
        self.emit(text);

        // Update last token for next spacing decision
        self.last_token = Some(category);
    }

    /// Set the last token category without emitting.
    ///
    /// # Layer 1 Integration
    ///
    /// Use this when you've emitted a token through other means (e.g., `emit()`)
    /// and want to set up correct spacing for subsequent tokens.
    pub fn set_last_token(&mut self, category: TokenCategory) {
        self.last_token = Some(category);
    }

    /// Clear the last token (e.g., after a newline or at start of context).
    ///
    /// # Layer 1 Integration
    ///
    /// This prevents spacing rules from applying at line starts.
    pub fn clear_last_token(&mut self) {
        self.last_token = None;
    }

    /// Emit a newline and reset column to 0.
    ///
    /// # Layer 1 Integration
    ///
    /// Clears the last token since we're at the start of a new line.
    pub fn emit_newline(&mut self) {
        self.emitter.emit_newline();
        self.column = 0;
        self.shape = self.shape.next_line(self.config.max_width);
        self.last_token = None; // Clear token state at line start
    }

    /// Emit indentation at the current level and update column.
    pub fn emit_indent(&mut self) {
        self.emitter.emit_indent(self.indent_level);
        let indent_width = self.indent_level * INDENT_WIDTH;
        self.column = indent_width;
        // After newline, shape has full width. Consume the indent width.
        // Note: next_line() already accounts for shape.indent, but emit_indent
        // may be called with different indent levels, so we sync by consuming.
        self.shape = Shape {
            width: self.config.max_width.saturating_sub(indent_width),
            offset: indent_width,
            indent: self.shape.indent,
        };
    }

    /// Emit a newline followed by indentation.
    pub fn emit_newline_indent(&mut self) {
        self.emit_newline();
        self.emit_indent();
    }

    /// Increment indentation level.
    pub fn indent(&mut self) {
        self.indent_level += 1;
        self.shape = self.shape.indent(INDENT_WIDTH);
    }

    /// Decrement indentation level.
    pub fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
        self.shape = self.shape.dedent(INDENT_WIDTH);
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
