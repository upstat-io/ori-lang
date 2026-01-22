# L: Rustc Query System Research

This document details the query system architecture used by rustc and how it applies to Sigil's V2 compiler.

---

## Red-Green Algorithm

### Concept

The red-green algorithm is Salsa's core mechanism for incremental recomputation:

- **Red**: Query result may have changed, needs revalidation
- **Green**: Query result is unchanged, safe to reuse

### How It Works

```
Initial State:
  All queries: Green (computed)

After Input Change:
  1. Mark changed input: Red
  2. Mark direct dependents: Red
  3. Recursively mark dependents: Red

On Query Access:
  If Green: Return cached result
  If Red:
    1. Recompute query
    2. If result == cached: Mark Green (early cutoff)
    3. If result != cached: Propagate Red to dependents
    4. Update cache
```

### Example Trace

```
Initial compile:
  tokens(file) = [...tokens...]      → Green
  parsed(file) = AST1                → Green
  typed(func)  = TypedAST1           → Green

User adds comment:
  1. file.text changed              → Mark tokens(file) Red
  2. Dependent: parsed(file)        → Mark Red
  3. Dependent: typed(func)         → Mark Red

Query tokens(file):
  - Recompute: [...tokens_with_comment...]
  - Different from cache → Stay Red, update cache

Query parsed(file):
  - Recompute: AST1 (comments stripped in AST)
  - Same as cache!  → Mark Green (early cutoff)

Query typed(func):
  - Check dependencies: parsed(file) is Green
  - No need to recompute → Stay Green, return cached
```

### Implementation

```rust
/// Query state
pub enum QueryState {
    /// Never computed
    NotComputed,
    /// Computed and valid
    Green {
        value: T,
        computed_at: Revision,
    },
    /// May need recomputation
    Red {
        cached: T,
        computed_at: Revision,
    },
}

/// Execute query with red-green tracking
pub fn execute_query<T: Eq>(
    db: &dyn Db,
    query: impl Query<Output = T>,
) -> T {
    let key = query.key();
    let mut state = db.get_state(key);

    match state {
        QueryState::NotComputed => {
            let value = query.compute(db);
            db.set_state(key, QueryState::Green {
                value: value.clone(),
                computed_at: db.current_revision(),
            });
            value
        }

        QueryState::Green { value, .. } => {
            // Fast path: already valid
            value.clone()
        }

        QueryState::Red { cached, .. } => {
            // Check if dependencies changed
            if db.dependencies_unchanged(key) {
                // Early cutoff: mark green
                db.set_state(key, QueryState::Green {
                    value: cached.clone(),
                    computed_at: db.current_revision(),
                });
                return cached;
            }

            // Recompute
            let value = query.compute(db);

            if value == cached {
                // Early cutoff: result unchanged
                db.set_state(key, QueryState::Green {
                    value,
                    computed_at: db.current_revision(),
                });
            } else {
                // Result changed: propagate
                db.set_state(key, QueryState::Green {
                    value: value.clone(),
                    computed_at: db.current_revision(),
                });
                db.invalidate_dependents(key);
            }

            value
        }
    }
}
```

---

## Dependency Tracking

### Automatic Tracking

Salsa automatically tracks which queries a query reads:

```rust
#[salsa::tracked]
pub fn typed_function(db: &dyn Db, func: Function) -> TypedFunction {
    // This read is automatically tracked
    let module = func.module(db);

    // This read is automatically tracked
    let arena = module.expr_arena(db);

    // This read is automatically tracked
    let imports = resolved_imports(db, module);

    // ... type checking logic ...
}

// Dependency graph:
// typed_function(func)
//   ├── func.module
//   ├── module.expr_arena
//   └── resolved_imports(module)
```

### Explicit Dependencies

Sometimes you need manual control:

```rust
#[salsa::tracked]
pub fn typed_function_with_deps(db: &dyn Db, func: Function) -> TypedFunction {
    // Register explicit dependency
    db.report_synthetic_read(SyntheticDep::StdlibVersion);

    // Normal computation
    let module = func.module(db);
    // ...
}
```

---

## Durability Levels

### rustc's Approach

rustc uses durability to optimize change detection:

```rust
pub enum Durability {
    /// Changes every edit (user code)
    LOW,

    /// Changes occasionally (build config)
    MEDIUM,

    /// Rarely changes (stdlib, dependencies)
    HIGH,
}
```

### Optimization

When only LOW durability inputs change:
- Skip validation of MEDIUM/HIGH durability queries
- Significant speedup for incremental builds

```rust
pub fn validate_query(&self, key: QueryKey) -> bool {
    let min_durability = self.min_input_durability(key);
    let changed_durability = self.max_changed_durability();

    if min_durability > changed_durability {
        // All inputs have higher durability than any change
        // Skip validation
        return true;
    }

    // Must validate
    self.validate_dependencies(key)
}
```

### Application to Sigil

```rust
// Standard library: HIGH durability
fn load_stdlib(db: &mut CompilerDb) {
    for file in STDLIB_FILES {
        let source = SourceFile::new(db, file.path, file.content);
        source.set_durability(db).to(Durability::HIGH);
    }
}

// User code: LOW durability (default)
fn load_user_file(db: &mut CompilerDb, path: &Path) {
    let content = fs::read_to_string(path)?;
    let source = SourceFile::new(db, path, content);
    // Durability::LOW is default
}
```

---

## Cycle Handling

### rustc's Approach

Some queries are legitimately cyclic (recursive types, mutual recursion). rustc handles this with:

1. **Cycle detection** during query execution
2. **Cycle recovery** for specific queries
3. **Error reporting** for invalid cycles

### Implementation

```rust
pub struct CycleDetector {
    /// Stack of currently executing queries
    stack: Vec<QueryKey>,
}

impl CycleDetector {
    pub fn enter(&mut self, key: QueryKey) -> Result<(), CycleError> {
        if self.stack.contains(&key) {
            let cycle = self.extract_cycle(&key);
            return Err(CycleError { cycle });
        }
        self.stack.push(key);
        Ok(())
    }

    pub fn exit(&mut self) {
        self.stack.pop();
    }
}

/// Query with cycle recovery
#[salsa::tracked(recovery_fn = recover_type_of)]
pub fn type_of(db: &dyn Db, expr: ExprId) -> TypeId {
    // ... may cycle for recursive types ...
}

fn recover_type_of(db: &dyn Db, cycle: &salsa::Cycle, expr: ExprId) -> TypeId {
    // Return error type to break cycle
    db.type_interner().intern(TypeKind::Error)
}
```

---

## Query Groups

### Organizing Queries

rustc groups related queries:

```rust
/// Parsing queries
#[salsa::query_group(SyntaxDatabase)]
pub trait SyntaxDb: SourceDatabase {
    fn tokens(&self, file: SourceFile) -> TokenList;
    fn parsed_module(&self, file: SourceFile) -> Module;
}

/// Type checking queries
#[salsa::query_group(TypeDatabase)]
pub trait TypeDb: SyntaxDb {
    fn typed_function(&self, func: Function) -> TypedFunction;
    fn typed_module(&self, module: Module) -> TypedModule;
}

/// Full database combines all
#[salsa::database(SyntaxDatabase, TypeDatabase, CodegenDatabase)]
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
}
```

---

## Parallelism with Salsa

### Parallel Query Execution

Salsa supports parallel queries:

```rust
// Multiple queries can execute in parallel
let typed_functions: Vec<_> = functions
    .par_iter()
    .map(|f| db.typed_function(*f))
    .collect();
```

### Synchronization

Salsa handles synchronization:
- Different queries on same key: Only one executes, others wait
- Different queries on different keys: Execute in parallel
- Write to input: Blocks until readers done

```rust
// Thread 1: Query typed_function(foo)
// Thread 2: Query typed_function(foo)
// → Thread 2 waits for Thread 1's result

// Thread 1: Query typed_function(foo)
// Thread 2: Query typed_function(bar)
// → Both execute in parallel
```

---

## Memory Management

### LRU Eviction

For memory-constrained environments, Salsa supports LRU eviction:

```rust
pub struct QueryStorage<T> {
    entries: HashMap<Key, Entry<T>>,
    lru: LruList,
    max_entries: usize,
}

impl<T> QueryStorage<T> {
    fn get(&mut self, key: Key) -> Option<&T> {
        let entry = self.entries.get(&key)?;
        self.lru.touch(key);  // Move to front
        Some(&entry.value)
    }

    fn insert(&mut self, key: Key, value: T) {
        if self.entries.len() >= self.max_entries {
            // Evict least recently used
            let evict = self.lru.pop_back();
            self.entries.remove(&evict);
        }

        self.entries.insert(key, Entry { value, .. });
        self.lru.push_front(key);
    }
}
```

### Garbage Collection

Salsa can garbage-collect unreachable cached values:

```rust
impl CompilerDb {
    /// Clean up old cached values
    pub fn gc(&mut self) {
        let current = self.current_revision();

        // Remove entries not accessed in last N revisions
        self.storage.retain(|_, entry| {
            current.since(entry.last_accessed) < GC_THRESHOLD
        });
    }
}
```

---

## Key Lessons for Sigil

1. **Red-green with early cutoff** - Most incremental wins come from early cutoff
2. **Durability levels** - Huge win for stdlib-heavy projects
3. **Automatic dependency tracking** - Less error-prone than manual tracking
4. **Parallel queries** - Free parallelism with query-based architecture
5. **Cycle handling** - Plan for recursive types from the start
