//! Shape struct for tracking available formatting space.

use crate::context::FormatConfig;

/// Available formatting space.
///
/// Shape tracks three key values as we descend into nested structures:
/// - `width`: Characters remaining on current line
/// - `indent`: Current indentation level (in spaces)
/// - `offset`: Position from start of line (for alignment)
///
/// # Usage Pattern
///
/// ```ignore
/// let shape = Shape::from_config(&config);
///
/// // Consume characters as we emit them
/// let shape = shape.consume("let x = ".len());
///
/// // Indent for nested block
/// let body_shape = shape.indent(config.indent_size);
///
/// // Check if content fits
/// if shape.fits(expr_width) {
///     // Render inline
/// } else {
///     // Render broken
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// Characters remaining on current line.
    pub width: usize,

    /// Current indentation level (in spaces).
    pub indent: usize,

    /// Position on current line (from start of line).
    pub offset: usize,
}

impl Shape {
    /// Create a new shape with given max width.
    #[inline]
    pub fn new(max_width: usize) -> Self {
        Shape {
            width: max_width,
            indent: 0,
            offset: 0,
        }
    }

    /// Create shape from formatter config.
    #[inline]
    pub fn from_config(config: &FormatConfig) -> Self {
        Shape::new(config.max_width)
    }

    /// Reduce width by n characters (for content already emitted on this line).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let shape = Shape::new(100);
    /// // After emitting "let x = " (8 chars):
    /// let shape = shape.consume(8);
    /// assert_eq!(shape.width, 92);
    /// assert_eq!(shape.offset, 8);
    /// ```
    #[inline]
    #[must_use = "consume returns a new Shape with reduced width"]
    pub fn consume(self, n: usize) -> Self {
        Shape {
            width: self.width.saturating_sub(n),
            offset: self.offset + n,
            ..self
        }
    }

    /// Add indentation for nested block.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let shape = Shape::new(100);
    /// let indented = shape.indent(4);
    /// assert_eq!(indented.indent, 4);
    /// assert_eq!(indented.width, 96);
    /// ```
    #[inline]
    #[must_use = "indent returns a new Shape with increased indentation"]
    pub fn indent(self, spaces: usize) -> Self {
        Shape {
            indent: self.indent + spaces,
            width: self.width.saturating_sub(spaces),
            ..self
        }
    }

    /// Remove indentation (dedent).
    #[inline]
    #[must_use = "dedent returns a new Shape with decreased indentation"]
    pub fn dedent(self, spaces: usize) -> Self {
        Shape {
            indent: self.indent.saturating_sub(spaces),
            width: self.width + spaces,
            ..self
        }
    }

    /// Check if content fits in remaining width.
    #[inline]
    pub fn fits(&self, content_width: usize) -> bool {
        content_width <= self.width
    }

    /// Check if string fits in remaining width.
    #[inline]
    pub fn fits_str(&self, s: &str) -> bool {
        self.fits(s.len())
    }

    /// Get shape for next line (reset position to indent).
    ///
    /// This is used when we break to a new line - the offset resets
    /// to the indentation level, and width is recalculated.
    #[inline]
    #[must_use = "next_line returns a new Shape for the next line"]
    pub fn next_line(self, max_width: usize) -> Self {
        Shape {
            width: max_width.saturating_sub(self.indent),
            offset: self.indent,
            indent: self.indent,
        }
    }

    /// Get remaining width.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.width
    }

    /// Check if we should break (content doesn't fit).
    #[inline]
    pub fn should_break(&self, content_width: usize) -> bool {
        !self.fits(content_width)
    }

    // ========================================================================
    // Nested construct handling
    // ========================================================================

    /// Create shape for nested construct (Spec lines 93-95).
    ///
    /// "Nested constructs break independently based on their own width"
    ///
    /// The nested construct gets a fresh width calculation from current indent,
    /// not from the current consumed position. This allows nested expressions
    /// to fit inline even when the parent needs to break.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Even though outer is broken, inner call fits:
    /// let result = run(
    ///     process(items.map(x -> x * 2)),  // This fits, stays inline
    ///     validate(result),
    /// )
    /// ```
    #[inline]
    #[must_use = "for_nested returns a new Shape for nested constructs"]
    pub fn for_nested(&self, config: &FormatConfig) -> Shape {
        Shape {
            width: config.max_width.saturating_sub(self.indent),
            indent: self.indent,
            offset: self.indent,
        }
    }

    // ========================================================================
    // Integration helpers
    // ========================================================================

    /// Get shape for function body (indented block).
    #[inline]
    #[must_use = "for_block returns a new Shape for the block body"]
    pub fn for_block(&self, config: &FormatConfig) -> Self {
        self.indent(config.indent_size).next_line(config.max_width)
    }

    /// Get shape for continuation (same indent, fresh line).
    #[inline]
    #[must_use = "for_continuation returns a new Shape for continuation lines"]
    pub fn for_continuation(&self, config: &FormatConfig) -> Self {
        self.next_line(config.max_width)
    }

    /// Get shape after emitting a prefix string.
    #[inline]
    #[must_use = "after returns a new Shape with consumed prefix width"]
    pub fn after(&self, prefix: &str) -> Self {
        self.consume(prefix.len())
    }

    /// Add visual offset without consuming width (for alignment).
    ///
    /// Used when content is placed at a specific column for alignment
    /// but we haven't emitted characters to get there.
    #[inline]
    #[must_use = "with_offset returns a new Shape with the given offset"]
    pub fn with_offset(self, offset: usize) -> Self {
        Shape { offset, ..self }
    }

    /// Check if at line start (offset equals indent).
    #[inline]
    pub fn at_line_start(&self) -> bool {
        self.offset == self.indent
    }
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            width: 100, // Default max width from spec
            indent: 0,
            offset: 0,
        }
    }
}
