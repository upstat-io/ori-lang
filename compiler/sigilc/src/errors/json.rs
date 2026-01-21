// JSON error rendering for AI tooling and structured output
//
// Provides JSON serialization of diagnostics for integration with
// IDEs, LSP servers, and AI-assisted development tools.

use super::{Diagnostic, DiagnosticCollector, Label, LabelStyle, Level, Suggestion};

/// Render a single diagnostic as JSON.
pub fn render_diagnostic(diag: &Diagnostic) -> String {
    serde_json::to_string(&DiagnosticJson::from(diag)).unwrap_or_else(|_| {
        // Fallback if serialization somehow fails
        format!(r#"{{"error":"{}","code":"{}"}}"#, diag.message, diag.code)
    })
}

/// Render a single diagnostic as pretty-printed JSON.
pub fn render_diagnostic_pretty(diag: &Diagnostic) -> String {
    serde_json::to_string_pretty(&DiagnosticJson::from(diag)).unwrap_or_else(|_| {
        render_diagnostic(diag)
    })
}

/// Render multiple diagnostics as a JSON array.
pub fn render_diagnostics(diagnostics: &DiagnosticCollector) -> String {
    let json_diagnostics: Vec<DiagnosticJson> = diagnostics
        .diagnostics()
        .iter()
        .map(DiagnosticJson::from)
        .collect();

    serde_json::to_string(&json_diagnostics).unwrap_or_else(|_| "[]".to_string())
}

/// Render multiple diagnostics as a pretty-printed JSON array.
pub fn render_diagnostics_pretty(diagnostics: &DiagnosticCollector) -> String {
    let json_diagnostics: Vec<DiagnosticJson> = diagnostics
        .diagnostics()
        .iter()
        .map(DiagnosticJson::from)
        .collect();

    serde_json::to_string_pretty(&json_diagnostics).unwrap_or_else(|_| "[]".to_string())
}

/// Render a full compilation result as JSON.
pub fn render_compilation_result<T: serde::Serialize>(
    success: bool,
    _value: Option<&T>,
    diagnostics: &DiagnosticCollector,
) -> String {
    let result = CompilationResultJson {
        success,
        error_count: diagnostics.error_count(),
        warning_count: diagnostics.warning_count(),
        diagnostics: diagnostics
            .diagnostics()
            .iter()
            .map(DiagnosticJson::from)
            .collect(),
    };

    serde_json::to_string(&result).unwrap_or_else(|_| {
        format!(r#"{{"success":{},"error_count":{}}}"#, success, diagnostics.error_count())
    })
}

// Internal JSON representation types

#[derive(serde::Serialize)]
struct DiagnosticJson {
    level: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    labels: Vec<LabelJson>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    help: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suggestions: Vec<SuggestionJson>,
}

#[derive(serde::Serialize)]
struct LabelJson {
    #[serde(rename = "type")]
    style: String,
    file: String,
    start: usize,
    end: usize,
    message: String,
}

#[derive(serde::Serialize)]
struct SuggestionJson {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    replacement: Option<ReplacementJson>,
}

#[derive(serde::Serialize)]
struct ReplacementJson {
    file: String,
    start: usize,
    end: usize,
    text: String,
}

#[derive(serde::Serialize)]
struct CompilationResultJson {
    success: bool,
    error_count: usize,
    warning_count: usize,
    diagnostics: Vec<DiagnosticJson>,
}

impl From<&Diagnostic> for DiagnosticJson {
    fn from(diag: &Diagnostic) -> Self {
        DiagnosticJson {
            level: match diag.level {
                Level::Error => "error".to_string(),
                Level::Warning => "warning".to_string(),
                Level::Note => "note".to_string(),
                Level::Help => "help".to_string(),
            },
            code: diag.code.as_string(),
            message: diag.message.clone(),
            labels: diag.labels.iter().map(LabelJson::from).collect(),
            notes: diag.notes.clone(),
            help: diag.help.clone(),
            suggestions: diag.suggestions.iter().map(SuggestionJson::from).collect(),
        }
    }
}

impl From<&Label> for LabelJson {
    fn from(label: &Label) -> Self {
        LabelJson {
            style: match label.style {
                LabelStyle::Primary => "primary".to_string(),
                LabelStyle::Secondary => "secondary".to_string(),
            },
            file: label.span.filename.clone(),
            start: label.span.range.start,
            end: label.span.range.end,
            message: label.message.clone(),
        }
    }
}

impl From<&Suggestion> for SuggestionJson {
    fn from(suggestion: &Suggestion) -> Self {
        SuggestionJson {
            message: suggestion.message.clone(),
            replacement: suggestion.replacement.as_ref().map(|r| ReplacementJson {
                file: r.span.filename.clone(),
                start: r.span.range.start,
                end: r.span.range.end,
                text: r.text.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::{codes::ErrorCode, Span};

    #[test]
    fn test_render_simple_diagnostic() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch");
        let json = render_diagnostic(&diag);

        assert!(json.contains("\"level\":\"error\""));
        assert!(json.contains("\"code\":\"E3001\""));
        assert!(json.contains("\"message\":\"type mismatch\""));
    }

    #[test]
    fn test_render_diagnostic_with_labels() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch")
            .with_label(Span::new("test.si", 10..20), "expected int");

        let json = render_diagnostic(&diag);

        assert!(json.contains("\"labels\""));
        assert!(json.contains("\"file\":\"test.si\""));
        assert!(json.contains("\"start\":10"));
        assert!(json.contains("\"end\":20"));
    }

    #[test]
    fn test_render_diagnostic_with_suggestion() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch")
            .with_suggestion(
                "convert to int",
                Some(Span::new("test.si", 10..15)),
                Some("int(x)".to_string()),
            );

        let json = render_diagnostic(&diag);

        assert!(json.contains("\"suggestions\""));
        assert!(json.contains("\"convert to int\""));
    }

    #[test]
    fn test_render_diagnostics_array() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::error(ErrorCode::E3001, "error 1"));
        collector.push(Diagnostic::warning(ErrorCode::E3005, "warning 1"));

        let json = render_diagnostics(&collector);

        assert!(json.starts_with('['));
        assert!(json.ends_with(']'));
        assert!(json.contains("\"error 1\""));
        assert!(json.contains("\"warning 1\""));
    }

    #[test]
    fn test_render_compilation_result() {
        let mut collector = DiagnosticCollector::new();
        collector.push(Diagnostic::error(ErrorCode::E3001, "error"));

        let json = render_compilation_result::<()>(false, None, &collector);

        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error_count\":1"));
        assert!(json.contains("\"warning_count\":0"));
    }

    #[test]
    fn test_pretty_print() {
        let diag = Diagnostic::error(ErrorCode::E3001, "type mismatch");
        let json = render_diagnostic_pretty(&diag);

        // Pretty printed JSON should have newlines
        assert!(json.contains('\n'));
    }
}
