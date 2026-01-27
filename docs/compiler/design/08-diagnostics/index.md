---
title: "Diagnostics Overview"
description: "Ori Compiler Design — Diagnostics Overview"
order: 800
section: "Diagnostics"
---

# Diagnostics Overview

The diagnostics system provides error reporting, warnings, and code fix suggestions for the Ori compiler.

## Location

The diagnostics system spans multiple crates:

```
compiler/
├── ori_diagnostic/       # Core diagnostic types (separate crate)
│   └── src/
│       ├── lib.rs              # Diagnostic, ErrorCode, Applicability, Severity, ErrorGuaranteed
│       ├── queue.rs            # DiagnosticQueue for deduplication/limits
│       ├── span_utils.rs       # Line/column computation from spans
│       ├── errors/             # Embedded error documentation for --explain
│       │   ├── mod.rs          # ErrorDocs registry
│       │   ├── E0001.md        # Error documentation files
│       │   ├── E0002.md
│       │   └── ...             # (35+ error codes documented)
│       ├── emitter/
│       │   ├── mod.rs          # Emitter trait
│       │   ├── terminal.rs     # Terminal output
│       │   ├── json.rs         # JSON output
│       │   └── sarif.rs        # SARIF format
│       └── fixes/
│           ├── mod.rs          # Code fix system
│           └── registry.rs     # Fix registry
├── ori-macros/           # Proc-macro crate for diagnostic derives
│   └── src/
│       ├── lib.rs              # Derive macro exports
│       ├── diagnostic.rs       # #[derive(Diagnostic)] implementation
│       └── subdiagnostic.rs    # #[derive(Subdiagnostic)] implementation
└── oric/src/
    ├── problem/            # Problem types (specific to compiler phases)
    │   ├── mod.rs              # Problem enum (Parse, Type, Semantic variants)
    │   └── semantic.rs         # SemanticProblem enum, DefinitionKind
    └── reporting/          # Diagnostic rendering (Problem → Diagnostic)
        ├── mod.rs              # Render trait, render_all, Report type
        ├── parse.rs            # ParseProblem rendering
        ├── semantic.rs         # SemanticProblem rendering
        └── type_errors.rs      # TypeProblem rendering
```

The `ori_diagnostic` crate contains the core `Diagnostic` type, `ErrorCode` enum, `Applicability` levels, diagnostic queue, and output emitters. It depends only on `ori_ir` (for `Span`). The proc-macros in `ori-macros` generate implementations of the `IntoDiagnostic` trait.

## Design Goals

1. **Helpful messages** - Clear, actionable error descriptions
2. **Machine-readable** - JSON/SARIF for tooling integration
3. **Code fixes** - Automatic fix suggestions
4. **Error codes** - Stable identifiers for documentation
5. **Error guarantees** - Type-level proof that errors were reported

## ErrorGuaranteed

The `ErrorGuaranteed` type provides type-level proof that at least one error was emitted. This prevents "forgotten" error conditions where code fails silently without reporting an error.

```rust
/// Proof that at least one error was emitted.
/// Can only be created by emitting an error via DiagnosticQueue.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct ErrorGuaranteed(());

impl ErrorGuaranteed {
    pub(crate) fn new() -> Self;  // Only callable from queue.rs
    pub fn from_error_count(count: usize) -> Option<Self>;  // For downstream
    pub fn new_for_downstream() -> Self;  // When errors verified elsewhere
}
```

### Usage Pattern

```rust
// Functions return ErrorGuaranteed to prove they reported errors
fn type_check(&mut self) -> Result<TypedModule, ErrorGuaranteed> {
    if let Some(error) = self.check_for_errors() {
        // Can only get ErrorGuaranteed by actually emitting
        let guarantee = self.queue.emit_error(error.to_diagnostic(), line, col);
        return Err(guarantee);
    }
    Ok(self.build_typed_module())
}
```

### DiagnosticQueue Methods

```rust
impl DiagnosticQueue {
    /// Emit error and get proof it was emitted.
    pub fn emit_error(&mut self, diag: Diagnostic, line: u32, col: u32) -> ErrorGuaranteed;

    /// Emit error with position computed from source.
    pub fn emit_error_with_source(&mut self, diag: Diagnostic, source: &str) -> ErrorGuaranteed;

    /// Check if any errors were emitted.
    pub fn has_errors(&self) -> Option<ErrorGuaranteed>;
}
```

### Salsa Compatibility

`ErrorGuaranteed` implements `Copy`, `Clone`, `Eq`, `Hash` for use in Salsa query results:

```rust
#[salsa::tracked]
fn typed(db: &dyn Db, file: SourceFile) -> Result<TypedModule, ErrorGuaranteed>
```

## Error Code Ranges

| Range | Category | Examples |
|-------|----------|----------|
| E0xxx | Lexer | E0001: Invalid character |
| E1xxx | Parser | E1001: Unexpected token |
| E2xxx | Type checker | E2001: Type mismatch |
| E3xxx | Patterns | E3001: Unknown pattern |
| E9xxx | Internal | E9001: Compiler bug, E9002: Too many errors |

## DiagnosticQueue

The `DiagnosticQueue` provides Go-style error handling with deduplication, limits, and sorting:

```rust
pub struct DiagnosticQueue {
    diagnostics: Vec<QueuedDiagnostic>,
    error_count: usize,
    config: DiagnosticConfig,
    // Deduplication state
    last_syntax_line: Option<u32>,
    last_error: Option<(u32, String)>,
    has_hard_error: bool,
}

pub struct DiagnosticConfig {
    pub error_limit: usize,      // Default: 10 (0 = unlimited)
    pub filter_follow_on: bool,  // Default: true
    pub deduplicate: bool,       // Default: true
}
```

### Features

1. **Error Limits** - Stop after N errors (default 10) to avoid overwhelming output
2. **Deduplication** - Same-line syntax errors and same-message errors are collapsed
3. **Follow-on Filtering** - Errors caused by previous errors (e.g., "invalid operand") are suppressed
4. **Soft Error Suppression** - After a hard error, soft errors (inference failures) are hidden
5. **Position-based Sorting** - Errors are sorted by source location for consistent output

### Usage

```rust
let config = DiagnosticConfig::default();
let mut queue = DiagnosticQueue::with_config(config);

// Add diagnostics with source for line computation
queue.add_with_source(diagnostic, source, is_soft);

// Check if error limit reached
if queue.limit_reached() {
    // Stop processing
}

// Flush sorted diagnostics
let sorted = queue.flush();
```

### Integration with TypeChecker

The type checker optionally uses DiagnosticQueue for production builds:

```rust
// With queue (production)
let typed = type_check_with_source(&parse_result, interner, source.clone());

// With custom config
let config = DiagnosticConfig { error_limit: 5, ..Default::default() };
let typed = type_check_with_config(&parse_result, interner, source, config);
```

## Diagnostic Rendering

The rendering system converts structured `Problem` types into user-facing `Diagnostic` messages. This separates "what went wrong" (Problem) from "how to display it" (Diagnostic).

### Render Trait

The `Render` trait provides the conversion interface:

```rust
pub trait Render {
    fn render(&self) -> Diagnostic;
}

impl Render for Problem {
    fn render(&self) -> Diagnostic {
        match self {
            Problem::Parse(p) => p.render(),
            Problem::Type(p) => p.render(),
            Problem::Semantic(p) => p.render(),
        }
    }
}
```

### Module Organization

Each problem category has its own rendering module:

| Module | Problem Type | Error Codes |
|--------|--------------|-------------|
| `parse.rs` | `ParseProblem` | E1xxx (parser errors) |
| `semantic.rs` | `SemanticProblem` | E2xxx (name resolution, duplicates) |
| `type_errors.rs` | `TypeProblem` | E2xxx (type mismatches, inference) |

This separation follows the Single Responsibility Principle—each module focuses on rendering one category of problems with domain-specific context and suggestions.

### Helper Functions

```rust
/// Render all problems to diagnostics.
pub fn render_all(problems: &[Problem]) -> Vec<Diagnostic>;

/// Process type errors through the diagnostic queue.
pub fn process_type_errors(
    errors: Vec<TypeCheckError>,
    source: &str,
    config: Option<DiagnosticConfig>,
) -> Vec<Diagnostic>;
```

## Diagnostic Structure

```rust
pub struct Diagnostic {
    /// Error code (e.g., E2001)
    pub code: ErrorCode,

    /// Severity level
    pub severity: Severity,

    /// Main message
    pub message: String,

    /// Labeled spans showing where the error occurred
    pub labels: Vec<Label>,

    /// Additional notes providing context
    pub notes: Vec<String>,

    /// Simple text suggestions (human-readable)
    pub suggestions: Vec<String>,

    /// Structured suggestions with spans and applicability (for `ori fix`)
    pub structured_suggestions: Vec<Suggestion>,
}

pub enum Severity {
    Error,
    Warning,
    Note,
    Help,
}
```

## Structured Suggestions

Structured suggestions enable `ori fix` to auto-apply fixes:

```rust
/// Applicability level for code suggestions
pub enum Applicability {
    /// Safe to auto-apply (typos, missing delimiters)
    MachineApplicable,

    /// Might be wrong (type conversions, imports)
    MaybeIncorrect,

    /// Contains placeholders needing user input
    HasPlaceholders,

    /// Unknown confidence level
    Unspecified,
}

pub struct Suggestion {
    pub message: String,
    pub substitutions: Vec<Substitution>,
    pub applicability: Applicability,
}

pub struct Substitution {
    pub span: Span,
    pub snippet: String,
}
```

Usage:

```rust
// Machine-applicable fix (safe to auto-apply)
Diagnostic::error(ErrorCode::E1001)
    .with_message("missing semicolon")
    .with_fix("add semicolon", span, ";")

// Maybe-incorrect fix (needs human review)
Diagnostic::error(ErrorCode::E2001)
    .with_maybe_fix("convert to int", span, "int(x)")
```

## Example Output

Terminal output:
```
error[E2001]: type mismatch
 --> src/mainsi:10:15
   |
10 |     let x: int = "hello"
   |            ---   ^^^^^^^ expected int, found str
   |            |
   |            expected due to this annotation
   |
   = help: consider using int() to convert
```

## Key Components

### ErrorCode

```rust
pub enum ErrorCode {
    // Lexer
    E0001,  // Invalid character
    E0002,  // Unterminated string

    // Parser
    E1001,  // Unexpected token
    E1002,  // Expected expression
    E1003,  // Missing closing delimiter

    // Type checker
    E2001,  // Type mismatch
    E2002,  // Undefined variable
    E2003,  // Missing capability

    // Patterns
    E3001,  // Unknown pattern
    E3002,  // Missing required argument

    // Internal
    E9001,  // Internal compiler error
    E9002,  // Too many errors
}
```

### Span Utilities

The `span_utils` module provides line/column computation for error positioning:

```rust
/// Compute 1-based line number from span and source.
pub fn line_number(source: &str, span: Span) -> u32;

/// Compute line number from byte offset.
pub fn line_from_offset(source: &str, offset: u32) -> u32;

/// Convert byte offset to (line, column) tuple.
pub fn offset_to_line_col(source: &str, offset: u32) -> (usize, usize);
```

These are used by `DiagnosticQueue` for position-based deduplication and sorting.

### Problem

```rust
pub enum Problem {
    // Parser problems
    UnexpectedToken { expected: Vec<TokenKind>, found: TokenKind },
    UnterminatedString,

    // Type problems
    TypeMismatch { expected: Type, found: Type },
    UndefinedVariable { name: Name },
    MissingCapability { required: Capability },

    // Pattern problems
    UnknownPattern { name: Name },
    MissingArgument { pattern: Name, arg: &'static str },
}
```

## Diagnostic Derive Macros

The `ori-macros` crate provides derive macros for declarative diagnostic definitions:

```rust
#[derive(Diagnostic)]
#[diag(E2001, "type mismatch: expected `{expected}`, found `{found}`")]
pub struct TypeMismatch {
    #[primary_span]
    #[label("expected `{expected}`")]
    pub span: Span,
    pub expected: String,
    pub found: String,
    #[suggestion("convert with `int({name})`", code = "int({name})", applicability = "maybe-incorrect")]
    pub conversion_span: Option<Span>,
}

// Usage:
let err = TypeMismatch { span, expected: "int".into(), found: "str".into(), conversion_span: None };
let diagnostic = err.into_diagnostic();
```

Supported attributes:

| Attribute | Level | Description |
|-----------|-------|-------------|
| `#[diag(CODE, "msg")]` | Struct | Error code and message template |
| `#[primary_span]` | Field | Main error location |
| `#[label("msg")]` | Field | Label for a span |
| `#[note("msg")]` | Field | Additional note |
| `#[suggestion(...)]` | Field | Structured fix suggestion |

Subdiagnostics can be added via `#[derive(Subdiagnostic)]`:

```rust
#[derive(Subdiagnostic)]
#[label("this type was expected")]
pub struct ExpectedTypeLabel {
    #[primary_span]
    pub span: Span,
}
```

## Emitters

Output formats:

| Format | Use Case |
|--------|----------|
| Terminal | Human-readable, colored output |
| JSON | IDE integration, tooling |
| SARIF | Static analysis tools |

## Error Documentation System

The `errors/` directory contains embedded markdown documentation for each error code, accessible via `ori --explain <code>`.

### ErrorDocs Registry

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

### Documentation Format

Each error code has a markdown file (e.g., `E2001.md`) with:

```markdown
# E2001: Type Mismatch

An expression has a different type than expected in the given context.

## Example

```ori
let x: int = "hello"  // error: expected `int`, found `str`
```

## Common Causes

1. Assigning wrong type to annotated variable
2. Return type doesn't match function signature
3. ...

## Solutions

- Remove type annotation if inference should determine the type
- Convert the value explicitly: `int(value)`
- ...
```

### Adding New Documentation

1. Create a new file `EXXXX.md` in `compiler/ori_diagnostic/src/errors/`
2. Add an entry to the `DOCS` array in `errors/mod.rs`:
   ```rust
   (ErrorCode::EXXXX, include_str!("EXXXX.md")),
   ```
3. Run `cargo build` to embed the new documentation

### CLI Integration

```bash
$ ori --explain E2001
# E2001: Type Mismatch

An expression has a different type than expected...
```

## Related Documents

- [Problem Types](problem-types.md) - Error categorization
- [Code Fixes](code-fixes.md) - Automatic fix suggestions
- [Emitters](emitters.md) - Output format handlers
