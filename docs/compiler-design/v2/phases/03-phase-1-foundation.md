# Phase 1: Foundation (Weeks 1-4)

## Goal

Build the core infrastructure that all subsequent phases depend on:
- Sharded string interner for O(1) identifier comparison
- Flattened AST with arena allocation
- Salsa query system integration

**Deliverable:** Incremental lexing and parsing of Sigil files.

---

## Week 1: String Interner

### Objective

Replace all `String` identifiers with `Name(u32)` - an index into a global interner.

### Data Structures

```rust
/// Interned string identifier - Copy, cheap to compare
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Name(u32);

impl Name {
    pub const EMPTY: Name = Name(0);  // Pre-interned ""
}

/// Thread-safe sharded string interner
pub struct StringInterner {
    shards: [RwLock<InternShard>; NUM_SHARDS],
}

const NUM_SHARDS: usize = 16;  // Tuned for typical core counts

struct InternShard {
    /// String → Name lookup
    map: FxHashMap<&'static str, Name>,
    /// Name → String lookup (by index within shard)
    strings: Vec<&'static str>,
    /// Arena for string storage
    arena: bumpalo::Bump,
}
```

### Implementation

```rust
impl StringInterner {
    pub fn new() -> Self {
        let shards = std::array::from_fn(|_| {
            RwLock::new(InternShard {
                map: FxHashMap::default(),
                strings: Vec::with_capacity(1024),
                arena: bumpalo::Bump::with_capacity(64 * 1024),
            })
        });

        let mut interner = Self { shards };

        // Pre-intern common strings
        interner.intern("");  // Name(0) = empty
        interner.intern("int");
        interner.intern("float");
        interner.intern("bool");
        interner.intern("str");
        // ... other keywords

        interner
    }

    pub fn intern(&self, s: &str) -> Name {
        let hash = fxhash::hash(s);
        let shard_idx = (hash as usize) % NUM_SHARDS;

        // Fast path: read lock
        {
            let shard = self.shards[shard_idx].read();
            if let Some(&name) = shard.map.get(s) {
                return name;
            }
        }

        // Slow path: write lock
        self.intern_slow(shard_idx, s, hash)
    }

    fn intern_slow(&self, shard_idx: usize, s: &str, hash: u64) -> Name {
        let mut shard = self.shards[shard_idx].write();

        // Double-check after acquiring write lock
        if let Some(&name) = shard.map.get(s) {
            return name;
        }

        // Allocate string in arena (lives forever)
        let allocated: &str = shard.arena.alloc_str(s);
        // Safety: Arena lives as long as interner
        let static_str: &'static str = unsafe {
            std::mem::transmute(allocated)
        };

        // Compute global name from shard index + local index
        let local_idx = shard.strings.len() as u32;
        let name = Name((shard_idx as u32) << 28 | local_idx);

        shard.strings.push(static_str);
        shard.map.insert(static_str, name);

        name
    }

    pub fn resolve(&self, name: Name) -> &'static str {
        let shard_idx = (name.0 >> 28) as usize;
        let local_idx = (name.0 & 0x0FFF_FFFF) as usize;

        let shard = self.shards[shard_idx].read();
        shard.strings[local_idx]
    }
}
```

### Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `intern` (hit) | O(1) + read lock | Fast path for existing strings |
| `intern` (miss) | O(n) + write lock | n = string length |
| `resolve` | O(1) + read lock | Direct array index |
| `Name == Name` | O(1) | Single integer compare |
| Memory per string | string.len() + 8 bytes | Arena + hash entry |

### Tests

```rust
#[test]
fn test_interner_basic() {
    let interner = StringInterner::new();

    let a = interner.intern("hello");
    let b = interner.intern("hello");
    let c = interner.intern("world");

    assert_eq!(a, b);  // Same string = same Name
    assert_ne!(a, c);  // Different string = different Name
    assert_eq!(interner.resolve(a), "hello");
}

#[test]
fn test_interner_concurrent() {
    let interner = Arc::new(StringInterner::new());

    let handles: Vec<_> = (0..16).map(|i| {
        let interner = Arc::clone(&interner);
        std::thread::spawn(move || {
            for j in 0..1000 {
                interner.intern(&format!("string_{}_{}", i, j));
            }
        })
    }).collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // All strings interned correctly
    assert_eq!(
        interner.resolve(interner.intern("string_5_500")),
        "string_5_500"
    );
}
```

### Benchmark Targets

| Metric | Target |
|--------|--------|
| Intern 100K unique strings | <50ms |
| Intern 1M duplicate strings | <100ms |
| Resolve 1M names | <20ms |

---

## Week 2: Flattened AST

### Objective

Replace `Box<Expr>` with `ExprId(u32)` indices into a flat arena.

### Data Structures

```rust
/// Index into expression arena
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct ExprId(u32);

/// Index into statement arena
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct StmtId(u32);

/// Range of expressions (for argument lists, etc.)
#[derive(Copy, Clone)]
pub struct ExprRange {
    pub start: u32,
    pub len: u16,
}

impl ExprRange {
    pub fn empty() -> Self {
        Self { start: 0, len: 0 }
    }

    pub fn indices(&self) -> impl Iterator<Item = ExprId> {
        (self.start..self.start + self.len as u32).map(ExprId)
    }
}

/// Source span
#[derive(Copy, Clone)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

/// Expression node
#[derive(Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// Expression variants - no Box anywhere
pub enum ExprKind {
    // Literals (no children)
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Name),  // Interned
    Char(char),

    // References
    Ident(Name),
    Config(Name),  // $config

    // Compound expressions (indices, not boxes)
    Binary { op: BinaryOp, left: ExprId, right: ExprId },
    Unary { op: UnaryOp, operand: ExprId },
    Call { func: ExprId, args: ExprRange },
    Field { receiver: ExprId, field: Name },
    Index { receiver: ExprId, index: ExprId },
    MethodCall { receiver: ExprId, method: Name, args: ExprRange },

    // Control flow
    If { cond: ExprId, then_branch: ExprId, else_branch: Option<ExprId> },
    Match { scrutinee: ExprId, arms: ArmRange },
    For { binding: Name, iter: ExprId, body: ExprId },
    Loop { body: ExprId },

    // Patterns (first-class)
    Pattern { kind: PatternKind, args: PatternArgsId },

    // Other
    Block { stmts: StmtRange, result: Option<ExprId> },
    Lambda { params: ParamRange, body: ExprId },
    Let { name: Name, ty: Option<TypeExprId>, init: ExprId },
    Return(Option<ExprId>),
    Break(Option<ExprId>),
    Continue,
}
```

### Expression Arena

```rust
/// Contiguous storage for all expressions in a module
pub struct ExprArena {
    exprs: Vec<Expr>,
    /// Flattened argument lists (Call args, etc.)
    expr_lists: Vec<ExprId>,
    /// Match arms
    arms: Vec<MatchArm>,
    /// Statements
    stmts: Vec<Stmt>,
}

impl ExprArena {
    pub fn new() -> Self {
        Self {
            exprs: Vec::with_capacity(4096),
            expr_lists: Vec::with_capacity(1024),
            arms: Vec::with_capacity(256),
            stmts: Vec::with_capacity(1024),
        }
    }

    /// Allocate a single expression
    pub fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    /// Allocate a list of expressions, return range
    pub fn alloc_list(&mut self, exprs: impl IntoIterator<Item = ExprId>) -> ExprRange {
        let start = self.expr_lists.len() as u32;
        self.expr_lists.extend(exprs);
        let len = (self.expr_lists.len() as u32 - start) as u16;
        ExprRange { start, len }
    }

    /// Get expression by ID
    pub fn get(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }

    /// Get expression list by range
    pub fn get_list(&self, range: ExprRange) -> &[ExprId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.expr_lists[start..end]
    }
}
```

### Parser Integration

```rust
impl Parser<'_> {
    fn parse_expr(&mut self) -> Result<ExprId> {
        self.parse_expr_bp(0)  // Pratt parser
    }

    fn parse_binary(&mut self, left: ExprId, op: BinaryOp) -> Result<ExprId> {
        let right = self.parse_expr_bp(op.precedence())?;
        let span = self.span_from(self.arena.get(left).span.start);

        Ok(self.arena.alloc(Expr {
            kind: ExprKind::Binary { op, left, right },
            span,
        }))
    }

    fn parse_call(&mut self, func: ExprId) -> Result<ExprId> {
        self.expect(TokenKind::LParen)?;

        let mut args = Vec::new();
        while !self.check(TokenKind::RParen) {
            args.push(self.parse_expr()?);
            if !self.eat(TokenKind::Comma) {
                break;
            }
        }

        self.expect(TokenKind::RParen)?;

        let args_range = self.arena.alloc_list(args);
        let span = self.span_from(self.arena.get(func).span.start);

        Ok(self.arena.alloc(Expr {
            kind: ExprKind::Call { func, args: args_range },
            span,
        }))
    }
}
```

### Memory Comparison

| Structure | V1 Size | V2 Size | Savings |
|-----------|---------|---------|---------|
| `Binary { left, right }` | 24+ bytes | 12 bytes | 50% |
| `Call { func, args: Vec }` | 40+ bytes | 12 bytes | 70% |
| `Ident(String)` | 24 bytes | 4 bytes | 83% |
| `If { cond, then, else }` | 48+ bytes | 20 bytes | 58% |

### Tests

```rust
#[test]
fn test_arena_allocation() {
    let mut arena = ExprArena::new();

    let lit = arena.alloc(Expr {
        kind: ExprKind::Int(42),
        span: Span { start: 0, end: 2 },
    });

    let ident = arena.alloc(Expr {
        kind: ExprKind::Ident(Name(1)),
        span: Span { start: 5, end: 6 },
    });

    let binary = arena.alloc(Expr {
        kind: ExprKind::Binary {
            op: BinaryOp::Add,
            left: lit,
            right: ident,
        },
        span: Span { start: 0, end: 6 },
    });

    assert_eq!(arena.exprs.len(), 3);
    match &arena.get(binary).kind {
        ExprKind::Binary { left, right, .. } => {
            assert_eq!(arena.get(*left).kind, ExprKind::Int(42));
        }
        _ => panic!("expected binary"),
    }
}
```

---

## Weeks 3-4: Salsa Integration

### Objective

Integrate Salsa query system for automatic incrementality.

### Database Definition

```rust
#[salsa::db]
pub trait Db: salsa::Database {
    /// String interner (shared across all queries)
    fn interner(&self) -> &StringInterner;

    /// Type interner (shared across all queries)
    fn type_interner(&self) -> &TypeInterner;
}

/// Input: Source file content
#[salsa::input]
pub struct SourceFile {
    /// File path (for error messages)
    #[return_ref]
    pub path: PathBuf,

    /// Source text
    #[return_ref]
    pub text: String,

    /// Durability level
    #[default]
    pub durability: Durability,
}

/// Concrete database implementation
#[salsa::db]
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
    interner: StringInterner,
    type_interner: TypeInterner,
}

impl salsa::Database for CompilerDb {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        // Optional: logging for debugging incrementality
        #[cfg(debug_assertions)]
        {
            let event = event();
            tracing::trace!("salsa event: {:?}", event);
        }
    }
}

impl Db for CompilerDb {
    fn interner(&self) -> &StringInterner {
        &self.interner
    }

    fn type_interner(&self) -> &TypeInterner {
        &self.type_interner
    }
}
```

### Query Definitions

```rust
/// Lexing query: Source → Tokens
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let source = file.text(db);
    let interner = db.interner();

    let mut lexer = Lexer::new(source, interner);
    lexer.lex_all()
}

/// Parsing query: Tokens → AST
#[salsa::tracked]
pub fn parsed_module(db: &dyn Db, file: SourceFile) -> Module {
    let tokens = tokens(db, file);
    let interner = db.interner();

    let mut parser = Parser::new(&tokens, interner);
    parser.parse_module()
}

/// Tracked struct for parsed modules
#[salsa::tracked]
pub struct Module<'db> {
    pub file: SourceFile,

    #[return_ref]
    pub items: Vec<ItemId>,

    #[return_ref]
    pub expr_arena: ExprArena,

    #[return_ref]
    pub imports: Vec<Import>,
}
```

### Durability Configuration

```rust
/// Durability levels for different input types
#[derive(Copy, Clone, Default)]
pub enum Durability {
    /// User code being edited - changes frequently
    #[default]
    Low,

    /// Project config - changes sometimes
    Medium,

    /// Standard library - rarely changes
    High,
}

impl From<Durability> for salsa::Durability {
    fn from(d: Durability) -> Self {
        match d {
            Durability::Low => salsa::Durability::LOW,
            Durability::Medium => salsa::Durability::MEDIUM,
            Durability::High => salsa::Durability::HIGH,
        }
    }
}

/// Load standard library with HIGH durability
pub fn load_stdlib(db: &mut CompilerDb) {
    for (path, content) in STDLIB_FILES {
        let file = SourceFile::new(
            db,
            PathBuf::from(path),
            content.to_string(),
        );
        file.set_durability(db).to(Durability::High);
    }
}
```

### Incremental Update

```rust
impl CompilerDb {
    /// Update a file and trigger recomputation
    pub fn update_file(&mut self, path: &Path, new_text: String) {
        // Find existing file
        if let Some(file) = self.find_file(path) {
            // Update text - Salsa handles invalidation
            file.set_text(self).to(new_text);
        } else {
            // New file
            SourceFile::new(self, path.to_path_buf(), new_text);
        }
    }

    /// Compile with incrementality
    pub fn compile(&self) -> CompileResult {
        let files = self.all_files();

        // Parse all files (cached if unchanged)
        let modules: Vec<_> = files
            .iter()
            .map(|f| parsed_module(self, *f))
            .collect();

        // Type check (Phase 2)
        // Codegen (Phase 3)
        // ...
    }
}
```

### Tests

```rust
#[test]
fn test_incremental_parse() {
    let mut db = CompilerDb::new();

    // Initial parse
    let file = SourceFile::new(
        &db,
        PathBuf::from("test.si"),
        "@add (a: int, b: int) -> int = a + b".to_string(),
    );

    let module1 = parsed_module(&db, file);
    assert_eq!(module1.items(&db).len(), 1);

    // Add a comment - should reparse
    file.set_text(&mut db).to(
        "// comment\n@add (a: int, b: int) -> int = a + b".to_string()
    );

    let module2 = parsed_module(&db, file);
    assert_eq!(module2.items(&db).len(), 1);

    // Verify it's a new module (reparsed)
    assert_ne!(module1, module2);
}

#[test]
fn test_durability_levels() {
    let mut db = CompilerDb::new();

    // Stdlib file with HIGH durability
    let stdlib = SourceFile::new(&db, "std/math.si".into(), MATH_SOURCE.into());
    stdlib.set_durability(&mut db).to(Durability::High);

    // User file with LOW durability
    let user = SourceFile::new(&db, "main.si".into(), USER_SOURCE.into());

    // Parse both
    let _ = parsed_module(&db, stdlib);
    let _ = parsed_module(&db, user);

    // Only LOW durability inputs changed
    user.set_text(&mut db).to(MODIFIED_USER_SOURCE.into());

    // Stdlib should NOT be revalidated
    // (verified via salsa event logging in debug builds)
}
```

---

## Phase 1 Deliverables Checklist

### Week 1: String Interner
- [ ] `Name(u32)` type with Copy, Clone, Eq, PartialEq, Hash
- [ ] Sharded `StringInterner` with 16 shards
- [ ] Fast path with read lock, slow path with write lock
- [ ] Pre-interned keywords and common strings
- [ ] Benchmarks passing targets

### Week 2: Flattened AST
- [ ] `ExprId(u32)` and `ExprRange` types
- [ ] `ExprArena` with alloc and lookup
- [ ] `ExprKind` enum with all expression variants
- [ ] No `Box<Expr>` anywhere in AST
- [ ] Parser updated to use arena

### Weeks 3-4: Salsa Integration
- [ ] `Db` trait with interner access
- [ ] `SourceFile` input with durability
- [ ] `tokens` query
- [ ] `parsed_module` query
- [ ] `Module` tracked struct
- [ ] Incrementality tests passing

### Integration Tests
- [ ] Parse all existing test files
- [ ] Compare AST structure with V1
- [ ] Verify incremental behavior
- [ ] Memory usage within targets

---

## Next Phase

With foundation complete, proceed to [Phase 2: Type System](04-phase-2-type-system.md) to build type interning and inference.
