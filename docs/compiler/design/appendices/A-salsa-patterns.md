---
title: "Appendix A: Salsa Patterns"
description: "Ori Compiler Design — Appendix A: Salsa Patterns"
order: 1001
section: "Appendices"
---

# Appendix A: Salsa Patterns

Common patterns for working with Salsa in the Ori compiler.

## Current Usage

The Ori compiler currently uses a **minimal subset** of Salsa features:

| Feature | Status | Notes |
|---------|--------|-------|
| `#[salsa::tracked]` | ✅ Used | Basic query caching |
| `#[salsa::input]` with `#[return_ref]` | ✅ Used | SourceFile input |
| `#[salsa::db]` | ✅ Used | CompilerDb setup |
| `#[salsa::accumulator]` | ❌ Not Used | DiagnosticQueue instead |
| `#[salsa::interned]` | ❌ Not Used | Custom StringInterner |
| `#[salsa::tracked]` struct | ❌ Not Used | Regular structs |
| Cycle detection (`cycle_fn`) | ❌ Not Used | No cyclic queries |

This appendix documents both **currently used patterns** and **available Salsa features** for potential future use.

## Currently Used Patterns

### Basic Tracked Function

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let text = file.text(db);
    lexer::tokenize(db, &text)
}
```

### With Return Reference

```rust
#[salsa::tracked(return_ref)]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult {
    let tokens = tokens(db, file);
    parser::parse(db, &tokens)
}
```

## Available Salsa Features (Not Currently Used)

The following patterns are available in Salsa but not currently used in Ori. Documented for reference and potential future use.

### Accumulator Pattern

For collecting items across queries (Ori uses `DiagnosticQueue` instead):

```rust
#[salsa::accumulator]
pub struct Diagnostics(Diagnostic);

#[salsa::tracked]
pub fn check_file(db: &dyn Db, file: SourceFile) {
    let typed = typed(db, file);
    for error in &typed.errors {
        Diagnostics::push(db, error.to_diagnostic());
    }
}

// Later: collect all accumulated diagnostics
let all_diagnostics = check_file::accumulated::<Diagnostics>(db, file);
```

## Input Definition

```rust
#[salsa::input]
pub struct SourceFile {
    #[return_ref]
    pub text: String,

    #[return_ref]
    pub path: PathBuf,
}

// Create
let file = SourceFile::new(&db, source_text, path);

// Read
let text = file.text(&db);

// Update (triggers recomputation)
file.set_text(&mut db).to(new_text);
```

### Interned Values

For deduplication (Ori uses a custom `StringInterner` instead):

```rust
#[salsa::interned]
pub struct InternedType {
    #[return_ref]
    data: TypeData,
}

// Intern a type
let interned = InternedType::new(&db, type_data);

// Same data -> same ID
let interned2 = InternedType::new(&db, type_data);
assert_eq!(interned, interned2);

// Get data back
let data = interned.data(&db);
```

### Tracked Struct

For mutable state that Salsa tracks (not currently used):

```rust
#[salsa::tracked]
pub struct TypedExpr {
    pub expr: ExprId,

    #[return_ref]
    pub ty: Type,
}
```

## Database Setup (Currently Used)

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
        // Optional: log events for debugging
        if std::env::var("ORI_DEBUG").is_ok() {
            eprintln!("[Salsa] {:?}", event());
        }
    }
}

impl Db for Database {
    fn interner(&self) -> &Interner {
        &self.interner
    }
}
```

## Parallel Queries

```rust
// These can run in parallel automatically
let tokens_a = tokens(db, file_a);
let tokens_b = tokens(db, file_b);

// Salsa handles synchronization
```

## Cycle Detection (Reference)

Salsa detects query cycles. Ori queries are currently acyclic, but this pattern is available if needed:

```rust
// This would panic with cycle error
#[salsa::tracked]
fn a(db: &dyn Db) -> i32 {
    b(db) + 1
}

#[salsa::tracked]
fn b(db: &dyn Db) -> i32 {
    a(db) + 1  // Cycle!
}
```

Handle cycles explicitly:

```rust
#[salsa::tracked(cycle_fn = handle_cycle)]
fn resolve_type(db: &dyn Db, name: Name) -> Type {
    // ...
}

fn handle_cycle(_db: &dyn Db, _cycle: &[String]) -> Type {
    Type::Error
}
```

## Early Cutoff

Salsa skips downstream recomputation when output unchanged:

```rust
// File changes slightly but tokens are same
file.set_text(&mut db).to("let x = 42 ");  // Added space

// tokens() re-runs (input changed)
// But if TokenList is equal to before...
// parsed() can skip! (early cutoff)
```

Requirements for early cutoff:
- Output type must implement `Eq`
- Comparison must be efficient

## Testing with Salsa

```rust
#[test]
fn test_incremental() {
    let mut db = Database::default();

    // Initial compilation
    let file = SourceFile::new(&db, "let x = 1".into(), "test.ori".into());
    let result1 = typed(&db, file);

    // Modify and recompile
    file.set_text(&mut db).to("let x = 2".into());
    let result2 = typed(&db, file);

    // Verify types are same
    assert_eq!(result1.expr_types, result2.expr_types);
}
```

## Common Mistakes

### 1. Forgetting Eq on Output Types

```rust
// Wrong - won't compile
#[salsa::tracked]
fn query(db: &dyn Db) -> MyType { ... }

struct MyType { ... }  // Missing Eq!

// Right
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
struct MyType { ... }
```

### 2. Side Effects in Queries

```rust
// Wrong - side effect in query
#[salsa::tracked]
fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    println!("Tokenizing...");  // Side effect!
    // ...
}

// Right - use event logging
fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
    println!("[Salsa] {:?}", event());
}
```

### 3. Non-Deterministic Queries

```rust
// Wrong - non-deterministic
#[salsa::tracked]
fn random_value(db: &dyn Db) -> i32 {
    rand::random()  // Different each call!
}

// Right - deterministic from inputs only
```

### 4. Large Clones

```rust
// Avoid - clones entire AST
#[salsa::tracked]
fn big_query(db: &dyn Db) -> LargeAst { ... }

// Better - return reference
#[salsa::tracked(return_ref)]
fn big_query(db: &dyn Db) -> LargeAst { ... }
```
