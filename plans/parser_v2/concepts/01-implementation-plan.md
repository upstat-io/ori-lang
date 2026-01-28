# Parser v2: Implementation Plan

## Phase Overview

The implementation is divided into 4 phases, each building on the previous:

```
Phase 1: Foundation     Phase 2: Core Parsing    Phase 3: Error UX      Phase 4: Optimization
─────────────────────   ─────────────────────    ─────────────────────  ─────────────────────
• SoA storage           • Pratt expressions      • Expected tokens      • Branch hints
• Progress tracking     • Context bitflags       • Contextual hints     • Lookahead cache
• Snapshot system       • Indent combinators     • Recovery sync        • Benchmarks
• Formal grammar        • Error types            • Error rendering      • Profiling
```

## Directory Structure

### Proposed Layout

```
compiler/ori_parse_v2/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Public API, re-exports
│   │
│   ├── storage/               # Memory management
│   │   ├── mod.rs
│   │   ├── node.rs            # NodeStorage (SoA)
│   │   ├── extra.rs           # Extra data array
│   │   └── arena.rs           # Bump allocation
│   │
│   ├── state/                 # Parser state
│   │   ├── mod.rs
│   │   ├── cursor.rs          # Token cursor with lookahead
│   │   ├── snapshot.rs        # Snapshot/restore
│   │   ├── context.rs         # ParseContext bitflags
│   │   └── indent.rs          # Indentation tracking
│   │
│   ├── error/                 # Error handling
│   │   ├── mod.rs
│   │   ├── types.rs           # Error enum hierarchy
│   │   ├── expected.rs        # Expected token accumulation
│   │   ├── recovery.rs        # Sync sets, recovery
│   │   └── render.rs          # Error message formatting
│   │
│   ├── combinators/           # Parser combinators
│   │   ├── mod.rs
│   │   ├── core.rs            # one_of!, speculate, etc.
│   │   ├── sequence.rs        # and, skip_first, skip_second
│   │   ├── repetition.rs      # zero_or_more, sep_by
│   │   └── indent.rs          # indented_seq, with_min_indent
│   │
│   ├── grammar/               # Grammar rules
│   │   ├── mod.rs
│   │   ├── module.rs          # Module, imports
│   │   ├── item/              # Top-level items
│   │   │   ├── mod.rs
│   │   │   ├── function.rs    # Functions, tests
│   │   │   ├── type_decl.rs   # Type declarations
│   │   │   ├── trait_def.rs   # Traits
│   │   │   ├── impl_def.rs    # Impl blocks
│   │   │   └── config.rs      # Config variables
│   │   │
│   │   ├── expr/              # Expressions
│   │   │   ├── mod.rs
│   │   │   ├── pratt.rs       # Pratt parser (binary ops)
│   │   │   ├── primary.rs     # Literals, identifiers
│   │   │   ├── postfix.rs     # Calls, field access
│   │   │   ├── control.rs     # if, match, for, loop
│   │   │   └── pattern.rs     # run, try, match patterns
│   │   │
│   │   ├── pattern/           # Pattern matching
│   │   │   ├── mod.rs
│   │   │   ├── literal.rs     # Literal patterns
│   │   │   ├── binding.rs     # Binding patterns
│   │   │   ├── struct_.rs     # Struct patterns
│   │   │   └── list.rs        # List patterns
│   │   │
│   │   ├── ty/                # Types
│   │   │   ├── mod.rs
│   │   │   ├── primitive.rs   # int, str, bool, etc.
│   │   │   ├── composite.rs   # [T], {K: V}, (T, U)
│   │   │   ├── function.rs    # (T) -> U
│   │   │   └── generic.rs     # T, T: Bound
│   │   │
│   │   └── attr.rs            # Attributes
│   │
│   └── tests/                 # Unit tests
│       ├── storage_tests.rs
│       ├── expr_tests.rs
│       ├── pattern_tests.rs
│       └── error_tests.rs
│
├── grammar.md                 # Formal BNF grammar
└── benches/
    └── parse_bench.rs         # Criterion benchmarks
```

## Phase 1: Foundation

### Goals
- Establish memory-efficient node storage
- Implement progress-aware result type
- Create snapshot/restore mechanism
- Document formal grammar

### Deliverables

#### 1.1 Node Storage (`storage/node.rs`)

```rust
// SoA layout for cache efficiency
pub struct NodeStorage {
    tags: Vec<NodeTag>,
    spans: Vec<Span>,
    data: Vec<NodeData>,
    extra: Vec<u32>,
}

impl NodeStorage {
    pub fn with_estimated_capacity(source_len: usize) -> Self {
        let est_tokens = source_len / 8;
        let est_nodes = est_tokens / 2;
        Self {
            tags: Vec::with_capacity(est_nodes),
            spans: Vec::with_capacity(est_nodes),
            data: Vec::with_capacity(est_nodes),
            extra: Vec::with_capacity(est_nodes / 4),
        }
    }

    pub fn alloc(&mut self, tag: NodeTag, span: Span, data: NodeData) -> NodeId {
        let id = NodeId(self.tags.len() as u32);
        self.tags.push(tag);
        self.spans.push(span);
        self.data.push(data);
        id
    }
}
```

#### 1.2 Progress Tracking (`state/mod.rs`)

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    Made,
    None,
}

pub struct ParseResult<T, E = ParseError> {
    pub progress: Progress,
    pub result: Result<T, E>,
}

impl<T, E> ParseResult<T, E> {
    pub fn ok(progress: Progress, value: T) -> Self {
        Self { progress, result: Ok(value) }
    }

    pub fn err(progress: Progress, error: E) -> Self {
        Self { progress, result: Err(error) }
    }

    pub fn made_progress(&self) -> bool {
        self.progress == Progress::Made
    }
}
```

#### 1.3 Snapshot System (`state/snapshot.rs`)

```rust
#[derive(Clone)]
pub struct Snapshot {
    token_index: u32,
    node_count: u32,
    extra_count: u32,
    error_count: u32,
    context: ParseContext,
    min_indent: u16,
}

impl Parser<'_> {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            token_index: self.cursor.index(),
            node_count: self.storage.len() as u32,
            extra_count: self.storage.extra_len() as u32,
            error_count: self.errors.len() as u32,
            context: self.context,
            min_indent: self.min_indent,
        }
    }

    pub fn restore(&mut self, snapshot: Snapshot) {
        self.cursor.set_index(snapshot.token_index);
        self.storage.truncate(snapshot.node_count as usize);
        self.storage.truncate_extra(snapshot.extra_count as usize);
        self.errors.truncate(snapshot.error_count as usize);
        self.context = snapshot.context;
        self.min_indent = snapshot.min_indent;
    }
}
```

#### 1.4 Formal Grammar

See [02-formal-grammar.md](02-formal-grammar.md)

### Tests

```rust
#[test]
fn test_node_storage_capacity() {
    // 1KB source → ~125 tokens → ~62 nodes
    let storage = NodeStorage::with_estimated_capacity(1024);
    assert!(storage.tags.capacity() >= 60);
}

#[test]
fn test_snapshot_restore() {
    let mut parser = Parser::new(source);
    let snap = parser.snapshot();
    parser.advance();
    parser.advance();
    parser.restore(snap);
    assert_eq!(parser.cursor.index(), snap.token_index);
}

#[test]
fn test_progress_tracking() {
    let result: ParseResult<_, _> = ParseResult::ok(Progress::Made, 42);
    assert!(result.made_progress());
}
```

## Phase 2: Core Parsing

### Goals
- Implement Pratt expression parser
- Add context bitflags
- Create indentation-aware combinators
- Define error type hierarchy

### Deliverables

#### 2.1 Pratt Parser (`grammar/expr/pratt.rs`)

See [04-pratt-parser.md](04-pratt-parser.md)

#### 2.2 Context Bitflags (`state/context.rs`)

```rust
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct ParseContext: u16 {
        const NONE           = 0;
        const IN_PATTERN     = 1 << 0;
        const IN_TYPE        = 1 << 1;
        const NO_STRUCT_LIT  = 1 << 2;  // if condition
        const CONST_EXPR     = 1 << 3;
        const IN_LOOP        = 1 << 4;
        const IN_FUNCTION    = 1 << 5;
        const ALLOW_YIELD    = 1 << 6;
    }
}

impl Parser<'_> {
    pub fn with_context<T>(
        &mut self,
        add: ParseContext,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old = self.context;
        self.context |= add;
        let result = f(self);
        self.context = old;
        result
    }

    pub fn without_context<T>(
        &mut self,
        remove: ParseContext,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old = self.context;
        self.context &= !remove;
        let result = f(self);
        self.context = old;
        result
    }
}
```

#### 2.3 Indentation Combinators (`combinators/indent.rs`)

See [06-indentation.md](06-indentation.md)

#### 2.4 Error Types (`error/types.rs`)

```rust
/// Top-level parse error
#[derive(Debug)]
pub enum ParseError {
    Expr(ExprError),
    Pattern(PatternError),
    Type(TypeParseError),
    Item(ItemError),
    Module(ModuleError),
    Lexical(LexicalError),
}

/// Expression-specific errors (30+ variants)
#[derive(Debug)]
pub enum ExprError {
    Start(Position),
    UnexpectedToken {
        found: TokenKind,
        expected: Vec<TokenKind>,
        hint: Option<String>,
    },

    // Bindings
    LetKeyword(Position),
    LetBindingName(Position),
    LetBindingEquals(Position),
    LetBindingValue(Position),

    // Conditionals
    IfCondition(Position),
    IfThen(Position),
    IfBody(Position),
    IfElse(Position),

    // Match
    MatchScrutinee(Position),
    MatchOpenBrace(Position),
    MatchPattern(Position),
    MatchArrow(Position),
    MatchBody(Position),

    // Lambdas
    LambdaParam(Position),
    LambdaArrow(Position),
    LambdaBody(Position),

    // Calls
    CallOpenParen(Position),
    CallArgument(Position),
    CallCloseParen(Position),

    // Run/Try/Match patterns
    RunComma(Position),
    RunExpr(Position),
    TryQuestion(Position),

    // ... more variants
}
```

### Tests

```rust
#[test]
fn test_pratt_precedence() {
    let ast = parse_expr("1 + 2 * 3");
    // Should be: 1 + (2 * 3)
    assert_matches!(ast, Binary { op: Add, right: Binary { op: Mul, .. }, .. });
}

#[test]
fn test_context_no_struct_literal() {
    // In if condition, `{` should not start struct literal
    let ast = parse_expr("if x {} else {}");
    assert!(ast.is_ok());
}

#[test]
fn test_indentation_enforced() {
    let result = parse_module("@foo () -> void =\nrun(\n  let x = 1,\n x)");
    // `x)` is not indented enough
    assert!(result.errors.iter().any(|e| matches!(e, ParseError::Indent(..))));
}
```

## Phase 3: Error UX

### Goals
- Implement expected token accumulation
- Add contextual hints
- Create recovery sync sets
- Build error message renderer

### Deliverables

#### 3.1 Expected Token Accumulation (`error/expected.rs`)

```rust
/// Bitset of expected tokens (up to 64 token kinds)
#[derive(Clone, Copy, Default)]
pub struct ExpectedTokens(u64);

impl ExpectedTokens {
    pub fn add(&mut self, kind: TokenKind) {
        self.0 |= 1 << (kind as u8);
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = TokenKind> {
        (0..64).filter_map(move |i| {
            if self.0 & (1 << i) != 0 {
                TokenKind::try_from(i as u8).ok()
            } else {
                None
            }
        })
    }

    pub fn to_vec(&self) -> Vec<TokenKind> {
        self.iter().collect()
    }
}
```

#### 3.2 Contextual Hints

```rust
impl ExprError {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            ExprError::LambdaArrow(_) => Some(
                "Lambda syntax is `x -> expr` or `(x, y) -> expr`"
            ),
            ExprError::MatchArrow(_) => Some(
                "Match arms use `->` not `=>`"
            ),
            ExprError::CallOpenParen(_) => Some(
                "Function calls require named arguments: `f(arg: value)`"
            ),
            _ => None,
        }
    }

    pub fn suggestion(&self, source: &str) -> Option<String> {
        match self {
            ExprError::UnexpectedToken { found: TokenKind::FatArrow, .. } => {
                Some("Replace `=>` with `->`".into())
            }
            _ => None,
        }
    }
}
```

#### 3.3 Recovery Sync Sets (`error/recovery.rs`)

```rust
/// Sync sets for error recovery (bitsets for efficiency)
pub struct SyncSets;

impl SyncSets {
    /// Tokens that can start a statement
    pub const STMT_START: u64 = {
        (1 << TokenKind::At as u8) |
        (1 << TokenKind::Use as u8) |
        (1 << TokenKind::Pub as u8) |
        (1 << TokenKind::Type as u8) |
        (1 << TokenKind::Trait as u8) |
        (1 << TokenKind::Impl as u8)
    };

    /// Tokens that end an expression
    pub const EXPR_END: u64 = {
        (1 << TokenKind::RParen as u8) |
        (1 << TokenKind::RBracket as u8) |
        (1 << TokenKind::RBrace as u8) |
        (1 << TokenKind::Comma as u8) |
        (1 << TokenKind::Newline as u8)
    };

    pub fn contains(set: u64, token: TokenKind) -> bool {
        set & (1 << token as u8) != 0
    }
}

impl Parser<'_> {
    pub fn synchronize(&mut self, sync_set: u64) {
        while !self.at_end() {
            if SyncSets::contains(sync_set, self.current_kind()) {
                return;
            }
            self.advance();
        }
    }
}
```

#### 3.4 Error Rendering (`error/render.rs`)

```rust
pub fn render_error(error: &ParseError, source: &str) -> String {
    let mut output = String::new();

    // Title
    writeln!(output, "-- {} --", error.title()).unwrap();
    writeln!(output).unwrap();

    // Source snippet with underline
    let pos = error.position();
    let line = get_line(source, pos.line);
    writeln!(output, "{:>4} | {}", pos.line, line).unwrap();

    // Underline
    let col = pos.column as usize;
    writeln!(output, "     | {}^", " ".repeat(col - 1)).unwrap();

    // Message
    writeln!(output).unwrap();
    writeln!(output, "{}", error.message()).unwrap();

    // Hint
    if let Some(hint) = error.hint() {
        writeln!(output).unwrap();
        writeln!(output, "Hint: {}", hint).unwrap();
    }

    // Suggestion
    if let Some(suggestion) = error.suggestion(source) {
        writeln!(output).unwrap();
        writeln!(output, "Try: {}", suggestion).unwrap();
    }

    output
}
```

### Tests

```rust
#[test]
fn test_expected_tokens() {
    let mut expected = ExpectedTokens::default();
    expected.add(TokenKind::Ident);
    expected.add(TokenKind::Int);
    assert!(expected.iter().collect::<Vec<_>>().contains(&TokenKind::Ident));
}

#[test]
fn test_sync_recovery() {
    let mut parser = Parser::new("@foo () -> = @bar () -> void = 1");
    let result = parser.parse_module();
    // Should recover and find @bar
    assert!(result.items.len() >= 1);
}

#[test]
fn test_error_hint() {
    let result = parse_expr("x => y");
    let err = result.unwrap_err();
    assert!(err.hint().unwrap().contains("->"));
}
```

## Phase 4: Optimization

### Goals
- Add branch hints on error paths
- Implement lookahead caching
- Create benchmarking suite
- Profile and optimize hot paths

### Deliverables

#### 4.1 Branch Hints

```rust
#[cold]
#[inline(never)]
fn make_error(kind: ExprError) -> ParseResult<NodeId> {
    ParseResult::err(Progress::None, ParseError::Expr(kind))
}

// Usage
if !self.at(TokenKind::Arrow) {
    return make_error(ExprError::LambdaArrow(self.position()));
}
```

#### 4.2 Lookahead Cache

```rust
/// Cache for expensive lookahead decisions
struct LookaheadCache {
    /// Position where cache was computed
    position: u32,
    /// Cached tristate results
    is_lambda: Option<Tristate>,
    is_struct_literal: Option<Tristate>,
}

impl Parser<'_> {
    fn is_lambda_cached(&mut self) -> Tristate {
        let pos = self.cursor.index();
        if self.lookahead_cache.position != pos {
            self.lookahead_cache = LookaheadCache {
                position: pos,
                is_lambda: None,
                is_struct_literal: None,
            };
        }

        *self.lookahead_cache.is_lambda.get_or_insert_with(|| {
            self.compute_is_lambda()
        })
    }
}
```

#### 4.3 Benchmarks (`benches/parse_bench.rs`)

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_parse_expr(c: &mut Criterion) {
    let inputs = [
        ("simple", "1 + 2"),
        ("nested", "((1 + 2) * 3) - (4 / 5)"),
        ("calls", "foo(a: bar(b: baz(c: 1)))"),
        ("lambda", "(x, y) -> x + y"),
    ];

    let mut group = c.benchmark_group("parse_expr");
    for (name, input) in inputs {
        group.bench_with_input(BenchmarkId::new("v2", name), input, |b, i| {
            b.iter(|| parse_expr(i))
        });
    }
    group.finish();
}

fn bench_parse_module(c: &mut Criterion) {
    let inputs = [
        ("small", include_str!("fixtures/small.ori")),
        ("medium", include_str!("fixtures/medium.ori")),
        ("large", include_str!("fixtures/large.ori")),
    ];

    let mut group = c.benchmark_group("parse_module");
    for (name, input) in inputs {
        group.bench_with_input(BenchmarkId::new("v2", name), input, |b, i| {
            b.iter(|| parse_module(i))
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse_expr, bench_parse_module);
criterion_main!(benches);
```

#### 4.4 Memory Profiling

```rust
#[test]
fn test_memory_usage() {
    let source = include_str!("fixtures/large.ori");
    let before = allocated_bytes();
    let _ast = parse_module(source);
    let after = allocated_bytes();

    let bytes_per_char = (after - before) as f64 / source.len() as f64;
    assert!(bytes_per_char < 1.5, "Memory usage too high: {:.2}", bytes_per_char);
}

#[test]
fn test_parser_state_size() {
    // Should fit in one cache line
    assert!(std::mem::size_of::<Parser>() <= 64);
}

#[test]
fn test_node_size() {
    // Tag + Span + Data = 1 + 8 + 8 = 17 bytes
    assert_eq!(std::mem::size_of::<NodeTag>(), 1);
    assert_eq!(std::mem::size_of::<Span>(), 8);
    assert_eq!(std::mem::size_of::<NodeData>(), 8);
}
```

## Migration Strategy

### Step 1: Parallel Development
- Build `ori_parse_v2` as separate crate
- Don't modify existing `ori_parse`
- Share `ori_ir` AST types where possible

### Step 2: Feature Flag
```toml
# Cargo.toml
[features]
parser_v2 = ["ori_parse_v2"]
```

```rust
// oric/src/lib.rs
#[cfg(feature = "parser_v2")]
use ori_parse_v2 as parser;

#[cfg(not(feature = "parser_v2"))]
use ori_parse as parser;
```

### Step 3: Test Parity
- Run all `tests/spec/` tests with both parsers
- Compare AST output for equivalence
- Track any semantic differences

### Step 4: Gradual Rollout
1. Enable v2 in CI alongside v1
2. Fix any spec test failures
3. Enable v2 as default
4. Deprecate v1

## Timeline Estimate

| Phase | Scope | Dependencies |
|-------|-------|--------------|
| Phase 1 | Foundation | None |
| Phase 2 | Core Parsing | Phase 1 |
| Phase 3 | Error UX | Phase 2 |
| Phase 4 | Optimization | Phase 3 |

## Risk Mitigation

### Risk: AST Compatibility
**Mitigation**: Share `ori_ir` types, add conversion layer if needed

### Risk: Error Message Regression
**Mitigation**: Snapshot tests for error output, compare v1 vs v2

### Risk: Performance Regression
**Mitigation**: Benchmark suite from Phase 4, run in CI

### Risk: Edge Case Differences
**Mitigation**: Fuzz testing with arbitrary input
