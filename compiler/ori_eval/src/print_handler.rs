//! Print handler for configurable output.
//!
//! The Print capability allows output to be directed to different destinations:
//! - Native: stdout (default)
//! - WASM: buffer for capture and display
//! - Tests: buffer for assertions
//!
//! # Performance
//! Uses enum dispatch instead of trait objects for O(1) static dispatch
//! on this frequently-used path.

use std::sync::Mutex;

/// Default print handler that writes to stdout.
#[derive(Default)]
pub struct StdoutPrintHandler;

impl StdoutPrintHandler {
    /// Print a line (with newline).
    pub fn println(&self, msg: &str) {
        println!("{msg}");
    }

    /// Print without newline.
    pub fn print(&self, msg: &str) {
        print!("{msg}");
    }

    /// Get all captured output (for testing/WASM).
    ///
    /// Returns empty string since stdout doesn't capture.
    pub fn get_output(&self) -> String {
        String::new()
    }

    /// Clear captured output.
    ///
    /// No-op for stdout.
    pub fn clear(&self) {
        // Nothing to clear
    }
}

/// Print handler that captures output to a buffer.
///
/// Used for WASM and testing where output needs to be captured.
pub struct BufferPrintHandler {
    buffer: Mutex<String>,
}

impl BufferPrintHandler {
    /// Create a new buffer print handler.
    pub fn new() -> Self {
        BufferPrintHandler {
            buffer: Mutex::new(String::new()),
        }
    }

    /// Print a line (with newline).
    pub fn println(&self, msg: &str) {
        let mut buf = self
            .buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        buf.push_str(msg);
        buf.push('\n');
    }

    /// Print without newline.
    pub fn print(&self, msg: &str) {
        let mut buf = self
            .buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        buf.push_str(msg);
    }

    /// Get all captured output.
    pub fn get_output(&self) -> String {
        self.buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Clear captured output.
    pub fn clear(&self) {
        self.buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

impl Default for BufferPrintHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Print handler implementation using enum dispatch.
///
/// Only two variants exist (Stdout, Buffer), so enum dispatch
/// is more efficient than trait objects (no vtable indirection).
pub enum PrintHandlerImpl {
    /// Writes to stdout (default)
    Stdout(StdoutPrintHandler),
    /// Captures to buffer (WASM/testing)
    Buffer(BufferPrintHandler),
}

impl PrintHandlerImpl {
    /// Print a line (with newline).
    pub fn println(&self, msg: &str) {
        match self {
            Self::Stdout(h) => h.println(msg),
            Self::Buffer(h) => h.println(msg),
        }
    }

    /// Print without newline.
    pub fn print(&self, msg: &str) {
        match self {
            Self::Stdout(h) => h.print(msg),
            Self::Buffer(h) => h.print(msg),
        }
    }

    /// Get all captured output (for testing/WASM).
    ///
    /// Returns empty string for handlers that don't capture (like stdout).
    pub fn get_output(&self) -> String {
        match self {
            Self::Stdout(h) => h.get_output(),
            Self::Buffer(h) => h.get_output(),
        }
    }

    /// Clear captured output.
    pub fn clear(&self) {
        match self {
            Self::Stdout(h) => h.clear(),
            Self::Buffer(h) => h.clear(),
        }
    }
}

/// Shared print handler that can be passed around.
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for SharedPrintHandler shared across threads"
)]
pub type SharedPrintHandler = std::sync::Arc<PrintHandlerImpl>;

/// Create a default stdout print handler.
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for SharedPrintHandler"
)]
pub fn stdout_handler() -> SharedPrintHandler {
    std::sync::Arc::new(PrintHandlerImpl::Stdout(StdoutPrintHandler))
}

/// Create a buffer print handler for capturing output.
#[expect(
    clippy::disallowed_types,
    reason = "Arc required for SharedPrintHandler"
)]
pub fn buffer_handler() -> SharedPrintHandler {
    std::sync::Arc::new(PrintHandlerImpl::Buffer(BufferPrintHandler::new()))
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;

    #[test]
    fn buffer_handler_println_captures_with_newline() {
        let handler = BufferPrintHandler::new();
        handler.println("hello");
        assert_eq!(handler.get_output(), "hello\n");
    }

    #[test]
    fn buffer_handler_print_captures_without_newline() {
        let handler = BufferPrintHandler::new();
        handler.print("hello");
        assert_eq!(handler.get_output(), "hello");
    }

    #[test]
    fn buffer_handler_multiple_prints() {
        let handler = BufferPrintHandler::new();
        handler.print("hello");
        handler.print(" ");
        handler.println("world");
        assert_eq!(handler.get_output(), "hello world\n");
    }

    #[test]
    fn buffer_handler_clear_empties_buffer() {
        let handler = BufferPrintHandler::new();
        handler.println("hello");
        assert!(!handler.get_output().is_empty());
        handler.clear();
        assert!(handler.get_output().is_empty());
    }

    #[test]
    fn stdout_handler_get_output_returns_empty() {
        let handler = StdoutPrintHandler;
        assert_eq!(handler.get_output(), "");
    }

    #[test]
    fn stdout_handler_clear_is_noop() {
        let handler = StdoutPrintHandler;
        // Should not panic
        handler.clear();
    }

    #[test]
    fn buffer_handler_factory_creates_working_handler() {
        let handler = buffer_handler();
        handler.println("test");
        assert_eq!(handler.get_output(), "test\n");
    }

    #[test]
    fn buffer_handler_is_thread_safe() {
        use std::thread;

        let handler = buffer_handler();
        let handler2 = handler.clone();

        let t1 = thread::spawn(move || {
            for _ in 0..100 {
                handler2.println("a");
            }
        });

        for _ in 0..100 {
            handler.println("b");
        }

        t1.join().unwrap();

        let output = handler.get_output();
        let line_count = output.lines().count();
        assert_eq!(line_count, 200);
    }
}
