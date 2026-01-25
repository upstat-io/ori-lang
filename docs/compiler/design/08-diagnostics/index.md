# Diagnostics Overview

The diagnostics system provides error reporting, warnings, and code fix suggestions for the Sigil compiler.

## Location

```
compiler/
├── sigil-macros/           # Proc-macro crate for diagnostic derives
│   └── src/
│       ├── lib.rs              # Derive macro exports
│       ├── diagnostic.rs       # #[derive(Diagnostic)] implementation
│       └── subdiagnostic.rs    # #[derive(Subdiagnostic)] implementation
└── sigilc/src/diagnostic/
    ├── mod.rs              # Core types, ErrorCode, Applicability (~600+ lines)
    ├── problem.rs          # Problem enum
    ├── report.rs           # Report formatting
    ├── fixes/
    │   └── mod.rs          # Code fix system (~258 lines)
    └── emitter/
        ├── mod.rs          # Emitter trait
        ├── terminal.rs     # Terminal output
        ├── json.rs         # JSON output
        └── sarif.rs        # SARIF format (~453 lines)
```

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
| E9xxx | Internal | E9001: Compiler bug |

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
}
```

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
