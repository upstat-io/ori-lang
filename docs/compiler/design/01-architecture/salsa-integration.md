# Salsa Integration

The Ori compiler uses [Salsa](https://github.com/salsa-rs/salsa), a framework for on-demand, incremental computation. This document explains how Salsa is integrated.

## What is Salsa?

Salsa is a Rust framework that provides:

- **Memoization** - Query results are cached
- **Dependency tracking** - Salsa tracks which queries depend on which inputs
- **Incremental recomputation** - Only recompute what's affected by changes
- **Parallelism** - Independent queries can run in parallel

## Database Setup

The Salsa database is defined in `db.rs`:

```rust
#[salsa::db]
pub trait Db: salsa::Database {
    fn interner(&self) -> &Interner;
}

#[salsa::db]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
    interner: Interner,
}

#[salsa::db]
impl salsa::Database for Database {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        // Optional: log Salsa events for debugging
    }
}
```

## Input vs Tracked

Salsa distinguishes between inputs (external data) and tracked functions (computed data).

### Inputs

Inputs are the "ground truth" that comes from outside:

```rust
#[salsa::input]
pub struct SourceFile {
    #[return_ref]
    pub text: String,

    #[return_ref]
    pub path: PathBuf,
}
```

To create an input:
```rust
let file = SourceFile::new(&db, source_text, path);
```

To update an input (triggers recomputation):
```rust
file.set_text(&mut db).to(new_text);
```

### Tracked Functions

Tracked functions compute derived data:

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let text = file.text(db);
    lexer::tokenize(db, text)
}

#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult {
    let tokens = tokens(db, file);  // Dependency on tokens()
    parser::parse(db, tokens)
}
```

## Salsa Compatibility Requirements

All types that appear in Salsa query signatures or stored in Salsa-tracked structs must implement:

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct MyType { ... }
```

This is because Salsa needs to:
- Clone values for caching
- Compare values for early cutoff
- Hash values for storage

### Types That Work

```rust
// Primitives
i32, u64, bool, String

// Standard collections with compatible elements
Vec<T>, HashMap<K, V>

// Custom types with derived traits
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TokenList { ... }
```

### Types That Don't Work

```rust
// Function pointers (not Eq/Hash)
fn(i32) -> i32

// Trait objects (not Clone)
Box<dyn Trait>

// Arc<Mutex<T>> (can change without Salsa knowing)
Arc<Mutex<Value>>
```

### Solution: Interning

For complex types, use interning:

```rust
// Instead of storing Type directly
struct ExprType {
    ty: Type,  // Complex enum
}

// Intern types to get a comparable ID
#[salsa::interned]
struct InternedType {
    #[return_ref]
    ty: Type,
}

// Now we can compare/hash the ID
```

## Early Cutoff

Salsa's "early cutoff" optimization skips downstream recomputation when a query's output is unchanged:

```rust
// Change source text
file.set_text(&mut db).to("let x = 42");

// tokens() re-runs because input changed
// But if the tokens are the same as before...
// parsed() can skip running and return cached result!
```

This is why `Eq` is required - Salsa compares old and new outputs.

## Debugging Salsa

Enable Salsa event logging:

```rust
#[salsa::db]
impl salsa::Database for Database {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        let event = event();
        if let salsa::EventKind::WillExecute { database_key } = event.kind {
            eprintln!("[Salsa] will_execute: {:?}", database_key);
        }
    }
}
```

Or use environment variable:
```bash
ORI_DEBUG=salsa cargo run
```

## Common Patterns

### Accumulating Errors

Salsa queries should return errors as data, not panic:

```rust
#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypedModule {
    let parsed = parsed(db, file);
    let (types, errors) = type_check(&parsed);
    TypedModule { expr_types: types, errors }
}
```

### Parallel Queries

Independent queries can run in parallel:

```rust
// These can run in parallel
let tokens_a = tokens(db, file_a);
let tokens_b = tokens(db, file_b);
```

### Input Modification

Always use the setter pattern for inputs:

```rust
// Correct - Salsa knows about the change
file.set_text(&mut db).to(new_text);

// Wrong - bypasses Salsa's tracking
file.text = new_text;  // Won't compile anyway
```
