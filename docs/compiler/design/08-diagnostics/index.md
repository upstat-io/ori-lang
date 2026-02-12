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
├── ori_diagnostic/               # Core diagnostic types (separate crate)
│   └── src/
│       ├── lib.rs                    # Module organization and re-exports
│       ├── error_code.rs             # ErrorCode enum, as_str(), Display
│       ├── diagnostic.rs             # Diagnostic, Label, Severity, Applicability, Suggestion
│       ├── guarantee.rs              # ErrorGuaranteed type-level proof
│       ├── queue.rs                  # DiagnosticQueue for deduplication/limits
│       ├── span_utils.rs             # Line/column computation from spans
│       ├── errors/                   # Embedded error documentation for --explain
│       │   ├── mod.rs                    # ErrorDocs registry
│       │   ├── E0001.md                  # Error documentation files
│       │   ├── E0002.md
│       │   └── ...                       # (35+ error codes documented)
│       ├── emitter/
│       │   ├── mod.rs                    # Emitter trait, trailing_comma() helper
│       │   ├── terminal.rs               # Terminal output
│       │   ├── json.rs                   # JSON output
│       │   └── sarif.rs                  # SARIF format (BTreeSet for rule dedup)
│       └── fixes/
│           ├── mod.rs                    # Code fix system
│           └── registry.rs               # Fix registry
└── oric/src/
    ├── problem/                  # Problem types (specific to compiler phases)
    │   ├── mod.rs                    # Problem enum (Lex, Parse, Semantic variants)
    │   ├── lex.rs                    # LexProblem enum
    │   └── semantic.rs               # SemanticProblem enum, DefinitionKind
    └── reporting/                # Diagnostic rendering (Problem → Diagnostic)
        ├── mod.rs                    # Render trait, render_all, Report type
        ├── lex.rs                    # LexProblem rendering
        ├── parse.rs                  # ParseProblem rendering
        └── semantic.rs               # SemanticProblem rendering

**Note:** The `problem/` and `reporting/` modules have an intentional 1:1 coupling.
Each problem variant in `problem/mod.rs` has a corresponding `Render` implementation
in `reporting/`. This separation keeps "what went wrong" (Problem) distinct from
"how to display it" (Diagnostic), while the 1:1 mapping ensures every problem
gets a tailored error message. Adding a new problem type requires adding its
renderer in the corresponding reporting module.
```

The `ori_diagnostic` crate is organized into focused submodules: `error_code.rs` (ErrorCode enum), `diagnostic.rs` (Diagnostic, Label, Severity, Applicability, Suggestion types), and `guarantee.rs` (ErrorGuaranteed). The `lib.rs` re-exports all public types. It depends only on `ori_ir` (for `Span`).

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
| E0xxx | Lexer | E0001: Invalid character, E0002: Unterminated string |
| E1xxx | Parser | E1001: Unexpected token, E1002: Expected expression |
| E2xxx | Type checker | E2001: Type mismatch, E2002: Undefined variable, E2003: Missing capability |
| E3xxx | Patterns | E3001: Unknown pattern, E3002: Missing required argument |
| E9xxx | Internal | E9001: Internal compiler error, E9002: Too many errors |

### Error Code Design

Error codes follow the `EXXXX` format where:
- First digit indicates the compiler phase (0=lexer, 1=parser, 2=type, 3=pattern, 9=internal)
- Remaining digits are sequential within that phase
- Codes are stable across versions for tooling compatibility
- The `E` prefix distinguishes errors from warnings (future: `W` prefix)

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

### Module Organization

Each problem category has its own rendering module:

| Module | Problem Type | Error Codes |
|--------|--------------|-------------|
| `lex.rs` | `LexProblem` | E0xxx (lexer errors) |
| `parse.rs` | `ParseProblem` | E1xxx (parser errors) |
| `semantic.rs` | `SemanticProblem` | E2xxx (name resolution, duplicates) |

**Note:** Type errors (E2xxx type mismatches, inference) are handled through `TypeCheckError` in `ori_typeck`, not through the Problem rendering system

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

    /// Simple text suggestions (human-readable, actionable)
    /// Use for "did you mean X?" style messages.
    pub suggestions: Vec<String>,

    /// Structured suggestions with spans and applicability (for `ori fix`)
    pub structured_suggestions: Vec<Suggestion>,
}

### Notes vs Suggestions

**Notes** (`.with_note()`) provide contextual information:
- "a type cannot contain itself"
- "available fields: x, y, z"
- "closures cannot recursively reference themselves"

**Suggestions** (`.with_suggestion()`) are actionable recommendations:
- "did you mean `length`?"
- "add explicit type annotation"
- "remove extra arguments"

pub enum Severity {
    Error,    // Compilation cannot continue
    Warning,  // Potential problem, compilation succeeds
    Note,     // Additional context information
    Help,     // Suggestion for improvement
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

The `span_utils` module provides line/column computation for error positioning.

#### Linear Scan Functions

For single or occasional lookups:

```rust
/// Compute 1-based line number from span and source.
pub fn line_number(source: &str, span: Span) -> u32;

/// Compute line number from byte offset.
pub fn line_from_offset(source: &str, offset: u32) -> u32;

/// Convert byte offset to (line, column) tuple.
pub fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32);
```

#### LineOffsetTable (Batch Lookups)

For repeated lookups on the same source (e.g., multiple diagnostics with multiple labels), `LineOffsetTable` pre-computes line offsets for O(log L) binary search instead of O(n) scanning:

```rust
pub struct LineOffsetTable {
    offsets: Vec<u32>,  // Byte offset of each line start
}

impl LineOffsetTable {
    /// Build from source text (O(n) once).
    pub fn build(source: &str) -> Self;

    /// Get 1-based line number from byte offset (O(log L)).
    pub fn line_from_offset(&self, offset: u32) -> u32;

    /// Get 1-based (line, column) from byte offset.
    pub fn offset_to_line_col(&self, source: &str, offset: u32) -> (u32, u32);

    /// Get byte offset of a line start (1-based line number).
    pub fn line_start_offset(&self, line: u32) -> Option<u32>;

    /// Number of lines in the source.
    pub fn line_count(&self) -> usize;
}
```

Usage:
```rust
let source = "line1\nline2\nline3";
let table = LineOffsetTable::build(source);

// O(log L) lookups instead of O(n)
assert_eq!(table.offset_to_line_col(source, 6), (2, 1));  // 'l' in line2
```

These utilities are used by `DiagnosticQueue` for position-based deduplication and sorting.

### Problem

The `Problem` enum uses a three-tier hierarchy (Lex, Parse, Semantic). Type checking errors use `TypeCheckError` directly from `ori_types`:

```rust
pub enum Problem {
    /// Lex-time problems (tokenization errors, confusables, cross-language habits).
    Lex(LexProblem),

    /// Parse-time problems (syntax errors).
    Parse(ParseProblem),

    /// Semantic analysis problems (name resolution, duplicates).
    Semantic(SemanticProblem),
}

impl Problem {
    pub fn span(&self) -> Span;
    pub fn is_lex(&self) -> bool;
    pub fn is_parse(&self) -> bool;
    pub fn is_semantic(&self) -> bool;
}
```

**Note:** Type checking errors use `TypeCheckError` from `ori_types` directly, rather than being wrapped in this enum. This allows the type checker to use structured error variants while other phases use this unified type.

Each category (LexProblem, ParseProblem, SemanticProblem) is a separate enum with category-specific variants. See [Problem Types](problem-types.md) for details.

## Diagnostic Derive Macros (Planned)

Derive macros (`#[derive(Diagnostic)]`, `#[derive(Subdiagnostic)]`) for declarative diagnostic definitions are planned for when the `Diagnostic` API stabilizes and enough repetitive boilerplate emerges (20+ diagnostic structs). Until then, diagnostics are constructed manually via the builder API.

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
