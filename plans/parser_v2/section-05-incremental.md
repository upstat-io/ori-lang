---
section: "05"
title: Incremental Parsing
status: complete
goal: Enable efficient reparsing for IDE scenarios with 70-90% AST reuse
sections:
  - id: "05.1"
    title: Syntax Cursor with Caching
    status: complete
  - id: "05.2"
    title: Node Reusability Predicates
    status: complete
  - id: "05.3"
    title: Change Range Propagation
    status: complete
  - id: "05.4"
    title: Lazy Token Capture
    status: complete
---

# Section 05: Incremental Parsing

**Status:** ✅ Complete (2026-02-04)
**Goal:** 70-90% AST reuse on typical edits for IDE responsiveness
**Source:** TypeScript (`src/compiler/parser.ts`), Rust (`compiler/rustc_parse/`)

---

## Background

Full reparsing is too slow for IDE scenarios:
- Keystroke latency target: <50ms
- Large file (10K lines): ~100ms to parse
- Need: Reuse unchanged portions of AST

TypeScript achieves 70-90% reuse through:
1. **Syntax cursor** — Navigate old AST by position
2. **Reusability predicates** — Determine which nodes can be reused
3. **Position caching** — Optimize sequential access patterns

Ori already has incremental infrastructure in `incremental.rs` — this section integrates and enhances it.

---

## 05.1 Syntax Cursor with Caching

**Status:** ✅ Complete (2026-02-04)
**Goal:** Efficient old-AST navigation with sequential access optimization

### Implementation Summary (2026-02-04)

The core `SyntaxCursor` exists in `compiler/ori_parse/src/incremental.rs`:

```rust
pub struct SyntaxCursor<'old> {
    module: &'old Module,
    arena: &'old ExprArena,
    marker: ChangeMarker,
    declarations: Vec<DeclRef>,
    current_index: usize,
}

impl<'old> SyntaxCursor<'old> {
    pub fn new(module, arena, marker) -> Self { ... }
    pub fn find_at(&mut self, pos: u32) -> Option<DeclRef> { ... }
    pub fn advance(&mut self) { ... }
    pub fn is_exhausted(&self) -> bool { ... }
    pub fn marker(&self) -> &ChangeMarker { ... }
    pub fn module(&self) -> &Module { ... }
    pub fn arena(&self) -> &ExprArena { ... }
}
```

#### What Exists ✅
- [x] `SyntaxCursor` struct with module, arena, marker, declarations, current_index, stats
- [x] `find_at()` — linear scan forward from current position
- [x] `advance()` — move past current declaration
- [x] `is_exhausted()` — check if all declarations processed
- [x] Integration with `parse_module_incremental()`
- [x] `CursorStats` — lookups, skipped, intersected tracking (added 2026-02-04)
- [x] `stats()` accessor and `total_declarations()` method

### Implementation Details (2026-02-04)

```rust
/// Statistics for cursor navigation (debugging/tuning).
#[derive(Clone, Debug, Default)]
pub struct CursorStats {
    pub lookups: u32,      // Total find_at() calls
    pub skipped: u32,      // Declarations skipped during forward scan
    pub intersected: u32,  // Declarations that couldn't be reused
}

impl CursorStats {
    pub fn total_examined(&self) -> u32 { self.skipped + self.intersected }
}

impl SyntaxCursor<'_> {
    pub fn stats(&self) -> &CursorStats { ... }
    pub fn total_declarations(&self) -> usize { ... }
}
```

#### Future Optimizations (Not Needed Yet)
- Position caching — `last_queried_pos` for repeated queries
- Binary search fallback — for non-sequential access patterns
- Performance logging on Drop

The current linear scan is efficient because:
1. Declarations are always processed sequentially (top-to-bottom)
2. `find_at()` only advances forward, never backwards
3. Typical files have few declarations (~10-50), so O(n) is fine

---

## 05.2 Node Reusability Predicates

**Status:** ✅ Complete (2026-02-04)
**Goal:** Determine which AST nodes can be safely reused

### Implementation Summary

Reusability checking is implemented via `ChangeMarker::intersects()` in `ori_ir/src/incremental.rs`:

```rust
// In parse_module_incremental() at lib.rs:782
if !state.cursor.marker().intersects(decl_ref.span) {
    // Safe to reuse - copy with span adjustment
    let copier = AstCopier::new(old_arena, state.cursor.marker().clone());
    // ... copy declaration
}
```

#### What Exists ✅
- [x] `ChangeMarker::intersects(span)` — checks if span overlaps affected region
- [x] `ChangeMarker::from_change(change, prev_token_end)` — creates extended region
- [x] Per-declaration-kind handling in `parse_module_incremental()`:
  - Function, Test, Type, Trait, Impl, DefImpl, Extend, Config all supported
- [x] Integration tests verifying reuse behavior

#### Design Notes

The current implementation uses a **conservative approach**:
- Any declaration that intersects the change region is reparsed
- Declarations entirely before or after the change are reused with span adjustment
- Content hashing (planned as optional verification) is not needed with this approach

This is sufficient for correct incremental parsing. More sophisticated predicates
(e.g., checking test target validity) can be added later if needed.

### Tasks

- [x] Define reusability criteria — `marker.intersects()` handles this
- [x] Per-node-kind predicates — all 9 `DeclKind` variants handled
- [x] Integration with incremental parse — working in `parse_module_incremental()`
- [ ] Optional: Content hashing for belt-and-suspenders verification

---

## 05.3 Change Range Propagation

**Status:** ✅ Complete (2026-02-04)
**Goal:** Adjust spans of reused nodes for new positions

### Implementation Summary

Span adjustment is fully implemented via `AstCopier` in `compiler/ori_parse/src/incremental.rs`:

```rust
pub struct AstCopier<'old> {
    old_arena: &'old ExprArena,
    marker: ChangeMarker,
}

impl<'old> AstCopier<'old> {
    fn adjust_span(&self, span: Span) -> Span {
        self.marker.adjust_span(span).unwrap_or(span)
    }

    pub fn copy_expr(&self, old_id: ExprId, new_arena: &mut ExprArena) -> ExprId { ... }
    pub fn copy_function(&self, func: &Function, new_arena: &mut ExprArena) -> Function { ... }
    pub fn copy_test(&self, test: &TestDef, new_arena: &mut ExprArena) -> TestDef { ... }
    pub fn copy_type_decl(&self, decl: &TypeDecl, new_arena: &mut ExprArena) -> TypeDecl { ... }
    pub fn copy_trait(&self, trait_def: &TraitDef, new_arena: &mut ExprArena) -> TraitDef { ... }
    pub fn copy_impl(&self, impl_def: &ImplDef, new_arena: &mut ExprArena) -> ImplDef { ... }
    // ... all declaration types covered
}
```

The `ChangeMarker::adjust_span()` in `ori_ir/src/incremental.rs` handles the actual delta calculation.

#### What Exists ✅
- [x] `ChangeMarker` struct with change start, old_length, new_length, delta
- [x] `ChangeMarker::adjust_span()` — shifts spans after change by delta
- [x] `AstCopier` — deep copies entire AST with span adjustment
- [x] All 60+ expression kinds handled in `copy_expr()`
- [x] All declaration types handled (Function, Test, Type, Trait, Impl, DefImpl, Extend, Config)
- [x] Nested types handled (Param, MatchArm, MatchPattern, ParsedType, etc.)
- [x] Integration with `parse_module_incremental()`
- [x] Tests: `test_parse_incremental_basic`, `test_parse_incremental_insert`, `test_parse_incremental_fresh_parse_on_overlap`

#### Design Notes

The implementation uses a **deep copy** approach:
- When reusing a declaration, `AstCopier` copies all nested expressions to the new arena
- Each span is adjusted via `adjust_span()` during the copy
- This ensures the new arena is self-consistent (no references to old arena)

This is more expensive than in-place span mutation but simpler and safer with
the arena-based design.

### Tasks

- [x] `ChangeMarker` with span adjustment — implemented
- [x] Recursive span adjustment via `AstCopier` — implemented
- [x] Integration with `parse_module_incremental()` — working
- [x] Tests verifying span adjustment — 3 integration tests

---

## 05.4 Lazy Token Capture

**Status:** ✅ Complete (2026-02-04)
**Goal:** Defer token stream reconstruction for attributes/patterns

### Implementation Summary

The original plan proposed a `LazyTokens` type with source hashing for validation.
However, Ori's architecture uses Salsa for caching, making the design simpler:

1. **TokenList is already cached** — No need to re-lex, just store indices
2. **Indices are stable** — TokenList is immutable within a parse
3. **No source hashing needed** — TokenList validity is guaranteed by Salsa

#### What Was Implemented ✅

**1. TokenCapture type** (`ori_ir/src/token.rs`)
```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TokenCapture {
    #[default]
    None,
    Range { start: u32, end: u32 },
}

impl TokenCapture {
    pub fn new(start: u32, end: u32) -> Self { ... }
    pub fn is_empty(&self) -> bool { ... }
    pub fn len(&self) -> usize { ... }
    pub fn span(&self, tokens: &TokenList) -> Option<Span> { ... }
}
```

**2. TokenList range access** (`ori_ir/src/token.rs`)
```rust
impl TokenList {
    pub fn get_range(&self, capture: TokenCapture) -> &[Token] { ... }
    pub fn try_get_range(&self, capture: TokenCapture) -> Option<&[Token]> { ... }
}
```

**3. Cursor capture helpers** (`ori_parse/src/cursor.rs`)
```rust
impl Cursor<'_> {
    pub fn start_capture(&self) -> u32 { ... }
    pub fn complete_capture(&self, start: u32) -> TokenCapture { ... }
    pub fn tokens(&self) -> &TokenList { ... }
}
```

**4. Parser capture helpers** (`ori_parse/src/lib.rs`)
```rust
impl Parser<'_> {
    pub(crate) fn start_capture(&self) -> u32 { ... }
    pub(crate) fn complete_capture(&self, start: u32) -> TokenCapture { ... }
    pub(crate) fn with_capture<T, F>(&mut self, f: F) -> (T, TokenCapture) { ... }
    pub(crate) fn capture_if<T, F>(&mut self, needs_capture: bool, f: F) -> (T, TokenCapture) { ... }
}
```

**5. ParsedAttrs integration** (`ori_parse/src/grammar/attr.rs`)
```rust
pub struct ParsedAttrs {
    // ... semantic fields ...
    pub token_range: TokenCapture,  // Captures all attribute tokens
}
```

#### Design Rationale

- **Memory efficient**: TokenCapture is 12 bytes (discriminant + start + end)
- **O(1) access**: Just index into the cached TokenList
- **Salsa compatible**: All required traits (Clone, Copy, Eq, Hash, Debug)
- **No re-lexing**: Uses existing token stream, not source positions

#### Tests Added

- `test_token_capture_*` in `ori_ir/src/token.rs` (6 tests)
- `test_token_capture*` in `ori_parse/src/cursor.rs` (2 tests)
- `test_parsed_attrs_token_capture` in `ori_parse/src/grammar/attr.rs` (2 tests)

---

## 05.5 Completion Checklist

- [x] `SyntaxCursor` core implemented ✅ (2026-02-04)
- [x] `CursorStats` for debugging/tuning ✅ (2026-02-04)
- [x] Reusability predicates for all node kinds ✅ (2026-02-04)
- [x] Span adjustment working correctly ✅ (2026-02-04)
- [x] Lazy token capture implemented ✅ (2026-02-04)
- [x] Integration tests for incremental parsing ✅ (4 tests)
- [x] TokenCapture tests ✅ (10 tests total)
- [ ] Performance benchmarks showing 70%+ reuse (future work)

**Exit Criteria:**
- [x] TokenCapture type with index-based lazy access
- [x] Parser helpers for capturing token ranges
- [x] ParsedAttrs captures attribute token range
- [x] No correctness regressions (all 308 parser tests pass)
- [ ] Performance benchmarks (deferred to LSP integration)
- [ ] LSP integration (separate plan: `plans/ori_lsp/`)

**Status:** Section 05 is functionally complete. The incremental parsing infrastructure
(cursor, reusability, span adjustment, lazy tokens) is in place. Performance benchmarks
and LSP integration will be addressed in the LSP plan.
