# Proposal: std.text — Comprehensive Text Processing Library

**Status:** Approved
**Approved:** 2026-04-02
**Created:** 2026-04-02
**Author:** Eric (with AI assistance)
**Affects:** Standard library, `library/std/text/`
**Related:** stdlib-philosophy-proposal.md (approved), stdlib-regex-ffi-proposal.md (draft)
**Prior art:** [Pretext](https://github.com/chenglou/pretext) (Cheng Lou), ICU4X (Unicode Consortium), Swift String, Elixir String, Rust unicode-* crates

---

## Summary

This proposal defines `std.text` — a comprehensive, layered text processing library that covers Unicode character properties, grapheme cluster segmentation, display width calculation, text normalization, case folding, string similarity, text analysis, pluggable text measurement, production-quality line breaking and layout, bidirectional text, confusable detection, encoding conversion, and high-level convenience functions (wrap, truncate, pad, slugify).

The library is organized in six tiers: Unicode foundation, display width, text analysis (ported from Pretext), pluggable measurement and layout, string similarity, and text transforms. Each tier builds on the previous, and all tiers share underlying Unicode data tables.

---

## Motivation

### The Universal Problem

Every major programming language fails at text processing in the same ways:

1. **No grapheme cluster support** — Rust, Go, Python, Java, and Zig all operate at the byte or codepoint level. `"👨‍👩‍👧‍👦".len()` returns 7 codepoints (Python), 11 UTF-16 units (JavaScript), or 25 bytes (Go) — but a human sees 1 character. Only Swift and Elixir handle this correctly in their standard libraries.

2. **No display width calculation** — `string-width` has 75 million weekly npm downloads. `unicode-width` has 130 million all-time Rust downloads. `go-runewidth` is depended upon by virtually every Go TUI. Yet no standard library provides this function.

3. **No Unicode-aware text wrapping** — Python's `textwrap` is the only standard library word-wrapper in any language, and it's broken for CJK text (counts characters, not display columns). CJK text has no spaces between words. Thai and Khmer need dictionary-based segmentation. Arabic has right-to-left punctuation rules. Nobody handles all of these.

4. **No text layout engine** — No standard library in any language provides a text measurement and layout system. Applications that need text layout (terminal UIs, GPU widgets, text editors, browser engines) must each implement their own line-breaking algorithm or pull in platform-specific APIs.

5. **No string similarity** — Only Elixir provides edit distance and Jaro-Winkler in its standard library. Every compiler needs "did you mean?" suggestions. Every search needs fuzzy matching. Every data pipeline needs deduplication. Everyone reimplements these.

6. **No normalization** — `"café"` (NFC, 4 codepoints) and `"café"` (NFD, 5 codepoints) look identical to humans but `==` returns false in every language except Swift. macOS stores filenames in NFD; Linux uses NFC. This causes real bugs in file comparison, database lookups, and user registration.

### Why This Belongs in std

Per the [stdlib philosophy proposal](../approved/stdlib-philosophy-proposal.md), a package belongs in `std.*` if it has **nearly universal need**, covers a **stable domain**, and benefits from **shared infrastructure**. `std.text` satisfies all three:

- **Nearly universal need**: Every program that outputs text to a terminal, formats a table, wraps a paragraph, compares user input, generates an error message, or processes multilingual data needs these functions.
- **Stable domain**: Unicode algorithms (UAX #9, #11, #14, #15, #29) are formally specified and stable across versions. Text layout is a solved problem (Pretext, ICU, HarfBuzz). These are not rapidly evolving.
- **Shared infrastructure**: Unicode property tables (East Asian Width, grapheme break properties, line break classes) are ~200KB of compiled data. Sharing them across `graphemes()`, `display_width()`, `wrap()`, `line_breaks()`, and `normalize()` avoids duplication. Separate crates would each embed their own copy.

### Why Now

The existing `library/std/text/mod.ori` is a TODO stub listing basic functions like `pad_left`, `words`, `wrap`. The `std.text.regex` submodule exists but is also a stub. There is no implementation. This proposal replaces the stub with a comprehensive design before any implementation commitments are made.

The availability of Pretext (cloned to `~/projects/reference_repos/pretext/`) provides a production-quality reference for the text analysis and layout components. Pretext's algorithms are browser-tested across Chrome, Safari, and Firefox with accuracy verified against CJK, Arabic, Thai, Myanmar, and emoji corpora.

---

## Design Principles

### 1. Correct by Default, Fast When You Need It

Display-oriented operations (`display_width`, `wrap`, `truncate`) work on grapheme clusters and terminal columns — the units humans perceive. Byte-oriented operations (`.byte_len()`, `.as_bytes()`, `.contains()`) remain available at O(1) or O(n) with small constants. The API makes the correct choice easy and the fast choice explicit.

### 2. One Library, Many Layers

Not 15 separate crates. A coherent module where `unicode` feeds `analysis`, `analysis` feeds `layout`, and everything composes via traits and iterators. Import what you need: `use std.text { wrap }` pulls in the minimum; `use std.text.layout { prepare, layout_lines }` gets you the full engine.

### 3. Measurement-Agnostic Layout

The key insight from Pretext: separate *what to break* (linguistic analysis) from *how wide is it* (measurement). The analysis pipeline is pure — no platform dependencies. The layout engine takes a `TextMeasure` trait. Terminals, GPU glyph atlases, browser canvas, HarfBuzz — all the same algorithms, different measurement backends.

### 4. Pure by Default

Almost everything in `std.text` is pure (no capabilities required). Only locale-dependent operations that access system locale data would need a capability, and those are deferred to a future `std.i18n` proposal. Everything in this proposal is capability-free.

### 5. Layered Complexity

Simple tasks are simple. Advanced tasks are possible. The same underlying engine powers both:

```ori
// Simple: one function call
let lines = wrap(text, 80)

// Advanced: full control over measurement, analysis, and layout
let measurer = CachedMeasure { inner: my_gpu_font }
let prepared = prepare(text, measurer, PrepareOptions { white_space: PreWrap })
for line in layout_lines(prepared, column_width) do
    render_line(line.text, line.width)
```

---

## Architecture

### Module Structure

```
std.text
├── (root module)              High-level convenience API
├── unicode/                   Unicode foundation (Layer 0)
│   ├── (root)                 Character properties
│   ├── segmentation           Grapheme/word/sentence/line break (UAX #29, #14)
│   ├── normalization          NFC/NFD/NFKC/NFKD (UAX #15)
│   ├── bidi                   Bidirectional algorithm (UAX #9)
│   └── security               Confusable detection (UTS #39)
├── width/                     Display width (UAX #11) (Layer 1)
├── similarity/                Fuzzy matching (Layer 2)
├── case/                      Case folding and conversion (Layer 3)
├── analysis/                  Pretext analysis pipeline (Layer 4)
├── measure/                   Pluggable text measurement (Layer 5)
├── layout/                    Line breaking and layout engine (Layer 6)
│   └── inline_flow            Mixed inline content layout
├── transform/                 Text transformations (Layer 7)
│   ├── slug                   URL-safe slugification
│   ├── ansi                   ANSI escape code handling
│   └── encoding               Legacy text encoding conversion
└── regex/                     Regular expressions (existing, separate proposal)
```

### Dependency Graph

```
                    ┌──────────┐
                    │ unicode  │  ← Foundation: properties, segmentation,
                    │ (Layer 0)│     normalization, bidi, security
                    └────┬─────┘
                         │
              ┌──────────┼──────────┐
              │          │          │
        ┌─────▼───┐ ┌───▼────┐ ┌──▼──────┐
        │  width  │ │  case  │ │similarity│
        │(Layer 1)│ │(Layer 3)│ │(Layer 2) │
        └────┬────┘ └────────┘ └──────────┘
             │
        ┌────▼─────┐
        │ analysis │  ← Pretext's segment merging pipeline
        │(Layer 4) │     Uses: unicode.segmentation, unicode.props
        └────┬─────┘
             │
        ┌────▼─────┐
        │ measure  │  ← TextMeasure trait + built-in impls
        │(Layer 5) │     Uses: width (for TerminalMeasure)
        └────┬─────┘
             │
        ┌────▼─────┐
        │  layout  │  ← Line breaking engine
        │(Layer 6) │     Uses: analysis, measure
        └────┬─────┘
             │
        ┌────▼──────┐
        │ std.text  │  ← High-level convenience (wrap, truncate, pad, etc.)
        │  (root)   │     Uses: layout, width, analysis
        └───────────┘
```

---

## Detailed API Design

### Layer 0: `std.text.unicode` — Unicode Foundation

#### `std.text.unicode` (root) — Character Properties

Based on Unicode Character Database (UAX #44). Provides access to character classification properties used by all higher layers.

```ori
// ── Types ────────────────────────────────────────────────────────────
type GeneralCategory =
    // Letters
    UppercaseLetter | LowercaseLetter | TitlecaseLetter
  | ModifierLetter | OtherLetter
    // Marks
  | NonspacingMark | SpacingMark | EnclosingMark
    // Numbers
  | DecimalNumber | LetterNumber | OtherNumber
    // Punctuation
  | ConnectorPunctuation | DashPunctuation | OpenPunctuation
  | ClosePunctuation | InitialPunctuation | FinalPunctuation
  | OtherPunctuation
    // Symbols
  | MathSymbol | CurrencySymbol | ModifierSymbol | OtherSymbol
    // Separators
  | SpaceSeparator | LineSeparator | ParagraphSeparator
    // Other
  | Control | Format | Surrogate | PrivateUse | Unassigned

type Script =
    Latin | Greek | Cyrillic | Armenian | Hebrew | Arabic | Syriac
  | Thaana | Devanagari | Bengali | Gurmukhi | Gujarati | Oriya
  | Tamil | Telugu | Kannada | Malayalam | Sinhala | Thai | Lao
  | Tibetan | Myanmar | Georgian | Hangul | Ethiopic | Cherokee
  | CanadianAboriginal | Ogham | Runic | Khmer | Mongolian | Han
  | Hiragana | Katakana | Bopomofo | Yi | OldItalic | Gothic
  | Deseret | Tagalog | Hanunoo | Buhid | Tagbanwa
  | Common | Inherited | Unknown
  // ... remaining scripts per Unicode 16.0

// ── Functions ────────────────────────────────────────────────────────
@general_category (c: char) -> GeneralCategory
@script (c: char) -> Script

// Derived properties (efficient: single lookup, not category comparison)
@is_letter (c: char) -> bool           // Lu | Ll | Lt | Lm | Lo
@is_uppercase (c: char) -> bool        // Lu
@is_lowercase (c: char) -> bool        // Ll
@is_digit (c: char) -> bool            // Nd (decimal digit)
@is_numeric (c: char) -> bool          // Nd | Nl | No
@is_whitespace (c: char) -> bool       // Unicode White_Space property
@is_alphabetic (c: char) -> bool       // Unicode Alphabetic property
@is_alphanumeric (c: char) -> bool     // Alphabetic or Nd
@is_punctuation (c: char) -> bool      // Pc | Pd | Ps | Pe | Pi | Pf | Po
@is_symbol (c: char) -> bool           // Sm | Sc | Sk | So
@is_control (c: char) -> bool          // Cc
@is_combining_mark (c: char) -> bool   // Mn | Mc | Me

// Script-specific classification
@is_cjk (c: char) -> bool             // CJK Unified Ideographs + all extensions
@is_hangul (c: char) -> bool           // Hangul syllables + jamo
@is_arabic_script (c: char) -> bool    // Script=Arabic
@is_emoji_presentation (c: char) -> bool  // Emoji_Presentation property

// Identifier properties (UAX #31) — useful for parsers/compilers
@is_xid_start (c: char) -> bool       // Can start an identifier
@is_xid_continue (c: char) -> bool    // Can continue an identifier
```

**Implementation**: Rust code in `ori_rt` using compile-time-generated lookup tables (two-level trie, ~80KB for all properties). Tables generated from Unicode Character Database files by a build script, matching the approach used by Rust's `unicode-segmentation` crate.

#### `std.text.unicode.segmentation` — Text Segmentation (UAX #29, UAX #14)

The single most important submodule. Provides grapheme cluster, word, sentence, and line break boundaries.

```ori
// ── Grapheme Clusters (UAX #29) ──────────────────────────────────────

@graphemes (s: str) -> Iterator<str>
// Iterates extended grapheme clusters.
// "👨‍👩‍👧‍👦"  → ["👨‍👩‍👧‍👦"]          (1 cluster)
// "café"   → ["c", "a", "f", "é"]   (4 clusters, regardless of NFC/NFD)
// "한국어"  → ["한", "국", "어"]      (3 clusters)

@grapheme_count (s: str) -> int
// Equivalent to graphemes(s) |> .count(), but may be optimized.

@grapheme_indices (s: str) -> Iterator<(int, str)>
// Yields (byte_offset, grapheme_cluster) pairs.
// Useful for building index mappings between byte and grapheme positions.

@is_grapheme_boundary (s: str, byte_offset: int) -> bool
// Returns true if the byte offset falls on a grapheme cluster boundary.
// Always true for 0 and byte_len(s).

@snap_to_grapheme_boundary (s: str, byte_offset: int) -> int
// Snaps a byte offset to the nearest grapheme boundary (rounds down).
// Useful for safe truncation: never splits a grapheme cluster.

// ── Word Boundaries (UAX #29) ────────────────────────────────────────

type WordSegment = {
    text: str,
    is_word_like: bool,
    byte_start: int,
}

@words (s: str) -> Iterator<WordSegment>
// Segments text at word boundaries per UAX #29.
// Handles CJK (per-character segmentation), Thai (dictionary-based in
// future; currently delegated to UAX #29 rules), Arabic, etc.
// "Hello, 世界!" → [
//   WordSegment { text: "Hello", is_word_like: true, byte_start: 0 },
//   WordSegment { text: ",",     is_word_like: false, byte_start: 5 },
//   WordSegment { text: " ",     is_word_like: false, byte_start: 6 },
//   WordSegment { text: "世",    is_word_like: true, byte_start: 7 },
//   WordSegment { text: "界",    is_word_like: true, byte_start: 10 },
//   WordSegment { text: "!",     is_word_like: false, byte_start: 13 },
// ]

@word_count (s: str) -> int
// Count of word-like segments only.

// ── Sentence Boundaries (UAX #29) ────────────────────────────────────

@sentences (s: str) -> Iterator<str>
// Segments text at sentence boundaries.
// Useful for NLP, text summarization, TTS.

// ── Line Break Opportunities (UAX #14) ───────────────────────────────

type LineBreakOpportunity = Mandatory | Allowed | Prohibited

type LineBreak = {
    byte_offset: int,
    opportunity: LineBreakOpportunity,
}

@line_breaks (s: str) -> Iterator<LineBreak>
// Raw UAX #14 line break opportunities.
// The analysis pipeline (Layer 4) builds on this with linguistic
// merging for better results. This is the low-level building block.
```

**Implementation**: UAX #29 grapheme break rules implemented as a state machine in `ori_rt`. Grapheme break property table (~15KB). Word break and sentence break properties share the same infrastructure. UAX #14 line break classes (~8KB table) with rule-based state machine.

#### `std.text.unicode.normalization` — Normalization Forms (UAX #15)

```ori
type NormalizationForm = NFC | NFD | NFKC | NFKD

@normalize (s: str, form: NormalizationForm = NormalizationForm.NFC) -> str
// Returns the normalized form of the string.
// normalize("café", NFC)  → "café" (4 codepoints, precomposed é)
// normalize("café", NFD)  → "café" (5 codepoints, decomposed e + ◌́)
// normalize("ﬁ", NFKC)   → "fi"   (compatibility decomposition)

@is_normalized (s: str, form: NormalizationForm = NormalizationForm.NFC) -> bool
// Quick check: returns true without allocating if already normalized.
// Uses the NFC_Quick_Check / NFD_Quick_Check properties.
// Fast path: ASCII strings are always NFC-normalized.

@canonical_equals (a: str, b: str) -> bool
// Returns true if a and b are canonically equivalent.
// "café" (NFC) canonical_equals "café" (NFD)  → true
// Implementation: if byte-equal, return true (fast path).
// Otherwise, normalize both to NFC and compare.

@compatibility_equals (a: str, b: str) -> bool
// Returns true if a and b are compatibility-equivalent.
// "ﬁ" compatibility_equals "fi"  → true

@normalize_stream (chars: Iterator<char>, form: NormalizationForm) -> Iterator<char>
// Streaming normalization for large texts. Buffers only the minimum
// necessary lookahead (bounded by Canonical_Combining_Class runs).
// NOTE: Phase 3+ — complex implementation, not needed for core normalize().
```

**Implementation**: Canonical decomposition and composition mappings (~60KB table). Quick-check properties for fast-path detection. Streaming normalization follows the "stream-safe" text format from UAX #15 Annex 4.

**Design rationale — why not make `==` canonically equivalent by default:**

Swift makes string `==` canonically equivalent. This is elegant but has costs:
- Every string comparison potentially triggers normalization (hidden O(n) with allocation)
- Hash maps keyed by strings would need to normalize for hashing (changing `Hashable` semantics)
- Pattern matching against string literals would need normalization
- Most real-world text is already in NFC, so the normalization rarely changes the result but always pays the cost

Ori's approach: `==` remains byte-equal (fast, predictable, current behavior). `canonical_equals` is one import away for when Unicode correctness matters. This follows Rust's principle of not hiding O(n) costs behind O(1) syntax, while making the correct path trivially accessible.

**Future consideration**: If community feedback strongly favors canonical equivalence as default, a future proposal could change `str`'s `Eq` implementation. The `canonical_equals` function exists regardless.

#### `std.text.unicode.bidi` — Bidirectional Algorithm (UAX #9)

```ori
type BidiClass =
    L | R | AL | AN | EN | ES | ET | CS | ON | BN | B | S | WS | NSM
  | LRE | LRO | RLE | RLO | PDF | LRI | RLI | FSI | PDI

type BidiLevel = byte  // 0 = LTR, 1 = RTL, higher = nested embedding

type Direction = LeftToRight | RightToLeft

@bidi_class (c: char) -> BidiClass
// Returns the Bidi_Class property of a character.

@paragraph_direction (s: str) -> Direction
// Determines the paragraph embedding level (first strong character rule).

@bidi_levels (s: str) -> Option<[BidiLevel]>
// Computes per-character embedding levels per UAX #9.
// Returns None if the text is purely LTR (fast path: no RTL characters).
// The returned array has one entry per code point (not per byte or grapheme).

@has_bidi_controls (s: str) -> bool
// Security check: detects explicit bidi formatting characters
// (LRE, RLE, LRO, RLO, PDF, LRI, RLI, FSI, PDI).
// These can be used for "Trojan Source" attacks (CVE-2021-42574).

@strip_bidi_controls (s: str) -> str
// Removes all explicit bidi formatting characters.

@reorder_visual (s: str, levels: [BidiLevel]) -> str
// Reorders characters from logical order to visual order.
// Used by renderers that need visual-order glyph sequences.
```

**Implementation**: Simplified UAX #9 (W1-W7, N1-N2, I1-I2 rules), matching Pretext's `bidi.ts` implementation which is derived from pdf.js. The full UAX #9 includes bracket pairing (BD16) and explicit isolate handling — those can be added in a future version.

#### `std.text.unicode.security` — Confusable Detection (UTS #39)

```ori
type RestrictionLevel =
    Ascii              // Only ASCII
  | SingleScript       // One script + Common/Inherited
  | HighlyRestrictive  // Recommended sets (Latin+Han+Hiragana+Katakana, etc.)
  | ModeratelyRestrictive  // Allows more script mixing
  | Unrestricted       // No restriction

type MixedScriptStatus =
    SingleScript(Script)
  | SafeMix([Script])       // Scripts that commonly co-occur (Latin + Common)
  | SuspiciousMix([Script]) // Scripts that shouldn't mix (Latin + Cyrillic)

@skeleton (s: str) -> str
// UTS #39 "skeleton" — normalizes confusable characters to a canonical form.
// skeleton("аpple") == skeleton("apple")  → true (Cyrillic а vs Latin a)
// skeleton("pаypal") == skeleton("paypal")  → true

@is_confusable (a: str, b: str) -> bool
// Returns true if a and b have the same skeleton.

@mixed_script_status (s: str) -> MixedScriptStatus
// Checks whether a string mixes scripts in a suspicious way.
// "Hello"        → SingleScript(Latin)
// "Hello世界"     → SafeMix([Latin, Han])
// "Hеllo"        → SuspiciousMix([Latin, Cyrillic])  // Cyrillic е

@restriction_level (s: str) -> RestrictionLevel
// Returns the most restrictive level the string satisfies.

@check_identifier (s: str) -> Result<void, IdentifierSecurityError>
// Combined check for identifiers: XID validity, restriction level,
// and mixed-script status. Useful for compilers, user registration.
```

**Implementation**: Confusable mappings table (~50KB, from `confusables.txt` in Unicode data). Script extension lookup table (shared with `unicode.props`).

---

### Layer 1: `std.text.width` — Display Width (UAX #11)

The universally missing function. Based on East Asian Width property (UAX #11) with emoji and grapheme cluster handling.

```ori
type EastAsianWidth = Narrow | Wide | FullWidth | HalfWidth | Ambiguous | Neutral

@east_asian_width (c: char) -> EastAsianWidth
// Returns the East Asian Width property of a character.

@char_width (c: char) -> int
// Returns the display width of a single codepoint in terminal columns.
// 0 for control characters and combining marks.
// 1 for narrow characters (most Latin, Cyrillic, Arabic).
// 2 for wide characters (CJK ideographs, some symbols).
// Ambiguous characters treated as narrow (Western terminal default).

@display_width (s: str) -> int
// Returns the total display width of a string in terminal columns.
// Iterates grapheme clusters, computing width per cluster:
//   - ASCII graphemes: sum of char widths (fast path)
//   - CJK graphemes: 2
//   - Emoji graphemes (including ZWJ sequences): 2
//   - Combining mark sequences: width of base character
//   - ANSI escape sequences: 0 (automatically stripped)
//
// display_width("Hello")       → 5
// display_width("こんにちは")   → 10  (5 CJK chars × 2)
// display_width("👨‍👩‍👧‍👦")         → 2   (1 emoji cluster)
// display_width("\x1b[31mHi\x1b[0m") → 2  (ANSI ignored)

@truncate_to_width (s: str, max_width: int, suffix: str = "…") -> str
// Truncates a string to fit within max_width display columns.
// Never breaks a grapheme cluster. Appends suffix if truncated.
// The suffix width is accounted for in max_width.
//
// truncate_to_width("Hello World", 8)         → "Hello W…"
// truncate_to_width("こんにちは世界", 8)         → "こんに…"  (6 + 1 = 7 ≤ 8)
// truncate_to_width("Hello", 10)              → "Hello"    (no truncation)

@pad_to_width (s: str, target_width: int, fill: char = ' ', align: Alignment = Alignment.Left) -> str
// Pads a string to exactly target_width display columns.
// If the string is already wider, returns it unchanged.
//
// pad_to_width("Hi", 10)                                → "Hi        "
// pad_to_width("名前", 10)                                → "名前      "  (4 + 6)
// pad_to_width("Hi", 10, align: Alignment.Right)        → "        Hi"
// pad_to_width("Hi", 10, align: Alignment.Center)       → "    Hi    "

@center_to_width (s: str, target_width: int, fill: char = ' ') -> str
// Shorthand for pad_to_width with Center alignment.
```

**Implementation**: East Asian Width table (~6KB, from `EastAsianWidth.txt`). Width calculation integrates with `unicode.segmentation` for grapheme awareness and with `transform.ansi` for ANSI stripping. Fast path: ASCII-only strings skip grapheme and CJK checks entirely.

---

### Layer 2: `std.text.similarity` — String Matching

Inspired by Elixir (the only language with built-in similarity) and Python's `difflib`. Essential for compiler diagnostics, search, spell-checking, and data deduplication.

```ori
// ── Edit Distance ────────────────────────────────────────────────────

@edit_distance (a: str, b: str) -> int
// Levenshtein distance: minimum insertions, deletions, and substitutions.
// edit_distance("kitten", "sitting")  → 3
// edit_distance("", "abc")            → 3
// edit_distance("abc", "abc")         → 0
// Operates on grapheme clusters, not bytes or codepoints.

@damerau_levenshtein (a: str, b: str) -> int
// Like edit_distance but transpositions count as 1 operation.
// damerau_levenshtein("ab", "ba")  → 1  (swap)
// damerau_levenshtein("ab", "ba")  → 2  (Levenshtein: delete + insert)

// ── Similarity Ratios ────────────────────────────────────────────────

@jaro_winkler (a: str, b: str) -> float
// Jaro-Winkler similarity: 0.0 (completely different) to 1.0 (identical).
// Good for name matching — gives bonus for shared prefix.
// jaro_winkler("Martha", "Marhta")   → ~0.961
// jaro_winkler("DIXON", "DICKSONX")  → ~0.813

@similarity_ratio (a: str, b: str) -> float
// Normalized edit distance: 1.0 - (edit_distance / max(len_a, len_b)).
// similarity_ratio("abc", "abc")  → 1.0
// similarity_ratio("abc", "xyz")  → 0.0
// similarity_ratio("abc", "abd")  → ~0.667

@longest_common_subsequence (a: str, b: str) -> str
// Returns the longest subsequence common to both strings.
// longest_common_subsequence("ABCBDAB", "BDCAB")  → "BCAB"

// ── Fuzzy Search ─────────────────────────────────────────────────────

@closest_match (needle: str, haystack: [str]) -> Option<str>
// Returns the most similar string from haystack, or None if haystack
// is empty or no match exceeds a minimum similarity threshold (0.6).
// Uses Jaro-Winkler internally.
//
// closest_match("prnt", ["print", "panic", "parse"])  → Some("print")
// closest_match("xyz", ["abc", "def"])                → None

@closest_matches (needle: str, haystack: [str], max_results: int = 3) -> [str]
// Returns up to max_results similar strings, sorted by similarity.
// closest_matches("tets", ["test", "text", "best", "rest", "temp"])
//   → ["test", "text", "best"]

// ── Natural Sort ─────────────────────────────────────────────────────

@natural_compare (a: str, b: str) -> Ordering
// Numeric-aware comparison: embedded numbers compared by value.
// natural_compare("file2", "file10")  → Less   (2 < 10)
// natural_compare("v1.9", "v1.10")   → Less   (9 < 10)
// "file2" < "file10" with standard compare → false (lexicographic: '2' > '1')

@natural_sort (items: [str]) -> [str]
// Sorts using natural_compare.
// natural_sort(["v1.10", "v1.2", "v1.1"])  → ["v1.1", "v1.2", "v1.10"]
```

**Implementation**: Pure Ori. Edit distance uses Wagner-Fischer dynamic programming (O(n·m) time, O(min(n,m)) space). Jaro-Winkler is O(n·m) worst case but typically much faster for short strings. Natural sort uses a custom comparator that extracts numeric runs. All similarity functions operate on grapheme clusters via `unicode.segmentation`.

---

### Layer 3: `std.text.case` — Case Folding and Conversion

```ori
// Case locales for locale-sensitive operations.
// Default handles all standard Unicode case mappings.
// Specific locales override for language-specific rules.
type CaseLocale =
    Default        // Standard Unicode case mapping
  | Turkish        // İ↔i, I↔ı (also applies to Azerbaijani)
  | Lithuanian     // Retains dot above on lowercase i with accent
  | Greek          // Removes accent on uppercase (ΕΛΛΗΝΙΚΆ → ΕΛΛΗΝΙΚΑ)

// ── Case Folding (locale-independent) ────────────────────────────────

@case_fold (s: str) -> str
// Unicode case folding for locale-independent comparison.
// "STRASSE" |> case_fold() == "straße" |> case_fold()  → true
// This is NOT the same as to_lowercase — case folding is a one-way
// transformation designed for comparison, not display.

@case_fold_equals (a: str, b: str) -> bool
// Case-insensitive equality using case folding.
// case_fold_equals("Hello", "HELLO")  → true
// case_fold_equals("straße", "STRASSE")  → true

@case_fold_compare (a: str, b: str) -> Ordering
// Case-insensitive ordering using case folding.

// ── Case Conversion (potentially locale-sensitive) ───────────────────

@to_uppercase (s: str, locale: CaseLocale = CaseLocale.Default) -> str
// "hello" → "HELLO"
// "straße" → "STRASSE"
// "istanbul" with Turkish → "İSTANBUL"

@to_lowercase (s: str, locale: CaseLocale = CaseLocale.Default) -> str
// "HELLO" → "hello"
// "ISTANBUL" with Turkish → "ıstanbul"
// "ΟΔΟΣ" → "οδός" (final sigma rule)

@to_titlecase (s: str, locale: CaseLocale = CaseLocale.Default) -> str
// Capitalizes the first letter of each word (using word boundaries).
// "hello world" → "Hello World"
// "the LORD of the RINGS" → "The Lord Of The Rings"
// Note: English-specific rules (lowering articles/prepositions) are NOT
// applied — that requires natural language processing, not Unicode rules.
// Use the output of to_titlecase as a starting point, not a final result.

// ── Case Testing ─────────────────────────────────────────────────────

@is_uppercase_str (s: str) -> bool
// True if all cased characters are uppercase.

@is_lowercase_str (s: str) -> bool
// True if all cased characters are lowercase.
```

**Implementation**: Case folding data from `CaseFolding.txt` (~15KB). Special casing data from `SpecialCasing.txt` (~5KB). Locale-specific rules for Turkish/Lithuanian/Greek are small and hand-coded.

---

### Layer 4: `std.text.analysis` — Text Analysis Pipeline

Ported from Pretext's `analysis.ts`. This is the linguistic intelligence that makes segment-width-summing match real text rendering. **Pure logic, zero platform dependencies.**

```ori
// ── Types ────────────────────────────────────────────────────────────

type WhiteSpaceMode =
    Normal    // CSS white-space: normal (collapse whitespace, break at spaces)
  | PreWrap   // CSS white-space: pre-wrap (preserve spaces/newlines, allow wrap)
  | Pre       // CSS white-space: pre (preserve everything, no wrap)
  | PreLine   // CSS white-space: pre-line (preserve newlines, collapse spaces)

type SegmentKind =
    Text           // Regular text content
  | Space          // Collapsible space (between words)
  | PreservedSpace // Non-collapsible space (in pre-wrap mode)
  | Tab            // Tab character (in pre-wrap mode)
  | Glue           // Non-breaking space (NBSP, NNBSP, WJ, ZWNBSP) — no break
  | ZeroWidthBreak // Zero-width space (U+200B) — break opportunity, no width
  | SoftHyphen     // Soft hyphen (U+00AD) — invisible unless chosen as break
  | HardBreak      // Newline (in pre-wrap/pre-line mode)

type TextSegment = {
    text: str,
    kind: SegmentKind,
    is_word_like: bool,
    byte_start: int,
}

type TextChunk = {
    start_index: int,      // First segment index in this chunk
    end_index: int,        // Past the last visible segment
    consumed_end: int,     // Past the hard-break terminator (if any)
}

type TextAnalysis = {
    normalized: str,       // Whitespace-normalized text
    segments: [TextSegment],
    chunks: [TextChunk],   // Hard-break-delimited chunks
}

type AnalysisOptions = {
    white_space: WhiteSpaceMode = WhiteSpaceMode.Normal,
    locale: Option<str> = None,
}

// ── Pipeline Entry Point ─────────────────────────────────────────────

@analyze (text: str, options: AnalysisOptions = AnalysisOptions {}) -> TextAnalysis
// Runs the full text analysis pipeline:
//
// Phase 1: Whitespace normalization
//   - Normal: collapse [ \t\n\r\f]+ → single space, strip leading/trailing
//   - PreWrap: normalize \r\n → \n, \r → \n, \f → \n
//   - Pre: no normalization
//   - PreLine: normalize line endings, collapse spaces
//
// Phase 2: Word segmentation (UAX #29)
//   - Uses Intl.Segmenter-equivalent word boundary detection
//   - Handles CJK (per-character), Thai, Arabic, etc.
//
// Phase 3: Break-kind classification
//   - Each segment character classified as text/space/tab/glue/etc.
//   - Multi-character segments split at kind boundaries
//
// Phase 4: Linguistic segment merging (12+ passes)
//   1.  Left-sticky punctuation: "better." measured as one unit
//   2.  CJK kinsoku: 。）etc. merge with preceding segment
//   3.  Forward-sticky clusters: （「 etc. merge with following segment
//   4.  Arabic no-space punctuation: :.,،؛ after Arabic merges into word
//   5.  Myanmar medial glue: ၏ as medial connector
//   6.  Escaped quote clusters: \"word\" as one unit
//   7.  Repeated single-char runs: ——— as one unit
//   8.  Glue-connected text: NBSP joins adjacent text (unbreakable)
//   9.  URL-like run merging: https://...?query as two units (path + query)
//   10. Numeric run merging: 7:00-9:00, 1,000.50 as one unit
//   11. ASCII punctuation chain merging: a,b,c as one unit
//   12. Hyphenated numeric splitting: 123-456 splits at hyphens
//   13. Forward-sticky carry across CJK: opening parens attach to next char
//   14. Arabic space+mark splitting: space + combining marks → split
//
// Phase 5: Hard-break chunk compilation
//   - In pre-wrap mode, segments between \n become separate chunks
//
// All passes are linear scans — the full pipeline is O(n) in segment count.
```

**Implementation**: Pure Ori code. The kinsoku tables (line-start-prohibited, line-end-prohibited, left-sticky punctuation) are small `Set<char>` constants. URL detection uses a simple pattern match (scheme + "://", or "www."). No regex dependency.

---

### Layer 5: `std.text.measure` — Pluggable Text Measurement

The critical abstraction that makes the layout engine platform-agnostic.

```ori
// ── The Trait ─────────────────────────────────────────────────────────

pub trait TextMeasure {
    @measure (self, text: str) -> float
    // Returns the width of the given text string in the measurer's units.
    // Units are measurer-defined: terminal columns, pixels, ems, etc.
    // The measurer shall be consistent: same text → same width.
}

// ── Built-in Implementations ─────────────────────────────────────────

type MonospaceMeasure = {
    char_width: float,  // Width per grapheme cluster (default: 1.0)
}

impl MonospaceMeasure: TextMeasure {
    @measure (self, text: str) -> float =
        grapheme_count(text) as float * self.char_width
}
// Use case: code editors, simple wrapping where every character is equal.

type TerminalMeasure = {
    narrow_width: float,  // Width of narrow characters (default: 1.0)
    wide_width: float,    // Width of wide characters (default: 2.0)
}

impl TerminalMeasure: TextMeasure {
    @measure (self, text: str) -> float = {
        let width = 0.0
        for g in graphemes(text) do {
            // Use display_width logic: CJK=2, emoji=2, combining=0, etc.
            let gw = display_width(g)
            width += match gw {
                0 -> 0.0,
                1 -> self.narrow_width,
                _ -> self.wide_width,
            }
        }
        width
    }
}
// Use case: terminal UIs, TUI frameworks, console output.

type CachedMeasure<M: TextMeasure> = {
    inner: M,
    cache: {str: float},
}

impl<M: TextMeasure> CachedMeasure<M>: TextMeasure {
    @measure (self, text: str) -> float =
        match self.cache[text] {
            Some(w) -> w,
            None -> {
                let w = self.inner.measure(text)
                self.cache[text] = w
                w
            },
        }
}
// Use case: wrapping any measurer with a segment→width cache.
// Essential for the layout engine, which measures the same segments
// repeatedly during line breaking.

// ── Constructor Helpers ──────────────────────────────────────────────

@monospace () -> MonospaceMeasure = MonospaceMeasure { char_width: 1.0 }

@terminal () -> TerminalMeasure = TerminalMeasure { narrow_width: 1.0, wide_width: 2.0 }

@cached<M: TextMeasure> (inner: M) -> CachedMeasure<M> =
    CachedMeasure { inner: inner, cache: {} }
```

**User-defined implementations** (not in std — examples for documentation):

```ori
// GPU font atlas — for game UIs, GPU widget toolkits
type FontAtlasMeasure = { atlas: FontAtlas }
impl FontAtlasMeasure: TextMeasure {
    @measure (self, text: str) -> float =
        self.atlas.shape_and_measure(text)
}

// HarfBuzz — for native proportional-font applications
type HarfBuzzMeasure = { font: HbFont }
impl HarfBuzzMeasure: TextMeasure {
    @measure (self, text: str) -> float =
        hb_shape_and_advance(self.font, text)
}

// Browser canvas — for WASM targets
type CanvasMeasure = { ctx: CPtr }
impl CanvasMeasure: TextMeasure {
    @measure (self, text: str) -> float =
        canvas_measure_text(self.ctx, text)
}
```

---

### Layer 6: `std.text.layout` — Line Breaking and Layout Engine

Ported from Pretext's `layout.ts` and `line-break.ts`. A production-quality greedy line-breaking engine that operates on pre-measured parallel arrays.

```ori
// ── Types ────────────────────────────────────────────────────────────

// Opaque handle to pre-analyzed, pre-measured text.
// Width-independent: the same PreparedText can be laid out at any maxWidth.
// Internally stores parallel arrays (SoA) for cache-friendly hot-path traversal:
//   widths[], lineEndFitAdvances[], lineEndPaintAdvances[], kinds[],
//   breakableWidths[][], chunks[], hyphenWidth, tabStopAdvance
type PreparedText

type LayoutCursor = {
    segment_index: int,
    grapheme_index: int,  // Within segment; 0 at segment boundaries
}

type LayoutResult = {
    line_count: int,
    height: float,         // line_count * line_height
}

type LayoutLine = {
    text: str,             // Full text content of this line
    width: float,          // Measured width of this line
    start: LayoutCursor,   // Inclusive start in prepared segments
    end: LayoutCursor,     // Exclusive end in prepared segments
}

type LayoutLineRange = {
    width: float,
    start: LayoutCursor,
    end: LayoutCursor,
}

type PrepareOptions = {
    white_space: WhiteSpaceMode = WhiteSpaceMode.Normal,
    locale: Option<str> = None,
    overflow_wrap: OverflowWrap = OverflowWrap.BreakWord,
}

type OverflowWrap =
    Normal     // Only break at allowed break points
  | BreakWord  // Break within words if no other break point fits
  | Anywhere   // Break between any two grapheme clusters

// ── Core API ─────────────────────────────────────────────────────────

@prepare<M: TextMeasure> (
    text: str,
    measurer: M,
    options: PrepareOptions = PrepareOptions {},
) -> PreparedText
// Expensive, once per text block. Steps:
//   1. Run analysis pipeline (Layer 4) → segments with break kinds
//   2. Measure each segment via measurer → cached widths
//   3. Pre-measure graphemes of long words (for overflow-wrap: break-word)
//   4. Compute line-end fit/paint advances (trailing space handling)
//   5. Compute soft-hyphen width, tab stop advance
//   6. Store everything in parallel arrays (SoA layout)
//
// The result is width-independent — reusable at any maxWidth.
// Call once when text first appears (e.g., when a message is received).

@layout (
    prepared: PreparedText,
    max_width: float,
    line_height: float,
) -> LayoutResult
// Cheap, on every resize. Pure arithmetic on cached widths.
// No measurer calls, no string operations, no allocations.
// ~0.0002ms per text block. Call on every resize.
//
// Returns line count and total height.

@layout_lines (
    prepared: PreparedText,
    max_width: float,
) -> Iterator<LayoutLine>
// Rich path: returns actual line content with text and geometry.
// Mirrors layout()'s break decisions with extra per-line bookkeeping.
// Use for rendering individual lines.

@layout_next_line (
    prepared: PreparedText,
    start: LayoutCursor,
    max_width: float,
) -> Option<LayoutLine>
// Streaming/variable-width path: lays out one line at a time from
// a cursor position. Different max_width per line enables:
//   - Text flowing around floated elements
//   - Column layouts with varying widths
//   - Incremental/streaming layout
// Returns None when all text is consumed.

@walk_line_ranges (
    prepared: PreparedText,
    max_width: float,
) -> Iterator<LayoutLineRange>
// Non-materializing geometry pass: line widths and cursors without
// building line text strings. Use for aggregate layout work:
//   - Shrink-wrap (find widest line)
//   - Hit testing (which line is at y-position?)
//   - Scroll calculations

@natural_width (prepared: PreparedText) -> float
// Intrinsic width: the widest line when container width is infinite.
// Only hard breaks force line breaks; returns the widest forced line.
// Useful for "shrink-wrap" layouts.
```

**Line breaking algorithm** (matching Pretext):

The engine has two specialized walkers:

1. **Simple fast path** — used when text has no hard breaks, tabs, preserved spaces, or soft hyphens (the common case for most text). Pure segment-width accumulation with greedy breaking.

2. **Full path** — handles all segment kinds. Additional logic:
   - **Tab advance**: distance to next 8-space tab stop from current line position
   - **Soft-hyphen handling**: invisible unless chosen as break point, then adds hyphen width to line. `fitSoftHyphenBreak` determines how many graphemes of the next word fit if we add a hyphen.
   - **Trailing whitespace hanging**: spaces at line end contribute zero to "fit width" (CSS behavior) — tracked via separate `lineEndFitAdvances` vs `lineEndPaintAdvances`.
   - **Overflow-wrap**: when a word is wider than max_width, breaks at grapheme boundaries using pre-measured grapheme widths.

#### `std.text.layout.inline_flow` — Mixed Inline Content Layout

For layouts with mixed content — text runs interleaved with atomic boxes (chips, pills, icons, images).

```ori
type InlineFlowItem = {
    text: str,
    font_key: str,                 // Identifier for measurer selection
    break_mode: BreakMode = BreakMode.Normal,
    extra_width: float = 0.0,      // Padding + borders around this item
}

type BreakMode =
    Normal  // Can break within this item
  | Never   // Atomic — never break within (pills, chips, icons)

type InlineFlowFragment = {
    item_index: int,               // Index into original items array
    text: str,                     // Text slice for this fragment
    gap_before: float,             // Collapsed inter-item gap
    occupied_width: float,         // Text width + extra_width
    start: LayoutCursor,
    end: LayoutCursor,
}

type InlineFlowLine = {
    fragments: [InlineFlowFragment],
    width: float,
    end: InlineFlowCursor,
}

type InlineFlowCursor = {
    item_index: int,
    segment_index: int,
    grapheme_index: int,
}

@prepare_inline_flow<M: TextMeasure> (
    items: [InlineFlowItem],
    measurer: M,
) -> PreparedInlineFlow
// Prepares mixed inline content for layout.
// Collapses boundary whitespace between items (CSS normal rules).
// Measures each item's natural width.

@layout_inline_flow_lines (
    prepared: PreparedInlineFlow,
    max_width: float,
) -> Iterator<InlineFlowLine>
// Lays out mixed inline content into lines.
// Atomic items (break_mode: Never) are kept whole.
// Normal items use layoutNextLine internally.

@measure_inline_flow (
    prepared: PreparedInlineFlow,
    max_width: float,
    line_height: float,
) -> LayoutResult
// Returns line count and total height for mixed inline content.
```

---

### Layer 7: `std.text.transform` — Text Transformations

#### `std.text.transform.slug`

```ori
type SlugOptions = {
    separator: str = "-",
    lowercase: bool = true,
    max_length: Option<int> = None,
    transliterate: bool = true,  // Convert diacritics and CJK to ASCII
}

@slugify (s: str, options: SlugOptions = SlugOptions {}) -> str
// Converts arbitrary Unicode text to a URL-safe slug.
// "Héllo Wörld! 你好" → "hello-world-ni-hao"
//
// Steps:
//   1. NFKD normalize (compatibility decomposition)
//   2. Strip combining marks (remove diacritics)
//   3. Transliterate remaining non-ASCII (CJK → pinyin/romaji approximation)
//   4. Lowercase (if enabled)
//   5. Replace non-alphanumeric with separator
//   6. Collapse consecutive separators
//   7. Strip leading/trailing separators
//   8. Truncate to max_length (if specified, on grapheme boundary)
```

#### `std.text.transform.ansi`

```ori
@strip_ansi (s: str) -> str
// Removes all ANSI escape sequences (CSI, OSC, SGR, etc.).
// "\x1b[1;31mError:\x1b[0m file not found" → "Error: file not found"

@ansi_display_width (s: str) -> int
// Display width ignoring ANSI escapes.
// Equivalent to display_width(strip_ansi(s)) but more efficient
// (single pass, no intermediate allocation).

@has_ansi (s: str) -> bool
// Returns true if the string contains any ANSI escape sequences.

type AnsiSegment =
    Text(str)        // Visible text content
  | Escape(str)      // ANSI escape sequence (raw bytes)

@parse_ansi (s: str) -> Iterator<AnsiSegment>
// Splits a string into alternating text and ANSI escape segments.
// Useful for ANSI-aware rendering and wrapping.
```

#### `std.text.transform.encoding`

```ori
type Encoding =
    Utf8 | Utf16Le | Utf16Be | Utf32Le | Utf32Be
  | Ascii | Latin1 | Windows1252
  | ShiftJis | EucJp | Iso2022Jp
  | Gb2312 | Gbk | Gb18030
  | Big5
  | EucKr
  | Iso8859(int)    // ISO 8859 parts 1-16

@decode (bytes: [byte], encoding: Encoding) -> Result<str, EncodingError>
// Decodes bytes in the given encoding to a UTF-8 string.

@encode (s: str, encoding: Encoding) -> Result<[byte], EncodingError>
// Encodes a UTF-8 string to the given encoding.
// Returns error if the string contains characters not representable
// in the target encoding.

@detect_encoding (bytes: [byte]) -> Encoding
// Heuristic encoding detection. Checks:
//   1. BOM (byte order mark) if present
//   2. UTF-8 validity (if valid UTF-8, returns Utf8)
//   3. Statistical analysis for common encodings
// Not infallible — encoding detection is inherently heuristic.

type EncodingError = {
    message: str,
    byte_offset: int,
    encoding: Encoding,
}
```

**Implementation**: Encoding conversion uses lookup tables (~100KB total for all supported encodings). Detection uses a simplified version of the Mozilla charset detection algorithm. FFI to `encoding_rs` (Rust crate, also used by Firefox) is an alternative backend option.

#### `std.text.transform` (root) — General Transforms

```ori
@remove_diacritics (s: str) -> str
// Removes combining marks (diacritics) from text.
// "café résumé" → "cafe resume"
// Implementation: NFKD normalize, then strip all Nonspacing_Mark characters.
// Lossy but essential for search normalization, approximate matching,
// and ASCII approximation. Used internally by slugify.

@to_ascii_approximation (s: str) -> str
// Best-effort transliteration to ASCII.
// "Ünlü" → "Unlu", "北京" → "Beijing"
// Uses remove_diacritics + basic script transliteration tables.
```

#### Case Conversion Utilities

```ori
// Code-style case conversions (not in std.text.case because these are
// programmer conventions, not Unicode operations)

@to_snake_case (s: str) -> str
// "helloWorld" → "hello_world"
// "HTTPServer" → "http_server"
// "XMLParser" → "xml_parser"

@to_camel_case (s: str) -> str
// "hello_world" → "helloWorld"
// "HTTP_SERVER" → "httpServer"

@to_pascal_case (s: str) -> str
// "hello_world" → "HelloWorld"

@to_kebab_case (s: str) -> str
// "helloWorld" → "hello-world"

@to_screaming_snake (s: str) -> str
// "helloWorld" → "HELLO_WORLD"
```

---

### Root Module: `std.text` — High-Level Convenience API

Re-exports the most commonly used functions from submodules for ergonomic access.

```ori
// ── Re-exports from submodules ───────────────────────────────────────
pub use std.text.unicode { is_letter, is_digit, is_whitespace, script }
pub use std.text.unicode.segmentation { graphemes, grapheme_count, words }
pub use std.text.unicode.normalization { normalize, canonical_equals, NormalizationForm }
pub use std.text.width { display_width, char_width, truncate_to_width, pad_to_width }
pub use std.text.similarity { edit_distance, jaro_winkler, closest_match, closest_matches, natural_compare, natural_sort }
pub use std.text.case { case_fold, case_fold_equals, to_uppercase, to_lowercase, to_titlecase }
pub use std.text.transform.ansi { strip_ansi }
pub use std.text.transform { remove_diacritics }
pub use std.text.transform.slug { slugify }
pub use std.text.transform.encoding { decode, encode, detect_encoding, Encoding }

// ── Convenience functions ────────────────────────────────────────────

@wrap (text: str, width: int) -> [str]
// Word-wraps text to the given width using display-width-aware
// terminal measurement. Uses the full analysis pipeline internally.
// Handles CJK, Thai, Arabic, URLs, soft hyphens, emoji.
//
// wrap("The quick brown fox jumps over the lazy dog", 20)
//   → ["The quick brown fox", "jumps over the lazy", "dog"]
//
// wrap("日本語のテスト文章です。", 12)
//   → ["日本語のテス", "ト文章です。"]
//   (kinsoku: 。 never starts a line)
//
// wrap("Hello https://example.com/path?query=1 world", 30)
//   → ["Hello", "https://example.com/path", "?query=1 world"]
//   (URL path and query are separate break units)

@wrap_lines (text: str, width: int) -> str
// Like wrap but returns a single string with newlines inserted.
// wrap_lines(text, 80) is equivalent to wrap(text, 80) |> .join(sep: "\n")

@wrap_measured<M: TextMeasure> (text: str, measurer: M, max_width: float) -> [str]
// Word-wraps text using a custom measurer. For proportional fonts,
// GPU text rendering, or any non-terminal context.

@truncate (text: str, max_graphemes: int, suffix: str = "...") -> str
// Truncates text to max_graphemes grapheme clusters.
// truncate("Hello World", 8)  → "Hello..."

@indent (text: str, prefix: str) -> str
// Indents each line of text with the given prefix.
// indent("a\nb\nc", "  ")  → "  a\n  b\n  c"

@dedent (text: str) -> str
// Removes common leading whitespace from all lines.
// dedent("  a\n  b\n  c")  → "a\nb\nc"
// dedent("    a\n  b\n    c")  → "  a\nb\n  c"

@is_blank (text: str) -> bool
// Returns true if the string is empty or contains only Unicode whitespace.
// is_blank("")        → true
// is_blank("  \t\n")  → true
// is_blank(" hello ") → false
// is_blank("\u{00A0}") → true  (NBSP is whitespace)
```

---

## FFI Backend

### Pure Ori vs. Rust Implementation

Most of `std.text` is pure Ori code — the analysis pipeline, similarity algorithms, case conversion utilities, slug generation, ANSI parsing, and the layout engine. These are algorithmic and benefit from Ori's expression-based style.

The **Unicode data tables** and **low-level algorithms** are implemented in Rust in `ori_rt` for performance:

| Component | Implementation | Rationale |
|-----------|---------------|-----------|
| Character properties | Rust (`ori_rt`) | Lookup tables (~80KB), hot path for all segmentation |
| Grapheme segmentation | Rust (`ori_rt`) | UAX #29 state machine, called per-character |
| Word segmentation | Rust (`ori_rt`) | UAX #29 state machine |
| Line break properties | Rust (`ori_rt`) | UAX #14 state machine |
| East Asian Width | Rust (`ori_rt`) | Lookup table (~6KB) |
| Normalization | Rust (`ori_rt`) | Decomposition/composition tables (~60KB), streaming |
| Case folding | Rust (`ori_rt`) | Case folding table (~15KB) |
| Encoding conversion | Rust (`ori_rt`) or FFI | Large lookup tables, or delegate to `encoding_rs` |
| Confusable mappings | Rust (`ori_rt`) | Table (~50KB) |
| Analysis pipeline | Pure Ori | Linguistic rules, benefits from high-level code |
| Layout engine | Pure Ori | Algorithmic, benefits from expression-based style |
| Similarity | Pure Ori | Standard algorithms (DP, Jaro-Winkler) |
| Slug / ANSI / case | Pure Ori | String manipulation |

Total Unicode data footprint: ~230KB compiled if all features are used. **Aggressive tree-shaking**: each table is a separate symbol in `ori_rt`, and linker dead-code elimination strips unused tables. Examples of actual binary impact by usage:

| Usage | Tables linked | Size |
|-------|--------------|------|
| `display_width` only | EAW + grapheme break | ~21KB |
| `display_width` + `wrap` | EAW + grapheme break + word break | ~30KB |
| + `normalize` | + decomposition/composition | ~90KB |
| + `case_fold` | + case folding | ~105KB |
| + `bidi_levels` | + bidi class | ~115KB |
| + `is_confusable` | + confusable mappings | ~165KB |
| All features | All tables | ~230KB |

### Alternative: FFI to ICU4X

ICU4X (the Unicode Consortium's Rust rewrite of ICU) provides all Unicode algorithms with modular data loading. It could serve as an alternative backend:

**Pros**: Maintained by Unicode Consortium, guaranteed compliance, supports data slicing (include only needed locales).
**Cons**: Additional dependency, larger binary for full coverage, API translation layer.

**Recommendation**: Start with hand-crafted Rust in `ori_rt` (matching the approach of Rust's `unicode-segmentation`, `unicode-normalization`, and `unicode-width` crates). Consider migrating to ICU4X in a future version if maintenance burden of keeping tables updated becomes significant.

---

## Data Table Management

Unicode tables must be regenerated when Unicode version updates. The process:

1. Download Unicode Character Database files from unicode.org
2. Run `scripts/generate-unicode-tables.py` (to be created)
3. This generates Rust source files in `compiler/ori_rt/src/unicode/tables/`
4. Tables are compile-time constants — zero runtime initialization cost

Table formats:
- **Two-level trie** for character properties (General_Category, Script): O(1) lookup, ~80KB
- **Sorted range list** for binary properties (is_cjk, is_emoji): O(log n) lookup, ~6KB each
- **Flat array** for small ranges (East Asian Width for BMP): O(1) lookup

---

## Comparison with Prior Art

| Feature | Ori std.text | Rust (crates) | Swift | Python | Elixir | Go (x/text) |
|---------|-------------|---------------|-------|--------|--------|-------------|
| Grapheme clusters | Yes (std) | unicode-segmentation | Yes (default) | No | Yes (std) | No |
| Display width | Yes (std) | unicode-width | No | No | No | x/text/width |
| Text wrapping | Yes (std, CJK-aware) | textwrap | No | textwrap (broken CJK) | No | No |
| Normalization | Yes (std) | unicode-normalization | Yes (Foundation) | Yes (unicodedata) | Yes (std) | x/text/norm |
| Case folding | Yes (std) | No (crate) | Yes (Foundation) | Yes (casefold) | No | x/text/cases |
| Similarity | Yes (std) | strsim crate | No | difflib | Yes (std) | No |
| Bidi | Yes (std) | unicode-bidi | No | No | No | x/text/bidi |
| Confusables | Yes (std) | No | No | No | No | No |
| Text layout | Yes (std, Pretext-derived) | No | CoreText (Apple only) | No | No | No |
| Slugify | Yes (std) | slug crate | No | No | No | No |
| ANSI handling | Yes (std) | strip-ansi-escapes | No | No | No | No |
| Encoding conversion | Yes (std) | encoding_rs | Yes (Foundation) | Yes (codecs) | No | x/text/encoding |
| Natural sort | Yes (std) | natord crate | No | natsort (3rd party) | No | No |

Ori would be the **first language with all of these in a single standard library module**.

---

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| `display_width` (ASCII) | < 5ns/char | Fast path: byte scan, no grapheme check |
| `display_width` (mixed) | < 50ns/char | Grapheme iteration + EAW lookup |
| `grapheme_count` | < 30ns/char | UAX #29 state machine |
| `normalize` (already NFC) | < 10ns/char | Quick-check fast path, no allocation |
| `normalize` (needs work) | < 100ns/char | Decomposition + composition |
| `edit_distance` | O(n·m) | Wagner-Fischer, O(min(n,m)) space |
| `analyze` (Pretext pipeline) | < 500ns/word | 12+ linear passes |
| `layout` (hot path) | < 200ns/block | Pure arithmetic on cached widths |
| `prepare` (cold path) | < 20µs/paragraph | Analysis + measurement |

---

## Testing Strategy

### Unit Tests (Rust, in `ori_rt`)

- Unicode property lookups against Unicode Character Database reference
- Grapheme segmentation against UAX #29 test suite (`GraphemeBreakTest.txt`)
- Word segmentation against UAX #29 test suite (`WordBreakTest.txt`)
- Normalization against UAX #15 test suite (`NormalizationTest.txt`)
- East Asian Width against UAX #11 data
- Line break against UAX #14 test suite (`LineBreakTest.txt`)

### Spec Tests (Ori, in `tests/spec/text/`)

```
tests/spec/text/
├── unicode/
│   ├── graphemes/       Grapheme cluster edge cases
│   ├── normalization/   NFC/NFD equivalence tests
│   ├── bidi/            Bidirectional text tests
│   └── security/        Confusable detection tests
├── width/
│   ├── display_width/   CJK, emoji, combining marks, ANSI
│   ├── truncate/        Grapheme-safe truncation
│   └── pad/             Width-aware padding
├── similarity/
│   ├── edit_distance/   Levenshtein edge cases
│   ├── jaro_winkler/    Name matching tests
│   └── fuzzy/           closest_match tests
├── analysis/
│   ├── whitespace/      Normalization modes
│   ├── merging/         Each merge pass individually
│   └── pipeline/        Full pipeline integration
├── layout/
│   ├── line_break/      Line breaking correctness
│   ├── soft_hyphen/     Soft hyphen handling
│   ├── overflow_wrap/   Word breaking
│   ├── variable_width/  layout_next_line tests
│   └── inline_flow/     Mixed inline content
├── case/
│   ├── fold/            Case folding
│   ├── turkish/         Turkish İ/ı locale
│   └── titlecase/       Title case rules
├── transform/
│   ├── slug/            Slugification
│   ├── ansi/            ANSI stripping and parsing
│   └── encoding/        Encoding conversion
└── integration/
    ├── wrap/            End-to-end wrapping (all scripts)
    ├── cjk/             CJK-specific integration
    ├── arabic/          Arabic/RTL integration
    └── emoji/           Emoji in all contexts
```

### Corpus Tests (from Pretext)

Pretext's accuracy corpus includes validated texts in English, Japanese, Chinese, Thai, Khmer, Myanmar, Arabic, and mixed app text. These shall be adapted to Ori spec tests to verify that the ported analysis pipeline produces identical segment boundaries.

---

## Phasing

### Phase 1: Unicode Foundation + Display Width + Convenience
**Estimated scope**: ~3,000 LOC Rust (tables + algorithms), ~1,500 LOC Ori (API + tests)

- `std.text.unicode` — character properties
- `std.text.unicode.segmentation` — graphemes, words, sentences
- `std.text.width` — display_width, char_width, truncate_to_width, pad_to_width
- `std.text.transform.ansi` — strip_ansi, ansi_display_width
- `std.text` root — wrap (basic, using display_width), truncate, indent, dedent, is_blank
- Spec tests for all of the above

**Enables**: Every console app, TUI, CLI tool. This alone makes Ori's text handling better than any language except Swift/Elixir.

### Phase 2: Normalization + Case + Similarity
**Estimated scope**: ~1,500 LOC Rust (tables), ~2,000 LOC Ori (algorithms + tests)

- `std.text.unicode.normalization` — normalize, is_normalized, canonical_equals
- `std.text.case` — case_fold, case_fold_equals, to_uppercase/lowercase/titlecase
- `std.text.similarity` — edit_distance, jaro_winkler, closest_match, natural_sort
- `std.text.transform.slug` — slugify
- `std.text.transform` case conversion — to_snake_case, to_camel_case, etc.
- Spec tests for all of the above

**Enables**: Compiler diagnostics ("did you mean?"), search, data processing, URL generation.

### Phase 3: Analysis Pipeline + Layout Engine
**Estimated scope**: ~3,000 LOC Ori (pipeline + engine), ~2,000 LOC Ori (tests)

- `std.text.analysis` — full Pretext analysis pipeline (12+ merge passes)
- `std.text.measure` — TextMeasure trait, MonospaceMeasure, TerminalMeasure, CachedMeasure
- `std.text.layout` — prepare, layout, layout_lines, layout_next_line, natural_width
- Upgrade `std.text.wrap` to use full pipeline (replacing basic Phase 1 implementation)
- Corpus tests from Pretext

**Enables**: Production-quality text layout for TUIs, editors, GPU widgets, and eventually browser engines.

### Phase 4: Advanced
**Estimated scope**: ~1,000 LOC Rust (tables), ~2,000 LOC Ori (algorithms + tests)

- `std.text.unicode.bidi` — full bidirectional algorithm
- `std.text.unicode.security` — confusable detection
- `std.text.layout.inline_flow` — mixed inline content layout
- `std.text.transform.encoding` — legacy encoding conversion
- `std.text.unicode.segmentation` — UAX #14 line break (full, beyond the simplified version in Phase 1)

**Enables**: RTL text rendering, security checks, legacy system interop, rich inline layout.

---

## Open Questions

1. **Should `str`'s `Eq` implementation use canonical equivalence?** This proposal recommends keeping byte equality for `==` and providing `canonical_equals` separately. Swift's approach (canonical equivalence by default) is more correct but has performance implications for hash maps and pattern matching. Community feedback should inform this decision.

2. **Should `display_width` treat Ambiguous-width characters as 1 or 2?** UAX #11 says "ambiguous" — the width depends on context (1 in Western terminals, 2 in East Asian terminals). This proposal defaults to 1 (Western), matching `unicode-width` and `string-width`. A `display_width_east_asian(s)` variant could default to 2.

3. **Should Thai/Lao word segmentation use a dictionary?** UAX #29 rules alone do not segment Thai words correctly (Thai has no spaces). A dictionary-based approach (like ICU's) would require shipping ~500KB of Thai dictionary data. This proposal defers dictionary segmentation to a future version and relies on UAX #29's rule-based segmentation (which at least doesn't break within grapheme clusters).

4. **Should the layout engine support Knuth-Plass optimal line breaking?** Pretext uses greedy line breaking (O(n), CSS behavior). Knuth-Plass (O(n²)) produces better-looking justified text. This proposal starts with greedy and considers optimal as a future addition.

5. **Should `std.text.regex` be merged into this proposal or remain separate?** Regex is currently a separate draft proposal. It has different FFI requirements (PCRE2 or RE2 backend). This proposal treats regex as a sibling module (`std.text.regex`) but does not specify its API.

6. **How should Unicode version updates be managed?** Unicode releases annually. Table regeneration should be automated and tracked. A `std.text.unicode.version() -> str` function could expose the Unicode version.

---

## Rejected Alternatives

### 1. Separate Crates (Rust Model)

Splitting into `std.text.unicode`, `std.text.width`, `std.text.wrap` as independent packages would cause:
- Duplicated Unicode tables (each package embeds its own)
- Version coordination problems (width calculation depends on segmentation)
- Import ergonomics degradation (3+ imports for basic wrapping)

The stdlib philosophy proposal explicitly supports cohesive packages. `std.text` is one package with multiple submodules.

### 2. ICU4X as Direct Dependency

Using ICU4X for all Unicode algorithms would provide Unicode Consortium-maintained correctness but:
- Adds a large external dependency to every Ori binary
- ICU4X's API surface is Rust-specific and would need significant wrapping
- The subset we need (segmentation, width, normalization) is well-served by standalone tables

ICU4X remains an option for future migration if table maintenance becomes burdensome.

### 3. Lazy/Dynamic Unicode Data Loading

Loading Unicode tables from external files at runtime would reduce binary size but:
- Adds a file system dependency (capability needed for pure text operations)
- Runtime initialization cost on first use
- Deployment complexity (must ship data files)
- Compile-time tables are fast and simple

### 4. Grapheme Clusters as Default String Unit

Making `str.len()` return grapheme count (Swift model) would be more correct but:
- Changes existing behavior (breaking change)
- Hides O(n) cost behind O(1) syntax
- Most string algorithms (contains, split, trim) work on byte patterns regardless

Ori's current design — byte-len O(1), explicit `grapheme_count()` — is the right tradeoff.

---

## Versioning

Per the [stdlib philosophy proposal](../approved/stdlib-philosophy-proposal.md), `std.text` follows semver independent of the compiler.

| Phase | Version | Contents |
|-------|---------|----------|
| Phase 1 | `std.text 0.1.0` | Unicode foundation, display width, basic wrap/truncate |
| Phase 2 | `std.text 0.2.0` | Normalization, case, similarity, slug, transforms |
| Phase 3 | `std.text 0.3.0` | Analysis pipeline, layout engine, TextMeasure |
| Phase 4 | `std.text 0.4.0` | Bidi, security, inline flow, encoding conversion |
| Stable  | `std.text 1.0.0` | After community feedback on 0.x API surface |

Breaking changes (if any) are permitted during the 0.x series. The 1.0.0 release signals API stability commitment.

---

## Future Work

The following are explicitly deferred to separate proposals:

- **`std.text.table`** — Column-aligned terminal table formatting (borders, alignment, merge cells, CSV input). High demand (Go's `tablewriter`, Python's `tabulate`, Rust's `tabled`). Deserves its own proposal due to distinct requirements.
- **`std.i18n`** — Locale-aware collation (UTS #10), number/date/currency formatting, plural rules, message formatting. Requires CLDR data (~500KB+). Separate package per stdlib philosophy.
- **Knuth-Plass optimal line breaking** — The layout engine ships with greedy (O(n), CSS behavior). Knuth-Plass (O(n²)) produces better justified text. Could be added as an option to `PrepareOptions` in a future `std.text` version.
- **Thai/Lao dictionary segmentation** — UAX #29 rules alone do not segment Thai words correctly. A dictionary-based approach requires ~500KB of dictionary data. Deferred until demand is assessed.
- **`std.text.diff`** — Structured text diffing (Myers diff, patience diff, semantic diff). Related to similarity but focused on producing edit scripts and patches.

---

## References

- [Unicode Standard, Version 16.0](https://www.unicode.org/versions/Unicode16.0.0/)
- [UAX #9: Unicode Bidirectional Algorithm](https://www.unicode.org/reports/tr9/)
- [UAX #11: East Asian Width](https://www.unicode.org/reports/tr11/)
- [UAX #14: Unicode Line Breaking Algorithm](https://www.unicode.org/reports/tr14/)
- [UAX #15: Unicode Normalization Forms](https://www.unicode.org/reports/tr15/)
- [UAX #29: Unicode Text Segmentation](https://www.unicode.org/reports/tr29/)
- [UTS #10: Unicode Collation Algorithm](https://www.unicode.org/reports/tr10/)
- [UTS #39: Unicode Security Mechanisms](https://www.unicode.org/reports/tr39/)
- [Pretext](https://github.com/chenglou/pretext) — Cheng Lou's text measurement and layout library
- [ICU4X](https://github.com/unicode-org/icu4x) — Unicode Consortium's Rust Unicode library
- [Swift String Manifesto](https://github.com/apple/swift/blob/main/docs/StringManifesto.md)
- [Stop Ascribing Meaning to Code Points](https://manishearth.github.io/blog/2017/01/14/stop-ascribing-meaning-to-unicode-code-points/) — Manish Goregaokar
- [Text layout is a loose hierarchy of segmentation](https://raphlinus.github.io/text/2020/10/26/text-layout.html) — Raph Levien
- [Rust `unicode-segmentation`](https://crates.io/crates/unicode-segmentation)
- [Rust `unicode-width`](https://crates.io/crates/unicode-width)
- [Rust `textwrap`](https://crates.io/crates/textwrap)
- [npm `string-width`](https://www.npmjs.com/package/string-width) (75M weekly downloads)
- [Ori stdlib philosophy proposal](../approved/stdlib-philosophy-proposal.md)
