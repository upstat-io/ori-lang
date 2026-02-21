---
title: "Problem Types"
description: "Ori Compiler Design — Problem Types"
order: 803
section: "Diagnostics"
---

# Problem Types

Problems are categorized by compiler phase and converted to diagnostics for display.

## Problem Type Organization

There is no unified `Problem` enum. Each compiler phase defines its own problem type independently, and each type implements its own `into_diagnostic()` method:

| Problem Type | Location | Error Codes | Conversion |
|---|---|---|---|
| `LexProblem` | `oric/src/problem/lex.rs` | E0xxx | `into_diagnostic()` |
| `SemanticProblem` | `oric/src/problem/semantic/mod.rs` | E2xxx, E3xxx | `into_diagnostic(&interner)` |
| `EvalError` | `ori_patterns` | E6xxx | `eval_error_to_diagnostic()` in `oric/src/problem/eval/mod.rs` |
| `CodegenProblem` | `oric/src/problem/codegen/mod.rs` | E4xxx, E5xxx | `into_diagnostic()` (behind `llvm` feature) |

Parse errors are rendered directly by `ori_parse::ParseError::to_queued_diagnostic()` and do not flow through the `oric` problem module.

Type checking errors use `TypeCheckError` from `ori_types` directly, rendered by `TypeErrorRenderer` in `oric/src/reporting/typeck/` for Pool-aware type name resolution.

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
    MissingTest {
        span: Span,
        func_name: Name,
    },
    // ... additional variants for capabilities, break/continue, etc.
}
```

Many `SemanticProblem` variants have full `into_diagnostic()` implementations, including `UnknownIdentifier`, `DuplicateDefinition`, `ImmutableMutation`, `BreakOutsideLoop`, `ContinueOutsideLoop`, and others. Currently produced in production code:
- `MissingTest` -- emitted by `check_test_coverage()` during test coverage analysis

The `NonExhaustiveMatch` and `RedundantPattern` variants have `into_diagnostic()` implementations but are not currently produced — pattern problems are converted directly to diagnostics via `pattern_problem_to_diagnostic()` without going through `SemanticProblem`. Most other variants serve as defensive infrastructure for a future dedicated semantic analysis pass.

Some variants like `UnknownIdentifier` and `DuplicateDefinition` overlap with the type checker's own `TypeCheckError` type, which handles these cases in the current implementation.

### Pattern Problems

Pattern problems originate from the `PatternProblem` type in `ori_ir::canon` (produced by `ori_canon::exhaustiveness`). The `check` command converts them directly to diagnostics via `pattern_problem_to_diagnostic()`. The corresponding `SemanticProblem` variants exist for future unification:

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

### EvalError (E6xxx)

Runtime/eval errors originate as `EvalError` (from `ori_patterns`) and are converted to diagnostics in `oric/src/problem/eval/mod.rs` via `eval_error_to_diagnostic()`. The `EvalErrorKind` enum maps to E6xxx error codes:

| Range | Category | Examples |
|-------|----------|----------|
| E6001-E6009 | Arithmetic | Division by zero, overflow |
| E6010-E6019 | Type/operator | Type mismatch, invalid binary op |
| E6020-E6029 | Access | Undefined variable/function/field/method, index out of bounds |
| E6030-E6039 | Function calls | Arity mismatch, stack overflow, not callable |
| E6040-E6049 | Pattern/match | Non-exhaustive match |
| E6050-E6059 | Assertion/test | Assertion failed, panic called |
| E6060-E6069 | Capability | Missing capability |
| E6070-E6079 | Const-eval | Budget exceeded |
| E6080-E6089 | Not-implemented | Feature not yet available |
| E6099 | Custom | Uncategorized runtime error |

The conversion adds primary span labels, context notes, backtrace information, and actionable suggestions for fixable errors. `snapshot_to_diagnostic()` provides an enriched variant that resolves backtrace spans to `file:line:col` using `LineOffsetTable`.

## Problem to Diagnostic Conversion

Problem types implement `into_diagnostic()` methods directly rather than through a shared trait. Each problem type converts itself to a `Diagnostic`:

```rust
impl SemanticProblem {
    pub fn into_diagnostic(&self, interner: &StringInterner) -> Diagnostic {
        match self {
            SemanticProblem::UnknownIdentifier { span, name, .. } => { ... }
            SemanticProblem::NonExhaustiveMatch { span, missing_patterns } => { ... }
            // ...
        }
    }
}

impl LexProblem {
    pub fn into_diagnostic(&self) -> Diagnostic { ... }
}
```

Each problem type lives in `oric/src/problem/` and handles its own rendering. Type errors use `TypeErrorRenderer` in `oric/src/reporting/typeck/` for Pool-aware type name resolution.

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

Related context is communicated through **secondary labels** rather than a separate `RelatedInfo` type. Secondary labels attach messages to additional source spans:

```rust
// Example: "expected due to this annotation" as a secondary label
Diagnostic::error(ErrorCode::E2001)
    .with_message("type mismatch")
    .with_label(Label::primary(expr_span, "expected int, found str"))
    .with_label(Label::secondary(annotation_span, "expected due to this annotation"))
```

**Note:** There is no `RelatedInfo` struct in the implementation. All related context uses `Label::secondary()` on the `Diagnostic` type.

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
