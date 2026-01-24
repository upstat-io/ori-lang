# Emitters

Emitters format diagnostics for different output targets: terminal, JSON, and SARIF.

## Emitter Trait

```rust
pub trait Emitter {
    fn emit(&mut self, diagnostic: &Diagnostic, source: &str);
    fn finish(&mut self);
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

impl Emitter for TerminalEmitter {
    fn emit(&mut self, diag: &Diagnostic, source: &str) {
        // Header: error[E2001]: message
        self.write_header(diag);

        // Location: --> file:line:col
        self.write_location(diag);

        // Source snippet with annotations
        self.write_snippet(diag, source);

        // Help text
        if let Some(help) = &diag.help {
            self.write_help(help);
        }

        // Suggested fixes
        for fix in &diag.fixes {
            self.write_fix_suggestion(fix);
        }

        writeln!(self.writer).ok();
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
            Severity::Info => "\x1b[1;36m",    // Bold cyan
            Severity::Hint => "\x1b[1;32m",    // Bold green
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
      "message": "type mismatch",
      "file": "src/main.si",
      "line": 10,
      "column": 15,
      "labels": [
        {
          "span": { "start": 150, "end": 157 },
          "message": "expected `int`, found `str`",
          "style": "primary"
        }
      ],
      "fixes": [
        {
          "message": "convert using `int()`",
          "edits": [
            { "span": { "start": 150, "end": 150 }, "newText": "int(" },
            { "span": { "start": 157, "end": 157 }, "newText": ")" }
          ],
          "applicability": "MaybeIncorrect"
        }
      ],
      "help": "consider using `int()` to convert"
    }
  ]
}
```

### Implementation

```rust
pub struct JsonEmitter {
    diagnostics: Vec<serde_json::Value>,
}

impl Emitter for JsonEmitter {
    fn emit(&mut self, diag: &Diagnostic, _source: &str) {
        self.diagnostics.push(json!({
            "code": diag.code.to_string(),
            "severity": diag.severity.to_string(),
            "message": diag.message,
            "span": {
                "start": diag.span.start,
                "end": diag.span.end,
            },
            "labels": diag.labels.iter().map(|l| json!({
                "span": { "start": l.span.start, "end": l.span.end },
                "message": l.message,
                "style": l.style.to_string(),
            }))collect::<Vec<_>>(),
            "fixes": diag.fixes.iter().map(|f| json!({
                "message": f.message,
                "edits": f.edits.iter().map(|e| json!({
                    "span": { "start": e.span.start, "end": e.span.end },
                    "newText": e.new_text,
                }))collect::<Vec<_>>(),
                "applicability": f.applicability.to_string(),
            }))collect::<Vec<_>>(),
            "help": diag.help,
        }));
    }

    fn finish(&mut self) {
        println!("{}", serde_json::to_string_pretty(&json!({
            "diagnostics": &self.diagnostics
        })).unwrap());
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
          "name": "sigilc",
          "version": "0.1.0",
          "rules": [
            {
              "id": "E2001",
              "shortDescription": { "text": "Type mismatch" },
              "helpUri": "https://sigil-lang.org/errors/E2001"
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
                "artifactLocation": { "uri": "src/main.si" },
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
                  "artifactLocation": { "uri": "src/main.si" },
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
    rules: HashSet<ErrorCode>,
}

impl Emitter for SarifEmitter {
    fn emit(&mut self, diag: &Diagnostic, source: &str) {
        self.rules.insert(diag.code);

        self.results.push(sarif::Result {
            rule_id: diag.code.to_string(),
            level: match diag.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "note",
                Severity::Hint => "none",
            }.into(),
            message: sarif::Message { text: diag.message.clone() },
            locations: vec![self.span_to_location(diag.span, source)],
            fixes: diag.fixes.iter().map(|f| self.convert_fix(f)).collect(),
            ..Default::default()
        });
    }

    fn finish(&mut self) {
        let sarif = sarif::Sarif {
            version: "2.1.0".into(),
            runs: vec![sarif::Run {
                tool: sarif::Tool {
                    driver: sarif::Driver {
                        name: "sigilc".into(),
                        version: env!("CARGO_PKG_VERSION").into(),
                        rules: self.build_rules(),
                    },
                },
                results: std::mem::take(&mut self.results),
            }],
        };

        println!("{}", serde_json::to_string_pretty(&sarif).unwrap());
    }
}
```

## Choosing an Emitter

```rust
pub fn create_emitter(format: OutputFormat) -> Box<dyn Emitter> {
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
sigil check src/main.si

# JSON
sigil check --format=json src/main.si

# SARIF
sigil check --format=sarif src/main.si > results.sarif
```
