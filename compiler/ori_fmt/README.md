# Ori Formatter (`ori_fmt`)

Code formatter for the Ori programming language.

## 5-Layer Architecture

The formatter uses a layered architecture inspired by modern formatters (rustfmt, Gleam, TypeScript):

### Layer 1: Token Spacing (`spacing/`)

O(1) declarative token spacing rules using hash-based lookup.

```rust
// Example: space around binary operators
use ori_fmt::spacing::{lookup_spacing, SpaceAction};
let action = lookup_spacing(Plus, Ident);
assert_eq!(action, SpaceAction::Space);
```

**Key types:**
- `SpaceAction` - None, Space, Newline, Preserve
- `TokenCategory` - Groups of similar tokens
- `TokenMatcher` - Flexible matching (Any, Exact, OneOf, Category)
- `RulesMap` - O(1) rule lookup

### Layer 2: Container Packing (`packing/`)

Container formatting decisions: when to inline vs break.

```rust
use ori_fmt::packing::{Packing, ConstructKind, determine_packing};

// Simple list can pack multiple items per line
let packing = determine_packing(ConstructKind::ListSimple, false, false, false, 10);
assert_eq!(packing, Packing::FitOrPackMultiple);

// run() at top level always stacks
let packing = determine_packing(ConstructKind::RunTopLevel, false, false, false, 3);
assert_eq!(packing, Packing::AlwaysStacked);
```

**Key types:**
- `Packing` - FitOrOnePerLine, FitOrPackMultiple, AlwaysOnePerLine, AlwaysStacked
- `ConstructKind` - 22 container types
- `Separator` - Comma, Space, Pipe

### Layer 3: Shape Tracking (`shape/`)

Width tracking through recursion for independent breaking decisions.

```rust
use ori_fmt::shape::Shape;

let shape = Shape::new(100);
let after = shape.consume(10).indent(4);
assert!(after.fits(80));
```

**Key types:**
- `Shape` - Tracks width, indent, offset
- `FormatConfig` - Max width, indent size, trailing commas
- `TrailingCommas` - Always, Never, Preserve

### Layer 4: Breaking Rules (`rules/`)

Eight Ori-specific breaking rules for special constructs:

| Rule | Description |
|------|-------------|
| `MethodChainRule` | All chain elements break together |
| `ShortBodyRule` | ~20 char threshold for yield/do bodies |
| `BooleanBreakRule` | 3+ `\|\|` clauses break with leading `\|\|` |
| `ChainedElseIfRule` | Kotlin style (first `if` with assignment) |
| `NestedForRule` | Rust-style indentation for nested `for` |
| `ParenthesesRule` | Preserve user parens, add when needed |
| `RunRule` | Top-level stacked, nested width-based |
| `LoopRule` | Complex body (run/try/match/for) breaks |

### Layer 5: Orchestration (`formatter/`)

Main formatter that coordinates all layers.

```rust
use ori_fmt::{format_module, format_expr};

// Format a complete module
let formatted = format_module(&module, &arena, &interner);

// Format a single expression
let formatted = format_expr(&arena, &interner, expr_id);
```

## Adding New Rules

1. Create `rules/<name>.rs` with rule struct
2. Add required methods and constants
3. Export from `rules/mod.rs`
4. Add tests in `rules/tests.rs`
5. Integrate with `formatter/` if needed

## Modifying Existing Rules

1. Find the rule in `rules/`
2. Update the logic
3. Update tests
4. Run `cargo test -p ori_fmt` to verify

## Module Organization

```
ori_fmt/
├── spacing/           # Layer 1: Token spacing
│   ├── action.rs      # SpaceAction enum
│   ├── category.rs    # TokenCategory grouping
│   ├── matcher.rs     # TokenMatcher patterns
│   ├── rules.rs       # Declarative rules
│   ├── lookup.rs      # O(1) RulesMap
│   └── tests.rs
├── packing/           # Layer 2: Container packing
│   ├── packing.rs     # Packing enum
│   ├── construct.rs   # ConstructKind
│   ├── separator.rs   # Separator enum
│   ├── simple.rs      # Simple item detection
│   └── tests.rs
├── shape/             # Layer 3: Shape tracking
│   ├── core.rs        # Shape struct
│   └── tests.rs
├── rules/             # Layer 4: Breaking rules
│   ├── method_chain.rs
│   ├── short_body.rs
│   ├── boolean_break.rs
│   ├── chained_else_if.rs
│   ├── nested_for.rs
│   ├── parentheses.rs
│   ├── run_rule.rs
│   ├── loop_rule.rs
│   └── tests.rs
├── formatter/         # Layer 5: Orchestration
│   ├── mod.rs         # Main Formatter struct
│   ├── inline.rs      # Single-line rendering
│   ├── broken.rs      # Multi-line rendering
│   ├── stacked.rs     # Always-stacked constructs
│   ├── helpers.rs     # Collection helpers
│   ├── patterns.rs    # Pattern rendering
│   ├── literals.rs    # Literal rendering
│   └── tests.rs
├── declarations/      # Module-level formatting
├── comments/          # Comment preservation
├── context/           # Formatting context
├── emitter/           # Output abstraction
├── width/             # Width calculation
└── incremental/       # Incremental formatting
```

## Testing

```bash
# Run all formatter tests
cargo test -p ori_fmt

# Run specific layer tests
cargo test -p ori_fmt spacing
cargo test -p ori_fmt packing
cargo test -p ori_fmt shape
cargo test -p ori_fmt rules

# Run property tests
cargo test -p ori_fmt --test property_tests
```

## Performance

The formatter is optimized for speed:
- O(1) spacing lookup via hash map
- Width calculated once per expression
- Incremental formatting for changed regions (~30% speedup)
- Parallel processing for multi-file formatting
