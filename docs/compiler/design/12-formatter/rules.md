---
title: "Formatting Rules"
description: "Ori Compiler Design — Line Breaking and Formatting Rules"
order: 1203
section: "Formatter"
---

# Formatting Rules (Layers 3–5)

This document is a deep dive into how the Ori formatter decides when and how to break lines. It covers three of the five layers in the formatter architecture: Shape tracking (Layer 3), breaking rules (Layer 4), and the orchestration engine that coordinates all layers into formatted output (Layer 5). Together, these three layers implement the two-pass measure-then-render algorithm that produces deterministic, backtrack-free formatting in O(n) time.

Readers of this document are expected to be familiar with the [Formatter Overview](./index.md) and [Packing](./packing.md) documents, which cover the architecture and the container packing decisions of Layers 1 and 2.

---

## The Two-Pass Algorithm

The central problem of a code formatter is: given an abstract syntax tree and a maximum line width, produce text that fits within that width without making the rendering algorithm look arbitrarily far ahead. Three broad families of solutions have been developed:

**Single-pass with backtracking.** The simplest approach is to render speculatively inline, detect overflow, then backtrack and re-render in broken mode. When a line exceeds the width limit, the renderer rewinds to the last breakpoint and retries with a newline. Prettier's internals use a variant of this idea. The algorithm is conceptually simple, but the worst-case behavior is exponential: a deeply nested expression with k potential breakpoints can require O(2^k) retries. In practice, Prettier's algorithm is linear because it limits backtracking, but the theoretical complexity is concerning for a production compiler that must format arbitrary user code.

**Document algebra (Wadler 1997, Lindig 2000).** The most theoretically elegant approach builds an intermediate representation — a "document IR" — before rendering. The document IR contains combinators like `group` (try to render on one line, fall back to broken), `indent` (increase indentation), and `line` (a breakable whitespace). A width-aware renderer walks the document IR and makes O(1) breaking decisions at each node. This is the algorithm described in [Wadler's "A Prettier Printer"](https://homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf) and refined by [Lindig's strict evaluation version](https://lindig.github.io/papers/strictly-pretty-2000.pdf). The approach is elegant but requires a separate phase to construct the document IR from the AST — two engineering efforts where simpler approaches need one.

**Measure-then-render (Ori's approach).** A third strategy separates the measurement from the rendering without introducing a document IR. A **bottom-up measure pass** computes the inline width of every AST node and caches the results. A subsequent **top-down render pass** uses those cached widths to make O(1) breaking decisions at each node: if the cached width fits in the remaining line width, render inline; otherwise, render broken. There is no intermediate document IR, no backtracking, and no speculative rendering. The total work is O(n) — each node is visited once in the measure pass and once in the render pass.

Ori's formatter uses the measure-then-render approach. The rest of this section walks through both passes in detail.

### The Measure Pass

The measure pass is implemented by `WidthCalculator` in `compiler/ori_fmt/src/width/mod.rs`. It performs a bottom-up traversal of the expression arena, computing the inline width of every `ExprId`. Results are stored in an `FxHashMap<ExprId, usize>` so that each node is measured at most once even when the same sub-expression appears multiple times in the render pass.

The key invariant of the measure pass is **ALWAYS_STACKED propagation**. Some constructs should never render inline regardless of their content — `match`, `try`, `recurse`, `parallel`, `spawn`, and `nursery`. These constructs return the sentinel value `ALWAYS_STACKED = usize::MAX` from their width calculation. This sentinel propagates upward: if any sub-expression of a compound construct returns `ALWAYS_STACKED`, the parent also returns `ALWAYS_STACKED`. This means the render pass never needs to check sub-expressions for stacked-ness — the sentinel in the cache is sufficient.

```rust
// From width/mod.rs
pub const ALWAYS_STACKED: usize = usize::MAX;

fn calculate_width(&mut self, expr_id: ExprId) -> usize {
    let expr = self.arena.get_expr(expr_id);
    match &expr.kind {
        ExprKind::Match { .. } => ALWAYS_STACKED,
        ExprKind::Binary { op, left, right } => {
            let left_w = self.width(*left);
            let right_w = self.width(*right);
            if left_w == ALWAYS_STACKED || right_w == ALWAYS_STACKED {
                return ALWAYS_STACKED;  // Propagate upward
            }
            left_w + binary_op_width(*op) + right_w
        }
        // ...
    }
}
```

The `accumulate_widths` helper in `width/helpers/mod.rs` handles the sentinel propagation for lists: if any element returns `ALWAYS_STACKED`, the accumulator immediately returns `ALWAYS_STACKED` rather than continuing.

### The Render Pass

The render pass is implemented by `Formatter::format()` in `compiler/ori_fmt/src/formatter/mod.rs`. It is a top-down traversal driven by the central dispatch function:

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

This three-way dispatch is the entire decision-making core. The measure pass has done all the work; the render pass simply reads the cached width and routes to the appropriate rendering function. `ctx.fits(width)` checks whether the current column position plus `width` is within the 100-character limit — an O(1) comparison. The three rendering functions `emit_inline`, `emit_broken`, and `emit_stacked` are exhaustive matches over every `ExprKind` variant, with compile-time enforcement that no variant is missing.

---

## Width Calculation

Width formulas are organized into submodules by expression category. Each module exposes free functions that accept a `WidthCalculator` and the relevant node data, returning a `usize` width or `ALWAYS_STACKED`:

| Module | Covers |
|--------|--------|
| `literals.rs` | `int_width`, `float_width`, `bool_width`, `string_width`, `char_width` |
| `compounds.rs` | `duration_width`, `size_width` |
| `operators.rs` | `binary_op_width`, `unary_op_width` |
| `calls.rs` | `call_width`, `call_named_width`, `method_call_width`, `method_call_named_width` |
| `collections.rs` | `list_width`, `map_width`, `struct_width`, `tuple_width`, `range_width` |
| `control.rs` | `if_width`, `for_width`, `block_width`, `assign_width`, `with_capability_width` |
| `wrappers.rs` | `ok_width`, `err_width`, `some_width`, `loop_width`, `try_width`, `cast_width`, `await_width` |
| `patterns.rs` | `binding_pattern_width`, `for_binding_pattern_width` |
| `helpers.rs` | `accumulate_widths()`, `COMMA_SEPARATOR_WIDTH` (= 2), `char_display_width()`, `decimal_digit_count()` |

Representative width formulas reveal the accounting conventions:

| Construct | Formula | Notes |
|-----------|---------|-------|
| Identifier | `name.len()` | Byte length, accurate for ASCII |
| Integer literal | `text.len()` | Includes underscores and `0x`/`0b` prefixes |
| String literal | `text.len() + 2` | Two quote characters |
| Binary expression | `left + binary_op_width(op) + right` | Op width includes surrounding spaces; e.g. `+` = 3 (` + `) |
| Function call | `name + 1 + args_width + separators + 1` | The `1`s are the two parentheses |
| Named argument | `name + 2 + value` | The `2` is `: ` |
| Struct literal | `name + 3 + fields_width + separators + 2` | `3` is ` { `, `2` is ` }` |
| List | `2 + items_width + separators` | `2` is `[` and `]` |
| Map | `2 + entries_width + separators` | `2` is `{` and `}` |
| Lambda (single param) | `param + 4 + body` | `4` is ` -> ` |

The `COMMA_SEPARATOR_WIDTH` constant is `2`, representing the `", "` separator between items. When items break to separate lines, the separator becomes `","` plus a newline — the trailing space is dropped and the newline is handled by the render pass, not the width calculation.

The `char_display_width` function in `helpers.rs` handles Unicode: CJK ideographs and most emoji return width 2, combining marks return width 0, and all other characters return width 1. This gives accurate terminal display widths for code that mixes ASCII and non-ASCII characters in string literals or comments.

---

## Shape Tracking (Layer 3)

The `Shape` struct, defined in `compiler/ori_fmt/src/shape/core.rs`, is a value type that tracks the formatter's position within the current line:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub width: usize,   // Characters remaining on current line
    pub indent: usize,  // Current indentation level in spaces
    pub offset: usize,  // Position from start of line
}
```

`Shape` is a **value type**: every method returns a new `Shape` rather than mutating in place. This functional style makes it impossible to forget to restore shape state after formatting a nested construct — the old shape is simply not referenced anymore. The Rust ownership system enforces that modified shapes are used correctly: a method that computes a nested shape without using it produces a compiler warning.

The key operations:

| Method | Effect |
|--------|--------|
| `consume(n)` | Reduce `width` by n, increase `offset` by n |
| `indent(spaces)` | Increase `indent`, reduce `width` |
| `dedent(spaces)` | Decrease `indent`, increase `width` |
| `next_line(max)` | Reset `offset` and `width` to reflect start of new line at current indent |
| `fits(w)` | True if `w <= width` |
| `should_break(w)` | True if `w > width` |
| `for_nested(config)` | Fresh `width` computed from `max_width - indent`, independent of `offset` |
| `for_block(config)` | `indent(indent_size).next_line(max_width)` — for block bodies |

### Independent Breaking via `for_nested`

The most consequential method is `for_nested`. When the formatter descends into a nested construct — a function call argument, a struct field value, a list element — it computes a fresh `Shape` that measures available width from the current indentation level, not from the current column position.

Consider why this matters. Suppose the formatter is at column 60 of a 100-character line, rendering the arguments of a broken function call. Each argument goes on its own indented line at indent level 1 (column 4). When the formatter encounters an argument that is itself a function call, it must decide whether that inner call fits inline. If it uses the current shape (with `width = 40`, reflecting 60 already-consumed characters), the inner call might not "fit" even though, on its own dedicated line with 4 characters of indentation, it has 96 characters of room.

`for_nested` solves this by computing width from the indent level:

```rust
pub fn for_nested(&self, config: &FormatConfig) -> Shape {
    Shape {
        width: config.max_width.saturating_sub(self.indent),
        indent: self.indent,
        offset: self.indent,
    }
}
```

The result: nested constructs break independently. A function call stays inline when it fits on its own line, even if the surrounding expression needed to break. This is the same strategy used by `rustfmt` and is essential for preventing unnecessary cascading breaks in deeply nested expressions:

```ori
let result = run(
    process(items.map(x -> x * 2)),  // fits on its own line; stays inline
    validate(result),
)
```

Without `for_nested`, the inner `process(...)` call would see only the remaining width of the broken outer call, and might itself break even though it easily fits on its dedicated argument line.

### FormatContext

The `FormatContext` struct in `compiler/ori_fmt/src/context/mod.rs` wraps a `Shape` together with the output emitter and formatting configuration. It is the object that methods call to emit text — every `emit()`, `indent()`, `dedent()`, and `emit_newline_indent()` call goes through `FormatContext`. Internally, `FormatContext` keeps `column` and the `Shape` synchronized: every `emit(text)` call both writes text and calls `shape.consume(text.len())`. This synchronization is the key invariant — the `Shape` always reflects the current column position.

The `fits(width)` method on `FormatContext` delegates to `shape.fits(width)`, so breaking decisions made through the context always reflect the actual available width at the current line position.

---

## Breaking Rules (Layer 4)

Layer 4 contains eight named rule types in `compiler/ori_fmt/src/rules/`. These rules encode Ori-specific formatting conventions that do not follow from simple width-based decisions. Each rule is a named struct with associated constants, decision methods, and data types for collecting the construct's sub-structure. The rules are consulted from within `emit_broken` — they apply only when the formatter has already determined that an expression does not fit inline.

### MethodChainRule

**File**: `rules/method_chain.rs`

Method chains — sequences of `.method()` calls on a single receiver — follow an all-or-nothing policy: they either fit entirely on one line, or every `.method()` call breaks to its own indented line. No selective breaking, where some calls stay on the same line and others break, is permitted. This policy is stricter than human intuition but produces more consistent output: the reader always knows that a method chain is either fully inline or fully broken.

The minimum chain length that triggers this logic is `MIN_CHAIN_LENGTH = 2`. A single method call on a receiver is not a "chain" and does not trigger chain-breaking logic — it breaks or stays inline based only on width.

The `collect_method_chain` function implements the collection phase by walking backward through the nested `MethodCall`/`MethodCallNamed` AST nodes:

```rust
pub fn collect_method_chain(arena: &ExprArena, expr_id: ExprId) -> Option<MethodChain> {
    let mut calls = Vec::new();
    let mut current = expr_id;
    loop {
        match &arena.get_expr(current).kind {
            ExprKind::MethodCall { receiver, method, .. } => {
                calls.push(ChainedCall { name: *method, has_named_args: false, expr_id: current });
                current = *receiver;
            }
            ExprKind::MethodCallNamed { receiver, method, .. } => {
                calls.push(ChainedCall { name: *method, has_named_args: true, expr_id: current });
                current = *receiver;
            }
            _ => break,
        }
    }
    calls.reverse();
    Some(MethodChain { receiver: current, calls })
}
```

The receiver — the non-method-call expression at the start of the chain — is collected as `current` when the loop exits. The calls vector is reversed from the walk-backward order to give receiver-first order.

In broken mode, the receiver stays on the current line, and each method call goes on its own line indented by one level:

```ori
// Inline (fits):
items.map(x -> x * 2).filter(x -> x > 0)

// Broken (all break together):
items
    .map(x -> x * 2)
    .filter(x -> x > 0)
    .take(n: 10)
```

The all-or-nothing property matters because mixed chains — where one method stays inline and the next breaks — are more confusing than either extreme. The reader sees a single `.` and does not know whether the next `.` will be on the same line or a new line. The all-or-nothing policy eliminates that ambiguity.

### ShortBodyRule

**File**: `rules/short_body.rs`

For `yield` and `do` bodies, there is a class of expressions that should never appear alone on their own line even when the overall `for` expression breaks. A lone identifier on its own line after `yield` reads as orphaned:

```ori
// Bad — identifier appears orphaned:
for user in users yield
    user

// Good — short body stays with yield:
for user in users yield user
```

`ShortBodyRule` defines "short body" as expressions that are guaranteed to be under `THRESHOLD = 20` characters: identifiers, integer/float/string/char/bool literals, `Duration` and `Size` literals, `None`, `Unit`, `self`, constants, and `break`/`continue` without a value. These are the irreducible leaf-level expressions that carry no internal structure to break.

The `suggest_break_point` function returns a `BreakPoint` enum that guides the broken renderer:

- `NoBreak` — the entire expression fits inline (checked at orchestration level).
- `AfterYield` — the body is complex (a call, binary expression, etc.) and should break to an indented new line after `yield`.
- `BeforeFor` — the body is short and should stay with `yield`, but the overall line is too long. The break happens before the entire `for` expression, not after `yield`.

This distinction — break before `for` rather than after `yield` — preserves the semantic grouping of the `yield`/`do` keyword with its body. The body and its keyword are a unit; breaking between them violates that unit.

### BooleanBreakRule

**File**: `rules/boolean_break.rs`

Boolean `||` expressions with three or more top-level clauses break with each clause on its own line, with the `||` operator leading the line rather than trailing it:

```ori
// Two clauses: no forced break
if user.active && user.verified || user.is_admin then ...

// Three or more clauses: each on its own line with leading ||
if user.active && user.verified
    || user.is_admin
    || user.bypass_check then ...
```

The threshold `OR_THRESHOLD = 3` is the minimum number of `||`-separated clauses to trigger automatic breaking. Two-clause expressions break only if they exceed the width limit. This mirrors `rustfmt`'s behavior, where long boolean conditions are more readable when each clause is visually distinct.

The `collect_or_clauses` function walks the left-recursive `Binary { op: Or }` tree and returns the clauses in left-to-right order. The walk accumulates clauses in reverse (because each `Or` node has the right clause as a leaf and recurses left), then reverses the result. For `a || b || c`, the tree is `Or(Or(a, b), c)`, and the walk collects `[c, b, a]` then reverses to `[a, b, c]`.

The rule applies only to top-level `||` — `&&` within a clause is not broken the same way. A clause like `user.active && user.verified` is a single unit from the `||`-breaking perspective.

### ChainedElseIfRule

**File**: `rules/chained_else_if.rs`

Chained `if-else-if` expressions use Kotlin-style formatting: the first `if` stays on the same line as any assignment or binding, and each subsequent `else` appears on its own line:

```ori
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

This differs from an older style where the entire chain would break to a new line after the `=`. The Kotlin style is more compact and makes the relationship between the binding and its value visually clear — the `=` and the first condition are on the same line.

The `collect_if_chain` function builds an `IfChain` struct by walking the nested `ExprKind::If` nodes:

```rust
pub struct IfChain {
    pub condition: ExprId,
    pub then_branch: ExprId,
    pub else_ifs: Vec<ElseIfBranch>,
    pub final_else: Option<ExprId>,
}
```

Each `ElseIfBranch` holds a condition and a then-branch. The final `else` branch (if present) is stored separately.

In the broken renderer, each `else if cond then branch` segment is checked independently for fit. If the segment `if cond then branch` fits on the remaining line width after `else `, it renders inline on the else line. If the segment does not fit, the `then branch` breaks to an indented new line under the `if cond`. This per-segment fitting means very long conditions can still break internally while short segments stay compact.

### NestedForRule

**File**: `rules/nested_for.rs`

Nested `for` loops — where the body of a `for` is itself a `for` — format with incremental indentation. Each nesting level gets its own line, and the innermost body breaks to an additionally-indented line:

```ori
for user in users yield
    for permission in user.permissions yield
        for action in permission.actions yield
            action.name
```

The `collect_for_chain` function walks the nested `ExprKind::For` nodes and builds a `ForChain`:

```rust
pub struct ForChain {
    pub levels: Vec<ForLevel>,  // outermost to innermost
    pub body: ExprId,           // final non-for body
}

pub struct ForLevel {
    pub pattern: BindingPatternId,
    pub iter: ExprId,
    pub guard: ExprId,  // ExprId::INVALID if absent
    pub is_yield: bool,
}
```

The broken renderer uses this chain structure to emit each level at increasing indentation, placing the `for pattern in iter yield` line at the appropriate depth, then breaking the next level or the final body to the next indentation level.

### ParenthesesRule

**File**: `rules/parentheses.rs`

The `ParenthesesRule` has two responsibilities: determining when parentheses are semantically required (and must be added by the formatter), and preserving user-written parentheses (a currently-known limitation).

The `ParenPosition` enum classifies three positions where complex expressions require parentheses:

- **`Receiver`** — the expression before a `.method()` call. If the receiver is a binary expression, unary expression, `if`, lambda, `let`, range, `for`, `loop`, `FunctionSeq`, or `FunctionExp`, parentheses are required. Example: `(a + b).method()` — without parens, `a + b.method()` would parse as `a + (b.method())`.
- **`CallTarget`** — the expression being called with `(args)`. If a lambda appears as a call target, parentheses are required: `(x -> x * 2)(5)`. Without parens, `x -> x * 2(5)` parses the `2(5)` as a call.
- **`IteratorSource`** — the expression after `in` in a `for` loop. If the source is itself a `for`, `if`, lambda, or `let`, parentheses are required to prevent ambiguous parsing.

The `is_simple_expr` function identifies expressions that never need parentheses in any of these positions: identifiers, literals of all kinds, unit, none, self, constants, calls, method calls, field access, and indexing.

**Known limitation**: The AST does not currently track whether the user explicitly wrote parentheses around an expression. `ParenthesesRule::has_user_parens()` always returns `false`. As a result, user-written parentheses that are semantically optional — added for clarity, not correctness — may be dropped by the formatter. Future work will add an `has_explicit_parens: bool` field to `ori_ir::Expr` during parsing to preserve user intent.

### LoopRule

**File**: `rules/loop_rule.rs`

`loop` bodies that contain complex control flow always break to a separate line, even when the body might technically fit within the width limit. "Complex" is defined as containing a `try` block, a `match` expression, a nested `for` loop, or another `loop`. Simple bodies — a single `if-then-else` with `break`/`continue` branches — can stay inline:

```ori
// Simple body: can stay inline
loop { if done then break else continue }

// Complex body: always breaks
loop {
    try {
        let input = read_line()?;
        if input == "quit" then break
    }
}
```

The `LoopRule::has_complex_body` method checks whether the immediate body expression is one of these complex constructs. The check is shallow — it does not recurse into the body's sub-expressions — because the measure pass has already marked those sub-expressions as `ALWAYS_STACKED` if they are themselves complex.

### Seq Helpers

**File**: `rules/seq_helpers.rs`

Three query functions identify patterns in `FunctionSeq` expressions:

- `is_try(arena, expr_id)` — true when the expression is a `FunctionSeq::Try { .. }`
- `is_match_seq(arena, expr_id)` — true when the expression is a `FunctionSeq::Match { .. }`
- `is_function_seq(arena, expr_id)` — true when the expression is any `FunctionSeq` variant

These are used by the orchestration layer to identify always-stacked patterns without needing to directly inspect `FunctionSeq` internals. They complement the `ALWAYS_STACKED` sentinel for cases where the caller needs to know what kind of stacked construct it is dealing with.

---

## Orchestration (Layer 5)

The `Formatter` struct in `compiler/ori_fmt/src/formatter/mod.rs` is the render-pass entry point. It holds:

- `arena: &ExprArena` — the expression arena from the parse phase
- `interner: &I` — the string interner, implementing `StringLookup`
- `width_calc: WidthCalculator<I>` — the measure-pass cache
- `ctx: FormatContext<StringEmitter>` — the format context tracking position and emitting text

The `format()` method is the central dispatch described in the two-pass algorithm section above. All three rendering paths — `emit_inline`, `emit_broken`, and `emit_stacked` — are exhaustive matches over every `ExprKind` variant. Adding a new `ExprKind` variant causes a compile error in all three dispatch functions, enforcing that new expression kinds always have a complete rendering implementation.

### emit\_inline

`emit_inline` renders every `ExprKind` on a single line. The core rendering conventions:

**Literals**: emitted verbatim — integers, floats, strings (with surrounding quotes), chars, `true`/`false`, `None`, `()`.

**Binary expressions**: left operand, space, operator string, space, right operand. Operands requiring precedence-based parentheses (lower-precedence child operations, or lambdas/lets/ifs as operands) are wrapped in `(...)`.

**Function calls** (`Call`): the call target, `(`, comma-separated positional arguments, `)`. Each argument is recursively formatted inline.

**Named calls** (`CallNamed`): same as `Call` but arguments are rendered as `name: value` pairs.

**Method calls**: receiver, `.`, method name, `(`, arguments, `)`.

**Blocks**: `{` space statements separated by `; ` space result `}`. Blocks with only a result expression and no statements render as `{ result }`.

**Lists**: `[` items `]`. **Maps**: `{ key: value, ... }`. **Structs**: `Name { field: value, ... }`. **Tuples**: `(a, b, c)`.

**If expressions**: `if ` condition ` then ` then-branch ` else ` else-branch.

**Lambda**: single-param lambdas omit parentheses around the parameter (`x -> x * 2`); multi-param lambdas use them (`(a, b) -> a + b`).

### emit\_broken

`emit_broken` renders expressions that do not fit inline. Constructs that are always stacked (`Block`, `Match`, `FunctionSeq`, `FunctionExp`) delegate immediately to `emit_stacked`. Leaf expressions and simple compounds that have no internal structure to break delegate to `emit_inline` — there is no way to make them shorter. The interesting cases are the compound expressions that have meaningful broken forms:

**Binary expressions**: the left operand renders first (potentially breaking its own contents), then a newline at the current indentation, then the operator and a space, then the right operand. This places the operator at the start of the continuation line, matching the formatting style used in Ori's standard library and specification examples.

**Function calls and method calls**: the call target renders first, then `(`, then a newline and indent, then each argument on its own indented line followed by a trailing comma, then a newline/dedent and `)`.

**If expressions**: the broken renderer first checks whether the initial segment — `if ` condition ` then ` then-branch — fits at the current column. If it does, that segment renders inline, and the `else` breaks to a new line. If the segment itself does not fit, the `then` keyword appears at the end of the condition line, and the then-branch breaks to an indented new line. Each `else if` chain is then handled recursively by `emit_else_branch`, which applies the same per-segment fit check.

**Let bindings**: `let ` pattern ` =` on the current line, then the initializer breaks to an indented new line. This prevents the common case where `let x = some_long_expression` pushes the initializer off the right margin without any visual indication of the break.

**For expressions**: `for pattern in iter yield` on the current line, then the body breaks to an indented new line. Short bodies (as classified by `ShortBodyRule`) stay on the same line as `yield`.

**Lambda expressions**: the parameter list renders inline, then `->` on the same line, then the body breaks to an indented new line.

**Collections** (lists, maps, structs, tuples): when broken, each item goes on its own indented line with a trailing comma. The `{` or `[` opening stays on the current line, and the `}` or `]` closing appears on its own line at the original indentation.

### emit\_stacked

`emit_stacked` handles constructs that are always multi-line regardless of width. The most important cases:

**Block** (`ExprKind::Block`): emits `{`, increases indentation, then for each statement emits a newline+indent, the statement, and `;`. If there are two or more statements followed by a result expression, a blank line appears before the result — this is the "setup + result" visual pattern that the Ori spec requires. Then the result expression, dedent, newline+indent, `}`.

```text
{
    let x = compute();
    let y = transform(x);

    x + y
}
```

**Match** (`ExprKind::Match`): emits `match ` scrutinee ` {`, increases indentation, then for each arm: newline+indent, the pattern, optional ` if ` guard, ` -> `, the body, `,`. Then dedent, newline+indent, `}`.

**Try** (`FunctionSeq::Try`): follows the same structure as `Block` but leads with `try {`. The `try` prefix signals to the type checker that `?` propagation is active within this block.

**FunctionExp** (`recurse`, `parallel`, `spawn`, etc.): the function name, `(`, newline+indent, then each named property (`name: value,`) on its own indented line, then dedent+newline+indent, `)`.

**ForPattern** (`FunctionSeq::ForPattern`): the `for(` function expression form with each named argument (`over:`, `map:`, `match:`, `default:`) on its own line.

For compound expressions that happen to be reached by `emit_stacked` but are not intrinsically stacked (binary expressions, calls, collections, `if`, `let`, `lambda`, `for`), `emit_stacked` delegates to `emit_broken`. For leaf expressions (literals, identifiers), it delegates to `emit_inline`. The exhaustive match in `emit_stacked` groups variants into three categories: custom stacked rendering, delegation to `emit_broken`, and delegation to `emit_inline`.

### Function Body Formatting

Function bodies present a three-way decision at the declaration level (handled by `ModuleFormatter`, not `Formatter`):

1. **Inline** (`= expr`): if the entire body fits on the same line as the function signature, the `=` and body appear on that line.
2. **Break to newline** (`=` newline indent body): if the body does not fit inline but the body expression renders internally as a single broken line, the `=` goes on the signature line and the body breaks to the next line at `indent_size` indent.
3. **Break internally**: block bodies, `match` expressions, and other always-stacked constructs that must span multiple lines render with their own internal structure.

### ModuleFormatter and Declaration Order

The `ModuleFormatter` in `compiler/ori_fmt/src/declarations/mod.rs` formats entire modules. It establishes the canonical declaration order that all Ori source files must follow:

1. File attributes (`#!target(...)`, `#!cfg(...)`)
2. Regular imports (`use ...`)
3. Extension imports (`extension ...`)
4. Constants (`let $name = ...`)
5. Type definitions (`type ...`)
6. Trait definitions (`trait ...`)
7. Impl blocks (`impl ...`)
8. Default impl blocks (`pub def impl ...`)
9. Extension blocks (`extend ...`)
10. Extern blocks (`extern "c" ...`, `extern "js" ...`)
11. Functions (`@name ...`)
12. Tests (`@name tests ...`)

Groups are separated by blank lines. Within each group (imports, constants, etc.), items are separated by single newlines. This ordering is enforced by the formatter regardless of the order in which the items appear in the source — a file with functions before imports will be reformatted with imports first.

---

## Comment Handling

Comments are a sidecar in Ori's formatter: they live outside the AST and are associated with AST positions through a position index rather than being embedded as AST nodes. This architecture allows the AST to remain clean while comments are still preserved correctly.

The `CommentIndex` struct in `compiler/ori_fmt/src/comments/mod.rs` maps source positions to comments using a `BTreeMap<u32, Vec<CommentRef>>`. The key is the start position of the AST node that follows each comment. When the formatter renders a declaration, it calls `take_comments_before(pos)` with the declaration's start position to retrieve and consume the comments that should appear immediately before that declaration.

The binary search (`partition_point`) on the sorted token position list ensures O(log n) comment association rather than O(n) linear scan. Once a comment is taken by `take_comments_before`, it is marked consumed and will not be returned again, even if the formatter later encounters the same position.

**Doc comment reordering**: Ori's doc comment syntax has four ordered categories:

1. Description (`// Description text`) — sort order 0
2. Member docs (`// * name: description`) — sort order 1
3. Warnings (`// ! Warning text`) — sort order 2
4. Examples (`// > expr -> result`) — sort order 3

When doc comments appear in an arbitrary order in the source, the formatter reorders them to this canonical sequence. Regular comments (`//` without a doc prefix) are not reordered and preserve their relative order within the block.

For function declarations, the `take_comments_before_function` method additionally reorders member doc comments (`// * param:`) to match the order of parameters in the function signature. If parameters are `(x: int, y: str)` but the doc comments appear in reverse order, the formatter emits them in signature order. The same logic applies to struct type declarations via `take_comments_before_type`, which reorders field doc comments to match field order.

The `format_comment` function normalizes comment text, ensuring exactly one space follows `//`. Comments that already have this spacing are unchanged; comments that lack a space have one inserted.

---

## Rule Interaction and Priorities

When multiple formatting concerns apply to the same expression, the formatter resolves them through a strict priority ordering:

1. **Always-stacked sentinel** takes unconditional precedence. If a node's cached width is `ALWAYS_STACKED`, `emit_stacked` is called immediately, and no width check or breaking rule is consulted. This applies to `match`, `try`, `recurse`, `parallel`, `spawn`, and `nursery`.

2. **Width check** determines inline vs broken for all other nodes. If `ctx.fits(width)` is true, the node renders inline, and no breaking rule is consulted. Breaking rules exist inside `emit_broken` and are irrelevant for nodes that fit inline.

3. **Breaking rules** (Layer 4) are consulted within `emit_broken`. The broken renderer checks for method chains (`is_method_chain`), boolean OR patterns (`is_or_expression`, `BooleanBreakRule::should_break_at_or`), chained if-else (`collect_if_chain`), nested for loops (`collect_for_chain`), and complex loop bodies (`LoopRule::has_complex_body`). These rules determine the specific multi-line layout within the broken rendering path.

4. **Packing strategy** (Layer 2) applies within container rendering. When a list, struct, or function call breaks, the `ConstructKind` and user intent signals (trailing comma, comments, blank lines) determine whether items pack multiple per line or go one per line. Packing is a sub-concern within the already-decided "this container is broken" path.

5. **Spacing rules** (Layer 1) apply at the individual token emission level within both inline and broken rendering. They are the final layer consulted before any text character is written. Token spacing cannot override any higher-level decision — it only determines whether a space appears between two adjacent tokens that the higher layers have already decided to emit.

This strict hierarchy means there are no ambiguous cases. A given expression has at most one applicable path through the hierarchy, and the priority order resolves it deterministically.

---

## Prior Art

**[Wadler's "A Prettier Printer"](https://homepages.inf.ed.ac.uk/wadler/papers/prettier/prettier.pdf)** (2003) established the document algebra approach to pretty-printing. The core insight is the `group` combinator: a group tries to render its contents on one line, and if that fails, switches to a line-by-line rendering. This gives the algorithm a clean separation between "measure" and "render" at the conceptual level, though Wadler's algorithm is actually single-pass with a clever look-ahead using lazy evaluation. The algebra — `group`, `indent`, `line`, `text` — is elegant and composable. Ori's approach achieves similar composability through width caching without the document IR.

**[Lindig's "Strictly Pretty"](https://lindig.github.io/papers/strictly-pretty-2000.pdf)** (2000) preceded Wadler's 2003 revision and gave the first efficient strict-evaluation implementation of the pretty-printing algebra. Where Wadler's Haskell implementation relied on lazy lists for efficiency, Lindig showed that a stack-based implementation with a scan-ahead buffer achieves the same result in strict languages. The Ori width-caching approach is in the same spirit: it replaces lazy evaluation with an explicit cache.

**[Prettier](https://prettier.io/)** is the most influential practical implementation of Wadler's ideas. Prettier converts the AST to a "print IR" with `group`, `indent`, and `hardline` nodes, then renders with a width-aware algorithm. Prettier's fill/group handling is subtly different from the pure Wadler algorithm and has been refined over many versions to handle practical cases. Ori's two-pass approach avoids the fill/group complexity by pre-computing widths rather than discovering them during rendering.

**[`rustfmt`](https://github.com/rust-lang/rustfmt)** is the closest analog in terms of feature set. `rustfmt` operates on the Rust AST, uses width-based breaking, preserves user intent signals (trailing commas), applies construct-specific rules (method chains, boolean expressions, chained if-else), and provides independent nested breaking. Ori's named rules for method chains, boolean breaks, and chained else-if all have direct counterparts in `rustfmt`. The key difference is that `rustfmt` offers many configuration options while Ori deliberately offers almost none.

**[`gofmt`](https://pkg.go.dev/cmd/gofmt)** established the zero-configuration philosophy and is the foundational inspiration for Ori's formatter. `gofmt` is source-preserving — it does not make width-based breaking decisions. Ori adds width-based breaking because Ori's expression-based syntax produces longer lines than Go's statement-based syntax; a formatter that preserves user formatting would fail to enforce the 100-character line limit.

**[`gleam format`](https://gleam.run/writing-gleam/)** influenced Ori's container packing strategy. Gleam's formatter uses a fit-or-stack approach with all-or-nothing container breaking similar to Ori's `FitOrOnePerLine` packing strategy. Gleam formats its AST without a document IR, making similar measure-then-render decisions.

---

## Design Tradeoffs

**Measure-then-render vs single-pass-with-backtracking.** The two-pass approach requires traversing every node twice and allocating a hash map for cached widths. A single-pass algorithm that backtracks on overflow would need no cache. The tradeoff is predictability: two-pass is guaranteed O(n) with no pathological cases, while single-pass-with-backtracking is O(n) on average but can degrade for deeply nested constructs. For a compiler formatter that must handle arbitrarily complex generated or machine-written code, the predictability guarantee is valuable.

**Named rules vs generic document IR.** The Layer 4 named rules — `MethodChainRule`, `BooleanBreakRule`, etc. — are language-specific and would need to be reimplemented for any other language. A document IR approach (Wadler/Prettier) is language-agnostic: each language constructs a document IR from its AST, and the renderer handles the rest. The named-rule approach is more verbose but more transparent — each rule's logic is in a self-contained file with a clear name and purpose. Adding a new formatting convention means adding a new named rule file rather than understanding how it interacts with the document algebra.

**`ALWAYS_STACKED` sentinel vs boolean flag.** Using `usize::MAX` as a sentinel rather than `Option<usize>` or a separate boolean flag means the width cache stores only one type of value. The sentinel propagates naturally through arithmetic in `accumulate_widths` — any item with sentinel width causes the accumulation to return sentinel immediately. The tradeoff is that the sentinel value is a magic number that must be documented clearly. The `ALWAYS_STACKED` constant and the `pub const ALWAYS_STACKED: usize = usize::MAX` declaration in `width/mod.rs` make this explicit.

**Independent breaking via `Shape`.** `for_nested` produces shapes that allow nested constructs to stay inline even when parents break. This produces readable output for typical code, but can occasionally surprise: a construct that would be broken at the top level (because it fills the line) stays inline at a nested level (because at the deeper indentation it has more room). Some formatters cascade breaks — any time a parent breaks, children also break — producing more uniform vertical layout at the cost of excessive expansion. Ori's independent breaking follows `rustfmt`'s approach and prioritizes compactness over uniformity.

**Comment sidecar vs inline comment nodes.** Storing comments in a `CommentIndex` keyed by source position rather than embedding them in the AST keeps the AST clean and the formatter logic simpler. The cost is that comments can only be associated with declarations, not with arbitrary sub-expressions. A comment inside a function body that appears between two statements is associated with the statement that follows it. Comments inside expressions (between terms of a binary expression, between list elements) are handled by the packing layer's user-intent signals, not the comment sidecar.

---

## Related Documents

- [Formatter Overview](./index.md) — Five-layer architecture, pipeline, CLI integration
- [Token Spacing](./spacing.md) — Layer 1: declarative O(1) spacing rules
- [Packing](./packing.md) — Layer 2: container inline/break decisions and user intent signals
