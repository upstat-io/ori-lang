---
section: "01"
title: "Compact Token Stream (SoA)"
status: done
goal: "Replace 26-byte AoS TokenList with 6-byte SoA CompactTokenStream for 4.3x cache density"
sections:
  - id: "01.1"
    title: "Design CompactTokenStream"
    status: done
  - id: "01.2"
    title: "Implement CompactTokenStream"
    status: done
  - id: "01.3"
    title: "Wire into Lexer Output"
    status: done
  - id: "01.4"
    title: "Salsa Compatibility"
    status: done
  - id: "01.5"
    title: "Performance Validation"
    status: not-started
---

# Section 01: Compact Token Stream (SoA)

**Status:** Done (01.5 Performance Validation remaining)
**Goal:** Replace the current `TokenList` (26 bytes/token, AoS layout) with a `CompactTokenStream` (6 bytes/token, SoA layout) that stores only what the raw scanner produces: tag, offset, and flags.

---

## Background

### Current Layout: `TokenList` (26 bytes/token)

```rust
// ori_ir/src/token/list.rs
struct TokenList {
    tokens: Vec<Token>,       // 24 bytes each (TokenKind 16 + Span 8)
    tags: Vec<u8>,            // 1 byte each (derived from TokenKind discriminant)
    flags: Vec<TokenFlags>,   // 1 byte each
}
```

Problem: `TokenKind` is 16 bytes because its largest variant is `Duration(u64, DurationUnit)`. But ~85% of tokens are zero-payload (keywords, operators, delimiters, newlines). Every token pays the 16-byte cost regardless.

For a 10,000-line file (~50K tokens):
- Current: `50K x 26 = 1.3 MB` (spills L1, fills L2)
- Target: `50K x 6 = 300 KB` (fits L2, mostly L1-resident)

### Target Layout: `CompactTokenStream` (6 bytes/token)

```rust
struct CompactTokenStream {
    tags: Vec<u8>,        // 1 byte: RawTag discriminant
    offsets: Vec<u32>,    // 4 bytes: byte position in source
    flags: Vec<u8>,       // 1 byte: TokenFlags
    // Total: 6 bytes/token
    // Token length derived from: offsets[i+1] - offsets[i]
    // Token span derived from: Span::new(offsets[i], offsets[i+1])
    // TokenKind materialized on demand by the lazy cooker (Section 02)
}
```

### Why SoA (Structure of Arrays)?

The parser's hot loop is: "check current tag, decide what to do, advance." It reads the `tags` array sequentially. In SoA layout, the `tags` array is contiguous in memory:

```
SoA tags:    [Ident][LParen][Ident][Comma][Ident][RParen][Arrow]...
              ^^^^^ these are all adjacent in memory = cache-friendly

AoS tokens:  [24-byte Token][24-byte Token][24-byte Token]...
              ^^^^^ 24 bytes apart = only 2.7 per cache line
```

With SoA, the parser can fit ~64 tags per cache line. With AoS, only ~2.7 tokens.

---

## 01.1 Design CompactTokenStream

- [x] Define `CompactTokenStream` struct in `ori_ir/src/token/`
  - [x] `tags: Vec<u8>` — raw tag discriminant (reuse `RawTag` repr(u8))
  - [x] `offsets: Vec<u32>` — byte position of each token start
  - [x] `flags: Vec<u8>` — `TokenFlags` bitfield per token
  - [x] Final offset entry = source length (sentinel, for computing last token's span)
- [x] Define access API
  - [x] `tag(index) -> u8` — O(1)
  - [x] `offset(index) -> u32` — O(1)
  - [x] `span(index) -> Span` — computed from `offsets[i]..offsets[i+1]`
  - [x] `flag(index) -> TokenFlags` — O(1)
  - [x] `len() -> usize`
  - [x] `is_empty() -> bool`
- [x] Define iteration API
  - [x] `iter_tags() -> &[u8]` — for sequential tag scanning
  - [x] `iter() -> impl Iterator<Item = (u8, Span, TokenFlags)>` — full iteration
- [x] Design the cooked value access (deferred to Section 02)
  - [x] Placeholder: `cooked_kind(index, source, interner) -> TokenKind`
- [x] Size assertions
  - [x] `CompactTokenStream` struct size <= 72 bytes (3 Vecs = 3x24)
  - [x] No per-token heap allocation

### Key Design Decisions

**Q: Should `offsets` store start positions or (start, end) pairs?**
A: Start positions only. Token length = `offsets[i+1] - offsets[i]`. This halves the offset storage (4 bytes vs 8 bytes per token). The sentinel entry at the end (`offsets[len] = source_len`) makes this work for the last token.

**Q: Should tags use `RawTag` (pre-cooking) or a new compact enum?**
A: Use `RawTag` directly. It's already `repr(u8)` with a well-designed discriminant layout. The cooker maps `RawTag -> TokenKind` on demand. This eliminates a redundant enum.

**Q: Where does this live — `ori_ir` or `ori_lexer_core`?**
A: In `ori_ir` alongside the existing `TokenList`, since the parser (in `ori_parse`) needs to consume it. The `ori_lexer_core` crate remains dependency-free.

---

## 01.2 Implement CompactTokenStream

- [x] Create `ori_ir/src/token/compact.rs`
  - [x] `CompactTokenStream` struct
  - [x] Constructor: `new()`, `with_capacity(source_len)`
  - [x] Push: `push(tag: u8, offset: u32, flags: u8)`
  - [x] Finalize: `seal(source_len: u32)` — appends sentinel offset
  - [x] Access: `tag()`, `offset()`, `span()`, `flag()`
  - [x] Iteration: `iter_tags()`, `iter()`
- [x] Derive required traits
  - [x] `Clone` — for Salsa
  - [x] `Debug` — summary format (count, not full dump)
  - [x] Manual `PartialEq`, `Eq`, `Hash` — compare only `tags` + `flags` (not offsets), matching current `TokenList` semantics for Salsa early-cutoff
- [x] Pre-allocation heuristic
  - [x] Reuse existing: `source_len / 2` tokens estimated (from `LexOutput::with_capacity`)
- [x] Tests in `ori_ir/src/token/compact/tests.rs`
  - [x] Push + access round-trip
  - [x] Span computation correctness
  - [x] Empty stream
  - [x] Single token
  - [x] Equality: same tags+flags, different offsets → equal (Salsa semantics)
  - [x] Hash consistency with equality

---

## 01.3 Wire into Lexer Output

- [x] Modify `lex_with_comments()` in `ori_lexer/src/lib.rs`
  - [x] Replace `TokenList` with `CompactTokenStream` in `LexOutput`
  - [x] Driver loop pushes `(raw.tag as u8, offset, pending_flags)` instead of constructing `Token`
  - [x] Trivia tokens (whitespace) still tracked in flags, not stored as tokens
  - [x] Comment tokens still captured in `CommentList` (unchanged)
  - [x] Newline tokens stored in compact stream (they're significant in Ori)
- [x] Update `LexResult` to carry `CompactTokenStream`
- [x] Update `lex()` and `lex_full()` entry points
- [x] Ensure `LexOutput::into_parts()` still produces `ModuleExtra` correctly

### Migration Strategy

During migration, both `TokenList` and `CompactTokenStream` can coexist:

```rust
// Temporary bridge: convert compact stream to legacy TokenList
impl CompactTokenStream {
    fn to_token_list(&self, source: &str, interner: &StringInterner) -> TokenList {
        // Cook all tokens eagerly — same behavior as V2
        // Used during migration while parser still expects TokenList
    }
}
```

This allows incremental migration: the lexer produces `CompactTokenStream` internally, converts to `TokenList` for the parser, and we validate equivalence. Once the parser is adapted (Section 05), the bridge is removed.

---

## 01.4 Salsa Compatibility

- [x] `CompactTokenStream` implements `Clone + Eq + PartialEq + Hash + Debug`
- [x] Equality semantics: compare `tags` + `flags` only (not `offsets`)
  - [x] Rationale: whitespace-only edits shift offsets but don't change token sequence
  - [x] This matches current `TokenList` equality semantics
- [x] Hash semantics: hash `tags` + `flags` only
- [x] Verify Salsa early-cutoff behavior
  - [x] Edit whitespace → re-lex → CompactTokenStream differs in offsets but equals in tags+flags → Salsa cuts off → parser not re-invoked
  - [x] Edit identifier → re-lex → tags differ → Salsa re-parses

---

## 01.5 Performance Validation

- [ ] Run `/benchmark short` before changes (record baseline)
- [ ] Run `/benchmark short` after changes
- [ ] Measure with `perf stat`:
  - [ ] L1 cache miss rate (expect significant reduction)
  - [ ] Instructions per token (expect reduction from smaller data)
  - [ ] Branch mispredictions (should be unchanged — scanning logic same)
- [ ] No regressions >5% vs baseline on any existing benchmark
- [ ] Document memory savings: bytes/token before vs after

**Exit Criteria:** `CompactTokenStream` passes all existing lexer tests, produces identical token sequences (via bridge conversion), and demonstrates measurable cache density improvement in benchmarks.
