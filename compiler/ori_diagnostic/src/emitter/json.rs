//! JSON Emitter
//!
//! Machine-readable diagnostic output in JSON format.

use std::io::Write;

use crate::Diagnostic;

use super::{escape_json, trailing_comma, DiagnosticEmitter};

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
            let comma = trailing_comma(i, diagnostic.labels.len());
            let _ = writeln!(self.writer, "      {{");
            let _ = writeln!(self.writer, "        \"start\": {},", label.span.start);
            let _ = writeln!(self.writer, "        \"end\": {},", label.span.end);
            let _ = writeln!(
                self.writer,
                "        \"message\": \"{}\",",
                escape_json(&label.message)
            );
            let _ = writeln!(self.writer, "        \"primary\": {},", label.is_primary);
            // Include source info for cross-file labels
            if let Some(ref src) = label.source_info {
                let _ = writeln!(
                    self.writer,
                    "        \"file\": \"{}\",",
                    escape_json(&src.path)
                );
                let _ = writeln!(self.writer, "        \"cross_file\": true");
            } else {
                let _ = writeln!(self.writer, "        \"cross_file\": false");
            }
            let _ = writeln!(self.writer, "      }}{comma}");
        }
        let _ = writeln!(self.writer, "    ],");

        // Notes
        let _ = writeln!(self.writer, "    \"notes\": [");
        for (i, note) in diagnostic.notes.iter().enumerate() {
            let comma = trailing_comma(i, diagnostic.notes.len());
            let _ = writeln!(self.writer, "      \"{}\"{}", escape_json(note), comma);
        }
        let _ = writeln!(self.writer, "    ],");

        // Suggestions (text-only)
        let _ = writeln!(self.writer, "    \"suggestions\": [");
        for (i, suggestion) in diagnostic.suggestions.iter().enumerate() {
            let comma = trailing_comma(i, diagnostic.suggestions.len());
            let _ = writeln!(
                self.writer,
                "      \"{}\"{}",
                escape_json(suggestion),
                comma
            );
        }
        let _ = writeln!(self.writer, "    ],");

        // Structured suggestions (with spans and applicability)
        let _ = writeln!(self.writer, "    \"structured_suggestions\": [");
        for (i, suggestion) in diagnostic.structured_suggestions.iter().enumerate() {
            let comma = trailing_comma(i, diagnostic.structured_suggestions.len());
            let _ = writeln!(self.writer, "      {{");
            let _ = writeln!(
                self.writer,
                "        \"message\": \"{}\"",
                escape_json(&suggestion.message)
            );
            let _ = writeln!(self.writer, "      }}{comma}");
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
mod tests;
