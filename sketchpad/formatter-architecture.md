# Ori Formatter Architecture Sketch

## Hybrid Approach: Rules + Packing + Shape

Combines the best patterns from TypeScript, Gleam, Zig, and Rust.

---

## Layer 1: Token Spacing Rules (TypeScript-style)

Declarative rules for spacing between tokens. O(1) lookup via `(left_token, right_token)` map.

```rust
/// What action to take between two tokens
#[derive(Clone, Copy)]
pub enum SpaceAction {
    None,           // No space
    Space,          // Single space
    Newline,        // Line break
    Preserve,       // Keep source spacing
}

/// A spacing rule
pub struct SpaceRule {
    pub name: &'static str,
    pub left: TokenMatcher,
    pub right: TokenMatcher,
    pub context: Option<fn(&FormattingContext) -> bool>,
    pub action: SpaceAction,
}

/// Matches tokens for rules
pub enum TokenMatcher {
    Any,
    Exact(TokenKind),
    OneOf(&'static [TokenKind]),
    Not(Box<TokenMatcher>),
}

// Example rules
const SPACE_RULES: &[SpaceRule] = &[
    // Space around binary operators
    SpaceRule::new("SpaceAroundBinOp",
        TokenMatcher::Any,
        TokenMatcher::OneOf(&[Plus, Minus, Star, Slash, Eq, Lt, Gt]),
        None,
        SpaceAction::Space),

    // No space inside parens
    SpaceRule::new("NoSpaceAfterLParen",
        TokenMatcher::Exact(LParen),
        TokenMatcher::Any,
        None,
        SpaceAction::None),

    // Space after colon in type annotations
    SpaceRule::new("SpaceAfterColon",
        TokenMatcher::Exact(Colon),
        TokenMatcher::Any,
        Some(|ctx| ctx.is_type_annotation()),
        SpaceAction::Space),

    // Space inside struct braces
    SpaceRule::new("SpaceInsideStructBraces",
        TokenMatcher::Exact(LBrace),
        TokenMatcher::Any,
        Some(|ctx| ctx.is_struct_literal()),
        SpaceAction::Space),
];

/// Pre-computed rule lookup table
pub struct RulesMap {
    // (left_kind, right_kind) -> applicable rules
    buckets: HashMap<(TokenKind, TokenKind), Vec<&'static SpaceRule>>,
}

impl RulesMap {
    pub fn lookup(&self, left: TokenKind, right: TokenKind) -> &[&SpaceRule] {
        self.buckets.get(&(left, right)).map(|v| v.as_slice()).unwrap_or(&[])
    }
}
```

---

## Layer 2: Container Packing (Gleam-style)

Decision tree for how to format lists, args, fields, etc.

```rust
/// How to pack items in a container
#[derive(Clone, Copy, Debug)]
pub enum Packing {
    /// Try single line; if doesn't fit, one item per line
    FitOrOnePerLine,

    /// Try single line; if doesn't fit, pack multiple per line
    FitOrPackMultiple,

    /// Always one item per line (trailing comma present, or rule says so)
    AlwaysOnePerLine,

    /// Always stacked with specific formatting (run, try, match, etc.)
    AlwaysStacked,
}

/// What separator to use between items
#[derive(Clone, Copy)]
pub enum Separator {
    /// ", " inline, ",\n" when broken
    Comma,
    /// " " inline, "\n" when broken
    Space,
    /// Always newline
    Newline,
}

/// Determines packing for a container
pub fn determine_packing(
    construct: ConstructKind,
    has_trailing_comma: bool,
    has_comments: bool,
    has_empty_lines: bool,
    item_count: usize,
) -> Packing {
    use ConstructKind::*;
    use Packing::*;

    // Always-stacked constructs (from spec)
    if matches!(construct,
        RunTopLevel | Try | Match | Recurse | Parallel | Spawn | Nursery
    ) {
        return AlwaysStacked;
    }

    // Empty lines between items → preserve vertical spacing
    if has_empty_lines {
        return AlwaysOnePerLine;
    }

    // Trailing comma signals user intent to break
    if has_trailing_comma {
        return AlwaysOnePerLine;
    }

    // Comments force breaking
    if has_comments {
        return AlwaysOnePerLine;
    }

    // Simple items can pack multiple per line (like number lists)
    if matches!(construct, List | Tuple) && item_count > 5 {
        return FitOrPackMultiple;
    }

    // Default: try inline, else one per line
    FitOrOnePerLine
}

/// Construct kinds for packing decisions
pub enum ConstructKind {
    // Always stacked
    RunTopLevel,
    RunNested,
    Try,
    Match,
    Recurse,
    Parallel,
    Spawn,
    Nursery,

    // Width-based
    FunctionParams,
    FunctionArgs,
    GenericParams,
    List,
    Map,
    Tuple,
    StructFields,
    StructLiteral,
    SumVariants,
    ImportItems,
    WhereConstraints,
}
```

---

## Layer 3: Shape Tracking (Rust-style, simplified)

Track available width as we descend into nested structures.

```rust
/// Available formatting space
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    /// Characters remaining on current line
    pub width: usize,

    /// Current indentation level (in spaces)
    pub indent: usize,

    /// Position on first line (for alignment)
    pub offset: usize,
}

impl Shape {
    pub fn new(max_width: usize) -> Self {
        Shape { width: max_width, indent: 0, offset: 0 }
    }

    /// Reduce width by n characters (for content already emitted)
    pub fn consume(self, n: usize) -> Self {
        Shape { width: self.width.saturating_sub(n), ..self }
    }

    /// Add indentation for nested block
    pub fn indent(self, spaces: usize) -> Self {
        Shape {
            indent: self.indent + spaces,
            width: self.width.saturating_sub(spaces),
            ..self
        }
    }

    /// Check if content fits in remaining width
    pub fn fits(&self, content_width: usize) -> bool {
        content_width <= self.width
    }

    /// Get shape for next line (reset to indent)
    pub fn next_line(self, max_width: usize) -> Self {
        Shape {
            width: max_width.saturating_sub(self.indent),
            offset: self.indent,
            ..self
        }
    }
}
```

---

## Layer 4: Breaking Rules (Ori-specific)

Specific rules for Ori constructs based on our formatting decisions.

```rust
/// Rules for how constructs break
pub struct BreakingRules;

impl BreakingRules {
    /// Method chains: receiver stays, all methods break
    pub const METHOD_CHAIN_ALL_BREAK: bool = true;

    /// Short body threshold (stays with yield/do)
    pub const SHORT_BODY_THRESHOLD: usize = 20;

    /// Long boolean: break at || when 3+ clauses
    pub const BOOLEAN_BREAK_AT_OR: bool = true;
    pub const BOOLEAN_OR_THRESHOLD: usize = 3;

    /// Nested for: Rust-style indentation when breaking
    pub const NESTED_FOR_RUST_STYLE: bool = true;

    /// Chained else-if: each clause on own line
    pub const ELSE_IF_EACH_LINE: bool = true;
}

/// What needs parentheses to preserve semantics
pub fn needs_parens(expr: &Expr, position: ParenPosition) -> bool {
    use ExprKind::*;
    use ParenPosition::*;

    match position {
        // Method receiver: (for x in items yield x).method()
        Receiver => matches!(expr.kind,
            Binary { .. } | Unary { .. } | If { .. } | Lambda { .. } |
            Let { .. } | Range { .. } | For { .. } | Loop { .. }
        ),

        // Call target: (x -> x * 2)(5)
        CallTarget => matches!(expr.kind,
            Binary { .. } | Unary { .. } | If { .. } | Lambda { .. } |
            Let { .. } | Range { .. } | For { .. } | Loop { .. }
        ),

        // Iterator source: for x in (for y in items yield y)
        IteratorSource => matches!(expr.kind,
            For { .. } | If { .. } | Lambda { .. } | Let { .. }
        ),
    }
}

pub enum ParenPosition {
    Receiver,
    CallTarget,
    IteratorSource,
}
```

---

## Layer 5: Main Formatter

Orchestrates all layers.

```rust
pub struct Formatter<'a> {
    arena: &'a ExprArena,
    rules_map: &'a RulesMap,
    config: FormatterConfig,

    // Output
    output: String,

    // State
    shape: Shape,
    indent_stack: Vec<usize>,
}

pub struct FormatterConfig {
    pub max_width: usize,        // Default: 100
    pub indent_size: usize,      // Default: 4
    pub trailing_commas: bool,   // Preserve or normalize
}

impl<'a> Formatter<'a> {
    pub fn format(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        // Try inline first
        if let Some(inline) = self.try_inline(expr_id) {
            if self.shape.fits(inline.len()) {
                self.emit(&inline);
                return;
            }
        }

        // Fall back to broken format
        self.format_broken(expr_id);
    }

    fn try_inline(&self, expr_id: ExprId) -> Option<String> {
        // Render to string without emitting
        // Return None if construct is always-stacked
        let expr = self.arena.get_expr(expr_id);

        // Check if always-stacked
        if self.is_always_stacked(expr) {
            return None;
        }

        // Try to render inline
        let mut inline_formatter = self.clone_for_inline();
        inline_formatter.emit_inline(expr_id);
        Some(inline_formatter.output)
    }

    fn is_always_stacked(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Call { func, .. } => {
                // Check if it's run/try/match/etc at top level
                if let Some(name) = self.get_builtin_name(func) {
                    matches!(name, "run" | "try" | "match" | "recurse" |
                                   "parallel" | "spawn" | "nursery")
                        && self.is_top_level()
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn format_broken(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);

        match &expr.kind {
            ExprKind::For { .. } => self.format_for(expr_id),
            ExprKind::Call { .. } => self.format_call(expr_id),
            ExprKind::MethodCall { .. } => self.format_method_chain(expr_id),
            ExprKind::If { .. } => self.format_if(expr_id),
            ExprKind::Binary { .. } => self.format_binary(expr_id),
            // ... etc
            _ => self.format_default(expr_id),
        }
    }

    fn format_method_chain(&mut self, expr_id: ExprId) {
        // Collect chain
        let chain = self.collect_method_chain(expr_id);

        // Emit receiver
        self.format(chain.receiver);

        // Emit each method on its own line (per our rule)
        for method in &chain.methods {
            self.newline();
            self.emit(".");
            self.emit(&method.name);
            self.emit("(");
            self.format_args(&method.args);
            self.emit(")");
        }
    }

    fn format_for(&mut self, expr_id: ExprId) {
        let expr = self.arena.get_expr(expr_id);
        if let ExprKind::For { pattern, iter, body, mode, .. } = &expr.kind {
            // Emit "for pattern in "
            self.emit("for ");
            self.format_pattern(pattern);
            self.emit(" in ");

            // Emit iterator (with parens if needed)
            if needs_parens(self.arena.get_expr(*iter), ParenPosition::IteratorSource) {
                self.emit("(");
                self.format(*iter);
                self.emit(")");
            } else {
                self.format(*iter);
            }

            // Emit mode (do/yield)
            let mode_str = match mode {
                ForMode::Do => " do",
                ForMode::Yield => " yield",
            };
            self.emit(mode_str);

            // Check if body is simple (short body rule)
            let body_expr = self.arena.get_expr(*body);
            if self.is_simple_body(body_expr) {
                self.emit(" ");
                self.format(*body);
            } else {
                self.newline();
                self.indent();
                self.format(*body);
                self.dedent();
            }
        }
    }

    fn is_simple_body(&self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ident { .. } => true,
            ExprKind::Literal { .. } => true,
            _ => {
                // Check inline length
                if let Some(inline) = self.try_inline_expr(expr) {
                    inline.len() <= BreakingRules::SHORT_BODY_THRESHOLD
                } else {
                    false
                }
            }
        }
    }
}
```

---

## Data Flow

```
Input: ExprArena + source positions
           │
           ▼
    ┌──────────────┐
    │ SpaceRules   │◄── Layer 1: Token spacing (O(1) lookup)
    │ RulesMap     │
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Packing      │◄── Layer 2: Container decisions
    │ determine_   │    (trailing comma, comments, construct type)
    │ packing()    │
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Shape        │◄── Layer 3: Width tracking
    │ (width,      │    (flows through recursion)
    │  indent)     │
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Breaking     │◄── Layer 4: Ori-specific rules
    │ Rules        │    (method chains, short body, etc.)
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │ Formatter    │◄── Layer 5: Orchestration
    │ try_inline() │    (try inline → fall back to broken)
    │ format_*()   │
    └──────────────┘
           │
           ▼
      Output: String
```

---

## Goal: Complex Example with All Rules Applied

This is the target output when the formatter is complete. It demonstrates all 8 Ori-specific rules working together:

```ori
@process_data (
    users: [User],
    config: Config,
) -> Result<[ProcessedUser], Error> uses Http, Logger = run(
    let active_users = for user in users yield if user.active && user.verified
        || user.is_admin
        || user.bypass_check then user
        else continue,
    let results = for user in active_users yield
        for permission in user.permissions yield
            run(
                let validated = (x -> x.validate())(user),
                let transformed = validated
                    .transform()
                    .normalize()
                    .sanitize(),
                let logged = run(print(msg: transformed.to_str()), transformed),
                logged,
            ),
    let final = (for r in results yield r)
        .flatten()
        .filter(x -> x.is_valid())
        .map(x -> x.finalize())
        .collect(),
    Ok(final),
)
```

### Rules Demonstrated

| Rule | Where It's Applied |
|------|-------------------|
| **1. MethodChainRule** | `validated.transform().normalize().sanitize()` — all methods break |
| **2. ShortBodyRule** | `yield user` stays with yield (under 20 chars) |
| **3. BooleanBreakRule** | 3 `\|\|` clauses break with leading `\|\|` |
| **4. ChainedElseIfRule** | `if ... then user` / `else continue` — Kotlin style |
| **5. NestedForRule** | `for user ... yield` / `for permission ... yield` — Rust-style indent |
| **6. ParenthesesRule** | `(x -> x.validate())(user)` and `(for r in results yield r)` preserved |
| **7. RunRule** | Top-level `run(` stacked; nested `run(print(...), transformed)` inline |
| **8. LoopRule** | (Not shown — would break if body contained run/try/match/for) |

---

## Benefits of This Architecture

1. **Declarative spacing rules** - Easy to add/modify token spacing
2. **Packing enum** - Clear decision tree for containers
3. **Shape tracking** - Handles nesting naturally
4. **Ori-specific rules** - Captures our formatting decisions
5. **Try-then-fallback** - Simple width-based breaking
6. **Testable layers** - Each layer can be tested independently

## Migration Path

1. Extract current rules into `SpaceRule` declarations
2. Add `Packing` enum and `determine_packing()` function
3. Refactor formatter to use `Shape` instead of ad-hoc width tracking
4. Consolidate Ori-specific rules into `BreakingRules`
5. Restructure main formatter to use try-inline-then-break pattern
