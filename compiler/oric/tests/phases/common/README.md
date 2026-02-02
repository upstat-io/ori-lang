# Common Test Utilities

Shared helpers for phase tests. These utilities reduce boilerplate and ensure consistent test setup.

## Parse Utilities (`parse.rs`)

```rust
use crate::common::{parse_source, parse_ok, parse_err, test_interner};

// Parse and check result
let output = parse_source("@add(a: int, b: int) -> int = a + b");
assert!(!output.has_errors());

// Assert successful parse
let output = parse_ok("let x = 42");

// Assert parse failure
parse_err("@foo(", "expected");

// Create isolated interner
let interner = test_interner();
```

## Type Check Utilities (`typecheck.rs`)

```rust
use crate::common::{typecheck_source, typecheck_ok, typecheck_err};

// Type check and inspect result
let typed = typecheck_source("@main () -> int = 42");
assert!(typed.errors.is_empty());

// Assert successful type check
let typed = typecheck_ok("@add(a: int, b: int) -> int = a + b");

// Assert type check failure
typecheck_err("let x: int = \"hello\"", "mismatch");
```

## Diagnostic Utilities (`diagnostics.rs`)

Tests for `DiagnosticQueue` from `ori_diagnostic`.

## Visitor Utilities (`visitor.rs`)

Tests for AST visitor infrastructure from `ori_ir`.

## Error Matching (`error_matching.rs`)

Tests for the `#compile_fail` test infrastructure.

## Adding New Utilities

1. Create a new file: `phases/common/{category}.rs`
2. Add the module to `mod.rs`
3. Re-export public helpers: `pub use {category}::*;`
4. Add documentation with usage examples

### Guidelines

- Each helper should do one thing well
- Provide clear error messages on assertion failure
- Include doc comments with examples
- Add tests for the helpers themselves
