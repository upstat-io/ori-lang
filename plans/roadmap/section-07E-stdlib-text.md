---
section: 7E
title: "std.text — Comprehensive Text Processing"
status: not-started
reviewed: true
tier: 2
goal: "Unicode foundation, display width, text wrapping, normalization, similarity, text analysis pipeline, layout engine, bidi, security, encoding"
depends_on: ["7A"]
spec: []
inspired_by:
  - "Pretext (github.com/chenglou/pretext) — text analysis + layout engine"
  - "ICU4X — modular Unicode algorithms"
  - "Rust unicode-segmentation, unicode-width, textwrap crates"
  - "Swift String — grapheme cluster correctness"
  - "Elixir String — built-in similarity functions"
third_party_review:
  status: none
  updated: null
sections:
  - id: "7E.1"
    title: "Phase 1: Unicode Foundation + Display Width + Convenience"
    status: not-started
  - id: "7E.2"
    title: "Phase 2: Normalization + Case + Similarity + Transforms"
    status: not-started
  - id: "7E.3"
    title: "Phase 3: Analysis Pipeline + Layout Engine"
    status: not-started
  - id: "7E.4"
    title: "Phase 4: Bidi + Security + Inline Flow + Encoding"
    status: not-started
  - id: "7E.5"
    title: "Section Completion Checklist"
    status: not-started
---

# Section 7E: std.text — Comprehensive Text Processing

**Goal**: Implement `std.text` per the approved [stdlib-text-api-proposal](../../docs/ori_lang/proposals/approved/stdlib-text-api-proposal.md) — a 7-layer text library covering Unicode properties, grapheme segmentation, display width, normalization, case folding, string similarity, text analysis (from Pretext), pluggable measurement, production-quality line breaking, bidi, confusable detection, encoding conversion, and high-level convenience functions.

> **PROPOSAL**: `proposals/approved/stdlib-text-api-proposal.md` — Full API design
> **REFERENCE**: `~/projects/reference_repos/pretext/` — Pretext source (analysis pipeline, layout engine)
> **REFERENCE**: Rust crates `unicode-segmentation`, `unicode-width`, `unicode-normalization` for table generation patterns

---

## Architecture

```
  ori_rt (Rust)                     library/std/text/ (Ori)
  ┌─────────────────────┐          ┌──────────────────────────────────┐
  │ Unicode tables       │          │ std.text (root)                  │
  │ ├── grapheme_break   │◄────────│ ├── unicode/                     │
  │ ├── word_break       │  FFI    │ │   ├── segmentation             │
  │ ├── east_asian_width │  calls  │ │   ├── normalization            │
  │ ├── normalization    │          │ │   ├── bidi                     │
  │ ├── case_fold        │          │ │   └── security                 │
  │ ├── bidi_class       │          │ ├── width/                       │
  │ ├── confusables      │          │ ├── similarity/                  │
  │ └── line_break       │          │ ├── case/                        │
  └─────────────────────┘          │ ├── analysis/    ← from Pretext  │
                                    │ ├── measure/     ← TextMeasure   │
                                    │ ├── layout/      ← from Pretext  │
                                    │ └── transform/                   │
                                    └──────────────────────────────────┘
```

**Key design**: Unicode data tables live in `ori_rt` as Rust compile-time constants (~230KB total, tree-shaken per function). All algorithms above the table layer are pure Ori.

**Binary size tree-shaking** (linker DCE per table symbol):

| Usage | Tables linked | Approx size |
|-------|--------------|-------------|
| `display_width` only | EAW + grapheme break | ~21KB |
| + `wrap` | + word break | ~30KB |
| + `normalize` | + decomposition/composition | ~90KB |
| + `case_fold` | + case folding | ~105KB |
| All features | All tables | ~230KB |

---

## 7E.1 Phase 1: Unicode Foundation + Display Width + Convenience

**Goal**: Ship the Unicode character properties, grapheme/word segmentation, East Asian Width, display width calculation, ANSI escape handling, and high-level convenience functions (wrap, truncate, pad, indent, dedent). This phase alone makes Ori's text handling better than any language except Swift/Elixir.

**Estimated scope**: ~3,000 LOC Rust (tables + state machines in `ori_rt`), ~1,500 LOC Ori (API + tests)

### 7E.1.1 Unicode Data Table Generation

- [ ] Create `scripts/generate-unicode-tables.py` — downloads Unicode Character Database files from unicode.org, generates Rust source in `compiler/ori_rt/src/unicode/tables/`
  - [ ] Two-level trie generator for `GeneralCategory`, `Script` (~80KB)
  - [ ] Sorted range list generator for `East_Asian_Width` (~6KB)
  - [ ] Grapheme break property table generator (`GraphemeBreakProperty.txt` → ~15KB)
  - [ ] Word break property table generator (`WordBreakProperty.txt` → ~10KB)
  - [ ] Generate `mod.rs` re-exporting all tables
- [ ] Verify all tables match Unicode 16.0 test data

### 7E.1.2 Character Property Functions in `ori_rt`

- [ ] Implement in `compiler/ori_rt/src/unicode/props.rs`:
  - [ ] `ori_char_general_category(c: u32) -> u8` — lookup in two-level trie
  - [ ] `ori_char_script(c: u32) -> u8` — lookup in two-level trie
  - [ ] `ori_char_is_cjk(c: u32) -> bool` — range check (CJK Unified + extensions)
  - [ ] `ori_char_east_asian_width(c: u32) -> u8` — lookup in range list
  - [ ] `ori_char_width(c: u32) -> i8` — 0/1/2 terminal column width
  - [ ] Derived: `is_letter`, `is_digit`, `is_whitespace`, `is_alphabetic`, etc.
- [ ] Register all functions as LLVM extern declarations in `ori_llvm`
- [ ] Register all functions in evaluator built-in dispatch
- [ ] **Tests**: Rust unit tests against Unicode test data files (100+ property checks)

### 7E.1.3 Grapheme Cluster Segmentation (UAX #29)

- [ ] Implement UAX #29 grapheme break state machine in `compiler/ori_rt/src/unicode/grapheme.rs`
  - [ ] `ori_grapheme_next(s: *const u8, len: usize, offset: usize) -> usize` — returns byte offset of next grapheme boundary
  - [ ] `ori_grapheme_count(s: *const u8, len: usize) -> usize` — count grapheme clusters
  - [ ] `ori_is_grapheme_boundary(s: *const u8, len: usize, offset: usize) -> bool`
- [ ] Expose as Ori functions in `library/std/text/unicode/segmentation.ori`:
  - [ ] `@graphemes (s: str) -> Iterator<str>` — yields grapheme cluster slices
  - [ ] `@grapheme_count (s: str) -> int`
  - [ ] `@grapheme_indices (s: str) -> Iterator<(int, str)>`
  - [ ] `@is_grapheme_boundary (s: str, byte_offset: int) -> bool`
- [ ] **Tests**: Run against `GraphemeBreakTest.txt` (700+ test cases from Unicode)
  - Matrix: ASCII, Latin + combining marks, CJK, Hangul jamo, emoji (ZWJ, flags, skin tone), Thai, Devanagari, mixed script

### 7E.1.4 Word Segmentation (UAX #29)

- [ ] Implement UAX #29 word break state machine in `compiler/ori_rt/src/unicode/word.rs`
  - [ ] `ori_word_next(s: *const u8, len: usize, offset: usize) -> (usize, bool)` — (boundary, is_word_like)
- [ ] Expose as `@words (s: str) -> Iterator<WordSegment>` in `library/std/text/unicode/segmentation.ori`
- [ ] **Tests**: Run against `WordBreakTest.txt` from Unicode
  - Matrix: English, CJK (per-char), contractions ("don't"), hyphenated words, numeric, mixed

### 7E.1.5 Display Width (`std.text.width`)

- [ ] Implement `library/std/text/width/mod.ori`:
  - [ ] `@east_asian_width (c: char) -> EastAsianWidth` — calls `ori_char_east_asian_width`
  - [ ] `@char_width (c: char) -> int` — calls `ori_char_width`
  - [ ] `@display_width (s: str) -> int` — iterates grapheme clusters, sums widths
    - Fast path: ASCII-only → byte length
    - CJK grapheme → 2
    - Emoji grapheme (ZWJ, flag) → 2
    - Combining mark → 0
    - ANSI escape → 0 (auto-strip)
  - [ ] `@truncate_to_width (s: str, max_width: int, suffix: str = "…") -> str`
  - [ ] `@pad_to_width (s: str, target_width: int, fill: char = ' ', align: Alignment = Alignment.Left) -> str`
  - [ ] `@center_to_width (s: str, target_width: int, fill: char = ' ') -> str`
- [ ] **Tests**:
  - Semantic pin: `display_width("こんにちは") == 10` (CJK = 2 each)
  - Semantic pin: `display_width("👨‍👩‍👧‍👦") == 2` (ZWJ emoji = 2)
  - Semantic pin: `display_width("\x1b[31mHi\x1b[0m") == 2` (ANSI = 0)
  - Matrix: ASCII, CJK, emoji, combining marks, control chars, ANSI escapes, mixed
  - Truncation: grapheme-safe (never splits cluster), CJK at boundary, suffix accounting
  - Padding: left/right/center, CJK content, emoji content

### 7E.1.6 ANSI Escape Handling (`std.text.transform.ansi`)

- [ ] Implement `library/std/text/transform/ansi.ori`:
  - [ ] `@strip_ansi (s: str) -> str`
  - [ ] `@ansi_display_width (s: str) -> int` — single pass, no intermediate allocation
  - [ ] `@has_ansi (s: str) -> bool`
  - [ ] `@parse_ansi (s: str) -> Iterator<AnsiSegment>` (Text | Escape variants)
- [ ] **Tests**: CSI sequences, OSC sequences, SGR reset, nested colors, empty strings

### 7E.1.7 High-Level Convenience Functions (`std.text` root)

- [ ] Implement `library/std/text/mod.ori` (replacing current stub):
  - [ ] `@wrap (text: str, width: int) -> [str]` — uses display_width + word segmentation
  - [ ] `@wrap_lines (text: str, width: int) -> str` — wrap + join with "\n"
  - [ ] `@truncate (text: str, max_graphemes: int, suffix: str = "...") -> str`
  - [ ] `@indent (text: str, prefix: str) -> str`
  - [ ] `@dedent (text: str) -> str`
  - [ ] `@is_blank (text: str) -> bool` — empty or all Unicode whitespace
  - [ ] Re-exports from submodules (display_width, graphemes, etc.)
- [ ] **Tests**:
  - `wrap`: English, CJK (no spaces), mixed CJK+Latin, long words, empty strings
  - Semantic pin: `wrap("日本語テスト", 6) == ["日本語", "テスト"]` (CJK width-aware)
  - `truncate`: grapheme-safe, suffix accounting
  - `indent`/`dedent`: multiline, mixed indentation, empty lines

### 7E.1.R Third-Party Review Findings

- None.

### 7E.1 Completion Checklist

- [ ] All functions implemented in `library/std/text/` and `compiler/ori_rt/src/unicode/`
- [ ] All Unicode test data suites pass (GraphemeBreakTest, WordBreakTest)
- [ ] Spec tests in `tests/spec/text/` cover all public APIs
- [ ] Evaluator and LLVM backends produce identical results for all tests
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks
- [ ] `./test-all.sh` passes
- [ ] `/tpr-review`
- [ ] `/impl-hygiene-review`

- [ ] **Subsection close-out (7E.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7E.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7E.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7E.2 Phase 2: Normalization + Case + Similarity + Transforms

**Goal**: Ship Unicode normalization (NFC/NFD/NFKC/NFKD), case folding, locale-aware case conversion, string similarity functions (edit distance, Jaro-Winkler, closest match), natural sort, slugification, and case style conversion. Enables compiler diagnostics ("did you mean?"), search, data processing, URL generation.

**Estimated scope**: ~1,500 LOC Rust (tables), ~2,000 LOC Ori (algorithms + tests)

**Depends on**: 7E.1 (grapheme segmentation used by similarity functions)

### 7E.2.1 Normalization Tables in `ori_rt`

- [ ] Generate normalization data tables via `scripts/generate-unicode-tables.py`:
  - [ ] Canonical decomposition mapping (~30KB)
  - [ ] Canonical composition mapping (~20KB)
  - [ ] Compatibility decomposition mapping (~10KB)
  - [ ] NFC/NFD Quick_Check properties (~5KB)
  - [ ] Canonical_Combining_Class table (~5KB)
- [ ] Implement in `compiler/ori_rt/src/unicode/normalize.rs`:
  - [ ] `ori_normalize_nfc(s: *const u8, len: usize, out: *mut u8, out_len: *mut usize)`
  - [ ] `ori_is_normalized_nfc(s: *const u8, len: usize) -> bool` — quick check fast path
  - [ ] NFD, NFKC, NFKD variants
- [ ] **Tests**: Run against `NormalizationTest.txt` from Unicode (18,000+ test cases)

### 7E.2.2 Normalization Ori API (`std.text.unicode.normalization`)

- [ ] Implement `library/std/text/unicode/normalization.ori`:
  - [ ] `@normalize (s: str, form: NormalizationForm = NormalizationForm.NFC) -> str`
  - [ ] `@is_normalized (s: str, form: NormalizationForm = NormalizationForm.NFC) -> bool`
  - [ ] `@canonical_equals (a: str, b: str) -> bool`
  - [ ] `@compatibility_equals (a: str, b: str) -> bool`
- [ ] **Tests**:
  - Semantic pin: `canonical_equals("café", "cafe\u{0301}") == true`
  - Semantic pin: `is_normalized("Hello", NFC) == true` (ASCII fast path, no allocation)
  - Matrix: ASCII (fast path), precomposed, decomposed, compatibility, Hangul, mixed

### 7E.2.3 Case Folding Tables in `ori_rt`

- [ ] Generate case folding table from `CaseFolding.txt` (~15KB)
- [ ] Generate special casing data from `SpecialCasing.txt` (~5KB)
- [ ] Implement `ori_case_fold(s, len, out, out_len)` in `ori_rt`

### 7E.2.4 Case Operations (`std.text.case`)

- [ ] Implement `library/std/text/case/mod.ori`:
  - [ ] `@case_fold (s: str) -> str`
  - [ ] `@case_fold_equals (a: str, b: str) -> bool`
  - [ ] `@case_fold_compare (a: str, b: str) -> Ordering`
  - [ ] `@to_uppercase (s: str, locale: CaseLocale = CaseLocale.Default) -> str`
  - [ ] `@to_lowercase (s: str, locale: CaseLocale = CaseLocale.Default) -> str`
  - [ ] `@to_titlecase (s: str, locale: CaseLocale = CaseLocale.Default) -> str`
- [ ] **Tests**:
  - Semantic pin: `case_fold_equals("straße", "STRASSE") == true`
  - Matrix: ASCII, Latin extended, Turkish İ/ı, German ß→SS, Greek final σ→ς, Lithuanian

### 7E.2.5 String Similarity (`std.text.similarity`)

- [ ] Implement `library/std/text/similarity/mod.ori` (pure Ori):
  - [ ] `@edit_distance (a: str, b: str) -> int` — Wagner-Fischer O(n·m), O(min(n,m)) space
  - [ ] `@damerau_levenshtein (a: str, b: str) -> int` — with transposition
  - [ ] `@jaro_winkler (a: str, b: str) -> float` — 0.0 to 1.0
  - [ ] `@similarity_ratio (a: str, b: str) -> float` — normalized edit distance
  - [ ] `@longest_common_subsequence (a: str, b: str) -> str`
  - [ ] `@closest_match (needle: str, haystack: [str]) -> Option<str>` — Jaro-Winkler, threshold 0.6
  - [ ] `@closest_matches (needle: str, haystack: [str], max_results: int = 3) -> [str]`
  - [ ] `@natural_compare (a: str, b: str) -> Ordering`
  - [ ] `@natural_sort (items: [str]) -> [str]`
- [ ] All similarity functions operate on grapheme clusters
- [ ] **Tests**:
  - Semantic pin: `edit_distance("kitten", "sitting") == 3`
  - Semantic pin: `closest_match("prnt", ["print", "panic", "parse"]) == Some("print")`
  - Matrix: empty strings, identical, completely different, Unicode (CJK, emoji), single char, long strings

### 7E.2.6 Text Transforms (`std.text.transform`)

- [ ] Implement `library/std/text/transform/slug.ori`:
  - [ ] `@slugify (s: str, options: SlugOptions = SlugOptions {}) -> str`
- [ ] Implement `library/std/text/transform/mod.ori`:
  - [ ] `@remove_diacritics (s: str) -> str` — NFKD + strip combining marks
  - [ ] `@to_ascii_approximation (s: str) -> str` — diacritics + basic transliteration
  - [ ] `@to_snake_case`, `@to_camel_case`, `@to_pascal_case`, `@to_kebab_case`, `@to_screaming_snake`
- [ ] **Tests**: slugify with diacritics, CJK, spaces, special chars; case conversion with camelCase, PascalCase, snake_case, mixed

### 7E.2.R Third-Party Review Findings

- None.

### 7E.2 Completion Checklist

- [ ] All normalization test suite entries pass (NormalizationTest.txt)
- [ ] All public APIs have spec tests
- [ ] Evaluator and LLVM produce identical results
- [ ] `ORI_CHECK_LEAKS=1` zero leaks
- [ ] `./test-all.sh` passes
- [ ] `/tpr-review`
- [ ] `/impl-hygiene-review`

- [ ] **Subsection close-out (7E.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7E.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7E.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7E.3 Phase 3: Analysis Pipeline + Layout Engine

**Goal**: Port Pretext's text analysis pipeline (12+ linguistic merging passes) and greedy line-breaking engine to Ori. Ship the `TextMeasure` trait with built-in `MonospaceMeasure`, `TerminalMeasure`, and `CachedMeasure`. Upgrade `wrap()` to use the full pipeline. This enables production-quality text layout for TUIs, editors, GPU widgets, and browser engines.

**Estimated scope**: ~3,000 LOC Ori (pipeline + engine), ~2,000 LOC Ori (tests)

**Depends on**: 7E.1 (segmentation, display width), 7E.2 (normalization for whitespace handling)

**Reference**: `~/projects/reference_repos/pretext/src/` — `analysis.ts`, `measurement.ts`, `layout.ts`, `line-break.ts`

### 7E.3.1 TextMeasure Trait and Built-in Implementations

- [ ] Implement `library/std/text/measure/mod.ori`:
  - [ ] `pub trait TextMeasure { @measure (self, text: str) -> float }`
  - [ ] `type MonospaceMeasure = { char_width: float }` + impl
  - [ ] `type TerminalMeasure = { narrow_width: float, wide_width: float }` + impl
  - [ ] `type CachedMeasure<M: TextMeasure> = { inner: M, cache: {str: float} }` + impl
  - [ ] Constructor helpers: `@monospace()`, `@terminal()`, `@cached<M>(inner: M)`
- [ ] **Tests**: measure ASCII, CJK, emoji, empty string with each measurer

### 7E.3.2 Kinsoku Tables and Segment Classification

- [ ] Implement `library/std/text/analysis/kinsoku.ori`:
  - [ ] `let $kinsoku_start: Set<char>` — CJK line-start-prohibited characters (26 chars)
  - [ ] `let $kinsoku_end: Set<char>` — CJK line-end-prohibited characters (18 chars)
  - [ ] `let $left_sticky_punctuation: Set<char>` — left-sticky punctuation (~30 chars)
  - [ ] `let $arabic_no_space_trailing: Set<char>` — Arabic trailing punctuation
  - [ ] `let $myanmar_medial_glue: Set<char>` — Myanmar medial connectors
  - [ ] Classification functions: `is_left_sticky_segment`, `is_forward_sticky_segment`, etc.
- [ ] Port from: `pretext/src/analysis.ts` lines 129-207 (kinsoku tables and classification)

### 7E.3.3 Text Analysis Pipeline

- [ ] Implement `library/std/text/analysis/mod.ori`:
  - [ ] Whitespace normalization (Normal, PreWrap, Pre, PreLine modes)
  - [ ] Segment-by-break-kind splitting
  - [ ] **12+ merging passes** (each a linear scan):
    1. Left-sticky punctuation merge
    2. CJK kinsoku merge (line-start/end prohibited)
    3. Forward-sticky cluster carry
    4. Arabic no-space punctuation merge
    5. Myanmar medial glue merge
    6. Escaped quote cluster merge
    7. Repeated single-char run merge
    8. Glue-connected text run merge (NBSP)
    9. URL-like run merge
    10. URL query run merge
    11. Numeric run merge
    12. ASCII punctuation chain merge
    13. Hyphenated numeric split
    14. Forward-sticky carry across CJK
    15. Arabic space+mark split
  - [ ] Hard-break chunk compilation
  - [ ] `@analyze (text: str, options: AnalysisOptions) -> TextAnalysis`
- [ ] Port from: `pretext/src/analysis.ts` (full file, ~1020 lines)
- [ ] **Tests**:
  - Per-pass unit tests (each merging pass tested independently)
  - Integration: English, CJK, Arabic, Thai, Myanmar, URLs, numeric, emoji, mixed
  - Semantic pin: `analyze("better.").segments.len() == 1` (punctuation merged)
  - Semantic pin: URL segments grouped correctly
  - Corpus tests adapted from Pretext's accuracy suite

### 7E.3.4 Line Breaking Engine

- [ ] Implement `library/std/text/layout/line_break.ori`:
  - [ ] `PreparedText` type with SoA internal arrays (widths, lineEndFitAdvances, lineEndPaintAdvances, kinds, breakableWidths, chunks)
  - [ ] `@prepare<M: TextMeasure> (text, measurer, options) -> PreparedText` — analysis + measurement
  - [ ] Simple fast path walker (no tabs, soft hyphens, preserved spaces)
  - [ ] Full path walker (tabs, soft hyphens, preserved spaces, hard breaks)
  - [ ] Dual fit/paint width tracking (trailing whitespace hanging)
  - [ ] Overflow-wrap grapheme-level breaking
  - [ ] Soft-hyphen fitting (`fitSoftHyphenBreak`)
  - [ ] Tab advance calculation (8-space stops)
- [ ] Port from: `pretext/src/line-break.ts` (full file, ~1060 lines)
- [ ] **Tests**:
  - Line count correctness at various widths
  - Semantic pin: trailing spaces don't trigger breaks
  - Semantic pin: soft hyphens invisible unless chosen as break
  - Matrix: maxWidth variations (narrow, wide, exact fit), all segment kinds

### 7E.3.5 Layout API

- [ ] Implement `library/std/text/layout/mod.ori`:
  - [ ] `@layout (prepared, max_width, line_height) -> LayoutResult` — hot path, pure arithmetic
  - [ ] `@layout_lines (prepared, max_width) -> Iterator<LayoutLine>` — with text materialization
  - [ ] `@layout_next_line (prepared, start, max_width) -> Option<LayoutLine>` — streaming
  - [ ] `@walk_line_ranges (prepared, max_width) -> Iterator<LayoutLineRange>` — non-materializing
  - [ ] `@natural_width (prepared) -> float` — intrinsic width
- [ ] Port from: `pretext/src/layout.ts` lines 495-716
- [ ] **Tests**:
  - Round-trip: `layout_lines` line count matches `layout` line count
  - `layout_next_line` with variable widths
  - `natural_width` for single-line and multi-line text
  - Matrix: various measurers (monospace, terminal), various content types

### 7E.3.6 Upgrade `wrap()` to Full Pipeline

- [ ] Replace Phase 1's basic `wrap()` with pipeline-backed implementation:
  - [ ] `wrap()` now calls `analyze()` + `prepare()` + `layout_lines()`
  - [ ] `wrap_measured<M: TextMeasure>()` for custom measurers
- [ ] Verify all existing Phase 1 wrap tests still pass
- [ ] Add new tests for kinsoku, soft hyphens, URLs, numeric runs

### 7E.3.R Third-Party Review Findings

- None.

### 7E.3 Completion Checklist

- [ ] Full Pretext analysis pipeline ported and tested
- [ ] Line breaking matches Pretext behavior on corpus texts
- [ ] All Phase 1 wrap tests still pass after upgrade
- [ ] Evaluator and LLVM produce identical results
- [ ] `ORI_CHECK_LEAKS=1` zero leaks
- [ ] `./test-all.sh` passes
- [ ] `/tpr-review`
- [ ] `/impl-hygiene-review`

- [ ] **Subsection close-out (7E.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7E.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7E.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7E.4 Phase 4: Bidi + Security + Inline Flow + Encoding

**Goal**: Ship the bidirectional text algorithm, confusable detection, mixed inline content layout, and legacy encoding conversion. This completes the full `std.text` API surface.

**Estimated scope**: ~1,000 LOC Rust (bidi class + confusable tables), ~2,000 LOC Ori (algorithms + tests)

**Depends on**: 7E.1 (character properties), 7E.3 (layout engine for inline flow)

### 7E.4.1 Bidi Tables and Algorithm

- [ ] Generate bidi class table in `ori_rt` (~10KB)
- [ ] Implement `library/std/text/unicode/bidi.ori`:
  - [ ] `@bidi_class (c: char) -> BidiClass`
  - [ ] `@paragraph_direction (s: str) -> Direction`
  - [ ] `@bidi_levels (s: str) -> Option<[BidiLevel]>` — simplified UAX #9 (W1-W7, N1-N2, I1-I2)
  - [ ] `@has_bidi_controls (s: str) -> bool` — security check
  - [ ] `@strip_bidi_controls (s: str) -> str`
  - [ ] `@reorder_visual (s: str, levels: [BidiLevel]) -> str`
- [ ] Port from: `pretext/src/bidi.ts` (174 lines)
- [ ] **Tests**: Pure LTR (fast path → None), pure RTL, mixed LTR+RTL, Arabic with numbers, bidi control detection

### 7E.4.2 Confusable Detection (UTS #39)

- [ ] Generate confusable mappings table in `ori_rt` from `confusables.txt` (~50KB)
- [ ] Implement `library/std/text/unicode/security.ori`:
  - [ ] `@skeleton (s: str) -> str`
  - [ ] `@is_confusable (a: str, b: str) -> bool`
  - [ ] `@mixed_script_status (s: str) -> MixedScriptStatus`
  - [ ] `@restriction_level (s: str) -> RestrictionLevel`
- [ ] **Tests**:
  - Semantic pin: `is_confusable("аpple", "apple") == true` (Cyrillic а vs Latin a)
  - Mixed script: Latin-only → SingleScript, Latin+Common → SafeMix, Latin+Cyrillic → SuspiciousMix

### 7E.4.3 Inline Flow Layout

- [ ] Implement `library/std/text/layout/inline_flow.ori`:
  - [ ] `@prepare_inline_flow<M> (items, measurer) -> PreparedInlineFlow`
  - [ ] `@layout_inline_flow_lines (prepared, max_width) -> Iterator<InlineFlowLine>`
  - [ ] `@measure_inline_flow (prepared, max_width, line_height) -> LayoutResult`
  - [ ] Atomic items (break: Never), boundary whitespace collapse, gap calculation
- [ ] Port from: `pretext/src/inline-flow.ts` (344 lines)
- [ ] **Tests**: Mixed text + atomic chips, boundary whitespace, variable item widths

### 7E.4.4 Encoding Conversion

- [ ] Generate encoding tables in `ori_rt` or FFI to `encoding_rs`:
  - [ ] UTF-8/16/32 interconversion
  - [ ] Latin-1, Windows-1252
  - [ ] Shift-JIS, EUC-JP, ISO-2022-JP
  - [ ] GB2312, GBK, GB18030
  - [ ] Big5, EUC-KR
  - [ ] ISO 8859 parts 1-16
- [ ] Implement `library/std/text/transform/encoding.ori`:
  - [ ] `@decode (bytes: [byte], encoding: Encoding) -> Result<str, EncodingError>`
  - [ ] `@encode (s: str, encoding: Encoding) -> Result<[byte], EncodingError>`
  - [ ] `@detect_encoding (bytes: [byte]) -> Encoding` — heuristic detection
- [ ] **Tests**: Round-trip encode/decode for each encoding, invalid byte sequences, BOM detection

### 7E.4.R Third-Party Review Findings

- None.

### 7E.4 Completion Checklist

- [ ] All bidi, security, inline flow, encoding APIs implemented
- [ ] Evaluator and LLVM produce identical results
- [ ] `ORI_CHECK_LEAKS=1` zero leaks
- [ ] `./test-all.sh` passes
- [ ] `/tpr-review`
- [ ] `/impl-hygiene-review`

- [ ] **Subsection close-out (7E.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7E.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7E.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7E.5 Section Completion Checklist

- [ ] All 4 phases implemented
- [ ] Full `std.text` API surface matches approved proposal
- [ ] All Unicode test suites pass (Grapheme, Word, Normalization)
- [ ] Pretext corpus tests adapted and passing
- [ ] `std.text 0.1.0` version tagged (Phase 1)
- [ ] `std.text 0.2.0` version tagged (Phase 2)
- [ ] `std.text 0.3.0` version tagged (Phase 3)
- [ ] `std.text 0.4.0` version tagged (Phase 4)
- [ ] `library/std/text/mod.ori` stub replaced with full implementation
- [ ] Performance meets targets from proposal (display_width < 5ns/ASCII char, layout < 200ns/block)
- [ ] `/tpr-review` (final)
- [ ] `/impl-hygiene-review` (final)
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

- [ ] **Subsection close-out (7E.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7E.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7E.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

