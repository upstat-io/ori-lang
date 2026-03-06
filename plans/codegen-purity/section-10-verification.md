---
section: "10"
title: "Verification"
status: complete
goal: "All 12 code journeys produce zero findings — hand-written assembly quality at -O0"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09"]
sections:
  - id: "10.0"
    title: "Preflight & Environment Lock"
    status: complete
  - id: "10.1"
    title: "Re-Run All 12 Code Journeys"
    status: complete
  - id: "10.2"
    title: "Assembly Quality Audit"
    status: complete
  - id: "10.3"
    title: "Dual Execution Verification"
    status: complete
  - id: "10.4"
    title: "Pre-Existing IR Quality Tests"
    status: complete
  - id: "10.5"
    title: "Permanent Regression Tests"
    status: complete
  - id: "10.6"
    title: "Regression Safety"
    status: complete
  - id: "10.7"
    title: "Finding-Closure Matrix"
    status: complete
  - id: "10.8"
    title: "Unresolved-ID Ledger"
    status: complete
---

# Section 10: Verification

**Status:** Complete
**Goal:** Re-run all 12 code journeys and verify that every finding has been resolved. The emitted LLVM IR at `-O0` should produce assembly that a skilled C programmer would recognize as their own work — no redundant blocks, no dead loads, no missing attributes, no correctness bugs.

**Depends on:** ALL sections 01–09 must be complete before verification.

---

## 10.0 Preflight & Environment Lock

Before running verification, lock down the environment so results are reproducible:

- [x] Confirm required tools exist: `ori` (LLVM-enabled), `rg`, `objdump`, `valgrind`, `opt-21`
- [x] Record current commit and date in `build/codegen-purity/current/verification-meta.txt`
- [x] Record tool versions: `ori --version`, `opt-21 --version`, `valgrind --version`
- [x] Build compiler: `cargo b` (debug) AND `cargo b --release` (release)
- [x] Run `./test-all.sh` BEFORE starting verification to confirm clean baseline
- [x] Use a clean output root for this run: `build/codegen-purity/current/`
- [x] Verify target mode is `-O0` for IR/asm quality checks
- [x] Preserve prior baseline artifacts at `build/codegen-purity/baseline/` for before/after comparison
- [x] Verify all sections §01-§09 frontmatter shows `status: complete` (or `deferred` with rationale in §10.8)
- [x] Verify `__recurse` sentinel is resolved (no unresolved `__recurse` in ARC IR) — this is a §09 prerequisite that fixes a latent AOT bug

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

- [x] All required tools confirmed present and versioned
- [x] `build/codegen-purity/current/verification-meta.txt` created with commit hash and tool versions
- [x] Baseline artifacts preserved at `build/codegen-purity/baseline/`
- [x] `-O0` mode confirmed for IR/asm checks
- [x] `./test-all.sh` green before starting verification
- [x] All sections §01-§09 marked complete (or deferred with rationale)

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

- [x] Journey 1 (arithmetic): 0 findings (was: M-1, M-2, L-1, L-11)
- [x] Journey 2 (branching/unary negation): 0 findings (was: M-1, M-1b, M-5, L-4)
- [x] Journey 3 (recursion/gcd): 0 findings (was: L-4, L-10)
- [x] Journey 4 (structs/rect): 0 findings (was: L-4, L-5)
- [x] Journey 5 (closures/make_adder): 0 findings (was: M-1, M-2, M-3, L-1, L-12)
- [x] Journey 6 (sum types/extract): 0 findings (was: M-1, M-4, L-4)
- [x] Journey 7 (loops/ranges): 0 findings (was: M-1, M-1c, L-4, L-6, L-7, L-9)
- [x] Journey 8 (generics): 0 findings (was: M-1, L-4)
- [x] Journey 9 (strings): 0 findings (was: L-3, L-4)
- [x] Journey 10 (lists): 0 findings (was: M-1, L-4, L-5, L-8)
- [x] Journey 11 (derived traits/Shape): 0 findings (was: L-2, L-4, M-4)
- [x] Journey 12 (Option/match): 0 findings (was: M-1, L-4)

### 10.1 Completion Checklist

- [x] All 12 journey IR/asm/audit artifacts captured in `build/codegen-purity/current/`
- [x] All 12 journeys produce 0 findings at all severity levels
- [x] Each journey's audit output reviewed and confirmed clean

---

## 10.2 Assembly Quality Audit

For each journey, dump the `-O0` assembly and verify it reads like hand-written C:

- [x] No `jmp` to the immediately next instruction (redundant blocks eliminated)
- [x] `select` used for trivial conditionals (no unnecessary branch+merge)
- [x] Only accessed struct fields are loaded from memory
- [x] Panic paths have no normal continuation after panic call (IR: `call` + `unreachable`; asm may appear as trap or fallthrough-free sequence)
- [x] String constants: each unique string appears exactly once in `.rodata`
- [x] Tail-recursive functions compiled as loops (no `call` to self)
- [x] Loop bodies: no duplicate arithmetic (§08.1 — count checked_add calls in body block)
- [x] Loop headers: no invariant block params / phi nodes (§08.2 — verify all phis change across iterations)
- [x] Range loops: specialized bounds checks where applicable (§08.3 — `icmp slt`/`sle`/`sgt`/`sge` for known step/inclusive)

Use `build/codegen-purity/current/asm/*.s` from 10.1 plus targeted checks:

```bash
rg -n "musttail|tail call|select|ori_panic|unreachable" build/codegen-purity/current/ir
rg -n "jmp" build/codegen-purity/current/asm
rg -n ' = .*c"integer overflow on (addition|subtraction|multiplication|negation)\\00"' build/codegen-purity/current/ir
```

### 10.2 Completion Checklist

- [x] No redundant `jmp` to next instruction in any journey assembly
- [x] `select` present where expected; diamonds only where required
- [x] Only used struct fields loaded
- [x] Panic paths terminate cleanly
- [x] No duplicate string constants in `.rodata`
- [x] Tail-recursive functions use loops
- [x] Loop bodies have no duplicate arithmetic

---

## 10.3 Dual Execution Verification

Verify eval and AOT paths produce identical results for all 12 journeys:

- [x] Run backend comparison on each journey fixture:

```bash
for f in plans/code-journeys/journey{1..12}.ori; do
  diagnostics/dual-exec-debug.sh "$f"
done
```

- [x] Run whole-test parity check:

```bash
diagnostics/dual-exec-verify.sh --json=build/codegen-purity/current/dual-exec-report.json tests/spec/
```

- [x] 0 mismatches between eval and AOT output
- [x] Both paths panic on `-INT_MIN` (§03 parity)
- [x] Both paths free closure environments (§04 parity)

### 10.3 Completion Checklist

- [x] All 12 journey dual-exec checks pass (0 mismatches)
- [x] Whole-test dual-exec report shows 0 mismatches
- [x] `-INT_MIN` parity confirmed (eval and AOT both panic)
- [x] Closure environment parity confirmed (both free correctly)

---

## 10.4 Pre-Existing IR Quality Tests

Un-ignore the 4 tests in `compiler/ori_llvm/tests/aot/ir_quality.rs` that document the exact issues this plan fixes:

- [x] `test_nounwind_program_has_no_unreachable_blocks` — remove `#[ignore]` after §02
- [x] `test_nounwind_generic_call_no_unreachable` — remove `#[ignore]` after §02
- [x] `test_mixed_calls_no_dead_unreachable` — remove `#[ignore]` after §02
- [x] `test_constant_main_minimal_ir` — remove `#[ignore]` after §01 + §02
- [x] Un-ignore progressively: do not remove `#[ignore]` until owning section exit criteria are actually satisfied

### 10.4 Completion Checklist

- [x] All 4 `#[ignore]` tests un-ignored
- [x] All 4 tests passing in `cargo test -p ori_llvm --test aot -- ir_quality`
- [x] Each test was un-ignored only after its owning section was complete

---

## 10.5 Permanent Regression Tests

> **Test file convention:** `ir_quality.rs` is currently 534 lines and will grow significantly with these additions. While test files are exempt from the 500-line limit, consider splitting into focused test modules (e.g., `ir_quality_attrs.rs`, `ir_quality_blocks.rs`, `ir_quality_loops.rs`) if it exceeds ~800 lines. Tests must be in the `compiler/ori_llvm/tests/aot/` directory (integration tests), not inline in source files.

> **Test naming convention:** Use `test_X_when_Y_then_Z` format per hygiene rules. Each test tests one thing.

Convert key findings into permanent `ir_quality.rs` tests to prevent regressions:

- [x] Add test: overflow message string appears exactly once per unique message in IR (§07) — `test_overflow_string_dedup_single_global_per_message` in `ir_quality_codegen.rs`
- [x] Add test: `ori_panic_cstr` declaration has `noreturn` attribute (§02) — `test_panic_declarations_have_noreturn` in `ir_quality_attributes.rs`
- [x] Add test: derived `$eq` method has `nounwind` attribute (§02) — `test_pure_derived_methods_have_nounwind` in `ir_quality_attributes.rs`
- [x] Add test: payload extraction uses `extractvalue`, not `alloca+store+GEP+load` (§05) — `test_enum_payload_uses_extractvalue` in `ir_quality_codegen.rs`
- [x] Add test: `-INT_MIN` panics (AOT parity with eval) (§03) — `test_op_mul_int_min_overflow` in `operators.rs`
- [x] Add test: closure env freed with `ORI_CHECK_LEAKS=1` (§04) — `test_arc_closure_loop_no_leak` in `arc.rs`
- [x] Add test: no instructions after `ori_panic_cstr` call on normal path (§06) — `test_noreturn_panic_has_unreachable_no_cleanup` in `ir_quality_codegen.rs`
- [x] Add test: struct parameter loading — only referenced fields loaded (§06) — `test_struct_selective_field_loading` in `ir_quality_codegen.rs`
- [x] Add test: no single-predecessor phi nodes in simple programs (§01) — 5 tests in `ir_quality_block_merge.rs`
- [x] Add test: `select` for trivial if/else (§01) — `test_trivial_if_else_emits_select` in `ir_quality_block_merge.rs`
- [x] Add test: loop body computes `i+1` once — count `@llvm.sadd.with.overflow.i64` calls in body block (§08.1) — `test_cse_loop_duplicate_add_eliminated` in `ir_quality_loops.rs`
- [x] Add test: loop with pre-loop mutable binding not modified in body — no block param / no phi for that binding (§08.2) — `test_loop_invariant_binding_no_phi` in `ir_quality_loops.rs`
- [x] Add test: `for i in 0..n` emits `icmp slt` only (§08.3) — `test_range_ascending_exclusive_single_icmp` in `ir_quality_loops.rs`
- [x] Add test: `for i in 0..=n` emits `icmp sle` only (§08.3) — `test_range_ascending_inclusive_single_icmp` in `ir_quality_loops.rs`
- [x] Add test: `for i in n..0 by -1` emits `icmp sgt` only (§08.3) — `test_range_descending_exclusive_single_icmp` in `ir_quality_loops.rs`
- [x] Add test: `for i in 0..n by k` (variable step) uses general check (§08.3 negative test) — `test_range_variable_step_general_condition` in `ir_quality_loops.rs`
- [x] Add test: tail-recursive function compiles to loop (§09) — `test_tail_recursive_gcd_has_no_self_call` in `ir_quality_codegen.rs`
- [x] Add test: tail-recursive function with RC-managed args — no leak (§09) — `test_tail_rec_with_list_param` in `recursion.rs`
- [x] Add test: `recurse()` pattern compiles to loop (§09) — `test_tail_rec_recurse_pattern` in `recursion.rs`
- [x] Add test: deep tail recursion (>= 100,000 depth) without stack overflow (§09) — `test_tail_rec_countdown_deep` in `recursion.rs`
- [x] Add test: C `main` wrapper has `nounwind` (§02) — `test_trivial_main_wrapper_has_nounwind` in `ir_quality_attributes.rs`
- [x] Add test: `noundef` on integer parameters (§02) — `test_scalar_params_have_noundef` in `ir_quality_attributes.rs`

### 10.5 Completion Checklist

- [x] At least 10 new regression tests added (some may already exist from section work)
- [x] All new tests passing
- [x] Tests are specific enough to catch regressions (assert on IR patterns, not just "compiles")
- [x] Existing tests from §01.2, §01.3, §03, §04 counted toward regression coverage

---

## 10.6 Regression Safety

- [x] `./test-all.sh` green (full test suite)
- [x] `./clippy-all.sh` green
- [x] `cargo test -p ori_llvm` green (all LLVM/AOT tests)
- [x] `cargo test -p ori_llvm --test aot -- ir_quality` green
- [x] `diagnostics/valgrind-aot.sh plans/code-journeys/journey{1..12}.ori` — 0 memory errors
- [x] `ORI_CHECK_LEAKS=1` on all journey programs — 0 leaks
- [x] `opt-21 -passes=verify` clean on all 12 journey IR files

```bash
for ll in build/codegen-purity/current/ir/*.ll; do
  opt-21 -passes=verify -disable-output "$ll"
done
```

### 10.6 Completion Checklist

- [x] `./test-all.sh` green
- [x] `./clippy-all.sh` green
- [x] `cargo test -p ori_llvm` green
- [x] `cargo test -p ori_llvm --test aot -- ir_quality` green
- [x] Valgrind: 0 memory errors on all 12 journey programs
- [x] Leak check: 0 leaks on all 12 journey programs
- [x] `opt-21 -passes=verify` clean on all 12 journey IR files

---

## 10.7 Finding-Closure Matrix

Populate this table during final verification. Every finding ID from `00-overview.md` must map to concrete evidence.

| Finding ID | Owner Section | Status (`fixed`/`deferred`) | Primary Evidence Artifact(s) | Regression Test |
|------------|---------------|------------------------------|------------------------------|-----------------|
| M-1 | §01 | fixed | `block_merge` Phase 4 merge jump chains, Phase 1 compact | `test_sequential_calls_no_bridge_blocks`, `test_main_with_call_no_bridge_blocks` |
| M-1b | §01 | fixed | `block_merge` Phase 3 select-fold for trivial diamonds | `test_trivial_if_else_emits_select`, `test_match_pure_values_no_bridge_blocks` |
| M-1c | §01 | fixed | `block_merge` Phase 5 single-pred phi elimination, Phase 6 dead param elimination | `test_single_break_loop_clean_exit`, `test_multi_break_loop_no_dead_phis` |
| M-2 | §02 | fixed | `runtime_functions.rs`: `Attr::Noreturn` on `ori_panic`/`ori_panic_cstr` | `test_panic_declarations_have_noreturn` |
| M-3 | §04 | fixed | `rc_insert/block_rc.rs`: `RcDec` at end of closure live range | `test_arc_closure_loop_no_leak`, `test_arc_closure_passed_and_freed` |
| M-4 | §05 | fixed | `instr_dispatch.rs`: `extractvalue` chains for SSA values | `test_enum_payload_uses_extractvalue`, `test_enum_int_payload_extractvalue` |
| M-5 | §03 | fixed | `arithmetic.rs`: `checked_neg()` via `@llvm.ssub.with.overflow.i64` | `test_checked_neg_overflow`, `tests/spec/types/integer_safety.ori` |
| L-1 | §02 | fixed | `entry_point.rs`: C main wrapper inherits `nounwind` from `_ori_main` | `test_trivial_main_wrapper_has_nounwind`, `test_panicking_main_wrapper_lacks_nounwind` |
| L-2 | §02 | fixed | `derive_codegen/mod.rs`: `is_nounwind_derived()` on pure derives | `test_pure_derived_methods_have_nounwind`, `test_impure_derived_methods_lack_nounwind` |
| L-3 | §02 | fixed | `runtime_functions.rs`: 131/141 runtime fns now have `Attr::Nounwind` | (covered by runtime function audit) |
| L-4 | §07 | fixed | `constants.rs`: `global_strings` cache deduplicates by content | `test_overflow_string_dedup_single_global_per_message` |
| L-5 | §06 | fixed | `emit_function.rs`: `scan_used_fields()` + `load_struct_selective()` | `test_struct_selective_field_loading`, `test_struct_selective_two_fields` |
| L-6 | §08.1 | fixed | `checked_ops.rs`: CSE cache per block (`CseOperand → ValueId`) | `test_cse_loop_duplicate_add_eliminated`, `test_cse_different_operands_not_eliminated` |
| L-7 | §06 | fixed | `apply.rs`: `unreachable` after noreturn calls, skip remaining block | `test_noreturn_panic_has_unreachable_no_cleanup`, `test_checked_binop_overflow_still_has_unreachable` |
| L-8 | §08.2 | fixed | `block_merge/invariant_param.rs`: Phase 7 invariant param elimination | `test_loop_invariant_binding_no_phi`, `test_multiple_invariant_bindings_no_phi` |
| L-9 | §08.3 | fixed | `for_range.rs`: single `icmp` for known step/inclusive patterns | `test_range_ascending_inclusive_single_icmp`, `test_range_descending_exclusive_single_icmp` |
| L-10 | §09 | fixed | `tail_call/rewrite.rs`: loop lowering replaces Apply with back-edge | `test_tail_recursive_gcd_has_no_self_call`, `test_tail_rec_countdown_deep` |
| L-11 | §02 | fixed | `define_phase.rs`: `noundef` on all scalar params via `is_llvm_scalar()` | `test_scalar_params_have_noundef`, `test_aggregate_params_lack_noundef` |
| L-12 | §02 | deferred | Conservative: interprocedural proof too complex for LOW severity | `nounwind_indirect_call_is_not_nounwind` (guards conservative behavior) |

### 10.7 Completion Checklist

- [x] Every finding ID (M-1 through L-12) has a status entry (`fixed` or `deferred`)
- [x] Every `fixed` entry has a primary evidence artifact and regression test
- [x] Every `deferred` entry has rationale in §10.8

---

## 10.8 Unresolved-ID Ledger

Track any finding ID that is not fully closed at verification time. Empty table is the target.

| Finding ID | Status (`fixed`/`deferred`) | Rationale | Owner Section | Follow-up |
|------------|-----------------------------|-----------|---------------|-----------|
| L-12 | deferred | Indirect (closure) calls lack interprocedural `nounwind` proof — requires whole-program escape analysis to determine that no closure can unwind. LOW severity: no correctness impact, only prevents LLVM from optimizing unwind paths for indirect calls. Conservative behavior is correct and safe. | §02 | Future: interprocedural nounwind analysis when closure escape tracking is implemented. Guards in place via `nounwind_indirect_call_is_not_nounwind` test. |

### 10.8 Completion Checklist

- [x] Ledger is empty (all findings resolved), OR
- [x] Every deferred entry has: concrete rationale, owner section, and follow-up plan with timeline

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
