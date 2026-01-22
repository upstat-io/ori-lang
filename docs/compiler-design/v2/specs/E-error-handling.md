# E: Error Handling Specification

This document specifies the diagnostic and error handling system for the V2 compiler.

---

## Diagnostic Structure

```rust
/// Compiler diagnostic (error, warning, note)
#[derive(Clone, Debug)]
pub struct Diagnostic {
    /// Severity level
    pub severity: Severity,

    /// Error code for categorization and lookup
    pub code: ErrorCode,

    /// Primary message
    pub message: String,

    /// Source labels (primary and secondary)
    pub labels: Vec<Label>,

    /// Additional notes
    pub notes: Vec<String>,

    /// Suggested fixes
    pub suggestions: Vec<Suggestion>,
}

/// Severity levels
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    /// Informational note
    Note,
    /// Warning (compilation continues)
    Warning,
    /// Error (compilation fails)
    Error,
    /// Internal compiler error
    Ice,
}

/// Source code label
#[derive(Clone, Debug)]
pub struct Label {
    /// Source span
    pub span: Span,
    /// Label message
    pub message: String,
    /// Label style
    pub style: LabelStyle,
}

#[derive(Copy, Clone)]
pub enum LabelStyle {
    /// Primary location (highlighted)
    Primary,
    /// Secondary/related location
    Secondary,
}

/// Suggested fix
#[derive(Clone, Debug)]
pub struct Suggestion {
    /// Description of the fix
    pub message: String,
    /// Replacement text
    pub replacement: String,
    /// Span to replace
    pub span: Span,
    /// Confidence level
    pub confidence: Confidence,
}

#[derive(Copy, Clone)]
pub enum Confidence {
    /// Definitely correct
    MachineApplicable,
    /// Probably correct
    MaybeIncorrect,
    /// User should review
    HasPlaceholders,
}
```

---

## Error Codes

### Code Ranges

| Range | Category |
|-------|----------|
| E1xxx | Syntax errors |
| E2xxx | Type errors |
| E3xxx | Name resolution errors |
| E4xxx | Test/coverage errors |
| E5xxx | Capability errors |
| E6xxx | Pattern errors |
| E7xxx | Import errors |
| E8xxx | Codegen errors |
| E9xxx | Internal errors |

### Syntax Errors (E1xxx)

```rust
pub enum SyntaxError {
    /// E1001: Unexpected token
    E1001,
    /// E1002: Unclosed delimiter
    E1002,
    /// E1003: Expected expression
    E1003,
    /// E1004: Expected type
    E1004,
    /// E1005: Expected identifier
    E1005,
    /// E1006: Expected pattern argument name
    E1006,
    /// E1007: Invalid number literal
    E1007,
    /// E1008: Unterminated string literal
    E1008,
    /// E1009: Invalid escape sequence
    E1009,
    /// E1010: Invalid character literal
    E1010,
    /// E1011: Unexpected end of file
    E1011,
    /// E1012: Invalid function definition
    E1012,
    /// E1013: Invalid type definition
    E1013,
    /// E1014: Missing function body
    E1014,
    /// E1015: Invalid pattern syntax
    E1015,
}
```

### Type Errors (E2xxx)

```rust
pub enum TypeError {
    /// E2001: Type mismatch
    E2001,
    /// E2002: Unknown type
    E2002,
    /// E2003: Cannot infer type
    E2003,
    /// E2004: Incompatible types in binary operation
    E2004,
    /// E2005: Not callable
    E2005,
    /// E2006: Wrong number of arguments
    E2006,
    /// E2007: Missing required argument
    E2007,
    /// E2008: Unknown field
    E2008,
    /// E2009: Not indexable
    E2009,
    /// E2010: Pattern requires list
    E2010,
    /// E2011: Transform must take one argument
    E2011,
    /// E2012: Expected function
    E2012,
    /// E2013: Recursive type
    E2013,
    /// E2014: Trait bound not satisfied
    E2014,
    /// E2015: Ambiguous type
    E2015,
    /// E2016: Cannot unify types
    E2016,
    /// E2017: Expected Option type
    E2017,
    /// E2018: Expected Result type
    E2018,
    /// E2019: Cannot use ? outside try
    E2019,
    /// E2020: Fold op must take two arguments
    E2020,
    /// E2021: Expected accumulator function
    E2021,
}
```

### Name Errors (E3xxx)

```rust
pub enum NameError {
    /// E3001: Undefined variable
    E3001,
    /// E3002: Undefined function
    E3002,
    /// E3003: Duplicate definition
    E3003,
    /// E3004: Undefined type
    E3004,
    /// E3005: Private item
    E3005,
    /// E3006: Undefined config
    E3006,
    /// E3007: Cannot shadow
    E3007,
    /// E3008: Use of moved value
    E3008,
    /// E3009: Ambiguous name
    E3009,
}
```

### Test Errors (E4xxx)

```rust
pub enum TestError {
    /// E4001: Missing tests for function
    E4001,
    /// E4002: Test failure
    E4002,
    /// E4003: Test timeout
    E4003,
    /// E4004: Invalid test target
    E4004,
    /// E4005: Test panicked
    E4005,
}
```

### Error Code Definition

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ErrorCode(u16);

impl ErrorCode {
    // Syntax
    pub const E1001: Self = Self(1001);
    pub const E1002: Self = Self(1002);
    // ... etc

    // Type
    pub const E2001: Self = Self(2001);
    // ... etc

    pub fn as_str(&self) -> &'static str {
        match self.0 {
            1001 => "E1001",
            1002 => "E1002",
            2001 => "E2001",
            // ...
            _ => "E????",
        }
    }

    pub fn description(&self) -> &'static str {
        match self.0 {
            1001 => "unexpected token",
            1002 => "unclosed delimiter",
            2001 => "type mismatch",
            // ...
            _ => "unknown error",
        }
    }

    pub fn url(&self) -> String {
        format!("https://sigil-lang.org/errors/{}", self.as_str())
    }
}
```

---

## Diagnostic Builder

```rust
impl Diagnostic {
    /// Create error diagnostic
    pub fn error(code: ErrorCode) -> DiagnosticBuilder {
        DiagnosticBuilder::new(Severity::Error, code)
    }

    /// Create warning diagnostic
    pub fn warning(code: ErrorCode) -> DiagnosticBuilder {
        DiagnosticBuilder::new(Severity::Warning, code)
    }

    /// Check if this is an error
    pub fn is_error(&self) -> bool {
        matches!(self.severity, Severity::Error | Severity::Ice)
    }
}

pub struct DiagnosticBuilder {
    severity: Severity,
    code: ErrorCode,
    message: Option<String>,
    labels: Vec<Label>,
    notes: Vec<String>,
    suggestions: Vec<Suggestion>,
}

impl DiagnosticBuilder {
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn with_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            span,
            message: message.into(),
            style: if self.labels.is_empty() {
                LabelStyle::Primary
            } else {
                LabelStyle::Secondary
            },
        });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_suggestion(
        mut self,
        span: Span,
        message: impl Into<String>,
        replacement: impl Into<String>,
    ) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
            replacement: replacement.into(),
            span,
            confidence: Confidence::MachineApplicable,
        });
        self
    }

    pub fn build(self) -> Diagnostic {
        Diagnostic {
            severity: self.severity,
            code: self.code,
            message: self.message.unwrap_or_else(|| self.code.description().to_string()),
            labels: self.labels,
            notes: self.notes,
            suggestions: self.suggestions,
        }
    }
}
```

---

## Common Error Constructors

```rust
/// Type mismatch error
pub fn type_mismatch(
    span: Span,
    expected: TypeId,
    found: TypeId,
    context: &str,
    interner: &TypeInterner,
) -> Diagnostic {
    Diagnostic::error(ErrorCode::E2001)
        .with_message(format!(
            "type mismatch: expected `{}`, found `{}`",
            format_type(expected, interner),
            format_type(found, interner),
        ))
        .with_label(span, format!("expected `{}`", format_type(expected, interner)))
        .with_note(format!("in {}", context))
        .build()
}

/// Undefined variable error
pub fn undefined_variable(name: Name, span: Span, interner: &StringInterner) -> Diagnostic {
    let name_str = interner.resolve(name);

    Diagnostic::error(ErrorCode::E3001)
        .with_message(format!("undefined variable `{}`", name_str))
        .with_label(span, "not found in this scope")
        .build()
}

/// Missing test error
pub fn missing_tests(func_name: Name, span: Span, interner: &StringInterner) -> Diagnostic {
    let name_str = interner.resolve(func_name);

    Diagnostic::error(ErrorCode::E4001)
        .with_message(format!("function `{}` has no tests", name_str))
        .with_label(span, "this function requires at least one test")
        .with_note("every function (except main) must have associated tests")
        .build()
}
```

---

## Error Recovery

### Parser Recovery

```rust
impl Parser<'_> {
    /// Recover from error by skipping to synchronization point
    fn recover_to(&mut self, sync_tokens: &[TokenKind]) {
        while !self.at_end() {
            if sync_tokens.contains(&self.current().kind) {
                return;
            }
            // Also sync on newline for statement-level recovery
            if self.at_newline() {
                self.advance();
                return;
            }
            self.advance();
        }
    }

    /// Parse function with recovery
    fn parse_function(&mut self) -> Option<Function> {
        let start = self.expect(TokenKind::At)?;

        let name = match self.expect_ident() {
            Some(n) => n,
            None => {
                self.error(ErrorCode::E1005, "expected function name after '@'");
                self.recover_to(&[TokenKind::Eq, TokenKind::LParen, TokenKind::At]);
                return None;
            }
        };

        // ... continue parsing with recovery at each point
    }
}
```

### Type Checker Recovery

```rust
impl TypeContext<'_> {
    /// Type check with error accumulation
    pub fn check_module(&mut self, module: Module) -> TypeCheckResult {
        let mut typed_functions = Vec::new();

        for func in module.functions(self.db) {
            // Each function type-checks independently
            match self.check_function(func) {
                Ok(typed) => typed_functions.push(typed),
                Err(diags) => {
                    // Record errors but continue
                    self.diagnostics.extend(diags);
                    // Create error-typed function for downstream
                    typed_functions.push(self.error_function(func));
                }
            }
        }

        TypeCheckResult {
            functions: typed_functions,
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }

    /// Create placeholder for errored function
    fn error_function(&self, func: Function) -> TypedFunction {
        TypedFunction {
            func,
            body_type: self.interner.intern(TypeKind::Infer),
            has_errors: true,
        }
    }
}
```

---

## Diagnostic Rendering

### Terminal Output

```rust
use codespan_reporting::term;

/// Render diagnostic to terminal
pub fn render_diagnostic(
    diagnostic: &Diagnostic,
    files: &SimpleFiles<String, String>,
    config: &term::Config,
) {
    let codespan_diag = to_codespan_diagnostic(diagnostic);
    term::emit(&mut StandardStream::stderr(ColorChoice::Auto), config, files, &codespan_diag)
        .unwrap();
}

fn to_codespan_diagnostic(d: &Diagnostic) -> codespan_reporting::diagnostic::Diagnostic<FileId> {
    let severity = match d.severity {
        Severity::Error => codespan_reporting::diagnostic::Severity::Error,
        Severity::Warning => codespan_reporting::diagnostic::Severity::Warning,
        Severity::Note => codespan_reporting::diagnostic::Severity::Note,
        Severity::Ice => codespan_reporting::diagnostic::Severity::Bug,
    };

    let mut diag = codespan_reporting::diagnostic::Diagnostic::new(severity)
        .with_code(d.code.as_str())
        .with_message(&d.message);

    for label in &d.labels {
        let style = match label.style {
            LabelStyle::Primary => codespan_reporting::diagnostic::LabelStyle::Primary,
            LabelStyle::Secondary => codespan_reporting::diagnostic::LabelStyle::Secondary,
        };
        diag = diag.with_labels(vec![
            codespan_reporting::diagnostic::Label::new(style, file_id, label.span.start..label.span.end)
                .with_message(&label.message)
        ]);
    }

    for note in &d.notes {
        diag = diag.with_notes(vec![note.clone()]);
    }

    diag
}
```

### Example Output

```
error[E2001]: type mismatch: expected `int`, found `str`
  --> src/main.si:10:12
   |
10 |     let x: int = "hello"
   |            ---   ^^^^^^^ expected `int`
   |            |
   |            declared as `int` here
   |
   = note: in variable binding

error[E4001]: function `add` has no tests
 --> src/math.si:5:1
  |
5 | @add (a: int, b: int) -> int = a + b
  | ^^^^ this function requires at least one test
  |
  = note: every function (except main) must have associated tests
```

---

## LSP Diagnostics

```rust
use lsp_types::{Diagnostic as LspDiagnostic, DiagnosticSeverity};

/// Convert to LSP diagnostic
pub fn to_lsp_diagnostic(d: &Diagnostic, file_uri: &Url) -> LspDiagnostic {
    let severity = match d.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Note => DiagnosticSeverity::INFORMATION,
        Severity::Ice => DiagnosticSeverity::ERROR,
    };

    let range = d.labels.first()
        .map(|l| span_to_range(l.span))
        .unwrap_or_default();

    LspDiagnostic {
        range,
        severity: Some(severity),
        code: Some(lsp_types::NumberOrString::String(d.code.as_str().to_string())),
        code_description: Some(lsp_types::CodeDescription {
            href: Url::parse(&d.code.url()).unwrap(),
        }),
        source: Some("sigil".to_string()),
        message: d.message.clone(),
        related_information: Some(
            d.labels.iter().skip(1).map(|l| {
                lsp_types::DiagnosticRelatedInformation {
                    location: lsp_types::Location {
                        uri: file_uri.clone(),
                        range: span_to_range(l.span),
                    },
                    message: l.message.clone(),
                }
            }).collect()
        ),
        tags: None,
        data: None,
    }
}
```
