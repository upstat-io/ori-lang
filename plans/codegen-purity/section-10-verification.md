---
section: "10"
title: "Verification"
status: not-started
goal: "All 12 code journeys produce zero findings — hand-written assembly quality at -O0"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09"]
sections:
  - id: "10.0"
    title: "Preflight & Environment Lock"
    status: not-started
  - id: "10.1"
    title: "Re-Run All 12 Code Journeys"
    status: not-started
  - id: "10.2"
    title: "Assembly Quality Audit"
    status: not-started
  - id: "10.3"
    title: "Dual Execution Verification"
    status: not-started
  - id: "10.4"
    title: "Pre-Existing IR Quality Tests"
    status: not-started
  - id: "10.5"
    title: "Permanent Regression Tests"
    status: not-started
  - id: "10.6"
    title: "Regression Safety"
    status: not-started
  - id: "10.7"
    title: "Completion Checklist"
    status: not-started
  - id: "10.8"
    title: "Finding-Closure Matrix"
    status: not-started
  - id: "10.9"
    title: "Unresolved-ID Ledger"
    status: not-started
---

# Section 10: Verification

**Status:** Not Started
**Goal:** Re-run all 12 code journeys and verify that every finding has been resolved. The emitted LLVM IR at `-O0` should produce assembly that a skilled C programmer would recognize as their own work — no redundant blocks, no dead loads, no missing attributes, no correctness bugs.

**Depends on:** ALL sections 01–09 must be complete before verification.

---

## 10.0 Preflight & Environment Lock

Before running verification, lock down the environment so results are reproducible:

- [ ] Confirm required tools exist: `ori` (LLVM-enabled), `rg`, `objdump`, `valgrind`, `opt-21`
- [ ] Record current commit and date in `build/codegen-purity/current/verification-meta.txt`
- [ ] Use a clean output root for this run: `build/codegen-purity/current/`
- [ ] Verify target mode is `-O0` for IR/asm quality checks
- [ ] Preserve prior baseline artifacts at `build/codegen-purity/baseline/` for before/after comparison

```bash
set -euo pipefail
mkdir -p build/codegen-purity/current
{
  date -u
  git rev-parse HEAD
  command -v ori rg objdump valgrind opt-21
} > build/codegen-purity/current/verification-meta.txt
```

---

## 10.1 Re-Run All 12 Code Journeys

Capture fresh IR/asm/audit artifacts for all 12 journey fixtures.

```bash
set -euo pipefail
mkdir -p build/codegen-purity/current/{ir,asm,audit}
for f in plans/code-journeys/journey{1..12}.ori; do
  base="$(basename "$f" .ori)"
  diagnostics/ir-dump.sh --raw "$f" > "build/codegen-purity/current/ir/${base}.ll"
  diagnostics/disasm-ori.sh "$f" > "build/codegen-purity/current/asm/${base}.s"
  diagnostics/codegen-audit.sh --strict "$f" > "build/codegen-purity/current/audit/${base}.txt"
done
```

- [ ] Journey 1 (arithmetic): 0 findings (was: M-1, M-2, L-1, L-11)
- [ ] Journey 2 (branching/unary negation): 0 findings (was: M-1, M-1b, M-5, L-4)
- [ ] Journey 3 (recursion/gcd): 0 findings (was: L-4, L-10)
- [ ] Journey 4 (structs/rect): 0 findings (was: L-4, L-5)
- [ ] Journey 5 (closures/make_adder): 0 findings (was: M-1, M-2, M-3, L-1, L-12)
- [ ] Journey 6 (sum types/extract): 0 findings (was: M-1, M-4, L-4)
- [ ] Journey 7 (loops/ranges): 0 findings (was: M-1, M-1c, L-4, L-6, L-7, L-9)
- [ ] Journey 8 (generics): 0 findings (was: M-1, L-4)
- [ ] Journey 9 (strings): 0 findings (was: L-3, L-4)
- [ ] Journey 10 (lists): 0 findings (was: M-1, L-4, L-5, L-8)
- [ ] Journey 11 (derived traits/Shape): 0 findings (was: L-2, L-4, M-4)
- [ ] Journey 12 (Option/match): 0 findings (was: M-1, L-4)

---

## 10.2 Assembly Quality Audit

For each journey, dump the `-O0` assembly and verify it reads like hand-written C:

- [ ] No `jmp` to the immediately next instruction (redundant blocks eliminated)
- [ ] `select` used for trivial conditionals (no unnecessary branch+merge)
- [ ] Only accessed struct fields are loaded from memory
- [ ] Panic paths have no normal continuation after panic call (IR: `call` + `unreachable`; asm may appear as trap or fallthrough-free sequence)
- [ ] String constants: each unique string appears exactly once in `.rodata`
- [ ] Tail-recursive functions compiled as loops (no `call` to self)
- [ ] Loop bodies: no duplicate arithmetic, no invariant value reloads

Use `build/codegen-purity/current/asm/*.s` from 10.1 plus targeted checks:

```bash
rg -n "musttail|select|ori_panic|unreachable" build/codegen-purity/current/ir
rg -n "jmp" build/codegen-purity/current/asm
rg -n ' = .*c"integer overflow on (addition|subtraction|multiplication|negation)\\00"' build/codegen-purity/current/ir
```

---

## 10.3 Dual Execution Verification

Verify eval and AOT paths produce identical results for all 12 journeys:

- [ ] Run backend comparison on each journey fixture:

```bash
for f in plans/code-journeys/journey{1..12}.ori; do
  diagnostics/dual-exec-debug.sh "$f"
done
```

- [ ] Run whole-test parity check:

```bash
diagnostics/dual-exec-verify.sh --json=build/codegen-purity/current/dual-exec-report.json tests/spec/
```

- [ ] 0 mismatches between eval and AOT output
- [ ] Both paths panic on `-INT_MIN` (§03 parity)
- [ ] Both paths free closure environments (§04 parity)

---

## 10.4 Pre-Existing IR Quality Tests

Un-ignore the 4 tests in `compiler/ori_llvm/tests/aot/ir_quality.rs` that document the exact issues this plan fixes:

- [ ] `test_nounwind_program_has_no_unreachable_blocks` — remove `#[ignore]` after §02
- [ ] `test_nounwind_generic_call_no_unreachable` — remove `#[ignore]` after §02
- [ ] `test_mixed_calls_no_dead_unreachable` — remove `#[ignore]` after §02
- [ ] `test_constant_main_minimal_ir` — remove `#[ignore]` after §01 + §02
- [ ] Un-ignore progressively: do not remove `#[ignore]` until owning section exit criteria are actually satisfied

---

## 10.5 Permanent Regression Tests

Convert key findings into permanent `ir_quality.rs` tests to prevent regressions:

- [ ] Add test: overflow message string appears exactly once per unique message in IR (§07)
- [ ] Add test: `ori_panic_cstr` declaration has `noreturn` attribute (§02)
- [ ] Add test: derived `$eq` method has `nounwind` attribute (§02)
- [ ] Add test: payload extraction uses `extractvalue`, not `alloca+store+GEP+load` (§05)
- [ ] Add test: `-INT_MIN` panics (AOT parity with eval) (§03)
- [ ] Add test: closure env freed with `ORI_CHECK_LEAKS=1` (§04)

---

## 10.6 Regression Safety

- [ ] `./test-all.sh` green (full test suite)
- [ ] `./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green (all LLVM/AOT tests)
- [ ] `cargo test -p ori_llvm --test ir_quality` green
- [ ] `diagnostics/valgrind-aot.sh plans/code-journeys/journey{1..12}.ori` — 0 memory errors
- [ ] `ORI_CHECK_LEAKS=1` on all journey programs — 0 leaks
- [ ] `opt-21 -passes=verify` clean on all 12 journey IR files

```bash
for ll in build/codegen-purity/current/ir/*.ll; do
  opt-21 -passes=verify -disable-output "$ll"
done
```

---

## 10.7 Completion Checklist

- [ ] All 12 code journeys produce 0 findings
- [ ] Assembly quality audit passes for all 12 journeys
- [ ] Dual execution verification: 0 mismatches
- [ ] All 4 pre-existing `#[ignore]` tests in `ir_quality.rs` un-ignored and passing
- [ ] Permanent regression tests added for key findings (§10.5)
- [ ] Finding-closure matrix in `§10.8` fully populated (one evidence row per ID M-1..L-12)
- [ ] Full test suite green
- [ ] Valgrind and leak checks clean
- [ ] All sections 01–09 marked complete in their frontmatter
- [ ] Verification artifacts stored under `build/codegen-purity/current/`
- [ ] `10.9` unresolved-ID ledger is empty (or each entry has explicit defer rationale + owner)

---

## 10.8 Finding-Closure Matrix

Populate this table during final verification. Every finding ID from `00-overview.md` must map to concrete evidence.

| Finding ID | Owner Section | Status (`fixed`/`deferred`) | Primary Evidence Artifact(s) | Regression Test |
|------------|---------------|------------------------------|------------------------------|-----------------|
| M-1 | §01 | | | |
| M-1b | §01 | | | |
| M-1c | §01 | | | |
| M-2 | §02 | | | |
| M-3 | §04 | | | |
| M-4 | §05 | | | |
| M-5 | §03 | | | |
| L-1 | §02 | | | |
| L-2 | §02 | | | |
| L-3 | §02 | | | |
| L-4 | §07 | | | |
| L-5 | §06 | | | |
| L-6 | §08 | | | |
| L-7 | §06 | | | |
| L-8 | §08 | | | |
| L-9 | §08 | | | |
| L-10 | §09 | | | |
| L-11 | §02 | | | |
| L-12 | §02 | | | |

---

## 10.9 Unresolved-ID Ledger

Track any finding ID that is not fully closed at verification time. Empty table is the target.

| Finding ID | Status (`fixed`/`deferred`) | Rationale | Owner Section | Follow-up |
|------------|-----------------------------|-----------|---------------|-----------|
| _none_ | | | | |

**Exit Criteria:** Re-running all journey fixtures produces zero unresolved findings at any severity, or only explicitly deferred IDs in `10.9` with concrete rationale and follow-up. The assembly output at `-O0` is structurally clean and consistent with hand-written C quality. `./test-all.sh`, `./clippy-all.sh`, `cargo test -p ori_llvm --test ir_quality`, `diagnostics/dual-exec-verify.sh`, `diagnostics/valgrind-aot.sh`, and leak checks all pass. Pre-existing IR quality tests un-ignored and green.
