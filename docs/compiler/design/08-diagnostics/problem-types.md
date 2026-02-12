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
    /// Lex-time problems (tokenization errors, confusables, cross-language habits).
    Lex(LexProblem),

    /// Parse-time problems (syntax errors).
    Parse(ParseProblem),

    /// Semantic analysis problems.
    Semantic(SemanticProblem),
}

impl Problem {
    pub fn span(&self) -> Span {
        match self {
            Problem::Lex(p) => p.span(),
            Problem::Parse(p) => p.span(),
            Problem::Semantic(p) => p.span(),
        }
    }

    pub fn is_lex(&self) -> bool { matches!(self, Problem::Lex(_)) }
    pub fn is_parse(&self) -> bool { matches!(self, Problem::Parse(_)) }
    pub fn is_semantic(&self) -> bool { matches!(self, Problem::Semantic(_)) }
}
```

**Note:** Type checking errors are **not** part of this enum. They use `TypeCheckError` directly from `ori_types`, allowing the type checker to use its own structured error variants.

### LexProblem (E0xxx)

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum LexProblem {
    InvalidCharacter {
        span: Span,
        ch: char,
    },
    UnterminatedString {
        span: Span,
    },
    Confusable {
        span: Span,
        found: char,
        suggested: char,
    },
    // ...
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
    NonExhaustiveMatch {
        span: Span,
        missing_patterns: Vec<String>,
    },
    RedundantPattern {
        span: Span,
        covered_by_span: Span,
    },
    // ...
}
```

### Pattern Problems

Pattern problems originate from the `PatternProblem` type in `ori_ir::canon` (produced by `ori_canon::exhaustiveness`). The `check` command converts them to `SemanticProblem` variants for unified diagnostic emission:

| PatternProblem | SemanticProblem | Severity |
|---------------|-----------------|----------|
| `NonExhaustive { match_span, missing }` | `NonExhaustiveMatch { span, missing_patterns }` | Error |
| `RedundantArm { arm_span, match_span, arm_index }` | `RedundantPattern { span, covered_by_span }` | Error |

Example diagnostics:

```
error: non-exhaustive match
 --> main.ori:5:1
  |
5 | match b {
  | ^^^^^ missing: false
```

```
error: redundant pattern
 --> main.ori:8:5
  |
8 |     _ -> "other"
  |     ^ this arm is unreachable
```

## Problem to Diagnostic Conversion

Problems are converted to diagnostics via the `Render` trait, which requires a `&StringInterner` parameter to resolve interned `Name` values:

```rust
pub trait Render {
    fn render(&self, interner: &StringInterner) -> Diagnostic;
}

impl Render for Problem {
    fn render(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            Problem::Lex(p) => p.render(interner),
            Problem::Parse(p) => p.render(interner),
            Problem::Semantic(p) => p.render(interner),
        }
    }
}
```

Each problem category has its own `Render` implementation in `oric/src/reporting/`.

## Error Code Documentation

Error codes are documented via the `ErrorDocs` registry, which loads embedded markdown files from `compiler/ori_diagnostic/src/errors/`. There is no `.explanation()` method on `ErrorCode` itself.

```rust
pub struct ErrorDocs;

impl ErrorDocs {
    /// Get documentation for an error code.
    pub fn get(code: ErrorCode) -> Option<&'static str>;

    /// Get all documented error codes.
    pub fn all_codes() -> impl Iterator<Item = ErrorCode>;

    /// Check if an error code has documentation.
    pub fn has_docs(code: ErrorCode) -> bool;
}
```

Each error code has a corresponding markdown file (e.g., `E2001.md`) in `compiler/ori_diagnostic/src/errors/`, accessible via `ori --explain E2001`.

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
