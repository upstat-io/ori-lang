---
section: "08"
title: Salsa Integration
status: not-started
goal: Incremental compilation support via Salsa queries
sections:
  - id: "08.1"
    title: Derive Requirements
    status: not-started
  - id: "08.2"
    title: Query Design
    status: not-started
  - id: "08.3"
    title: TypedModule Output
    status: not-started
  - id: "08.4"
    title: Pool Sharing
    status: not-started
  - id: "08.5"
    title: Determinism Guarantees
    status: not-started
---

# Section 08: Salsa Integration

**Status:** Not Started
**Goal:** Full Salsa compatibility for incremental compilation
**Source:** Current Ori oric implementation, Salsa documentation

---

## Background

Salsa requires all query inputs and outputs to be:
- `Clone` — Can be cloned for caching
- `Eq` — Can be compared for change detection
- `PartialEq` — Required by Eq
- `Hash` — Can be hashed for memoization
- `Debug` — For debugging

Must NOT contain:
- `Arc<Mutex<T>>` — Not Eq/Hash
- Function pointers — Not Hash
- `dyn Trait` — Not Eq/Hash
- Non-deterministic operations (random, time, IO)

---

## 08.1 Derive Requirements

**Goal:** Ensure all types derive required traits

### Types That Must Derive

```rust
// Core types
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Idx(u32);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Tag;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeFlags;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Rank(u16);

// Error types
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeCheckError;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeErrorKind;

// Output types
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypedModule;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionSig;
```

### Tasks

- [ ] Audit all public types for required derives
- [ ] Add derives where missing
- [ ] Remove any Salsa-incompatible fields
- [ ] Add compile-time verification

---

## 08.2 Query Design

**Goal:** Design Salsa queries for type checking

### Query Structure

```rust
// In oric/src/query/mod.rs

/// Type check a module and return typed representation.
#[salsa::tracked]
pub fn type_check_module(
    db: &dyn Database,
    module_id: ModuleId,
) -> TypeCheckResult {
    let parsed = db.parse_module(module_id);
    let pool = db.type_pool();

    let mut engine = InferEngine::new(pool, &parsed.arena);

    // Register imports
    for import in &parsed.imports {
        let imported_module = db.type_check_module(import.module_id);
        engine.register_imports(&imported_module);
    }

    // Type check all items
    let typed = engine.check_module(&parsed.module);

    TypeCheckResult {
        module: typed,
        errors: engine.take_errors(),
    }
}

/// Get the type of a specific function.
#[salsa::tracked]
pub fn function_type(
    db: &dyn Database,
    func_id: FunctionId,
) -> Option<Idx> {
    let module = func_id.module(db);
    let result = db.type_check_module(module);
    result.module.function_type(func_id.name(db))
}

/// Get type errors for a module.
#[salsa::tracked]
pub fn module_type_errors(
    db: &dyn Database,
    module_id: ModuleId,
) -> Vec<TypeCheckError> {
    let result = db.type_check_module(module_id);
    result.errors
}
```

### Tasks

- [ ] Define `type_check_module` query
- [ ] Define `function_type` query
- [ ] Define `module_type_errors` query
- [ ] Ensure queries are incremental-friendly
- [ ] Add tests for query caching

---

## 08.3 TypedModule Output

**Goal:** Define the typed module output structure

### Design

```rust
/// The result of type checking a module.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypedModule {
    /// Function signatures by name.
    pub functions: FxHashMap<Name, FunctionSig>,
    /// Type definitions.
    pub types: FxHashMap<Name, TypeDef>,
    /// Trait definitions.
    pub traits: FxHashMap<Name, TraitDef>,
    /// Expression types (expr index -> type).
    pub expr_types: Vec<Idx>,
    /// Import information.
    pub imports: Vec<ImportInfo>,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct FunctionSig {
    pub name: Name,
    pub type_params: Vec<Name>,
    pub params: Vec<(Name, Idx)>,
    pub return_type: Idx,
    pub capabilities: Vec<Name>,
    pub span: Span,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeDef {
    pub name: Name,
    pub idx: Idx,
    pub kind: TypeDefKind,
    pub type_params: Vec<Name>,
    pub span: Span,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum TypeDefKind {
    Struct { fields: Vec<(Name, Idx)> },
    Enum { variants: Vec<VariantInfo> },
    Alias { target: Idx },
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TypeCheckResult {
    pub module: TypedModule,
    pub errors: Vec<TypeCheckError>,
}
```

### Tasks

- [ ] Create `ori_typeck/src/output/mod.rs`
- [ ] Define `TypedModule` with all fields
- [ ] Define `FunctionSig` with full signature info
- [ ] Define `TypeDef` for type exports
- [ ] Define `TypeCheckResult` wrapper
- [ ] Ensure all types are Salsa-compatible

---

## 08.4 Pool Sharing

**Goal:** Share the type pool across queries efficiently

### Design

```rust
/// Shared type pool for all queries in a database.
#[salsa::input]
pub fn type_pool(db: &dyn Database) -> Arc<RwLock<Pool>> {
    Arc::new(RwLock::new(Pool::new(db.string_interner())))
}

// Alternative: Per-module pools that get merged
#[salsa::tracked]
pub fn module_type_pool(
    db: &dyn Database,
    module_id: ModuleId,
) -> ModulePool {
    // Create a module-local pool
    // Later merge into global pool
}
```

### Considerations

1. **Global Pool**: Single shared pool across all modules
   - Pro: Maximum deduplication
   - Con: Contention, harder to invalidate

2. **Per-Module Pool**: Each module has its own pool
   - Pro: Better incremental behavior
   - Con: Cross-module types need translation

3. **Hybrid**: Global pool for primitives/builtins, per-module for user types
   - Pro: Best of both worlds
   - Con: More complex implementation

### Tasks

- [ ] Decide on pool sharing strategy
- [ ] Implement pool as Salsa input or tracked
- [ ] Handle cross-module type references
- [ ] Add tests for incremental scenarios

---

## 08.5 Determinism Guarantees

**Goal:** Ensure type checking is fully deterministic

### Requirements

1. **No Random**: No random number generation
2. **No Time**: No timestamps or time-based logic
3. **No IO**: No file reads during type checking
4. **Stable Ordering**: Use sorted collections for iteration
5. **Stable IDs**: Type variable IDs must be deterministic

### Implementation

```rust
impl InferEngine<'_> {
    /// Create fresh variable with deterministic ID.
    fn fresh_var(&mut self) -> Idx {
        // IDs are sequential from counter
        // Counter is reset per module or per-query
        let id = self.next_var_id;
        self.next_var_id += 1;
        self.pool.intern(Tag::Var, id)
    }
}

impl TypedModule {
    /// Ensure stable iteration order.
    pub fn functions_sorted(&self) -> Vec<(&Name, &FunctionSig)> {
        let mut items: Vec<_> = self.functions.iter().collect();
        items.sort_by_key(|(name, _)| *name);
        items
    }
}
```

### Tasks

- [ ] Audit for non-deterministic operations
- [ ] Use BTreeMap where iteration order matters
- [ ] Reset variable counters appropriately
- [ ] Add determinism tests (same input = same output)

---

## 08.6 Error Handling Integration

**Goal:** Ensure errors work with Salsa's error accumulation

### Design

```rust
/// Accumulator for type errors.
#[salsa::accumulator]
pub struct TypeErrors(TypeCheckError);

#[salsa::tracked]
pub fn type_check_module(
    db: &dyn Database,
    module_id: ModuleId,
) -> TypedModule {
    let mut engine = InferEngine::new(...);

    let typed = engine.check_module(...);

    // Accumulate errors
    for error in engine.take_errors() {
        TypeErrors::push(db, error);
    }

    typed
}

/// Get all type errors for a module.
pub fn all_type_errors(db: &dyn Database, module_id: ModuleId) -> Vec<TypeCheckError> {
    type_check_module::accumulated::<TypeErrors>(db, module_id)
}
```

### Tasks

- [ ] Define `TypeErrors` accumulator
- [ ] Push errors during type checking
- [ ] Add query to retrieve accumulated errors
- [ ] Test error accumulation across modules

---

## 08.7 Completion Checklist

- [ ] All public types derive required traits
- [ ] `type_check_module` query defined
- [ ] `TypedModule` output structure complete
- [ ] Pool sharing strategy implemented
- [ ] Determinism verified with tests
- [ ] Error accumulation working
- [ ] Incremental compilation tested

**Exit Criteria:** Type checking integrates cleanly with Salsa. Changing one function only re-type-checks affected code. All queries are deterministic and cacheable.
