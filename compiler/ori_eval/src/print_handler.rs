//! Print handler trait for configurable output.
//!
//! The Print capability allows output to be directed to different destinations:
//! - Native: stdout (default)
//! - WASM: buffer for capture and display
//! - Tests: buffer for assertions

use std::sync::Mutex;

/// Handler trait for print output.
///
/// Implementations determine where print output goes.
pub trait PrintHandler: Send + Sync {
    /// Print a line (with newline).
    fn println(&self, msg: &str);

    /// Print without newline.
    fn print(&self, msg: &str);

    /// Get all captured output (for testing/WASM).
    ///
    /// Returns empty string for handlers that don't capture (like stdout).
    fn get_output(&self) -> String;

    /// Clear captured output.
    fn clear(&self);
}

/// Default print handler that writes to stdout.
#[derive(Default)]
pub struct StdoutPrintHandler;

impl PrintHandler for StdoutPrintHandler {
    fn println(&self, msg: &str) {
        println!("{msg}");
    }

    fn print(&self, msg: &str) {
        print!("{msg}");
    }

    fn get_output(&self) -> String {
        // stdout doesn't capture
        String::new()
    }

    fn clear(&self) {
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
}

impl Default for BufferPrintHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl PrintHandler for BufferPrintHandler {
    fn println(&self, msg: &str) {
        let mut buf = self.buffer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        buf.push_str(msg);
        buf.push('\n');
    }

    fn print(&self, msg: &str) {
        let mut buf = self.buffer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        buf.push_str(msg);
    }

    fn get_output(&self) -> String {
        self.buffer.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
    }

    fn clear(&self) {
        self.buffer.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
    }
}

/// Shared print handler that can be passed around.
#[expect(clippy::disallowed_types, reason = "Arc required for SharedPrintHandler dyn trait object shared across threads")]
pub type SharedPrintHandler = std::sync::Arc<dyn PrintHandler>;

/// Create a default stdout print handler.
#[expect(clippy::disallowed_types, reason = "Arc required for SharedPrintHandler dyn trait object")]
pub fn stdout_handler() -> SharedPrintHandler {
    std::sync::Arc::new(StdoutPrintHandler)
}

/// Create a buffer print handler for capturing output.
#[expect(clippy::disallowed_types, reason = "Arc required for SharedPrintHandler dyn trait object")]
pub fn buffer_handler() -> SharedPrintHandler {
    std::sync::Arc::new(BufferPrintHandler::new())
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
