# A: Data Structures Specification

This document specifies all core data structures for the V2 compiler.

---

## Interned Identifiers

### Name

```rust
/// Interned string identifier
///
/// Layout: 32-bit index split into shard (4 bits) + local index (28 bits)
/// - Bits 31-28: Shard index (0-15)
/// - Bits 27-0: Local index within shard
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Name(u32);

impl Name {
    /// Pre-interned empty string
    pub const EMPTY: Name = Name(0);

    /// Maximum local index per shard
    pub const MAX_LOCAL: u32 = 0x0FFF_FFFF;

    /// Number of shards
    pub const NUM_SHARDS: usize = 16;

    /// Create from shard and local index
    #[inline]
    pub const fn new(shard: u32, local: u32) -> Self {
        debug_assert!(shard < 16);
        debug_assert!(local <= Self::MAX_LOCAL);
        Name((shard << 28) | local)
    }

    /// Extract shard index
    #[inline]
    pub const fn shard(self) -> usize {
        (self.0 >> 28) as usize
    }

    /// Extract local index
    #[inline]
    pub const fn local(self) -> usize {
        (self.0 & Self::MAX_LOCAL) as usize
    }
}
```

**Memory:** 4 bytes (vs 24 bytes for `String`)

**Performance:**
- Equality: O(1) integer compare
- Hashing: O(1) direct use of u32
- Copy: trivially copyable

---

## Expression IDs and Ranges

### ExprId

```rust
/// Index into expression arena
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct ExprId(u32);

impl ExprId {
    pub const INVALID: ExprId = ExprId(u32::MAX);

    #[inline]
    pub const fn new(index: u32) -> Self {
        ExprId(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}
```

### ExprRange

```rust
/// Range of expressions in flattened list
///
/// Layout: 6 bytes total
/// - start: u32 (4 bytes) - start index in expr_lists
/// - len: u16 (2 bytes) - number of expressions
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct ExprRange {
    pub start: u32,
    pub len: u16,
}

impl ExprRange {
    pub const EMPTY: ExprRange = ExprRange { start: 0, len: 0 };

    #[inline]
    pub const fn new(start: u32, len: u16) -> Self {
        ExprRange { start, len }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn indices(&self) -> impl Iterator<Item = u32> {
        self.start..(self.start + self.len as u32)
    }
}
```

**Memory:** 6 bytes (vs 24 bytes for `Vec<ExprId>`)

---

## Type IDs

### TypeId

```rust
/// Interned type identifier
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct TypeId(u32);

impl TypeId {
    // Pre-interned primitive types
    pub const INT: TypeId = TypeId(0);
    pub const FLOAT: TypeId = TypeId(1);
    pub const BOOL: TypeId = TypeId(2);
    pub const STR: TypeId = TypeId(3);
    pub const CHAR: TypeId = TypeId(4);
    pub const BYTE: TypeId = TypeId(5);
    pub const VOID: TypeId = TypeId(6);
    pub const NEVER: TypeId = TypeId(7);
    pub const INFER: TypeId = TypeId(8);  // Placeholder during inference

    pub const FIRST_COMPOUND: u32 = 9;

    #[inline]
    pub const fn new(index: u32) -> Self {
        TypeId(index)
    }

    #[inline]
    pub const fn is_primitive(self) -> bool {
        self.0 < Self::FIRST_COMPOUND
    }
}
```

### TypeRange

```rust
/// Range of types (for function params, tuple elements)
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct TypeRange {
    pub start: u32,
    pub len: u16,
}
```

---

## Spans

### Span

```rust
/// Source location span
///
/// Layout: 8 bytes total
/// - start: u32 - byte offset from file start
/// - end: u32 - byte offset (exclusive)
#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
#[repr(C)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    #[inline]
    pub const fn new(start: u32, end: u32) -> Self {
        Span { start, end }
    }

    #[inline]
    pub const fn len(&self) -> u32 {
        self.end - self.start
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    #[inline]
    pub fn contains(&self, offset: u32) -> bool {
        offset >= self.start && offset < self.end
    }

    #[inline]
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}
```

---

## Expression Node

### Expr

```rust
/// Expression node
///
/// Memory layout: ~24-40 bytes depending on variant
#[derive(Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}
```

### ExprKind

```rust
/// Expression variants
///
/// All children are indices, not boxes
#[derive(Clone)]
pub enum ExprKind {
    // ===== Literals (no children) =====

    /// Integer literal: 42, 1_000
    Int(i64),

    /// Float literal: 3.14, 2.5e-8
    Float(f64),

    /// Boolean literal: true, false
    Bool(bool),

    /// String literal (interned)
    String(Name),

    /// Char literal: 'a', '\n'
    Char(char),

    /// Duration: 100ms, 5s
    Duration(Duration),

    /// Size: 4kb, 10mb
    Size(u64),

    // ===== References =====

    /// Variable reference
    Ident(Name),

    /// Config reference: $name
    Config(Name),

    // ===== Compound expressions =====

    /// Binary operation: left op right
    Binary {
        op: BinaryOp,
        left: ExprId,
        right: ExprId,
    },

    /// Unary operation: op operand
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },

    /// Function call: func(args...)
    Call {
        func: ExprId,
        args: ExprRange,
    },

    /// Method call: receiver.method(args...)
    MethodCall {
        receiver: ExprId,
        method: Name,
        args: ExprRange,
    },

    /// Field access: receiver.field
    Field {
        receiver: ExprId,
        field: Name,
    },

    /// Index access: receiver[index]
    Index {
        receiver: ExprId,
        index: ExprId,
    },

    // ===== Control flow =====

    /// Conditional: if cond then t else e
    If {
        cond: ExprId,
        then_branch: ExprId,
        else_branch: Option<ExprId>,
    },

    /// Pattern match
    Match {
        scrutinee: ExprId,
        arms: ArmRange,
    },

    /// For loop: for x in iter do/yield body
    For {
        binding: Name,
        iter: ExprId,
        body: ExprId,
        is_yield: bool,
    },

    /// Loop: loop(body)
    Loop {
        body: ExprId,
    },

    /// Block: { stmts; result }
    Block {
        stmts: StmtRange,
        result: Option<ExprId>,
    },

    // ===== Binding =====

    /// Let binding: let name = init
    Let {
        name: Name,
        ty: Option<TypeExprId>,
        init: ExprId,
        mutable: bool,
    },

    /// Lambda: params -> body
    Lambda {
        params: ParamRange,
        ret_ty: Option<TypeExprId>,
        body: ExprId,
    },

    // ===== Patterns (first-class) =====

    /// Pattern invocation: map(.over: x, .transform: f)
    Pattern {
        kind: PatternKind,
        args: PatternArgsId,
    },

    // ===== Collections =====

    /// List literal: [a, b, c]
    List(ExprRange),

    /// Map literal: {k: v, ...}
    Map(MapEntryRange),

    /// Struct literal: Point { x: 0, y: 0 }
    Struct {
        name: Name,
        fields: FieldInitRange,
    },

    /// Tuple: (a, b, c)
    Tuple(ExprRange),

    // ===== Control =====

    /// Return from function
    Return(Option<ExprId>),

    /// Break from loop
    Break(Option<ExprId>),

    /// Continue loop
    Continue,

    /// Await async operation
    Await(ExprId),

    /// Propagate error: expr?
    Try(ExprId),

    /// Range: start..end or start..=end
    Range {
        start: Option<ExprId>,
        end: Option<ExprId>,
        inclusive: bool,
    },
}
```

---

## Arena

### ExprArena

```rust
/// Contiguous storage for all expressions in a module
pub struct ExprArena {
    /// All expressions
    exprs: Vec<Expr>,

    /// Flattened expression lists (for Call args, List elements, etc.)
    expr_lists: Vec<ExprId>,

    /// Match arms
    arms: Vec<MatchArm>,

    /// Statements
    stmts: Vec<Stmt>,

    /// Pattern arguments
    pattern_args: Vec<PatternArgs>,

    /// Map entries
    map_entries: Vec<MapEntry>,

    /// Field initializers
    field_inits: Vec<FieldInit>,

    /// Parameters
    params: Vec<Param>,
}

impl ExprArena {
    /// Initial capacity hints
    pub fn new() -> Self {
        Self {
            exprs: Vec::with_capacity(4096),
            expr_lists: Vec::with_capacity(2048),
            arms: Vec::with_capacity(256),
            stmts: Vec::with_capacity(1024),
            pattern_args: Vec::with_capacity(512),
            map_entries: Vec::with_capacity(256),
            field_inits: Vec::with_capacity(256),
            params: Vec::with_capacity(256),
        }
    }

    /// Allocate expression, return ID
    #[inline]
    pub fn alloc(&mut self, expr: Expr) -> ExprId {
        let id = ExprId::new(self.exprs.len() as u32);
        self.exprs.push(expr);
        id
    }

    /// Get expression by ID
    #[inline]
    pub fn get(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()]
    }

    /// Allocate expression list, return range
    pub fn alloc_list(&mut self, exprs: impl IntoIterator<Item = ExprId>) -> ExprRange {
        let start = self.expr_lists.len() as u32;
        self.expr_lists.extend(exprs);
        let len = (self.expr_lists.len() as u32 - start) as u16;
        ExprRange::new(start, len)
    }

    /// Get expression list by range
    #[inline]
    pub fn get_list(&self, range: ExprRange) -> &[ExprId] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.expr_lists[start..end]
    }

    /// Reset arena for reuse (keeps capacity)
    pub fn reset(&mut self) {
        self.exprs.clear();
        self.expr_lists.clear();
        self.arms.clear();
        self.stmts.clear();
        self.pattern_args.clear();
        self.map_entries.clear();
        self.field_inits.clear();
        self.params.clear();
    }
}
```

---

## Memory Layout Summary

| Structure | V1 Size | V2 Size | Savings |
|-----------|---------|---------|---------|
| Identifier | 24 bytes (String) | 4 bytes (Name) | **83%** |
| Expression child | 8 bytes (Box) | 4 bytes (ExprId) | **50%** |
| Argument list | 24+ bytes (Vec) | 6 bytes (ExprRange) | **75%** |
| Type reference | 8+ bytes (clone) | 4 bytes (TypeId) | **50%+** |
| Span | 16 bytes | 8 bytes | **50%** |

**Estimated overall AST memory reduction: 60-70%**

---

## Alignment and Packing

All structures use `#[repr(C)]` for predictable layout:

```rust
// Struct field ordering: largest to smallest
#[repr(C)]
pub struct OptimalLayout {
    ptr: *const u8,     // 8 bytes
    count: u32,         // 4 bytes
    flags: u16,         // 2 bytes
    tag: u8,            // 1 byte
    // 1 byte padding at end
}
// Total: 16 bytes

// Bad ordering would add internal padding
#[repr(C)]
pub struct BadLayout {
    tag: u8,            // 1 byte
    // 7 bytes padding!
    ptr: *const u8,     // 8 bytes
    flags: u16,         // 2 bytes
    // 2 bytes padding
    count: u32,         // 4 bytes
}
// Total: 24 bytes (50% larger!)
```

---

## Thread Safety

All ID types (`Name`, `ExprId`, `TypeId`) are:
- `Copy` - trivially copyable
- `Send + Sync` - safe to share across threads
- No interior mutability

Arenas and interners use appropriate synchronization:
- `StringInterner`: Sharded `RwLock`s
- `TypeInterner`: `DashMap` for concurrent access
- `ExprArena`: Thread-local per parser instance
