---
title: "Appendix B: Memory Management"
description: "Ori Compiler Design — Appendix B: Memory Management"
order: 1002
section: "Appendices"
---

# Appendix B: Memory Management

Memory management strategies used in the Ori compiler.

## Stack Safety

Deeply nested expressions can cause stack overflow in recursive parsing, type-checking, and evaluation. The compiler uses the `stacker` crate to dynamically grow the stack when needed:

```rust
// src/stack.rs
const RED_ZONE: usize = 100 * 1024;     // 100KB minimum remaining
const STACK_PER_RECURSION: usize = 1024 * 1024;  // 1MB growth

#[inline]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}
```

Applied to recursive functions:

```rust
// Parser
pub fn parse_expr(&mut self) -> Result<ExprId, ParseError> {
    ensure_sufficient_stack(|| self.parse_expr_inner())
}

// Type checker
pub fn infer_expr(checker: &mut TypeChecker<'_>, expr_id: ExprId) -> Type {
    ensure_sufficient_stack(|| infer_expr_inner(checker, expr_id))
}

// Evaluator
pub fn eval(&mut self, expr_id: ExprId) -> EvalResult {
    ensure_sufficient_stack(|| self.eval_inner(expr_id))
}
```

This prevents stack overflow on deeply nested code like `((((((((((1))))))))))` while adding minimal overhead to normal execution.

## Arena Allocation

Expressions use arena allocation:

```rust
pub struct ExprArena {
    exprs: Vec<Expr>,
}

impl ExprArena {
    pub fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }
}
```

Benefits:
- Contiguous memory (cache-friendly)
- No individual deallocations
- Simple lifetime management

## String Interning

All identifiers are interned:

```rust
pub struct Interner {
    strings: Vec<String>,
    lookup: HashMap<String, Name>,
}
```

Memory savings:
- "foo" appears 100 times → stored once
- Name is 4 bytes vs String's ~24 bytes

### SharedInterner

`SharedInterner` is an `Arc`-wrapped `StringInterner` that enables sharing the interner across database instances and threads. It is `Clone`-cheap (reference-counted pointer) and the underlying `StringInterner` uses per-shard `RwLock`s for concurrent access:

```rust
#[derive(Clone)]
pub struct SharedInterner(Arc<StringInterner>);
```

The `CompilerDb` exposes its interner as a `SharedInterner`, and test harnesses clone it to create isolated databases that share the same interned string pool.

## Arc for Shared Values

Runtime values use Arc for sharing:

```rust
pub enum Value {
    String(Arc<String>),
    List(Arc<Vec<Value>>),
    // ...
}
```

Why Arc:
- Closures capture environment by cloning
- Multiple references to same list
- Safe concurrent access

## SharedRegistry vs SharedMutableRegistry

The compiler uses two registry patterns:

### SharedRegistry<T> (Immutable)

For registries that are built completely before use:

```rust
pub struct SharedRegistry<T>(Arc<T>);

impl<T> SharedRegistry<T> {
    pub fn new(registry: T) -> Self {
        SharedRegistry(Arc::new(registry))
    }
}
```

Use when:
- Registry is fully populated before access
- No modifications needed after construction
- Salsa query compatibility required

### SharedMutableRegistry<T> (Interior Mutability)

For registries that need modification after dependent structures are built:

```rust
pub struct SharedMutableRegistry<T>(Arc<parking_lot::RwLock<T>>);

impl<T> SharedMutableRegistry<T> {
    pub fn new(registry: T) -> Self {
        SharedMutableRegistry(Arc::new(parking_lot::RwLock::new(registry)))
    }

    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.0.read()
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.0.write()
    }
}
```

Use when:
- Need to add entries after construction
- Dependent structures (like cached dispatchers) must see updates
- Acceptable trade-off: RwLock overhead vs rebuilding cost

**Example: Method Dispatch Caching**

The `MethodDispatcher` is cached in the Evaluator to avoid rebuilding the resolver
chain on every method call. However, `load_module()` registers new methods after
the Evaluator is constructed. Using `SharedMutableRegistry<UserMethodRegistry>`:

```rust
// In EvaluatorBuilder::build():
let user_method_registry = SharedMutableRegistry::new(UserMethodRegistry::new());
let method_dispatcher = MethodDispatcher::new(vec![
    Box::new(UserMethodResolver::new(user_method_registry.clone())),
    // ... other resolvers
]);

// In load_module():
self.user_method_registry.write().merge(new_methods);

// In method resolution:
if let Some(method) = self.registry.read().lookup(type_name, method_name) { ... }
```

This avoids 4 Box allocations per method call while still allowing dynamic
method registration.

## Heap<T> Wrapper

Ensures consistent allocation:

```rust
pub struct Heap<T>(Arc<T>);

impl<T> Heap<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(value))
    }
}
```

Prevents:
- Accidental bare Arc creation
- Inconsistent allocation patterns

## Copy Types

Small types are Copy:

```rust
#[derive(Clone, Copy)]
pub struct ExprId(u32);

#[derive(Clone, Copy)]
pub struct Name(u32);

#[derive(Clone, Copy)]
pub struct Span { start: u32, end: u32 }
```

Benefits:
- No heap allocation
- Trivial to pass around
- No lifetime complications

## MethodKey

Type-safe key for method registry lookups. Fields are interned `Name` values (not `String`), making `MethodKey` `Copy` and enabling zero-allocation comparisons:

```rust
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct MethodKey {
    pub type_name: Name,
    pub method_name: Name,
}

impl MethodKey {
    pub const fn new(type_name: Name, method_name: Name) -> Self;
}
```

Because `Name` is an opaque interned index, `MethodKey` does not implement `Display` directly. A `MethodKeyDisplay` helper resolves the names through the interner for formatting:

```rust
impl MethodKey {
    pub fn display<'a>(&self, interner: &'a SharedInterner) -> MethodKeyDisplay<'a>;
}

// Displays as "Point::distance"
```

Benefits:
- `Copy` type with zero-allocation lookups (vs tuple of strings)
- Better error messages (`Point::distance` instead of `("Point", "distance")`)
- Hashable for use in registries

## Token Storage

Tokens stored in parallel arrays:

```rust
pub struct TokenList {
    kinds: Vec<TokenKind>,
    spans: Vec<Span>,
}
```

Better than `Vec<Token>` because:
- TokenKind often accessed without span
- Better memory locality for iteration

## Session-Scoped Side-Caches

Three caches store data **outside** Salsa's dependency graph because their values cannot satisfy Salsa's `Clone + Eq + Hash` requirements:

```rust
/// Stores type-checking Pool results per file.
pub struct PoolCache(Arc<RwLock<HashMap<PathBuf, Arc<Pool>>>>);

/// Stores canonicalized results per file.
pub struct CanonCache(Arc<RwLock<HashMap<PathBuf, SharedCanonResult>>>);

/// Stores resolved imports per file.
pub struct ImportsCache(Arc<RwLock<HashMap<PathBuf, Arc<ResolvedImports>>>>);
```

These caches exist on `CompilerDb` and are keyed by file path. The `typed()` Salsa query populates `PoolCache` and `CanonCache` after type checking, while `ImportsCache` is populated during import resolution.

**Invalidation is explicit**: `invalidate_file_caches()` must be called before re-type-checking a file to clear stale entries. Any future code path that triggers re-type-checking MUST also call `invalidate_file_caches()` — failing to do so will cause silent correctness bugs from stale cache reads.

```rust
// In the typed() query (simplified):
invalidate_file_caches(db, &path);  // Clear stale entries
let type_result = type_check(...);
db.pool_cache().store(&path, pool);  // Write new Pool
```

## Module Caching

Evaluated modules are cached:

```rust
pub struct ModuleCache {
    cache: HashMap<PathBuf, ModuleEvalResult>,
}
```

Prevents:
- Re-evaluating same module
- Memory bloat from duplicates

## Scope Cleanup

Scopes are cleaned up immediately:

```rust
fn eval_let(&mut self, name: Name, value: ExprId, body: ExprId) -> Result<Value, EvalError> {
    let value = self.eval_expr(value)?;

    self.env.push_scope();
    self.env.bind(name, value);

    let result = self.eval_expr(body);

    self.env.pop_scope();  // Immediate cleanup
    result
}
```

## Type Representation

Types avoid excessive boxing:

```rust
// Primitives are inline
Type::Int
Type::Bool

// Compound types box only where needed
Type::List(Box<Type>)  // One allocation
Type::Function { params: Vec<Type>, ret: Box<Type> }
```

## Memory Profiling (Planned)

> **Status: NOT IMPLEMENTED**
>
> Memory profiling is planned but not yet available.

The planned interface would be:

```bash
# Planned: Run with memory profiler
ORI_PROFILE_MEMORY=1 ori run large_file.ori

# Expected output:
Arena: 1.2 MB (12,000 expressions)
Interner: 0.3 MB (5,000 strings)
Values: 2.1 MB
Total: 3.6 MB
```

## Performance Annotations

The compiler uses Rust attributes to help the optimizer:

### `#[inline]`

Used on small, frequently-called functions:

```rust
impl Span {
    #[inline]
    pub const fn new(start: u32, end: u32) -> Self { ... }

    #[inline]
    pub const fn len(&self) -> u32 { ... }
}
```

### `#[track_caller]`

Used on panicking accessors for better error messages:

```rust
impl ExprArena {
    #[inline]
    #[track_caller]
    pub fn get_expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()]
    }
}
```

### `#[cold]`

Used on error factory functions to hint they're unlikely paths:

```rust
// eval/errors.rs - all 33 error factories marked #[cold]
#[cold]
pub fn division_by_zero() -> EvalError {
    EvalError::new("division by zero")
}

#[cold]
pub fn undefined_variable(name: &str) -> EvalError {
    EvalError::new(format!("undefined variable: {}", name))
}
```

## Guidelines

### Do

- Use arena allocation for AST nodes
- Intern all identifiers
- Use Arc for shared heap values
- Make small types Copy
- Clean up scopes immediately
- Mark error paths as `#[cold]`
- Add `#[track_caller]` to panicking functions
- Use `ensure_sufficient_stack` in recursive functions

### Don't

- Box individual expressions
- Store String in AST (use Name)
- Clone large structures unnecessarily
- Keep references to temporary values
- Leak memory in error paths
- Deeply recurse without stack safety
