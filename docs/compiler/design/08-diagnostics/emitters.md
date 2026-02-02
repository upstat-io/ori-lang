---
title: "Emitters"
description: "Ori Compiler Design — Emitters"
order: 802
section: "Diagnostics"
---

# Emitters

Emitters format diagnostics for different output targets: terminal, JSON, and SARIF.

## DiagnosticEmitter Trait

```rust
/// Trait for emitting diagnostics in various formats.
pub trait DiagnosticEmitter {
    /// Emit a single diagnostic.
    fn emit(&mut self, diagnostic: &Diagnostic);

    /// Emit multiple diagnostics.
    fn emit_all(&mut self, diagnostics: &[Diagnostic]) {
        for diag in diagnostics {
            self.emit(diag);
        }
    }

    /// Flush any buffered output.
    fn flush(&mut self);

    /// Emit a summary of errors/warnings.
    fn emit_summary(&mut self, error_count: usize, warning_count: usize);
}
```

## Terminal Emitter

Human-readable output with colors:

```
error[E2001]: type mismatch
 --> src/mainsi:10:15
   |
10 |     let x: int = "hello"
   |            ---   ^^^^^^^ expected `int`, found `str`
   |            |
   |            expected due to this annotation
   |
   = help: consider using `int()` to convert
```

### Implementation

```rust
pub struct TerminalEmitter {
    writer: Box<dyn Write>,
    colors: bool,
}

impl DiagnosticEmitter for TerminalEmitter {
    fn emit(&mut self, diag: &Diagnostic) {
        // Header: error[E2001]: message
        self.write_header(diag);

        // Labels with source snippets
        for label in &diag.labels {
            self.write_label(label);
        }

        // Notes
        for note in &diag.notes {
            self.write_note(note);
        }

        // Suggestions (human-readable)
        for suggestion in &diag.suggestions {
            self.write_suggestion(suggestion);
        }

        // Structured suggestions (for ori fix)
        for suggestion in &diag.structured_suggestions {
            self.write_structured_suggestion(suggestion);
        }

        writeln!(self.writer).ok();
    }

    fn flush(&mut self) {
        // TerminalEmitter writes directly, no buffering
    }

    fn emit_summary(&mut self, error_count: usize, warning_count: usize) {
        // e.g., "error: aborting due to 3 previous errors"
    }
}
```

### Color Scheme

```rust
impl TerminalEmitter {
    fn severity_color(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Error => "\x1b[1;31m",   // Bold red
            Severity::Warning => "\x1b[1;33m", // Bold yellow
            Severity::Note => "\x1b[1;36m",    // Bold cyan
            Severity::Help => "\x1b[1;32m",    // Bold green
        }
    }
}
```

## JSON Emitter

Machine-readable JSON output:

```json
{
  "diagnostics": [
    {
      "code": "E2001",
      "severity": "error",
      "message": "type mismatch: expected `int`, found `str`",
      "labels": [
        {
          "span": { "start": 150, "end": 157 },
          "message": "expected `int`",
          "isPrimary": true
        }
      ],
      "notes": [],
      "suggestions": ["consider using `int()` to convert"],
      "structuredSuggestions": [
        {
          "message": "convert using `int()`",
          "substitutions": [
            { "span": { "start": 150, "end": 150 }, "snippet": "int(" },
            { "span": { "start": 157, "end": 157 }, "snippet": ")" }
          ],
          "applicability": "MaybeIncorrect"
        }
      ]
    }
  ]
}
```

### Implementation

```rust
pub struct JsonEmitter {
    diagnostics: Vec<serde_json::Value>,
}

impl DiagnosticEmitter for JsonEmitter {
    fn emit(&mut self, diag: &Diagnostic) {
        self.diagnostics.push(json!({
            "code": diag.code.to_string(),
            "severity": diag.severity.to_string(),
            "message": diag.message,
            "labels": diag.labels.iter().map(|l| json!({
                "span": { "start": l.span.start, "end": l.span.end },
                "message": l.message,
                "isPrimary": l.is_primary,
            })).collect::<Vec<_>>(),
            "notes": diag.notes,
            "suggestions": diag.suggestions,
            "structuredSuggestions": diag.structured_suggestions.iter().map(|s| json!({
                "message": s.message,
                "substitutions": s.substitutions.iter().map(|sub| json!({
                    "span": { "start": sub.span.start, "end": sub.span.end },
                    "snippet": sub.snippet,
                })).collect::<Vec<_>>(),
                "applicability": format!("{:?}", s.applicability),
            })).collect::<Vec<_>>(),
        }));
    }

    fn flush(&mut self) {
        println!("{}", serde_json::to_string_pretty(&json!({
            "diagnostics": &self.diagnostics
        })).unwrap());
    }

    fn emit_summary(&mut self, _error_count: usize, _warning_count: usize) {
        // Summary is implicit in the JSON output
    }
}
```

## SARIF Emitter

[SARIF](https://sarifweb.azurewebsites.net/) (Static Analysis Results Interchange Format) for static analysis tools:

```json
{
  "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "oric",
          "version": "0.1.0",
          "rules": [
            {
              "id": "E2001",
              "shortDescription": { "text": "Type mismatch" },
              "helpUri": "https://ori-lang.org/errors/E2001"
            }
          ]
        }
      },
      "results": [
        {
          "ruleId": "E2001",
          "level": "error",
          "message": { "text": "expected `int`, found `str`" },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": { "uri": "src/main.ori" },
                "region": {
                  "startLine": 10,
                  "startColumn": 15,
                  "endLine": 10,
                  "endColumn": 22
                }
              }
            }
          ],
          "fixes": [
            {
              "description": { "text": "convert using `int()`" },
              "artifactChanges": [
                {
                  "artifactLocation": { "uri": "src/main.ori" },
                  "replacements": [
                    {
                      "deletedRegion": { "startLine": 10, "startColumn": 15, "endColumn": 15 },
                      "insertedContent": { "text": "int(" }
                    }
                  ]
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}
```

### Implementation

```rust
pub struct SarifEmitter {
    results: Vec<sarif::Result>,
    rules: BTreeSet<ErrorCode>,  // BTreeSet for stable rule ordering
}

impl DiagnosticEmitter for SarifEmitter {
    fn emit(&mut self, diag: &Diagnostic) {
        self.rules.insert(diag.code);

        self.results.push(sarif::Result {
            rule_id: diag.code.to_string(),
            level: match diag.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Note => "note",
                Severity::Help => "none",
            }.into(),
            message: sarif::Message { text: diag.message.clone() },
            locations: self.labels_to_locations(&diag.labels),
            fixes: diag.structured_suggestions.iter()
                .map(|s| self.convert_suggestion(s))
                .collect(),
            ..Default::default()
        });
    }

    fn flush(&mut self) {
        let sarif = sarif::Sarif {
            version: "2.1.0".into(),
            runs: vec![sarif::Run {
                tool: sarif::Tool {
                    driver: sarif::Driver {
                        name: "oric".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                        rules: self.build_rules(),
                    },
                },
                results: std::mem::take(&mut self.results),
            }],
        };

        println!("{}", serde_json::to_string_pretty(&sarif).unwrap());
    }

    fn emit_summary(&mut self, _error_count: usize, _warning_count: usize) {
        // Summary is implicit in SARIF format
    }
}
```

## Choosing an Emitter

```rust
pub fn create_emitter(format: OutputFormat) -> Box<dyn DiagnosticEmitter> {
    match format {
        OutputFormat::Terminal => Box::new(TerminalEmitter::new(true)),
        OutputFormat::Plain => Box::new(TerminalEmitter::new(false)),
        OutputFormat::Json => Box::new(JsonEmitter::new()),
        OutputFormat::Sarif => Box::new(SarifEmitter::new()),
    }
}
```

## CLI Usage

```bash
# Terminal (default)
ori check src/main.ori

# JSON
ori check --format=json src/main.ori

# SARIF
ori check --format=sarif src/main.ori > results.sarif
```
