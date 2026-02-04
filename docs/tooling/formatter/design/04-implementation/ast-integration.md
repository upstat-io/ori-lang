---
title: "AST Integration"
description: "Ori Formatter Design — Working with the Ori AST"
order: 2
section: "Implementation"
---

# AST Integration

This document describes how the formatter integrates with the Ori compiler's AST.

## AST Structure

The Ori compiler uses a flat AST with arena allocation. Key types:

| Type | Purpose |
|------|---------|
| `ExprArena` | Stores all expressions |
| `ExprId` | Handle to an expression in the arena |
| `Module` | Top-level container for a file |
| `Item` | Top-level declarations |
| `Expr` | Expression variants |

## Relevant Crates

| Crate | Purpose | Formatter Usage |
|-------|---------|-----------------|
| `ori_ir` | AST types, spans | Read AST structure |
| `ori_lexer` | Tokens | Access source text for literals |
| `ori_parse` | Parser | Parse source before formatting |

## Expression Types

The formatter handles each `Expr` variant:

```rust
pub enum Expr {
    // Literals
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Char(char),

    // Identifiers
    Identifier(Name),

    // Collections
    List(Vec<ExprId>),
    Map(Vec<(ExprId, ExprId)>),
    Tuple(Vec<ExprId>),
    Struct { name: Name, fields: Vec<Field> },

    // Operations
    Binary { left: ExprId, op: BinaryOp, right: ExprId },
    Unary { op: UnaryOp, operand: ExprId },
    Call { func: ExprId, args: Vec<Argument> },
    MethodCall { receiver: ExprId, method: Name, args: Vec<Argument> },
    FieldAccess { receiver: ExprId, field: Name },
    Index { receiver: ExprId, index: ExprId },

    // Control flow
    If { condition: ExprId, then_branch: ExprId, else_branch: Option<ExprId> },
    Match { scrutinee: ExprId, arms: Vec<MatchArm> },

    // Bindings
    Let { pattern: Pattern, value: ExprId },
    Lambda { params: Vec<Param>, body: ExprId },

    // Patterns
    Run { steps: Vec<ExprId>, pre_check: Option<ExprId>, post_check: Option<ExprId> },
    Try { steps: Vec<ExprId> },
    Recurse { condition: ExprId, base: ExprId, step: ExprId, memo: bool, parallel: Option<ExprId> },
    Parallel { tasks: ExprId, max_concurrent: Option<ExprId>, timeout: Option<ExprId> },
    // ... other patterns
}
```

## Item Types

Top-level declarations:

```rust
pub enum Item {
    Function(Function),
    Type(TypeDef),
    Trait(TraitDef),
    Impl(ImplBlock),
    Const(ConstDef),
    Use(UseStatement),
}

pub struct Function {
    pub visibility: Visibility,
    pub name: Name,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub where_clause: Option<WhereClause>,
    pub capabilities: Vec<Capability>,
    pub body: ExprId,
    pub tests: Vec<Name>,  // For test functions
}
```

## Accessing Source Text

For literals and identifiers, the formatter may need original source text:

```rust
impl Formatter {
    fn format_string_literal(&mut self, expr_id: ExprId) {
        let span = self.arena.span(expr_id);
        let original = &self.source[span.start..span.end];
        self.emit(original);  // Preserve original escaping, quotes, etc.
    }
}
```

## Span Information

Spans track source locations. The formatter uses spans to:
- Preserve original literal text
- Associate comments with nodes
- Report formatting errors

```rust
pub struct Span {
    pub start: usize,  // Byte offset
    pub end: usize,    // Byte offset
}

impl ExprArena {
    pub fn span(&self, id: ExprId) -> Span;
}
```

## Name Interning

Identifiers are interned for efficiency:

```rust
pub struct Name {
    index: u32,
}

impl Interner {
    pub fn resolve(&self, name: Name) -> &str;
}
```

The formatter resolves names to strings for output:

```rust
fn format_identifier(&mut self, name: Name) {
    let text = self.interner.resolve(name);
    self.emit(text);
}
```

## Creating the Formatter

The formatter uses a generic parameter `I: StringLookup` for name resolution, enabling both the standard `StringInterner` and test mocks:

```rust
pub struct Formatter<'a, I: StringLookup> {
    arena: &'a ExprArena,
    interner: &'a I,
    width_calc: WidthCalculator<'a, I>,
    pub(crate) ctx: FormatContext<StringEmitter>,
}

impl<'a, I: StringLookup> Formatter<'a, I> {
    /// Create a new formatter with default config.
    pub fn new(arena: &'a ExprArena, interner: &'a I) -> Self {
        Self::with_config(arena, interner, FormatConfig::default())
    }

    /// Create a new formatter with custom config.
    pub fn with_config(arena: &'a ExprArena, interner: &'a I, config: FormatConfig) -> Self {
        let width_calc = WidthCalculator::new(arena, interner);
        let ctx = FormatContext::new(StringEmitter::new(), config);
        Self { arena, interner, width_calc, ctx }
    }

    /// Format a module and return the output string.
    pub fn format_module(&mut self, module: &Module) -> String {
        ModuleFormatter::new(self).format(module);
        self.ctx.emitter.take()
    }
}
```

**Key design decisions:**

| Field | Purpose |
|-------|---------|
| `arena` | Read-only access to the flat AST |
| `interner` | Resolve interned `Name` values to strings |
| `width_calc` | Bottom-up width calculation with LRU caching |
| `ctx` | Column/indent tracking via `FormatContext<StringEmitter>` |

**Note:** The formatter does NOT store source text or comments directly. Comments are handled separately during module formatting via the parser's comment output.
```

## Integration with Salsa

The formatter can be a Salsa query for incremental formatting:

```rust
#[salsa::query_group(FormatterDatabase)]
pub trait FormatterDb: ParserDb {
    fn formatted(&self, file: SourceFile) -> String;
}

fn formatted(db: &dyn FormatterDb, file: SourceFile) -> String {
    let parsed = db.parsed(file);
    let mut formatter = Formatter::new(
        &parsed.arena,
        db.interner(),
        db.source(file),
        parsed.comments.clone(),
    );
    formatter.format_module(&parsed.module)
}
```

## Handling Parse Errors

The AST may contain error nodes for invalid syntax:

```rust
pub enum Expr {
    // ... valid variants ...
    Error(Span),  // Represents unparseable region
}

impl Formatter {
    fn format_expr(&mut self, id: ExprId) {
        match self.arena.get(id) {
            Expr::Error(span) => {
                // Preserve original text
                self.emit(&self.source[span.start..span.end]);
            }
            // ... handle valid expressions
        }
    }
}
```

## Module Structure

The formatter processes modules in order:

```rust
fn format_module(&mut self, module: &Module) {
    // 1. Format imports (sorted)
    self.format_imports(&module.uses);

    // 2. Blank line after imports
    if !module.uses.is_empty() {
        self.emit_newline();
    }

    // 3. Format constants
    self.format_constants(&module.constants);

    // 4. Blank line after constants
    if !module.constants.is_empty() {
        self.emit_newline();
    }

    // 5. Format types
    for type_def in &module.types {
        self.format_type_def(type_def);
        self.emit_newline();
    }

    // 6. Format traits
    for trait_def in &module.traits {
        self.format_trait(trait_def);
        self.emit_newline();
    }

    // 7. Format impls
    for impl_block in &module.impls {
        self.format_impl(impl_block);
        self.emit_newline();
    }

    // 8. Format functions
    for (i, func) in module.functions.iter().enumerate() {
        if i > 0 {
            self.emit_newline();
        }
        self.format_function(func);
    }
}
```

## CLI Integration

```rust
pub fn format_file(path: &Path) -> Result<(), FormatError> {
    let source = std::fs::read_to_string(path)?;
    let parsed = parse(&source)?;

    let mut formatter = Formatter::new(
        &parsed.arena,
        &parsed.interner,
        &source,
        parsed.comments,
    );

    let formatted = formatter.format_module(&parsed.module);

    if formatted != source {
        std::fs::write(path, &formatted)?;
    }

    Ok(())
}
```
