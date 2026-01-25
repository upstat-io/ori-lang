# Diagnostics Overview

The diagnostics system provides error reporting, warnings, and code fix suggestions for the Sigil compiler.

## Location

The diagnostics system spans multiple crates:

```
compiler/
├── sigil_diagnostic/       # Core diagnostic types (separate crate)
│   └── src/
│       ├── lib.rs              # Diagnostic, ErrorCode, Applicability, Severity (~660 lines)
│       ├── queue.rs            # DiagnosticQueue for deduplication/limits (~494 lines)
│       ├── span_utils.rs       # Line/column computation from spans (~120 lines)
│       ├── emitter/
│       │   ├── mod.rs          # Emitter trait (~90 lines)
│       │   ├── terminal.rs     # Terminal output (~285 lines)
│       │   ├── json.rs         # JSON output (~176 lines)
│       │   └── sarif.rs        # SARIF format (~453 lines)
│       └── fixes/
│           ├── mod.rs          # Code fix system (~258 lines)
│           └── registry.rs     # Fix registry (~245 lines)
├── sigil-macros/           # Proc-macro crate for diagnostic derives
│   └── src/
│       ├── lib.rs              # Derive macro exports
│       ├── diagnostic.rs       # #[derive(Diagnostic)] implementation
│       └── subdiagnostic.rs    # #[derive(Subdiagnostic)] implementation
└── sigilc/src/
    └── problem/            # Problem types (specific to compiler phases)
        ├── mod.rs              # Problem enum
        └── report.rs           # Report formatting
```

The `sigil_diagnostic` crate contains the core `Diagnostic` type, `ErrorCode` enum, `Applicability` levels, diagnostic queue, and output emitters. It depends only on `sigil_ir` (for `Span`). The proc-macros in `sigil-macros` generate implementations of the `IntoDiagnostic` trait.

## Design Goals

1. **Helpful messages** - Clear, actionable error descriptions
2. **Machine-readable** - JSON/SARIF for tooling integration
3. **Code fixes** - Automatic fix suggestions
4. **Error codes** - Stable identifiers for documentation

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

    /// Structured suggestions with spans and applicability (for `sigil fix`)
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

Structured suggestions enable `sigil fix` to auto-apply fixes:

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

The `sigil-macros` crate provides derive macros for declarative diagnostic definitions:

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

## Related Documents

- [Problem Types](problem-types.md) - Error categorization
- [Code Fixes](code-fixes.md) - Automatic fix suggestions
- [Emitters](emitters.md) - Output format handlers
