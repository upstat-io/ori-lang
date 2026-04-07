---
section: "06"
title: "Verification"
status: not-started
reviewed: false
goal: "Prove the bytecode VM is correct (identical to tree-walker) and fast (matches Python 3.12) with permanent regression guards"
inspired_by:
  - "Zig comptime/runtime dual execution verification"
  - "Ori dual-exec-verify.sh (interpreter vs LLVM parity)"
  - "Roc test_mono (compiler output verification)"
depends_on: ["01", "04", "05", "07"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Behavioral Equivalence"
    status: not-started
  - id: "06.2"
    title: "Performance Regression Suite"
    status: not-started
  - id: "06.3"
    title: "Stress Tests"
    status: not-started
  - id: "06.4"
    title: "Code Journey"
    status: not-started
  - id: "06.5"
    title: "Documentation and Cleanup"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
**Goal:** Prove the bytecode VM produces identical results to the tree-walking interpreter for all programs, and maintains performance within target thresholds. This section is the quality gate — nothing ships without it.

**Context:** The bytecode VM replaces the tree-walking interpreter as the default execution path. This is a high-risk change — any behavioral difference is a regression. Ori already has a dual-execution pattern (`dual-exec-verify.sh` for interpreter vs LLVM); this section extends it to interpreter vs bytecode VM.

**Depends on:** Sections 01 (benchmarks), 04 (bytecode compilation), 05 (VM), and 07 (Salsa integration). Sections 02-03 are optional tree-walker optimizations and are not prerequisites for verification.

---

## 06.1 Behavioral Equivalence

**File(s):** `diagnostics/bytecode-verify.sh` (new), test infrastructure

Verify that the bytecode VM produces identical runtime output to the tree-walker for every evaluation-bearing test, while preserving existing interpreter-vs-LLVM parity where applicable.

- [ ] **Create dual-execution script** `diagnostics/bytecode-verify.sh`:
  - Run each spec test through both the tree-walker and bytecode VM
  - Compare stdout, stderr, exit code
  - Report any mismatches with full diff
  - Support `--test-only`, `--main-only`, `--json` flags (same as `dual-exec-verify.sh`)
- [ ] **Run against all spec tests that execute the evaluator**: Zero mismatches required
- [ ] **Run against all run-pass tests** (including Rosetta tasks): Zero mismatches
- [ ] **Do not treat compile-fail as a bytecode-parity gate**: compile-fail coverage remains a frontend/compiler obligation. Keep those tests green, but do not count them as VM-vs-tree-walker evidence because no runtime execution occurs.
- [ ] **Verify full Salsa pipeline**: Run programs through `ori run --vm` (Section 07) and verify the result matches `ori run` (tree-walker). This tests the full pipeline: Salsa query -> parse -> type-check -> canonicalize -> compile bytecode -> VM execute -> ModuleEvalResult.
- [ ] **Verify ConstEval preservation**: Run programs with `$` constants and `#cfg` conditions. Verify the type checker's const-eval (which still uses the tree-walker) produces correct results when the runtime uses the VM. This is a cross-mode interaction test.
- [ ] **Verify TestRun mode**: Run `ori test --vm` on representative spec test files. Verify output capture, test result collection, and `only_attached` filtering all work correctly through the VM.
- [ ] **Preserve dual-backend confidence**: for programs that also run through LLVM, compare tree-walker, bytecode VM, and LLVM outputs to catch semantic drift relative to the existing production backend.
- [ ] **Matrix tests by feature category** (verified via `diagnostics/bytecode-verify.sh`):
  - Arithmetic and operators: int/float/bool/str x binary(+,-,*,/,%,**)/unary(-,!,~)/comparison(==,!=,<,<=,>,>=)/bitwise(&,|,^,<<,>>)
  - Control flow: if/match/for/while/loop/break/continue/labels x simple/nested/labeled x value-carrying (break value, continue value in yield)
  - Functions: single-clause/multi-clause/default-params/variadics/recursion/tail-recursion/memoized
  - Closures: capture 0/1/5 vars x flat/nested x called immediately/stored/passed as arg
  - Collections: list/map/set/tuple x creation/method-call/iteration/spread x empty/single/many
  - Pattern matching: literal/binding/wildcard/guard/nested/variant/struct x exhaustive/with-default
  - Error handling: `?` on Result/Option x try block x propagation across 1/2/3 call frames
  - Traits and method dispatch: inherent/trait/derived/extension/associated-function x user-type/builtin-type
  - Strings: interpolation with `\`{x}\`` / methods (split, trim, contains) / Unicode (multi-byte chars)
  - Module namespaces, associated functions, `FunctionExp` builtins (all 15 `FunctionExpKind` variants), and capability injection via `with...in`
  - Derived trait methods: Eq/Clone/Debug/Printable/Hashable/Comparable/Default x struct/enum/newtype
  - Newtype: construction `UserId(42)` and `.inner` access
  - `FormatWith`: template string formatting with spec (`{x:08x}`, `{y:>10.2f}`)
  - `Unsafe` blocks: transparent pass-through (same result as without unsafe wrapper)

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-06.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 06.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 06.2 Performance Regression Suite

**File(s):** `compiler/oric/benches/interpreter.rs` (extend from Section 01)

Establish hard performance gates that CI enforces.

- [ ] **Gate test: A(3,8) in <1s** — the primary performance gate. If this regresses, something is wrong.
- [ ] **Benchmark suite targets** (measured via Criterion, with Python 3.12 baselines recorded as reference numbers rather than hard-coded cross-machine CI thresholds):

  | Benchmark | Python 3.12 | Target (VM) | Tolerance |
  |-----------|-------------|-------------|-----------|
  | ackermann_3_5 | ~0.006s | <0.01s | 2x Python |
  | ackermann_3_8 | ~0.386s | <0.5s | 1.3x Python |
  | fibonacci_30 | TBD | TBD | 2x Python |
  | loop_sum_1m | TBD | TBD | 2x Python |
  | closure_map_10k | TBD | TBD | 3x Python |
  | pattern_match_1k | TBD | TBD | 3x Python |
  | method_dispatch_10k | TBD | TBD | 3x Python |

- [ ] **Measure Python baselines** for all benchmark programs on the dev machine and record as the target
- [ ] **Add regression detection** to benchmark script against recorded local baselines: >10% regression on the same host = failure
- [ ] **Record before/after for the entire plan**:
  - Phase 0 baseline (current tree-walker): 63µs/call
  - Phase 1 result (zero-alloc tree-walker): target ≤10µs/call only if Sections 02-03 were explicitly chosen in Section 01
  - If Sections 02-03 were skipped, record the Section 01 decision and treat Phase 0 -> Phase 2 as the relevant before/after comparison
  - Phase 2 result (bytecode VM): target ≤0.5µs/call

- [ ] **Subsection close-out (06.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-06.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 06.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 06.3 Stress Tests

**File(s):** `tests/benchmarks/stress/` (new directory)

Push the VM to its limits to find edge cases.

- [ ] **Deep recursion** (10,000+ frames): Verify clean stack overflow error, not Rust panic
- [ ] **Wide recursion** (function with 50+ parameters): Verify register allocation handles it
- [ ] **Many closures** (10,000 closures in a list): Verify upvalue management doesn't leak
- [ ] **Long-running iteration** (1M element list map/filter/fold): Verify iterator protocol holds
- [ ] **Nested pattern matching** (5+ levels deep): Verify bytecode dispatch chain works
- [ ] **Mixed execution** (bytecode calls built-in calls bytecode): Verify frame transitions
- [ ] **Matrix tests** (in `tests/benchmarks/stress/` and `compiler/ori_eval/src/bytecode/vm/tests.rs`):
  - **Dimensions**: stress category (deep recursion, wide params, many closures, long iteration, nested patterns, mixed execution) x scale (moderate: 1000, high: 10000, extreme: 100000+) x error path (normal completion, stack overflow, timeout)
  - **Semantic pin**: deep recursion at exactly MAX_FRAMES produces a clean `StackOverflow` error with correct frame count in backtrace
  - **Negative pin**: deep recursion at MAX_FRAMES+1 does NOT produce a Rust stack overflow (process does not segfault or abort)
  - **TDD ordering**: write the stress test harness and expected-error assertions FIRST, verify the extreme cases fail or produce wrong errors, then verify the VM handles them correctly

- [ ] **Subsection close-out (06.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-06.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 06.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 06.4 Code Journey

Run `/code-journey` to test the bytecode VM pipeline end-to-end with progressively complex Ori programs.

- [ ] Run `/code-journey` — journeys escalate until the VM breaks down
- [ ] All CRITICAL findings from journey results triaged (fixed or tracked)
- [ ] Tree-walker and bytecode VM produce identical results for all passing journeys
- [ ] Journey results archived in `plans/code-journeys/`

- [ ] **Subsection close-out (06.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-06.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 06.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 06.5 Documentation and Cleanup

- [ ] Update CLAUDE.md with new commands/paths:
  - Bytecode-related env vars (e.g., `ORI_DUMP_BYTECODE=1`, `ORI_USE_BYTECODE_VM=1`)
  - New CLI flags (`--vm`, `--dual`)
  - New benchmark commands
  - VM architecture notes
- [ ] Update `.claude/rules/eval.md` with bytecode VM architecture:
  - Add bytecode module file layout
  - Document dispatch loop location
  - Document the tree-walker/VM split (tree-walker = const-eval, VM = runtime)
- [ ] Tree-walker retirement is handled by Section 07.4. This section only verifies the cleanup is correct.
- [ ] Verify tree-walker preservation:
  - `ConstEval` mode (budget-limited, no I/O, deterministic) — used by the type checker for compile-time evaluation of `$name` constants and `#cfg` conditions
  - `PatternExecutor` trait implementation — used by `ori_patterns` for pattern system integration
  - Method dispatch chain (`MethodDispatcher` and resolvers) — shared between tree-walker and VM
- [ ] Plan annotation cleanup: remove all temporary references to this plan from source code
- [ ] Update memory files with bytecode VM performance characteristics

- [ ] **Subsection close-out (06.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-06.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 06.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] Behavioral equivalence: 0 mismatches across all 5,800+ spec tests (`bytecode-verify.sh --json` shows 100% match)
- [ ] Behavioral equivalence: 0 mismatches across all run-pass tests
- [ ] Performance gate: A(3,8) completes in <1s
- [ ] Performance gate: per-call cost ≤0.5µs (Criterion benchmark)
- [ ] Stress tests: all pass (deep recursion, wide params, many closures, long iteration)
- [ ] Code journey: no CRITICAL findings unaddressed
- [ ] All documentation updated
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 06` returns 0 annotations
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** The bytecode VM is ready to become the default execution path for `ori run` only after runtime parity is proven against the tree-walker, LLVM parity remains intact where applicable, and local benchmark gates show the VM is materially faster than the tree-walker. Python 3.12 remains the stretch comparison, not the only acceptable CI threshold.
