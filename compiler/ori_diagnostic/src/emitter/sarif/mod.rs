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
///
/// The `'src` lifetime ties to the source text, which is borrowed (not cloned).
pub struct SarifEmitter<'src, W: Write> {
    writer: W,
    tool_name: String,
    tool_version: String,
    artifact_uri: Option<String>,
    /// Source text for column computation (borrowed, not cloned).
    source: Option<&'src str>,
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
    /// Artifact URI for cross-file locations (overrides default).
    artifact_uri: Option<String>,
}

impl<'src, W: Write> SarifEmitter<'src, W> {
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
    /// The source is borrowed, not cloned.
    #[must_use]
    pub fn with_source(mut self, source: &'src str) -> Self {
        self.line_table = Some(LineOffsetTable::build(source));
        self.source = Some(source);
        self
    }

    /// Convert a byte offset to (line, column) using 1-based indexing.
    ///
    /// Uses pre-computed line offset table for O(log L) lookup.
    /// Without source text, returns (1, 1) as a placeholder.
    fn offset_to_line_col(&self, offset: u32) -> (usize, usize) {
        let (Some(table), Some(source)) = (&self.line_table, self.source) else {
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

        // Use per-location artifact URI for cross-file labels, otherwise use default
        let uri = loc.artifact_uri.as_ref().or(self.artifact_uri.as_ref());
        if let Some(uri) = uri {
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

impl<W: Write> DiagnosticEmitter for SarifEmitter<'_, W> {
    fn emit(&mut self, diagnostic: &Diagnostic) {
        let mut locations = Vec::new();
        let mut related_locations = Vec::new();

        for label in &diagnostic.labels {
            // Cross-file labels have spans relative to SourceInfo.content, not the main file.
            // Build a temporary LineOffsetTable from the cross-file source for correct positions.
            let (start_line, start_col, end_line, end_col) = if let Some(ref src_info) =
                label.source_info
            {
                let cross_table = LineOffsetTable::build(&src_info.content);
                let (sl, sc) = cross_table.offset_to_line_col(&src_info.content, label.span.start);
                let (el, ec) = cross_table.offset_to_line_col(&src_info.content, label.span.end);
                (sl as usize, sc as usize, el as usize, ec as usize)
            } else {
                let (sl, sc) = self.offset_to_line_col(label.span.start);
                let (el, ec) = self.offset_to_line_col(label.span.end);
                (sl, sc, el, ec)
            };

            // For cross-file labels, use the source_info's path as the artifact URI
            let artifact_uri = label.source_info.as_ref().map(|src| src.path.clone());

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
                artifact_uri,
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
                artifact_uri: None,
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
mod tests;
