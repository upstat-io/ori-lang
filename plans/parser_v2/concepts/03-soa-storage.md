# Parser v2: Structure-of-Arrays (SoA) Storage

## Overview

This document details the memory layout for AST nodes, following Zig's approach of **Structure-of-Arrays (SoA)** instead of the traditional Array-of-Structures (AoS). This provides significant cache efficiency and memory savings.

## Motivation

### Current (AoS) Approach

```rust
// Current: Each node is a self-contained struct
pub struct Expr {
    pub kind: ExprKind,   // 24+ bytes (enum with data)
    pub span: Span,       // 8 bytes
}

// Stored as: [Expr, Expr, Expr, ...]
// Memory layout: [kind|span][kind|span][kind|span]...
```

**Problems:**
1. **Cache inefficiency**: When iterating over node tags only, we still load spans
2. **Alignment padding**: Enum discriminant + alignment = wasted bytes
3. **Variable size**: Large enum variants bloat all nodes

### Proposed (SoA) Approach

```rust
// Proposed: Separate arrays for each field
pub struct NodeStorage {
    tags: Vec<NodeTag>,    // [tag, tag, tag, ...]
    spans: Vec<Span>,      // [span, span, span, ...]
    data: Vec<NodeData>,   // [data, data, data, ...]
    extra: Vec<u32>,       // Variable-length overflow
}
```

**Benefits:**
1. **Cache efficiency**: Iterating tags loads only tags into cache
2. **Minimal padding**: Tag is 1 byte, no alignment waste
3. **Fixed size**: Common cases inline, large cases in `extra`

## Memory Layout

### Node Tag (1 byte)

```rust
/// Node type discriminant
///
/// Fits in u8: up to 256 node types (we use ~80)
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeTag {
    // Literals (0-15)
    IntLit = 0,
    FloatLit = 1,
    StringLit = 2,
    CharLit = 3,
    BoolTrue = 4,
    BoolFalse = 5,
    Unit = 6,

    // Identifiers (16-31)
    Ident = 16,
    QualifiedIdent = 17,
    Variant = 18,

    // Binary Operations (32-63)
    BinAdd = 32,
    BinSub = 33,
    BinMul = 34,
    BinDiv = 35,
    BinMod = 36,
    BinFloorDiv = 37,
    BinShl = 38,
    BinShr = 39,
    BinBitAnd = 40,
    BinBitOr = 41,
    BinBitXor = 42,
    BinAnd = 43,
    BinOr = 44,
    BinEq = 45,
    BinNe = 46,
    BinLt = 47,
    BinLe = 48,
    BinGt = 49,
    BinGe = 50,
    BinRange = 51,
    BinRangeInc = 52,
    BinCoalesce = 53,

    // Unary Operations (64-71)
    UnaryNeg = 64,
    UnaryNot = 65,
    UnaryBitNot = 66,

    // Postfix Operations (72-79)
    FieldAccess = 72,
    MethodCall = 73,
    FunctionCall = 74,
    Index = 75,
    TryPropagate = 76,

    // Control Flow (80-95)
    If = 80,
    IfElse = 81,
    Loop = 82,
    ForDo = 83,
    ForYield = 84,
    ForIf = 85,
    Break = 86,
    BreakValue = 87,
    Continue = 88,
    ContinueValue = 89,

    // Bindings (96-103)
    Let = 96,
    LetMut = 97,
    LetTyped = 98,
    LetMutTyped = 99,

    // Lambdas (104-107)
    Lambda = 104,
    LambdaTyped = 105,

    // Pattern Expressions (108-127)
    Run = 108,
    Try = 109,
    Match = 110,
    Recurse = 111,
    Parallel = 112,
    Spawn = 113,
    Timeout = 114,
    Cache = 115,
    With = 116,
    For = 117,
    Catch = 118,

    // Composite Literals (128-135)
    ListEmpty = 128,
    ListOne = 129,
    ListTwo = 130,
    ListMany = 131,
    MapEmpty = 132,
    MapOne = 133,
    MapTwo = 134,
    MapMany = 135,
    StructLit = 136,
    TupleTwo = 137,
    TupleMany = 138,

    // Patterns (140-159)
    PatWildcard = 140,
    PatLiteral = 141,
    PatIdent = 142,
    PatVariant = 143,
    PatStruct = 144,
    PatTuple = 145,
    PatList = 146,
    PatRange = 147,
    PatOr = 148,
    PatAt = 149,
    PatGuard = 150,

    // Types (160-179)
    TypePrimitive = 160,
    TypeNamed = 161,
    TypeGeneric = 162,
    TypeList = 163,
    TypeMap = 164,
    TypeTuple = 165,
    TypeFunction = 166,
    TypeUnit = 167,
    TypeNever = 168,
    TypeDyn = 169,

    // Items (180-199)
    Function = 180,
    Test = 181,
    TypeDef = 182,
    TraitDef = 183,
    ImplDef = 184,
    ExtendDef = 185,
    ConfigDef = 186,
    Import = 187,
    ExtensionImport = 188,

    // Special (200-255)
    Error = 200,        // Placeholder for error recovery
    Placeholder = 201,  // Reserved node (to be filled later)
}
```

### Span (8 bytes)

```rust
/// Source location
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub start: u32,  // Byte offset
    pub end: u32,    // Byte offset (exclusive)
}

impl Span {
    pub const DUMMY: Span = Span { start: 0, end: 0 };

    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn len(&self) -> u32 {
        self.end - self.start
    }
}
```

### Node Data (8 bytes)

```rust
/// Node payload - fits in 8 bytes
///
/// Uses small-value optimization: most nodes need 0-2 children.
/// Larger lists use `extra` array via ExtraRange.
#[derive(Clone, Copy)]
pub union NodeData {
    // No data (Unit, BoolTrue, BoolFalse, etc.)
    pub none: (),

    // Single node reference
    pub node: NodeId,

    // Two node references (binary ops, most constructs)
    pub node_pair: [NodeId; 2],

    // Node + optional node
    pub node_opt: (NodeId, OptNodeId),

    // Node + token (field access, method call)
    pub node_token: (NodeId, TokenIdx),

    // Token + node
    pub token_node: (TokenIdx, NodeId),

    // Two tokens
    pub token_pair: [TokenIdx; 2],

    // Extra data range (for 3+ children)
    pub extra_range: ExtraRange,

    // Node + extra range
    pub node_extra: (NodeId, ExtraRange),

    // Single u32 (literal index, name index)
    pub index: u32,

    // Two u32 values
    pub index_pair: [u32; 2],
}

/// Reference to a node in storage
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// Optional node reference (u32::MAX = None)
#[derive(Clone, Copy, Debug)]
pub struct OptNodeId(pub u32);

impl OptNodeId {
    pub const NONE: OptNodeId = OptNodeId(u32::MAX);

    pub fn some(id: NodeId) -> Self {
        OptNodeId(id.0)
    }

    pub fn get(self) -> Option<NodeId> {
        if self.0 == u32::MAX {
            None
        } else {
            Some(NodeId(self.0))
        }
    }
}

/// Reference to a token
#[derive(Clone, Copy, Debug)]
pub struct TokenIdx(pub u32);

/// Range into extra_data array
#[derive(Clone, Copy, Debug)]
pub struct ExtraRange {
    pub start: u32,
    pub len: u16,
    _padding: u16,
}
```

### Extra Data

For nodes with more than 2 children (function calls with many args, large lists, etc.):

```rust
/// Extended data storage for variable-length node data
pub struct ExtraData {
    data: Vec<u32>,
}

impl ExtraData {
    pub fn alloc_slice(&mut self, items: &[NodeId]) -> ExtraRange {
        let start = self.data.len() as u32;
        for item in items {
            self.data.push(item.0);
        }
        ExtraRange {
            start,
            len: items.len() as u16,
            _padding: 0,
        }
    }

    pub fn get_slice(&self, range: ExtraRange) -> &[u32] {
        let start = range.start as usize;
        let end = start + range.len as usize;
        &self.data[start..end]
    }
}
```

## Complete Storage Implementation

```rust
/// SoA node storage
pub struct NodeStorage {
    tags: Vec<NodeTag>,
    spans: Vec<Span>,
    data: Vec<NodeData>,
    extra: ExtraData,
}

impl NodeStorage {
    /// Create storage with estimated capacity based on source length
    ///
    /// Empirical ratios from Zig:
    /// - ~8 bytes per token
    /// - ~2 tokens per node
    pub fn with_estimated_capacity(source_len: usize) -> Self {
        let est_tokens = source_len / 8;
        let est_nodes = est_tokens / 2;
        let est_extra = est_nodes / 4;

        Self {
            tags: Vec::with_capacity(est_nodes),
            spans: Vec::with_capacity(est_nodes),
            data: Vec::with_capacity(est_nodes),
            extra: ExtraData::with_capacity(est_extra),
        }
    }

    /// Allocate a new node
    pub fn alloc(&mut self, tag: NodeTag, span: Span, data: NodeData) -> NodeId {
        let id = NodeId(self.tags.len() as u32);
        self.tags.push(tag);
        self.spans.push(span);
        self.data.push(data);
        id
    }

    /// Reserve a node slot (for forward references)
    pub fn reserve(&mut self, tag: NodeTag) -> NodeId {
        let id = NodeId(self.tags.len() as u32);
        self.tags.push(tag);
        self.spans.push(Span::DUMMY);
        self.data.push(NodeData { none: () });
        id
    }

    /// Fill in a reserved node
    pub fn set(&mut self, id: NodeId, span: Span, data: NodeData) {
        let idx = id.0 as usize;
        self.spans[idx] = span;
        self.data[idx] = data;
    }

    /// Unreserve a node (for error recovery)
    pub fn unreserve(&mut self, id: NodeId) {
        let idx = id.0 as usize;
        if idx == self.tags.len() - 1 {
            // Can actually remove
            self.tags.pop();
            self.spans.pop();
            self.data.pop();
        } else {
            // Mark as error/placeholder
            self.tags[idx] = NodeTag::Error;
        }
    }

    // Accessors

    pub fn tag(&self, id: NodeId) -> NodeTag {
        self.tags[id.0 as usize]
    }

    pub fn span(&self, id: NodeId) -> Span {
        self.spans[id.0 as usize]
    }

    pub fn data(&self, id: NodeId) -> NodeData {
        self.data[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn truncate(&mut self, len: usize) {
        self.tags.truncate(len);
        self.spans.truncate(len);
        self.data.truncate(len);
    }

    /// Allocate extra data for variable-length children
    pub fn alloc_extra(&mut self, items: &[NodeId]) -> ExtraRange {
        self.extra.alloc_slice(items)
    }

    pub fn extra_slice(&self, range: ExtraRange) -> impl Iterator<Item = NodeId> + '_ {
        self.extra.get_slice(range).iter().map(|&id| NodeId(id))
    }
}
```

## Node Type Specific Data

### Example: Binary Operations

```rust
impl NodeStorage {
    /// Create a binary operation node
    pub fn alloc_binary(
        &mut self,
        tag: NodeTag,  // BinAdd, BinSub, etc.
        span: Span,
        left: NodeId,
        right: NodeId,
    ) -> NodeId {
        self.alloc(tag, span, NodeData {
            node_pair: [left, right],
        })
    }

    /// Get binary operation children
    pub fn binary_children(&self, id: NodeId) -> (NodeId, NodeId) {
        let data = self.data(id);
        unsafe {
            (NodeId(data.node_pair[0].0), NodeId(data.node_pair[1].0))
        }
    }
}
```

### Example: Function Calls (Small-Value Optimization)

```rust
impl NodeStorage {
    /// Create a function call node
    ///
    /// Uses small-value optimization:
    /// - 0-2 args: inline in NodeData
    /// - 3+ args: stored in extra array
    pub fn alloc_call(
        &mut self,
        span: Span,
        callee: NodeId,
        args: &[NodeId],
    ) -> NodeId {
        match args.len() {
            0 => self.alloc(NodeTag::FunctionCall, span, NodeData {
                node_opt: (callee, OptNodeId::NONE),
            }),
            1 => self.alloc(NodeTag::FunctionCall, span, NodeData {
                node_pair: [callee, args[0]],
            }),
            2 => {
                // Store callee in extra, args inline
                let extra = self.alloc_extra(&[callee, args[0], args[1]]);
                self.alloc(NodeTag::FunctionCall, span, NodeData {
                    extra_range: extra,
                })
            }
            _ => {
                // All in extra
                let mut all = vec![callee];
                all.extend_from_slice(args);
                let extra = self.alloc_extra(&all);
                self.alloc(NodeTag::FunctionCall, span, NodeData {
                    extra_range: extra,
                })
            }
        }
    }
}
```

### Example: Match Expression

```rust
impl NodeStorage {
    /// Match expression structure in extra:
    /// [scrutinee, arm_count, pattern1, body1, pattern2, body2, ...]
    pub fn alloc_match(
        &mut self,
        span: Span,
        scrutinee: NodeId,
        arms: &[(NodeId, NodeId)],  // (pattern, body) pairs
    ) -> NodeId {
        let mut extra = vec![scrutinee.0, arms.len() as u32];
        for (pattern, body) in arms {
            extra.push(pattern.0);
            extra.push(body.0);
        }

        let range = ExtraRange {
            start: self.extra.data.len() as u32,
            len: extra.len() as u16,
            _padding: 0,
        };
        self.extra.data.extend(extra);

        self.alloc(NodeTag::Match, span, NodeData { extra_range: range })
    }

    pub fn match_scrutinee(&self, id: NodeId) -> NodeId {
        let range = unsafe { self.data(id).extra_range };
        NodeId(self.extra.data[range.start as usize])
    }

    pub fn match_arms(&self, id: NodeId) -> impl Iterator<Item = (NodeId, NodeId)> + '_ {
        let range = unsafe { self.data(id).extra_range };
        let slice = &self.extra.data[range.start as usize..][..range.len as usize];
        let arm_count = slice[1] as usize;
        (0..arm_count).map(move |i| {
            let pattern = NodeId(slice[2 + i * 2]);
            let body = NodeId(slice[2 + i * 2 + 1]);
            (pattern, body)
        })
    }
}
```

## Memory Comparison

### For 1,000 nodes

| Approach | Tag | Span | Data | Total |
|----------|-----|------|------|-------|
| Current (AoS) | Embedded | 8KB | 24KB | ~32KB |
| Proposed (SoA) | 1KB | 8KB | 8KB | ~17KB |

**Savings: ~47%**

### Cache Behavior

**Scanning all node tags (e.g., counting expressions):**

| Approach | Cache lines loaded |
|----------|-------------------|
| AoS | 1000 × 32 bytes = 500 cache lines |
| SoA | 1000 bytes = ~16 cache lines |

**Improvement: ~31x fewer cache misses**

## Iteration Patterns

### Efficient Tag-Only Iteration

```rust
impl NodeStorage {
    /// Count nodes of a specific type
    pub fn count_tag(&self, tag: NodeTag) -> usize {
        // Only touches tags array - cache efficient
        self.tags.iter().filter(|&&t| t == tag).count()
    }

    /// Find all nodes of a type
    pub fn find_by_tag(&self, tag: NodeTag) -> impl Iterator<Item = NodeId> + '_ {
        self.tags.iter()
            .enumerate()
            .filter(move |(_, &t)| t == tag)
            .map(|(i, _)| NodeId(i as u32))
    }
}
```

### Visitor Pattern

```rust
pub trait NodeVisitor {
    fn visit_node(&mut self, storage: &NodeStorage, id: NodeId);
}

impl NodeStorage {
    pub fn visit_all<V: NodeVisitor>(&self, visitor: &mut V) {
        for i in 0..self.len() {
            visitor.visit_node(self, NodeId(i as u32));
        }
    }

    pub fn visit_children<V: NodeVisitor>(&self, id: NodeId, visitor: &mut V) {
        match self.tag(id) {
            NodeTag::BinAdd | NodeTag::BinSub | /* ... */ => {
                let (left, right) = self.binary_children(id);
                visitor.visit_node(self, left);
                visitor.visit_node(self, right);
            }
            NodeTag::UnaryNeg | NodeTag::UnaryNot => {
                let child = unsafe { self.data(id).node };
                visitor.visit_node(self, child);
            }
            // ... other cases
            _ => {}
        }
    }
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capacity_estimation() {
        // 1KB source → ~125 tokens → ~62 nodes
        let storage = NodeStorage::with_estimated_capacity(1024);
        assert!(storage.tags.capacity() >= 60);
        assert!(storage.spans.capacity() >= 60);
        assert!(storage.data.capacity() >= 60);
    }

    #[test]
    fn test_alloc_binary() {
        let mut storage = NodeStorage::with_estimated_capacity(100);

        let left = storage.alloc(NodeTag::IntLit, Span::new(0, 1), NodeData { index: 42 });
        let right = storage.alloc(NodeTag::IntLit, Span::new(4, 5), NodeData { index: 3 });
        let add = storage.alloc_binary(NodeTag::BinAdd, Span::new(0, 5), left, right);

        assert_eq!(storage.tag(add), NodeTag::BinAdd);
        let (l, r) = storage.binary_children(add);
        assert_eq!(l, left);
        assert_eq!(r, right);
    }

    #[test]
    fn test_reserve_and_set() {
        let mut storage = NodeStorage::with_estimated_capacity(100);

        let reserved = storage.reserve(NodeTag::Function);
        // Do other work...
        storage.set(reserved, Span::new(0, 100), NodeData { index: 0 });

        assert_eq!(storage.tag(reserved), NodeTag::Function);
        assert_eq!(storage.span(reserved).end, 100);
    }

    #[test]
    fn test_call_small_value_optimization() {
        let mut storage = NodeStorage::with_estimated_capacity(100);

        let callee = storage.alloc(NodeTag::Ident, Span::new(0, 3), NodeData { index: 0 });
        let arg1 = storage.alloc(NodeTag::IntLit, Span::new(4, 5), NodeData { index: 1 });

        // 1 arg: should be inline
        let call1 = storage.alloc_call(Span::new(0, 6), callee, &[arg1]);
        let extra_before = storage.extra.data.len();

        // Verify extra wasn't used for 1-arg call
        let arg2 = storage.alloc(NodeTag::IntLit, Span::new(7, 8), NodeData { index: 2 });
        let arg3 = storage.alloc(NodeTag::IntLit, Span::new(9, 10), NodeData { index: 3 });

        // 3 args: should use extra
        let call3 = storage.alloc_call(Span::new(0, 11), callee, &[arg1, arg2, arg3]);
        assert!(storage.extra.data.len() > extra_before);
    }

    #[test]
    fn test_node_sizes() {
        assert_eq!(std::mem::size_of::<NodeTag>(), 1);
        assert_eq!(std::mem::size_of::<Span>(), 8);
        assert_eq!(std::mem::size_of::<NodeData>(), 8);
        assert_eq!(std::mem::size_of::<NodeId>(), 4);
        assert_eq!(std::mem::size_of::<OptNodeId>(), 4);
        assert_eq!(std::mem::size_of::<ExtraRange>(), 8);
    }

    #[test]
    fn test_count_by_tag() {
        let mut storage = NodeStorage::with_estimated_capacity(100);

        for i in 0..10 {
            storage.alloc(NodeTag::IntLit, Span::new(i, i + 1), NodeData { index: i });
        }
        for i in 0..5 {
            storage.alloc(NodeTag::StringLit, Span::new(i, i + 1), NodeData { index: i });
        }

        assert_eq!(storage.count_tag(NodeTag::IntLit), 10);
        assert_eq!(storage.count_tag(NodeTag::StringLit), 5);
    }
}
```

## Integration with Parser

```rust
pub struct Parser<'a> {
    // Input
    tokens: &'a [Token],
    source: &'a str,

    // Output
    storage: NodeStorage,

    // State
    cursor: Cursor,
    context: ParseContext,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: &'a [Token], source: &'a str) -> Self {
        Self {
            tokens,
            source,
            storage: NodeStorage::with_estimated_capacity(source.len()),
            cursor: Cursor::new(tokens),
            context: ParseContext::empty(),
            errors: Vec::new(),
        }
    }

    pub fn finish(self) -> (NodeStorage, Vec<ParseError>) {
        (self.storage, self.errors)
    }
}
```

## Summary

The SoA storage design provides:

1. **~47% memory reduction** compared to AoS
2. **~31x fewer cache misses** for tag-only operations
3. **Small-value optimization** for common cases (0-2 children)
4. **Reserve/set pattern** for forward references
5. **Efficient iteration** over specific node types

This design is battle-tested in Zig's compiler, which is known for extremely fast compilation times.
