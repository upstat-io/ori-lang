---
section: "05"
title: "Parser Adaptation"
status: not-started
goal: "Adapt ori_parse to consume CompactTokenStream with lazy cooking, replacing TokenList dependency"
sections:
  - id: "05.1"
    title: "Audit Parser Token Access Patterns"
    status: not-started
  - id: "05.2"
    title: "Design TokenCursor Abstraction"
    status: not-started
  - id: "05.3"
    title: "Implement TokenCursor for CompactTokenStream"
    status: not-started
  - id: "05.4"
    title: "Migrate Parser to TokenCursor"
    status: not-started
  - id: "05.5"
    title: "Remove Legacy TokenList Path"
    status: not-started
  - id: "05.6"
    title: "Performance Validation"
    status: not-started
---

# Section 05: Parser Adaptation

**Status:** Planned
**Goal:** Adapt the parser (`ori_parse`) to consume the new `CompactTokenStream` + lazy cooker, replacing the current `TokenList` dependency without changing parsing behavior.

---

## Background

The parser currently consumes `TokenList` which provides:
1. `tags: &[u8]` — for fast discriminant dispatch (the hot path)
2. `tokens: &[Token]` — for `TokenKind` values when the parser needs payload data
3. `flags: &[TokenFlags]` — for whitespace/trivia context

The key observation is that **the parser already uses the `tags` array for most dispatch**. It only accesses the full `Token` (and thus `TokenKind`) when it needs to extract an identifier name, literal value, or keyword identity. This means the migration is mostly mechanical: replace `TokenList` access with `CompactTokenStream` access + lazy cooking for the ~30% of accesses that need values.

---

## 05.1 Audit Parser Token Access Patterns

- [ ] Grep parser for all `TokenList` / `Token` / `TokenKind` access points
- [ ] Classify each access into:
  - [ ] **Tag-only** (most common): `tag(i) == TokenKind::LParen as u8` — needs only the tag byte
  - [ ] **Kind match**: `match token.kind { TokenKind::Ident(name) => ..., TokenKind::Int(n) => ... }` — needs cooked value
  - [ ] **Span access**: `token.span` — needs offset computation
  - [ ] **Flags access**: `flag(i)` — needs flags byte
- [ ] Count access pattern distribution (expect ~70% tag-only, ~20% kind-match, ~10% span/flags)
- [ ] Identify hot loops where tag-only access dominates (expression parsing, statement parsing)
- [ ] Identify cold paths where kind-match dominates (literal extraction, error messages)

---

## 05.2 Design TokenCursor Abstraction

Design a cursor type that the parser uses, abstracting over the token storage:

```rust
/// Parser cursor over a compact token stream with lazy cooking.
///
/// Provides tag-based dispatch (fast path) and on-demand value
/// materialization (lazy cooking path).
pub struct TokenCursor<'src> {
    stream: &'src CompactTokenStream,
    cooker: LazyCooker<'src>,
    pos: usize,
}

impl<'src> TokenCursor<'src> {
    // === Fast path: tag-based dispatch (no cooking) ===

    /// Current token's raw tag. O(1), no cooking.
    fn current_tag(&self) -> u8;

    /// Peek at tag N positions ahead. O(1), no cooking.
    fn peek_tag(&self, n: usize) -> u8;

    /// Check if current tag matches. O(1), no cooking.
    fn at(&self, tag: u8) -> bool;

    /// Advance to next token. O(1).
    fn advance(&mut self);

    /// Current token's span. O(1), computed from offsets.
    fn current_span(&self) -> Span;

    /// Current token's flags. O(1).
    fn current_flags(&self) -> TokenFlags;

    // === Lazy path: value materialization (cooks on demand) ===

    /// Get the full TokenKind for current token. Cooks if needed.
    fn current_kind(&mut self) -> TokenKind;

    /// Extract identifier Name from current token. Cooks if needed.
    /// Panics if current token is not an identifier.
    fn current_ident(&mut self) -> Name;

    /// Extract integer value from current token. Cooks if needed.
    fn current_int(&mut self) -> u64;

    /// Extract string Name from current token. Cooks if needed.
    fn current_string(&mut self) -> Name;

    // ... specialized extractors for each payload type
}
```

- [ ] Define the `TokenCursor` trait/struct API
- [ ] Ensure tag constants are accessible from the parser
  - [ ] Either: parser uses `RawTag` discriminant values directly
  - [ ] Or: define `const` tag values that map to `RawTag` repr
  - [ ] The current `TokenList.tag()` already returns `u8` discriminant indices — ensure the new tags use compatible values or define a clean mapping
- [ ] Design the `RawTag` ↔ parser tag compatibility layer
  - [ ] Current parser uses `TokenKind::discriminant_index()` values (0-123)
  - [ ] New system uses `RawTag` repr values (different numbering)
  - [ ] Need either: (a) renumber `RawTag` to match, (b) add a mapping table, or (c) define new parser tag constants

### Tag Mapping Strategy

The cleanest approach: define a unified `TokenTag` enum (`repr(u8)`) that both `RawTag` and the parser use. This enum covers all final token identities:

```rust
#[repr(u8)]
pub enum TokenTag {
    // Identifiers
    Ident = 0,
    // Keywords (resolved from Ident during cooking or SIMD pass)
    If = 1, Else = 2, Let = 3, ...
    // Operators
    Plus = 32, Minus = 33, ...
    // Delimiters
    LParen = 80, RParen = 81, ...
    // Literals (payload in lazy cache)
    Int = 128, Float = 129, String = 130, ...
    // Trivia
    Newline = 240, Eof = 255,
}
```

This replaces both `RawTag` and `TokenKind::discriminant_index()` with a single `u8` vocabulary shared across the entire pipeline. Operators and delimiters are fully identified by tag alone. Identifiers and keywords require the SIMD pass to emit keyword-specific tags (or the lazy cooker resolves them on access).

- [ ] Design `TokenTag` enum
- [ ] Map from `RawTag` to `TokenTag` (1:1 for most, Ident→Keyword resolution is the exception)
- [ ] Decide: resolve keywords in SIMD pass or defer to lazy cooker?
  - [ ] Option A: SIMD pass emits `Ident` tag, lazy cooker resolves keywords → simpler SIMD, but parser can't distinguish `if` from `foo` without cooking
  - [ ] Option B: Post-SIMD keyword resolution pass scans all `Ident` tokens and resolves keywords → adds a pass but parser gets keyword tags for free
  - [ ] Recommend Option B: keyword resolution is cheap (length-bucketed match on source bytes) and the parser needs keyword tags on the fast path

---

## 05.3 Implement TokenCursor for CompactTokenStream

- [ ] Implement `TokenCursor` struct
- [ ] Implement fast-path methods (tag, peek, at, advance, span, flags)
- [ ] Implement lazy-path methods (current_kind, current_ident, current_int, etc.)
- [ ] Wire lazy cooker into cursor
- [ ] Add error forwarding: cooking errors accumulated in cooker, retrieved after parse
- [ ] Tests:
  - [ ] Tag-only access never triggers cooking
  - [ ] Value access triggers cooking exactly once (cached)
  - [ ] Span computation matches expected values
  - [ ] Error propagation from cooker

---

## 05.4 Migrate Parser to TokenCursor

This is the largest task. The parser (`ori_parse`) needs to be updated to use `TokenCursor` instead of direct `TokenList` access.

- [ ] Identify the parser's token access entry point (likely a `Parser` struct with a token position)
- [ ] Replace the `Parser` struct's token storage with `TokenCursor`
- [ ] Migrate tag-based dispatch (should be mostly mechanical rename)
- [ ] Migrate kind-based extraction (replace `token.kind` access with `cursor.current_kind()`)
- [ ] Migrate span access (replace `token.span` with `cursor.current_span()`)
- [ ] Run all parser tests after each file migration
- [ ] Run all spec tests after complete migration

### Migration Approach

**Incremental, not big-bang.** The bridge conversion from Section 01.3 (`CompactTokenStream::to_token_list()`) allows the parser to keep using `TokenList` while individual parse functions are migrated one at a time. Once all parse functions are migrated, remove the bridge.

---

## 05.5 Remove Legacy TokenList Path

- [ ] Remove `TokenList` from `LexOutput` (replaced by `CompactTokenStream`)
- [ ] Remove `CompactTokenStream::to_token_list()` bridge
- [ ] Remove `TokenList` parallel arrays (`tokens`, `tags`, `flags`) — replaced by `CompactTokenStream`
- [ ] Keep `TokenList` available for external consumers (e.g., tests) if needed, or remove entirely
- [ ] Update Salsa query types to use `CompactTokenStream`
- [ ] Clean up `ori_ir/src/token/` — remove unused code
- [ ] Run `./test-all.sh` to verify nothing broke

---

## 05.6 Performance Validation

- [ ] Run `/benchmark short` before parser migration (record baseline — includes Section 01+02+03+04 gains)
- [ ] Run `/benchmark short` after parser migration
- [ ] Measure:
  - [ ] Parse throughput (tokens/sec) — expect improvement from cache density
  - [ ] Cooking rate: what fraction of tokens actually get cooked? (expect 30-50%)
  - [ ] End-to-end `ori check` time on representative files
- [ ] `perf stat`:
  - [ ] L1 cache misses during parse (expect significant reduction)
  - [ ] Instructions retired during parse
- [ ] No regressions in any existing test
- [ ] Run `./test-all.sh` clean

**Exit Criteria:** Parser produces identical AST for all existing test inputs. End-to-end `ori check` time shows measurable improvement. Lazy cooking rate confirms 30-50% of tokens are never cooked. All spec tests pass.
