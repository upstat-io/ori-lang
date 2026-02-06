---
section: "01"
title: Data-Oriented AST
status: complete
goal: Replace pointer-based AST with index-based, cache-friendly storage
sections:
  - id: "01.1"
    title: MultiArrayList-style Storage
    status: complete
  - id: "01.2"
    title: Index-based Node References
    status: complete
  - id: "01.3"
    title: Extra Data Buffer
    status: complete
  - id: "01.4"
    title: Pre-allocation Heuristics
    status: complete
  - id: "01.5"
    title: Direct Arena Append
    status: complete
---

# Section 01: Data-Oriented AST

**Status:** ✅ Complete (2026-02-05)
**Goal:** Achieve 2-3x memory efficiency and improved cache locality through Zig-inspired data layout
**Source:** Zig compiler (`lib/std/zig/Parse.zig`, `lib/std/zig/Ast.zig`)

---

## Completion Summary (2026-02-05)

SoA migration fully implemented across 57 files, 9 crates. All 8474 tests pass.

### Results

| Metric | Before | After | Reduction |
|--------|--------|-------|-----------|
| `ExprKind` size | 80 bytes | 24 bytes | 70% |
| `Expr` size | 88 bytes | 32 bytes | 64% |
| Per-expression memory | ~88 bytes | ~32 bytes | 64% |
| Storage layout | `Vec<Expr>` (AoS) | `Vec<ExprKind>` + `Vec<Span>` (SoA) | Cache-friendly |

### What Was Done

**Phase 1 — Shrink ExprKind (80 → 24 bytes):**
1. Arena-allocated `FunctionSeq` behind `FunctionSeqId(u32)` (saved ~68 bytes from largest variant)
2. Arena-allocated `FunctionExp` behind `FunctionExpId(u32)` (saved ~16 bytes)
3. Arena-allocated `BindingPattern` behind `BindingPatternId(u32)` (removed `Vec` from enum)
4. Replaced inline `ParsedType` with `ParsedTypeId` in Let/Lambda/Cast variants (used existing infrastructure)
5. Normalized `ExprList` → `ExprRange` everywhere, deleted `inline_list.rs` (544 lines dead code)
6. Replaced `Option<ExprId>` with `ExprId::INVALID` sentinel in 10 variants (saved 4 bytes each)

**Phase 2 — SoA Storage Split:**
1. Split `Vec<Expr>` into parallel `Vec<ExprKind>` + `Vec<Span>`
2. Made `Expr` and `ExprKind` `Copy` — enables transparent by-value `get_expr()` (no consumer changes needed)
3. Added `expr_kind(id)` and `expr_span(id)` accessors for incremental SoA adoption
4. Skipped `ExprTag(u8)` — 50-variant sync cost outweighs marginal benefit at 24 bytes/kind

### Current Memory Layout

```rust
// ExprArena — Struct-of-Arrays storage
pub struct ExprArena {
    expr_kinds: Vec<ExprKind>,     // 24 bytes each (SoA: kinds)
    expr_spans: Vec<Span>,         // 8 bytes each (SoA: spans)
    expr_lists: Vec<ExprId>,       // Flat extra buffer for variable-length data
    stmts: Vec<Stmt>,              // Statement storage
    params: Vec<Param>,            // Parameter storage
    function_seqs: Vec<FunctionSeq>,    // Arena-allocated (was inline in ExprKind)
    function_exps: Vec<FunctionExp>,    // Arena-allocated (was inline in ExprKind)
    binding_patterns: Vec<BindingPattern>, // Arena-allocated (was inline in ExprKind)
    // ... more specialized vectors
}

// Expr — reconstructed by value (Copy)
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,  // 24 bytes
    pub span: Span,      // 8 bytes
}
// Total: 32 bytes (was 88)
```

### Key Architectural Decisions

1. **Copy semantics over references**: Making `Expr`/`ExprKind` `Copy` allowed `get_expr()` to return by value, making the SoA split transparent to all consumers. No `&'ast Expr` lifetime issues.

2. **Sentinel over Option**: `ExprId::INVALID` (u32::MAX) replaces `Option<ExprId>` everywhere. Added `is_present()` for readability at call sites. Saves 4 bytes per optional field (no discriminant + padding).

3. **No ExprTag**: The plan proposed a `#[repr(u8)]` tag enum mirroring ExprKind's 50 variants. Maintenance cost of keeping two enums in sync outweighs the cache benefit when ExprKind is already 24 bytes.

4. **Compat shim**: `get_expr(id) -> Expr` reconstructs from parallel arrays, so existing code works unchanged. New code can use `expr_kind(id)` / `expr_span(id)` for targeted access.

---

## Background

Traditional AST storage uses "Array of Structs" (AoS):
```rust
// Before: Each node is a contiguous struct
exprs: Vec<Expr>  // [Expr{kind, span}, Expr{...}, ...]
```

Zig's breakthrough: "Struct of Arrays" (SoA) with MultiArrayList:
```rust
// After: Separate arrays per field
expr_kinds: Vec<ExprKind>  // [kind0, kind1, kind2, ...]
expr_spans: Vec<Span>      // [span0, span1, span2, ...]
```

**Why this matters:**
- Cache lines load 64 bytes at a time
- Span-only queries (common in diagnostics) don't load ExprKind data
- Sequential access patterns get hardware prefetching
- Smaller ExprKind = more nodes per cache line

---

## 01.1 MultiArrayList-style Storage

**Status:** ✅ Complete (2026-02-05)

- [x] Split `Vec<Expr>` into `Vec<ExprKind>` + `Vec<Span>` (parallel arrays)
- [x] Implement accessor methods: `expr_kind(id)`, `expr_span(id)`, `get_expr(id)` (compat)
- [x] Add compile-time size assertions: `ExprKind = 24`, `Expr = 32`
- [x] Shrink ExprKind from 80 → 24 bytes via arena-allocation of large embedded types

---

## 01.2 Index-based Node References

**Status:** ✅ Complete (pre-existing + enhanced)

Ori already used `ExprId(u32)` indices. The SoA migration enhanced this with:

- [x] `ExprId(u32)` with `INVALID = u32::MAX` sentinel — already existed
- [x] `FunctionSeqId(u32)`, `FunctionExpId(u32)`, `BindingPatternId(u32)` — new arena IDs
- [x] `ParsedTypeId(u32)` — already existed, now used in ExprKind variants
- [x] `Option<ExprId>` eliminated — replaced with sentinel pattern across 10 variants
- [x] `ExprId::is_present()` convenience method for sentinel checks

---

## 01.3 Extra Data Buffer

**Status:** ✅ Complete (pre-existing + enhanced)

- [x] `expr_lists: Vec<ExprId>` — flat extra buffer for variable-length expression lists
- [x] `ExprRange { start: u32, len: u16 }` — 8-byte range type for list references
- [x] `alloc_expr_list_inline()` returns `ExprRange` (was `ExprList`, inline type deleted)
- [x] Specialized range types: `StmtRange`, `ParamRange`, `MatchPatternRange`, `CallArgRange`, etc.

---

## 01.4 Pre-allocation Heuristics

**Status:** ✅ Complete (pre-existing)

- [x] `with_capacity(source_len / 20)` — ~1 expr per 20 bytes of source
- [x] Applied to all arena vectors in `ExprArena::with_capacity()`

---

## 01.5 Direct Arena Append

**Status:** ✅ Complete (2026-02-06)

Replaced the deferred scratch buffer approach with direct arena append: `start_*/push_*/finish_*` method triples eliminate intermediate Vec allocations for non-recursive list parsing. Added `series_direct()` combinator that uses `FnMut(&mut Self) -> Result<bool>` instead of collecting into Vec.

- [x] `define_direct_append!` macro generates `start_*/push_*/finish_*` for 10 buffer types
- [x] `series_direct()` combinator + `paren_/bracket_/brace_/angle_series_direct()` wrappers
- [x] Direct push for `params` (function parameters) — safe, non-recursive
- [x] Direct push for `generic_params` (generic declarations) — safe, non-recursive
- [x] Vec-based `series_direct()` for recursive buffers (arms, named_exprs, parsed_type_lists, match_pattern_lists, list_elements, map_elements, struct_lit_fields) — avoids same-buffer nesting corruption
- [x] Deleted `scratch.rs` (replaced by direct arena append approach)

**Key finding:** Same-buffer nesting is a fundamental constraint — any buffer whose items contain expressions/types/patterns can recursively trigger pushes to the same buffer. Only "leaf" grammar constructs (params, generic_params) are safe for zero-copy direct push. Recursive constructs still benefit from `series_direct()` (avoids the series combinator's own Vec) while collecting items in a local Vec.

---

## 01.6 Completion Checklist

- [x] All AST nodes use index-based references
- [x] SoA storage passes all existing parser tests (8474 pass)
- [x] Memory usage reduced by 64% per expression (88 → 32 bytes)
- [x] No performance regression in parsing speed
- [x] `Expr`/`ExprKind` now `Copy` for zero-cost by-value access
- [x] Dead code removed (`inline_list.rs`, 544 lines)
- [x] Size assertions guard against regression

**Exit Criteria — All Met:**
- 64% memory reduction per expression (exceeds 40% target)
- All `tests/spec/` files parse correctly
- All 8474 tests pass (unit + spec + LLVM + WASM)
- Clippy clean across all crates

---

## 01.7 Hot Path Optimizations (2026-02-06)

Extension of the data-oriented philosophy to the parser's hot path. While not part of the original AST SoA plan, these optimizations apply the same Zig-inspired principles to token dispatch:

### Tag-Based Token Dispatch

Added parallel `Vec<u8>` of discriminant tags to `TokenList` (partial SoA — see Section 02.9). This enables O(1) tag checks throughout the parser:

| Optimization | Technique | Impact |
|-------------|-----------|--------|
| `check()` via tags | 1-byte tag comparison instead of 16-byte enum discriminant | Foundation |
| `OPER_TABLE[128]` | Static lookup table indexed by tag for Pratt parser | Replaces 20-arm match |
| `POSTFIX_BITSET` | Two-u64 bitset for postfix token membership | O(1) set membership |
| `parse_primary()` fast path | Direct tag dispatch before `one_of!` macro | Skips snapshot/restore for ~95% of cases |
| `#[cold]` split expect() | Error path in `#[cold] #[inline(never)]` function | Better LLVM code layout |
| Branchless `advance()` | Removed bounds check (EOF sentinel guarantees safety) | One fewer branch per token |

### Results

| Workload | Before | After | Improvement |
|----------|--------|-------|-------------|
| 10 funcs | 109 MiB/s | 120 MiB/s | +10% |
| 50 funcs | 126 MiB/s | 143 MiB/s | +13% |
| 100 funcs | 133 MiB/s | 154 MiB/s | +16% |
| 500 funcs | 144 MiB/s | 163 MiB/s | +13% |
| 1000 funcs | 144 MiB/s | 161 MiB/s | +12% |

All 8311 tests pass, clippy clean.
