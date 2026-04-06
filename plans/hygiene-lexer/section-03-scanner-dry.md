---
section: "03"
title: "Scanner Layer Algorithmic DRY"
status: not-started
reviewed: false
goal: "Consolidate 6 simple operator functions with identical control-flow skeletons"
success_criteria:
  - "6 simple operator functions use a shared helper"
  - "Complex operator functions (less, dot, pipe, ampersand, equal, minus_or_arrow) unchanged"
  - "All existing tests pass unchanged"
inspired_by:
  - "rustc_lexer — uses match-based dispatch, not per-operator functions"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Extract simple_or_compound helper"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Scanner Layer Algorithmic DRY

**Status:** Not Started
**Goal:** Consolidate 6 simple operator scanning functions in `raw_scanner/operators.rs` that share the identical skeleton: advance → check `=` → compound or simple tag.

**Success Criteria:**

- [ ] A shared `simple_or_compound()` helper exists in `raw_scanner/operators.rs`
- [ ] `plus`, `star`, `percent`, `caret`, `at`, `bang` use the shared helper
- [ ] Complex operators (`less`, `dot`, `pipe`, `ampersand`, `equal`, `minus_or_arrow`, `colon`, `hash`, `question`) are unchanged — they have unique multi-level lookahead trees
- [ ] All existing tests pass unchanged
- [ ] Satisfies mission criterion: "Simple operator scanning consolidated"

**Context:** Six operator functions in `compiler/ori_lexer_core/src/raw_scanner/operators.rs` share identical structure:
```
advance() → if current() == b'=' { advance(); compound_tag } else { simple_tag }
```
The functions are `plus` (line 19), `star` (line 59), `percent` (line 75), `caret` (line 91), `at` (line 107), `bang` (line 147). Each is 14 lines and differs only in the two `RawTag` values.

**Depends on:** Section 01 (bug fix must land first).

---

## 03.1 Extract simple_or_compound helper

**File(s):** `compiler/ori_lexer_core/src/raw_scanner/operators.rs`

- [ ] Add a shared helper method:
  ```rust
  /// Scan a simple operator that may have a `=` compound form.
  ///
  /// Advance past the operator byte. If the next byte is `=`, advance
  /// again and return `compound_tag`. Otherwise return `simple_tag`.
  fn simple_or_compound(
      &mut self, start: u32,
      simple_tag: RawTag, compound_tag: RawTag,
  ) -> RawToken {
      self.cursor.advance();
      if self.cursor.current() == b'=' {
          self.cursor.advance();
          RawToken { tag: compound_tag, len: self.cursor.pos() - start }
      } else {
          RawToken { tag: simple_tag, len: self.cursor.pos() - start }
      }
  }
  ```

- [ ] Replace 6 functions with delegating one-liners:
  ```rust
  pub(super) fn plus(&mut self, start: u32) -> RawToken {
      self.simple_or_compound(start, RawTag::Plus, RawTag::PlusEq)
  }
  ```

- [ ] Keep `single()` (line 11) as-is — it handles operators with NO compound form (different pattern)
- [ ] Keep all complex operators as-is — they have multi-level lookahead trees that cannot be parameterized by two tags
- [ ] Verify: `timeout 150 cargo test -p ori_lexer_core` — all scanner tests pass unchanged
- [ ] File stays well under 500 lines (currently 395 → should shrink to ~310)

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] `simple_or_compound()` helper exists in `operators.rs`
- [ ] 6 simple operator functions delegate to the helper
- [ ] Complex operators unchanged
- [ ] `timeout 150 cargo test -p ori_lexer_core` green (debug)
- [ ] `timeout 150 cargo test -p ori_lexer_core --release` green (release)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] `operators.rs` under 500 lines
- [ ] Plan annotation cleanup: no stale annotations
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** The 6 simple operator functions in `operators.rs` delegate to `simple_or_compound()`. All existing scanner tests pass unchanged. The operator dispatch pattern is DRY — adding a new simple `X` / `X=` operator requires one line, not 14.
