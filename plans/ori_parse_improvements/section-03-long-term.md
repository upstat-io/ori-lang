---
section: "03"
title: Long-Term Architecture
status: not-started
phase: 3
goal: Major architectural investments for IDE support and performance at scale
reference_parsers:
  - TypeScript (incremental parsing)
  - Roc (SIMD preprocessing)
sections:
  - id: "3.1"
    title: Incremental Parsing
    status: not-started
  - id: "3.2"
    title: Language Server Integration
    status: not-started
  - id: "3.3"
    title: SIMD Preprocessing
    status: not-started
  - id: "3.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 03: Long-Term Architecture

**Status:** 📋 Planned
**Goal:** Major architectural investments for IDE support and performance at scale
**Timeline:** 4-8 weeks
**Dependencies:** Section 02 (snapshots, storage improvements)

---

## Overview

These improvements require significant architectural work but unlock critical capabilities:

| Improvement | Source | Benefit |
|-------------|--------|---------|
| Incremental Parsing | TypeScript | Sub-100ms IDE response times |
| Language Server Integration | LSP standard | First-class IDE experience |
| SIMD Preprocessing | Roc | 2-5x faster tokenization for large files |

---

## 3.1 Incremental Parsing

> **Reference**: TypeScript's `IncrementalParser` reuses unchanged AST subtrees, achieving <50ms re-parse on edits.

### Motivation

When a user edits a file in an IDE:
- **Without incremental**: Re-parse entire file (100ms-1s for large files)
- **With incremental**: Re-parse only affected region (~10-50ms)

This is **essential** for responsive IDE features like:
- Real-time error highlighting
- Autocomplete
- Hover information
- Go-to-definition

### Architecture Overview

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────┐
│  Edit Event     │ ──▶ │  Compute     │ ──▶ │  Selective  │
│  (position,     │     │  Change      │     │  Re-parse   │
│   old/new text) │     │  Region      │     │  Region     │
└─────────────────┘     └──────────────┘     └─────────────┘
                                                    │
                        ┌──────────────┐            │
                        │  Update      │ ◀──────────┘
                        │  Node        │
                        │  Positions   │
                        └──────────────┘
```

### Key Components

#### 1. Node Identity

Nodes need stable identity for reuse decisions:

```rust
/// Unique identifier for AST nodes
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct NodeId(u32);

/// Every node carries identity metadata
pub struct NodeMeta {
    id: NodeId,
    span: Span,
    flags: NodeFlags,
}

bitflags! {
    pub struct NodeFlags: u8 {
        /// Node contains a parse error
        const HAS_ERROR = 0b0001;
        /// Node was incrementally parsed (not reused)
        const FRESHLY_PARSED = 0b0010;
        /// Node intersects with change region
        const IN_CHANGE_REGION = 0b0100;
    }
}
```

#### 2. Syntax Cursor

Navigate old tree to find reusable nodes:

```rust
pub struct SyntaxCursor<'a> {
    old_tree: &'a Module,
    position: usize,
}

impl<'a> SyntaxCursor<'a> {
    /// Returns a reusable node at position, or None
    pub fn node_at(&self, position: usize) -> Option<ReusableNode> {
        let node = self.find_node_containing(position)?;

        // Nodes with errors cannot be reused
        if node.flags.contains(NodeFlags::HAS_ERROR) {
            return None;
        }

        // Nodes in change region cannot be reused
        if node.flags.contains(NodeFlags::IN_CHANGE_REGION) {
            return None;
        }

        Some(ReusableNode {
            id: node.id,
            kind: node.kind(),
            span: node.span,
        })
    }
}
```

#### 3. Change Region Computation

```rust
pub struct TextChange {
    /// Start position in old text
    pub start: usize,
    /// Length of removed text
    pub old_length: usize,
    /// Length of inserted text
    pub new_length: usize,
}

impl TextChange {
    /// Delta for positions after this change
    pub fn delta(&self) -> i64 {
        self.new_length as i64 - self.old_length as i64
    }

    /// Check if a span intersects with changed region
    pub fn intersects(&self, span: Span) -> bool {
        let change_end = self.start + self.old_length;
        span.start < change_end && span.end > self.start
    }
}
```

#### 4. Incremental Parser

```rust
pub fn reparse(
    old_tree: &Module,
    source: &str,
    change: TextChange,
) -> Module {
    // 1. Mark nodes in change region
    let marked = mark_changed_nodes(old_tree, &change);

    // 2. Create syntax cursor for reuse
    let cursor = SyntaxCursor::new(&marked);

    // 3. Parse with cursor
    let mut parser = Parser::new_incremental(source, cursor);
    let new_tree = parser.parse_module();

    // 4. Update positions for nodes outside change region
    update_positions(&new_tree, &change);

    new_tree
}

impl Parser<'_> {
    fn try_reuse_node(&mut self) -> Option<ExprId> {
        let cursor = self.syntax_cursor.as_ref()?;

        // Check if there's a reusable node at current position
        let reusable = cursor.node_at(self.position())?;

        // Verify kind matches what we're parsing
        if !self.expected_kind_matches(reusable.kind) {
            return None;
        }

        // Reuse the node!
        self.advance_by(reusable.span.len());
        Some(self.arena.reuse_node(reusable))
    }
}
```

### Implementation Phases

#### Phase 3.1.1: Node Identity Infrastructure
- [ ] Add `NodeId` to all AST nodes
- [ ] Implement `NodeFlags` bitflags
- [ ] Thread identity through arena allocation

#### Phase 3.1.2: Syntax Cursor
- [ ] Implement `SyntaxCursor` for old tree navigation
- [ ] Add `node_at()` for reuse lookup
- [ ] Handle nested nodes (prefer largest reusable)

#### Phase 3.1.3: Change Region Marking
- [ ] Implement `TextChange` representation
- [ ] Add `mark_changed_nodes()` traversal
- [ ] Test intersection logic

#### Phase 3.1.4: Incremental Parser Core
- [ ] Add `try_reuse_node()` at key parse points
- [ ] Implement position adjustment post-parse
- [ ] Integrate with existing progress tracking

#### Phase 3.1.5: Integration
- [ ] Wire into Salsa for automatic cache invalidation
- [ ] Add metrics for reuse percentage
- [ ] Benchmark incremental vs full re-parse

### Tasks

- [ ] **Design node identity system**
  - [ ] Choose ID generation strategy (sequential, hash-based)
  - [ ] Decide on flags needed
  - Document: Create ADR (Architecture Decision Record)

- [ ] **Implement NodeMeta**
  - [ ] Add to ExprArena
  - [ ] Thread through all allocation sites
  - Location: `compiler/ori_parse/src/arena.rs`

- [ ] **Implement SyntaxCursor**
  - [ ] Tree navigation
  - [ ] Position-based lookup
  - [ ] Reuse eligibility check
  - Location: `compiler/ori_parse/src/incremental.rs` (new file)

- [ ] **Implement TextChange**
  - [ ] Change representation
  - [ ] Position adjustment arithmetic
  - [ ] Intersection detection

- [ ] **Integrate into Parser**
  - [ ] Add optional cursor parameter
  - [ ] Call try_reuse_node() at appropriate points
  - [ ] Preserve progress tracking behavior

- [ ] **Add tests**
  - [ ] Single-line edit in middle of file
  - [ ] Multi-line insertion/deletion
  - [ ] Edit that invalidates nested nodes
  - [ ] Edit in error region

- [ ] **Benchmark**
  - [ ] Full parse time vs incremental
  - [ ] Reuse percentage metrics
  - [ ] Memory overhead of identity tracking

### Verification

- [ ] Incremental parse produces identical AST to full parse
- [ ] 80%+ node reuse on typical edits
- [ ] <50ms incremental parse time
- [ ] All existing tests pass

---

## 3.2 Language Server Integration

> **Reference**: LSP (Language Server Protocol) standard, TypeScript's tsserver.

### Motivation

With incremental parsing (3.1), we can build a responsive language server:

- **Real-time diagnostics**: Errors shown as you type
- **Autocomplete**: Context-aware suggestions
- **Hover**: Type information and documentation
- **Go-to-definition**: Jump to declarations
- **Find references**: All usages of a symbol
- **Rename**: Safe refactoring across files

### Architecture

```
┌───────────────┐     JSON-RPC      ┌─────────────────┐
│   IDE/Editor  │ ◀──────────────▶ │  ori-lsp        │
│   (VS Code,   │                   │  (Language      │
│    Neovim)    │                   │   Server)       │
└───────────────┘                   └─────────────────┘
                                            │
                                    ┌───────┴───────┐
                                    │               │
                              ┌─────▼─────┐   ┌─────▼─────┐
                              │ Incremental│   │ Semantic  │
                              │ Parser     │   │ Analysis  │
                              └───────────┘   └───────────┘
```

### Parser Requirements for LSP

#### 1. Position ↔ AST Node Mapping

```rust
impl Module {
    /// Find the innermost node at a given position
    pub fn node_at_position(&self, pos: Position) -> Option<&Node> {
        // Binary search through nodes by span
        // Return smallest containing node
    }

    /// Find all nodes of a given kind
    pub fn nodes_of_kind(&self, kind: NodeKind) -> impl Iterator<Item = &Node> {
        // ...
    }
}
```

#### 2. Partial Parsing (Error Recovery)

The parser must produce a usable AST even with errors:

```rust
// Before: stops at first error
fn parse_function(&mut self) -> Result<Function, ParseError>

// After: continues past errors with placeholder nodes
fn parse_function(&mut self) -> Function  // Always returns something
```

This is where Section 01's rich error types and Section 02's snapshots help.

#### 3. Comment and Whitespace Preservation

For formatting and doc extraction:

```rust
pub struct ParsedFile {
    pub module: Module,
    pub comments: Vec<Comment>,
    pub trivia: TriviaMap,  // Maps nodes to preceding comments
}
```

### Tasks

- [ ] **Position mapping**
  - [ ] `node_at_position()` method
  - [ ] `span_to_range()` conversion
  - Location: `compiler/ori_parse/src/position.rs` (new file)

- [ ] **Error-tolerant parsing**
  - [ ] Return placeholder nodes on error
  - [ ] Continue parsing after errors
  - [ ] Mark error regions in AST

- [ ] **Comment preservation**
  - [ ] Collect comments during lexing
  - [ ] Associate comments with nodes
  - [ ] Expose via ParsedFile

- [ ] **LSP crate scaffolding**
  - [ ] Create `compiler/ori_lsp/` crate
  - [ ] Implement LSP protocol types
  - [ ] Basic lifecycle handlers
  - Reference: `plans/ori_lsp/` (existing plan)

### Verification

- [ ] `node_at_position()` works for all node types
- [ ] Parsing always produces usable AST
- [ ] Comments correctly associated

---

## 3.3 SIMD Preprocessing

> **Reference**: Roc's `src64.rs` uses SSE2/NEON for file preprocessing.

### Motivation

For large files (>100KB), tokenization can become a bottleneck. SIMD instructions can:
- Scan for line breaks 16-64 bytes at a time
- Detect ASCII vs UTF-8 boundaries
- Find string/comment delimiters in parallel

### Architecture

```
┌────────────────┐     ┌───────────────┐     ┌──────────────┐
│  File Read     │ ──▶ │  SIMD Prep    │ ──▶ │  Standard    │
│  (OS read)     │     │  (alignment,  │     │  Lexer       │
│                │     │   newlines)   │     │              │
└────────────────┘     └───────────────┘     └──────────────┘
```

### Key Optimizations

#### 1. 64-Byte Alignment

```rust
/// Align buffer for SIMD operations
pub fn align_buffer(data: &[u8]) -> AlignedBuffer {
    let aligned_size = (data.len() + 63) & !63;  // Round up to 64
    let mut buffer = vec![0u8; aligned_size];
    buffer[..data.len()].copy_from_slice(data);
    AlignedBuffer {
        data: buffer,
        len: data.len(),
    }
}
```

#### 2. SIMD Newline Scanning

```rust
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Count newlines in aligned buffer using SSE2
pub fn count_newlines_simd(data: &[u8]) -> usize {
    unsafe {
        let newline = _mm_set1_epi8(b'\n' as i8);
        let mut count = 0;

        for chunk in data.chunks_exact(16) {
            let block = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
            let cmp = _mm_cmpeq_epi8(block, newline);
            let mask = _mm_movemask_epi8(cmp);
            count += mask.count_ones() as usize;
        }

        count
    }
}
```

#### 3. Line Offset Table

```rust
/// Build line offset table using SIMD
pub fn build_line_offsets(data: &[u8]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(data.len() / 40);  // Estimate
    offsets.push(0);

    // SIMD scan for newlines
    // ... (platform-specific implementation)

    offsets
}
```

### Platform Support

| Platform | Instruction Set | Register Width |
|----------|-----------------|----------------|
| x86_64 | SSE2, AVX2 | 128/256 bits |
| aarch64 | NEON | 128 bits |
| wasm32 | SIMD128 | 128 bits |
| Fallback | Scalar | 64 bits |

### Tasks

- [ ] **Design abstraction layer**
  - [ ] Platform-agnostic interface
  - [ ] Runtime feature detection
  - [ ] Graceful fallback to scalar
  - Location: `compiler/ori_lexer/src/simd/` (new module)

- [ ] **Implement x86_64 SSE2**
  - [ ] Newline scanning
  - [ ] ASCII detection
  - [ ] String delimiter search

- [ ] **Implement aarch64 NEON**
  - [ ] Same operations for ARM
  - [ ] Test on Apple Silicon

- [ ] **Implement scalar fallback**
  - [ ] Same interface, no SIMD
  - [ ] Used on unsupported platforms

- [ ] **Integrate with lexer**
  - [ ] Pre-build line offset table
  - [ ] Use aligned buffer for lexing
  - Location: `compiler/ori_lexer/src/lib.rs`

- [ ] **Benchmark**
  - [ ] Compare SIMD vs scalar on large files
  - [ ] Measure startup time improvement
  - [ ] Test various file sizes

### Verification

- [ ] Correct results on all platforms
- [ ] 2x+ speedup on files >100KB
- [ ] No regressions on small files

---

## 3.4 Section Completion Checklist

- [ ] **3.1 Incremental Parsing**
  - [ ] NodeId and NodeFlags implemented
  - [ ] SyntaxCursor for reuse lookup
  - [ ] TextChange and position adjustment
  - [ ] 80%+ node reuse on typical edits
  - [ ] <50ms incremental parse time

- [ ] **3.2 Language Server Integration**
  - [ ] Position ↔ node mapping
  - [ ] Error-tolerant parsing
  - [ ] Comment preservation
  - [ ] Basic LSP scaffolding

- [ ] **3.3 SIMD Preprocessing**
  - [ ] x86_64 SSE2 implementation
  - [ ] aarch64 NEON implementation
  - [ ] Scalar fallback
  - [ ] 2x+ speedup on large files

- [ ] **Integration**
  - [ ] All parser tests pass
  - [ ] Incremental parsing produces correct ASTs
  - [ ] LSP responds to basic requests
  - [ ] SIMD works on all target platforms

**Exit Criteria**: IDE-quality parser with sub-100ms response times.

---

## Notes

- These improvements are substantial undertakings
- 3.1 (incremental) should come before 3.2 (LSP) for best results
- 3.3 (SIMD) is independent and can be done anytime
- Consider incremental rollout behind feature flags
- Extensive testing essential — regressions here are costly
