---
title: "Layer 4: Breaking Rules"
description: "Ori Formatter Design — Ori-Specific Breaking Rules"
order: 5
section: "Layers"
---

# Layer 4: Breaking Rules

The rules layer contains eight Ori-specific breaking rules for constructs that don't fit simple packing strategies. Each rule encapsulates a formatting decision for a particular pattern.

## Architecture

```
Expression ──▶ Rule Detection ──▶ Rule-Specific Formatting
                    │
    ┌───────────────┼───────────────┐
    │               │               │
MethodChainRule  ShortBodyRule  BooleanBreakRule  ...
```

## The Eight Rules

| Rule | Purpose | Key Decision |
|------|---------|--------------|
| `MethodChainRule` | Method chains | All elements break together |
| `ShortBodyRule` | For/loop bodies | ~20 char threshold for yield/do |
| `BooleanBreakRule` | Boolean expressions | 3+ `\|\|` clauses break with leading `\|\|` |
| `ChainedElseIfRule` | If-else chains | Kotlin style (first `if` with assignment) |
| `NestedForRule` | Nested for expressions | Rust-style indentation |
| `ParenthesesRule` | Parentheses | Preserve user parens, add when needed |
| `BlockRule` | Block expressions (`{ }`) | Top-level stacked, nested width-based |
| `LoopRule` | loop() expressions | Complex body breaks |

---

## MethodChainRule

**Principle**: Method chains break as a unit — either all inline or all broken.

```ori
// Inline (fits):
items.filter(x -> x > 0).map(x -> x * 2).collect()

// Broken (all elements break together):
items
    .filter(x -> x > 0)
    .map(x -> x * 2)
    .collect()
```

**Key insight**: Unlike some formatters that break individual calls, Ori chains break uniformly for visual consistency.

### Implementation

```rust
pub struct MethodChainRule;

pub struct MethodChain {
    pub receiver: ExprId,
    pub calls: Vec<ChainedCall>,
}

pub struct ChainedCall {
    pub method: Name,
    pub args: Vec<ExprId>,
}

pub fn collect_method_chain(arena: &ExprArena, expr_id: ExprId) -> Option<MethodChain> {
    // Recursively collect .method() calls
}

pub fn is_method_chain(arena: &ExprArena, expr_id: ExprId) -> bool {
    // True if 2+ chained method calls
}
```

---

## ShortBodyRule

**Principle**: A simple body must remain with `yield`/`do` even when the overall line is long.

```ori
// Good (short body stays with yield):
for user in users yield user

// Bad (lone identifier on own line):
for user in users yield
    user
```

**Threshold**: ~20 characters

### Implementation

```rust
pub struct ShortBodyRule;

impl ShortBodyRule {
    pub const THRESHOLD: usize = 20;
}

pub fn is_short_body(arena: &ExprArena, expr_id: ExprId) -> bool {
    matches!(
        &expr.kind,
        ExprKind::Ident(_)
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Unit
            | ExprKind::None
            | ExprKind::Continue(None)
            | ExprKind::Break(None)
            | ...
    )
}
```

### Break Point Decision

```rust
pub enum BreakPoint {
    BeforeFor,  // Break before `for` (short body)
    AfterYield, // Break after `yield` (complex body)
    NoBreak,    // Fits inline
}

pub fn suggest_break_point(arena: &ExprArena, body: ExprId) -> BreakPoint {
    if is_short_body(arena, body) {
        BreakPoint::BeforeFor  // Keep body with yield
    } else {
        BreakPoint::AfterYield // Complex body on new line
    }
}
```

---

## BooleanBreakRule

**Principle**: 3+ `||` clauses break with leading `||` for visual alignment.

```ori
// 2 clauses (inline):
is_admin || is_moderator

// 3+ clauses (break with leading ||):
is_admin
    || is_moderator
    || is_owner
    || has_permission
```

### Implementation

```rust
pub struct BooleanBreakRule;

pub fn collect_or_clauses(arena: &ExprArena, expr_id: ExprId) -> Vec<ExprId> {
    // Recursively collect all || operands
}

pub fn is_or_expression(arena: &ExprArena, expr_id: ExprId) -> bool {
    matches!(&arena.get_expr(expr_id).kind, ExprKind::Binary { op: BinaryOp::Or, .. })
}
```

---

## ChainedElseIfRule

**Principle**: Kotlin-style formatting — first `if` stays with assignment, else-if chains on new lines.

```ori
// Inline conditional:
let status = if success then "ok" else "error"

// Chained else-if:
let status = if score >= 90 then "A"
    else if score >= 80 then "B"
    else if score >= 70 then "C"
    else "F"
```

### Implementation

```rust
pub struct ChainedElseIfRule;

pub struct IfChain {
    pub branches: Vec<ElseIfBranch>,
    pub final_else: Option<ExprId>,
}

pub struct ElseIfBranch {
    pub condition: ExprId,
    pub then_branch: ExprId,
}

pub fn collect_if_chain(arena: &ExprArena, expr_id: ExprId) -> Option<IfChain> {
    // Recursively collect if-else-if-else chain
}
```

---

## NestedForRule

**Principle**: Rust-style indentation for nested `for` expressions.

```ori
// Nested for with consistent indentation:
for x in xs
    for y in ys
        for z in zs yield
            process(x, y, z)
```

### Implementation

```rust
pub struct NestedForRule;

pub struct ForChain {
    pub levels: Vec<ForLevel>,
    pub body: ExprId,
}

pub struct ForLevel {
    pub binding: Pattern,
    pub iterator: ExprId,
    pub condition: Option<ExprId>,  // if guard
}

pub fn collect_for_chain(arena: &ExprArena, expr_id: ExprId) -> ForChain {
    // Recursively collect nested for expressions
}
```

---

## ParenthesesRule

**Principle**: Preserve user parentheses. Add when semantically needed, never remove.

### Required Parentheses

```ori
// Method receiver (complex expr):
(for x in items yield x).fold(0, acc, x -> acc + x)

// Call target (lambda):
(x -> x * 2)(5)

// Iterator source (nested for):
for x in (inner) yield x
```

### Implementation

```rust
pub struct ParenthesesRule;

pub enum ParenPosition {
    Receiver,       // x in x.method()
    CallTarget,     // f in f(args)
    IteratorSource, // y in `for x in y`
    BinaryOperand,  // operand precedence
    UnaryOperand,   // unary operand
}

pub fn needs_parens(arena: &ExprArena, expr_id: ExprId, position: ParenPosition) -> bool {
    let expr = arena.get_expr(expr_id);

    match position {
        ParenPosition::Receiver => matches!(
            &expr.kind,
            ExprKind::Binary { .. }
                | ExprKind::Lambda { .. }
                | ExprKind::For { .. }
                | ExprKind::Block { .. }
                | ...
        ),
        // ... other positions
    }
}
```

### Current Limitation

The AST does not track whether parentheses were explicitly written by the user. `ParenthesesRule::has_user_parens()` always returns `false`. User parentheses that are semantically optional but aid readability may be removed.

---

## BlockRule

**Principle**: Top-level blocks always stack. Nested blocks use width-based decisions.

```ori
// Top-level block (always stacked):
@main () -> void = {
    let x = compute();
    let y = process(x);
    x + y
}

// Nested block (can inline if fits):
let result = if condition
    then { a; b }
    else { c; d }
```

### Implementation

```rust
pub struct BlockRule;

pub enum BlockContext {
    TopLevel,  // Function body level
    Nested,    // Inside another expression
}

pub fn is_block(arena: &ExprArena, expr_id: ExprId) -> bool {
    matches!(&arena.get_expr(expr_id).kind, ExprKind::Block { .. })
}

pub fn is_try(arena: &ExprArena, expr_id: ExprId) -> bool {
    // Check for try { ... } pattern
}
```

---

## LoopRule

**Principle**: Complex loop body (block/try/match/for) breaks to new line.

```ori
// Simple body inline:
loop { process() }

// Complex body breaks:
loop {
    step1;
    step2;
}
```

### Implementation

```rust
pub struct LoopRule;

pub fn is_loop(arena: &ExprArena, expr_id: ExprId) -> bool {
    matches!(&arena.get_expr(expr_id).kind, ExprKind::Loop { .. })
}

pub fn get_loop_body(arena: &ExprArena, expr_id: ExprId) -> Option<ExprId> {
    // Extract body from Loop expression
}

pub fn is_simple_conditional_body(arena: &ExprArena, body: ExprId) -> bool {
    // Simple if without complex nesting
}
```

---

## Adding New Rules

1. **Create module**: `rules/my_rule.rs`

2. **Define rule struct**:
   ```rust
   pub struct MyRule;

   impl MyRule {
       pub const THRESHOLD: usize = 42;  // if needed
   }
   ```

3. **Add detection function**:
   ```rust
   pub fn is_my_pattern(arena: &ExprArena, expr_id: ExprId) -> bool {
       // Pattern detection
   }
   ```

4. **Add decision function**:
   ```rust
   pub fn my_decision(arena: &ExprArena, expr_id: ExprId) -> MyDecision {
       // Formatting decision
   }
   ```

5. **Export from `rules/mod.rs`**:
   ```rust
   mod my_rule;
   pub use my_rule::{is_my_pattern, my_decision, MyRule};
   ```

6. **Add tests in `rules/tests.rs`**

7. **Integrate with formatter**: Update orchestration layer to use the rule.

## Implementation Status

Not all rules are fully integrated into the formatter pipeline:

- **`MethodChainRule`**: Infrastructure defined (`collect_method_chain()`, `is_method_chain()`) but **not yet invoked** by the emitter. Method chains currently fall through to generic expression formatting.
- **Incremental formatting**: Implemented in `ori_fmt/src/incremental.rs` with declaration-level granularity. Supports LSP format-on-type and large-file partial formatting. Covered by tests in `ori_fmt/tests/incremental_tests.rs`. Current limitation: changes to imports or constants trigger a full reformat.

## Spec Reference

Rules implement various sections of the formatting spec:
- Lines 751-766: Short body rule
- Lines 974-1023: Parentheses rules
- Various sections for other rules
