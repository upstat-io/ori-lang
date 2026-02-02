---
section: "04"
title: Breaking Rules
status: not-started
goal: Ori-specific breaking rules for special constructs
sections:
  - id: "04.1"
    title: MethodChainRule
    status: not-started
  - id: "04.2"
    title: ShortBodyRule
    status: not-started
  - id: "04.3"
    title: BooleanBreakRule
    status: not-started
  - id: "04.4"
    title: ChainedElseIfRule
    status: not-started
  - id: "04.5"
    title: NestedForRule
    status: not-started
  - id: "04.6"
    title: ParenthesesRule
    status: not-started
  - id: "04.7"
    title: RunRule
    status: not-started
  - id: "04.8"
    title: LoopRule
    status: not-started
---

# Section 04: Breaking Rules

**Status:** 📋 Planned
**Goal:** Implement 8 Ori-specific breaking rules that don't fit into simple packing

> **Spec Reference:** Various sections in 16-formatting.md
> **Design Decisions:** See `00-overview.md` finalized decisions table

---

## 04.1 MethodChainRule

**Decision:** Strict all-or-nothing. All chain elements break together.

> **Spec Reference:** Lines 493-510

- [ ] **Create** `ori_fmt/src/rules/method_chain.rs`
- [ ] **Implement** `MethodChainRule`

```rust
/// "Receiver stays on assignment/yield line, break at every . once any break needed"
pub struct MethodChainRule;

impl MethodChainRule {
    /// All methods break together (not selective)
    pub const ALL_METHODS_BREAK: bool = true;

    pub fn format(
        chain: &MethodChain,
        shape: Shape,
        emitter: &mut Emitter,
        arena: &ExprArena,
    ) {
        // Try inline first
        if let Some(inline) = Self::try_inline(chain, arena) {
            if shape.fits_str(&inline) {
                emitter.emit(&inline);
                return;
            }
        }

        // Broken: receiver on current line, all methods on new lines
        emitter.format_expr(chain.receiver);
        for method in &chain.calls {
            emitter.newline();
            emitter.emit(".");
            emitter.emit(&method.name);
            emitter.emit("(");
            emitter.format_args(&method.args);
            emitter.emit(")");
        }
    }
}
```

- [ ] **Tests**: Chains that fit inline, chains that break

**Example:**
```ori
// Fits inline:
items.map(x -> x * 2).filter(x -> x > 0)

// Breaks all together:
items
    .map(x -> x * 2)
    .filter(x -> x > 0)
    .take(n: 10)
```

---

## 04.2 ShortBodyRule

**Decision:** ~20 character threshold. Under 20 chars stays with yield/do.

> **Spec Reference:** Lines 751-766

- [ ] **Create** `ori_fmt/src/rules/short_body.rs`
- [ ] **Implement** `ShortBodyRule`

```rust
/// "A simple body must remain with yield/do even when overall line is long"
/// "A lone identifier or literal never appears on its own line"
pub struct ShortBodyRule;

impl ShortBodyRule {
    pub const THRESHOLD: usize = 20;

    pub fn is_short_body(expr: &Expr, arena: &ExprArena) -> bool {
        match &expr.kind {
            ExprKind::Ident { .. } => true,
            ExprKind::Literal { .. } => true,
            _ => {
                // Check inline length
                let inline = try_inline(expr, arena);
                inline.map(|s| s.len() <= Self::THRESHOLD).unwrap_or(false)
            }
        }
    }

    /// When body is short, break BEFORE for, not after yield
    pub fn break_point(for_expr: &ForExpr, arena: &ExprArena) -> BreakPoint {
        let body = arena.get_expr(for_expr.body);
        if Self::is_short_body(body, arena) {
            BreakPoint::BeforeFor
        } else {
            BreakPoint::AfterYield
        }
    }
}

pub enum BreakPoint {
    BeforeFor,
    AfterYield,
}
```

- [ ] **Tests**: Various body lengths around threshold

**Example:**
```ori
// Short body (≤20 chars) stays with yield:
for user in users yield user.name

// Long body breaks after yield:
for user in users yield
    user.transform().validate().save()
```

---

## 04.3 BooleanBreakRule

**Decision:** 3+ `||` clauses OR exceeds width triggers breaking.

> **Spec Reference:** Lines 473-483

- [ ] **Create** `ori_fmt/src/rules/boolean_break.rs`
- [ ] **Implement** `BooleanBreakRule`

```rust
/// "When a boolean expression contains multiple || clauses,
///  each clause receives its own line with || at the start"
pub struct BooleanBreakRule;

impl BooleanBreakRule {
    pub const OR_THRESHOLD: usize = 3;  // 3+ || clauses

    pub fn should_break_at_or(expr: &Expr) -> bool {
        let or_count = Self::count_top_level_or(expr);
        or_count >= Self::OR_THRESHOLD
    }

    fn count_top_level_or(expr: &Expr) -> usize {
        match &expr.kind {
            ExprKind::Binary { op: BinaryOp::Or, left, right } => {
                1 + Self::count_top_level_or(left)
            }
            _ => 0,
        }
    }

    pub fn format(
        clauses: &[ExprId],
        shape: Shape,
        emitter: &mut Emitter,
        arena: &ExprArena,
    ) {
        // First clause on current line
        emitter.format_expr(clauses[0]);

        // Subsequent clauses with leading ||
        for clause in &clauses[1..] {
            emitter.newline();
            emitter.emit("|| ");
            emitter.format_expr(*clause);
        }
    }
}
```

- [ ] **Tests**: 2 clauses (no break), 3 clauses (break), mixed

**Example:**
```ori
// 2 clauses (no break):
if a || b then x

// 3+ clauses (break with leading ||):
if user.active && user.verified
    || user.is_admin
    || user.bypass_check then x
```

---

## 04.4 ChainedElseIfRule

**Decision:** Kotlin style — first `if` stays with assignment, else clauses indented.

> **Spec Reference:** Lines 428-444
> **⚠️ SPEC UPDATE REQUIRED** — Current spec differs

- [ ] **Create** `ori_fmt/src/rules/chained_else_if.rs`
- [ ] **Implement** `ChainedElseIfRule`

```rust
/// Kotlin style: first if stays with assignment, else clauses on own lines
pub struct ChainedElseIfRule;

impl ChainedElseIfRule {
    pub fn format(
        if_expr: &IfExpr,
        shape: Shape,
        emitter: &mut Emitter,
        arena: &ExprArena,
    ) {
        // Try inline for simple cases (no else-if)
        if !if_expr.has_else_if() {
            if let Some(inline) = Self::try_inline(if_expr, arena) {
                if shape.fits_str(&inline) {
                    emitter.emit(&inline);
                    return;
                }
            }
        }

        // Broken: Kotlin style
        emitter.emit("if ");
        emitter.format_expr(if_expr.condition);
        emitter.emit(" then ");
        emitter.format_expr(if_expr.then_branch);

        for else_if in &if_expr.else_ifs {
            emitter.newline();
            emitter.emit("else if ");
            emitter.format_expr(else_if.condition);
            emitter.emit(" then ");
            emitter.format_expr(else_if.then_branch);
        }

        if let Some(else_branch) = &if_expr.else_branch {
            emitter.newline();
            emitter.emit("else ");
            emitter.format_expr(*else_branch);
        }
    }
}
```

- [ ] **Tests**: Simple if, if-else, if-else-if chains

**Example (NEW - Kotlin style):**
```ori
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

**Example (OLD spec - to be replaced):**
```ori
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

---

## 04.5 NestedForRule

**Decision:** Rust-style. Each nested for increases indentation.

> **Spec Reference:** Lines 818-830

- [ ] **Create** `ori_fmt/src/rules/nested_for.rs`
- [ ] **Implement** `NestedForRule`

```rust
/// "Each nesting level gets its own line with incremented indentation"
pub struct NestedForRule;

impl NestedForRule {
    pub fn format(
        for_expr: &ForExpr,
        shape: Shape,
        emitter: &mut Emitter,
        arena: &ExprArena,
        config: &FormatterConfig,
    ) {
        emitter.emit("for ");
        emitter.format_pattern(&for_expr.pattern);
        emitter.emit(" in ");
        emitter.format_expr(for_expr.iter);
        emitter.emit(" yield");

        let body = arena.get_expr(for_expr.body);

        // If body is another for, break and indent
        if matches!(body.kind, ExprKind::For { .. }) {
            emitter.newline();
            emitter.indent();
            Self::format(body.as_for(), shape.indent(config.indent_size), emitter, arena, config);
            emitter.dedent();
        } else if ShortBodyRule::is_short_body(body, arena) {
            // Short body stays with yield
            emitter.emit(" ");
            emitter.format_expr(for_expr.body);
        } else {
            // Long body on next line
            emitter.newline();
            emitter.indent();
            emitter.format_expr(for_expr.body);
            emitter.dedent();
        }
    }
}
```

- [ ] **Tests**: Single for, nested for (2 levels), deeply nested (3+ levels)

**Example:**
```ori
for user in users yield
    for permission in user.permissions yield
        for action in permission.actions yield
            action.name
```

---

## 04.6 ParenthesesRule

**Decision:** Preserve all user parens. Add when semantically needed, never remove.

> **Spec Reference:** Lines 974-1023

- [ ] **Create** `ori_fmt/src/rules/parentheses.rs`
- [ ] **Implement** `needs_parens()` function

```rust
/// Determines when parens are semantically required
pub fn needs_parens(expr: &Expr, position: ParenPosition) -> bool {
    use ExprKind::*;
    use ParenPosition::*;

    match position {
        // Spec lines 978-992: Method receiver
        Receiver => matches!(expr.kind,
            Binary { .. } | Unary { .. } | If { .. } | Lambda { .. } |
            Let { .. } | Range { .. } | For { .. } | Loop { .. }
        ),

        // Spec lines 994-1001: Call target
        CallTarget => matches!(expr.kind,
            Binary { .. } | Unary { .. } | If { .. } | Lambda { .. } |
            Let { .. } | Range { .. } | For { .. } | Loop { .. }
        ),

        // Spec lines 1003-1010: Iterator source
        IteratorSource => matches!(expr.kind,
            For { .. } | If { .. } | Lambda { .. } | Let { .. }
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParenPosition {
    Receiver,       // x.method() — x needs parens if complex
    CallTarget,     // f(args) — f needs parens if complex
    IteratorSource, // for x in y — y needs parens if complex
}
```

- [ ] **Implement** user paren preservation

```rust
/// Track user-provided parentheses in AST
pub struct ParenInfo {
    /// Expressions that had explicit parens in source
    pub user_parens: HashSet<ExprId>,
}

impl ParenInfo {
    pub fn has_user_parens(&self, expr: ExprId) -> bool {
        self.user_parens.contains(&expr)
    }
}
```

- [ ] **Tests**: All ParenPosition cases, user paren preservation

**Example:**
```ori
// Required parens (semantically needed):
(for x in items yield x).fold(init: 0, f: acc, x -> acc + x)
(x -> x * 2)(5)

// User parens (preserved even if not required):
let result = (a + b) * c  // User's parens preserved
```

---

## 04.7 RunRule

**Decision:** Top-level = always stacked; nested = width-based.

> **Spec Reference:** Lines 514-564

- [ ] **Create** `ori_fmt/src/rules/run_rule.rs`
- [ ] **Implement** `RunRule`

```rust
/// Top-level run = always stacked; nested run = width-based
pub struct RunRule;

impl RunRule {
    pub fn packing(is_top_level: bool) -> Packing {
        if is_top_level {
            Packing::AlwaysStacked
        } else {
            Packing::FitOrOnePerLine
        }
    }

    pub fn is_top_level(ctx: &FormattingContext) -> bool {
        // Top-level means: function body, not inside another expression
        ctx.depth == 0 || ctx.is_function_body
    }
}
```

- [ ] **Tests**: Top-level run stacks, nested run can inline

**Example:**
```ori
// Top-level run (always stacked):
@main () -> void = run(
    let x = 1,
    print(msg: x.to_str()),
)

// Nested run (width-based, can inline if fits):
let logged = run(print(msg: value.to_str()), value)
```

---

## 04.8 LoopRule

**Decision:** Complex body (contains run/try/match/for) always breaks.

> **Spec Reference:** Lines 589-617

- [ ] **Create** `ori_fmt/src/rules/loop_rule.rs`
- [ ] **Implement** `LoopRule`

```rust
/// "When loop contains complex body (run, try, match, for), break after loop("
pub struct LoopRule;

impl LoopRule {
    pub fn has_complex_body(body: &Expr) -> bool {
        match &body.kind {
            ExprKind::Call { func, .. } => {
                // Check if it's run/try/match
                Self::is_complex_builtin(func)
            }
            ExprKind::For { .. } => true,
            _ => false,
        }
    }

    fn is_complex_builtin(func: &Expr) -> bool {
        if let ExprKind::Ident { name, .. } = &func.kind {
            matches!(name.as_str(), "run" | "try" | "match" | "recurse")
        } else {
            false
        }
    }

    pub fn format(
        loop_expr: &LoopExpr,
        shape: Shape,
        emitter: &mut Emitter,
        arena: &ExprArena,
    ) {
        let body = arena.get_expr(loop_expr.body);

        if Self::has_complex_body(body) {
            // Complex: always break after loop(
            emitter.emit("loop(");
            emitter.newline();
            emitter.indent();
            emitter.format_expr(loop_expr.body);
            emitter.dedent();
            emitter.newline();
            emitter.emit(")");
        } else if let Some(inline) = try_inline_expr(loop_expr, arena) {
            if shape.fits_str(&inline) {
                emitter.emit(&inline);
                return;
            }
            // Doesn't fit: break
            Self::format_broken(loop_expr, emitter, arena);
        } else {
            Self::format_broken(loop_expr, emitter, arena);
        }
    }

    fn format_broken(
        loop_expr: &LoopExpr,
        emitter: &mut Emitter,
        arena: &ExprArena,
    ) {
        emitter.emit("loop(");
        emitter.newline();
        emitter.indent();
        emitter.format_expr(loop_expr.body);
        emitter.dedent();
        emitter.newline();
        emitter.emit(")");
    }
}
```

- [ ] **Tests**: Simple loop inline, complex loop breaks

**Example:**
```ori
// Simple body (can inline):
loop(if done then break else continue)

// Complex body (always breaks):
loop(
    run(
        let input = read_line(),
        if input == "quit" then break else continue,
    )
)
```

---

## 04.9 Completion Checklist

- [ ] All 8 breaking rules implemented
- [ ] Each rule has its own module under `ori_fmt/src/rules/`
- [ ] Unit tests for each rule
- [ ] Integration tests for rule interactions
- [ ] Documentation for each rule's semantics

**Exit Criteria:** All Ori-specific breaking rules are encapsulated in named, documented rule structs; the main formatter delegates to these rules rather than containing inline logic.
