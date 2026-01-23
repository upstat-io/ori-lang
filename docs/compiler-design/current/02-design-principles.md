# Design Principles

This document establishes the architectural philosophy for `sigilc-v2`. Every component must adhere to these principles.

---

## SOLID Principles (Compiler-Specific)

### S - Single Responsibility

Each module has exactly one reason to change.

**Good: Separate concerns**
```rust
// intern/strings.rs - Only string interning
pub struct StringInterner { ... }

// intern/types.rs - Only type interning
pub struct TypeInterner { ... }

// syntax/lexer.rs - Only tokenization
pub fn lex(source: &str) -> TokenStream { ... }

// syntax/parser.rs - Only parsing
pub fn parse(tokens: TokenStream) -> Ast { ... }
```

**Bad: Mixed responsibilities**
```rust
// Don't do this
pub struct Compiler {
    pub fn lex_and_parse(&self, source: &str) -> Ast { ... }
    pub fn parse_and_typecheck(&self, source: &str) -> TypedAst { ... }
}
```

**Compiler application:**
- Lexer doesn't know about AST
- Parser doesn't know about types
- Type checker doesn't know about codegen
- Each pass is independently testable

### O - Open/Closed

Extend behavior without modifying existing code.

**Good: Pattern system uses traits**
```rust
// patterns/definition.rs
pub trait PatternDefinition: Send + Sync {
    fn name(&self) -> &'static str;
    fn parse(&self, parser: &mut Parser) -> Result<PatternNode>;
    fn type_check(&self, ctx: &mut TypeContext, node: &PatternNode) -> Result<TypeId>;
    fn evaluate(&self, env: &mut Environment, node: &PatternNode) -> Result<Value>;
    fn codegen(&self, ctx: &mut CodegenContext, node: &PatternNode) -> Result<CCode>;
}

// Adding a new pattern = implement trait, register in registry
// No changes to existing code required
```

**Bad: Switch statements on pattern kind**
```rust
// Don't do this
fn evaluate_pattern(kind: PatternKind, args: &Args) -> Value {
    match kind {
        PatternKind::Map => { ... }
        PatternKind::Filter => { ... }
        // Must modify this function for every new pattern
    }
}
```

**Compiler application:**
- New patterns via trait implementation
- New diagnostics via error code registration
- New optimizations via pass registration

### L - Liskov Substitution

Subtypes must be substitutable for their base types.

**Good: Query trait hierarchy**
```rust
// All queries implement the base trait
pub trait Query {
    type Input;
    type Output;
    fn execute(&self, db: &dyn Db, input: Self::Input) -> Self::Output;
}

// Specialized queries extend without breaking contract
pub trait IncrementalQuery: Query {
    fn is_valid(&self, db: &dyn Db, revision: Revision) -> bool;
}

// Any code expecting `Query` works with `IncrementalQuery`
```

**Compiler application:**
- All AST nodes share common interface (span, kind)
- All types implement equality/hashing
- All passes share common lifecycle

### I - Interface Segregation

Don't force dependencies on unused methods.

**Good: Focused traits**
```rust
// Separate traits for separate concerns
pub trait Parseable {
    fn parse(parser: &mut Parser) -> Result<Self>;
}

pub trait TypeCheckable {
    fn type_check(&self, ctx: &mut TypeContext) -> Result<TypeId>;
}

pub trait Evaluatable {
    fn evaluate(&self, env: &mut Environment) -> Result<Value>;
}

// A component implements only what it needs
impl Parseable for Literal { ... }
impl TypeCheckable for Literal { ... }
impl Evaluatable for Literal { ... }
```

**Bad: God trait**
```rust
// Don't do this
pub trait AstNode {
    fn parse(...);
    fn type_check(...);
    fn evaluate(...);
    fn codegen(...);
    fn format(...);
    fn serialize(...);
    // Every implementor must provide all methods
}
```

**Compiler application:**
- `Spanned` trait - just span access
- `Named` trait - just name access
- `Typed` trait - just type access
- Compose traits as needed

### D - Dependency Inversion

Depend on abstractions, not concretions.

**Good: Database abstraction**
```rust
// Core compiler depends on abstract database
pub trait Db: salsa::Database {
    fn tokens(&self, file: SourceFile) -> TokenList;
    fn parsed_module(&self, file: SourceFile) -> Module;
    fn typed_function(&self, func: Function) -> TypedFunction;
}

// Concrete implementation provided at runtime
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
    interner: StringInterner,
    type_interner: TypeInterner,
}

impl Db for CompilerDb { ... }
```

**Bad: Direct dependencies**
```rust
// Don't do this
pub fn type_check(db: &CompilerDb, file: SourceFile) { ... }
// ^ Tied to specific implementation
```

**Compiler application:**
- All passes depend on `dyn Db`
- Test harness can provide mock database
- LSP can provide incremental database

---

## DRY Principle (Don't Repeat Yourself)

### Centralized Abstractions

**Pattern argument parsing:**
```rust
// One function for all pattern argument parsing
pub fn parse_pattern_args(
    parser: &mut Parser,
    expected: &[ArgSpec],
) -> Result<PatternArgs> {
    // Handles: .name: value syntax
    // Handles: positional fallback (if allowed)
    // Handles: missing required args
    // Handles: unknown args
}

// Every pattern uses this
impl PatternDefinition for MapPattern {
    fn parse(&self, parser: &mut Parser) -> Result<PatternNode> {
        let args = parse_pattern_args(parser, &[
            ArgSpec::required("over"),
            ArgSpec::required("transform"),
        ])?;
        // ...
    }
}
```

**Type error reporting:**
```rust
// One function for type mismatch errors
pub fn type_mismatch(
    span: Span,
    expected: TypeId,
    found: TypeId,
    context: &str,
) -> Diagnostic {
    Diagnostic::error(ErrorCode::E2001)
        .with_message(format!(
            "type mismatch: expected `{}`, found `{}`",
            format_type(expected),
            format_type(found),
        ))
        .with_label(span, context)
}

// Used consistently throughout type checker
```

### Code Generation Templates

**Single template for all collection patterns:**
```rust
// Template for map/filter/collect/etc.
const COLLECTION_PATTERN_TEMPLATE: &str = r#"
    {init_code}
    for ({iter_type} __iter = {iter_init}; {iter_cond}; {iter_step}) {{
        {elem_type} __elem = {iter_deref};
        {transform_code}
        {accumulate_code}
    }}
    {result_code}
"#;

// Each pattern fills in the blanks
fn codegen_map(template: &Template, args: &MapArgs) -> CCode {
    template.fill(&[
        ("init_code", codegen_list_init()),
        ("transform_code", codegen_call(args.transform)),
        ("accumulate_code", "list_push(__result, __transformed);"),
        // ...
    ])
}
```

### What NOT to Centralize

Some duplication is acceptable when:
- Coupling would be worse than duplication
- Code paths will diverge in the future
- Abstractions would obscure intent

**Example: Keep lexer/parser error handling separate**
```rust
// Lexer errors are fundamentally different from parser errors
// Don't force shared abstraction

// lexer/errors.rs
pub enum LexError {
    UnterminatedString(Span),
    InvalidCharacter(char, Span),
    InvalidNumber(Span),
}

// parser/errors.rs
pub enum ParseError {
    UnexpectedToken { expected: TokenKind, found: Token },
    UnclosedDelimiter { open: Span, expected: char },
    InvalidExpression(Span),
}
```

---

## Performance Principles

### Measure Before Optimizing

**Required for any optimization:**
1. Benchmark showing current performance
2. Profile showing hotspot location
3. After-optimization benchmark showing improvement
4. Verification that behavior unchanged

```rust
// Good: Documented optimization with benchmark
// Before: 150ms for 10K functions (profile: 40% in HashMap lookup)
// After: 45ms for 10K functions (interned names)
// Benchmark: benches/type_check.rs::bench_10k_functions
```

### Allocation is the Enemy

**Allocation hierarchy (fastest to slowest):**
1. Stack allocation (free)
2. Arena allocation (bulk free)
3. Interned allocation (deduplicated)
4. Individual heap allocation (avoid)

```rust
// Prefer stack
let small_array: [ExprId; 4] = [a, b, c, d];

// Prefer arena for AST
let expr = arena.alloc(Expr { kind, span });

// Prefer interning for repeated data
let name = interner.intern("function_name");

// Avoid individual heap allocation
let boxed = Box::new(expr);  // Only if necessary
```

### Cache Locality Matters

**Good: Flat data structures**
```rust
pub struct ExprArena {
    exprs: Vec<Expr>,        // Contiguous memory
    args: Vec<ExprId>,       // Contiguous memory
}

// Iteration touches sequential memory addresses
for expr in &arena.exprs {
    process(expr);  // Cache-friendly
}
```

**Bad: Pointer chasing**
```rust
pub struct Expr {
    left: Box<Expr>,   // Pointer jump
    right: Box<Expr>,  // Another pointer jump
}

// Iteration jumps around memory
fn visit(expr: &Expr) {
    visit(&expr.left);   // Cache miss likely
    visit(&expr.right);  // Cache miss likely
}
```

### Parallelize at the Right Granularity

**Too fine-grained:**
```rust
// Bad: Overhead exceeds benefit
exprs.par_iter().map(|e| e.span()).collect()  // Don't parallelize trivial ops
```

**Too coarse-grained:**
```rust
// Bad: Single thread does all work
let result = huge_module.type_check();  // Should split into functions
```

**Just right:**
```rust
// Good: File-level parallelism for parsing
files.par_iter().map(|f| parse(f)).collect()

// Good: Function-level parallelism for type checking
functions.par_iter().map(|f| type_check(f)).collect()
```

---

## Error Handling Principles

### Fail Fast, Recover Gracefully

**Lexer: Fail fast on invalid input**
```rust
fn lex_number(&mut self) -> Token {
    // Stop at first invalid character
    while self.peek().is_ascii_digit() {
        self.advance();
    }
    // Don't try to "fix" malformed numbers
}
```

**Parser: Recover to report multiple errors**
```rust
fn parse_function(&mut self) -> Option<Function> {
    let name = self.expect(TokenKind::At)?;
    let name = match self.expect_ident() {
        Some(n) => n,
        None => {
            self.error("expected function name after '@'");
            self.recover_to(&[TokenKind::Eq, TokenKind::LParen]);
            return None;  // Skip this function, continue parsing
        }
    };
    // ...
}
```

**Type checker: Accumulate all errors**
```rust
fn type_check_module(&mut self, module: &Module) -> TypeCheckResult {
    let mut errors = Vec::new();

    for func in &module.functions {
        if let Err(e) = self.type_check_function(func) {
            errors.push(e);
            // Continue checking other functions
        }
    }

    if errors.is_empty() {
        Ok(typed_module)
    } else {
        Err(errors)
    }
}
```

### Error Messages Are User Interface

**Required elements:**
1. Error code (for searchability)
2. Clear message (what went wrong)
3. Primary span (where it went wrong)
4. Context (why it's wrong)
5. Suggestions (how to fix)

```rust
Diagnostic::error(ErrorCode::E2001)
    .with_message("type mismatch in function return")
    .with_label(
        return_span,
        format!("expected `{}`, found `{}`", expected, found)
    )
    .with_note(format!(
        "function `{}` declares return type `{}`",
        func_name, expected
    ))
    .with_suggestion(
        "consider changing the return type or the return value"
    )
```

---

## Testing Principles

### Test at Every Level

```
Unit tests:     Interner, individual patterns, type unification
Integration:    Full compilation of code snippets
Compatibility:  V1/V2 output comparison
Performance:    Criterion benchmarks with baselines
```

### Incrementality Tests Are Critical

```rust
#[test]
fn test_incremental_single_edit() {
    let mut db = setup_project();

    // Initial compile
    let result1 = db.compile();
    let query_count1 = db.query_count();

    // Edit one file
    db.edit_file("src/utils.si", add_comment);

    // Recompile
    let result2 = db.compile();
    let query_count2 = db.query_count();

    // Should reuse most queries
    assert!(query_count2 < query_count1 / 10);
    assert_eq!(result1.output, result2.output);
}
```

### Property-Based Testing for Parser

```rust
#[test]
fn parse_roundtrip() {
    // Any valid AST should parse back to itself
    proptest!(|(ast: Ast)| {
        let source = format(&ast);
        let reparsed = parse(&source)?;
        assert_eq!(ast, reparsed);
    });
}
```

---

## Documentation Principles

### Code Should Be Self-Documenting

**Good: Types express intent**
```rust
// The types tell the story
pub fn type_check(
    db: &dyn Db,
    module: Module,
) -> Result<TypedModule, Vec<Diagnostic>> { ... }
```

**Bad: Comments repeat code**
```rust
// Don't do this
/// Type checks a module and returns a typed module or diagnostics
pub fn type_check(module: Module) -> Result<TypedModule, Vec<Diagnostic>>
```

### Document the Why, Not the What

**Good: Explains non-obvious decisions**
```rust
// Use sharded interner to reduce lock contention during parallel parsing.
// Benchmarks show 3x improvement with 16 shards on 8-core machine.
// See: appendices/G-benchmarks.md#string-interning
pub struct ShardedInterner<const SHARDS: usize> { ... }
```

**Bad: States the obvious**
```rust
// This is an interner with shards
pub struct ShardedInterner { ... }
```

### Keep Specs and Implementation in Sync

When implementation diverges from this documentation:
1. Update the documentation, OR
2. File an issue explaining the deviation

Never let documentation become stale.
