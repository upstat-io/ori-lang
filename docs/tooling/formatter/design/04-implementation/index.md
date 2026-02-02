---
title: "Implementation Overview"
description: "Ori Formatter Design — Implementation Approach"
order: 1
section: "Implementation"
---

# Implementation Overview

This document describes the implementation of the Ori formatter in `compiler/ori_fmt/`.

## Reference: Go's gofmt

The Ori formatter follows gofmt's philosophy: **zero-config, deterministic, implementation is the spec**.

Key techniques from gofmt that inform Ori's implementation:

| Technique | gofmt | Ori adaptation |
|-----------|-------|----------------|
| **Two-phase pipeline** | AST printer → tabwriter | Width calculator → formatter |
| **Idempotence** | Core guarantee | Core guarantee |
| **No configuration** | Deliberately denied | Zero-config |

## Architecture

The formatter operates on the parsed AST and produces formatted source text.

```
Source Text → Lexer → Parser → AST → Formatter → Formatted Text
```

### Crate Structure

```
compiler/ori_fmt/
├── src/
│   ├── lib.rs              # Public API, tabs_to_spaces()
│   ├── width/              # Width calculation module
│   │   ├── mod.rs          # WidthCalculator, ALWAYS_STACKED
│   │   ├── calls.rs        # Call/method call width
│   │   ├── collections.rs  # List/map/tuple/struct width
│   │   ├── compounds.rs    # Duration/size width
│   │   ├── control.rs      # Control flow width
│   │   ├── helpers.rs      # Shared utilities
│   │   ├── literals.rs     # Literal width
│   │   ├── operators.rs    # Binary/unary operator width
│   │   ├── patterns.rs     # Binding pattern width
│   │   └── wrappers.rs     # Ok/Err/Some/etc. width
│   ├── formatter/          # Core formatting engine
│   │   └── mod.rs          # Formatter struct, format/emit_inline/emit_broken/emit_stacked
│   ├── context.rs          # FormatContext, column/indent tracking
│   ├── emitter.rs          # Emitter trait, StringEmitter implementation
│   ├── declarations.rs     # ModuleFormatter for top-level items
│   ├── comments.rs         # CommentIndex, doc comment reordering
│   └── incremental.rs      # Incremental formatting for LSP
└── tests/
    ├── golden_tests.rs     # Golden file tests
    ├── idempotence_tests.rs # Idempotence verification
    ├── incremental_tests.rs # Incremental formatting tests
    ├── property_tests.rs   # Property-based tests
    └── width_tests.rs      # Width calculation tests
```

### Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `WidthCalculator` | `width/mod.rs` | Bottom-up width calculation with caching |
| `Formatter` | `formatter/mod.rs` | Top-down rendering with inline/broken/stacked modes |
| `FormatContext` | `context.rs` | Column tracking, indentation, line width checking |
| `ModuleFormatter` | `declarations.rs` | Module-level formatting (functions, types, etc.) |
| `CommentIndex` | `comments.rs` | Comment association and doc comment reordering |
| `Emitter` | `emitter.rs` | Output abstraction (StringEmitter implemented) |

## Width Calculation

### The `WidthCalculator` Struct

The `WidthCalculator` performs bottom-up traversal to compute inline width of each expression. Results are cached in an `FxHashMap` for efficiency.

```rust
pub struct WidthCalculator<'a, I: StringLookup> {
    arena: &'a ExprArena,
    interner: &'a I,
    cache: FxHashMap<ExprId, usize>,
}
```

### The ALWAYS_STACKED Sentinel

Some constructs bypass width-based decisions and always use stacked format:

```rust
pub const ALWAYS_STACKED: usize = usize::MAX;
```

When `width()` returns `ALWAYS_STACKED`, the formatter skips inline rendering and goes directly to stacked format.

**Always-stacked constructs:**
- `run`, `try` (sequential blocks)
- `match` (arms always stack)
- `recurse`, `parallel`, `spawn`, `catch`
- `nursery`

### Width Formulas

| Construct | Width Formula |
|-----------|---------------|
| Identifier | `name.len()` |
| Integer literal | digit count |
| Float literal | formatted string length |
| String literal | `content.len() + 2` (quotes) |
| Binary expr | `left + op_width + right` (3 for spaced ops like ` + `) |
| Function call | `func + 1 + args_width + 1` |
| Named argument | `name + 2 + value` (`: `) |
| Struct literal | `name + 3 + fields_width + 2` (` { ` + ` }`) |
| List | `2 + items_width` (`[` + `]`) |
| Map | `2 + entries_width` (`{` + `}`) |
| Tuple | `2 + items_width + trailing_comma` |

### Width Calculation Example

```rust
impl<'a, I: StringLookup> WidthCalculator<'a, I> {
    pub fn width(&mut self, expr_id: ExprId) -> usize {
        if let Some(&cached) = self.cache.get(&expr_id) {
            return cached;
        }

        let width = self.calculate_width(expr_id);
        self.cache.insert(expr_id, width);
        width
    }

    fn calculate_width(&mut self, expr_id: ExprId) -> usize {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::Int(n) => int_width(*n),
            ExprKind::Binary { op, left, right } => {
                let left_w = self.width(*left);
                let right_w = self.width(*right);
                if left_w == ALWAYS_STACKED || right_w == ALWAYS_STACKED {
                    return ALWAYS_STACKED;
                }
                left_w + binary_op_width(*op) + right_w
            }
            ExprKind::FunctionSeq(..) => ALWAYS_STACKED,
            // ... other cases
        }
    }
}
```

## Formatting Algorithm

### The `Formatter` Struct

The `Formatter` wraps a width calculator and format context to produce formatted output:

```rust
pub struct Formatter<'a, I: StringLookup> {
    arena: &'a ExprArena,
    interner: &'a I,
    width_calc: WidthCalculator<'a, I>,
    ctx: FormatContext<StringEmitter>,
}
```

### Three Rendering Modes

The formatter uses three rendering modes:

1. **Inline** (`emit_inline`): Single-line, all content on current line
2. **Broken** (`emit_broken`): Multi-line, content breaks according to construct rules
3. **Stacked** (`emit_stacked`): Always multi-line, for run/try/match/etc.

```rust
pub fn format(&mut self, expr_id: ExprId) {
    let width = self.width_calc.width(expr_id);

    if width == ALWAYS_STACKED {
        self.emit_stacked(expr_id);
    } else if self.ctx.fits(width) {
        self.emit_inline(expr_id);
    } else {
        self.emit_broken(expr_id);
    }
}
```

### Format Context

The `FormatContext` tracks state during formatting:

```rust
pub struct FormatContext<E: Emitter> {
    emitter: E,
    column: usize,        // Current column (0-indexed)
    indent_level: usize,  // Nesting depth
    config: FormatConfig, // Max width, etc.
}
```

Key methods:
- `fits(width)`: Returns `true` if `column + width <= max_width`
- `emit(text)`: Emit text and update column
- `emit_newline()`: Emit newline and reset column to 0
- `emit_indent()`: Emit indentation spaces
- `indent()` / `dedent()`: Adjust indentation level

### Breaking Behavior

**Binary expressions** break before the operator:
```rust
fn emit_broken(&mut self, expr_id: ExprId) {
    // ...
    ExprKind::Binary { op, left, right } => {
        self.format(*left);
        self.ctx.emit_newline_indent();
        self.ctx.emit(binary_op_str(*op));
        self.ctx.emit_space();
        self.format(*right);
    }
    // ...
}
```

**Collections** have two breaking modes:
- Simple items (literals, identifiers): wrap multiple per line
- Complex items (structs, calls, nested): one per line

```rust
fn emit_broken_list(&mut self, items: &[ExprId]) {
    let all_simple = items.iter().all(|id| self.is_simple_item(*id));

    if all_simple {
        self.emit_broken_list_wrap(items);     // Multiple per line
    } else {
        self.emit_broken_list_one_per_line(items);
    }
}
```

## Comment Handling

### Comment Association

Comments are associated with AST nodes by source position. A comment "belongs to" the node that immediately follows it.

```rust
pub struct CommentIndex {
    comments_by_position: BTreeMap<u32, Vec<CommentRef>>,
    consumed: Vec<bool>,
}
```

### Doc Comment Reordering

Doc comments are reordered to canonical order:
1. `// #Description` (may span multiple lines)
2. `// @param name` (in signature order)
3. `// @field name` (in struct order)
4. `// !Warning` or `// !Error`
5. `// >example -> result`

Regular comments (`//`) preserve their original order.

### Function Parameter Reordering

For functions, `@param` comments are reordered to match parameter order:

```rust
pub fn take_comments_before_function<I: StringLookup>(
    &mut self,
    pos: u32,
    param_names: &[&str],  // From function signature
    comments: &CommentList,
    interner: &I,
) -> Vec<usize>
```

## Declaration Formatting

### Module Structure

`ModuleFormatter` handles top-level declarations in order:
1. Imports (stdlib first, then relative)
2. Constants/configs
3. Type definitions
4. Traits
5. Impls
6. Functions
7. Tests

Blank lines separate different declaration kinds.

### Function Signatures

Function formatting considers trailing width (return type, capabilities, where clauses) when deciding whether to break parameters:

```rust
fn calculate_function_trailing_width(&mut self, func: &Function) -> usize {
    let mut width = 0;
    // Return type: " -> Type"
    if let Some(ref ret_ty) = func.return_ty {
        width += 4 + self.calculate_type_width(ret_ty);
    }
    // Capabilities, where clauses, " = "
    // ...
    width
}
```

### Function Bodies

Function bodies break differently based on construct type:
- Conditionals break to new line with indent
- Always-stacked constructs stay on same line, break internally
- Other constructs break internally as needed

## Incremental Formatting

### Purpose

Format only declarations that overlap with a changed region, rather than reformatting the entire file. Useful for LSP format-on-type.

### API

```rust
pub fn format_incremental<I: StringLookup>(
    module: &Module,
    comments: &CommentList,
    arena: &ExprArena,
    interner: &I,
    change_start: usize,
    change_end: usize,
) -> IncrementalResult
```

### Results

- `Regions(Vec<FormattedRegion>)`: Specific regions to replace
- `FullFormatNeeded`: Change affects imports/configs, need full format
- `NoChangeNeeded`: Change is between declarations

### Limitations

- Minimum unit is a complete top-level declaration
- Import and config changes require full format (block-formatted)
- Multi-declaration changes format all affected declarations

## Testing

### Test Types

| Test Type | Location | Purpose |
|-----------|----------|---------|
| Golden tests | `tests/golden_tests.rs` | Compare against expected output |
| Idempotence | `tests/idempotence_tests.rs` | Verify `format(format(x)) == format(x)` |
| Property tests | `tests/property_tests.rs` | Random input validation |
| Width tests | `tests/width_tests.rs` | Width calculation accuracy |
| Incremental | `tests/incremental_tests.rs` | Incremental formatting |

### Idempotence Testing

```rust
fn test_idempotence(input: &str) {
    let first = format(input);
    let second = format(&first);
    assert_eq!(first, second);
}
```

### Property-Based Testing

Verifies that formatting preserves semantics by comparing ASTs:

```rust
fn test_semantic_preservation(input: &str) {
    let original_ast = parse(input);
    let formatted = format(input);
    let formatted_ast = parse(&formatted);
    assert_eq!(strip_spans(&original_ast), strip_spans(&formatted_ast));
}
```

## Tooling Integration

### LSP Server

The LSP server (`ori_lsp`) provides formatting via `textDocument/formatting`:

```rust
// In ori_lsp
pub fn format_document(source: &str) -> String {
    let (module, comments, arena, interner) = parse(source);
    format_module_with_comments(&module, &comments, &arena, &interner)
}
```

### Playground

The Ori Playground uses format-on-run: code is automatically formatted when the user clicks Run. The LSP compiles to WASM and runs in-browser.

### CLI

```bash
ori fmt src/         # Format all .ori files in directory
ori fmt file.ori     # Format single file
ori check --fmt      # Check formatting without modifying
```

## Performance

### Caching

Width calculations are cached per expression ID:

```rust
cache: FxHashMap<ExprId, usize>
```

### Pre-allocation

The formatter pre-allocates based on estimated output size:

```rust
pub fn with_capacity(capacity: usize) -> Self {
    Self::with_emitter(StringEmitter::with_capacity(capacity))
}
```

### Output Abstraction

The `Emitter` trait enables different output targets:

```rust
pub trait Emitter {
    fn emit(&mut self, text: &str);
    fn emit_space(&mut self);
    fn emit_newline(&mut self);
    fn emit_indent(&mut self, level: usize);
}
```

Currently only `StringEmitter` is implemented. The trait exists to enable future file-streaming output without building full strings in memory.
