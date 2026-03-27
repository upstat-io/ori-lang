---
section: "05"
title: "Salsa Integration"
status: not-started
reviewed: false
goal: "Reduce Salsa query overhead and activate incremental parsing infrastructure for sub-linear re-parse on small edits."
inspired_by:
  - "Chumsky src/cache.rs — parser caching for lifetime abstraction"
  - "Go src/go/parser/interface.go — Mode bitfield for parse depth control"
  - "Rust-Analyzer salsa integration — per-file query granularity with memoization"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Query Overhead Profiling"
    status: not-started
  - id: "05.2"
    title: "Incremental Parsing Activation"
    status: not-started
  - id: "05.3"
    title: "Query Granularity Evaluation"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Salsa Integration

**Status:** Not Started
**Goal:** Salsa query overhead for parsing is quantified precisely (currently estimated at 30-40%). Incremental parsing infrastructure (already implemented in `ori_parse/incremental/`) is activated and connected to the Salsa query pipeline. Small edits to large files trigger sub-linear re-parse rather than full re-parse.

**Context:** Ori's Salsa integration provides excellent caching — unchanged files return cached results instantly. However, the 30-40% overhead for first-time parsing is significant. More importantly, the `ori_parse/incremental/` module (1,595 lines of copier, cursor, declaration tracking) exists but is NOT connected to the `parsed()` Salsa query. The Salsa query always calls `parser::parse()` (full reparse). Connecting incremental parsing to Salsa would mean: edit one function → re-parse only that function → reuse all other AST nodes.

Additionally, Ori already has `parse_incremental()` exposed from `ori_parse` and benchmarked in `compiler/oric/benches/parser.rs` (incremental_benches module), but the Salsa `parsed()` query doesn't use it.

**Reference implementations:**
- **Rust-Analyzer**: Uses Salsa with per-file query granularity. Unchanged files return cached AST. The solution to query overhead is fine-grained queries, not additional caching layers.
- **Go** `src/go/parser/interface.go`: `Mode` bitfield (`PackageClauseOnly`, `ImportsOnly`) for parse depth control. Not applicable — Ori's `ori check` always needs the full AST for type checking.

**Depends on:** Section 01 (baselines for overhead measurement).

**Hygiene warning:** `oric/src/query/mod.rs` is 582 lines (80 lines over the 500-line limit). Before adding incremental parsing logic in Section 05.2, split `query/mod.rs` first. Suggested extraction: move `canonicalize_cached()` and related canonicalization functions (~100 lines) to `query/canonicalize.rs`, and/or move `evaluated()` and related evaluation queries to `query/eval.rs`. This is a prerequisite for 05.2 if approach (a) is chosen (which adds ~30 lines to the `parsed()` query area).

---

## 05.1 Query Overhead Profiling

**File(s):** `compiler/oric/src/query/mod.rs`, `compiler/oric/benches/parser.rs`

Quantify the exact Salsa overhead by comparing raw parse throughput vs Salsa-mediated throughput on identical workloads.

- [ ] Use the `parser/salsa_overhead` benchmark from Section 01.2 as a starting point. Extend it to cover additional workload sizes (100, 1000, 5000 functions) and cache modes (cold, warm, modified) if needed:
  - Raw parse throughput (lexer + parser directly)
  - Salsa-mediated throughput (via `parsed()` query with fresh `SourceFile`)
  - Compute overhead percentage: `(raw - salsa) / raw * 100`

- [ ] Profile where the overhead comes from using `ORI_LOG=oric=trace`:
  - `SourceFile` creation and hashing
  - `tokens()` query dispatch (dependency tracking)
  - `parsed()` query dispatch (dependency tracking + result hashing)
  - Result equality comparison (for early cutoff)

- [ ] Document overhead breakdown:
  - If overhead is in `SourceFile` creation: input construction is the bottleneck
  - If overhead is in query dispatch: Salsa tracking mechanism
  - If overhead is in result hashing: `ParseOutput` hash computation
  - If overhead is in equality comparison: `ParseOutput` equality check

- [ ] Record results in `baselines.json` as `salsa_overhead_percent` and `salsa_overhead_breakdown`.

**Matrix dimensions:**
- Workload sizes: 100, 500, 1000, 5000 functions
- Modes: first parse (cold), re-parse same input (warm cache), re-parse modified input

---

## 05.2 Incremental Parsing Activation

**File(s):** `compiler/oric/src/query/mod.rs`, `compiler/ori_parse/src/lib.rs` (post-split), `compiler/ori_parse/src/incremental/`

Connect the existing incremental parsing infrastructure to the Salsa `parsed()` query. Currently `parsed()` always calls `parser::parse()`. With incremental parsing, it should detect small edits and call `parse_incremental()` instead.

- [ ] **Prerequisite:** Split `compiler/oric/src/query/mod.rs` (582L) to get under 500 lines before adding incremental parsing logic. See hygiene warning above for suggested extraction targets (`canonicalize_cached()` to `query/canonicalize.rs`).

- [ ] Study the existing incremental parsing API:
  ```rust
  // Already exists in ori_parse (lib.rs:1286):
  pub fn parse_incremental(
      tokens: &TokenList,
      interner: &StringInterner,
      old_result: &ParseOutput,
      change: ori_ir::incremental::TextChange,  // { start: u32, old_end: u32, new_len: u32 }
  ) -> ParseOutput
  ```

- [ ] Evaluate how to detect the `TextChange` in the Salsa context:
  - `SourceFile` tracks `text(db)`. When text changes, Salsa invalidates `lex_result()`.
  - Challenge: Salsa doesn't provide the *diff* — only the new value. The `TextChange` (`start: u32`, `old_end: u32`, `new_len: u32`) is not available from Salsa's invalidation path.
  - Options:
    **(a) Store previous text in a side-cache** (like `PoolCache`): Before `tokens_with_metadata()` re-lexes, compute `TextChange` from old text vs new text. Store old text in a `TextCache` keyed by path.
    **(b) Accept text from caller**: Modify the `SourceFile` input to carry the `TextChange` alongside the new text. This requires API changes to `SourceFile::set_text()`.
    **(c) Diff computation at query time**: Use a fast diff algorithm (e.g., `similar::TextDiff`) to compute the change span from old and new text.

- [ ] Evaluate which approach is best:
  - **(a)** is simplest and follows the `PoolCache`/`CanonCache` pattern already in use
  - **(b)** is most correct but requires API changes
  - **(c)** is most expensive but requires no infrastructure
  - **Critical constraint:** `TextChange` represents a single contiguous edit region (`start`, `old_end`, `new_len`). For multi-cursor edits or format-on-save (many scattered changes), the diff must conservatively cover the entire modified range. For edits that span the whole file, incremental falls back to full reparse anyway. This limits the win to typical interactive editing (single-point edits).
  - **Additional prerequisite for all options:** `compute_text_change(old_text, new_text) -> Option<TextChange>` must be implemented. A simple O(n) scan from both ends to find the single differing region is sufficient and avoids external dependencies. Returns `None` if texts are identical (Salsa cutoff already handles this) or if a precise single-region change cannot be determined.

- [ ] Implement `compute_text_change(old_text: &str, new_text: &str) -> Option<TextChange>`:
  - O(n) scan from both ends to find the single differing region
  - Returns `None` if texts are identical or if a precise single-region change cannot be determined (e.g., multiple disjoint changes that require conservative whole-file coverage — fall back to full reparse)
  - Place in `compiler/ori_parse/src/incremental/` (alongside existing incremental infrastructure), e.g., `incremental/text_diff.rs`
  - **Algorithm sketch:**
    ```rust
    pub fn compute_text_change(old: &str, new: &str) -> Option<TextChange> {
        if old == new { return None; }
        let old_bytes = old.as_bytes();
        let new_bytes = new.as_bytes();
        // Scan from front to find first difference
        let prefix_len = old_bytes.iter().zip(new_bytes).take_while(|(a, b)| a == b).count();
        // Scan from back to find last difference (don't overlap with prefix)
        let old_suffix = &old_bytes[prefix_len..];
        let new_suffix = &new_bytes[prefix_len..];
        let suffix_len = old_suffix.iter().rev().zip(new_suffix.iter().rev())
            .take_while(|(a, b)| a == b).count();
        let start = prefix_len as u32;
        let old_end = (old.len() - suffix_len) as u32;
        let new_len = (new.len() - suffix_len - prefix_len) as u32;
        Some(TextChange { start, old_end, new_len })
    }
    ```
  - **Caveat:** Must respect UTF-8 char boundaries — the scan above operates on bytes which is correct for finding the differing region, but verify `start`/`old_end` land on char boundaries. If not, widen the region to the nearest char boundary.
  - Add unit tests in `incremental/text_diff/tests.rs` for: identical texts, single insertion, single deletion, single replacement, prefix-only change, suffix-only change, whole-file change, empty old/new, multi-byte UTF-8 characters at edit boundary

- [ ] If approach (a) is chosen, implement `ParseCache` in `compiler/oric/src/db/mod.rs`:
  - `struct ParseCache(Arc<RwLock<HashMap<PathBuf, (ParseOutput, String)>>>)` — follows `PoolCache`/`CanonCache` structural pattern
  - Add `parse_cache: ParseCache` field to `CompilerDb`
  - Add `fn parse_cache(&self) -> &ParseCache` to `Db` trait
  - Implement `store()`, `get()`, `invalidate()` methods
  - **Important:** Do NOT wire into `invalidate_file_caches()`. Unlike `PoolCache`/`CanonCache` which must be cleared before re-type-checking, `ParseCache` stores the PREVIOUS parse result which is needed by `parsed()` for incremental reparse. Clearing it would defeat the purpose. The cache is self-invalidating: `store()` overwrites the old entry each time `parsed()` runs. The only manual invalidation needed is when a file is removed from the project (wire into file-removal paths, if any).

- [ ] If approach (a) is chosen, modify the `parsed()` query along these lines (design sketch — verify actual API signatures before implementing):
  ```rust
  // DESIGN SKETCH — prerequisites and notes:
  // - db.parse_cache() does NOT exist yet; must be added to CompilerDb (like PoolCache)
  //   Pattern: struct ParseCache(Arc<RwLock<HashMap<PathBuf, (ParseOutput, String)>>>)
  //   Add to CompilerDb, implement store/get/invalidate methods
  // - parse_incremental is a free function: ori_parse::parse_incremental(tokens, interner, old, change)
  //   Signature: (tokens: &TokenList, interner: &StringInterner, old_result: &ParseOutput, change: TextChange) -> ParseOutput
  // - compute_text_change() must be implemented (diff algorithm)
  //   TextChange has fields: start: u32, old_end: u32, new_len: u32
  //
  // In query/mod.rs:
  #[salsa::tracked]
  pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseOutput {
      let toks = tokens(db, file);
      let path = file.path(db);

      // Try incremental reparse if previous result exists
      if let Some((old_result, old_text)) = db.parse_cache().get(path) {
          let new_text = file.text(db);
          if let Some(change) = compute_text_change(&old_text, new_text) {
              let result = ori_parse::parse_incremental(&toks, db.interner(), &old_result, change);
              db.parse_cache().store(path, result.clone(), new_text.to_string());
              return result;
          }
      }

      // Full parse fallback
      let result = ori_parse::parse(&toks, db.interner());
      db.parse_cache().store(path, result.clone(), file.text(db).to_string());
      result
  }
  ```

- [ ] Benchmark incremental vs full reparse using existing benchmarks:
  ```bash
  cargo bench -p oric --bench parser -- "incremental"
  ```
  Expected: Incremental reparse of a single-function edit in a 200-function file should be 5-10x faster than full reparse.

- [ ] Add new benchmark: Salsa-mediated incremental (edit `SourceFile`, re-query `parsed()`).

**Matrix dimensions:**
- File sizes: 10, 50, 100, 200 functions
- Edit locations: start, middle, end of file
- Edit types: single-function body change, function addition, function deletion
- Modes: full reparse, incremental reparse, Salsa cached (no change)

**Semantic pin:** Incremental reparse of a 200-function file with one function edited should complete in < 50% of full reparse time.

---

## 05.3 Query Granularity Evaluation

**File(s):** `compiler/oric/src/query/mod.rs`

Evaluate whether finer-grained Salsa queries (per-function rather than per-file) would reduce overhead for downstream consumers (type checker, evaluator).

- [ ] Analyze the current query dependency chain:
  ```
  SourceFile → lex_result → tokens → parsed → typed → evaluated
  ```
  A change to one function in a file invalidates `parsed()`, which invalidates `typed()`, which re-type-checks the entire file.

- [ ] Evaluate per-function query granularity:
  - Would `parsed_function(db, file, fn_id) -> FunctionAst` queries enable per-function type checking?
  - What would the query key be? (Function name? Declaration index?)
  - How would new/deleted functions be handled?

- [ ] Decision: implement or skip with documented evidence. If the incremental parsing in 05.2 achieves < 50% of full reparse for single-function edits, document the measured speedup and explain why per-function queries are not needed. If 05.2 does NOT achieve that threshold, implement per-function queries as a mandatory follow-up.

- [ ] If implementing, design the query split:
  ```rust
  #[salsa::tracked]
  pub fn parsed_items(db: &dyn Db, file: SourceFile) -> Vec<ParsedItem> { ... }

  #[salsa::tracked]
  pub fn parsed_function_body(db: &dyn Db, file: SourceFile, item_id: ItemId) -> ExprId { ... }
  ```
  This would allow unchanged function bodies to skip re-type-checking.

- [ ] Document the decision and rationale in this section.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] `oric/src/query/mod.rs` split to < 500 lines (prerequisite for 05.2)
- [ ] Salsa overhead precisely quantified (not estimated)
- [ ] Overhead breakdown documented (source: input creation, query dispatch, result hashing, or equality)
- [ ] Incremental parsing evaluated and connected to Salsa, or skipped with documented benchmark evidence showing it is not needed
- [ ] If incremental parsing activated: `compute_text_change()` implemented with unit tests
- [ ] If incremental parsing activated via approach (a): `ParseCache` added to `CompilerDb` (NOT wired into `invalidate_file_caches()` -- see 05.2 design notes)
- [ ] Query granularity evaluated and implemented, or skipped with documented benchmark evidence showing incremental parsing (05.2) already meets the < 50% threshold
- [ ] Benchmark improvement measured for incremental path
- [ ] `timeout 150 cargo t -p oric` passes
- [ ] `timeout 150 cargo st` passes
- [ ] `./test-all.sh` green
- [ ] Debug AND release builds pass: `timeout 150 cargo t -p oric --release`
- [ ] If incremental parsing activated: behavioral equivalence verified (incremental parse output == full parse output for all test cases)

**Correctness verification:** Section 05.2 introduces a significant behavioral change -- the Salsa `parsed()` query will sometimes return incrementally-reparsed ASTs instead of fully-reparsed ASTs. These MUST be semantically equivalent. Testing strategy:

1. **`compute_text_change()` unit tests** (write BEFORE implementation -- TDD):
   - Test matrix dimensions: edit types (insertion, deletion, replacement) x edit locations (prefix, middle, suffix, whole-file) x content types (ASCII, multi-byte UTF-8 at boundary)
   - Edge cases: identical texts (returns `None`), empty old text, empty new text, single-char change
   - Semantic pin: verify that `compute_text_change("abcXdef", "abcYZdef")` returns `TextChange { start: 3, old_end: 4, new_len: 2 }` -- this test ONLY passes with correct diff computation

2. **Incremental parsing equivalence tests** (write BEFORE wiring into Salsa):
   - For each test case: full-parse the modified source AND incremental-parse from old result + TextChange. Compare the two `ParseOutput` values for equality.
   - Test matrix: edit types (add function, delete function, modify function body, add import, modify type signature) x file sizes (10, 50, 200 functions)
   - Semantic pin: parse a 100-function file, modify one function body, verify incremental result equals full parse result. This ensures the incremental path produces correct ASTs.

3. **Integration regression**: After wiring into Salsa, run `./test-all.sh` and `cargo st` to verify no downstream regressions in type checking or evaluation from incrementally-parsed ASTs.

**Exit Criteria:** Salsa overhead is precisely measured and documented. If incremental parsing is activated, single-function edits in 200-function files re-parse in < 50% of full reparse time via `parsed()` Salsa query. All test suites green.
