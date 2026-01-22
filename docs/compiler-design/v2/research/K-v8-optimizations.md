# K: V8 Optimizations Research

This document summarizes optimization techniques from V8 (Chrome's JavaScript engine) applicable to Sigil's compiler.

---

## Lazy Parsing

### Concept

V8 doesn't fully parse functions until they're called. This dramatically reduces startup time for large codebases.

### V8's Two-Stage Approach

```
Stage 1: PreParsing (Lazy)
- Scan for function boundaries
- Record parameter count and scope info
- Skip function body entirely
- O(n) where n = source length, not AST complexity

Stage 2: Full Parsing (On Demand)
- Parse function body when first called
- Generate AST and bytecode
- Cache for subsequent calls
```

### Application to Sigil

```rust
/// Lazy function representation
pub struct LazyFunction {
    /// Always parsed: signature
    pub name: Name,
    pub params: Vec<(Name, TypeId)>,
    pub return_type: TypeId,
    pub capabilities: Vec<Name>,

    /// Token range for body (not yet parsed)
    pub body_tokens: TokenRange,

    /// Parsed body (populated on demand)
    pub body: OnceCell<ExprId>,
}

impl LazyFunction {
    /// Parse body on first access
    pub fn get_body(&self, db: &dyn Db) -> ExprId {
        *self.body.get_or_init(|| {
            let tokens = db.get_token_range(self.body_tokens);
            parse_function_body(db, &tokens)
        })
    }
}
```

### Benefits for LSP

| Operation | With Lazy Parsing | Without |
|-----------|-------------------|---------|
| Open file | Parse signatures only | Full parse |
| Hover on function | Return type immediately | Must wait for parse |
| Type check function | Parse single body | Parse all bodies |

### Implementation Strategy

```rust
/// Skip function body during initial parse
fn skip_function_body(parser: &mut Parser) -> TokenRange {
    let start = parser.position();

    // Track brace/paren nesting
    let mut depth = 0;

    loop {
        match parser.current().kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => {
                depth += 1;
            }
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                if depth == 0 {
                    // End of function body
                    break;
                }
                depth -= 1;
            }
            TokenKind::At if depth == 0 => {
                // Next function definition
                break;
            }
            TokenKind::Eof => break,
            _ => {}
        }
        parser.advance();
    }

    TokenRange {
        start,
        end: parser.position(),
    }
}
```

---

## Hidden Classes (Shapes)

### Concept

V8 optimizes object property access by grouping objects with the same "shape" (set of properties in the same order).

### V8's Approach

```javascript
// These objects share the same hidden class
const a = { x: 1, y: 2 };
const b = { x: 3, y: 4 };

// Property access is O(1) via offset lookup
a.x  // Offset 0 in hidden class
a.y  // Offset 1 in hidden class
```

### Application to Sigil: Struct Layouts

```rust
/// Compile-time struct layout
pub struct StructLayout {
    pub type_id: TypeId,
    pub field_names: Vec<Name>,
    pub field_offsets: Vec<u32>,
    pub field_types: Vec<TypeId>,
}

/// Runtime struct with indexed fields
pub struct StructValue {
    pub layout_id: LayoutId,
    pub fields: Vec<Value>,  // Indexed by offset, not by name
}

impl StructValue {
    /// O(1) field access
    pub fn get_field(&self, offset: u32) -> &Value {
        &self.fields[offset as usize]
    }
}

/// At compile time, convert name to offset
fn compile_field_access(
    receiver: ExprId,
    field: Name,
    layout: &StructLayout,
) -> CompiledAccess {
    let offset = layout.field_offsets[layout.field_names.iter()
        .position(|&n| n == field)
        .unwrap()];

    CompiledAccess::FieldOffset(receiver, offset)
}
```

### Memory Comparison

| Approach | Field Access | Memory per Instance |
|----------|--------------|---------------------|
| HashMap<Name, Value> | O(1) hash | 48+ bytes overhead |
| Vec<Value> + LayoutId | O(1) index | 4 bytes overhead |

---

## Allocation Sinking

### Concept

V8's TurboFan compiler can "sink" allocations into the branches that actually need them, eliminating allocation in hot paths.

### Example

```javascript
function example(cond) {
    const obj = { x: 1, y: 2 };  // May not need allocation
    if (cond) {
        return obj.x;  // Only need x, not the object
    }
    return obj;  // Object escapes here
}
```

V8 transforms this to:
```javascript
function example(cond) {
    if (cond) {
        return 1;  // No allocation!
    }
    return { x: 1, y: 2 };  // Allocation only here
}
```

### Application to Sigil: Escape Analysis

```rust
/// Determine if a value escapes its scope
pub fn analyze_escapes(func: &TypedFunction, arena: &ExprArena) -> EscapeInfo {
    let mut info = EscapeInfo::new();

    for let_binding in func.let_bindings() {
        let escapes = check_escapes(let_binding, func, arena);
        info.record(let_binding.id, escapes);
    }

    info
}

/// Check if a let binding escapes
fn check_escapes(binding: &LetBinding, func: &TypedFunction, arena: &ExprArena) -> bool {
    let mut visitor = EscapeVisitor::new(binding.name);
    visitor.visit(func.body, arena);

    visitor.escapes()
}

struct EscapeVisitor {
    target: Name,
    escaped: bool,
}

impl EscapeVisitor {
    fn visit(&mut self, expr: ExprId, arena: &ExprArena) {
        let node = arena.get(expr);

        match &node.kind {
            // Return escapes
            ExprKind::Return(Some(e)) if self.references_target(*e, arena) => {
                self.escaped = true;
            }

            // Passed to unknown function escapes
            ExprKind::Call { args, .. } => {
                for arg in arena.get_list(*args) {
                    if self.references_target(*arg, arena) {
                        self.escaped = true;
                    }
                }
            }

            // Stored in collection escapes
            ExprKind::List(elems) | ExprKind::Map(..) => {
                self.escaped = true;
            }

            _ => {
                // Recurse into children
                self.visit_children(expr, arena);
            }
        }
    }
}
```

### Codegen Optimization

```rust
/// Generate code with escape analysis
fn codegen_let(
    ctx: &mut CodegenContext,
    binding: &LetBinding,
    escape_info: &EscapeInfo,
) -> CCode {
    if escape_info.escapes(binding.id) {
        // Must heap allocate
        ctx.emit(format!(
            "{type}* {name} = malloc(sizeof({type}));",
            type = ctx.c_type(binding.ty),
            name = binding.name,
        ))
    } else {
        // Can stack allocate
        ctx.emit(format!(
            "{type} {name}_storage; {type}* {name} = &{name}_storage;",
            type = ctx.c_type(binding.ty),
            name = binding.name,
        ))
    }
}
```

---

## Inline Caching

### Concept

V8 caches the result of property lookups at the call site, making subsequent accesses faster.

### Application to Sigil: Method Resolution Cache

```rust
/// Cached method resolution
pub struct MethodCache {
    entries: Vec<CacheEntry>,
}

struct CacheEntry {
    /// Receiver type
    receiver_type: TypeId,
    /// Method name
    method: Name,
    /// Resolved implementation
    impl_id: ImplId,
}

impl MethodCache {
    /// Lookup with caching
    pub fn resolve(
        &mut self,
        db: &dyn Db,
        receiver: TypeId,
        method: Name,
    ) -> Option<ImplId> {
        // Check cache first
        for entry in &self.entries {
            if entry.receiver_type == receiver && entry.method == method {
                return Some(entry.impl_id);
            }
        }

        // Cache miss - resolve and cache
        let impl_id = db.resolve_method(receiver, method)?;

        self.entries.push(CacheEntry {
            receiver_type: receiver,
            method,
            impl_id,
        });

        Some(impl_id)
    }
}
```

---

## Deoptimization

### Concept

V8 can "bail out" from optimized code when assumptions are violated, falling back to slower but correct code.

### Application to Sigil: Pattern Specialization

```rust
/// Specialized pattern with fallback
pub enum SpecializedPattern {
    /// Optimized for specific types
    Specialized {
        signature: PatternSignature,
        code: CompiledTemplate,
        /// Conditions that must hold
        assumptions: Vec<Assumption>,
    },

    /// Generic fallback
    Generic {
        code: CompiledTemplate,
    },
}

#[derive(Clone)]
pub enum Assumption {
    /// Type is exactly this (not a subtype)
    ExactType(TypeId),
    /// Collection has at least N elements
    MinLength(usize),
    /// No concurrent modification
    NoAliasing,
}

impl SpecializedPattern {
    /// Check if specialization still valid
    pub fn check_assumptions(&self, ctx: &RuntimeContext) -> bool {
        match self {
            Self::Specialized { assumptions, .. } => {
                assumptions.iter().all(|a| a.holds(ctx))
            }
            Self::Generic { .. } => true,
        }
    }
}
```

---

## Key Takeaways for Sigil

1. **Lazy parsing** - Critical for LSP performance
2. **Struct layouts** - O(1) field access via compile-time offsets
3. **Escape analysis** - Enable stack allocation when possible
4. **Method caching** - Speed up repeated method calls
5. **Specialization with bailout** - Optimize common cases, handle edge cases
