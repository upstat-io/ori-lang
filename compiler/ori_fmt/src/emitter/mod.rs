//! Output Emitter
//!
//! Abstraction for output production during formatting.
//! Supports string building for in-memory formatting.

/// Trait for emitting formatted output.
///
/// The formatter writes to an emitter during rendering. Different implementations
/// support in-memory strings, file output, or other destinations.
pub trait Emitter {
    /// Emit a text fragment.
    fn emit(&mut self, text: &str);

    /// Emit a newline (Unix-style `\n`).
    fn emit_newline(&mut self);

    /// Emit indentation as the given number of spaces.
    fn emit_indent(&mut self, spaces: usize);

    /// Emit a single space.
    fn emit_space(&mut self);
}

/// String-based emitter for in-memory formatting.
///
/// This is the primary emitter used for most formatting operations.
/// It builds a string incrementally and provides the result.
#[derive(Default)]
pub struct StringEmitter {
    buffer: String,
}

impl StringEmitter {
    /// Create a new string emitter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
        }
    }

    /// Get the current buffer contents without consuming.
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    /// Get the current length of the buffer.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Ensure the output ends with a single newline.
    ///
    /// This is called at the end of formatting to enforce the trailing newline rule.
    pub fn ensure_trailing_newline(&mut self) {
        if !self.buffer.ends_with('\n') {
            self.buffer.push('\n');
        }
    }

    /// Remove trailing blank lines, leaving only content followed by single newline.
    ///
    /// This is called at the end of formatting to enforce no trailing blank lines.
    pub fn trim_trailing_blank_lines(&mut self) {
        // Remove trailing whitespace and blank lines
        while self.buffer.ends_with("\n\n") || self.buffer.ends_with(" \n") {
            self.buffer.pop();
        }
    }

    /// Get the formatted output.
    pub fn output(self) -> String {
        self.buffer
    }
}

impl Emitter for StringEmitter {
    fn emit(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn emit_newline(&mut self) {
        self.buffer.push('\n');
    }

    fn emit_indent(&mut self, spaces: usize) {
        for _ in 0..spaces {
            self.buffer.push(' ');
        }
    }

    fn emit_space(&mut self) {
        self.buffer.push(' ');
    }
}

#[cfg(test)]
mod tests;
