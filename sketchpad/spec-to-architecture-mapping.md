# Spec Rules → Architecture Layer Mapping

Maps each rule from `16-formatting.md` to the proposed 5-layer architecture.

---

## Finalized Decisions (from interactive review)

| # | Rule | Decision |
|---|------|----------|
| 1 | **MethodChainRule** | Strict all-or-nothing. All chain elements break together. |
| 2 | **ShortBodyRule** | ~20 character threshold. Under 20 chars stays with yield/do. |
| 3 | **BooleanBreakRule** | 3+ `\|\|` clauses OR exceeds width triggers breaking. |
| 4 | **ChainedElseIfRule** | **Kotlin style** — first `if` stays with assignment, else clauses indented. ⚠️ *Spec update needed* |
| 5 | **NestedForRule** | Rust-style. Each nested for increases indentation. |
| 6 | **ParenthesesRule** | Preserve all. Add when semantically needed, never remove user's parens. |
| 7 | **RunRule** | Top-level = stacked; nested = width-based. All statements on new lines. |
| 8 | **LoopRule** | Complex = contains run/try/match/for. Complex body breaks. |

### Spec Change Required

**ChainedElseIfRule** differs from current spec (lines 432-436):

```ori
// Current spec:
let size =
    if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"

// New decision (Kotlin style):
let size = if n < 10 then "small"
    else if n < 100 then "medium"
    else "large"
```

---

## Layer 1: Token Spacing Rules

**Source:** Spec lines 25-47 (Spacing table)

These map directly to declarative `SpaceRule` entries:

```rust
const SPACE_RULES: &[SpaceRule] = &[
    // Binary operators: Space around
    rule("SpaceBeforeBinOp", AnyToken, BinaryOp, Space),
    rule("SpaceAfterBinOp", BinaryOp, AnyToken, Space),

    // Arrows: Space around
    rule("SpaceBeforeArrow", AnyToken, Arrow, Space),
    rule("SpaceAfterArrow", Arrow, AnyToken, Space),

    // Colons: Space after (type annotations)
    rule("SpaceAfterColon", Colon, AnyToken, Space),
    rule("NoSpaceBeforeColon", AnyToken, Colon, None),  // implicit

    // Commas: Space after
    rule("SpaceAfterComma", Comma, AnyToken, Space),
    rule("NoSpaceBeforeComma", AnyToken, Comma, None),

    // Parentheses: No space inside
    rule("NoSpaceAfterLParen", LParen, Not(RParen), None),
    rule("NoSpaceBeforeRParen", Not(LParen), RParen, None),

    // Brackets: No space inside
    rule("NoSpaceAfterLBracket", LBracket, Not(RBracket), None),
    rule("NoSpaceBeforeLBracket", Not(LBracket), RBracket, None),

    // Struct braces: Space inside
    rule("SpaceAfterLBrace", LBrace, Not(RBrace), Space, ctx: is_struct),
    rule("SpaceBeforeRBrace", Not(LBrace), RBrace, Space, ctx: is_struct),

    // Empty delimiters: No space
    rule("NoSpaceEmptyParens", LParen, RParen, None),
    rule("NoSpaceEmptyBrackets", LBracket, RBracket, None),
    rule("NoSpaceEmptyBraces", LBrace, RBrace, None),

    // Field/member access: No space around .
    rule("NoSpaceBeforeDot", AnyToken, Dot, None),
    rule("NoSpaceAfterDot", Dot, AnyToken, None),

    // Range operators: No space around ../..=
    rule("NoSpaceBeforeRange", AnyToken, DotDot, None),
    rule("NoSpaceAfterRange", DotDot, AnyToken, None),
    rule("NoSpaceBeforeRangeInc", AnyToken, DotDotEq, None),
    rule("NoSpaceAfterRangeInc", DotDotEq, AnyToken, None),

    // Range step: Space around by
    rule("SpaceBeforeBy", AnyToken, KwBy, Space),
    rule("SpaceAfterBy", KwBy, AnyToken, Space),

    // Spread: No space after ...
    rule("NoSpaceAfterSpread", DotDotDot, AnyToken, None),

    // Unary operators: No space after
    rule("NoSpaceAfterUnaryMinus", UnaryMinus, AnyToken, None, ctx: is_unary),
    rule("NoSpaceAfterNot", Bang, AnyToken, None, ctx: is_unary),
    rule("NoSpaceAfterBitNot", Tilde, AnyToken, None),

    // Error propagation: No space before ?
    rule("NoSpaceBeforeQuestion", AnyToken, Question, None),

    // Labels: No space around :
    rule("NoSpaceLabelColon", Ident, Colon, None, ctx: is_label),
    rule("NoSpaceAfterLabelColon", Colon, Ident, None, ctx: after_label),

    // Type conversion: Space around as/as?
    rule("SpaceBeforeAs", AnyToken, KwAs, Space),
    rule("SpaceAfterAs", KwAs, AnyToken, Space),

    // Visibility: Space after pub
    rule("SpaceAfterPub", KwPub, AnyToken, Space),

    // Generic bounds: Space after :, around +
    rule("SpaceAfterBoundColon", Colon, AnyToken, Space, ctx: is_bound),
    rule("SpaceAroundPlus", AnyToken, Plus, Space, ctx: is_bound),

    // Default type params: Space around =
    rule("SpaceAroundDefaultEq", AnyToken, Eq, Space, ctx: is_generic_default),

    // Sum type variants: Space around |
    rule("SpaceAroundPipe", AnyToken, Pipe, Space, ctx: is_sum_type),

    // Comments: Space after //
    rule("SpaceAfterComment", CommentStart, AnyToken, Space),
];
```

**Comment normalization** (lines 902-936) also fits here as post-processing rules.

---

## Layer 2: Container Packing (Gleam-style)

**Source:** Spec lines 58-92

### Packing Enum

```rust
pub enum Packing {
    /// Try single line; if doesn't fit, one item per line
    FitOrOnePerLine,

    /// Try single line; if doesn't fit, pack multiple per line
    FitOrPackMultiple,

    /// Always one item per line
    AlwaysOnePerLine,

    /// Always stacked (special formatting)
    AlwaysStacked,
}
```

### Construct → Packing Mapping

```rust
pub fn determine_packing(construct: ConstructKind, ctx: &PackingContext) -> Packing {
    use ConstructKind::*;
    use Packing::*;

    match construct {
        // === ALWAYS STACKED (Spec lines 78-90) ===
        RunTopLevel => AlwaysStacked,
        Try => AlwaysStacked,
        Match => AlwaysStacked,
        Recurse => AlwaysStacked,
        Parallel => AlwaysStacked,
        Spawn => AlwaysStacked,
        Nursery => AlwaysStacked,

        // === WIDTH-BASED: One per line when broken (Spec lines 64-74) ===
        FunctionParams => FitOrOnePerLine,
        FunctionArgs => FitOrOnePerLine,
        GenericParams => FitOrOnePerLine,
        WhereConstraints => FitOrOnePerLine,
        Capabilities => FitOrOnePerLine,
        StructFieldsDef => FitOrOnePerLine,
        StructFieldsLiteral => FitOrOnePerLine,
        SumVariants => FitOrOnePerLine,
        MapEntries => FitOrOnePerLine,
        TupleElements => FitOrOnePerLine,
        ImportItems => FitOrOnePerLine,

        // === WIDTH-BASED: Multiple per line for simple items (Spec line 75) ===
        ListSimple => FitOrPackMultiple,

        // === WIDTH-BASED: One per line for complex items (Spec line 76) ===
        ListComplex => FitOrOnePerLine,

        // === CONTEXT-DEPENDENT ===
        RunNested => FitOrOnePerLine,  // Width-based (Spec line 91)
    }
}
```

### Simple vs Complex Item Detection (for Lists)

```rust
/// Spec lines 225-242: Simple = literals, identifiers; Complex = structs, calls, nested
pub fn is_simple_item(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Literal { .. } => true,
        ExprKind::Ident { .. } => true,
        _ => false,
    }
}

pub fn list_packing(items: &[ExprId], arena: &ExprArena) -> Packing {
    if items.iter().all(|id| is_simple_item(arena.get_expr(*id))) {
        Packing::FitOrPackMultiple
    } else {
        Packing::FitOrOnePerLine
    }
}
```

---

## Layer 3: Shape Tracking

**Source:** Spec lines 14, 19, 93-95

### Configuration

```rust
pub struct FormatterConfig {
    pub max_width: usize,      // Default: 100 (Spec line 19)
    pub indent_size: usize,    // Default: 4 (Spec line 18)
}
```

### Shape Flow

```rust
impl Shape {
    /// Width-based breaking check (Spec line 14, 60)
    pub fn fits(&self, content: &str) -> bool {
        content.len() <= self.width
    }

    /// Independent breaking (Spec lines 93-95)
    /// "Nested constructs break independently based on their own width"
    pub fn for_nested(&self, max_width: usize) -> Shape {
        // Nested gets fresh width calculation from current position
        Shape {
            width: max_width.saturating_sub(self.indent),
            indent: self.indent,
            offset: self.indent,
        }
    }
}
```

---

## Layer 4: Breaking Rules (Ori-Specific)

These are the **special cases** that don't fit into simple packing or spacing.

### 4.1 Method Chains (Spec lines 493-510)

```rust
/// "Receiver stays on assignment/yield line, break at every . once any break needed"
pub struct MethodChainRule;

impl MethodChainRule {
    pub const ALL_METHODS_BREAK: bool = true;

    pub fn format(chain: &MethodChain, shape: Shape, f: &mut Formatter) {
        // Try inline first
        if let Some(inline) = chain.try_inline(shape) {
            if shape.fits(&inline) {
                f.emit(&inline);
                return;
            }
        }

        // Broken: receiver on current line, all methods on new lines
        f.format(chain.receiver);
        for method in &chain.calls {
            f.newline();
            f.emit(".");
            f.emit(&method.name);
            f.emit("(");
            f.format_args(&method.args);
            f.emit(")");
        }
    }
}
```

### 4.2 Short Body Rule (Spec lines 751-766)

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
        if Self::is_short_body(arena.get_expr(for_expr.body), arena) {
            BreakPoint::BeforeFor
        } else {
            BreakPoint::AfterYield
        }
    }
}
```

### 4.3 Long Boolean Expressions (Spec lines 473-483)

```rust
/// "When a boolean expression contains multiple || clauses,
///  each clause receives its own line with || at the start"
pub struct BooleanBreakRule;

impl BooleanBreakRule {
    pub const OR_THRESHOLD: usize = 3;  // 3+ || clauses

    pub fn should_break_at_or(expr: &Expr) -> bool {
        let or_count = count_top_level_or(expr);
        or_count >= Self::OR_THRESHOLD
    }

    pub fn format(expr: &BinaryExpr, shape: Shape, f: &mut Formatter) {
        if !Self::should_break_at_or(expr) || shape.fits_inline(expr) {
            f.format_inline(expr);
            return;
        }

        // First clause on current line
        f.format(expr.clauses[0]);

        // Subsequent clauses with leading ||
        for clause in &expr.clauses[1..] {
            f.newline();
            f.emit("|| ");
            f.format(clause);
        }
    }
}
```

### 4.4 Chained Else-If (Spec lines 428-444)

```rust
/// "Each clause goes on its own line"
pub struct ChainedElseIfRule;

impl ChainedElseIfRule {
    pub fn format(if_expr: &IfExpr, shape: Shape, f: &mut Formatter) {
        // Try inline for simple cases
        if !if_expr.has_else_if() && shape.fits_inline(if_expr) {
            f.format_inline(if_expr);
            return;
        }

        // Broken: each clause on own line
        f.emit("if ");
        f.format(if_expr.condition);
        f.emit(" then ");
        f.format(if_expr.then_branch);

        for else_if in &if_expr.else_ifs {
            f.newline();
            f.emit("else if ");
            f.format(else_if.condition);
            f.emit(" then ");
            f.format(else_if.then_branch);
        }

        if let Some(else_branch) = &if_expr.else_branch {
            f.newline();
            f.emit("else ");
            f.format(else_branch);
        }
    }
}
```

### 4.5 Nested For - Rust-style (Spec lines 818-830)

```rust
/// "Each nesting level gets its own line with incremented indentation"
pub struct NestedForRule;

impl NestedForRule {
    pub fn format(for_expr: &ForExpr, shape: Shape, f: &mut Formatter) {
        f.emit("for ");
        f.format_pattern(&for_expr.pattern);
        f.emit(" in ");
        f.format(for_expr.iter);
        f.emit(" yield");

        let body = f.arena.get_expr(for_expr.body);

        // If body is another for, break and indent
        if matches!(body.kind, ExprKind::For { .. }) {
            f.newline();
            f.indent();
            Self::format(body.as_for(), shape.indent(4), f);  // Recurse
            f.dedent();
        } else if ShortBodyRule::is_short_body(body, f.arena) {
            // Short body stays with yield
            f.emit(" ");
            f.format(for_expr.body);
        } else {
            // Long body on next line
            f.newline();
            f.indent();
            f.format(for_expr.body);
            f.dedent();
        }
    }
}
```

### 4.6 Parentheses Preservation (Spec lines 974-1023)

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
```

### 4.7 Run: Top-level vs Nested (Spec lines 514-564)

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
}
```

### 4.8 Loop with Complex Body (Spec lines 589-617)

```rust
/// "When loop contains complex body (run, try, match, for), break after loop("
pub struct LoopRule;

impl LoopRule {
    pub fn has_complex_body(body: &Expr) -> bool {
        matches!(body.kind,
            ExprKind::Call { .. } if is_run_try_match(body) |
            ExprKind::For { .. }
        )
    }

    pub fn format(loop_expr: &LoopExpr, shape: Shape, f: &mut Formatter) {
        let body = f.arena.get_expr(loop_expr.body);

        if Self::has_complex_body(body) {
            // Complex: break after loop(
            f.emit("loop(");
            f.newline();
            f.indent();
            f.format(loop_expr.body);
            f.dedent();
            f.newline();
            f.emit(")");
        } else if shape.fits_inline(loop_expr) {
            // Simple and fits: inline
            f.emit("loop(");
            f.format(loop_expr.body);
            f.emit(")");
        } else {
            // Simple but doesn't fit: break
            f.emit("loop(");
            f.newline();
            f.indent();
            f.format(loop_expr.body);
            f.dedent();
            f.newline();
            f.emit(")");
        }
    }
}
```

---

## Layer 5: Formatter Orchestration

### 5.1 General Rules (Spec lines 17-21)

```rust
impl Formatter {
    // Line 18: 4 spaces indentation
    const INDENT: &'static str = "    ";

    // Line 20: Trailing commas required in multi-line, forbidden in single-line
    fn emit_trailing_comma(&mut self, is_multiline: bool) {
        if is_multiline {
            self.emit(",");
        }
    }

    // Line 21: No consecutive, leading, or trailing blank lines
    fn normalize_blank_lines(&mut self) {
        // Post-process to remove consecutive blank lines
    }
}
```

### 5.2 Blank Lines (Spec lines 49-56)

```rust
impl Formatter {
    /// One blank line between top-level declarations
    fn format_module(&mut self, module: &Module) {
        self.format_imports(&module.imports);
        self.blank_line();  // After imports block

        self.format_constants(&module.constants);
        self.blank_line();  // After constants block

        for (i, decl) in module.declarations.iter().enumerate() {
            if i > 0 {
                self.blank_line();  // Between declarations
            }
            self.format_declaration(decl);
        }
    }

    /// One blank line between trait/impl methods (except single-method)
    fn format_impl_methods(&mut self, methods: &[Method]) {
        if methods.len() == 1 {
            self.format_method(&methods[0]);
        } else {
            for (i, method) in methods.iter().enumerate() {
                if i > 0 {
                    self.blank_line();
                }
                self.format_method(method);
            }
        }
    }
}
```

### 5.3 Import Ordering (Spec lines 848-877)

```rust
impl Formatter {
    /// Stdlib first, relative second, blank line between. Sorted alphabetically.
    fn format_imports(&mut self, imports: &[Import]) {
        let (stdlib, relative): (Vec<_>, Vec<_>) =
            imports.iter().partition(|i| i.is_stdlib());

        let mut stdlib: Vec<_> = stdlib;
        let mut relative: Vec<_> = relative;

        stdlib.sort_by(|a, b| a.path.cmp(&b.path));
        relative.sort_by(|a, b| a.path.cmp(&b.path));

        for import in &stdlib {
            self.format_import(import);
            self.newline();
        }

        if !stdlib.is_empty() && !relative.is_empty() {
            self.blank_line();
        }

        for import in &relative {
            self.format_import(import);
            self.newline();
        }
    }
}
```

---

## Summary: Where Each Spec Section Maps

| Spec Section | Lines | Layer |
|--------------|-------|-------|
| **Spacing table** | 25-47 | Layer 1 (Token Rules) |
| **Width-based breaking** | 58-76 | Layer 2 (Packing) |
| **Always-stacked** | 78-90 | Layer 2 (Packing) |
| **Independent breaking** | 93-95 | Layer 3 (Shape) |
| **Function signatures** | 99-132 | Layer 2 + 3 |
| **Function calls** | 134-155 | Layer 2 + 3 |
| **Generics/Where/Capabilities** | 157-215 | Layer 2 + 3 |
| **Lists/Maps/Tuples** | 217-283 | Layer 2 (Packing) |
| **Struct literals** | 285-306 | Layer 2 (Packing) |
| **Type definitions** | 308-358 | Layer 2 (Packing) |
| **Trait/Impl blocks** | 360-382 | Layer 5 (Blank lines) |
| **Lambdas** | 384-410 | Layer 2 + 4 |
| **Conditionals** | 412-455 | Layer 4 (ChainedElseIfRule) |
| **Binary expressions** | 457-483 | Layer 4 (BooleanBreakRule) |
| **Method chains** | 485-510 | Layer 4 (MethodChainRule) |
| **run/try** | 512-576 | Layer 2 + 4 (RunRule) |
| **loop** | 578-617 | Layer 4 (LoopRule) |
| **match** | 619-657 | Layer 2 (AlwaysStacked) |
| **recurse/parallel/spawn/nursery** | 659-846 | Layer 2 (AlwaysStacked) |
| **for loops** | 731-830 | Layer 4 (ShortBodyRule, NestedForRule) |
| **Imports** | 848-877 | Layer 5 (Import ordering) |
| **Constants** | 879-889 | Layer 5 (Blank lines) |
| **Comments** | 891-938 | Layer 1 (Token Rules) |
| **Ranges** | 940-949 | Layer 1 (Token Rules) |
| **Destructuring** | 951-960 | Layer 1 (Token Rules) |
| **Strings** | 962-972 | Layer 3 (Never break inside) |
| **Parentheses preservation** | 974-1023 | Layer 4 (needs_parens) |

---

## Observations

### Good Fit ✅

1. **Token spacing** → Layer 1 maps cleanly (22 rules from spec table)
2. **Container packing** → Layer 2 handles most constructs
3. **Width-based breaking** → Layer 3 provides the foundation
4. **Import/blank line rules** → Layer 5 handles module-level concerns

### Requires Special Handling ⚠️

These need **Layer 4 (Ori-specific rules)** because they're more complex than simple packing:

1. **Method chains** - "all methods break together" rule
2. **Short body rule** - changes WHERE we break, not just IF
3. **Long boolean** - break at `||` specifically
4. **Chained else-if** - each clause on own line
5. **Nested for** - Rust-style indentation increment
6. **Parentheses** - context-dependent preservation
7. **Run top-level vs nested** - position-dependent packing
8. **Loop with complex body** - content-dependent breaking

### Total Rule Count

| Layer | Count | Description |
|-------|-------|-------------|
| Layer 1 | ~35 | Token spacing rules |
| Layer 2 | ~18 | Construct → Packing mappings |
| Layer 3 | 2 | Shape + config |
| Layer 4 | 8 | Ori-specific breaking rules |
| Layer 5 | 3 | Module-level orchestration |
| **Total** | **~66** | Discrete formatting decisions |
