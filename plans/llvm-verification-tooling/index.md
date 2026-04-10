---
reroute: true
name: "LLVM Verify"
full_name: "LLVM & AIMS Verification Tooling"
status: active
order: 1
---

# LLVM & AIMS Verification Tooling Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

## Keyword Clusters by Section

### Section 01: Verifier Gates & Quick Wins
**File:** `section-01-verifier-gates.md` | **Status:** Not Started

```
verify_each, ORI_VERIFY_EACH, function-level verify, fn_val.verify
opt -lint, opt_lint, codegen audit, ORI_AUDIT_CODEGEN
run_verify, run_aims_verify, tracing::warn, blocking failure
debug_flags.rs, OptimizationConfig, config.rs, verify_each
pipeline/mod.rs, postprocess.rs, debug_assert, FIP
```

---

### Section 02: Shared Test Harness Infrastructure
**File:** `section-02-shared-harness.md` | **Status:** Not Started

```
directive parser, //@, CHECK:, CHECK-LABEL, CHECK-NOT
revision system, revisions, --bless, bless mode
artifact naming, .before.arc, .after.arc, .diff, .ll
ori_test_harness, workspace library, oric subcommand
compiletest, miropt-test-tools, FileCheck
test runner, snapshot comparison, diff generation
```

---

### Section 03: AIMS Pass-Level Snapshot Tests
**File:** `section-03-aims-snapshots.md` | **Status:** Not Started

```
AIMS pipeline, 12 steps, pass-level snapshots
realize_rc_reuse, merge_blocks, realize_annotations, normalize_function
.before.arc, .after.arc, .diff, per-pass dump hooks
tests/arc-opt/, snapshot corpus, baseline artifacts
ORI_DUMP_AFTER_ARC, ArcFunction, ArcInstr, ArcTerminator
MIR-opt pattern, EMIT_MIR, Rust compiletest
```

---

### Section 04: AIMS Lattice Property Verification
**File:** `section-04-lattice-properties.md` | **Status:** Not Started

```
proptest, property-based testing, lattice axioms
AimsState, 7D product lattice, 11520 raw states
join commutativity, associativity, idempotence
transfer monotonicity, canonicalize idempotence
fixpoint convergence, iteration bound, chain height 15
AccessClass, Consumption, Cardinality, Uniqueness
Locality, ShapeClass, EffectClass, AimsState::SCALAR
dimensions.rs, lattice/mod.rs, transfer/mod.rs
```

---

### Section 05: Contract Coherence Oracle
**File:** `section-05-contract-oracle.md` | **Status:** Not Started

```
MemoryContract, ParamContract, contract coherence
re-derive from realized IR, RcInc, RcDec, Reuse
interprocedural, analyze_program, extract_contract
intraprocedural, analyze_function, AimsStateMap
FIP evidence, verify_fip_contract, may_deallocate
second pass, batch.rs, pipeline coherence
contract vs realization, non-negotiable invariant
```

---

### Section 06: Protocol Builtin Verification Matrix
**File:** `section-06-protocol-builtins.md` | **Status:** Not Started

```
ProtocolBuiltin, __index, iter, __iter_next
ori_iter_drop, __collect_set, ownership matrix
Owned, Borrowed, per-argument, per-position
RC balance, codegen audit, protocol/tests.rs
builtin_constants, protocol/mod.rs
aims/builtins/tests.rs, AnnotatedSig
```

---

### Section 07: FileCheck-Style IR Pattern Matching
**File:** `section-07-filecheck.md` | **Status:** Not Started

```
FileCheck, CHECK:, CHECK-LABEL:, CHECK-NOT:
directive-based, IR assertions, LLVM IR patterns
tests/codegen/, RC emission, COW patterns
closure codegen, ABI patterns, iterator codegen
revision system, debug, release, no-repr-opt
.matches mode, .exact mode, Zig addCheckFile
Rust compiletest, codegen tests, ir-dump.sh
```

---

### Section 08: Sanitizer Integration
**File:** `section-08-sanitizers.md` | **Status:** Not Started

```
ASan, UBSan, AddressSanitizer, UndefinedBehaviorSanitizer
ORI_SANITIZE, generated code, AOT binary
LLVM pass pipeline, asan-module, SanitizerMode
separate CI job, smoke subset, nightly full sweep
sharding, 150s timeout, buffer overflow, use-after-free
linker integration, libasan, libubsan, -fsanitize
```

---

### Section 09: Alive2 Formal Verification
**File:** `section-09-alive2.md` | **Status:** Not Started

```
Alive2, alive-tv, translation validation, SMT
Z3, refinement checking, pre-opt vs post-opt
curated subset, nightly, pure functions
llvm2alive, TransformVerify, Transform
limitations, memory operations, loops, external calls
formal verification, mathematical proof, counterexample
```

---

### Section 10: Differential Oracle Fuzzing
**File:** `section-10-differential-fuzzing.md` | **Status:** Not Started

```
differential testing, eval vs LLVM, oracle
cargo-fuzz, libfuzzer-sys, fuzz_target
coverage-guided, random programs, mutation
fuzz/, fuzz_targets/, corpus/, artifacts/
eval∩LLVM subset, ORI_CHECK_LEAKS, divergence
ori_parse fuzzer, ori_aot_compile fuzzer
nightly Rust, -Zsanitizer, seed corpus
```

---

### Section 11: CI Integration & ARC IR Parity
**File:** `section-11-ci-integration.md` | **Status:** Not Started

```
.github/workflows/ci.yml, CI workflow
verify_each in CI, function-level verify in CI
LLVM backend spec tests, ori test --backend=llvm
ARC IR parity, debug vs release, ORI_DUMP_AFTER_ARC
debug-release-compare.sh, ARC IR diff
opt-bisect, opt --opt-bisect-limit, diagnostics/opt-bisect.sh
tiered execution, every-commit, nightly, weekly
ORI_VERIFY_EACH, ORI_SANITIZE, ORI_FILECHECK
```

---

### Section 12: Verification Dashboard & Regression Tracking
**File:** `section-12-regression-dashboard.md` | **Status:** Not Started

```
IR baseline, regression tracking, trend detection
scripts/ir-baseline.sh, --capture, --compare, --bless
golden IR, key programs, baseline corpus
llvm-reduce, test case reduction, interestingness test
verification metrics, pass counts, failure trends
dashboard, CI artifacts, historical comparison
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Verifier Gates & Quick Wins | `section-01-verifier-gates.md` |
| 02 | Shared Test Harness Infrastructure | `section-02-shared-harness.md` |
| 03 | AIMS Pass-Level Snapshot Tests | `section-03-aims-snapshots.md` |
| 04 | AIMS Lattice Property Verification | `section-04-lattice-properties.md` |
| 05 | Contract Coherence Oracle | `section-05-contract-oracle.md` |
| 06 | Protocol Builtin Verification Matrix | `section-06-protocol-builtins.md` |
| 07 | FileCheck-Style IR Pattern Matching | `section-07-filecheck.md` |
| 08 | Sanitizer Integration | `section-08-sanitizers.md` |
| 09 | Alive2 Formal Verification | `section-09-alive2.md` |
| 10 | Differential Oracle Fuzzing | `section-10-differential-fuzzing.md` |
| 11 | CI Integration & ARC IR Parity | `section-11-ci-integration.md` |
| 12 | Verification Dashboard & Regression Tracking | `section-12-regression-dashboard.md` |
