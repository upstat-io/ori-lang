---
title: "String Interning"
description: "Ori Compiler Design — String Interning"
order: 203
section: "Intermediate Representation"
---

# String Interning

The Ori compiler interns all identifiers to enable O(1) comparison and reduce memory usage.

## What is String Interning?

String interning stores each unique string once and represents it with a small ID. When the same string appears multiple times, they all share the same ID.

```rust
let interner = Interner::new();

let name1 = interner.intern("foo");  // Name(0)
let name2 = interner.intern("foo");  // Name(0) - same!
let name3 = interner.intern("bar");  // Name(1)

assert_eq!(name1, name2);  // O(1) comparison
```

## Implementation

### Name Type

```rust
/// Interned string identifier
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct Name(pub u32);

impl Name {
    pub const EMPTY: Name = Name(0);  // Pre-interned empty string ""

    pub fn index(self) -> usize {
        self.0 as usize
    }
}
```

`Name` is just a 32-bit index. `Name::EMPTY` is the pre-interned empty string, used as the default value. It's:
- `Copy` - Cheap to pass around
- `Eq, Hash` - Can be used in collections
- 4 bytes (vs ~24 bytes for String on 64-bit)

### StringInterner

The interner uses a 16-shard concurrent design with per-shard `RwLock` for thread-safe access:

```rust
pub struct StringInterner {
    shards: [RwLock<InternShard>; 16],  // 16 lock-striped shards
    total_count: AtomicUsize,            // O(1) len()
}

struct InternShard {
    map: FxHashMap<&'static str, u32>,   // Lookup by string
    strings: Vec<&'static str>,          // Reverse lookup by local index
}
```

Strings are leaked via `Box::leak()` for `'static` lifetime, enabling zero-copy storage as both map keys and values.

**Name encoding**: `Name(u32)` packs a shard index (bits 31-28) and local index (bits 27-0), giving 16 shards of up to ~268M strings each.

```rust
impl StringInterner {
    /// Intern a string (thread-safe, may allocate)
    pub fn intern(&self, s: &str) -> Name { ... }

    /// Fallible version with overflow detection
    pub fn try_intern(&self, s: &str) -> Result<Name, InternError> { ... }

    /// Resolve Name back to &str
    pub fn lookup(&self, name: Name) -> &str { ... }

    /// Resolve to &'static str (zero-copy)
    pub fn lookup_static(&self, name: Name) -> &'static str { ... }
}
```

## Usage

### During Lexing

The lexer interns identifiers as it tokenizes:

```rust
impl Lexer {
    fn scan_identifier(&mut self) -> Token {
        let text = self.read_while(is_identifier_char);
        let name = self.interner.intern(&text);
        Token::Ident(name)
    }
}
```

### In the AST

AST nodes store Names, not Strings:

```rust
struct Function {
    name: Name,           // Not String
    params: Vec<Param>,
    body: ExprId,
}

struct Param {
    name: Name,           // Not String
    ty: Type,
}

enum ExprKind {
    Ident(Name),          // Variable reference
    Field { name: Name }, // Field access
    // ...
}
```

### In the Type Checker

Type names are also interned:

```rust
enum Type {
    Named(Name),  // User-defined type name
    // ...
}
```

### In the Evaluator

Environment uses Names for variable lookup:

```rust
struct Environment {
    scopes: Vec<HashMap<Name, Value>>,
}

impl Environment {
    fn get(&self, name: Name) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(value) = scope.get(&name) {
                return Some(value);
            }
        }
        None
    }
}
```

## Benefits

### 1. Fast Comparison

```rust
// String comparison: O(n) where n = string length
"long_identifier_name" == "long_identifier_name"

// Name comparison: O(1)
Name(42) == Name(42)
```

### 2. Reduced Memory

Without interning:
```
let x = "foo"  // "foo" allocated
let y = "foo"  // Another "foo" allocated
```

With interning:
```
let x = intern("foo")  // Name(0), "foo" stored once
let y = intern("foo")  // Name(0), reuses existing
```

### 3. Hashable

Names can be used in HashMaps without hashing strings:

```rust
// HashMap<String, Value> - must hash entire string
// HashMap<Name, Value>   - hashes 4-byte integer
```

### 4. Salsa Compatible

Name is `Copy + Eq + Hash`, perfect for Salsa queries.

## Fallible Interning and Overflow

The `try_intern()` and `try_intern_owned()` methods return `Result<Name, InternError>` instead of panicking on overflow. `InternError::ShardOverflow` is produced when a shard's local index exceeds `u32` capacity. The infallible `intern()` and `intern_owned()` methods unwrap internally and are appropriate for normal compilation where overflow is not expected.

`intern_owned()` accepts a `String` directly, avoiding a re-allocation when the caller already has an owned string (e.g., string literal processing in the lexer).

## Thread Safety

The `StringInterner` is thread-safe by design. Each shard is protected by its own `RwLock`, so concurrent reads to different shards never contend. The hash-based shard selection distributes identifiers evenly across shards.

`SharedInterner(Arc<StringInterner>)` is a newtype wrapper for cross-thread sharing. The test runner shares a single `SharedInterner` across all parallel test threads, avoiding per-file re-interning of common identifiers. `SharedInterner` dereferences to `StringInterner`, so all methods are available transparently.

## Pre-Interned Identifiers

The `StringInterner::new()` constructor pre-interns ~60 keywords and common identifiers via a private `pre_intern_keywords()` method:

```rust
// Pre-interned at construction time (predictable Name values):
// Keywords: if, else, let, fn, match, impl, trait, use, pub, self, type, ...
// Built-in types: int, float, bool, str, char, byte, Never, Option, Result, ...
// Common identifiers: main, print, len, compare, panic, assert, assert_eq, ...
```

This ensures keywords have predictable `Name` values, enabling fast keyword checking during lexing without string comparison.

## Debugging

### Printing Names

When debugging, resolve Names back to strings:

```rust
fn debug_expr(interner: &Interner, expr: &Expr) {
    match &expr.kind {
        ExprKind::Ident(name) => {
            println!("Ident: {}", interner.lookup(*name));
        }
        // ...
    }
}
```

### Display Implementations

```rust
impl Name {
    pub fn display<'a>(&self, interner: &'a Interner) -> impl Display + 'a {
        interner.lookup(*self)
    }
}
```

## StringLookup Trait

The `StringLookup` trait provides a minimal interface for Name resolution, avoiding
circular dependencies between crates:

```rust
// In ori_ir::interner
pub trait StringLookup {
    fn lookup(&self, name: Name) -> &str;
}

impl StringLookup for StringInterner {
    fn lookup(&self, name: Name) -> &str {
        StringInterner::lookup(self, name)
    }
}
```

This trait is re-exported from `ori_patterns` and used by `Value::type_name_with_interner()`:

```rust
// In ori_patterns::value
pub fn type_name_with_interner<I: StringLookup>(&self, interner: &I) -> Cow<'static, str> {
    match self {
        Value::Struct(s) => Cow::Owned(interner.lookup(s.type_name).to_string()),
        _ => Cow::Borrowed(self.type_name()),
    }
}
```

This pattern allows the value crate to resolve struct type names without depending
on the full interner implementation.

## Limitations

1. **Interned strings are never freed** - The interner only grows
2. **Requires interner access** - Must pass interner to display names
3. **Global state** - Interner must be accessible throughout compilation

These are acceptable tradeoffs for the performance benefits.
