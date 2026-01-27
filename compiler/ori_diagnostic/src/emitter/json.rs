//! JSON Emitter
//!
//! Machine-readable diagnostic output in JSON format.

use super::{escape_json, DiagnosticEmitter};
use crate::Diagnostic;
use std::io::Write;

/// JSON emitter for machine-readable output.
pub struct JsonEmitter<W: Write> {
    writer: W,
    first: bool,
}

impl<W: Write> JsonEmitter<W> {
    /// Create a new JSON emitter.
    pub fn new(writer: W) -> Self {
        JsonEmitter {
            writer,
            first: true,
        }
    }

    /// Begin the JSON array output.
    pub fn begin(&mut self) {
        let _ = writeln!(self.writer, "[");
    }

    /// End the JSON array output.
    pub fn end(&mut self) {
        let _ = writeln!(self.writer, "\n]");
    }
}

impl<W: Write> DiagnosticEmitter for JsonEmitter<W> {
    fn emit(&mut self, diagnostic: &Diagnostic) {
        if !self.first {
            let _ = writeln!(self.writer, ",");
        }
        self.first = false;

        // Build JSON manually (to avoid serde dependency)
        let _ = writeln!(self.writer, "  {{");
        let _ = writeln!(
            self.writer,
            "    \"code\": \"{}\",",
            diagnostic.code.as_str()
        );
        let _ = writeln!(
            self.writer,
            "    \"severity\": \"{:?}\",",
            diagnostic.severity
        );
        let _ = writeln!(
            self.writer,
            "    \"message\": \"{}\",",
            escape_json(&diagnostic.message)
        );

        // Labels
        let _ = writeln!(self.writer, "    \"labels\": [");
        for (i, label) in diagnostic.labels.iter().enumerate() {
            let comma = if i + 1 < diagnostic.labels.len() {
                ","
            } else {
                ""
            };
            let _ = writeln!(self.writer, "      {{");
            let _ = writeln!(self.writer, "        \"start\": {},", label.span.start);
            let _ = writeln!(self.writer, "        \"end\": {},", label.span.end);
            let _ = writeln!(
                self.writer,
                "        \"message\": \"{}\",",
                escape_json(&label.message)
            );
            let _ = writeln!(self.writer, "        \"primary\": {}", label.is_primary);
            let _ = writeln!(self.writer, "      }}{comma}");
        }
        let _ = writeln!(self.writer, "    ],");

        // Notes
        let _ = writeln!(self.writer, "    \"notes\": [");
        for (i, note) in diagnostic.notes.iter().enumerate() {
            let comma = if i + 1 < diagnostic.notes.len() {
                ","
            } else {
                ""
            };
            let _ = writeln!(self.writer, "      \"{}\"{}", escape_json(note), comma);
        }
        let _ = writeln!(self.writer, "    ],");

        // Suggestions
        let _ = writeln!(self.writer, "    \"suggestions\": [");
        for (i, suggestion) in diagnostic.suggestions.iter().enumerate() {
            let comma = if i + 1 < diagnostic.suggestions.len() {
                ","
            } else {
                ""
            };
            let _ = writeln!(
                self.writer,
                "      \"{}\"{}",
                escape_json(suggestion),
                comma
            );
        }
        let _ = writeln!(self.writer, "    ]");

        let _ = write!(self.writer, "  }}");
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }

    fn emit_summary(&mut self, _error_count: usize, _warning_count: usize) {
        // JSON output doesn't need a summary - the data speaks for itself
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::ErrorCode;
    use ori_ir::Span;

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch: expected `int`, found `str`")
            .with_label(Span::new(10, 15), "expected `int`")
            .with_secondary_label(Span::new(0, 5), "defined here")
            .with_note("int and str are incompatible")
            .with_suggestion("use `int(x)` to convert")
    }

    #[test]
    fn test_json_emitter() {
        let mut output = Vec::new();
        let mut emitter = JsonEmitter::new(&mut output);

        emitter.begin();
        emitter.emit(&sample_diagnostic());
        emitter.end();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("\"code\": \"E2001\""));
        assert!(text.contains("\"severity\": \"Error\""));
        assert!(text.contains("\"message\":"));
        assert!(text.contains("\"labels\":"));
        assert!(text.contains("\"start\":"));
        assert!(text.contains("\"end\":"));
    }

    #[test]
    fn test_json_emitter_multiple() {
        let mut output = Vec::new();
        let mut emitter = JsonEmitter::new(&mut output);

        let diag1 = Diagnostic::error(ErrorCode::E1001).with_message("error 1");
        let diag2 = Diagnostic::warning(ErrorCode::E3001).with_message("warning 1");

        emitter.begin();
        emitter.emit(&diag1);
        emitter.emit(&diag2);
        emitter.end();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("E1001"));
        assert!(text.contains("E3001"));
        assert!(text.contains("Error"));
        assert!(text.contains("Warning"));
    }
}
