---
bug: "BUG-04-019"
title: "Niche-encoded Option/Result extraction paths missing RC retain and tag guards"
severity: "medium"
status: complete
goal: "Niche-encoded Option/Result helper methods (unwrap, unwrap_err, unwrap_or, expect, expect_err) emit correct LLVM IR with tag guards and RC retain on payload extraction, mirroring the explicit-tag pattern established by BUG-04-013"
success_criteria:
  - "Each unwrap-style niche helper computes a niche-aware variant predicate using `niche_is_sentinel` and calls `emit_unwrap_branch` (panic on wrong variant) before payload extraction"
  - "Each expect-style niche helper threads the user message through `emit_expect_branch` with a niche-aware predicate"
  - "Each extraction path that returns a payload calls `inc_value_rc(payload, inner_ty, 1)` after extraction (unconditional after panic guard, conditional via cond_br for unwrap_or-style)"
  - "Result.unwrap, Result.unwrap_err, Result.unwrap_or are differentiated semantically (currently collapsed into a single arm that ignores the method name)"
  - "A Rust unit test in `option_result_helpers/tests.rs` directly invokes each fixed helper with a synthetic niche TagEncoding and asserts the printed LLVM IR contains both `ori_panic*` (tag guard) and `ori_str_rc_inc` (RC retain)"
  - "New niche spec tests `tests/spec/types/enum/niche/{option_unwrap,result_unwrap}.ori` exercise unwrap/expect/unwrap_err/expect_err on Option<str> and Result<str, error>; pass via interpreter today, will validate the LLVM niche path when NICHE_CODEGEN_READY flips on"
subsystem: "compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs"
found: "2026-04-02"
source: "tpr-review (BUG-04-013 follow-up)"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-019 — Niche-encoded Option/Result extraction paths missing RC retain and tag guards

**Status:** Complete (2026-04-07)
**Severity:** medium
**Goal:** The niche-encoded helper methods in `option_result_helpers.rs` emit correct LLVM IR for `unwrap` / `unwrap_err` / `unwrap_or` / `expect` / `expect_err` on Option and Result, with both tag guards (panic on the wrong variant) and RC retain on the extracted payload. The fix mirrors the BUG-04-013 explicit-tag pattern from `option_result.rs` but uses `niche_is_sentinel` for variant detection (no separate tag field exists in niche encoding). When `NICHE_CODEGEN_READY` is later flipped to `true`, the existing niche spec tests will validate the fix end-to-end.

**Success Criteria:**
- [x] `emit_option_niche` for `"unwrap"`: computes `is_some` via `niche_is_sentinel`, calls `emit_unwrap_branch`, extracts payload, calls `inc_value_rc` unconditionally
- [x] `emit_option_niche` for `"expect"`: computes `is_some`, calls `emit_expect_branch` with the user msg, extracts payload, calls `inc_value_rc` unconditionally
- [x] `emit_option_niche` for `"unwrap_or"`: already has tag check (preserves), adds conditional `inc_value_rc` via cond_br + merge blocks (only when Some)
- [x] `emit_result_niche` for `"unwrap"`: computes `is_ok` via niche check + `niche_variant_idx`, calls `emit_unwrap_branch`, extracts payload, calls `inc_value_rc` with `ok_ty`
- [x] `emit_result_niche` for `"unwrap_err"`: computes `is_err` (inverse of `is_ok`), calls `emit_unwrap_branch`, extracts payload, calls `inc_value_rc` with `err_ty`
- [x] `emit_result_niche` for `"unwrap_or"`: tag check + select + conditional `inc_value_rc` with `ok_ty`
- [x] `emit_result_niche` for `"expect"`: tag check + `emit_expect_branch` + extract + `inc_value_rc` with `ok_ty`
- [x] `emit_result_niche` for `"expect_err"`: tag check + `emit_expect_branch` + extract + `inc_value_rc` with `err_ty`
- [x] Rust unit test `niche_unwrap_emits_panic_branch_and_rc_retain` in `option_result_helpers/tests.rs` constructs minimal `ArcIrEmitter` (per the `drop_fn_trivial_generates_rc_free` pattern in `arc_emitter/tests.rs`), invokes each fixed helper with a synthetic `TagEncoding::new(EnumTag::Niche { ... }, 2)`, and asserts the printed LLVM IR contains both `ori_panic` and `ori_str_rc_inc`
- [x] New niche spec tests `tests/spec/types/enum/niche/option_unwrap.ori` and `result_unwrap.ori` cover `Some(s).unwrap()`, `Some(s).expect("msg")`, `Ok(v).unwrap()`, `Err(e).unwrap_err()`, `Ok(v).unwrap_or(default)`, `Err(e).unwrap_or(default)` and pass via the interpreter (LLVM gate-blocked behavioral validation rides on §07.2 NICHE_CODEGEN_READY)
- [x] `cargo test -p ori_llvm` green; `timeout 150 ./test-all.sh` green
- [x] BUG-04-019 entry marked `- [x]` with resolution note
- [x] `plans/repr-opt/section-07-enum-repr.md` §07.2 "Codegen consumers updated" list extended with `option_result_helpers.rs` entry

**Resolution (2026-04-07):** All success criteria met. The niche helpers now mirror the explicit-tag pattern from `option_result.rs`: variant predicate via `niche_is_sentinel` → `emit_unwrap_branch` / `emit_expect_branch` → payload extraction → unconditional (or conditional, for `unwrap_or`) `inc_value_rc`. The collapsed `Result.unwrap | unwrap_err | unwrap_or` arm was split into three semantically distinct match cases. `emit_result_niche` gained a `receiver_ty: Idx` parameter for `TypeInfo::Result { ok, err }` lookup. 9 unit tests in `option_result_helpers/tests.rs` assert each fixed helper emits both `ori_panic*` and `ori_str_rc_inc` substrings, plus a differentiation pin proving `Result.unwrap` and `Result.unwrap_err` produce non-identical IR. Section 07.2 codegen consumer list extended with the `option_result_helpers.rs` entry.

**Context:** BUG-04-013 fixed the analogous bugs in the explicit-tag paths of `option_result.rs` — adding `emit_unwrap_branch` panic guards plus unconditional `inc_value_rc` after the guard for `Option.unwrap`, `Result.unwrap`, `Result.unwrap_err`, etc. The sister niche-encoded helpers in `option_result_helpers.rs` were never updated to the same standard. Today the bug is latent because `NICHE_CODEGEN_READY = false` (`compiler/ori_repr/src/canonical/type_repr.rs:231`) — all Option/Result types use the explicit-tag layout, and `emit_option_niche` / `emit_result_niche` are dead code reached only via `option_result.rs:74-76` `if let Some(encoding) = self.get_niche_encoding(receiver_ty)`. When the §07.2 plan completes its remaining consumer updates and the gate is flipped, the niche helpers will start firing, and these bugs would cause: (a) `unwrap` on `None` would silently return garbage payload bytes instead of panicking (memory unsafety + spec violation), (b) `unwrap_err` returning the same payload as `unwrap` regardless of variant (Result semantics broken), (c) extracted payloads sharing inner heap data with the wrapper without an RC retain, leading to use-after-free when either is dropped first.

---

## 1. Root Cause Analysis

- **Symptom**: The niche-encoded helpers in `option_result_helpers.rs` emit LLVM IR that:
  - Returns the payload regardless of variant (no panic on `None`/`Err` for unwrap-style methods)
  - Conflates `Result.unwrap`, `unwrap_err`, `unwrap_or` into a single match arm `(line 124-126)` that doesn't differentiate semantics
  - Extracts payload via raw `extract_value` without `inc_value_rc`, so the payload's RC count is not bumped to reflect the new owning reference
- **Proximate cause**: The original niche helper implementation (per the §07.2 timeline, completed 2026-03-31) only handled the easy variant predicates (`is_some`/`is_none`/`is_ok`/`is_err`) and stubbed the unwrap-family arms with raw `extract_value` calls. The author marked the path as "complete" because it produced syntactically valid IR that passes type checks at the LLVM level. The semantic correctness gap was caught by tpr-review on 2026-04-02 (BUG-04-013 follow-up) but downgraded to medium because the path is gated off.
- **Root cause**: Niche encoding has no separate tag field — variant identity is encoded via a sentinel value in one of the payload fields. The `is_some`/`is_ok`/`is_err` cases already handle this correctly via `niche_is_sentinel(field, niche_value, label)`. The unwrap-family methods need to:
  1. Compute the same variant predicate (already-implemented logic)
  2. Use that predicate in `emit_unwrap_branch` / `emit_expect_branch` (panic on wrong variant)
  3. Extract the payload (already done)
  4. RC-retain the extracted payload via `inc_value_rc(payload, inner_ty, 1)` to reflect that the wrapper's RC and the extracted payload now both reference the inner heap data
  Steps 2 and 4 are missing from all unwrap-family arms in the niche helpers.
- **Blast radius**: Currently zero observable effect — `NICHE_CODEGEN_READY = false`, all Option/Result use the explicit-tag path in `option_result.rs` which already has the BUG-04-013 fix. When the gate flips (tracked in `plans/repr-opt/section-07-enum-repr.md` §07.2), all uses of `unwrap`/`unwrap_err`/`unwrap_or`/`expect`/`expect_err` on niche-encoded Option/Result would manifest these bugs.
- **Affected files**:
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs` — the buggy `emit_option_niche` (lines 18-84) and `emit_result_niche` (lines 87-137) need their unwrap-family arms rewritten to mirror the explicit-tag pattern
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers/tests.rs` — NEW file containing the unit test that exercises each fixed helper with a synthetic niche encoding
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs` — add `#[cfg(test)] mod tests;` declaration
  - `tests/spec/types/enum/niche/option_unwrap.ori` — NEW spec test for Option niche unwrap/expect
  - `tests/spec/types/enum/niche/result_unwrap.ori` — NEW spec test for Result niche unwrap/unwrap_err/unwrap_or/expect/expect_err
  - `plans/repr-opt/section-07-enum-repr.md` — extend §07.2 "Codegen consumers updated" list with `option_result_helpers.rs` referencing this fix
  - `plans/bug-tracker/section-04-codegen-llvm.md` — mark BUG-04-019 as `[x]` with resolution note

**Reference implementations** (the existing fix pattern):
- **BUG-04-013** (`compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs:94-110, 142-153, 204-241, 397-438`): The explicit-tag versions of `Option.unwrap`, `Option.expect`, `Result.unwrap`, `Result.unwrap_err`, `Result.expect`, `Result.expect_err` all follow the pattern: compute variant predicate via `icmp_eq` on the tag field → call `emit_unwrap_branch`/`emit_expect_branch` → extract payload via `extract_tagged_union_payload` (or `extract_value` for Option) → call `inc_value_rc(payload, inner_ty, 1)` unconditionally after the guard. The `unwrap_or` cases use a conditional cond_br/merge sequence to RC-retain only on the matching variant. My fix mirrors this exact shape, substituting `niche_is_sentinel`-based predicate construction for the explicit-tag `icmp_eq`.

---

## 2. TDD — Test Matrix

Write ALL tests BEFORE the fix. Verify they fail against current code.

### Exact failing case
- [ ] Rust unit test: invoke `emit_option_niche("unwrap", receiver, args, opt_str_ty, &niche_encoding)` and assert IR contains `ori_panic` AND `ori_str_rc_inc` (current code emits NEITHER — both assertions fail)

### Cross-method coverage (per-method correctness)
- [ ] `emit_option_niche` `"unwrap"` — IR contains `ori_panic_cstr` + `ori_str_rc_inc`
- [ ] `emit_option_niche` `"expect"` — IR contains `ori_panic` + `ori_str_rc_inc`
- [ ] `emit_option_niche` `"unwrap_or"` — IR contains `ori_str_rc_inc` (no panic; conditional retain via cond_br/merge)
- [ ] `emit_result_niche` `"unwrap"` — IR contains `ori_panic_cstr` + `ori_str_rc_inc`
- [ ] `emit_result_niche` `"unwrap_err"` — IR contains `ori_panic_cstr` + `ori_str_rc_inc`
- [ ] `emit_result_niche` `"unwrap_or"` — IR contains `ori_str_rc_inc` (no panic; conditional retain)
- [ ] `emit_result_niche` `"expect"` — IR contains `ori_panic` + `ori_str_rc_inc`
- [ ] `emit_result_niche` `"expect_err"` — IR contains `ori_panic` + `ori_str_rc_inc`

### Cross-type coverage (the fix is type-dependent through `inc_value_rc`)
- [ ] Option<str> niche unwrap — emits `ori_str_rc_inc` (str-specific RC inc)
- [ ] Result<str, error> niche unwrap_err — emits the appropriate RC inc for the error type

### Spec tests (interpreter today, LLVM when gate flips)
- [ ] `tests/spec/types/enum/niche/option_unwrap.ori` — `Some("hello").unwrap() == "hello"`, `Some("x").expect("must be Some") == "x"`, repeated unwrap pattern across a loop (RC correctness)
- [ ] `tests/spec/types/enum/niche/result_unwrap.ori` — `Ok("ok").unwrap() == "ok"`, `Err("err").unwrap_err() == "err"`, `Ok("a").unwrap_or("b") == "a"`, `Err("x").unwrap_or("default") == "default"`, `Ok("v").expect("msg") == "v"`, `Err("e").expect_err("msg") == "e"`

### Negative pin
- [ ] Spec test `option_unwrap.ori` includes `#fail("called \`Option.unwrap()\` on a \`None\` value")` for `(None : Option<str>).unwrap()` — pins the panic message
- [ ] Spec test `result_unwrap.ori` includes `#fail("called \`Result.unwrap()\` on an \`Err\` value")` for `(Err("x") : Result<str, str>).unwrap()`

### Semantic pin
- [ ] The Rust unit test asserts `ori_panic*` substring presence — fails immediately if anyone reverts `emit_unwrap_branch` / `emit_expect_branch` calls
- [ ] The Rust unit test asserts `ori_str_rc_inc` substring presence — fails immediately if anyone removes `inc_value_rc`
- [ ] The Rust unit test asserts the IR for `Result.unwrap` and `Result.unwrap_err` are NOT identical (current bug collapses them) — proves semantic differentiation

### Verify tests fail before fix
- [ ] Run the unit tests against current code → all assertions fail (no panic, no RC inc)
- [ ] Run new spec tests against current code → pass via interpreter (interpreter doesn't go through these helpers); LLVM path uses explicit tag (already correct via BUG-04-013), so they pass via LLVM today too. The spec tests are FUTURE regression guards activated by the gate flip.

---

## 3. Implementation

- [x] **Rewrite `emit_option_niche` arms** in `option_result_helpers.rs`:
  - `"unwrap"`: extract `is_some` via niche_is_sentinel + select pattern (mirrors line 36 logic), call `emit_unwrap_branch(is_some, "called \`Option.unwrap()\` on a \`None\` value", "opt_unwrap")?`, then `extract_value(receiver, 0, "opt.payload")`, then `inc_value_rc(payload, inner_ty, 1)`. Inner type via `pool.option_inner(pool.resolve_fully(receiver_ty))`.
  - `"expect"` (line 56-66): preserve existing `is_some` computation and `emit_expect_branch` call, but add `inc_value_rc(payload, inner_ty, 1)` after the payload extraction.
  - `"unwrap_or"` (line 45-55): preserve tag check + select, but add a conditional `inc_value_rc` block (cond_br is_some → inc_bb / merge_bb pattern from `option_result.rs:125-135`).

- [x] **Rewrite `emit_result_niche` arms**:
  - Split the collapsed `"unwrap" | "unwrap_err" | "unwrap_or"` arm (line 124) into three separate arms.
  - `"unwrap"`: compute `is_ok` (depends on `niche_variant_idx`, mirrors line 98-110), call `emit_unwrap_branch(is_ok, "called \`Result.unwrap()\` on an \`Err\` value", "res_unwrap")?`, extract via `extract_value(receiver, 0, "res.payload")`, get `ok_ty` from `TypeInfo::Result { ok, .. }`, call `inc_value_rc(payload, ok_ty, 1)`.
  - `"unwrap_err"`: compute `is_err` (inverse via niche pattern, mirrors line 111-122), call `emit_unwrap_branch(is_err, "called \`Result.unwrap_err()\` on an \`Ok\` value", "res_unwrap_err")?`, extract, get `err_ty`, call `inc_value_rc(payload, err_ty, 1)`.
  - `"unwrap_or"`: compute `is_ok`, conditional inc via cond_br/merge, select(is_ok, payload, default).
  - `"expect"`: compute `is_ok`, `emit_expect_branch(is_ok, msg, "res_expect")?`, extract, retain via `inc_value_rc(payload, ok_ty, 1)`.
  - `"expect_err"`: compute `is_err`, `emit_expect_branch(is_err, msg, "res_expect_err")?`, extract, retain via `inc_value_rc(payload, err_ty, 1)`.

  Sketch (Option.unwrap):
  ```rust
  "unwrap" => {
      let field = self.builder.extract_value(receiver, niche_idx, "opt.niche")?;
      let is_niche = self.niche_is_sentinel(field, niche_value, "is_niche");
      let t = self.builder.const_bool(true);
      let f = self.builder.const_bool(false);
      let is_some = self.builder.select(is_niche, f, t, "is_some");
      self.emit_unwrap_branch(
          is_some,
          "called `Option.unwrap()` on a `None` value",
          "opt_unwrap_niche",
      )?;
      let payload = self.builder.extract_value(receiver, 0, "opt.payload")?;
      let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
      self.inc_value_rc(payload, inner_ty, 1);
      Some(payload)
  }
  ```

- [x] **Receiver type for Result niche**: `emit_result_niche` doesn't currently take `receiver_ty` as a parameter (only `encoding`). Update its signature to `emit_result_niche(method, receiver, arg_vals, receiver_ty, encoding)` so the unwrap arms can look up `TypeInfo::Result { ok, err }` from the type info store. Update the single call site in `option_result.rs:186` accordingly.

- [x] **Add `#[cfg(test)] mod tests;` declaration** at the bottom of `option_result_helpers.rs`.

- [x] **Create `option_result_helpers/tests.rs`** following the `arc_emitter/tests.rs` `drop_fn_trivial_generates_rc_free` pattern. The test:
  1. Sets up `Pool` (`pool.option(Idx::STR)` for the receiver type)
  2. Creates LLVM `Context`, `SimpleCx`, `IrBuilder`, declares runtime
  3. Creates a host function with no params
  4. Constructs a synthetic `TagEncoding::new(EnumTag::Niche { field_index: 0, niche_value: 0, niche_variant_idx: 1 }, 2)` for Option (None=variant 1 is the niche)
  5. Allocates a synthetic receiver via `builder.const_zero_ty(opt_str_llvm_ty)` so `extract_value` calls work
  6. Calls `em.emit_option_niche("unwrap", receiver, &[receiver], opt_str_ty, &encoding)`
  7. Asserts `scx.llmod.print_to_string()` contains both `"ori_panic"` (any panic-family runtime call) and `"ori_str_rc_inc"`
  8. Repeats for Option `"expect"` (uses an arg_val for the message), `"unwrap_or"`
  9. For Result, builds `pool.result(Idx::STR, Idx::STR)` and a Niche encoding with `niche_variant_idx: 0` (Ok) or 1 (Err); tests `unwrap`, `unwrap_err`, `unwrap_or`, `expect`, `expect_err`
  10. Adds a differentiation assertion: the IR for `Result.unwrap` and `Result.unwrap_err` MUST differ (proves the collapsed-arm bug is fixed)

- [x] **Create spec tests** at `tests/spec/types/enum/niche/option_unwrap.ori` and `result_unwrap.ori` with the test bodies described in §2.

- [x] **Update §07.2 plan** in `plans/repr-opt/section-07-enum-repr.md` "Codegen consumers updated" list to add `option_result_helpers.rs — niche helpers for unwrap/unwrap_err/unwrap_or/expect/expect_err now have tag guards and RC retain (BUG-04-019)`.

- [x] **Update bug entry** in `plans/bug-tracker/section-04-codegen-llvm.md` to mark BUG-04-019 as `[x]` with resolution details.

---

## 4. Completion Checklist

- [x] All new tests pass unchanged after fix (no test modifications needed)
- [x] Matrix completeness verified — every method (unwrap, unwrap_err, unwrap_or, expect, expect_err) × wrapper (Option, Result) has a unit test cell
- [x] Debug AND release builds pass (`cargo b && cargo b --release`)
- [x] `cargo test -p ori_llvm` green
- [x] `timeout 150 ./test-all.sh` green — no regressions
- [x] `timeout 150 ./clippy-all.sh` green
- [ ] `/commit-push` — commit all changes before review (pending; will commit after TPR-07-016 fix is bundled)
- [x] Bug entry in `plans/bug-tracker/section-04-codegen-llvm.md` updated: `- [x]` with resolution details and a note that runtime LLVM verification rides on the §07.2 NICHE_CODEGEN_READY gate
- [x] Fix section frontmatter `status` updated to `complete`
- [x] `plans/repr-opt/section-07-enum-repr.md` §07.2 "Codegen consumers updated" list extended
- [ ] `/tpr-review` passed (medium severity: expected) — pending; bundled with TPR-07-016 review
- [ ] `/impl-hygiene-review` passed (medium severity: recommended) — pending; bundled with TPR-07-016 review
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** Running `cargo test -p ori_llvm option_result_helpers` produces a green test that exercises each of the 8 fixed niche helper method-cases and asserts each emits both a panic branch and an RC retain call. The IR for `Result.unwrap` and `Result.unwrap_err` differ (proves the collapsed-arm bug is fixed). The new `tests/spec/types/enum/niche/{option_unwrap,result_unwrap}.ori` spec tests pass via the interpreter. `timeout 150 ./test-all.sh` produces zero regressions. The bug entry is marked `[x]` with a clear note that the LLVM-side runtime parity check rides on the existing `<!-- blocked-by:NICHE_CODEGEN_READY gate -->` items in §07.2 (the same gate the rest of the niche-encoded codegen path waits on).
