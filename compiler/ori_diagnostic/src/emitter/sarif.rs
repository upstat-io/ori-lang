//! SARIF Emitter
//!
//! Static Analysis Results Interchange Format (SARIF) output for CI/CD integration.
//!
//! SARIF is a standardized JSON format for static analysis tools, supported by:
//! - GitHub Code Scanning
//! - VS Code SARIF Viewer
//! - Azure DevOps
//! - Many CI/CD platforms
//!
//! See: <https://sarifweb.azurewebsites.net>/

use std::collections::BTreeSet;
use std::io::Write;

use crate::span_utils::LineOffsetTable;
use crate::{Diagnostic, Severity};

use super::{escape_json, trailing_comma, DiagnosticEmitter};

/// SARIF emitter for Static Analysis Results Interchange Format.
pub struct SarifEmitter<W: Write> {
    writer: W,
    tool_name: String,
    tool_version: String,
    artifact_uri: Option<String>,
    /// Source text for column computation (characters vs bytes).
    source: Option<String>,
    /// Pre-computed line offset table for O(log L) lookups.
    line_table: Option<LineOffsetTable>,
    results: Vec<SarifResult>,
}

/// Internal representation of a SARIF result before serialization.
struct SarifResult {
    rule_id: &'static str,
    level: &'static str,
    message: String,
    locations: Vec<SarifLocation>,
    related_locations: Vec<SarifLocation>,
}

/// Internal representation of a SARIF location.
struct SarifLocation {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
    message: Option<String>,
}

impl<W: Write> SarifEmitter<W> {
    /// Create a new SARIF emitter.
    pub fn new(writer: W, tool_name: impl Into<String>, tool_version: impl Into<String>) -> Self {
        SarifEmitter {
            writer,
            tool_name: tool_name.into(),
            tool_version: tool_version.into(),
            artifact_uri: None,
            source: None,
            line_table: None,
            results: Vec::new(),
        }
    }

    /// Set the artifact URI (file path) for locations.
    #[must_use]
    pub fn with_artifact(mut self, uri: impl Into<String>) -> Self {
        self.artifact_uri = Some(uri.into());
        self
    }

    /// Set the source text for computing line/column from byte offsets.
    ///
    /// Builds a line offset table for O(log L) lookups where L is the line count.
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        let source = source.into();
        self.line_table = Some(LineOffsetTable::build(&source));
        self.source = Some(source);
        self
    }

    /// Convert a byte offset to (line, column) using 1-based indexing.
    ///
    /// Uses pre-computed line offset table for O(log L) lookup.
    /// Without source text, returns (1, 1) as a placeholder.
    fn offset_to_line_col(&self, offset: u32) -> (usize, usize) {
        let (Some(table), Some(source)) = (&self.line_table, &self.source) else {
            // Without source/table, we cannot compute accurate positions.
            return (1, 1);
        };

        let (line, col) = table.offset_to_line_col(source, offset);
        (line as usize, col as usize)
    }

    /// Convert severity to SARIF level.
    fn severity_to_level(severity: Severity) -> &'static str {
        match severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note | Severity::Help => "note",
        }
    }

    /// Write the complete SARIF document.
    pub fn finish(&mut self) {
        let _ = writeln!(self.writer, "{{");
        let _ = writeln!(
            self.writer,
            "  \"$schema\": \"https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json\","
        );
        let _ = writeln!(self.writer, "  \"version\": \"2.1.0\",");
        let _ = writeln!(self.writer, "  \"runs\": [{{");

        // Tool information
        let _ = writeln!(self.writer, "    \"tool\": {{");
        let _ = writeln!(self.writer, "      \"driver\": {{");
        let _ = writeln!(
            self.writer,
            "        \"name\": \"{}\",",
            escape_json(&self.tool_name)
        );
        let _ = writeln!(
            self.writer,
            "        \"version\": \"{}\",",
            escape_json(&self.tool_version)
        );

        // Collect unique rules (BTreeSet gives deterministic order without sort+dedup)
        let rules: BTreeSet<&str> = self.results.iter().map(|r| r.rule_id).collect();

        let _ = writeln!(self.writer, "        \"rules\": [");
        for (i, rule_id) in rules.iter().enumerate() {
            let comma = trailing_comma(i, rules.len());
            let _ = writeln!(self.writer, "          {{");
            let _ = writeln!(self.writer, "            \"id\": \"{rule_id}\"");
            let _ = writeln!(self.writer, "          }}{comma}");
        }
        let _ = writeln!(self.writer, "        ]");

        let _ = writeln!(self.writer, "      }}");
        let _ = writeln!(self.writer, "    }},");

        // Results - take ownership to avoid borrow issues
        let results = std::mem::take(&mut self.results);
        let results_len = results.len();

        let _ = writeln!(self.writer, "    \"results\": [");
        for (i, result) in results.iter().enumerate() {
            let comma = trailing_comma(i, results_len);
            self.write_result(result);
            let _ = write!(self.writer, "{comma}");
            let _ = writeln!(self.writer);
        }
        let _ = writeln!(self.writer, "    ]");

        // Restore results in case finish() is called again
        self.results = results;

        let _ = writeln!(self.writer, "  }}]");
        let _ = writeln!(self.writer, "}}");
    }

    fn write_result(&mut self, result: &SarifResult) {
        let _ = writeln!(self.writer, "      {{");
        let _ = writeln!(self.writer, "        \"ruleId\": \"{}\",", result.rule_id);
        let _ = writeln!(self.writer, "        \"level\": \"{}\",", result.level);
        let _ = writeln!(self.writer, "        \"message\": {{");
        let _ = writeln!(
            self.writer,
            "          \"text\": \"{}\"",
            escape_json(&result.message)
        );
        let _ = writeln!(self.writer, "        }},");

        // Primary locations
        let _ = writeln!(self.writer, "        \"locations\": [");
        for (i, loc) in result.locations.iter().enumerate() {
            let comma = trailing_comma(i, result.locations.len());
            self.write_location(loc, false);
            let _ = writeln!(self.writer, "{comma}");
        }
        let _ = write!(self.writer, "        ]");

        // Related locations (secondary labels)
        if !result.related_locations.is_empty() {
            let _ = writeln!(self.writer, ",");
            let _ = writeln!(self.writer, "        \"relatedLocations\": [");
            for (i, loc) in result.related_locations.iter().enumerate() {
                let comma = trailing_comma(i, result.related_locations.len());
                self.write_location(loc, true);
                let _ = writeln!(self.writer, "{comma}");
            }
            let _ = write!(self.writer, "        ]");
        }

        let _ = writeln!(self.writer);
        let _ = write!(self.writer, "      }}");
    }

    fn write_location(&mut self, loc: &SarifLocation, include_id: bool) {
        let _ = writeln!(self.writer, "          {{");

        if include_id {
            let _ = writeln!(self.writer, "            \"id\": 0,");
        }

        let _ = writeln!(self.writer, "            \"physicalLocation\": {{");

        if let Some(uri) = &self.artifact_uri {
            let _ = writeln!(self.writer, "              \"artifactLocation\": {{");
            let _ = writeln!(
                self.writer,
                "                \"uri\": \"{}\"",
                escape_json(uri)
            );
            let _ = writeln!(self.writer, "              }},");
        }

        let _ = writeln!(self.writer, "              \"region\": {{");
        let _ = writeln!(
            self.writer,
            "                \"startLine\": {},",
            loc.start_line
        );
        let _ = writeln!(
            self.writer,
            "                \"startColumn\": {},",
            loc.start_column
        );
        let _ = writeln!(
            self.writer,
            "                \"endLine\": {},",
            loc.end_line
        );
        let _ = writeln!(
            self.writer,
            "                \"endColumn\": {}",
            loc.end_column
        );
        let _ = writeln!(self.writer, "              }}");
        let _ = writeln!(self.writer, "            }}");

        if let Some(msg) = &loc.message {
            let _ = write!(self.writer, ",");
            let _ = writeln!(self.writer);
            let _ = writeln!(self.writer, "            \"message\": {{");
            let _ = writeln!(
                self.writer,
                "              \"text\": \"{}\"",
                escape_json(msg)
            );
            let _ = write!(self.writer, "            }}");
        }

        let _ = writeln!(self.writer);
        let _ = write!(self.writer, "          }}");
    }
}

impl<W: Write> DiagnosticEmitter for SarifEmitter<W> {
    fn emit(&mut self, diagnostic: &Diagnostic) {
        let mut locations = Vec::new();
        let mut related_locations = Vec::new();

        for label in &diagnostic.labels {
            let (start_line, start_col) = self.offset_to_line_col(label.span.start);
            let (end_line, end_col) = self.offset_to_line_col(label.span.end);

            let loc = SarifLocation {
                start_line,
                start_column: start_col,
                end_line,
                end_column: end_col,
                message: if label.message.is_empty() {
                    None
                } else {
                    Some(label.message.clone())
                },
            };

            if label.is_primary {
                locations.push(loc);
            } else {
                related_locations.push(loc);
            }
        }

        // If no primary location, create a dummy one
        if locations.is_empty() {
            locations.push(SarifLocation {
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
                message: None,
            });
        }

        self.results.push(SarifResult {
            rule_id: diagnostic.code.as_str(),
            level: Self::severity_to_level(diagnostic.severity),
            message: diagnostic.message.clone(),
            locations,
            related_locations,
        });
    }

    fn flush(&mut self) {
        let _ = self.writer.flush();
    }

    fn emit_summary(&mut self, _error_count: usize, _warning_count: usize) {
        // SARIF doesn't need a separate summary - call finish() instead
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use crate::ErrorCode;
    use ori_ir::Span;

    /// Test fixture version - intentionally stable for snapshot testing.
    /// This is NOT the compiler version; it's a constant for test reproducibility.
    const TEST_TOOL_VERSION: &str = "0.1.0";

    fn sample_diagnostic() -> Diagnostic {
        Diagnostic::error(ErrorCode::E2001)
            .with_message("type mismatch: expected `int`, found `str`")
            .with_label(Span::new(10, 15), "expected `int`")
            .with_secondary_label(Span::new(0, 5), "defined here")
            .with_note("int and str are incompatible")
            .with_suggestion("use `int(x)` to convert")
    }

    #[test]
    fn test_sarif_emitter_basic() {
        let mut output = Vec::new();
        let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION)
            .with_artifact("src/main.ori")
            .with_source("let x = 42\nlet y = \"hello\"");

        emitter.emit(&sample_diagnostic());
        emitter.finish();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        // Check SARIF structure
        assert!(text.contains("\"$schema\":"));
        assert!(text.contains("sarif-schema-2.1.0"));
        assert!(text.contains("\"version\": \"2.1.0\""));
        assert!(text.contains("\"name\": \"oric\""));
        assert!(text.contains("\"ruleId\": \"E2001\""));
        assert!(text.contains("\"level\": \"error\""));
        assert!(text.contains("\"startLine\":"));
        assert!(text.contains("\"startColumn\":"));
    }

    #[test]
    fn test_sarif_emitter_multiple_diagnostics() {
        let mut output = Vec::new();
        let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION);

        let diag1 = Diagnostic::error(ErrorCode::E1001).with_message("parse error");
        let diag2 = Diagnostic::warning(ErrorCode::E3001).with_message("pattern warning");

        emitter.emit(&diag1);
        emitter.emit(&diag2);
        emitter.finish();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("\"ruleId\": \"E1001\""));
        assert!(text.contains("\"ruleId\": \"E3001\""));
        assert!(text.contains("\"level\": \"error\""));
        assert!(text.contains("\"level\": \"warning\""));
    }

    #[test]
    fn test_sarif_emitter_related_locations() {
        let mut output = Vec::new();
        let source = "let x = 10\nlet y = x + z";
        let mut emitter =
            SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION).with_source(source);

        let diag = Diagnostic::error(ErrorCode::E2003)
            .with_message("unknown identifier `z`")
            .with_label(Span::new(22, 23), "not found")
            .with_secondary_label(Span::new(4, 5), "similar: `x`");

        emitter.emit(&diag);
        emitter.finish();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        assert!(text.contains("\"locations\":"));
        assert!(text.contains("\"relatedLocations\":"));
        assert!(text.contains("not found"));
        assert!(text.contains("similar: `x`"));
    }

    #[test]
    fn test_sarif_emitter_line_column_conversion() {
        let mut output = Vec::new();
        let source = "line1\nline2\nline3";
        let mut emitter =
            SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION).with_source(source);

        // Span at "line2" (offset 6-11)
        let diag = Diagnostic::error(ErrorCode::E1001)
            .with_message("error on line 2")
            .with_label(Span::new(6, 11), "here");

        emitter.emit(&diag);
        emitter.finish();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        // line2 starts at line 2, column 1
        assert!(text.contains("\"startLine\": 2"));
        assert!(text.contains("\"startColumn\": 1"));
    }

    #[test]
    fn test_sarif_emitter_rules_deduplication() {
        let mut output = Vec::new();
        let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION);

        // Two diagnostics with same error code
        let diag1 = Diagnostic::error(ErrorCode::E2001).with_message("error 1");
        let diag2 = Diagnostic::error(ErrorCode::E2001).with_message("error 2");

        emitter.emit(&diag1);
        emitter.emit(&diag2);
        emitter.finish();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        // Should only have one rule definition for E2001
        let rule_count = text.matches("\"id\": \"E2001\"").count();
        assert_eq!(rule_count, 1, "rules should be deduplicated");

        // But should have two results
        let result_count = text.matches("\"ruleId\": \"E2001\"").count();
        assert_eq!(result_count, 2, "should have two results");
    }

    #[test]
    fn test_sarif_emitter_escapes_json() {
        let mut output = Vec::new();
        let mut emitter = SarifEmitter::new(&mut output, "oric", TEST_TOOL_VERSION);

        let diag =
            Diagnostic::error(ErrorCode::E1001).with_message("error with \"quotes\" and\nnewline");

        emitter.emit(&diag);
        emitter.finish();
        emitter.flush();

        let text = String::from_utf8(output).unwrap();

        // Should be properly escaped
        assert!(text.contains("\\\"quotes\\\""));
        assert!(text.contains("\\n"));
    }
}
