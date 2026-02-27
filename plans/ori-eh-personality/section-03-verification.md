---
section: "03"
title: "Verification"
status: not-started
goal: "Full test suite green, AOT binaries contain only ori_eh_personality, no behavioral regressions"
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "Test Suite"
    status: not-started
  - id: "03.2"
    title: "Symbol Audit"
    status: not-started
  - id: "03.3"
    title: "Code Journey Regression"
    status: not-started
  - id: "03.4"
    title: "Memory Safety"
    status: not-started
  - id: "03.5"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Verification

**Status:** Not Started
**Goal:** Complete confidence that the personality function swap is correct: all existing tests pass, AOT binaries contain only `ori_eh_personality`, no behavioral regressions in eval or AOT paths, and no memory safety issues.

**Context:** The personality function is on the critical path of exception handling. If it's implemented incorrectly, panics will crash instead of unwinding, ARC cleanup won't run (causing leaks or use-after-free), and `catch()` will stop working. Testing must cover both the happy path (no exceptions) and the unwind path (panic, catch, ARC cleanup).

**Depends on:** Sections 01 and 02 (personality function exists and codegen references it).

---

## 03.1 Test Suite

**File(s):** N/A (running existing tests)

- [ ] `cargo build -p ori_rt` — runtime builds cleanly (both rlib and staticlib)
- [ ] `cargo bl` — compiler + runtime build succeeds (debug)
- [ ] `cargo blr` — compiler + runtime build succeeds (release)
- [ ] `./test-all.sh` — full test suite passes
  - This covers: Rust unit tests, Ori spec tests (JIT), LLVM unit tests, AOT integration tests
  - Key test categories that exercise exception handling:
    - `tests/spec/patterns/catch.ori` — catch/recover semantics
    - `tests/spec/functions/recursion.ori` — invoke/landingpad for recursive calls
    - `tests/spec/errors/` — panic behavior
    - `compiler/ori_llvm/tests/aot/` — AOT compilation + execution
- [ ] `./clippy-all.sh` — no new warnings
- [ ] `./fmt-all.sh` — formatting clean

---

## 03.2 Symbol Audit

**File(s):** N/A (inspection commands)

Verify that the symbol swap is complete at every level.

- [ ] **Source code audit** — zero references to `rust_eh_personality`:
  ```bash
  grep -r "rust_eh_personality" compiler/ library/ tests/
  # Expected: 0 matches
  ```

- [ ] **LLVM IR audit** — generated IR uses `ori_eh_personality`:
  ```bash
  # Pick a program that generates invoke/landingpad
  ORI_DEBUG_LLVM=1 ori check tests/spec/functions/recursion.ori 2>&1 \
    | grep "personality"
  # Expected: personality ptr @ori_eh_personality (one per function with invoke)
  # Expected: declare i32 @ori_eh_personality(i32) #N (one declaration)
  ```

- [ ] **Static library audit** — symbol exists in libori_rt.a:
  ```bash
  nm compiler/ori_rt/target/debug/libori_rt.a | grep "ori_eh_personality"
  # Expected: T ori_eh_personality (T = text/code section)

  nm compiler/ori_rt/target/debug/libori_rt.a | grep "rust_eh_personality"
  # Expected: still present (from Rust std embedded in staticlib — this is OK)
  # The point is that OUR code references ori_eh_personality, not rust_eh_personality
  ```

  Note: `rust_eh_personality` may still appear in the staticlib because Rust's std is embedded in it. This is expected — Rust's internal unwinding uses its own personality. What matters is that Ori's generated LLVM IR references `ori_eh_personality`.

- [ ] **AOT binary audit** — compiled Ori program uses ori_eh_personality:
  ```bash
  ori build tests/spec/functions/recursion.ori -o /tmp/test_personality
  nm /tmp/test_personality | grep "eh_personality"
  # Expected: ori_eh_personality in the symbol table
  # rust_eh_personality may also be present (from libori_rt.a's embedded Rust std)
  ```

---

## 03.3 Code Journey Regression

**File(s):** `plans/code-journeys/`

Re-run the code journeys that exercise exception handling to verify behavioral equivalence.

- [ ] **Journey 3** ("I am recursive") — the journey that first identified the `rust_eh_personality` dependency:
  - Eval = 61 (must still be correct)
  - AOT = 61 (must still be correct)
  - IR now shows `@ori_eh_personality` instead of `@rust_eh_personality`

- [ ] **Journey 9** ("I am a string") — first journey with ARC runtime functions:
  - Eval = 13, AOT = 13 (must still be correct)
  - ARC cleanup paths exercise landing pads

- [ ] **Journey 10** ("I am a list") — lists with ARC lifecycle:
  - Eval = 33, AOT = 33 (must still be correct)

- [ ] **Journey 12** ("I am an option") — uses `catch`-style landing pads:
  - Eval = 33 (must still be correct)
  - AOT = 144 (known bug C4 — must not get worse)

---

## 03.4 Memory Safety

- [ ] **Valgrind verification** — no new memory errors from personality function:
  ```bash
  ./scripts/valgrind-aot.sh
  # Expected: same results as before (no new errors)
  ```

- [ ] **Leak check** — ARC cleanup still works during unwind:
  ```bash
  ORI_CHECK_LEAKS=1 ori run tests/spec/patterns/catch.ori
  # Expected: exit code 0 (no leaks)
  ```

  If catch tests allocate RC-managed values that are cleaned up during unwind, the personality function must correctly route through cleanup landing pads for RC decrements to happen.

---

## 03.5 Completion Checklist

- [ ] `cargo build -p ori_rt` succeeds
- [ ] `cargo bl` and `cargo blr` succeed
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Zero `rust_eh_personality` references in compiler source
- [ ] LLVM IR shows `@ori_eh_personality`
- [ ] `nm libori_rt.a` shows `ori_eh_personality` symbol
- [ ] Code journeys J3, J9, J10, J12 produce same results as before
- [ ] `./scripts/valgrind-aot.sh` no new errors
- [ ] `ORI_CHECK_LEAKS=1` catch tests pass

**Exit Criteria:** `./test-all.sh` passes (all existing tests green, zero regressions). `grep -r "rust_eh_personality" compiler/` returns 0 matches. `ORI_DEBUG_LLVM=1` on any invoke-bearing program shows `@ori_eh_personality`. Code journeys J3/J9/J10/J12 produce identical eval and AOT results as before this change. `./scripts/valgrind-aot.sh` shows no new memory errors.
