---
section: "04"
title: "Parser Optimizations"
status: not-started
reviewed: false
goal: "Measurable throughput improvement in ori_parse benchmarks via inline audit, arena pre-allocation, snapshot enhancement, and expression parsing tuning."
inspired_by:
  - "Chumsky src/input.rs:1487-1523 — checkpoint rewind (cursor + error count truncation)"
  - "Chumsky src/pratt.rs — binding power model with OperatorResult error separation"
  - "Rust rustc_ast/src/ast.rs — per-node with_capacity hints for collections"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Cross-Crate Inline Audit"
    status: not-started
  - id: "04.2"
    title: "ExprArena Pre-Allocation"
    status: not-started
  - id: "04.3"
    title: "Snapshot Enhancement"
    status: not-started
  - id: "04.4"
    title: "Expression Parsing Tuning"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Parser Optimizations

**Status:** Not Started
**Goal:** Parser throughput improves from ~95-128 MiB/s baseline by at least 10%, verified via `cargo bench -p oric --bench parser -- "parser/raw"`. No regressions in test suite.

**Context:** The parser is already well-optimized — parallel `tags[u8]` arrays for O(1) discriminant checks, static `OPER_TABLE[128]` binding power table, `ParseOutcome<T>` four-way progress tracking, and `TokenSet` `[u128; 2]` bitset recovery. Research confirmed these patterns are superior to Chumsky's approach. Remaining gains are in: (1) cross-crate inline annotations for parser functions; (2) `ExprArena` pre-allocation from file size; (3) enhanced snapshot/checkpoint state; (4) micro-optimizations in the Pratt parser loop.

**Reference implementations:**
- **Chumsky** `src/input.rs:1487-1523`: Checkpoint saves `(cursor, error_count)` and rewinds by truncating the error vec. Ori's `ParserSnapshot` saves cursor + context but not error state.
- **Rust** `rustc_ast/src/ast.rs`: Uses `with_capacity(len_hint * 8)` for per-node collections with comments noting it "works well in practice."
- **Chumsky** `src/pratt.rs`: `OperatorResult` enum distinguishes NoMatch from fatal Error — cleaner than returning `Option<(bp, op)>`.

**Depends on:** Section 01 (baselines), Section 02 (`lib.rs` must be split first — parser struct methods are in `lib.rs`).

---

## 04.1 Cross-Crate Inline Audit

**File(s):** `compiler/ori_parse/src/cursor/mod.rs`, `compiler/ori_parse/src/grammar/expr/mod.rs`, `compiler/ori_parse/src/grammar/expr/operators.rs`, `compiler/ori_parse/src/outcome/mod.rs`

Audit all functions in the parser expression hot path. The Pratt parser loop (`parse_binary_pratt`) calls multiple functions per token — any unannotated cross-crate call multiplies.

- [ ] **Write benchmark baseline test**: Record `parser/raw` before changes.

**Note:** File paths assume Section 02 splits are complete. Parser methods currently in `lib.rs` will have moved to `parser/mod.rs`. The grammar modules (`grammar/expr/`, `cursor/`) remain in their current locations.

**Note on `ori_ir` inline annotations:** Section 03.1 also audits `ori_ir` types (`TokenList::push_with_tag`). This section covers the `ExprArena`/`Expr` side of `ori_ir`. Both sections modify `ori_ir` — coordinate if working in parallel.

**Hygiene warning:** `ori_ir/src/arena/mod.rs` (530L) and `ori_ir/src/ast/expr.rs` (510L) both exceed the 500-line limit. Adding `#[inline]` to `Expr::new()` is safe (no net lines added), but if ANY code is added to these files beyond annotations, split them first. See Section 02.3 codebase scan findings for details.

- [ ] Audit functions in the Pratt parser loop (`parse_binary_pratt()` in `grammar/expr/mod.rs`):
  - `self.cursor.skip_newlines()` — already `#[inline]`. Verify.
  - `self.cursor.current_kind()` — already `#[inline]`. Verify.
  - `self.infix_binding_power()` — already `#[inline]`. Verify.
  - `self.cursor.advance()` — already `#[inline]`. Verify.
  - `self.arena.alloc_expr(Expr::new(...))` — `alloc_expr()` **already has `#[inline]`** (`ori_ir/src/arena/mod.rs:197`). Called for every AST node. Crosses crate boundary.
  - `self.arena.get_expr(id)` — **already has `#[inline]`** AND `#[track_caller]` (`ori_ir/src/arena/mod.rs:213-214`). Called for span merging in the Pratt loop. No action needed.
  - `Expr::new(kind, span)` — **missing `#[inline]`** (`ori_ir/src/ast/expr.rs:42`). Constructor called per node. Two-field struct construction — should be `#[inline]`. **Annotate.**

- [ ] Audit functions in `parse_unary()`:
  - `self.match_unary_op()` — already `#[inline]`. Verify.
  - `self.parse_call()` — NOT `#[inline]` (too large). Correct.

- [ ] Audit `ParseOutcome` conversions:
  - `ParseOutcome::consumed_ok()` — **already has `#[inline]`** (verified in `outcome/mod.rs:107`).
  - `ParseOutcome::into_result()` — **missing `#[inline]`** (`outcome/mod.rs:390`). Called in the Pratt loop via `parse_binary_pratt(r_bp).into_result()?`. **Annotate.**
  - `chain!`, `committed!`, `require!` macros — these are already expanded inline.

- [ ] Annotate any unannotated hot-path functions. Focus on `Expr::new()` and `ParseOutcome::into_result()` which are missing `#[inline]` on cross-crate hot paths. `ExprArena::alloc_expr()` and `get_expr()` already have `#[inline]`.

- [ ] **Verify improvement**: Run benchmarks, compare against baseline.

**Matrix dimensions:**
- Input types: simple functions, expression-heavy, nested conditionals
- Sizes: 100, 500, 1000 functions
- Nesting depths: 5, 10, 20, 50

---

## 04.2 ExprArena Pre-Allocation

**File(s):** `compiler/ori_parse/src/parser/mod.rs` (post-Section 02 split), `compiler/ori_ir/src/arena/mod.rs`

**Note:** File paths assume Section 02 splits are complete. `Parser::new()` will be in `parser/mod.rs` after the split.

Current arena sizing: `Parser::new()` estimates source byte length as `tokens.len() * 5`, then `ExprArena::with_capacity(estimated_source_len)` divides by 20 to get estimated expression count. The chain is: `tokens * 5 / 20 = tokens / 4` expressions. This is indirect — source byte length is estimated from token count.

**Feasibility note:** The parser receives `&TokenList`, not source text. Actual `source.len()` is not available without API changes. However, the EOF token's span end position equals the source length, so `tokens.last().span.end` could be used as a proxy without API changes.

- [ ] Profile actual arena usage vs. capacity for different workloads:
  - `tokens.len() / 4` (current estimated expressions) vs actual `arena.len()` after parsing
  - Compare against `eof_span.end / 20` heuristic (using actual source length from EOF token)

- [ ] If the EOF-based heuristic is more accurate, switch to:
  ```rust
  // Use EOF token's span end as actual source length
  // TokenList has no last() method, use as_slice().last()
  let source_len = tokens.as_slice().last().map_or(0, |t| t.span.end as usize);
  ExprArena::with_capacity(source_len)
  ```

- [ ] Verify no reallocation occurs during parsing for benchmark workloads (instrument `Vec::capacity()` after parse completes, compare to initial capacity).

---

## 04.3 Snapshot Enhancement

**File(s):** `compiler/ori_parse/src/snapshot/mod.rs`

Chumsky's checkpoint captures `(cursor, error_count)` and rewinds by truncating the error vec. Ori's `ParserSnapshot` captures cursor position + `ParseContext` flags but not error state. Adding error count to snapshots enables more aggressive speculative parsing without accumulating spurious errors.

- [ ] Read `snapshot/mod.rs` to understand current snapshot state.

- [ ] **Hygiene fix while here**: `Parser::try_parse()` (currently in `lib.rs:439`, will move to `parser/mod.rs` after Section 02.1) has `#[allow(dead_code, reason = "...")]` which should be `#[expect(dead_code, reason = "...")]` per hygiene rules. Fix this during the Section 02.1 split.

- [ ] Evaluate whether adding `error_count: usize` to `ParserSnapshot` is beneficial:
  - Survey all `snapshot()`/`restore()` call sites
  - Determine if any speculative parsing path currently accumulates errors that are then discarded
  - If yes: add error count to snapshot, truncate on restore
  - If no: document why it's not needed and skip

- [ ] If implementing:
  ```rust
  pub struct ParserSnapshot {
      position: usize,
      context: ParseContext,
      error_count: usize,  // NEW: for error truncation on rewind
  }
  ```
  - `snapshot()`: captures `self.deferred_errors.len()`
  - `restore()`: truncates `self.deferred_errors` to saved count

- [ ] If implementing, write correctness tests BEFORE the change (TDD):
  - **Test matrix for error truncation on snapshot restore:**
    - Snapshot/restore with zero errors accumulated between snapshot and restore
    - Snapshot/restore with 1 error accumulated (verify it is discarded on restore)
    - Snapshot/restore with multiple errors accumulated (verify all discarded)
    - Nested snapshots (snapshot A, then snapshot B, restore B, restore A)
    - Snapshot taken, errors accumulated, then parsing COMMITTED (not restored) -- errors must be kept
  - **Semantic pin:** Parse input that triggers speculative parsing (e.g., ambiguous expression vs struct literal context). Verify that errors from the rejected speculative branch do NOT appear in the final error list. This test must FAIL without the error truncation and PASS with it.
  - Place tests in `compiler/ori_parse/src/snapshot/tests.rs`

- [ ] Benchmark impact: speculative parsing should be slightly faster due to avoiding error processing for rejected branches.

---

## 04.4 Expression Parsing Tuning

**File(s):** `compiler/ori_parse/src/grammar/expr/mod.rs`, `compiler/ori_parse/src/grammar/expr/operators.rs`

Micro-optimizations in the Pratt parser loop and expression dispatch. These are individually small but accumulate since expression parsing is the hottest parser path.

- [ ] **Newline skipping optimization**: `parse_binary_pratt()` calls `self.cursor.skip_newlines()` at the top of every loop iteration. **Verified: `skip_newlines()` already has a fast-path check internally** (`cursor/mod.rs:468-471`: `while self.current_tag() == TAG_NEWLINE { self.advance(); }`). When there are no newlines, it returns immediately after one tag comparison. An outer check would be redundant. **Skip this optimization** — no action needed.

- [ ] **`op_from_u8()` optimization** — **WHERE:** `compiler/ori_parse/src/grammar/expr/operators.rs:80-87`, generated by `define_operators!` macro (line 66). Currently uses an if-chain: `if op == 0 { return BinaryOp::Add; } if op == 1 { ... }`. `unsafe { transmute }` is **infeasible**: `BinaryOp` is defined in `ori_ir` which has `#![deny(unsafe_code)]`, and `ori_parse` should also deny unsafe per hygiene rules. **Alternative**: modify the `define_operators!` macro to also generate a `const OP_LOOKUP: [BinaryOp; TABLE_OPERATOR_COUNT] = [BinaryOp::$op, ...];` array, then replace `op_from_u8(idx)` body with `OP_LOOKUP[idx as usize]` -- a single array index instead of N branches. Verify the existing `op_from_u8_roundtrips_all_table_entries` test (line 348) still passes. Profile whether this matters (the if-chain may already compile to a jump table with optimizations enabled).

- [ ] **Range check elimination** — **WHERE:** `compiler/ori_parse/src/grammar/expr/operators.rs`, `infix_binding_power()` (line 174). Has a `tag >= 128` guard that returns `None` for non-operator tags. Profile via: (1) run `cargo bench -p oric --bench parser -- "parser/raw"` as baseline, (2) temporarily remove the guard and use `OPER_TABLE.get(tag as usize)` instead of index+guard, (3) re-run benchmark. If improvement < 1%, revert and skip. The guard is almost certainly well-predicted (operator tags are always < 128) so this may not be worth changing.

- [ ] Benchmark all changes: `cargo bench -p oric --bench parser -- "parser/raw"` and `parser/nesting`.

**Semantic pin:** Expression-heavy benchmark (from Section 01) should show measurable improvement (> 3%) from cumulative micro-optimizations.

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] All per-expression functions in Pratt parser loop audited for `#[inline]`
- [ ] `ExprArena::alloc_expr()`, `get_expr()` confirmed `#[inline]` (already done); `Expr::new()` and `ParseOutcome::into_result()` annotated with `#[inline]`
- [ ] Arena pre-allocation heuristic profiled and tuned
- [ ] Snapshot enhancement evaluated (implemented or rejected with evidence)
- [ ] Expression parsing micro-optimizations evaluated and benchmarked
- [ ] Benchmark improvement measured against Section 01 baselines
- [ ] `timeout 150 cargo t -p ori_parse` passes
- [ ] `timeout 150 cargo st` passes
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Debug AND release builds pass: `timeout 150 cargo t -p ori_parse --release`
- [ ] If snapshot enhancement implemented: correctness tests in `snapshot/tests.rs` pass (error truncation matrix + semantic pin)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Correctness verification:** Sections 04.1, 04.2, and 04.4 are performance-only (adding `#[inline]`, tuning allocation, micro-optimizations). The existing parser test suite serves as the correctness regression guard. Run the full suite BEFORE any optimization to establish a green baseline, then after each change. Section 04.3 (snapshot enhancement) adds a behavioral change -- it requires dedicated correctness tests written BEFORE the implementation (see test matrix above). If any test fails after an optimization, the optimization introduced a bug -- revert and investigate.

**Exit Criteria:** `cargo bench -p oric --bench parser -- "parser/raw"` shows throughput improvement compared to baselines. All test suites green in debug AND release. Final improvement measurement deferred to Section 06 (unified verification).
