---
section: "02"
title: "Lazy Cooking"
status: not-started
goal: "Defer keyword resolution, string interning, and numeric parsing to first access, avoiding 30-50% of cooking work"
sections:
  - id: "02.1"
    title: "Design LazyCooker API"
    status: not-started
  - id: "02.2"
    title: "Implement LazyCooker"
    status: not-started
  - id: "02.3"
    title: "Cache Strategy"
    status: not-started
  - id: "02.4"
    title: "Performance Validation"
    status: not-started
---

# Section 02: Lazy Cooking

**Status:** Planned
**Goal:** Replace the current eager cooking model (all tokens cooked during lexing) with a lazy/on-demand model where `TokenKind` values are materialized only when the parser inspects them.

---

## Background

### Current Model: Eager Cooking

```
RawScanner produces (RawTag, len)
  --> TokenCooker.cook() called for EVERY non-trivia token
  --> Keyword lookup (nested match on string content)
  --> String interning (hash + lookup + possibly allocate)
  --> Numeric parsing (digit iteration + overflow check)
  --> Escape processing (byte-by-byte for strings/chars)
  --> Result: TokenKind (16 bytes, fully resolved)
```

This work is wasted when:
- **Parser error recovery** skips tokens to resynchronize (~10-30% of tokens in error scenarios)
- **Salsa early-cutoff** determines the parse result is unchanged (common for whitespace-only edits — all cooking work is thrown away)
- **Lookahead** — parser peeks ahead to decide a grammar rule, then backtracks; peeked tokens were cooked for nothing
- **Formatter** only needs tags + spans + flags, never inspects `TokenKind` payloads

### Target Model: Lazy Cooking

```
RawScanner produces (RawTag, len)
  --> Store in CompactTokenStream as (tag, offset, flags)
  --> NO cooking during lexing
  --> Parser calls cooker.cook(index) ONLY when it needs the value
  --> Results cached for repeated access
```

### What Gets Deferred

| Token Type | Current (Eager) | Lazy | Savings |
|-----------|----------------|------|---------|
| Operators/delimiters | RawTag → TokenKind (trivial map) | Tag byte is sufficient for parser dispatch | 100% — never cooked |
| Keywords | String comparison (length-bucketed) | Deferred to first access | Most keywords identified by tag alone after SIMD pass |
| Identifiers | Keyword lookup + intern | Intern on first access | ~30% never accessed (error recovery, lookahead) |
| Integer literals | Parse digits, check overflow | Parse on first access | 100% deferred |
| Float literals | Parse, convert to bits | Parse on first access | 100% deferred |
| String literals | Unescape + intern | Unescape + intern on first access | 100% deferred |
| Duration/Size | Parse + detect suffix | Parse on first access | 100% deferred |

### Key Insight: Operators and Keywords Don't Need Cooking

For operators (`+`, `->`, `==`, etc.) and delimiters (`(`, `)`, `{`, etc.), the `RawTag` discriminant byte already uniquely identifies the token. The parser's hot loop just needs to check "is this a `LParen`?" — it doesn't need a `TokenKind`. The `RawTag` byte (1 byte) encodes this information identically to what the parser currently reads from `TokenList.tags`.

For keywords, the raw scanner currently emits `RawTag::Ident` and the cooker resolves the keyword. With SIMD classification (Section 03), we can identify keyword candidates during scanning and resolve them lazily. But even without SIMD, the parser can check the tag byte (`RawTag::Ident`) and only cook when it needs to distinguish `if` from `foo`.

---

## 02.1 Design LazyCooker API

- [ ] Define `LazyCooker` struct
  ```rust
  /// Lazy cooking layer that materializes TokenKind on demand.
  ///
  /// Holds a reference to the source bytes and interner, and caches
  /// cooked results to avoid re-cooking on repeated access.
  pub struct LazyCooker<'src> {
      source: &'src [u8],
      interner: &'src StringInterner,
      /// Cached cooked results. `None` = not yet cooked.
      /// Only populated for tokens that have payload (idents, literals).
      cache: Vec<Option<CookedValue>>,
      /// Accumulated cooking errors (same as current TokenCooker).
      errors: Vec<LexError>,
  }
  ```
- [ ] Define `CookedValue` — the lazy result type
  ```rust
  /// The payload portion of a TokenKind, without the discriminant.
  /// Only needed for tokens that carry data (identifiers, literals).
  enum CookedValue {
      /// Identifier or keyword. `None` = keyword (tag is sufficient).
      Ident(Name),
      /// Keyword resolved from identifier text.
      Keyword(TokenKind),
      /// Integer literal value.
      Int(u64),
      /// Float literal value (bits).
      Float(u64),
      /// String literal (interned).
      String(Name),
      /// Char literal.
      Char(char),
      /// Duration literal.
      Duration(u64, DurationUnit),
      /// Size literal.
      Size(u64, SizeUnit),
      /// Template part (interned).
      Template(Name),
      /// Format spec (interned).
      FormatSpec(Name),
      /// Error token (cooking failed).
      Error,
  }
  ```
- [ ] Define access API
  - [ ] `cook(index, tag, offset, len) -> TokenKind` — full materialization
  - [ ] `cook_ident(index, offset, len) -> TokenKind` — identifier/keyword resolution
  - [ ] `cook_literal(index, tag, offset, len) -> TokenKind` — numeric/string parsing
  - [ ] `is_keyword(index, offset, len) -> Option<TokenKind>` — keyword check without full cook
- [ ] Define error collection API
  - [ ] `errors() -> &[LexError]` — same as current
  - [ ] `into_errors() -> Vec<LexError>` — consume

---

## 02.2 Implement LazyCooker

- [ ] Extract cooking logic from current `TokenCooker` into reusable functions
  - [ ] `cook_ident_value(source, offset, len, interner) -> (TokenKind, bool)` — returns (kind, is_contextual_kw)
  - [ ] `cook_int_value(source, offset, len) -> Result<u64, LexError>`
  - [ ] `cook_float_value(source, offset, len) -> Result<u64, LexError>`
  - [ ] `cook_string_value(source, offset, len, interner) -> Result<Name, Vec<LexError>>`
  - [ ] `cook_char_value(source, offset, len) -> Result<char, Vec<LexError>>`
  - [ ] `cook_duration_value(source, offset, len) -> Result<(u64, DurationUnit), LexError>`
  - [ ] `cook_size_value(source, offset, len) -> Result<(u64, SizeUnit), LexError>`
  - [ ] Template cooking functions (head, middle, tail, complete)
- [ ] Implement `LazyCooker::cook()`
  - [ ] Check cache first
  - [ ] If uncached: dispatch on `RawTag`, call appropriate cooking function
  - [ ] Cache result
  - [ ] Return `TokenKind`
- [ ] Implement direct-map path for operators/delimiters
  - [ ] These never need caching — the `RawTag` → `TokenKind` mapping is a trivial `match`
  - [ ] Static lookup table: `RAWTAG_TO_TOKENKIND: [Option<TokenKind>; 256]`
  - [ ] If entry is `Some`, return directly (no cache, no cooking)
  - [ ] If entry is `None`, fall through to lazy cooking
- [ ] Port error accumulation from `TokenCooker`
  - [ ] Errors pushed during cooking (same behavior as current)
  - [ ] `last_cook_had_error()` check still works (compare error count before/after)

---

## 02.3 Cache Strategy

### Option A: Full Cache (Vec<Option<CookedValue>>)

Pre-allocate `Vec<Option<CookedValue>>` with same length as token stream. Each entry starts as `None`, populated on first access. `Option<CookedValue>` is 24 bytes (enum discriminant + largest variant).

- Pro: O(1) access, simple
- Con: 24 bytes/token overhead even for never-cooked tokens
- Net: 6 (compact) + 24 (cache) = 30 bytes/token when fully cooked — WORSE than current

### Option B: Sparse Cache (HashMap<usize, CookedValue>)

Only cache tokens that are actually cooked. Most tokens (operators, delimiters, keywords) don't need caching.

- Pro: Only pays for tokens that carry payload
- Con: HashMap overhead (hashing, pointer chasing)
- Net: 6 (compact) + ~50 bytes per cached entry — worse for pathological cases

### Option C: Inline Cache via Side-Vec (Recommended)

Only tokens with payload need caching. These are: `Ident`, `Int`, `Float`, `String`, `Char`, `Duration`, `Size`, `Template*`, `FormatSpec`. Approximately 30-40% of tokens in typical code.

```rust
struct LazyCooker<'src> {
    source: &'src [u8],
    interner: &'src StringInterner,
    /// Parallel to CompactTokenStream. Entry is set on first cook.
    /// Only tokens with RawTag in {Ident, Int, Float, HexInt, BinInt,
    /// String, Char, Duration, Size, Template*, FormatSpec} populate this.
    values: Vec<CookedSlot>,
    errors: Vec<LexError>,
}

/// Compact cached value. 16 bytes (same as TokenKind).
/// Uninitialized slots use a sentinel discriminant.
#[repr(C)]
union CookedSlot {
    uninit: u64,     // sentinel: 0xFFFF_FFFF_FFFF_FFFF
    name: Name,      // 4 bytes (ident, string, template)
    int_val: u64,    // 8 bytes
    float_bits: u64, // 8 bytes
    char_val: char,  // 4 bytes
    duration: (u64, u8), // 9 bytes (value + unit discriminant)
    size: (u64, u8),     // 9 bytes
}
```

Actually, a simpler approach: since `TokenKind` is already 16 bytes and the cache is only populated on demand, just use `Vec<TokenKind>` initialized to `TokenKind::Eof` (sentinel), and check for sentinel on access.

- [ ] Decide on cache strategy (recommend Option C simplified: `Vec<TokenKind>` with sentinel)
- [ ] Implement cache initialization (lazy — only allocate when first cook happens)
- [ ] Implement cache lookup + populate
- [ ] Benchmark cache hit rate on representative Ori files

---

## 02.4 Performance Validation

- [ ] Run `/benchmark short` before changes (record baseline)
- [ ] Run `/benchmark short` after changes
- [ ] Measure cooking work avoided:
  - [ ] Count tokens cooked vs total tokens (expect 50-70% cooked)
  - [ ] Count tokens cooked more than once (expect ~0 with cache)
- [ ] Profile with `perf stat`:
  - [ ] Instructions retired (expect reduction from avoided cooking)
  - [ ] Time in `StringInterner::intern` (expect reduction)
  - [ ] Time in keyword lookup (expect reduction)
- [ ] No regressions >5% vs baseline
- [ ] Test with error-heavy files (parser error recovery should show bigger gains)

**Exit Criteria:** Lazy cooker produces identical `TokenKind` values as eager cooker for all existing tests. Benchmark shows measurable reduction in instructions retired. Error recovery paths demonstrably skip cooking for skipped tokens.
