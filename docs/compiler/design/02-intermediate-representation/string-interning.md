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
    pub fn index(self) -> usize {
        self.0 as usize
    }
}
```

`Name` is just a 32-bit index. It's:
- `Copy` - Cheap to pass around
- `Eq, Hash` - Can be used in collections
- 4 bytes (vs ~24 bytes for String on 64-bit)

### Interner

```rust
#[derive(Clone, Debug, Default)]
pub struct Interner {
    /// All interned strings
    strings: Vec<String>,

    /// Map from string to Name
    lookup: HashMap<String, Name>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a string, returning its Name
    pub fn intern(&mut self, s: &str) -> Name {
        if let Some(&name) = self.lookup.get(s) {
            return name;
        }

        let name = Name(self.strings.len() as u32);
        self.strings.push(s.to_string());
        self.lookup.insert(s.to_string(), name);
        name
    }

    /// Resolve Name back to string
    pub fn resolve(&self, name: Name) -> &str {
        &self.strings[name.index()]
    }

    /// Check if string is interned
    pub fn get(&self, s: &str) -> Option<Name> {
        self.lookup.get(s).copied()
    }
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

## Thread Safety

The current `Interner` is not thread-safe. For parallel compilation:

```rust
// Option 1: Mutex-protected
struct SharedInterner {
    inner: Mutex<Interner>,
}

// Option 2: Concurrent hashmap
struct ConcurrentInterner {
    strings: DashMap<String, Name>,
    reverse: Vec<RwLock<String>>,
}
```

Currently, the compiler is single-threaded during lexing/parsing, so this isn't needed yet.

## Common Identifiers

Frequently used identifiers are pre-interned:

```rust
impl Interner {
    pub fn with_keywords() -> Self {
        let mut interner = Self::new();

        // Pre-intern keywords
        interner.intern("if");
        interner.intern("else");
        interner.intern("let");
        interner.intern("fn");
        // ...

        interner
    }
}
```

This ensures keywords have predictable Names (useful for fast keyword checking).

## Debugging

### Printing Names

When debugging, resolve Names back to strings:

```rust
fn debug_expr(interner: &Interner, expr: &Expr) {
    match &expr.kind {
        ExprKind::Ident(name) => {
            println!("Ident: {}", interner.resolve(*name));
        }
        // ...
    }
}
```

### Display Implementations

```rust
impl Name {
    pub fn display<'a>(&self, interner: &'a Interner) -> impl Display + 'a {
        interner.resolve(*self)
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
