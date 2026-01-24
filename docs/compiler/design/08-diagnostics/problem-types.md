# Problem Types

Problems are categorized by compiler phase and converted to diagnostics for display.

## Problem Enum

```rust
pub enum Problem {
    // === Lexer Problems (E0xxx) ===
    InvalidCharacter {
        char: char,
        span: Span,
    },
    UnterminatedString {
        start: Span,
    },
    InvalidEscape {
        escape: char,
        span: Span,
    },
    InvalidNumber {
        text: String,
        span: Span,
    },

    // === Parser Problems (E1xxx) ===
    UnexpectedToken {
        expected: Vec<TokenKind>,
        found: TokenKind,
        span: Span,
    },
    UnexpectedEof {
        expected: Vec<TokenKind>,
    },
    ExpectedExpression {
        found: TokenKind,
        span: Span,
    },
    MissingClosingDelimiter {
        delimiter: TokenKind,
        opening: Span,
        span: Span,
    },
    InvalidPattern {
        span: Span,
    },

    // === Type Problems (E2xxx) ===
    TypeMismatch {
        expected: Type,
        found: Type,
        span: Span,
        context: Option<String>,
    },
    UndefinedVariable {
        name: Name,
        span: Span,
        similar: Vec<Name>,
    },
    UndefinedType {
        name: Name,
        span: Span,
        similar: Vec<Name>,
    },
    UndefinedFunction {
        name: Name,
        span: Span,
        similar: Vec<Name>,
    },
    MissingCapability {
        required: Capability,
        span: Span,
    },
    InfiniteType {
        var: TypeVarId,
        ty: Type,
        span: Span,
    },
    NotCallable {
        ty: Type,
        span: Span,
    },
    WrongArgCount {
        expected: usize,
        found: usize,
        span: Span,
    },
    MissingField {
        struct_name: Name,
        field: Name,
        span: Span,
    },

    // === Pattern Problems (E3xxx) ===
    UnknownPattern {
        name: Name,
        span: Span,
        similar: Vec<Name>,
    },
    MissingRequiredArg {
        pattern: Name,
        arg: String,
        span: Span,
    },
    UnexpectedArg {
        pattern: Name,
        arg: Name,
        span: Span,
    },
    InvalidPatternArg {
        pattern: Name,
        arg: String,
        expected: Type,
        found: Type,
        span: Span,
    },

    // === Evaluation Problems (E4xxx) ===
    DivisionByZero {
        span: Span,
    },
    IndexOutOfBounds {
        index: i64,
        length: usize,
        span: Span,
    },
    AssertionFailed {
        message: Option<String>,
        span: Span,
    },
    Panic {
        message: String,
        span: Span,
    },

    // === Import Problems (E5xxx) ===
    ModuleNotFound {
        path: String,
        span: Span,
    },
    ItemNotExported {
        module: String,
        item: Name,
        span: Span,
    },
    CircularImport {
        path: PathBuf,
        cycle: Vec<PathBuf>,
        span: Span,
    },

    // === Internal Problems (E9xxx) ===
    InternalError {
        message: String,
        span: Option<Span>,
    },
}
```

## Problem to Diagnostic Conversion

```rust
impl Problem {
    pub fn to_diagnostic(&self, interner: &Interner) -> Diagnostic {
        match self {
            Problem::TypeMismatch { expected, found, span, context } => {
                Diagnostic {
                    code: ErrorCode::E2001,
                    severity: Severity::Error,
                    message: format!(
                        "expected `{}`, found `{}`",
                        expected.display(),
                        found.display()
                    ),
                    span: *span,
                    labels: vec![
                        Label::primary(*span, format!("expected `{}`", expected.display())),
                    ],
                    fixes: self.suggest_type_conversion(expected, found, *span),
                    help: context.clone(),
                    related: vec![],
                }
            }

            Problem::UndefinedVariable { name, span, similar } => {
                let mut diagnostic = Diagnostic {
                    code: ErrorCode::E2002,
                    severity: Severity::Error,
                    message: format!(
                        "cannot find value `{}` in this scope",
                        interner.resolve(*name)
                    ),
                    span: *span,
                    labels: vec![
                        Label::primary(*span, "not found in this scope"),
                    ],
                    fixes: vec![],
                    help: None,
                    related: vec![],
                };

                // Add "did you mean?" suggestion
                if !similar.is_empty() {
                    let suggestions: Vec<_> = similar
                        .iter()
                        .map(|n| interner.resolve(*n))
                        .collect();
                    diagnostic.help = Some(format!(
                        "did you mean: {}?",
                        suggestions.join(", ")
                    ));
                }

                diagnostic
            }

            // ... more conversions
        }
    }
}
```

## Error Code Documentation

Each error code has documentation:

```rust
impl ErrorCode {
    pub fn explanation(&self) -> &'static str {
        match self {
            ErrorCode::E2001 => r#"
This error occurs when a value's type doesn't match what was expected.

Example:
```sigil
let x: int = "hello"  // Error: expected int, found str
```

To fix this, ensure the value matches the expected type, or use
explicit type conversion if appropriate.
"#,
            // ...
        }
    }
}
```

## Severity Levels

```rust
pub enum Severity {
    /// Compilation cannot continue
    Error,

    /// Potential problem, but compilation succeeds
    Warning,

    /// Informational note
    Info,

    /// Suggestion for improvement
    Hint,
}
```

### Error

Compilation fails:
```
error[E2001]: type mismatch
```

### Warning

Compilation succeeds, but something is suspicious:
```
warning: unused variable `x`
```

### Info

Additional context:
```
info: type inferred as `int`
```

### Hint

Suggestions:
```
hint: consider using `map` instead of `for..yield`
```

## Related Information

Link to related locations:

```rust
pub struct RelatedInfo {
    pub message: String,
    pub span: Span,
}

// Example: "expected due to this annotation"
diagnostic.related.push(RelatedInfo {
    message: "expected due to this annotation".into(),
    span: type_annotation_span,
});
```

## Labels

Mark specific locations:

```rust
pub struct Label {
    pub span: Span,
    pub message: String,
    pub style: LabelStyle,
}

pub enum LabelStyle {
    Primary,   // Main error location
    Secondary, // Related context
}
```
