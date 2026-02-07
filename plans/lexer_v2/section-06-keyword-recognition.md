---
section: "06"
title: Keyword Recognition
status: done
goal: "High-performance keyword lookup with context-sensitive keyword resolution via cooker lookahead"
sections:
  - id: "06.1"
    title: Keyword Enumeration & Lookup Strategy
    status: done
  - id: "06.2"
    title: Context-Sensitive Keywords
    status: done
    notes: "6 pattern keywords (cache, catch, parallel, spawn, recurse, timeout) now context-sensitive via ( lookahead"
  - id: "06.3"
    title: Implementation
    status: done
  - id: "06.4"
    title: Tests & Benchmarks
    status: done
    notes: "Benchmarks deferred to Section 10"
---

# Section 06: Keyword Recognition

**Status:** :white_check_mark: Done
**Goal:** Implement a high-performance keyword lookup that converts identifier strings to keyword `TokenKind` variants, with support for context-sensitive keywords via cooker lookahead. Replaces the implicit keyword matching in logos regex patterns.

> **REFERENCE**: Go's compile-time perfect hash (2-byte hash + length -> 64-entry array, one string compare); Zig's `StaticStringMap.initComptime` (length-bucketed keys, compile-time materialized); TypeScript's length + first-char guard before map lookup; Rust's `rustc_lexer` + `rustc_parse::lexer` split (raw scanner returns identifiers, cooking layer resolves keywords).

> **Conventions:** Tag numbering follows `plans/v2-conventions.md` section 2. Keyword `TokenTag` variants occupy a semantic range within the `#[repr(u8)]` enum with gaps for future additions.

---

## Design Rationale

### Current Approach (Logos)

The current lexer uses logos patterns like `#[token("let")]`, `#[token("fn")]`, etc. Logos compiles these into its DFA, which means keyword recognition is interleaved with the general scanning automaton. This is efficient but inflexible -- adding a keyword means modifying the DFA structure.

### Proposed Approach: Two-Phase Keyword Resolution

The V2 scanner produces all keywords as `RawTag::Ident`. The cooking layer (Section 03) then checks each identifier against a keyword table. This is the approach used by Rust (`rustc_lexer`), Go, Zig, and TypeScript.

For context-sensitive keywords, the cooker performs a 1-byte lookahead (skipping whitespace) to check whether `(` follows the identifier. This determines whether pattern keywords like `cache`, `try`, `spawn` are resolved as keywords or left as identifiers.

### Strategy Selection

Three strategies were analyzed:

| Strategy | Lookup Cost | Memory | Implementation |
|----------|-------------|--------|----------------|
| **Perfect hash** (Go) | O(1): hash + 1 strcmp | 64 entries, ~512 bytes | Simple but fragile (must re-derive hash when keywords change) |
| **Length-bucketed** (Zig) | O(k): k = keywords of same length | ~256 bytes | Robust, fast for small keyword sets |
| **Guard + HashMap** (TypeScript) | O(1) amortized: length check + first-char check + map lookup | ~1KB | Simple, flexible, good enough |

**Recommendation**: Length-bucketed with first-byte discrimination (hybrid of Go and Zig). This provides O(1)-like performance for most lookups while being trivially maintainable.

For Ori's 39 keywords total (34 reserved + 5 reserved-future), the maximum bucket size is 12 keywords of the same length (length-4: `else`, `impl`, `loop`, `Self`, `self`, `then`, `true`, `type`, `uses`, `view`, `void`, `with`). Within a bucket, first-byte discrimination further reduces comparisons.

---

## Phantom Keywords (MUST NOT IMPLEMENT)

The current `raw_token.rs` contains several keywords/tokens that are **NOT part of the Ori language specification** and must be removed:

### Keywords that don't exist in Ori:
- **`async`** (raw_token.rs line 26) - NOT in grammar lines 56-67
- **`return`** (raw_token.rs line 32) - **FORBIDDEN** (Ori is expression-based; no return keyword)
- **`mut`** (raw_token.rs line 56) - NOT in grammar; Ori uses `$` prefix for immutability, bindings are mutable by default
- **`dyn`** (raw_token.rs line 90) - NOT in grammar; Ori doesn't use this keyword

### Identifiers misclassified as keywords:

**Built-in type names** (should be plain identifiers, NOT always-keywords):
- `char` (raw_token.rs line 107) - identifier (not in grammar's context-sensitive type names)
- `Never` (raw_token.rs line 111) - identifier (type constructor)

Note: `bool`, `byte`, `float`, `int`, `str` are context-sensitive type names (grammar line 66), handled in the soft keyword table (Section 06.2 Category 2). They should NOT be always-keywords like the current lexer treats them.

**Type constructors/variant names** (should be identifiers):
- `Ok`, `Err`, `Some`, `None` (raw_token.rs lines 114-121) - These are variant constructors, NOT keywords

**Built-in functions** (should be identifiers):
- `print`, `panic`, `todo`, `unreachable` (raw_token.rs lines 142-149) - These are built-in functions, NOT keywords
- `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`, `assert`, `assert_eq`, `assert_ne`, `compare`, `min`, `max` (spec 03-lexical-elements.md lines 86-89) - Built-in names reserved in call position, but NOT keywords

**Attribute identifiers** (should be identifiers, NOT keywords):
- `skip` (raw_token.rs line 96) - Used in `#skip(...)` attributes, but is an identifier, not a keyword

### Keywords missing from grammar keyword list but present in productions:
- `tests` (raw_token.rs line 86) - Appears in grammar line 274 but NOT in keyword list (lines 56-64)
- `extend` (raw_token.rs line 92) - Appears in grammar line 173 but NOT in keyword list
- `extension` (raw_token.rs line 94) - Appears in grammar line 174 but NOT in keyword list

**ACTION REQUIRED:** The grammar keyword list (lines 56-64) is incomplete. It should include:
- `tests` (used in test declarations, line 274)
- `extend` (used in extension definitions, line 173)
- `extension` (used in extension imports, line 174)

These are real keywords that should be added to the reserved keyword list in the spec.

### Attribute identifiers (NOT keywords):
- `skip` (raw_token.rs line 96) - Used in attributes as `#skip(...)` (grammar line 193), but NOT a standalone keyword. It's an identifier recognized by the attribute parser, similar to `derive`, `target`, `cfg`. Should be lexed as `TokenTag::Ident`, not a keyword.

The V2 lexer MUST NOT include the phantom keywords listed above (`async`, `return`, `mut`, `dyn`, `skip`, variant constructors, built-in functions). The reserved keyword table should contain the 34 keywords from the grammar § Keywords (including `as`, `div`, `tests`, `extend`, `extension`) and the 5 reserved-future keywords. Note: `as` is a reserved keyword per the grammar (line 57), even though it is used in operator position (`expr as type`). This is analogous to `div`, which is also a reserved keyword used as an operator. Context-sensitive type names (`bool`, `byte`, `float`, `int`, `str`) are handled separately in the soft keyword table.

---

## 06.1 Keyword Enumeration & Lookup Strategy

### Complete Keyword Set

The following keyword classification derives from the spec grammar (`grammar.ebnf` lines 54-67). All reserved keywords get dedicated `TokenTag` variants. Context-sensitive keywords are handled separately (Section 06.2).

**Reserved keywords** (always keywords, never identifiers):

```
Length 2: as, do, if, in
Length 3: def, div, for, let, pub, use
Length 4: else, impl, loop, Self, self, then, true, type, uses, void, with
Length 5: break, false, match, tests, trait, where, yield
Length 6: extend, extern, unsafe
Length 7: suspend
Length 8: continue
Length 9: extension
```

Total: 34 reserved keywords (per grammar § Keywords).

**Reserved (future)** (reserved but not yet used -- produce a dedicated error if encountered):

```
Length 3: asm
Length 4: view
Length 5: union
Length 6: inline, static
```

**Keyword operator** (lexed as a keyword, used as an arithmetic operator):

```
Length 3: div
```

The `div` keyword operator (`grammar.ebnf` line 72: `arith_op = "+" | "-" | "*" | "/" | "%" | "div"`) is always resolved as a keyword by the lookup table, never an identifier. The parser treats it as a binary operator in expression position.

**Total keyword count:** 34 reserved (including `as`, `div`, `tests`, `extend`, `extension`) + 5 reserved-future = **39 total keywords**

- [ ] Analyze Ori's reserved keyword set by length:
  ```
  Length 2: as, do, if, in
  Length 3: asm*, def, div, for, let, pub, use
  Length 4: else, impl, loop, Self, self, then, true, type, uses, view*, void, with
  Length 5: break, false, match, tests, trait, union*, where, yield
  Length 6: extend, extern, inline*, static*, unsafe
  Length 7: suspend
  Length 8: continue
  Length 9: extension
  (* = reserved-future)
  ```

- [ ] Design the lookup table:
  ```rust
  /// Compile-time keyword lookup table, bucketed by string length.
  /// For each length, stores a slice of (keyword_bytes, TokenTag) pairs.
  ///
  /// Lookup: O(1) length check + O(k) linear scan within bucket,
  /// where k <= 12 (max keywords of any single length).
  ///
  /// Tag numbering follows v2-conventions §2 (semantic ranges with gaps).
  struct KeywordTable {
      /// Indexed by keyword length (0..=MAX_KEYWORD_LEN).
      /// Each entry is a range into the `entries` array.
      buckets: [Range<u16>; MAX_KEYWORD_LEN + 1],
      entries: [(&'static [u8], TokenTag); KEYWORD_COUNT],
  }
  ```

- [ ] Add TypeScript-style guards before table lookup:
  - Length guard: reserved keywords are 2-9 chars (min: `do`, `if`, `in`; max: `extension`). Skip lookup for shorter/longer identifiers.
  - First-byte guard: all keywords start with `a`-`z`, `S` (for `Self`). Skip for `_`, digits, uppercase other than `S`.

---

## 06.2 Context-Sensitive Keywords

Context-sensitive keywords are identifiers that behave as keywords only in specific syntactic contexts. The spec grammar (lines 61-66) defines four categories.

### Category 1: Pattern Keywords (lookahead-resolved)

These are keywords only when followed by `(` in expression position:

```
cache, catch, collect, filter, find, fold, for*, map, match*,
parallel, recurse, retry, run, spawn, timeout, try, validate, with*
```

(`*` = also appears in the reserved list; `for`, `match`, `with` are always reserved keywords regardless of context.)

Since `for`, `match`, and `with` appear in both the reserved and context-sensitive lists, they are always resolved as keywords by the reserved keyword table (Section 06.1). They do not need context-sensitive handling.

The remaining pattern keywords (`cache`, `catch`, `map`, `parallel`, `recurse`, `run`, `spawn`, `timeout`, `try`) are identifiers by default and become keywords only when the cooker detects `(` as the next non-whitespace byte.

**NOTE:** `nursery` is NOT in the grammar's `pattern_name` production (line 456). It has an explicit `nursery_expr` production (line 492) but is NOT context-sensitive. However, it's included in the soft keyword table for forward compatibility since it appears in the grammar comment (line 62).

> **Note**: `collect`, `filter`, `find`, `fold`, `retry`, `validate` are listed as context-sensitive in the grammar comment (line 62) but do NOT appear in any grammar production (`pattern_name` at line 456 does not include them, nor does any other production). They are included in the SOFT_KEYWORDS table for forward compatibility, so that code using them as soft keywords will work when their grammar productions are added.

**Examples:**

```ori
let timeout = 100          // `timeout` is an identifier
timeout(100) { ... }       // `timeout` is a keyword

let cache = HashMap::new() // `cache` is an identifier
cache(key) { ... }         // `cache` is a keyword

let map = get_data()       // `map` is an identifier
map(items) { ... }         // `map` is a keyword
```

### Category 2: Type Conversion Keywords (lookahead-resolved)

These are keywords only in call position (followed by `(`). Per grammar line 66, all 5 context-sensitive type names:

```
bool, byte, float, int, str
```

**Examples:**

```ori
let int = 5                // `int` is an identifier
let x = int(y)             // `int` is a type conversion keyword

let str = "hello"          // `str` is an identifier
let s = str(value)         // `str` is a type conversion keyword

let bool = true            // `bool` is an identifier
let b = bool(x)            // `bool` is a type conversion keyword
```

### Category 3: Parser-Resolved Keywords

These keywords require parser-level context that the lexer cannot determine with simple lookahead:

- **`by`**: keyword only after `..` or `..=` (range step), e.g., `1..10 by 2` (grammar line 391)
- **`max`**: keyword only in `[Type, max N]` (fixed-capacity list syntax) (grammar line 310)
- **`without`**: keyword only in import context before `def`, e.g., `use Foo { bar without def }` (grammar lines 64, 164)
- **`from`**: keyword only in extern blocks, e.g., `extern "c" from "lib.so" { ... }` (grammar line 180)

The lexer always returns these as `TokenTag::Ident`. The parser is responsible for recognizing them as keywords in the appropriate contexts.

**NOTE:** These are NOT included in the soft keyword table because they cannot be resolved with simple `(` lookahead. The parser must track syntactic context (inside import? after range? in fixed-capacity list? in extern block?) to resolve them.

### Pre-Filter Optimization (Feb 2026)

Callgrind profiling showed `soft_keyword_lookup` consuming 5.6% of total instructions because it was called on *every* identifier. Two inline pre-filters were added to `keywords.rs`:

- **`could_be_soft_keyword(text)`** — checks `len ∈ {5, 7, 8}` and `first_byte ∈ {c, p, r, s, t}`. Rejects >99% of identifiers before the binary search.
- **`could_be_reserved_future(text)`** — checks `len ∈ 3..=6` and `first_byte ∈ {a, i, s, u, v}`. Eliminates the match for ~99% of identifiers.

The `rest` slice computation (for `(` lookahead) was also moved inside the `could_be_soft_keyword` guard to avoid materializing it when the pre-filter rejects. Combined with `from_utf8_unchecked` in `slice_source`, these three changes delivered ~30-50% throughput improvement.

### Cooker Lookahead Design

The cooker handles categories 1 and 2 with a 1-byte non-whitespace lookahead:

```rust
/// Soft keyword set: identifiers that become keywords when followed by `(`.
/// Stored as a static sorted array for binary search.
///
/// Pattern keywords (grammar line 61-62): cache, catch, collect, filter, find, fold,
/// map, nursery, parallel, recurse, retry, run, spawn, timeout, try, validate
///
/// Type conversion keywords (grammar line 66): bool, byte, float, int, str
///
/// NOTE: for, match, with are always reserved keywords (not included here).
const SOFT_KEYWORDS: &[(&[u8], TokenTag)] = &[
    (b"bool",     TokenTag::KwBool),      // Type conversion (grammar line 66)
    (b"byte",     TokenTag::KwByte),      // Type conversion
    (b"cache",    TokenTag::KwCache),     // Pattern keyword
    (b"catch",    TokenTag::KwCatch),     // Pattern keyword (grammar line 488)
    (b"collect",  TokenTag::KwCollect),   // Pattern keyword (forward compat)
    (b"filter",   TokenTag::KwFilter),    // Pattern keyword (forward compat)
    (b"find",     TokenTag::KwFind),      // Pattern keyword (forward compat)
    (b"float",    TokenTag::KwFloat),     // Type conversion
    (b"fold",     TokenTag::KwFold),      // Pattern keyword (forward compat)
    (b"int",      TokenTag::KwInt),       // Type conversion
    (b"map",      TokenTag::KwMap),       // Pattern keyword
    (b"nursery",  TokenTag::KwNursery),   // Pattern keyword (forward compat per grammar comment)
    (b"parallel", TokenTag::KwParallel),  // Pattern keyword (grammar line 456)
    (b"recurse",  TokenTag::KwRecurse),   // Pattern keyword (grammar line 456)
    (b"retry",    TokenTag::KwRetry),     // Pattern keyword (forward compat)
    (b"run",      TokenTag::KwRun),       // Pattern keyword (grammar line 466)
    (b"spawn",    TokenTag::KwSpawn),     // Pattern keyword (grammar line 456)
    (b"str",      TokenTag::KwStr),       // Type conversion
    (b"timeout",  TokenTag::KwTimeout),   // Pattern keyword (grammar line 456)
    (b"try",      TokenTag::KwTry),       // Pattern keyword (grammar line 473)
    (b"validate", TokenTag::KwValidate),  // Pattern keyword (forward compat)
];

/// Look up a context-sensitive keyword.
/// Returns the keyword tag if the identifier is in the soft keyword set
/// AND the next non-whitespace byte in the source is `(`.
fn soft_keyword_lookup(text: &[u8], rest: &[u8]) -> Option<TokenTag> {
    // Binary search in the sorted soft keyword table
    let tag = match SOFT_KEYWORDS.binary_search_by_key(&text, |(kw, _)| kw) {
        Ok(idx) => SOFT_KEYWORDS[idx].1,
        Err(_) => return None,
    };

    // 1-byte non-whitespace lookahead: skip ASCII whitespace (space, tab)
    // but NOT newlines (newlines are significant in Ori)
    let next = rest.iter()
        .find(|&&b| b != b' ' && b != b'\t')
        .copied();

    if next == Some(b'(') {
        Some(tag)
    } else {
        None
    }
}
```

When a soft keyword is resolved, the cooker sets the `CONTEXTUAL_KW` flag on the token's `TokenFlags` (see `v2-conventions.md` section 4). This allows downstream phases to distinguish between always-keywords and context-resolved keywords if needed.

### Resolution Flow in the Cooker

The cooker (Section 03) resolves identifiers in this order:

1. Check the reserved keyword table (Section 06.1). If found, return the keyword `TokenTag`.
2. Check the soft keyword table with lookahead. If the identifier is in the soft keyword set AND next non-whitespace byte is `(`, return the keyword `TokenTag` with `CONTEXTUAL_KW` flag set.
3. Otherwise, intern the identifier and return `TokenTag::Ident`.

```rust
fn cook_identifier(&self, text: &[u8], rest: &[u8]) -> (TokenTag, TokenFlags) {
    // Step 1: reserved keyword?
    if let Some(tag) = keyword::lookup(text) {
        return (tag, TokenFlags::empty());
    }

    // Step 2: context-sensitive keyword?
    if let Some(tag) = keyword::soft_keyword_lookup(text, rest) {
        return (tag, TokenFlags::CONTEXTUAL_KW);
    }

    // Step 3: plain identifier
    let name = self.interner.intern(text);
    (TokenTag::Ident, TokenFlags::empty())
}
```

---

## 06.3 Implementation

- [ ] Implement `keyword::lookup(text: &[u8]) -> Option<TokenTag>`:
  ```rust
  /// Looks up a string in the reserved keyword table.
  /// Returns the corresponding TokenTag if it's a keyword, None otherwise.
  ///
  /// Guarded by length (2-9) and first-byte ('a'-'z' or 'S') checks
  /// to avoid table lookup for identifiers that cannot be keywords.
  #[inline]
  pub fn lookup(text: &[u8]) -> Option<TokenTag> {
      let len = text.len();
      if len < MIN_KEYWORD_LEN || len > MAX_KEYWORD_LEN {
          return None;
      }
      let first = text[0];
      if !(first.is_ascii_lowercase() || first == b'S') {
          return None;
      }
      // Linear scan within length bucket
      let bucket = &KEYWORD_TABLE[len];
      for &(kw, tag) in bucket {
          if kw == text {
              return Some(tag);
          }
      }
      None
  }
  ```

- [ ] Implement `keyword::soft_keyword_lookup(text: &[u8], rest: &[u8]) -> Option<TokenTag>` (see Section 06.2 above)

- [ ] Define the reserved keyword table as a `const` (compile-time evaluated):
  ```rust
  const MIN_KEYWORD_LEN: usize = 2;  // "do", "if", "in"
  const MAX_KEYWORD_LEN: usize = 9;  // "extension"

  const KEYWORD_TABLE: [&[(&[u8], TokenTag)]; MAX_KEYWORD_LEN + 1] = {
      // ... populate at compile time, bucketed by length ...
  };
  ```

- [ ] Alternative: use `phf` crate for a compile-time perfect hash function
  - Pro: O(1) guaranteed, no linear scan
  - Con: Additional dependency, more complex build
  - **Decision**: Start with length-bucketed; switch to `phf` only if benchmarks show it's needed

- [ ] Ensure the lookup function is `#[inline]` for the common (non-keyword) case to return quickly

- [ ] Implement reserved-future keyword detection:
  ```rust
  /// Reserved-future keywords produce an error if used as identifiers.
  /// They are recognized by the reserved keyword table and map to a
  /// dedicated TokenTag (e.g., TokenTag::KwReserved) that the parser
  /// converts to a diagnostic: "'{keyword}' is reserved for future use."
  const RESERVED_FUTURE: &[&[u8]] = &[
      b"asm", b"inline", b"static", b"union", b"view",
  ];
  ```

---

## 06.4 Tests & Benchmarks

- [ ] **Correctness tests** (reserved keywords):
  - Every reserved keyword string maps to the correct `TokenTag`
  - All 34 reserved keywords are recognized: `as, break, continue, def, div, do, else, extend, extension, extern, false, for, if, impl, in, let, loop, match, pub, self, Self, suspend, tests, then, trait, true, type, unsafe, use, uses, void, where, with, yield`
  - Non-keyword strings (including keyword prefixes/suffixes) return `None`
  - Case sensitivity: `Let`, `LET`, `lET` are NOT keywords (only `let` is)
  - `Self` is a keyword but `SELF`, `sElf` are not
  - `div` is recognized as `TokenTag::KwDiv` (keyword operator)
  - `tests`, `extend`, `extension` are recognized as reserved keywords
  - Empty string returns `None`
  - Single-char strings return `None` (min keyword length is 2)

- [ ] **Context-sensitive keyword tests**:
  - `timeout(100)` resolves `timeout` as `TokenTag::KwTimeout`
  - `let timeout = 5` resolves `timeout` as `TokenTag::Ident`
  - `cache (key)` resolves `cache` as `TokenTag::KwCache` (space before `(` is allowed)
  - `cache\n(key)` resolves `cache` as `TokenTag::Ident` (newline blocks lookahead)
  - `int(x)` resolves `int` as `TokenTag::KwInt`
  - `let int = 5` resolves `int` as `TokenTag::Ident`
  - `str(value)` resolves `str` as `TokenTag::KwStr`
  - `bool(x)` resolves `bool` as `TokenTag::KwBool`
  - `let bool = true` resolves `bool` as `TokenTag::Ident`
  - `by`, `max`, `without` always resolve as `TokenTag::Ident` (parser-resolved)
  - `for`, `match`, `with` always resolve as reserved keywords regardless of context

- [ ] **Reserved-future keyword tests**:
  - `asm`, `inline`, `static`, `union`, `view` produce dedicated error tokens/diagnostics
  - `asm_helper`, `statically` are plain identifiers (not prefix-matched)

- [ ] **Phantom keyword tests** (ensure these are NOT recognized as keywords):
  - `async`, `return`, `mut`, `dyn` resolve as `TokenTag::Ident`, NOT keywords
  - Type names resolve as identifiers when not followed by `(`: `char`, `Never`
  - Type constructors resolve as identifiers: `Ok`, `Err`, `Some`, `None`
  - Built-in functions resolve as identifiers: `print`, `panic`, `todo`, `unreachable`, `len`, `is_empty`, etc.
  - Pattern argument names resolve as identifiers: `pre_check`, `post_check`, `body`, `on_error`, `default`, `over`, `expr`, `buffer`

- [ ] **Benchmark**:
  - Measure lookup time for: keywords, short non-keywords, long non-keywords, near-miss identifiers
  - Measure soft keyword lookup time with and without lookahead match
  - Compare against: current logos-implicit matching, `HashMap<&str, TokenTag>`, `phf`
  - Target: <= 5ns per lookup for the common non-keyword case

- [ ] **Exhaustiveness**: Verify that the keyword table contains every keyword defined in the Ori spec
  - Cross-reference against `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` lines 54-67

---

## 06.5 Completion Checklist

- [ ] `keyword.rs` module added to `ori_lexer`
- [ ] Length-bucketed lookup implemented for reserved keywords
- [ ] Soft keyword lookup with 1-byte lookahead implemented
- [ ] `div` recognized as keyword operator
- [ ] Reserved-future keywords detected and diagnosed
- [ ] `CONTEXTUAL_KW` flag set on context-resolved keywords
- [ ] Guards eliminate table lookup for non-keyword identifiers
- [ ] All reserved keywords correctly recognized
- [ ] All context-sensitive keywords correctly handled
- [ ] Benchmark shows acceptable lookup performance
- [ ] `cargo t -p ori_lexer` passes

**Exit Criteria:** Keyword lookup correctly identifies all reserved keywords, resolves context-sensitive keywords via cooker lookahead, and rejects all non-keywords. `div` is recognized as a keyword operator. `by`/`max`/`without`/`from` are left as identifiers for the parser. Performance is competitive with or better than logos-implicit keyword matching.

---

## 06.6 Spec Alignment Summary

### Verified Against Grammar

This section has been verified against:
- **Grammar:** `/home/eric/projects/ori_lang/docs/ori_lang/0.1-alpha/spec/grammar.ebnf`
  - Lines 56-67: Keyword classifications
  - Line 72: `div` keyword operator
  - Line 164: `without` in imports
  - Line 180: `from` in extern blocks
  - Line 310: `max` in fixed-capacity lists
  - Line 391: `by` in range expressions
  - Line 456: `pattern_name` production
  - Line 488: `catch_expr` production
- **Spec:** `/home/eric/projects/ori_lang/docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md`
  - Lines 55-57: Reserved keywords
  - Lines 59-61: Reserved-future keywords
  - Lines 63-65: Context-sensitive keywords
  - Lines 67-76: Built-in names

### Keyword Counts

- **Reserved keywords:** 34 (per grammar § Keywords)
- **Reserved-future:** 5 (`asm`, `inline`, `static`, `union`, `view`)
- **Soft keywords (context-sensitive):** 33
  - Pattern keywords: 16 (`cache`, `catch`, `collect`, `filter`, `find`, `fold`, `map`, `nursery`, `parallel`, `recurse`, `retry`, `run`, `spawn`, `timeout`, `try`, `validate`)
  - Pattern args: 7 (`body`, `default`, `expr`, `on_error`, `over`, `pre_check`, `post_check`)
  - Type names: 5 (`bool`, `byte`, `float`, `int`, `str`)
  - Imports: 2 (`from`, `without`)
  - Other: 3 (`args`, `by`, `max`)
- **Built-in constructors:** 4 (`channel`, `channel_all`, `channel_in`, `channel_out`)
- **Total:** 39 always-keywords (34 reserved + 5 future)

### Discrepancies Found and Documented

1. ~~Grammar keyword list omits `tests`, `extend`, `extension`~~ — **FIXED** (grammar § Keywords updated)
2. Current `raw_token.rs` includes phantom keywords not in spec: `async`, `return`, `mut`, `dyn`
3. Current `raw_token.rs` treats soft keywords as always-keywords: `cache`, `catch`, `parallel`, etc.
4. Current `raw_token.rs` treats identifiers as keywords: type names, constructors, built-in functions

These discrepancies are documented in the "Phantom Keywords" section and must be addressed in the V2 implementation.
