---
section: "01"
title: Data-Oriented AST
status: in-progress
goal: Replace pointer-based AST with index-based, cache-friendly storage
sections:
  - id: "01.1"
    title: MultiArrayList-style Storage
    status: analysis-complete
  - id: "01.2"
    title: Index-based Node References
    status: already-implemented
  - id: "01.3"
    title: Extra Data Buffer
    status: already-implemented
  - id: "01.4"
    title: Pre-allocation Heuristics
    status: already-implemented
  - id: "01.5"
    title: Scratch Buffer Integration
    status: deferred
---

# Section 01: Data-Oriented AST

**Status:** 🔄 In Progress (analysis complete, evaluating SoA migration)
**Goal:** Achieve 2-3x memory efficiency and improved cache locality through Zig-inspired data layout
**Source:** Zig compiler (`lib/std/zig/Parse.zig`, `lib/std/zig/Ast.zig`)

---

## Current State Analysis (2026-02-04)

Investigation revealed that **Ori already has strong data-oriented foundations**:

### Existing Implementation (Already Optimal)

| Feature | Plan Target | Current Ori | Status |
|---------|-------------|-------------|--------|
| Index-based refs | `NodeIdx(u32)` | `ExprId(u32)` | ✅ Already 4 bytes |
| Extra data buffer | `Vec<u32>` | `expr_lists: Vec<ExprId>` | ✅ Flat lists |
| Pre-allocation | Source-based heuristics | `with_capacity(source_len/20)` | ✅ Implemented |
| Two-tier storage | 0-2 inline | `ExprList` inline 0-2 | ✅ Implemented |
| Range types | 8 bytes | `ExprRange { start: u32, len: u16 }` | ✅ 8 bytes |

### Current Memory Layout

```rust
// ExprArena - flat Vec-based storage (good)
pub struct ExprArena {
    exprs: Vec<Expr>,           // Main expression storage
    expr_lists: Vec<ExprId>,    // Flattened lists (extra buffer)
    stmts: Vec<Stmt>,           // Statement storage
    params: Vec<Param>,         // Parameter storage
    // ... 10+ more specialized vectors
}

// Expr - two fields (could be split for SoA)
pub struct Expr {
    pub kind: ExprKind,  // ~56+ bytes (large enum)
    pub span: Span,      // 8 bytes
}
```

### SoA Migration Evaluation

**Potential SoA structure:**
```rust
pub struct AstStorage {
    tags: Vec<ExprTag>,      // 1 byte each
    spans: Vec<Span>,        // 8 bytes each
    data: Vec<ExprData>,     // Variable per kind
}
```

**Trade-offs:**
| Aspect | Current (AoS) | SoA |
|--------|---------------|-----|
| Cache locality for spans | Load 64+ bytes | Load 8 bytes |
| Cache locality for kinds | Load 64+ bytes | Load 1 byte tag + data |
| Code complexity | Simple `get_expr(id)` | Multiple arrays to sync |
| Migration effort | N/A | ~40 ExprKind variants, entire codebase |
| Risk | N/A | High - touches type checker, evaluator |

**Decision:** The current implementation is already quite efficient. Full SoA migration would require significant refactoring across the entire codebase. Recommend **deferring** until profiling shows spans/tags are a bottleneck.

---

---

## Background

Traditional AST storage uses "Array of Structs" (AoS):
```rust
// Current: Each node is a contiguous struct
nodes: Vec<Node>  // [Node{tag, token, data}, Node{...}, ...]
```

Zig's breakthrough: "Struct of Arrays" (SoA) with MultiArrayList:
```rust
// Target: Separate arrays per field
tags:   Vec<NodeTag>    // [tag0, tag1, tag2, ...]
tokens: Vec<TokenIdx>   // [tok0, tok1, tok2, ...]
data:   Vec<NodeData>   // [data0, data1, data2, ...]
```

**Why this matters:**
- Cache lines load 64 bytes at a time
- Tag-only queries (common in traversal) don't load token/data
- Sequential access patterns get hardware prefetching
- Measured: 1-3 GB/s parsing speed in Zig

---

## 01.1 MultiArrayList-style Storage

**Goal:** Create `AstStorage` with separate arrays per node field

### Tasks

- [ ] Design `AstStorage` struct
  - [ ] Separate `tags: Vec<NodeTag>` (1 byte each)
  - [ ] Separate `main_tokens: Vec<TokenIdx>` (4 bytes each)
  - [ ] Separate `data: Vec<NodeData>` (8 bytes each)
  - [ ] Ensure alignment-friendly layout

- [ ] Implement accessor methods
  - [ ] `fn tag(&self, idx: NodeIdx) -> NodeTag`
  - [ ] `fn main_token(&self, idx: NodeIdx) -> TokenIdx`
  - [ ] `fn data(&self, idx: NodeIdx) -> &NodeData`
  - [ ] `fn node(&self, idx: NodeIdx) -> NodeView` (combined view)

- [ ] Add compile-time size assertions
  ```rust
  const_assert!(std::mem::size_of::<NodeTag>() == 1);
  const_assert!(std::mem::size_of::<NodeData>() == 8);
  ```

- [ ] Benchmark: Compare memory usage with current arena

### Design

```rust
/// Cache-friendly AST storage using Struct-of-Arrays layout
pub struct AstStorage {
    /// Node tags (1 byte each) - most frequently accessed
    tags: Vec<NodeTag>,

    /// Main token indices (4 bytes each)
    main_tokens: Vec<TokenIdx>,

    /// Node data (8 bytes each) - unions for different node kinds
    data: Vec<NodeData>,

    /// Variable-length extra data (see Section 01.3)
    extra: Vec<u32>,
}

/// Read-only view of a single node
pub struct NodeView<'a> {
    pub tag: NodeTag,
    pub main_token: TokenIdx,
    pub data: &'a NodeData,
}
```

---

## 01.2 Index-based Node References

**Goal:** Replace `ExprId` with raw `u32` indices for minimal overhead

### Tasks

- [ ] Define `NodeIdx` newtype
  ```rust
  #[derive(Copy, Clone, Eq, PartialEq, Hash)]
  pub struct NodeIdx(u32);
  ```

- [ ] Add sentinel values
  - [ ] `NodeIdx::ROOT = 0` for module root
  - [ ] `NodeIdx::NONE = u32::MAX` for optional nodes

- [ ] Create `OptionalNodeIdx` for nullable references
  ```rust
  pub struct OptionalNodeIdx(u32);  // MAX = none
  ```

- [ ] Update all node data to use indices
  - [ ] Replace `Box<Expr>` with `NodeIdx`
  - [ ] Replace `Option<Box<Expr>>` with `OptionalNodeIdx`
  - [ ] Replace `Vec<Expr>` with `ExtraRange` (see 01.3)

- [ ] Migrate existing `ExprId` usage
  - [ ] Audit all files using `ExprId`
  - [ ] Create migration shim if needed
  - [ ] Remove old `ExprId` once complete

### Benefits

| Metric | Before (ExprId) | After (NodeIdx) |
|--------|-----------------|-----------------|
| Size | 8 bytes (usize) | 4 bytes (u32) |
| Max nodes | ~18 quintillion | ~4 billion |
| Cache lines | 1 per 8 refs | 1 per 16 refs |

---

## 01.3 Extra Data Buffer

**Goal:** Store variable-length node data in a single flat buffer

### Tasks

- [ ] Define `ExtraIndex` and `ExtraRange`
  ```rust
  pub struct ExtraIndex(u32);  // Index into extra buffer
  pub struct ExtraRange {
      start: ExtraIndex,
      end: ExtraIndex,
  }
  ```

- [ ] Design `NodeData` union variants
  ```rust
  pub union NodeData {
      node: NodeIdx,
      opt_node: OptionalNodeIdx,
      token: TokenIdx,
      node_and_node: (NodeIdx, NodeIdx),
      node_and_extra: (NodeIdx, ExtraIndex),
      extra_range: ExtraRange,
      // ... more variants as needed
  }
  ```

- [ ] Implement list-to-span conversion
  ```rust
  fn list_to_span(&mut self, items: &[NodeIdx]) -> ExtraRange {
      let start = ExtraIndex(self.extra.len() as u32);
      for &item in items {
          self.extra.push(item.0);
      }
      let end = ExtraIndex(self.extra.len() as u32);
      ExtraRange { start, end }
  }
  ```

- [ ] Handle 0, 1, 2 items specially (Zig's Members pattern)
  - [ ] 0 items: Empty range
  - [ ] 1 item: Store directly in `NodeData::node`
  - [ ] 2 items: Store in `NodeData::node_and_node`
  - [ ] 3+ items: Store in extra buffer

### Design

```rust
/// Pattern for small vs large collections (from Zig)
pub struct Members {
    len: usize,
    data: NodeData,
    trailing: bool,
}

impl Members {
    pub fn to_span(&self, storage: &mut AstStorage) -> ExtraRange {
        match self.len {
            0 => storage.list_to_span(&[]),
            1 => {
                let node = unsafe { self.data.node };
                storage.list_to_span(&[node])
            }
            2 => {
                let (a, b) = unsafe { self.data.node_and_node };
                storage.list_to_span(&[a, b])
            }
            _ => unsafe { self.data.extra_range },
        }
    }
}
```

---

## 01.4 Pre-allocation Heuristics

**Goal:** Minimize reallocations with empirical capacity estimates

### Tasks

- [ ] Measure source-to-token ratio on Ori codebase
  - [ ] Target: ~8:1 (8 bytes source per token)
  - [ ] Collect statistics from `tests/spec/` files

- [ ] Measure token-to-node ratio
  - [ ] Target: ~2:1 (2 tokens per AST node)
  - [ ] Collect statistics from `tests/spec/` files

- [ ] Implement `with_capacity_for_source`
  ```rust
  impl AstStorage {
      pub fn with_capacity_for_source(source_len: usize) -> Self {
          let estimated_tokens = source_len / 8;
          let estimated_nodes = estimated_tokens / 2;
          let estimated_extra = estimated_nodes / 4;

          Self {
              tags: Vec::with_capacity(estimated_nodes),
              main_tokens: Vec::with_capacity(estimated_nodes),
              data: Vec::with_capacity(estimated_nodes),
              extra: Vec::with_capacity(estimated_extra),
          }
      }
  }
  ```

- [ ] Add telemetry to validate ratios
  - [ ] Track actual vs estimated capacities
  - [ ] Log when significant reallocation occurs
  - [ ] Adjust ratios based on real data

### Expected Memory Usage

For 1MB source file:
- Tokens: ~125K tokens × 8 bytes = 1 MB
- Nodes: ~62.5K nodes × 13 bytes = 812 KB
- Extra: ~15K entries × 4 bytes = 60 KB
- **Total: ~2 MB** (2x source size)

---

## 01.5 Scratch Buffer Integration

**Goal:** Integrate existing scratch buffer infrastructure for temporary allocations

### Tasks

- [ ] Review existing `scratch.rs` implementation
  - [ ] Verify LIFO semantics work for parser use cases
  - [ ] Check capacity management

- [ ] Replace ad-hoc `Vec` allocations with scratch buffer
  - [ ] Function argument lists
  - [ ] Pattern alternatives
  - [ ] Match arms
  - [ ] Generic parameters

- [ ] Implement scratch buffer pattern (from Zig)
  ```rust
  fn parse_argument_list(&mut self) -> Result<ExtraRange, ParseError> {
      let scratch_top = self.scratch.len();
      defer! { self.scratch.truncate(scratch_top) };

      while !self.check(&TokenKind::RParen) {
          let arg = self.parse_expr()?;
          self.scratch.push(arg);
          if !self.eat(&TokenKind::Comma) { break; }
      }

      let args = &self.scratch[scratch_top..];
      Ok(self.storage.list_to_span(args))
  }
  ```

- [ ] Benchmark: Compare allocation patterns before/after

---

## 01.6 Completion Checklist

- [ ] All AST nodes use index-based references
- [ ] `AstStorage` passes all existing parser tests
- [ ] Memory usage reduced by 40%+ on benchmark files
- [ ] No performance regression in parsing speed
- [ ] Documentation updated with new architecture

**Exit Criteria:**
- Benchmark showing 40%+ memory reduction
- All `tests/spec/` files parse correctly
- `cargo test` passes in `ori_parse`
