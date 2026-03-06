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
    title: "Finding-Closure Matrix"
    status: not-started
  - id: "10.8"
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
- [ ] Record tool versions: `ori --version`, `opt-21 --version`, `valgrind --version`
- [ ] Build compiler: `cargo b` (debug) AND `cargo b --release` (release)
- [ ] Run `./test-all.sh` BEFORE starting verification to confirm clean baseline
- [ ] Use a clean output root for this run: `build/codegen-purity/current/`
- [ ] Verify target mode is `-O0` for IR/asm quality checks
- [ ] Preserve prior baseline artifacts at `build/codegen-purity/baseline/` for before/after comparison
- [ ] Verify all sections §01-§09 frontmatter shows `status: complete` (or `deferred` with rationale in §10.8)
- [ ] Verify `__recurse` sentinel is resolved (no unresolved `__recurse` in ARC IR) — this is a §09 prerequisite that fixes a latent AOT bug

```bash
set -euo pipefail
mkdir -p build/codegen-purity/current
{
  echo "Date: $(date -u)"
  echo "Commit: $(git rev-parse HEAD)"
  echo "Branch: $(git branch --show-current)"
  echo "ori: $(ori --version 2>&1 || echo 'not found')"
  echo "opt-21: $(opt-21 --version 2>&1 | head -1 || echo 'not found')"
  echo "valgrind: $(valgrind --version 2>&1 || echo 'not found')"
  command -v ori rg objdump valgrind opt-21
} > build/codegen-purity/current/verification-meta.txt
```

### 10.0 Completion Checklist

- [ ] All required tools confirmed present and versioned
- [ ] `build/codegen-purity/current/verification-meta.txt` created with commit hash and tool versions
- [ ] Baseline artifacts preserved at `build/codegen-purity/baseline/`
- [ ] `-O0` mode confirmed for IR/asm checks
- [ ] `./test-all.sh` green before starting verification
- [ ] All sections §01-§09 marked complete (or deferred with rationale)

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

### 10.1 Completion Checklist

- [ ] All 12 journey IR/asm/audit artifacts captured in `build/codegen-purity/current/`
- [ ] All 12 journeys produce 0 findings at all severity levels
- [ ] Each journey's audit output reviewed and confirmed clean

---

## 10.2 Assembly Quality Audit

For each journey, dump the `-O0` assembly and verify it reads like hand-written C:

- [ ] No `jmp` to the immediately next instruction (redundant blocks eliminated)
- [ ] `select` used for trivial conditionals (no unnecessary branch+merge)
- [ ] Only accessed struct fields are loaded from memory
- [ ] Panic paths have no normal continuation after panic call (IR: `call` + `unreachable`; asm may appear as trap or fallthrough-free sequence)
- [ ] String constants: each unique string appears exactly once in `.rodata`
- [ ] Tail-recursive functions compiled as loops (no `call` to self)
- [ ] Loop bodies: no duplicate arithmetic (§08.1 — count checked_add calls in body block)
- [ ] Loop headers: no invariant block params / phi nodes (§08.2 — verify all phis change across iterations)
- [ ] Range loops: specialized bounds checks where applicable (§08.3 — `icmp slt`/`sle`/`sgt`/`sge` for known step/inclusive)

Use `build/codegen-purity/current/asm/*.s` from 10.1 plus targeted checks:

```bash
rg -n "musttail|tail call|select|ori_panic|unreachable" build/codegen-purity/current/ir
rg -n "jmp" build/codegen-purity/current/asm
rg -n ' = .*c"integer overflow on (addition|subtraction|multiplication|negation)\\00"' build/codegen-purity/current/ir
```

### 10.2 Completion Checklist

- [ ] No redundant `jmp` to next instruction in any journey assembly
- [ ] `select` present where expected; diamonds only where required
- [ ] Only used struct fields loaded
- [ ] Panic paths terminate cleanly
- [ ] No duplicate string constants in `.rodata`
- [ ] Tail-recursive functions use loops
- [ ] Loop bodies have no duplicate arithmetic

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

### 10.3 Completion Checklist

- [ ] All 12 journey dual-exec checks pass (0 mismatches)
- [ ] Whole-test dual-exec report shows 0 mismatches
- [ ] `-INT_MIN` parity confirmed (eval and AOT both panic)
- [ ] Closure environment parity confirmed (both free correctly)

---

## 10.4 Pre-Existing IR Quality Tests

Un-ignore the 4 tests in `compiler/ori_llvm/tests/aot/ir_quality.rs` that document the exact issues this plan fixes:

- [ ] `test_nounwind_program_has_no_unreachable_blocks` — remove `#[ignore]` after §02
- [ ] `test_nounwind_generic_call_no_unreachable` — remove `#[ignore]` after §02
- [ ] `test_mixed_calls_no_dead_unreachable` — remove `#[ignore]` after §02
- [ ] `test_constant_main_minimal_ir` — remove `#[ignore]` after §01 + §02
- [ ] Un-ignore progressively: do not remove `#[ignore]` until owning section exit criteria are actually satisfied

### 10.4 Completion Checklist

- [ ] All 4 `#[ignore]` tests un-ignored
- [ ] All 4 tests passing in `cargo test -p ori_llvm --test aot -- ir_quality`
- [ ] Each test was un-ignored only after its owning section was complete

---

## 10.5 Permanent Regression Tests

> **Test file convention:** `ir_quality.rs` is currently 534 lines and will grow significantly with these additions. While test files are exempt from the 500-line limit, consider splitting into focused test modules (e.g., `ir_quality_attrs.rs`, `ir_quality_blocks.rs`, `ir_quality_loops.rs`) if it exceeds ~800 lines. Tests must be in the `compiler/ori_llvm/tests/aot/` directory (integration tests), not inline in source files.

> **Test naming convention:** Use `test_X_when_Y_then_Z` format per hygiene rules. Each test tests one thing.

Convert key findings into permanent `ir_quality.rs` tests to prevent regressions:

- [ ] Add test: overflow message string appears exactly once per unique message in IR (§07)
- [ ] Add test: `ori_panic_cstr` declaration has `noreturn` attribute (§02)
- [ ] Add test: derived `$eq` method has `nounwind` attribute (§02)
- [ ] Add test: payload extraction uses `extractvalue`, not `alloca+store+GEP+load` (§05)
- [ ] Add test: `-INT_MIN` panics (AOT parity with eval) (§03) — **note: this already exists in `operators.rs` per §03 completion**
- [ ] Add test: closure env freed with `ORI_CHECK_LEAKS=1` (§04) — **note: this already exists in `arc.rs` per §04 completion**
- [ ] Add test: no instructions after `ori_panic_cstr` call on normal path (§06)
- [ ] Add test: struct parameter loading — only referenced fields loaded (§06)
- [ ] Add test: no single-predecessor phi nodes in simple programs (§01) — **note: 4 tests already exist per §01.3 completion**
- [ ] Add test: `select` for trivial if/else (§01) — **note: already exists per §01.2 completion**
- [ ] Add test: loop body computes `i+1` once — count `@llvm.sadd.with.overflow.i64` calls in body block (§08.1, if implemented)
- [ ] Add test: loop with pre-loop mutable binding not modified in body — no block param (ARC) / no phi (LLVM) for that binding (§08.2, if implemented)
- [ ] Add test: `for i in 0..n` emits `icmp slt` only (not 8-instruction general check) (§08.3, if implemented)
- [ ] Add test: `for i in 0..=n` emits `icmp sle` only (§08.3, if implemented)
- [ ] Add test: `for i in n..0 by -1` emits `icmp sgt` only (§08.3, if implemented)
- [ ] Add test: `for i in 0..n by k` (variable step) uses general check (§08.3 negative test, if implemented)
- [ ] Add test: tail-recursive function compiles to loop — no `call @_ori_gcd` in IR, has back-edge `br label %loop` (§09, if implemented)
- [ ] Add test: tail-recursive function with RC-managed args — no leak under `ORI_CHECK_LEAKS=1` (§09, if implemented)
- [ ] Add test: `recurse()` pattern compiles to loop (§09 — verify `__recurse` sentinel is resolved)
- [ ] Add test: deep tail recursion (>= 100,000 depth) runs without stack overflow (§09, if implemented)
- [ ] Add test: C `main` wrapper has `nounwind` (§02, if applicable)
- [ ] Add test: `noundef` on integer parameters (§02)

### 10.5 Completion Checklist

- [ ] At least 10 new regression tests added (some may already exist from section work)
- [ ] All new tests passing
- [ ] Tests are specific enough to catch regressions (assert on IR patterns, not just "compiles")
- [ ] Existing tests from §01.2, §01.3, §03, §04 counted toward regression coverage

---

## 10.6 Regression Safety

- [ ] `./test-all.sh` green (full test suite)
- [ ] `./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green (all LLVM/AOT tests)
- [ ] `cargo test -p ori_llvm --test aot -- ir_quality` green
- [ ] `diagnostics/valgrind-aot.sh plans/code-journeys/journey{1..12}.ori` — 0 memory errors
- [ ] `ORI_CHECK_LEAKS=1` on all journey programs — 0 leaks
- [ ] `opt-21 -passes=verify` clean on all 12 journey IR files

```bash
for ll in build/codegen-purity/current/ir/*.ll; do
  opt-21 -passes=verify -disable-output "$ll"
done
```

### 10.6 Completion Checklist

- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `cargo test -p ori_llvm --test aot -- ir_quality` green
- [ ] Valgrind: 0 memory errors on all 12 journey programs
- [ ] Leak check: 0 leaks on all 12 journey programs
- [ ] `opt-21 -passes=verify` clean on all 12 journey IR files

---

## 10.7 Finding-Closure Matrix

Populate this table during final verification. Every finding ID from `00-overview.md` must map to concrete evidence.

| Finding ID | Owner Section | Status (`fixed`/`deferred`) | Primary Evidence Artifact(s) | Regression Test |
|------------|---------------|------------------------------|------------------------------|-----------------|
| M-1 | §01 | fixed (01.1) | block_merge Phase 4 merge; IR quality tests | `test_sequential_calls_no_bridge_blocks`, `test_main_with_call_no_bridge_blocks` |
| M-1b | §01 | fixed (01.2) | block_merge Phase 3 select-fold | `test_trivial_if_else_emits_select`, `test_match_pure_values_no_bridge_blocks` |
| M-1c | §01.4 | | | |
| M-2 | §02 | | | |
| M-3 | §04 | fixed | `ORI_TRACE_RC=1` output, `ORI_CHECK_LEAKS=1` | `test_arc_closure_loop_no_leak`, `test_arc_closure_passed_and_freed` |
| M-4 | §05 | | | |
| M-5 | §03 | fixed | AOT operator tests, spec tests | `test_checked_neg_overflow`, `tests/spec/types/integer_safety.ori` |
| L-1 | §02 | | | |
| L-2 | §02 | | | |
| L-3 | §02 | | | |
| L-4 | §07 | | | |
| L-5 | §06 | | | |
| L-6 | §08.1 | | CSE eliminates duplicate `i+1` in loop body | `test_loop_cse_single_checked_add` (planned) |
| L-7 | §06 | | | |
| L-8 | §08.2 | | Invariant block param eliminated from loop header | `test_loop_invariant_param_eliminated` (planned) |
| L-9 | §08.3 | | Range specialization: `1..=n` emits `icmp sle` | `test_range_specialization_icmp_sle` (planned) |
| L-10 | §09 | | Loop lowering: no `call @_ori_gcd` in IR; `run_arc_pipeline` updated; `__recurse` resolved | `test_tail_recursive_gcd_emits_loop` (planned), `test_deep_tail_recursion_no_stack_overflow` (planned) |
| L-11 | §02 | | | |
| L-12 | §02 | | | |

### 10.7 Completion Checklist

- [ ] Every finding ID (M-1 through L-12) has a status entry (`fixed` or `deferred`)
- [ ] Every `fixed` entry has a primary evidence artifact and regression test
- [ ] Every `deferred` entry has rationale in §10.8

---

## 10.8 Unresolved-ID Ledger

Track any finding ID that is not fully closed at verification time. Empty table is the target.

| Finding ID | Status (`fixed`/`deferred`) | Rationale | Owner Section | Follow-up |
|------------|-----------------------------|-----------|---------------|-----------|
| _none_ | | | | |

### 10.8 Completion Checklist

- [ ] Ledger is empty (all findings resolved), OR
- [ ] Every deferred entry has: concrete rationale, owner section, and follow-up plan with timeline

---

## Realistic Verification Strategy

Not all sections may complete in a single cycle. The verification strategy should accommodate partial completion:

1. **Sections that are safe to defer:** §08 (Loop IR Quality) is a quality improvement with no correctness implications. §09 (Tail Call Optimization) is a **spec conformance requirement** (Annex E guarantees TCO, Clause 15.3 guarantees `recurse` compiles to O(1) stack) — deferral is acceptable only with explicit rationale in §10.8. Both can be deferred to a follow-up cycle if needed.
2. **Sections that MUST complete:** §02 (Function Attributes) — noreturn is needed by §06. §06 (Dead Code Pruning) depends on §02. §05 and §07 are independent and low-risk.
3. **Partial verification is acceptable** if the §10.8 unresolved ledger is fully populated with concrete follow-up plans.

If §08 and/or §09 are deferred:
- L-6, L-8, L-9 (§08 findings) go to §10.8 with "deferred to loop quality follow-up"
- L-10 (§09 finding) goes to §10.8 with "deferred to tail call follow-up"
- §10 verification can still close with these deferrals documented

## Section 10 Exit Criteria

Re-running all journey fixtures produces zero unresolved findings at any severity, or only explicitly deferred IDs in §10.8 with concrete rationale and follow-up. The assembly output at `-O0` is structurally clean and consistent with hand-written C quality. `./test-all.sh`, `./clippy-all.sh`, `cargo test -p ori_llvm --test aot -- ir_quality`, `diagnostics/dual-exec-verify.sh`, `diagnostics/valgrind-aot.sh`, and leak checks all pass. Pre-existing IR quality tests un-ignored and green. All sections 01–09 marked complete (or explicitly deferred in §10.8) in their frontmatter. Verification artifacts stored under `build/codegen-purity/current/`.
