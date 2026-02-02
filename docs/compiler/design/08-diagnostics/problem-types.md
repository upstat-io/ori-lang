---
title: "Problem Types"
description: "Ori Compiler Design — Problem Types"
order: 803
section: "Diagnostics"
---

# Problem Types

Problems are categorized by compiler phase and converted to diagnostics for display.

## Problem Enum

The `Problem` enum uses a **three-tier hierarchy** for organization by compiler phase:

```rust
/// Unified problem enum for all compilation phases.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Problem {
    /// Parse-time problems (syntax errors).
    Parse(ParseProblem),

    /// Type checking problems.
    Type(TypeProblem),

    /// Semantic analysis problems.
    Semantic(SemanticProblem),
}

impl Problem {
    pub fn span(&self) -> Span {
        match self {
            Problem::Parse(p) => p.span(),
            Problem::Type(p) => p.span(),
            Problem::Semantic(p) => p.span(),
        }
    }

    pub fn is_parse(&self) -> bool { matches!(self, Problem::Parse(_)) }
    pub fn is_type(&self) -> bool { matches!(self, Problem::Type(_)) }
    pub fn is_semantic(&self) -> bool { matches!(self, Problem::Semantic(_)) }
}
```

### ParseProblem (E1xxx)

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParseProblem {
    UnexpectedToken {
        span: Span,
        expected: String,
        found: String,
    },
    ExpectedExpression {
        span: Span,
        found: String,
    },
    UnclosedDelimiter {
        found_span: Span,
        open_span: Span,
        delimiter: char,
    },
    // ...
}
```

### TypeProblem (E2xxx)

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeProblem {
    TypeMismatch {
        span: Span,
        expected: String,
        found: String,
    },
    ArgCountMismatch {
        span: Span,
        expected: usize,
        found: usize,
    },
    InfiniteType { span: Span },
    CannotInfer { span: Span, context: String },
    UnknownType { span: Span, name: String },
    NotCallable { span: Span, found_type: String },
    NoSuchField {
        span: Span,
        type_name: String,
        field_name: String,
        available_fields: Vec<String>,
    },
    NoSuchMethod {
        span: Span,
        type_name: String,
        method_name: String,
        available_methods: Vec<String>,
    },
    InvalidBinaryOp {
        span: Span,
        op: String,
        left_type: String,
        right_type: String,
    },
    MissingNamedArg { span: Span, arg_name: String },
    ReturnTypeMismatch {
        span: Span,
        expected: String,
        found: String,
        func_name: String,
    },
    ConditionNotBool { span: Span, found_type: String },
    MatchArmTypeMismatch {
        span: Span,
        first_type: String,
        this_type: String,
        first_span: Span,
    },
    CyclicType { span: Span, type_name: String },
    ClosureSelfReference { span: Span },
    // ...
}
```

### SemanticProblem

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum SemanticProblem {
    UnknownIdentifier {
        span: Span,
        name: String,
        similar: Option<String>,
    },
    DuplicateDefinition {
        span: Span,
        name: String,
        kind: DefinitionKind,
        first_span: Span,
    },
    // ...
}
```
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
```ori
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

    /// Additional context information
    Note,

    /// Suggestion for improvement
    Help,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Note => write!(f, "note"),
            Severity::Help => write!(f, "help"),
        }
    }
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

### Note

Additional context:
```
note: type inferred as `int`
```

### Help

Suggestions:
```
help: consider using `map` instead of `for..yield`
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
    pub is_primary: bool,
}

impl Label {
    /// Create a primary label (the main error location).
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Label { span, message: message.into(), is_primary: true }
    }

    /// Create a secondary label (related context).
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Label { span, message: message.into(), is_primary: false }
    }
}
```
