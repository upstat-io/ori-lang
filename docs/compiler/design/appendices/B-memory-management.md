# Appendix B: Memory Management

Memory management strategies used in the Sigil compiler.

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

## Memory Profiling

For large programs:

```bash
# Run with memory profiler
SIGIL_PROFILE_MEMORY=1 sigil run large_file.si

# Output
Arena: 1.2 MB (12,000 expressions)
Interner: 0.3 MB (5,000 strings)
Values: 2.1 MB
Total: 3.6 MB
```

## Guidelines

### Do

- Use arena allocation for AST nodes
- Intern all identifiers
- Use Arc for shared heap values
- Make small types Copy
- Clean up scopes immediately

### Don't

- Box individual expressions
- Store String in AST (use Name)
- Clone large structures unnecessarily
- Keep references to temporary values
- Leak memory in error paths
