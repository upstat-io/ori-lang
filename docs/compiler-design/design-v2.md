# Sigil Compiler v2 Design Document

## Executive Summary

This document describes the architecture for a complete rewrite of the Sigil compiler, incorporating lessons learned from v1 and state-of-the-art compiler optimization techniques. The v2 compiler implements **Semantic Structural Compilation (SSC)** - a novel approach that exploits Sigil's pattern-based design for unprecedented compilation speed and incrementality.

**Goals:**
- 10x faster cold compilation than v1
- Sub-100ms incremental rebuilds for single-file changes
- Full compatibility with v1 (same tests pass, same semantics)
- Foundation for LSP with instant feedback

---

## Part 1: V1 Lessons Learned

### What Works Well (Keep)

| Aspect | Details |
|--------|---------|
| **Phase separation** | Lexer → Parser → AST → Types → IR → Codegen is clean |
| **Pattern system** | `PatternDefinition` trait is excellent, 14 patterns work |
| **Type system** | Comprehensive coverage, generics, traits, capabilities |
| **Error handling** | `DiagnosticResult` with spans and codes |
| **TIR design** | `TExpr` carrying type enables direct codegen |
| **Pass infrastructure** | `Pass` trait with dependencies |
| **Test runner** | Parallel execution, coverage checking |

### Pain Points (Fix)

| Issue | Impact | Root Cause |
|-------|--------|------------|
| **String everywhere** | 15-20% overhead | No interning; `HashMap<String, _>` lookups |
| **Environment cloning** | 10-15% overhead | Function calls clone entire `functions` and `configs` maps |
| **Box-heavy AST** | Memory fragmentation | `Box<Expr>` for every child node |
| **No parallelism** | Linear with file count | Single-threaded type checking |
| **No incrementality** | Full rebuild every time | No caching of intermediate results |
| **Dual codegen paths** | Maintenance burden | AST-based and TIR-based coexist |
| **Module re-parsing** | Wasted work | Imports trigger file I/O and parsing |
| **HashMap for struct fields** | Allocation per instance | `Value::Struct { fields: HashMap }` |

### V1 Statistics

```
compiler/sigilc/src/: 157 files, 40,268 lines
- lexer/: ~250 lines (logos-based, good)
- parser/: ~1,500 lines
- ast/: ~1,200 lines
- types/: ~1,118 lines (core complexity)
- ir/: ~900 lines
- eval/: ~2,000 lines (interpreter)
- codegen/: ~896 lines
- patterns/: ~1,500 lines (14 implementations)
```

---

## Part 2: V2 Architecture Overview

### The Semantic Structural Compilation Model

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SALSA QUERY DATABASE                           │
│                                                                             │
│   All compilation artifacts stored as memoized queries:                     │
│   - Inputs: SourceFile, Configuration                                       │
│   - Derived: Tokens, AST, Types, TIR, Code                                  │
│   - Durability: LOW (user code), HIGH (stdlib)                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           CONTENT-ADDRESSED STORAGE                         │
│                                                                             │
│   Everything stored by semantic hash:                                       │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│   │  Interned   │  │  Flattened  │  │   Pattern   │  │   Type      │       │
│   │  Strings    │  │    AST      │  │  Templates  │  │   Cache     │       │
│   │  (global)   │  │  (arena)    │  │  (shared)   │  │  (interned) │       │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          PARALLEL COMPILATION ENGINE                        │
│                                                                             │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│   │   Parallel  │  │   Parallel  │  │   Parallel  │  │   Parallel  │       │
│   │   Lexing    │  │   Parsing   │  │ Type Check  │  │   Codegen   │       │
│   │  (per file) │  │ (per file)  │  │(per module) │  │  (per func) │       │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TEST-GATED INVALIDATION                             │
│                                                                             │
│   Implementation changes + tests pass = NO downstream invalidation          │
│   Tests act as semantic contracts between modules                           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Core Design Principles

1. **Query Everything**: Every compilation step is a Salsa query
2. **Intern Everything**: Strings, types, AST subtrees all content-addressed
3. **Flatten Everything**: No `Box<Expr>`, use `ExprId(u32)` indices
4. **Parallelize Everything**: Files, modules, functions compile concurrently
5. **Cache Everything**: Cross-project pattern template sharing

---

## Part 3: Data Structures

### 3.1 Interned Identifiers

```rust
// Replace ALL String identifiers with this
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Name(u32);  // Index into global interner

// Global string interner (thread-safe, sharded)
pub struct Interner {
    shards: [RwLock<InternShard>; NUM_SHARDS],
}

struct InternShard {
    map: FxHashMap<&'static str, Name>,
    strings: Vec<&'static str>,  // Arena-allocated
    arena: bumpalo::Bump,
}

impl Interner {
    pub fn intern(&self, s: &str) -> Name {
        let shard_idx = fxhash(s) % NUM_SHARDS;
        // Fast path: read lock
        if let Some(&name) = self.shards[shard_idx].read().map.get(s) {
            return name;
        }
        // Slow path: write lock, allocate
        self.intern_slow(shard_idx, s)
    }

    pub fn resolve(&self, name: Name) -> &'static str {
        // O(1) lookup by index
    }
}

// Comparison is now O(1) integer compare, not O(n) string compare
impl PartialEq for Name {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0  // Single integer comparison!
    }
}
```

### 3.2 Flattened AST

```rust
// Instead of Box<Expr>, use indices into arena
#[derive(Copy, Clone)]
pub struct ExprId(u32);

#[derive(Copy, Clone)]
pub struct StmtId(u32);

#[derive(Copy, Clone)]
pub struct TypeId(u32);

// Expression stored in flat array
#[derive(Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

// No Box anywhere - just indices
pub enum ExprKind {
    // Literals (no children)
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Name),  // Interned!

    // References (interned names)
    Ident(Name),
    Config(Name),

    // Compound (indices, not boxes)
    Binary { op: BinaryOp, left: ExprId, right: ExprId },
    Unary { op: UnaryOp, operand: ExprId },
    Call { func: ExprId, args: ExprRange },  // Range into args array
    Field { receiver: ExprId, field: Name },
    Index { receiver: ExprId, index: ExprId },

    // Control flow
    If { cond: ExprId, then_branch: ExprId, else_branch: Option<ExprId> },
    Match { scrutinee: ExprId, arms: ArmRange },

    // Patterns (first-class)
    Pattern { kind: PatternKind, args: PatternArgsId },

    // etc.
}

// Contiguous storage for all expressions in a module
pub struct ExprArena {
    exprs: Vec<Expr>,
    args: Vec<ExprId>,      // Flattened argument lists
    arms: Vec<MatchArm>,    // Flattened match arms
}

impl ExprArena {
    pub fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    pub fn get(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0 as usize]
    }
}

// Range into flattened array (for variable-length children)
#[derive(Copy, Clone)]
pub struct ExprRange {
    start: u32,
    len: u16,
}
```

**Memory comparison:**

| Structure | V1 Size | V2 Size | Savings |
|-----------|---------|---------|---------|
| `Binary { left, right }` | 24 bytes (2 Box) | 12 bytes (2 u32 + op) | 50% |
| `Call { func, args: Vec }` | 40+ bytes | 12 bytes (u32 + range) | 70% |
| `Ident(String)` | 24 bytes | 4 bytes (Name) | 83% |

### 3.3 Interned Types

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TypeId(u32);

pub enum TypeKind {
    // Primitives (singletons, no allocation)
    Int, Float, Bool, Str, Char, Byte, Void, Never,

    // Compound (indices)
    List(TypeId),
    Map(TypeId, TypeId),
    Tuple(TypeRange),          // Range into types array
    Function { params: TypeRange, ret: TypeId },

    // User-defined (interned name)
    Named(Name),
    Generic { base: Name, args: TypeRange },

    // Special
    Option(TypeId),
    Result(TypeId, TypeId),
}

// Global type interner
pub struct TypeInterner {
    map: DashMap<TypeKind, TypeId>,
    types: RwLock<Vec<TypeKind>>,
}

impl TypeInterner {
    pub fn intern(&self, kind: TypeKind) -> TypeId {
        // Dedup identical types
        *self.map.entry(kind.clone()).or_insert_with(|| {
            let mut types = self.types.write();
            let id = TypeId(types.len() as u32);
            types.push(kind);
            id
        })
    }
}

// Type comparison is O(1)!
// List(Int) == List(Int) because they get the same TypeId
```

### 3.4 Module Representation

```rust
#[salsa::input]
pub struct SourceFile {
    #[return_ref]
    pub path: PathBuf,

    #[return_ref]
    pub text: String,

    // LOW for user code, HIGH for stdlib
    #[default]
    pub durability: Durability,
}

// Parsed module (Salsa tracked struct)
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

// Function with full type information
#[salsa::tracked]
pub struct Function<'db> {
    pub name: Name,
    pub module: Module<'db>,

    #[return_ref]
    pub params: Vec<(Name, TypeId)>,

    pub return_type: TypeId,
    pub body: ExprId,

    #[return_ref]
    pub capabilities: Vec<Name>,
}
```

---

## Part 4: Salsa Query System

### 4.1 Query Definitions

```rust
#[salsa::db]
pub trait Db: salsa::Database {
    // Inputs
    fn source_file(&self, path: PathBuf) -> SourceFile;

    // Lexing (per file)
    fn tokens(&self, file: SourceFile) -> TokenList;

    // Parsing (per file)
    fn parsed_module(&self, file: SourceFile) -> Module<'_>;

    // Name resolution (per module)
    fn resolved_module(&self, module: Module<'_>) -> ResolvedModule<'_>;

    // Type checking (per function)
    fn typed_function(&self, func: Function<'_>) -> TypedFunction<'_>;

    // TIR lowering (per function)
    fn lowered_function(&self, func: Function<'_>) -> TirFunction<'_>;

    // Codegen (per function)
    fn generated_code(&self, func: Function<'_>) -> GeneratedCode;

    // Test results (per test)
    fn test_result(&self, test: Test<'_>) -> TestResult;
}
```

### 4.2 Incremental Recompilation

```rust
// When a file changes:
fn on_file_change(db: &mut Database, path: &Path, new_text: String) {
    // 1. Update the input
    let file = db.source_file(path.to_path_buf());
    file.set_text(db).to(new_text);

    // 2. Salsa automatically:
    //    - Invalidates tokens(file)
    //    - Invalidates parsed_module(file)
    //    - Checks if AST actually changed (early cutoff)
    //    - If changed, invalidates typed_function for affected functions
    //    - If types changed, invalidates dependents
    //    - Stdlib (HIGH durability) NEVER revalidated

    // 3. On next query, only recompute what's needed
    let module = db.parsed_module(file);  // Recomputed
    for func in module.functions(db) {
        let typed = db.typed_function(func);  // Cached if unchanged
    }
}
```

### 4.3 Durability Levels

```rust
pub enum Durability {
    /// User code being edited - changes frequently
    Low,

    /// Project config, Cargo.toml equivalent - changes sometimes
    Medium,

    /// Standard library, dependencies - rarely changes
    High,
}

// On startup:
fn load_stdlib(db: &mut Database) {
    for file in stdlib_files() {
        let source = db.source_file(file.path.clone());
        // Mark as HIGH durability - never revalidate on user edits
        source.set_durability(db).to(Durability::High);
        source.set_text(db).to(file.contents);
    }
}

// Optimization: If only LOW durability inputs changed,
// skip validation of all HIGH durability queries
```

### 4.4 Test-Gated Invalidation

```rust
#[salsa::tracked]
fn typed_function_with_tests<'db>(
    db: &'db dyn Db,
    func: Function<'db>,
) -> TypedFunctionResult<'db> {
    let typed = typed_function(db, func);

    // Run associated tests
    let tests = func.tests(db);
    let all_pass = tests.iter().all(|t| db.test_result(*t).passed);

    TypedFunctionResult {
        typed,
        tests_pass: all_pass,
        // If tests pass, downstream can skip deep validation
        semantic_hash: if all_pass {
            Some(compute_semantic_hash(&typed))
        } else {
            None
        },
    }
}

// Downstream query checks semantic hash
#[salsa::tracked]
fn dependent_function<'db>(
    db: &'db dyn Db,
    func: Function<'db>,
) -> TypedFunction<'db> {
    for dep in func.dependencies(db) {
        let dep_result = typed_function_with_tests(db, dep);

        if let Some(hash) = dep_result.semantic_hash {
            // Tests passed - only check signature, not implementation
            verify_signature_compatible(func, dep, hash);
        } else {
            // Tests failed or don't exist - full validation
            verify_full_compatibility(func, dep);
        }
    }
    // ...
}
```

---

## Part 5: Parallel Compilation

### 5.1 File-Level Parallelism

```rust
use rayon::prelude::*;

fn compile_project(db: &Database, files: Vec<PathBuf>) -> Result<Program> {
    // Phase 1: Parse all files in parallel
    let modules: Vec<Module> = files
        .par_iter()
        .map(|path| {
            let file = db.source_file(path.clone());
            db.parsed_module(file)
        })
        .collect();

    // Phase 2: Resolve names (requires import graph)
    let resolved = resolve_modules(db, &modules)?;

    // Phase 3: Type check all functions in parallel
    let typed: Vec<TypedFunction> = resolved
        .functions()
        .par_iter()
        .map(|func| db.typed_function(*func))
        .collect::<Result<Vec<_>>>()?;

    // Phase 4: Generate code in parallel
    let code: Vec<GeneratedCode> = typed
        .par_iter()
        .map(|func| db.generated_code(func.function))
        .collect();

    // Phase 5: Link
    link(code)
}
```

### 5.2 Pattern-Parallel Parsing

```rust
// SIMD structural scan finds pattern boundaries
fn find_pattern_boundaries(source: &[u8]) -> Vec<PatternSpan> {
    // AVX2: Process 32 bytes at a time
    // Find: 'map(', 'filter(', 'fold(', 'run(', 'try(', etc.

    let mut spans = Vec::new();
    let mut i = 0;

    // SIMD-accelerated keyword detection
    while i + 32 <= source.len() {
        let chunk = load_256(source, i);

        // Check for pattern keywords
        let m_mask = eq_mask(chunk, b'm');  // Potential 'map'
        let f_mask = eq_mask(chunk, b'f');  // Potential 'filter', 'fold', 'find'
        let r_mask = eq_mask(chunk, b'r');  // Potential 'run', 'retry', 'recurse'

        // Process matches...
        i += 32;
    }

    spans
}

// Parse patterns in parallel
fn parse_patterns_parallel(
    source: &str,
    spans: &[PatternSpan],
    arena: &ExprArena,
) -> Vec<ExprId> {
    spans
        .par_iter()
        .map(|span| {
            let pattern_src = &source[span.start..span.end];
            parse_pattern(pattern_src, arena)
        })
        .collect()
}
```

### 5.3 Work-Stealing Type Checker

```rust
use crossbeam_deque::{Injector, Stealer, Worker};

struct TypeCheckPool {
    injector: Injector<TypeCheckTask>,
    workers: Vec<Worker<TypeCheckTask>>,
    stealers: Vec<Stealer<TypeCheckTask>>,
}

enum TypeCheckTask {
    Function(FunctionId),
    Method(ImplId, MethodId),
    TraitImpl(ImplId),
}

impl TypeCheckPool {
    fn run(&self, db: &Database) -> Vec<TypeCheckResult> {
        std::thread::scope(|s| {
            let handles: Vec<_> = self.workers
                .iter()
                .enumerate()
                .map(|(i, worker)| {
                    let stealers = &self.stealers;
                    let injector = &self.injector;

                    s.spawn(move || {
                        loop {
                            // Try local queue first
                            let task = worker.pop()
                                // Then try global queue
                                .or_else(|| injector.steal().success())
                                // Then steal from others
                                .or_else(|| {
                                    stealers.iter()
                                        .filter(|s| !std::ptr::eq(*s, &stealers[i]))
                                        .find_map(|s| s.steal().success())
                                });

                            match task {
                                Some(task) => process_task(db, task),
                                None => break,
                            }
                        }
                    })
                })
                .collect();

            handles.into_iter().map(|h| h.join().unwrap()).collect()
        })
    }
}
```

---

## Part 6: Pattern Template System

### 6.1 Pattern Signatures

```rust
// Pattern semantic identity (for caching)
#[derive(Hash, Eq, PartialEq)]
pub struct PatternSignature {
    pub kind: PatternKind,
    pub input_types: Vec<TypeId>,
    pub output_type: TypeId,
    pub transform_sig: Option<FunctionSig>,
}

// Example: map(.over: [int], .transform: x -> x * 2)
// Signature: PatternSignature {
//     kind: Map,
//     input_types: [List(Int)],
//     output_type: List(Int),
//     transform_sig: Some(FunctionSig { params: [Int], ret: Int }),
// }
```

### 6.2 Template Compilation

```rust
// Compile pattern template once, instantiate many times
pub struct PatternTemplateCache {
    templates: DashMap<PatternSignature, CompiledTemplate>,
}

impl PatternTemplateCache {
    pub fn get_or_compile(
        &self,
        sig: &PatternSignature,
        compile: impl FnOnce() -> CompiledTemplate,
    ) -> &CompiledTemplate {
        self.templates.entry(sig.clone()).or_insert_with(compile)
    }
}

// Template with holes for specific values
pub struct CompiledTemplate {
    /// Generic code with placeholders
    pub code: Vec<Instruction>,

    /// Positions where transform function pointer goes
    pub transform_slots: Vec<usize>,

    /// Positions where type-specific operations go
    pub type_slots: Vec<(usize, TypeSlotKind)>,
}

// Instantiation just fills in the slots
pub fn instantiate_template(
    template: &CompiledTemplate,
    transform_fn: FunctionPtr,
    type_impls: &[TypeImpl],
) -> Vec<Instruction> {
    let mut code = template.code.clone();

    for &slot in &template.transform_slots {
        code[slot] = Instruction::Call(transform_fn);
    }

    for &(slot, kind) in &template.type_slots {
        code[slot] = type_impls[kind as usize].instruction();
    }

    code
}
```

### 6.3 Pattern Fusion

```rust
// Detect fusible pattern chains
fn detect_pattern_chain(expr: ExprId, arena: &ExprArena) -> Option<PatternChain> {
    let mut chain = Vec::new();
    let mut current = expr;

    loop {
        let expr = arena.get(current);
        match &expr.kind {
            ExprKind::Pattern { kind, args } => {
                chain.push((*kind, *args));

                // Check if input is another pattern
                if let Some(input) = get_pattern_input(arena, *args) {
                    if let ExprKind::Pattern { .. } = arena.get(input).kind {
                        current = input;
                        continue;
                    }
                }
            }
            _ => {}
        }
        break;
    }

    if chain.len() >= 2 {
        Some(PatternChain(chain))
    } else {
        None
    }
}

// Fuse map -> filter -> fold into single pass
fn fuse_map_filter_fold(chain: &PatternChain) -> Option<FusedPattern> {
    match chain.as_slice() {
        [(PatternKind::Fold, fold_args),
         (PatternKind::Filter, filter_args),
         (PatternKind::Map, map_args)] => {
            Some(FusedPattern::MapFilterFold {
                input: get_input(map_args),
                map_fn: get_transform(map_args),
                filter_fn: get_predicate(filter_args),
                init: get_init(fold_args),
                fold_fn: get_op(fold_args),
            })
        }
        // Other fusion patterns...
        _ => None,
    }
}
```

---

## Part 7: Memory Management

### 7.1 Arena-Based Allocation

```rust
// Per-module arena for AST nodes
pub struct ModuleArena {
    bump: bumpalo::Bump,
    exprs: Vec<Expr>,
    types: Vec<TypeExpr>,
    items: Vec<Item>,
}

impl ModuleArena {
    pub fn new() -> Self {
        Self {
            bump: bumpalo::Bump::with_capacity(64 * 1024),  // 64KB initial
            exprs: Vec::with_capacity(1024),
            types: Vec::with_capacity(256),
            items: Vec::with_capacity(64),
        }
    }

    pub fn alloc_expr(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    // Bulk deallocation - just drop the arena
    pub fn reset(&mut self) {
        self.bump.reset();
        self.exprs.clear();
        self.types.clear();
        self.items.clear();
    }
}
```

### 7.2 Zero-Copy Token Stream

```rust
// Tokens reference source string, no allocation
pub struct Token<'src> {
    pub kind: TokenKind,
    pub text: &'src str,  // Slice into source
    pub span: Span,
}

pub struct TokenStream<'src> {
    source: &'src str,
    tokens: Vec<Token<'src>>,
}

// Lexer produces tokens without allocating strings
fn lex<'src>(source: &'src str) -> TokenStream<'src> {
    let mut tokens = Vec::with_capacity(source.len() / 4);  // Estimate
    let mut lexer = Lexer::new(source);

    while let Some(token) = lexer.next() {
        tokens.push(token);
    }

    TokenStream { source, tokens }
}
```

### 7.3 Struct Field Indexing

```rust
// At type check time, compute field indices
pub struct StructLayout {
    pub name: Name,
    pub fields: Vec<(Name, TypeId)>,
    pub field_indices: FxHashMap<Name, u32>,  // Name -> index
}

// Runtime struct: Vec, not HashMap
pub enum Value {
    Struct {
        layout: StructLayoutId,  // Index into layout table
        fields: Vec<Value>,      // Indexed by field_indices
    },
    // ...
}

// Field access is O(1) array index, not O(1) hash lookup
fn get_field(value: &Value, field: Name, layouts: &StructLayouts) -> &Value {
    match value {
        Value::Struct { layout, fields } => {
            let idx = layouts.get(*layout).field_indices[&field];
            &fields[idx as usize]
        }
        _ => panic!("not a struct"),
    }
}
```

---

## Part 8: Error Handling

### 8.1 Diagnostic System (Keep from V1, Enhanced)

```rust
#[derive(Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: ErrorCode,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

pub struct Label {
    pub span: Span,
    pub message: String,
    pub style: LabelStyle,
}

// Error codes for categorization
#[derive(Debug, Copy, Clone)]
pub enum ErrorCode {
    // Syntax errors: E1xxx
    E1001,  // Unexpected token
    E1002,  // Unclosed delimiter

    // Type errors: E2xxx
    E2001,  // Type mismatch
    E2002,  // Unknown type

    // Name errors: E3xxx
    E3001,  // Undefined variable
    E3002,  // Undefined function

    // Test errors: E4xxx
    E4001,  // Missing tests
    E4002,  // Test failure
}
```

### 8.2 Error Recovery

```rust
// Continue compilation after errors to report multiple issues
pub struct DiagnosticCollector {
    diagnostics: Vec<Diagnostic>,
    has_errors: bool,
}

impl DiagnosticCollector {
    pub fn error(&mut self, diag: Diagnostic) {
        self.has_errors = true;
        self.diagnostics.push(diag);
    }

    pub fn warning(&mut self, diag: Diagnostic) {
        self.diagnostics.push(diag);
    }

    pub fn into_result<T>(self, value: T) -> DiagnosticResult<T> {
        if self.has_errors {
            Err(self.diagnostics)
        } else {
            Ok((value, self.diagnostics))
        }
    }
}

// Parser with error recovery
fn parse_function(p: &mut Parser) -> Option<Function> {
    let start = p.expect(TokenKind::At)?;
    let name = p.expect_ident().or_else(|| {
        p.error("expected function name after '@'");
        p.recover_to(&[TokenKind::Eq, TokenKind::LParen]);
        None
    })?;
    // Continue parsing...
}
```

---

## Part 9: Testing Infrastructure

### 9.1 Parallel Test Runner

```rust
pub struct TestRunner {
    db: Database,
    thread_pool: rayon::ThreadPool,
}

impl TestRunner {
    pub fn run_all(&self) -> TestReport {
        let tests: Vec<Test> = self.db.all_tests();

        let results: Vec<TestResult> = tests
            .par_iter()
            .map(|test| self.run_test(test))
            .collect();

        TestReport::from_results(results)
    }

    fn run_test(&self, test: &Test) -> TestResult {
        // Each test runs in isolation
        let mut local_db = self.db.snapshot();  // Copy-on-write

        let start = Instant::now();
        let result = std::panic::catch_unwind(|| {
            eval_test(&local_db, test)
        });
        let duration = start.elapsed();

        match result {
            Ok(Ok(())) => TestResult::Pass { duration },
            Ok(Err(e)) => TestResult::Fail { error: e, duration },
            Err(panic) => TestResult::Panic { panic, duration },
        }
    }
}
```

### 9.2 Coverage Tracking

```rust
// Track which functions have tests
#[salsa::tracked]
fn function_test_coverage(db: &dyn Db, module: Module<'_>) -> CoverageReport {
    let functions: Vec<Function> = module.functions(db);
    let tests: Vec<Test> = module.tests(db);

    let mut coverage = FxHashMap::default();

    for func in &functions {
        if func.name(db).as_str() == "main" {
            continue;  // main exempt
        }
        coverage.insert(func.name(db), false);
    }

    for test in &tests {
        for target in test.targets(db) {
            coverage.insert(target, true);
        }
    }

    let uncovered: Vec<Name> = coverage
        .iter()
        .filter(|(_, covered)| !**covered)
        .map(|(name, _)| *name)
        .collect();

    if !uncovered.is_empty() {
        // Compilation error!
        CoverageReport::Incomplete { uncovered }
    } else {
        CoverageReport::Complete
    }
}
```

---

## Part 10: Implementation Phases

### Phase 1: Foundation (Weeks 1-4)

**Goal:** Core infrastructure without full language support

1. **String Interner** (Week 1)
   - Sharded concurrent interner
   - FxHash for internal hashing
   - Integration tests

2. **Flattened AST** (Week 2)
   - ExprArena, ExprId types
   - Basic expression kinds
   - Parser producing flat AST

3. **Salsa Integration** (Weeks 3-4)
   - Database trait definition
   - SourceFile input
   - tokens, parsed_module queries
   - Basic incrementality tests

**Deliverable:** Can lex and parse files incrementally

### Phase 2: Type System (Weeks 5-8)

**Goal:** Full type checking with interned types

1. **Type Interner** (Week 5)
   - TypeId, TypeKind
   - Primitive type singletons
   - Compound type interning

2. **Name Resolution** (Week 6)
   - Scope tracking
   - Import resolution
   - Module graph

3. **Type Checker** (Weeks 7-8)
   - Bidirectional inference
   - Trait bounds checking
   - Capability tracking
   - Error recovery

**Deliverable:** Can type check programs, report multiple errors

### Phase 3: Patterns & Evaluation (Weeks 9-12)

**Goal:** Pattern system and interpreter

1. **Pattern Infrastructure** (Week 9)
   - PatternDefinition trait (keep from v1)
   - Pattern signature computation
   - Template cache structure

2. **Core Patterns** (Week 10)
   - run, try, match
   - map, filter, fold
   - Template instantiation

3. **Interpreter** (Weeks 11-12)
   - Value representation (with struct indexing)
   - Environment without cloning
   - All patterns working

**Deliverable:** Can run programs via interpreter

### Phase 4: Parallelism (Weeks 13-16)

**Goal:** Parallel compilation

1. **Parallel Parsing** (Week 13)
   - Per-file parallel lex/parse
   - Pattern boundary detection

2. **Parallel Type Checking** (Weeks 14-15)
   - Work-stealing pool
   - Function-level parallelism
   - Dependency-aware scheduling

3. **Parallel Codegen** (Week 16)
   - Per-function code generation
   - Template parallel instantiation

**Deliverable:** Full parallel compilation pipeline

### Phase 5: Advanced Features (Weeks 17-20)

**Goal:** Production readiness

1. **Test-Gated Invalidation** (Week 17)
   - Semantic hashing
   - Test result caching
   - Invalidation optimization

2. **Pattern Fusion** (Week 18)
   - Chain detection
   - Fusion rules
   - Benchmarking

3. **LSP Support** (Weeks 19-20)
   - Diagnostic streaming
   - Completion
   - Go-to-definition

**Deliverable:** Production-ready compiler with LSP

---

## Part 11: Migration Strategy

### 11.1 Test Compatibility

```rust
// V1 tests should pass unchanged
// Run both compilers, compare outputs

fn verify_compatibility(test_file: &Path) {
    let v1_result = v1::run_file(test_file);
    let v2_result = v2::run_file(test_file);

    assert_eq!(v1_result, v2_result, "Output mismatch for {}", test_file);
}

// Run on entire test suite
fn full_compatibility_check() {
    for test in glob("tests/**/*.si") {
        verify_compatibility(&test);
    }
}
```

### 11.2 Gradual Rollout

```
Phase 1: V2 as alternative (`sigil2 run`)
Phase 2: V2 as default, V1 available (`sigil --v1 run`)
Phase 3: V1 deprecated
Phase 4: V1 removed
```

### 11.3 Feature Flags

```rust
// Enable incremental features gradually
pub struct CompilerConfig {
    pub use_interning: bool,        // Phase 1
    pub use_flat_ast: bool,         // Phase 1
    pub use_salsa: bool,            // Phase 1
    pub use_parallel_parse: bool,   // Phase 4
    pub use_parallel_check: bool,   // Phase 4
    pub use_test_gating: bool,      // Phase 5
    pub use_pattern_fusion: bool,   // Phase 5
}
```

---

## Part 12: Benchmarks & Targets

### Expected Performance

| Metric | V1 | V2 Target | Speedup |
|--------|-----|-----------|---------|
| Cold compile (1K LOC) | 500ms | 50ms | 10x |
| Cold compile (10K LOC) | 5s | 300ms | 16x |
| Incremental (1 file) | 500ms | 50ms | 10x |
| Memory (10K LOC) | 200MB | 50MB | 4x |
| Type check throughput | 5K LOC/s | 50K LOC/s | 10x |

### Benchmark Suite

```rust
#[bench]
fn bench_parse_1k_lines(b: &mut Bencher) {
    let source = generate_source(1000);
    b.iter(|| parse(&source));
}

#[bench]
fn bench_typecheck_1k_functions(b: &mut Bencher) {
    let module = setup_module_with_functions(1000);
    b.iter(|| typecheck(&module));
}

#[bench]
fn bench_incremental_single_edit(b: &mut Bencher) {
    let mut db = setup_project();
    b.iter(|| {
        db.edit_file("src/main.si", add_comment);
        db.compile();
    });
}
```

---

## Appendix A: File Structure

```
compiler/sigilc-v2/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── db.rs               # Salsa database definition
│   │
│   ├── intern/             # Interning infrastructure
│   │   ├── mod.rs
│   │   ├── strings.rs      # String interner
│   │   └── types.rs        # Type interner
│   │
│   ├── syntax/             # Parsing layer
│   │   ├── mod.rs
│   │   ├── lexer.rs        # Logos-based lexer
│   │   ├── parser.rs       # Recursive descent
│   │   ├── ast.rs          # Flattened AST
│   │   └── arena.rs        # Expression arena
│   │
│   ├── hir/                # High-level IR (typed)
│   │   ├── mod.rs
│   │   ├── types.rs        # Type representation
│   │   ├── expr.rs         # Typed expressions
│   │   └── lower.rs        # AST → HIR
│   │
│   ├── check/              # Type checking
│   │   ├── mod.rs
│   │   ├── infer.rs        # Type inference
│   │   ├── unify.rs        # Unification
│   │   ├── traits.rs       # Trait resolution
│   │   └── capabilities.rs # Capability checking
│   │
│   ├── tir/                # Typed IR (lowered)
│   │   ├── mod.rs
│   │   ├── expr.rs
│   │   └── lower.rs        # HIR → TIR
│   │
│   ├── patterns/           # Pattern system
│   │   ├── mod.rs
│   │   ├── definition.rs   # PatternDefinition trait
│   │   ├── builtins/       # Built-in patterns
│   │   ├── templates.rs    # Template compilation
│   │   └── fusion.rs       # Pattern chain fusion
│   │
│   ├── eval/               # Interpreter
│   │   ├── mod.rs
│   │   ├── value.rs        # Runtime values
│   │   ├── env.rs          # Environment
│   │   └── exec.rs         # Execution
│   │
│   ├── codegen/            # Code generation
│   │   ├── mod.rs
│   │   ├── c/              # C backend
│   │   └── templates.rs    # Pattern templates
│   │
│   ├── tests/              # Test infrastructure
│   │   ├── mod.rs
│   │   ├── runner.rs       # Parallel runner
│   │   └── coverage.rs     # Coverage checking
│   │
│   ├── errors/             # Diagnostics
│   │   ├── mod.rs
│   │   ├── codes.rs
│   │   └── render.rs
│   │
│   └── cli/                # Command line
│       ├── mod.rs
│       ├── run.rs
│       ├── build.rs
│       └── test.rs
│
└── tests/
    ├── compatibility/      # V1/V2 comparison tests
    ├── incremental/        # Incrementality tests
    └── parallel/           # Parallelism tests
```

---

## Appendix B: Key Dependencies

```toml
[dependencies]
# Query system
salsa = "0.17"

# Parallelism
rayon = "1.8"
crossbeam = "0.8"
parking_lot = "0.12"

# Data structures
dashmap = "5.5"
rustc-hash = "1.1"      # FxHash
bumpalo = "3.14"        # Arena allocator
thin-vec = "0.2"        # Small vec optimization

# Lexing
logos = "0.13"

# Serialization (for caching)
rkyv = "0.7"            # Zero-copy deserialization

# Error reporting
codespan-reporting = "0.11"
```

---

## Appendix C: Comparison with V1

| Aspect | V1 | V2 |
|--------|-----|-----|
| **Identifiers** | `String` | `Name(u32)` interned |
| **AST children** | `Box<Expr>` | `ExprId(u32)` |
| **Type storage** | `Clone` everywhere | `TypeId(u32)` interned |
| **Environment** | Clone on call | Persistent + local overlay |
| **Struct fields** | `HashMap<String, Value>` | `Vec<Value>` indexed |
| **Caching** | None | Salsa queries |
| **Parallelism** | None | Rayon + work-stealing |
| **Incrementality** | None | Full via Salsa |
| **Pattern templates** | Recompile each use | Compile once, instantiate |

---

## Summary

The V2 compiler represents a fundamental rearchitecture that:

1. **Eliminates allocation overhead** through pervasive interning and flat data structures
2. **Enables parallelism** through careful dependency tracking and work-stealing
3. **Provides incrementality** through Salsa's query system
4. **Exploits Sigil's patterns** for template sharing and fusion optimization
5. **Uses tests as contracts** for smarter invalidation

The result should be a compiler that's 10x+ faster for cold builds and provides sub-100ms incremental rebuilds, enabling the responsive development experience that Sigil's AI-first design philosophy demands.
