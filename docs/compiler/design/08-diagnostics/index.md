# Diagnostics Overview

The diagnostics system provides error reporting, warnings, and code fix suggestions for the Sigil compiler.

## Location

```
compiler/sigilc/src/diagnostic/
├── mod.rs              # Core types, ErrorCode (~479 lines)
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

    /// Primary source location
    pub span: Span,

    /// Additional labels
    pub labels: Vec<Label>,

    /// Suggested fixes
    pub fixes: Vec<CodeFix>,

    /// Help text
    pub help: Option<String>,

    /// Related information
    pub related: Vec<RelatedInfo>,
}

pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}
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

### CodeFix

```rust
pub struct CodeFix {
    /// Description of the fix
    pub message: String,

    /// Edits to apply
    pub edits: Vec<TextEdit>,

    /// Applicability level
    pub applicability: Applicability,
}

pub enum Applicability {
    /// Safe to apply automatically
    MachineApplicable,

    /// May have side effects
    MaybeIncorrect,

    /// Needs human review
    HasPlaceholders,
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
