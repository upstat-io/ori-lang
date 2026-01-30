//! Output Emitter
//!
//! Abstraction for output production during formatting.
//! Supports string building for in-memory formatting and streaming for file output.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

/// Trait for emitting formatted output.
///
/// The formatter writes to an emitter during rendering. Different implementations
/// support in-memory strings, file output, or other destinations.
pub trait Emitter {
    /// Emit a text fragment.
    fn emit(&mut self, text: &str);

    /// Emit a newline (Unix-style `\n`).
    fn emit_newline(&mut self);

    /// Emit indentation (4 spaces per level).
    fn emit_indent(&mut self, level: usize);

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

    /// Get the formatted output.
    pub fn output(self) -> String {
        self.buffer
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
}

impl Emitter for StringEmitter {
    fn emit(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn emit_newline(&mut self) {
        self.buffer.push('\n');
    }

    fn emit_indent(&mut self, level: usize) {
        let spaces = level * 4;
        for _ in 0..spaces {
            self.buffer.push(' ');
        }
    }

    fn emit_space(&mut self) {
        self.buffer.push(' ');
    }
}

/// File-based emitter for streaming output to a file.
///
/// Uses buffered writing for efficiency with large files.
pub struct FileEmitter {
    writer: BufWriter<File>,
}

impl FileEmitter {
    /// Create a new file emitter.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    /// Flush any buffered output to the file.
    ///
    /// # Errors
    ///
    /// Returns an error if flushing fails.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Finish writing and return any error that occurred.
    ///
    /// # Errors
    ///
    /// Returns an error if final flush fails.
    pub fn finish(mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl Emitter for FileEmitter {
    fn emit(&mut self, text: &str) {
        // Ignore write errors during emit; caller should check flush/finish
        let _ = self.writer.write_all(text.as_bytes());
    }

    fn emit_newline(&mut self) {
        let _ = self.writer.write_all(b"\n");
    }

    fn emit_indent(&mut self, level: usize) {
        let spaces = level * 4;
        for _ in 0..spaces {
            let _ = self.writer.write_all(b" ");
        }
    }

    fn emit_space(&mut self) {
        let _ = self.writer.write_all(b" ");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_emitter_basic() {
        let mut emitter = StringEmitter::new();
        emitter.emit("hello");
        emitter.emit_space();
        emitter.emit("world");
        assert_eq!(emitter.output(), "hello world");
    }

    #[test]
    fn string_emitter_newline() {
        let mut emitter = StringEmitter::new();
        emitter.emit("line1");
        emitter.emit_newline();
        emitter.emit("line2");
        assert_eq!(emitter.output(), "line1\nline2");
    }

    #[test]
    fn string_emitter_indentation() {
        let mut emitter = StringEmitter::new();
        emitter.emit("fn main");
        emitter.emit_newline();
        emitter.emit_indent(1);
        emitter.emit("body");
        emitter.emit_newline();
        emitter.emit_indent(2);
        emitter.emit("nested");
        assert_eq!(emitter.output(), "fn main\n    body\n        nested");
    }

    #[test]
    fn string_emitter_trailing_newline() {
        let mut emitter = StringEmitter::new();
        emitter.emit("content");
        emitter.ensure_trailing_newline();
        assert_eq!(emitter.output(), "content\n");
    }

    #[test]
    fn string_emitter_trailing_newline_already_present() {
        let mut emitter = StringEmitter::new();
        emitter.emit("content");
        emitter.emit_newline();
        emitter.ensure_trailing_newline();
        assert_eq!(emitter.output(), "content\n");
    }

    #[test]
    fn string_emitter_trim_trailing_blank_lines() {
        let mut emitter = StringEmitter::new();
        emitter.emit("content");
        emitter.emit_newline();
        emitter.emit_newline();
        emitter.emit_newline();
        emitter.trim_trailing_blank_lines();
        emitter.ensure_trailing_newline();
        assert_eq!(emitter.output(), "content\n");
    }

    #[test]
    fn string_emitter_with_capacity() {
        let emitter = StringEmitter::with_capacity(1024);
        assert!(emitter.is_empty());
        assert_eq!(emitter.len(), 0);
    }
}
