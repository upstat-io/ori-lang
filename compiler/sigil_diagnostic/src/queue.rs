//! Diagnostic queue for collecting, deduplicating, and sorting diagnostics.
//!
//! Based on Go compiler's error handling patterns:
//! - Error limits to prevent overwhelming output
//! - Deduplication of same-line errors
//! - Soft error suppression after hard errors
//! - Follow-on error filtering

use crate::{Diagnostic, ErrorCode};
use sigil_ir::Span;

/// Configuration for diagnostic processing.
#[derive(Clone, Debug)]
pub struct DiagnosticConfig {
    /// Maximum number of errors before stopping (0 = unlimited).
    pub error_limit: usize,
    /// Filter out follow-on errors that result from previous errors.
    pub filter_follow_on: bool,
    /// Deduplicate diagnostics with same line and similar content.
    pub deduplicate: bool,
}

impl Default for DiagnosticConfig {
    fn default() -> Self {
        DiagnosticConfig {
            error_limit: 10,
            filter_follow_on: true,
            deduplicate: true,
        }
    }
}

impl DiagnosticConfig {
    /// Create a config with no limits (for testing).
    pub fn unlimited() -> Self {
        DiagnosticConfig {
            error_limit: 0,
            filter_follow_on: false,
            deduplicate: false,
        }
    }
}

/// Queued diagnostic with metadata for sorting and deduplication.
#[derive(Clone, Debug)]
pub struct QueuedDiagnostic {
    /// The diagnostic itself.
    pub diagnostic: Diagnostic,
    /// Line number (1-based) for sorting.
    pub line: u32,
    /// Column number (1-based) for sorting within a line.
    pub column: u32,
    /// Whether this is a soft error (can be suppressed after hard errors).
    pub soft: bool,
}

impl QueuedDiagnostic {
    /// Create a new queued diagnostic.
    pub fn new(diagnostic: Diagnostic, line: u32, column: u32, soft: bool) -> Self {
        QueuedDiagnostic {
            diagnostic,
            line,
            column,
            soft,
        }
    }
}

/// Queue for collecting, deduplicating, and sorting diagnostics.
///
/// # Example
///
/// ```ignore
/// let mut queue = DiagnosticQueue::new();
/// queue.add(diagnostic, line, column, is_soft);
/// // ... add more diagnostics
/// let sorted = queue.flush();
/// ```
#[derive(Clone, Debug)]
pub struct DiagnosticQueue {
    /// Collected diagnostics.
    diagnostics: Vec<QueuedDiagnostic>,
    /// Count of errors (not warnings/notes).
    error_count: usize,
    /// Last line with a syntax error (for dedup).
    last_syntax_line: Option<u32>,
    /// Last (line, `message_prefix`) for non-syntax error dedup.
    last_error: Option<(u32, String)>,
    /// Whether we've seen a hard error.
    has_hard_error: bool,
    /// Configuration.
    config: DiagnosticConfig,
}

impl Default for DiagnosticQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticQueue {
    /// Create a new diagnostic queue with default configuration.
    pub fn new() -> Self {
        DiagnosticQueue {
            diagnostics: Vec::new(),
            error_count: 0,
            last_syntax_line: None,
            last_error: None,
            has_hard_error: false,
            config: DiagnosticConfig::default(),
        }
    }

    /// Create a diagnostic queue with custom configuration.
    pub fn with_config(config: DiagnosticConfig) -> Self {
        DiagnosticQueue {
            diagnostics: Vec::new(),
            error_count: 0,
            last_syntax_line: None,
            last_error: None,
            has_hard_error: false,
            config,
        }
    }

    /// Add a diagnostic to the queue.
    ///
    /// Returns `true` if the diagnostic was added, `false` if it was filtered.
    pub fn add(&mut self, diag: Diagnostic, line: u32, column: u32, soft: bool) -> bool {
        // Check error limit
        if self.config.error_limit > 0 && self.error_count >= self.config.error_limit {
            return false;
        }

        let is_error = diag.is_error();

        // Suppress soft errors after hard errors
        if soft && self.has_hard_error {
            return false;
        }

        // Filter follow-on errors
        if self.config.filter_follow_on && Self::is_follow_on(&diag) {
            return false;
        }

        // Deduplicate
        if self.config.deduplicate && self.is_duplicate(&diag, line) {
            return false;
        }

        // Track hard errors
        if is_error && !soft {
            self.has_hard_error = true;
        }

        // Update dedup tracking
        if is_error {
            if Self::is_syntax_error(&diag) {
                self.last_syntax_line = Some(line);
            } else {
                // Take first ~30 chars of message for dedup
                let prefix = diag.message.chars().take(30).collect();
                self.last_error = Some((line, prefix));
            }
        }

        // Add to queue
        self.diagnostics.push(QueuedDiagnostic::new(diag, line, column, soft));

        if is_error {
            self.error_count += 1;
        }

        true
    }

    /// Add a diagnostic with position computed from source.
    pub fn add_with_source(&mut self, diag: Diagnostic, source: &str, soft: bool) -> bool {
        let (line, column) = if let Some(span) = diag.primary_span() {
            crate::span_utils::offset_to_line_col(source, span.start)
        } else {
            (1, 1)
        };
        self.add(diag, line, column, soft)
    }

    /// Check if the error limit has been reached.
    pub fn limit_reached(&self) -> bool {
        self.config.error_limit > 0 && self.error_count >= self.config.error_limit
    }

    /// Get the number of errors collected.
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Check if any hard errors have been recorded.
    pub fn has_hard_error(&self) -> bool {
        self.has_hard_error
    }

    /// Sort diagnostics by position and return them.
    ///
    /// Clears the queue after flushing.
    pub fn flush(&mut self) -> Vec<Diagnostic> {
        // Sort by line, then column
        self.diagnostics.sort_by_key(|d| (d.line, d.column));

        // Extract diagnostics
        let result: Vec<Diagnostic> = self.diagnostics.drain(..).map(|d| d.diagnostic).collect();

        // Reset state
        self.error_count = 0;
        self.last_syntax_line = None;
        self.last_error = None;
        self.has_hard_error = false;

        result
    }

    /// Get diagnostics without clearing the queue.
    pub fn peek(&self) -> Vec<&Diagnostic> {
        self.diagnostics.iter().map(|d| &d.diagnostic).collect()
    }

    /// Check if a diagnostic is a follow-on error.
    ///
    /// Follow-on errors contain phrases like "invalid operand" or "invalid type"
    /// which typically result from a previous error.
    fn is_follow_on(diag: &Diagnostic) -> bool {
        if !diag.is_error() {
            return false;
        }

        let msg = diag.message.to_lowercase();
        msg.contains("invalid operand")
            || msg.contains("invalid type")
            || msg.contains("<error>")
    }

    /// Check if a diagnostic is a duplicate of a recent one.
    fn is_duplicate(&self, diag: &Diagnostic, line: u32) -> bool {
        if !diag.is_error() {
            return false;
        }

        // Syntax errors: dedupe same line
        if Self::is_syntax_error(diag) {
            if let Some(last_line) = self.last_syntax_line {
                if last_line == line {
                    return true;
                }
            }
        } else {
            // Non-syntax errors: dedupe same line + similar message
            if let Some((last_line, ref last_prefix)) = self.last_error {
                if last_line == line {
                    let prefix: String = diag.message.chars().take(30).collect();
                    if prefix == *last_prefix {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if a diagnostic is a syntax (parser) error.
    fn is_syntax_error(diag: &Diagnostic) -> bool {
        matches!(
            diag.code,
            ErrorCode::E1001
                | ErrorCode::E1002
                | ErrorCode::E1003
                | ErrorCode::E1004
                | ErrorCode::E1005
                | ErrorCode::E1006
                | ErrorCode::E1007
                | ErrorCode::E1008
                | ErrorCode::E1009
                | ErrorCode::E1010
                | ErrorCode::E1011
                | ErrorCode::E1012
                | ErrorCode::E1013
                | ErrorCode::E1014
        )
    }
}

/// Create a "too many errors" diagnostic.
pub fn too_many_errors(limit: usize, span: Span) -> Diagnostic {
    Diagnostic::error(ErrorCode::E9002)
        .with_message(format!("aborting due to {limit} previous errors"))
        .with_label(span, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_error(code: ErrorCode, message: &str, start: u32) -> Diagnostic {
        Diagnostic::error(code)
            .with_message(message)
            .with_label(Span::new(start, start + 5), "error here")
    }

    #[test]
    fn test_queue_basic() {
        let mut queue = DiagnosticQueue::new();

        let diag = make_error(ErrorCode::E2001, "type mismatch", 0);
        assert!(queue.add(diag, 1, 1, false));
        assert_eq!(queue.error_count(), 1);

        let errors = queue.flush();
        assert_eq!(errors.len(), 1);
        assert_eq!(queue.error_count(), 0);
    }

    #[test]
    fn test_queue_error_limit() {
        let config = DiagnosticConfig {
            error_limit: 2,
            ..Default::default()
        };
        let mut queue = DiagnosticQueue::with_config(config);

        // Add up to limit
        assert!(queue.add(make_error(ErrorCode::E2001, "error 1", 0), 1, 1, false));
        assert!(queue.add(make_error(ErrorCode::E2001, "error 2", 10), 2, 1, false));

        // At limit
        assert!(queue.limit_reached());

        // Rejected
        assert!(!queue.add(make_error(ErrorCode::E2001, "error 3", 20), 3, 1, false));

        let errors = queue.flush();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_queue_soft_error_suppression() {
        let mut queue = DiagnosticQueue::new();

        // Hard error
        queue.add(make_error(ErrorCode::E2001, "hard error", 0), 1, 1, false);
        assert!(queue.has_hard_error());

        // Soft error should be suppressed
        assert!(!queue.add(make_error(ErrorCode::E2005, "soft error", 10), 2, 1, true));

        let errors = queue.flush();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_queue_follow_on_filtering() {
        let mut queue = DiagnosticQueue::new();

        // First error
        queue.add(make_error(ErrorCode::E2001, "type mismatch", 0), 1, 1, false);

        // Follow-on error should be filtered
        assert!(!queue.add(
            make_error(ErrorCode::E2001, "invalid operand due to previous error", 10),
            2,
            1,
            false
        ));

        let errors = queue.flush();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn test_queue_deduplication_syntax() {
        let mut queue = DiagnosticQueue::new();

        // First syntax error on line 1
        queue.add(make_error(ErrorCode::E1001, "unexpected token", 0), 1, 1, false);

        // Second syntax error on same line should be deduped
        assert!(!queue.add(
            make_error(ErrorCode::E1002, "expected expression", 5),
            1,
            5,
            false
        ));

        // Error on different line should be added
        assert!(queue.add(
            make_error(ErrorCode::E1001, "another error", 20),
            2,
            1,
            false
        ));

        let errors = queue.flush();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_queue_deduplication_same_message() {
        let mut queue = DiagnosticQueue::new();

        // Same message on same line
        queue.add(
            make_error(ErrorCode::E2001, "type mismatch: expected int, found str", 0),
            1,
            1,
            false,
        );
        assert!(!queue.add(
            make_error(ErrorCode::E2001, "type mismatch: expected int, found str", 5),
            1,
            5,
            false
        ));

        // Different message on same line should be added
        assert!(queue.add(
            make_error(ErrorCode::E2001, "different error message here", 10),
            1,
            10,
            false
        ));

        let errors = queue.flush();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_queue_sorting() {
        let mut queue = DiagnosticQueue::with_config(DiagnosticConfig::unlimited());

        // Add out of order
        queue.add(make_error(ErrorCode::E2001, "error on line 3", 40), 3, 1, false);
        queue.add(make_error(ErrorCode::E2001, "error on line 1", 0), 1, 1, false);
        queue.add(make_error(ErrorCode::E2001, "error on line 2", 20), 2, 1, false);

        let errors = queue.flush();
        assert_eq!(errors.len(), 3);
        assert!(errors[0].message.contains("line 1"));
        assert!(errors[1].message.contains("line 2"));
        assert!(errors[2].message.contains("line 3"));
    }

    #[test]
    fn test_queue_sorting_within_line() {
        let mut queue = DiagnosticQueue::with_config(DiagnosticConfig::unlimited());

        // Add errors on same line, out of order by column
        queue.add(make_error(ErrorCode::E2001, "error at col 10", 10), 1, 10, false);
        queue.add(make_error(ErrorCode::E2001, "error at col 1", 0), 1, 1, false);
        queue.add(make_error(ErrorCode::E2001, "error at col 5", 5), 1, 5, false);

        let errors = queue.flush();
        assert_eq!(errors.len(), 3);
        assert!(errors[0].message.contains("col 1"));
        assert!(errors[1].message.contains("col 5"));
        assert!(errors[2].message.contains("col 10"));
    }

    #[test]
    fn test_too_many_errors_diagnostic() {
        let diag = too_many_errors(10, Span::new(0, 5));
        assert_eq!(diag.code, ErrorCode::E9002);
        assert!(diag.message.contains("10"));
    }

    #[test]
    fn test_warnings_not_counted() {
        let config = DiagnosticConfig {
            error_limit: 1,
            ..Default::default()
        };
        let mut queue = DiagnosticQueue::with_config(config);

        // Warning doesn't count toward limit
        let warning = Diagnostic::warning(ErrorCode::E2001)
            .with_message("warning")
            .with_label(Span::new(0, 5), "here");
        assert!(queue.add(warning, 1, 1, false));
        assert!(!queue.limit_reached());
        assert_eq!(queue.error_count(), 0);

        // Error counts
        assert!(queue.add(make_error(ErrorCode::E2001, "error", 10), 2, 1, false));
        assert!(queue.limit_reached());
    }
}
