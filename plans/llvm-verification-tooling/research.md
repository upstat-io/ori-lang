# LLVM & AIMS Verification Tooling — Pre-Plan Research

**Date**: 2026-04-09 (v2: expanded after dual-source TPR review)
**Status**: Research complete, plan created (2026-04-10)
**Blocked by**: ~~`plans/diagnostic-tooling-improvements`~~ — **COMPLETED** (archived to `plans/completed/diagnostic-tooling-improvements/` on 2026-04-09; all 13 mission criteria met, 16,964 tests passing). No longer blocking.
**Impacts**: `plans/code-journey-rework` (should wait for this plan too)
**Source**: Conversation analyzing LLVM diagnostic tools, reference compiler tooling, and Ori's current state; dual-source TPR review (codex + gemini, iteration 1)

## Dependency Chain

```
diagnostic-tooling-improvements (COMPLETED — archived 2026-04-09)
  └─ completed → diagnostics/ scripts stable, test-all.sh stable
      └─ llvm-verification-tooling (THIS PLAN — UNBLOCKED)
          └─ completes → adds AIMS verification, FileCheck, sanitizers, etc.
              └─ code-journey-rework
                  └─ Section 03 orchestrator consumes ALL tools (old + new)
                  └─ Section 04 adds --json to ALL tools (old + new)
                  └─ Phase B AI analysis scope SHRINKS (tools catch more automatically)
```

## What Ori Currently Has (Strong Foundation)

### LLVM Layer

| Tool | Status | Key Files |
|------|--------|-----------|
| Module-level IR verify | `module.verify()` pre-opt + JIT | `aot/passes/mod.rs:172`, `evaluator/compile.rs` |
| RC Balance verification | Linear walk, alloc/inc/dec/free tracking (274 lines) | `verify/rc_balance.rs` |
| COW Rules verification | Pointer invalidation checks (170 lines) | `verify/cow_rules.rs` |
| ABI Check | Aggregate loads, arg counts, nounwind+invoke (197 lines) | `verify/abi_check.rs` |
| Safety Checks | Panic/assert density analysis (216 lines) | `verify/safety_checks.rs` |
| Codegen Audit pipeline | `ORI_AUDIT_CODEGEN=1` with strict mode + function filter | `verify/mod.rs` |
| verify_each config | Plumbing exists (`OptimizationConfig.verify_each`), OFF by default | `aot/passes/config.rs:210` |
| Dual execution | Interpreter vs AOT comparison, auto-dumps on mismatch | `diagnostics/dual-exec-{debug,verify}.sh` |
| Debug/release compare | Behavioral + LLVM IR diff on mismatch | `diagnostics/debug-release-compare.sh` |
| Phase dumps | `ORI_DUMP_AFTER_{PARSE,TYPECK,ARC,LLVM}` to stderr | `oric/src/{arc_dump,llvm_dump}/` |
| Runtime memory diagnostics | `ORI_CHECK_LEAKS=1`, `ORI_TRACE_RC=1`, `ORI_RT_DEBUG=1` | `ori_rt` |
| Valgrind | Memory errors on AOT binaries (manual, not in CI) | `diagnostics/valgrind-aot.sh` |

### AIMS/ARC Layer (Ori-Specific — NOT in Research v1)

The AIMS pipeline already includes multiple verification checkpoints at steps 5a, 6, 7, and 11:

| Tool | Pipeline Step | Purpose | Key Files |
|------|--------------|---------|-----------|
| FIP contract verification | Step 5a | Verify FipContract::Certified ↔ zero unmatched alloc/dealloc | `aims/verify/fip.rs` |
| ARC IR sanity check | Step 6 | Structural validity of ARC IR after RC/reuse emission | `pipeline/aims_pipeline.rs` via `verify()` |
| AIMS contract vs IR consistency | Step 7 | Verify MemoryContract agrees with realized IR | `pipeline/aims_pipeline.rs` via `run_aims_verify()` |
| Final ARC IR sanity check | Step 11 | Post-COW/drop-hints structural validity | `pipeline/aims_pipeline.rs` via `verify()` |
| TRMC soundness verification | Step 3a | Verify TRMC context region correctness | `aims/normalize/verify.rs` |
| Per-phase RC snapshots | After each realize phase | Trace-level event per `(phase, function, block)` with RC ops | `aims/realize/emit_unified.rs` |
| Protocol builtin contracts | Test-time | Pin ownership semantics for `__index`, `__iter_next`, etc. | `aims/builtins/tests.rs` |
| ARC verification flag | Config | `ORI_VERIFY_ARC=1` for extra post-pipeline checks | `pipeline/aims_pipeline.rs:49` |

### Test Infrastructure

| Tool | Coverage | Key Files |
|------|----------|-----------|
| AOT integration tests | 94 test files (56 top-level + 38 nested) | `ori_llvm/tests/aot/` |
| Journey guards | Codegen quality regression pins | `ori_llvm/tests/aot/journey_guard.rs` |
| Iterator/protocol-builtin matrix | RC on iter drop, break, yield, element types | `ori_llvm/tests/aot/iterator_drop.rs`, `sets.rs` |
| `test-all.sh` | 7-suite pipeline (Rust + Ori + AOT + LLVM) | `test-all.sh` (576 lines) |
| `llvm-test.sh` | Build + ori_rt + ori_llvm unit tests | `llvm-test.sh` |
| Diagnostic self-tests | Regression harness for diagnostic scripts | `diagnostics/self-test.sh` |

### Diagnostic Scripts (`diagnostics/`)

14 scripts, all support `--help`, `--no-color`/`--color`:

| Script | Purpose |
|--------|---------|
| `ir-dump.sh` | Annotated LLVM IR (`--raw`, `--optimized`, `--function`) |
| `arc-dump.sh` | ARC IR post-lowering (`--raw`, `--function`) |
| `ir-diff.sh` | Compare two programs' IR |
| `disasm-ori.sh` | Native disassembly with Ori demangling |
| `rc-stats.sh` | Per-function RC balance summary |
| `codegen-audit.sh` | Static RC/COW/ABI analysis (`--strict`, `--function`) |
| `diagnose-aot.sh` | All-in-one: build + run + leak check + RC stats + IR |
| `dual-exec-debug.sh` | Interpreter vs AOT comparison |
| `dual-exec-verify.sh` | Batch interpreter vs LLVM (`--test-only`, `--main-only`, `--json`) |
| `debug-release-compare.sh` | Debug vs release behavioral comparison |
| `valgrind-aot.sh` | Valgrind memory errors |
| `check-debug-flags.sh` | Validate debug flag consistency |
| `self-test.sh` | Regression harness for all diagnostic scripts |

## What's Missing — Prioritized

### Tier 0: AIMS/ARC Pipeline Verification (Ori-Specific)

This is the highest-value tier because it tests Ori's **unique** verification surface — the AIMS pipeline. No other compiler has this exact architecture, so there are no off-the-shelf tools; everything must be purpose-built.

#### 0.1 AIMS Pass-Level Snapshot Tests

**The Problem**: The AIMS pipeline runs 12 steps (see `.claude/rules/arc.md` §Pipeline). Currently, only the final ARC IR is observable via `ORI_DUMP_AFTER_ARC`. If a pass regresses (e.g., RC elision stops firing for a case it previously caught), the regression is invisible until it manifests as a runtime leak or wrong behavior.

**The Solution**: Per-pass snapshot tests, inspired by Rust's MIR-opt infrastructure (`tests/mir-opt/`):
- Rust's MIR-opt uses `EMIT_MIR` directives to capture the IR **before and after** a specific pass, storing results as `.before.mir` / `.after.mir` / `.diff` files alongside the test source
- The tooling lives in `rust/src/tools/miropt-test-tools/src/lib.rs` (the `.before`/`.after`/`.diff` workflow at lines 77-99)
- Tests declare which pass to observe: `//@ test-mir-pass: CopyProp`

**Ori Analogue**: Create a test harness that captures pipeline artifacts at configurable boundaries. The 12 pipeline steps divide into three artifact types — snapshots must match each step's nature:

| Step | Name | Artifact Type | Snapshot Strategy |
|------|------|---------------|-------------------|
| 1-2 | `analyze_program`, `apply_ownership` | Whole-program contracts | Dump `MemoryContract` + `ArcParam.ownership` per function |
| 3 | `compute_var_reprs` | Per-variable metadata | Dump `ValueRepr` map |
| 3a | `normalize_function` | IR rewrite | `.before.arc` / `.after.arc` / `.diff` |
| 4 | `analyze_function` | Analysis state | Dump `AimsStateMap` (entry/exit states per block) |
| 5 | `realize_rc_reuse` | IR rewrite | `.before.arc` / `.after.arc` / `.diff` (the primary target) |
| 5a | `verify_fip_contract` | Check-only | **Already a verifier** — no snapshot needed; add regression pins for expected FIP outcomes |
| 6 | `verify()` | Check-only | **Already a verifier** — no snapshot needed |
| 7 | `run_aims_verify()` | Check-only | **Already a verifier** — no snapshot needed |
| 8 | `detect/rewrite_tail_calls` | IR rewrite | `.before.arc` / `.after.arc` / `.diff` |
| 8b | `unwind_cleanup` / `add_invoke_unwind_cleanup` | IR rewrite | `.before.arc` / `.after.arc` / `.diff` (runs between tail calls and merge; see `aims_pipeline.rs:342-344`) |
| 9 | `merge_blocks` | IR rewrite | `.before.arc` / `.after.arc` / `.diff` |
| 10 | `realize_annotations` | IR rewrite | `.before.arc` / `.after.arc` / `.diff` — see **State Map Caveat** below |
| 11 | `verify()` | Check-only | **Already a verifier** — no snapshot needed |
| 12 | FBIP enforcement | Diagnostic | Capture diagnostic output, pin expected results |

**Directive format**: `// @test-arc-pass: realize_rc_reuse` (captures IR before/after step 5)
**`--bless` mode**: update baselines when intentional changes are made — shared harness with LLVM IR tests (see §Shared Bless Harness below)

**State Map Caveat (Step 10)**: `realize_annotations` (step 10) runs AFTER `merge_blocks` (step 9). Per `.claude/rules/arc.md`: "Position-keyed state maps (`entry_states`, `exit_states`, `instr_states`) are invalid after `merge_blocks()`." The snapshot harness for step 10 MUST NOT attempt to dump position-keyed state map fields — only ArcVarId-keyed lookups and the IR itself are safe post-merge. This is a load-bearing constraint that the implementation must respect.

**Priority passes to snapshot**: `realize_rc_reuse` (step 5, highest value), `merge_blocks` (step 9), `realize_annotations` (step 10), `normalize_function` (step 3a)

**What This Catches**: RC elision regressions, COW annotation regressions, block merge changes, reuse detection failures — all invisible to behavioral tests when the optimizer papers over the codegen issue.

**Effort**: Medium — requires ARC IR serialization format (already exists in `ORI_DUMP_AFTER_ARC`), per-pass dump hooks, per-step artifact type awareness, shared test harness, initial snapshot corpus.

#### 0.2 AIMS Lattice State Sanity Checker

**The Problem**: The AIMS pipeline relies on a 7-dimensional product lattice (`AimsState` = AccessClass × Consumption × Cardinality × Uniqueness × Locality × ShapeClass × EffectClass). The backward dataflow analysis (step 4) must converge to a fixpoint, and the lattice meet/join operations must be monotone. Bugs in lattice operations can lead to unsound RC elision without triggering any existing verifier.

**The Solution**: A verification pass that asserts lattice properties at pipeline boundaries:
- **Monotonicity check**: verify that iterative worklist updates are non-decreasing across rounds. The AIMS backward analysis computes `block_entry_state` from `block_exit_state` by walking instructions backward and adding operand demand — demand can only increase across worklist iterations (defined via `a ≤ b iff a.join(b) == b`). The correct check is: for each worklist round, the new state for any block must be ≥ the previous state for that block. Do NOT assert entry ≤ exit within a single block — backward analysis works in the opposite direction.
- **Convergence check**: after analysis completes, recompute `compute_block_exit_state` and `compute_block_entry_state` for all blocks and verify that the recomputed states match the stored `AimsStateMap`. If they differ, the fixpoint was not reached — this is a soundness bug.
- **Dimension consistency**: verify that no `AimsState` has impossible dimension combinations (e.g., `Consumed` + `Unique` on the same variable)
- **Gate via existing surface**: route through `AimsPipelineConfig.verify_arc` (already controls structural ARC verification and contract-vs-IR checks, enabled by `ORI_VERIFY_ARC=1` or `debug_assertions`; see `pipeline/aims_pipeline.rs:43-50`, `pipeline/mod.rs:122-161`, `oric/src/debug_flags.rs:126-132`). Do NOT introduce a separate `ORI_VERIFY_AIMS_LATTICE=1` — that would create a second parallel verifier toggle for the same pipeline, violating SSOT. The lattice checks are a natural extension of the existing AIMS verification surface.

**Prior Art**: Lean4's IR Checker (`src/Lean/Compiler/IR/Checker.lean:31-139`) validates type consistency, variable uniqueness, and scope validity during IR generation — similar in spirit but for a different IR level.

**Effort**: Medium — the lattice is already well-defined in `ori_arc/src/aims/lattice/`; the checker is a new pass that walks the state map and asserts properties.

#### 0.3 Protocol Builtin Verification

**The Problem**: Compiler-internal protocol functions (`__index`, `iter`, `__iter_next`, `ori_iter_drop`, `__collect_set`) have per-argument ownership semantics that must be correct for borrow inference to work. These are defined in `ori_ir/src/builtin_constants/protocol/mod.rs` and consumed by `ori_arc`. A change to the ownership of any argument is catastrophic.

**Current State**: Protocol builtins have two existing test surfaces:
- `ori_arc/src/aims/builtins/tests.rs:143-149` — existence check only (verifies every protocol builtin has *some* contract via `sigs.contains_key`; does NOT pin per-argument ownership)
- `ori_ir/src/builtin_constants/protocol/tests.rs:20-58` — SSOT ownership pins (pins the actual per-argument `Ownership::Owned` / `Ownership::Borrowed` values for each protocol builtin)
- Borrow-semantics tests in `ori_arc/src/borrow/tests.rs` pin `AnnotatedSig` contracts

**The Solution**: Expand the existing pins into a full matrix:
- Every protocol builtin × every ownership combination × every argument position
- Assert that the AIMS pipeline produces the expected RC operations for each combination
- **Additionally**: verify RC balance for each test case. Note: the existing `rc_balance` checker (`ori_llvm/src/verify/rc_balance.rs`) operates on **LLVM IR** (`inkwell::module::Module`), not ARC IR. Two options: (a) compile protocol fixtures through LLVM and run the existing codegen audit / `rc_balance` checker on the LLVM module, or (b) build a new ARC-level RC balance checker that operates on `ArcFunction` directly. Option (a) reuses existing infrastructure; option (b) catches issues before LLVM lowering. The plan should choose one. Either way, pinning RC operations alone doesn't prove they're balanced — a test that perfectly pins an incorrect sequence would pass without the balance check.
- Add a contract-drift detection test: if any ownership annotation changes, the test names the specific builtin and argument that changed

**Effort**: Low — extends existing test infrastructure.

#### 0.4 ARC IR Parity Verification (Debug vs Release)

**The Problem**: `debug-release-compare.sh` currently compares **behavioral output** (exit codes + stdout) and **LLVM IR diffs** between debug and release builds. It does NOT compare ARC IR — the AIMS pipeline output. AIMS pipeline divergences between debug and release (due to different pass ordering, flags, or analysis precision) can cause subtle behavioral drift that's masked by LLVM optimization.

**The Solution**: Extend `debug-release-compare.sh` to capture and diff ARC IR (`ORI_DUMP_AFTER_ARC=1`) between debug and release builds. Structural ARC IR differences should be flagged, even when behavioral output is identical.

**Prior Art**: Swift uses `-enable-sil-verify-all` (`swift/validation-test/SIL/verify_all_overlays.py:1-6`) to run ALL SIL verifiers on every compilation, catching pipeline divergences early.

**Effort**: Low — add `ORI_DUMP_AFTER_ARC=1` capture to existing script, diff the ARC IR output.

#### 0.5 Journey Guard Regression Coverage

**The Problem**: Ori has 20 code journeys and journey guards in `compiler/ori_llvm/tests/aot/journey_guard.rs`, but the research doesn't mention integrating new verification tools with the journey system.

**The Solution**: Each new verification tool should produce journey-compatible output so the code-journey-rework plan can consume it:
- FileCheck results → journey dimension: "IR shape correctness"
- Sanitizer results → journey dimension: "memory safety"
- AIMS snapshot diffs → journey dimension: "pipeline stability"

**Effort**: Low planning, deferred implementation (depends on code-journey-rework).

### Tier 1: High Impact, Easy Integration

#### 1.1 Enable `verify_each` in CI/Test Builds

**What It Does**: LLVM runs the IR verifier after EVERY optimization pass. Catches which pass breaks IR well-formedness.

**Current State**: Plumbing exists (`OptimizationConfig.verify_each: bool` at `aot/passes/config.rs:210`, `with_verify_each()` at line 321). Default is `false`.

**What Rust/Swift Do**: Enabled in debug/CI builds. Swift uses `-enable-sil-verify-all` for the SIL layer.

**Integration Plan** (concrete, not hand-wave):

| Gate | Tool | Enabled | Env Var |
|------|------|---------|---------|
| `test-all.sh` | verify_each | YES — set `ORI_VERIFY_EACH=1` before LLVM test suites | `ORI_VERIFY_EACH` |
| `llvm-test.sh` | verify_each | YES | Same |
| `cargo test -p ori_llvm` | verify_each | YES (via test setup) | Compile-time cfg |
| `.github/workflows/ci.yml` | verify_each | YES — add to `cargo test --workspace` env | Same |
| `ori test --backend=llvm` | verify_each | Only when `ORI_VERIFY_EACH=1` | Opt-in |
| Release builds | verify_each | NO — too expensive | N/A |

**CI Gap (CRITICAL)**: `.github/workflows/ci.yml` currently runs `cargo test --workspace` + `cargo run -p oric -- test tests/` (interpreter only) + `cargo test -p ori_rt`. There is NO:
- `ori test --backend=llvm tests/` (LLVM backend spec tests)
- `cargo test -p ori_llvm --test aot` (AOT integration tests are covered by `--workspace` but worth calling out)
- `diagnostics/self-test.sh`
- `diagnostics/check-debug-flags.sh`

This CI gap should be addressed by `diagnostic-tooling-improvements` or as a prerequisite of this plan.

**Effort**: One config change + env var gate + CI workflow update.
**Expected Time Cost**: ~30-60% increase in LLVM test wall time (from running verifier after every pass).

#### 1.2 `opt -lint` Pass

Static lint for likely-undefined behavior in LLVM IR (beyond well-formedness):
- Division by potential zero
- Suspicious alignment
- Unreachable patterns
- Undefined behavior patterns that `verify_each` doesn't catch

**Integration**: Run via `opt -lint` on emitted IR in the codegen audit pipeline, or integrate as an additional LLVM pass in `aot/passes/mod.rs`.

**Effort**: Low — run existing LLVM tool on emitted IR.

#### 1.3 Function-Level Verification

**Current State**: `module.verify()` runs at module boundary only.

**The Solution**: Add `fn_val.verify(true)` after each function's codegen completes, before module-level verification. Earlier error detection = better diagnostics (the error points to a specific function, not "somewhere in the module").

**Prior Art**: Rust, Swift, and Zig all verify at function level. The rules file `.claude/rules/llvm.md` already mentions "Verify at multiple points" but function-level verification isn't consistently applied outside JIT.

**Effort**: Low — add Inkwell call after each function in the define pass.

### Tier 2: Medium Impact, Medium Effort

#### 2.1 IR Pattern Matching (FileCheck-Style) — THE CRITICAL TOOL

**THE SINGLE BIGGEST GAP.** Tests verify observable output but never IR shape. A program can generate correct output from terrible IR (optimizer papers over codegen bugs). Optimization regressions are invisible without IR shape tests.

**Canonical Format Decision** (per TPR finding [TPR-XX-003-codex] — SSOT requires ONE canonical format, not three competing options):

**Primary**: Directive-based pattern matching (Ori's own harness, inspired by Rust/Zig)
- Directives embedded in Ori test files: `// CHECK: call void @_ori_rc_dec`
- Support Zig's `.matches` mode (order-independent substring search) as default
- Support `.exact` mode for precise IR validation (rarely needed)
- Build in Rust (not an external `FileCheck` binary dependency)

**Secondary**: Per-pass snapshot diffs (for AIMS passes — Tier 0.1)
- `.before.arc` / `.after.arc` / `.diff` artifacts scoped to individual pipeline passes
- This is NOT a competing assertion system — it's a complementary tool for AIMS-specific verification

**Revision System** (per TPR finding [TPR-XX-001-gemini]):
- Support `// @revisions: debug release no-repr-opt` to run tests against multiple flag sets
- Inspired by Rust's compiletest revision system (`//@ revisions: foo bar`)
- Eliminates file duplication when testing feature interactions with optimization flags
- **FastISel limitation**: revisions that test debug vs release LLVM IR cannot catch FastISel-specific bugs (e.g., the >16B aggregate load bug documented in `.claude/rules/llvm.md`). FastISel operates at instruction selection time, not at IR level — the LLVM IR is identical in both modes. FastISel bugs require behavioral testing (existing `debug-release-compare.sh` and dual-execution), not IR pattern matching. Revisions should focus on optimization-level differences, not FastISel differences.

**Directory Layout**:
```
tests/codegen/               # LLVM IR pattern tests
  rc/                        # RC emission patterns
  cow/                       # COW patterns
  closures/                  # Closure codegen
  abi/                       # ABI patterns
  iterator/                  # Iterator codegen
tests/arc-opt/               # AIMS pass snapshot tests (Tier 0.1)
  realize_rc_reuse/          # Step 5 snapshots
  merge_blocks/              # Step 9 snapshots
  realize_annotations/       # Step 10 snapshots
```

### Shared Bless Harness (SSOT Requirement)

Tier 0.1 (AIMS snapshots) and Tier 2.1 (LLVM IR assertions) both need directive parsing, revision expansion, artifact naming, `--bless` mode, and failure diffing. These MUST share a single runner to avoid duplicated parser/bless logic and drifting conventions. The shared harness:

- **One runner binary** (e.g., `ori-check`, built in Rust) that:
  - Parses directives (`// @test-arc-pass:`, `// CHECK:`, `// @revisions:`) from test source files
  - Dispatches to the appropriate backend (ARC dump, LLVM IR dump)
  - Handles artifact naming (`.before.arc`, `.after.arc`, `.diff` for ARC; `.ll` for LLVM)
  - Implements `--bless` mode to update baselines across both artifact types
  - Produces unified failure output consumable by the code-journey system
- **Directory split** reflects artifact type, not harness split:
  - `tests/codegen/` → LLVM IR assertions (`.matches` / `.exact` / `CHECK:`)
  - `tests/arc-opt/` → AIMS pass snapshots (`.before.arc` / `.after.arc` / `.diff`)
  - Both consumed by the same runner

This prevents the SSOT failure mode where two overlapping harnesses with duplicated logic drift apart.

**Prior Art Design-Input Matrix**:

| Compiler | Artifact Shape | Assertion Granularity | Trigger Point | Failure Output | Ori Decision |
|----------|---------------|----------------------|---------------|----------------|-------------|
| **Rust** (FileCheck) | `// CHECK:` directives in `.rs` files | Line-level, regex groups, ordered | `tests/codegen/` via compiletest | Shows expected vs actual IR | **Adopt**: directive syntax |
| **Rust** (MIR-opt) | `.before.mir` / `.after.mir` / `.diff` files | Per-pass snapshots | `tests/mir-opt/` via `EMIT_MIR` | Full diff of pass output | **Adopt**: for AIMS passes |
| **Rust** (Revisions) | `//@ revisions: foo bar` | Per-config parameterization | Compiletest | Runs test N times with different flags | **Adopt**: revision system |
| **Zig** | `.matches` / `.exact` modes | Substring vs full match | Custom `addCheckFile()` in `test/src/LlvmIr.zig:45-73` | Expected pattern not found in output | **Adopt**: `.matches` as default mode |
| **Swift** (SILVerifier) | 7+ specialized verifiers | Structural, ownership, flow, debug, memory | Every compilation via `-enable-sil-verify-all` | Verifier assertion + location | **Adapt**: AIMS verifier expansion |
| **Lean4** (IR Checker) | Type/scope/uniqueness validation | Per-instruction, per-variable | IR generation time, `Checker.lean:31-139` | Checker assertion message | **Adapt**: lattice sanity checker |
| **Koka** (FBIP check) | Functional-but-in-place verification | Per-function, pattern-level | `Core/CheckFBIP.hs` (per `koka/test/fip/README.md:120-121`) | Allocation site that violates FIP | **Already have**: FIP enforcement at step 12 |

**Effort**: Medium — framework + initial test suite of ~30-50 key patterns.

#### 2.2 Sanitizer-Instrumented Test Binaries (ASan/UBSan)

Currently only Valgrind (20-50x slower, not in CI).

| Sanitizer | Overhead | Catches |
|-----------|----------|---------|
| ASan | ~2x | Buffer overflows, use-after-free, stack overflow, use-after-return |
| UBSan | ~1.2x | Integer overflow, null deref, signed overflow, shift overflow |
| MSan | ~3x | Uninitialized reads (requires full-program instrumentation) |

**Key**: Instrument the **GENERATED code** (the AOT binary), not the compiler itself.

**Integration**: Pass `-fsanitize=address,undefined` flags when emitting test binaries. Add to `test-all.sh` as an optional suite (gated by `ORI_SANITIZE=1`).

**Timeout Compliance (CRITICAL)**: CLAUDE.md mandates a strict 150-second timeout for ALL test commands — no exceptions. Sanitized binaries run at 2-3x overhead, which means individual sanitizer test commands would routinely hit the 150s ceiling. The solution is NOT to raise the timeout (that violates project policy and masks hung tests). Instead:
- **Shard sanitizer suites**: split AOT tests into smaller batches so each batch completes within 150s even with ASan/MSan overhead
- **Narrow fixtures**: sanitizer tests should use the smallest programs that exercise the code path, not the full test corpus
- **Separate sanitizer test commands**: each shard is its own `timeout 150` command, so a hang in one shard doesn't block others
- If analysis shows that even sharded tests cannot complete in 150s, that is a prerequisite signal: either the test is too large (simplify it) or the project timeout policy needs a formal amendment before sanitizer integration proceeds

**Effort**: Medium — requires LLVM pass configuration for sanitizer instrumentation of generated code + timeout scaling logic.

#### 2.3 `opt --opt-bisect-limit` — LLVM Pass Bisection

Binary-searches which LLVM optimization pass breaks the code. Different from AIMS phase bisection.

**Integration**: Add `diagnostics/opt-bisect.sh` script.
**Usage**: `./diagnostics/opt-bisect.sh file.ori` — automatically bisects, reports the failing pass.

**Effort**: Low — script around existing LLVM flag.

#### 2.4 `llvm-mca` — Machine Code Analyzer

Simulates execution on microarchitecture model. Predicts throughput, latency, pipeline stalls.

**Use Case**: Hot paths — iterator `next()`, RC operations, closure dispatch.
**Integration**: Add `diagnostics/mca-analyze.sh` script. Pipe assembly to `llvm-mca --timeline`.

**Effort**: Low — script around existing LLVM tool.

### Tier 3: High Impact, High Effort

#### 3.1 Alive2 — Formal Verification of LLVM Lowering

**What It Does**: Alive2 is the modern standard for proving that one LLVM IR program is a refinement of another. It can verify that Ori's ARC-lowered LLVM IR preserves the semantics of the source program.

**Use Case for Ori**: Verify that AIMS-to-LLVM lowering is correct — not just "produces right output for test inputs" but "is mathematically guaranteed to be a valid refinement."

**Integration Options**:
- **Alive2 `opt` plugin**: verify that each LLVM optimization pass preserves semantics
- **Standalone `alive-tv`**: compare two IR files (pre-opt vs post-opt) for refinement
- **CI integration**: run `alive-tv` on key test programs nightly

**Limitations**: Alive2 works at the LLVM IR level — it cannot verify ARC IR semantics directly. It would verify that the LLVM IR produced by the ARC emitter is a valid refinement of the "obvious" LLVM IR that a naive lowering would produce.

**Effort**: High — requires Alive2 build, integration harness, and careful selection of test programs.

#### 3.2 `llvm-reduce` — Automatic Test Case Reduction

**Note**: `bugpoint` is deprecated as of LLVM 19+ in favor of `llvm-reduce`. The research should NOT invest in `bugpoint` integration.

`llvm-reduce` shrinks large failing LLVM IR to the minimum reproducing case. Saves hours of manual reduction when debugging miscompiles.

**Integration**: Write an "interestingness test" script that detects the failure condition, then:
```bash
llvm-reduce --test reduce-test.sh input.ll -o reduced.ll
```

**Effort**: Medium — write interestingness test scripts for common failure modes (wrong output, crash, leak).

#### 3.3 Coverage-Guided Fuzzing of the Backend

- `llvm-stress` generates random valid IR
- `libFuzzer` with `FuzzMutate` for structured IR mutations
- Feed random IR through `ori_llvm` to find crashes/miscompiles

**Integration**: Build Rust fuzzing harness with `cargo-fuzz`. Add `fuzz/` directory to workspace.

**Effort**: High — build fuzzing harness, mutation strategies, crash triage pipeline.

#### 3.4 IR Baseline/Regression Tracking

Capture IR output for key programs, store as baselines, diff on each commit. Different from pattern matching (Tier 2.1) — catches ANY IR change, not just pattern violations.

**Note**: This is the secondary assertion mode complementing Tier 2.1's primary pattern matching. Scoped to pass-local diff artifacts (per Tier 0.1), not a second general-purpose assertion system.

**Integration**: Similar to existing `perf-baseline.sh` pattern. `scripts/ir-baseline.sh [--capture|--compare|--bless]`.

**Effort**: Medium — framework + baseline capture for ~20-30 key programs.

### Tier 4: Additional Tools

#### 4.1 Valgrind Supplementary Tools

| Tool | Purpose | Use Case | Effort |
|------|---------|----------|--------|
| Cachegrind | Cache/branch prediction simulation | Profile cache misses in generated code | Low |
| Callgrind | Call-graph profiling | Already used for parser — apply to generated code | Low |
| Massif | Heap profiler | Track heap usage patterns in ARC programs | Low |
| DHAT | Dynamic heap analysis | Find short-lived allocations (ARC optimization opportunities) | Low |
| Helgrind/DRD | Thread error detection | When concurrency support lands | Deferred |

#### 4.2 `llvm-dwarfdump --verify` — Debug Info Verification

Verifies DWARF debug info well-formedness. Needed when source-level debugging matures (debug info generation exists at `ori_llvm/src/aot/debug/`).

**Effort**: Low once debug info is being actively tested.

## CI Integration Architecture

**Current CI State** (`.github/workflows/ci.yml`):

| Job | What It Runs | LLVM Coverage |
|-----|-------------|---------------|
| `test` | `cargo test --workspace` | Includes ori_llvm unit + integration tests |
| `test` | `cargo run -p oric -- test tests/` | Interpreter-only Ori spec tests |
| `test` | `cargo test -p ori_rt` | Runtime unit tests |
| `cross-platform` | `cargo test --workspace` (macOS/Windows) | Same as above |
| **MISSING** | `ori test --backend=llvm tests/` | LLVM backend spec tests |
| **MISSING** | `diagnostics/self-test.sh` | Diagnostic script regression |
| **MISSING** | `diagnostics/check-debug-flags.sh` | Flag consistency |

### Tiered Execution Strategy

| Tier | When | Tools | Budget |
|------|------|-------|--------|
| **Every commit** | PR CI | `verify_each`, `opt -lint`, function-level verify, AIMS verifiers (steps 6/7/11) | +2-3 min |
| **Nightly** | Scheduled | FileCheck test suite, ASan/UBSan on AOT tests, AIMS snapshot tests, ARC IR parity, Alive2 on key programs | +15-20 min |
| **Weekly** | Scheduled | Full sanitizer matrix, llvm-mca hot path analysis, IR baseline comparison, DHAT heap profiling | +30-45 min |
| **On-demand** | Manual | Fuzzing, full Valgrind suite, Massif/Cachegrind, opt-bisect | Unbounded |

### Performance Budget

- Current `test-all.sh` wall time: ~90-120 seconds locally
- Every-commit additions should add **at most 3 minutes** to CI
- Nightly additions can run up to **30 minutes**
- Weekly additions can run up to **60 minutes**

**Fallback Knobs** (when CI time becomes a problem):
- `ORI_VERIFY_EACH=0` to disable per-pass verification
- `ORI_SANITIZE=0` to skip sanitizer builds
- `ORI_FILECHECK=0` to skip IR pattern tests
- Each tool is independently gatable

## Diagnostics Integration

### Integration with `diagnostics/` Framework

New verification tools MUST follow the existing `diagnostics/` conventions:
1. **Script location**: `diagnostics/<tool-name>.sh`
2. **Common flags**: `--help`, `--no-color`/`--color`, `--verbose`, `--json`
3. **Exit codes**: 0 = pass, 1 = findings, 2 = error
4. **Machine-readable output**: `--json` flag produces structured output consumable by code-journey-rework
5. **Self-test**: each new script adds entries to `diagnostics/self-test.sh`
6. **Documentation**: each new script documented in `diagnostics/README.md`

### JSON Output Contract

All new verification tools that produce findings should emit JSON compatible with the code-journey system:
```json
{
  "tool": "aims-snapshot-check",
  "status": "findings",
  "findings": [
    {
      "severity": "high",
      "location": "function_name::step_5",
      "message": "RC elision regression: inc/dec pair not eliminated",
      "before": "...",
      "after": "..."
    }
  ]
}
```

This contract enables `code-journey-rework` §03 (orchestrator) and §04 (`--json` for all tools) to consume results automatically.

### Meta-Testing Strategy (Who Tests the Testers?)

Every new verification tool must have its own regression tests:
1. **Positive test**: a program that passes verification
2. **Negative test**: a program with a known defect that the tool MUST catch
3. **False-positive test**: a program that looks problematic but is actually correct — tool must NOT flag it
4. **Integration in `self-test.sh`**: each tool's positive/negative/false-positive tests run as part of the diagnostic self-test suite

### Incremental Adoption Order

For each tool, the adoption path is:
1. **Optional debug script** — `diagnostics/foo.sh`, manually invoked
2. **Required local gate** — added to `test-all.sh` (gated by env var, default ON)
3. **Required CI gate** — added to `.github/workflows/ci.yml`
4. **Release gate** — runs in release pipeline (`auto-release.yml`)

Each tool progresses through these stages independently. A tool doesn't need to reach stage 4 before the next tool starts at stage 1.

## Key Architectural Insight

Ori already has IR-level verification — the AIMS pipeline includes structural verifiers at steps 5a/6/7/11, TRMC soundness checking, and the codegen audit suite (`ORI_AUDIT_CODEGEN=1`). What Ori **lacks** is stable **regression pinning** of IR shapes. The existing verifiers catch well-formedness violations and contract mismatches, but they don't detect when a pass *regresses* (stops optimizing a case it previously handled) or when IR quality *drifts* (correct but worse IR that the optimizer happens to fix). These are different questions:

| Scenario | Observable Output | IR Quality | Detection |
|----------|------------------|------------|-----------|
| Correct output from bad ARC IR | Correct | Wrong (LLVM opts paper over it) | AIMS snapshots (Tier 0.1) |
| Correct output from bad LLVM IR | Correct | Wrong (optimizer papers over it) | FileCheck (Tier 2.1) |
| Correct IR producing wrong output | Wrong | Correct | Existing behavioral tests |
| Wrong ARC IR producing wrong LLVM IR | May be correct or wrong | Wrong at both levels | Dual-layer verification |

Mature compilers test at **all layers**:
- **ARC/AIMS layer**: pass-level snapshots + lattice verification + contract checking (Tier 0)
- **LLVM IR layer**: FileCheck + verify_each (Tiers 1-2)
- **Runtime layer**: sanitizers + Valgrind (Tier 2)
- **Formal layer**: Alive2 refinement checking (Tier 3)

Ori's unique advantage is that it has **two IR levels** (ARC IR and LLVM IR) plus an **interpreter**, enabling triple-redundancy verification that no other compiler has.

## Reference Compiler Design-Input Matrix

Expanded from the v1 comparison table into a design-input matrix with concrete mechanisms:

| Compiler | IR Test Infrastructure | Pass-Level Verification | Sanitizer Integration | Unique Approach | Ori Takeaway |
|----------|----------------------|------------------------|----------------------|----------------|-------------|
| **Rust** | `tests/codegen/` (FileCheck, hundreds of tests), `tests/mir-opt/` (.before/.after/.diff), revision system (`//@ revisions:`) | MIR-opt per-pass snapshots via `EMIT_MIR` directive, MIR validation passes | ASan/MSan/TSan/UBSan via LLVM instrumentation; `tests/ui/sanitizer/` | Two-layer testing: MIR (pre-LLVM) AND LLVM IR (post-lowering). Revision system for multi-config testing. | **Adopt all three**: FileCheck for LLVM IR, MIR-opt-style for AIMS passes, revisions for multi-config |
| **Swift** | `test/IRGen/` (FileCheck), SIL tests | 7+ specialized SIL verifiers: Structural, Ownership, Flow-Sensitive, Debug, MemoryLifetime, TypeLayout, ARC. `-enable-sil-verify-all` runs all. | Full sanitizer support | Layered verification — different verifiers catch different bug classes. SILOwnershipVerifier specifically for ARC. | **Adapt**: expand AIMS verifiers (currently 4 → target 7+ covering lattice, monotonicity, convergence) |
| **Zig** | Custom `addCheckFile()` with `.matches`/`.exact` modes (`test/src/LlvmIr.zig:45-73, 120-127`). 20+ compile flags per test. | Per-function verification | Minimal (TSan flag support) | Pragmatic `.matches` mode reduces test brittleness vs full FileCheck. | **Adopt**: `.matches` as default IR assertion mode |
| **Lean4** | Minimal IR testing | IR Checker validates type consistency, variable uniqueness, scope validity (`Checker.lean:31-139`). Borrow analysis validates owned/borrowed semantics. | None | Formal verification via Lean's type system. IR Checker as compile-time invariant enforcement. | **Adapt**: lattice sanity checker (Tier 0.2) inspired by IR Checker's invariant approach |
| **Koka** | FIP-specific test suite (`test/fip/`) | FBIP checking (`Core/CheckFBIP.hs`) — similar to Ori's FIP enforcement at step 12 | None | FBIP as a language-level guarantee, not just an optimization hint. | **Already have**: FIP enforcement. Could strengthen with FBIP-specific regression tests. |

## Risk Analysis

### What Happens If These Tools Aren't Built

| Risk | Consequence | Likelihood | Tier That Mitigates |
|------|------------|------------|-------------------|
| AIMS pass regression goes undetected | Leak/double-free in production code; user blames Ori, not the optimizer | High | Tier 0 (AIMS snapshots) |
| LLVM opt papers over codegen bug | Bug resurfaces when LLVM version changes or optimization level changes | High | Tier 2.1 (FileCheck) |
| CI doesn't run LLVM backend tests | Regressions detected only on developer machines, never in automated pipeline | High | Tier 1.1 (verify_each + CI integration) |
| Memory errors in generated code | Silent corruption, mysterious crashes for users | Medium | Tier 2.2 (sanitizers) |
| Lattice bug causes unsound RC elision | Use-after-free that passes all behavioral tests | Medium | Tier 0.2 (lattice checker) |
| Miscompile goes unfound for months | Difficult debugging, user trust eroded | Medium | Tier 3.1 (Alive2) |

### Priority Based on Risk

The risk analysis confirms the tier ordering:
1. **Tier 0** (AIMS/ARC) mitigates the highest-likelihood, Ori-specific risks
2. **Tier 1** (verify_each, function verify) mitigates CI-gap risks
3. **Tier 2** (FileCheck, sanitizers) mitigates the industry-standard risks
4. **Tier 3** (Alive2, fuzzing) mitigates long-tail risks

## LLVM Discourse Context

Thread: https://discourse.llvm.org/t/the-need-for-better-frontend-benchmarks/90257

Key takeaways:
- **Chandler Carruth (Carbon)**: Synthesized source code benchmarks — controlled properties, reproducible randomization. `carbon-lang/testing/base/source_gen.h`.
- **LLVM 22 shipped with documented 16% compile-time regression** (hansw2000).
- **Users reporting >50% regression on template-heavy code** (mikeynap).
- **The real bottleneck is personnel, not infrastructure** (efriedma-quic).

Relevant: the thread highlights that even LLVM itself struggles with regression detection. Ori's code journey system + AIMS verification + the LLVM verification tooling from this plan would give Ori better regression detection than LLVM has for its own backend — a genuine competitive advantage.

## Recommended Implementation Order

```
Priority 0 (Ori-Specific — highest value, no external deps)
├── AIMS pass-level snapshot tests (Tier 0.1)
├── AIMS lattice state sanity checker (Tier 0.2)
├── Protocol builtin verification expansion (Tier 0.3)
└── ARC IR parity verification (Tier 0.4)

Priority 1 (Easy Wins — first week after Priority 0)
├── Enable verify_each in CI/test builds (Tier 1.1)
├── Add opt -lint to codegen audit pipeline (Tier 1.2)
└── Add function-level verify() after each fn codegen (Tier 1.3)

Priority 2 (Biggest Gaps — core of the plan)
├── FileCheck-style IR pattern matching with revision system (Tier 2.1)
├── ASan/UBSan on generated test binaries (Tier 2.2)
├── opt-bisect diagnostic script (Tier 2.3)
└── llvm-mca integration for hot paths (Tier 2.4)

Priority 3 (Hardening)
├── Alive2 formal verification (Tier 3.1)
├── llvm-reduce integration (Tier 3.2)
├── Coverage-guided fuzzing (Tier 3.3)
└── IR baseline/regression tracking (Tier 3.4)

Priority 4 (Polish)
├── llvm-dwarfdump --verify (Tier 4.2)
├── Massif/DHAT for ARC heap analysis (Tier 4.1)
└── Cachegrind/Callgrind for generated code (Tier 4.1)
```
