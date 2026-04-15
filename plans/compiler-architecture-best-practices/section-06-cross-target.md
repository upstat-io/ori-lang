---
section: "06"
title: "Cross-Target Codegen Verification"
status: not-started
reviewed: false
goal: "Mandate and implement FileCheck-based ABI assertions for non-host targets so ABI/layout/calling-convention changes are verified beyond the host architecture"
success_criteria:
  - "FileCheck codegen tests exist for at least one non-host target (aarch64 if host is x86_64, or vice versa)"
  - "An end-to-end --target flag test compiles an Ori program for a non-host target and verifies the LLVM IR"
  - "codegen-rules.md contains the cross-target verification mandate"
  - "ABI-sensitive codegen changes (AB-*, NR-*, TM-*) require non-host FileCheck evidence"
inspired_by:
  - "Go test/codegen/README:17-26 (host-architecture testing default, comprehensive cross-arch recommended)"
  - "Rust cross-compile test suite (compile-only for non-host targets, FileCheck for IR verification)"
  - "LLVM FileCheck (existing ori_test_harness support: CHECK, CHECK-NOT, CHECK-LABEL, CHECK-NEXT)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Non-Host FileCheck Tests"
    status: not-started
  - id: "06.2"
    title: "--target End-to-End Test"
    status: not-started
  - id: "06.3"
    title: "Cross-Target Verification Rule"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Cross-Target Codegen Verification

**Status:** Not Started
**Goal:** Add FileCheck-based ABI assertions for non-host targets and a mandatory rule requiring cross-target evidence for ABI-sensitive changes.

**Success Criteria:**
- [ ] Non-host FileCheck tests exist and pass — satisfies mission criterion "cross-target verification"
- [ ] `--target` end-to-end test works (compile-only, not execute) — satisfies mission criterion "non-host target testing"
- [ ] `codegen-rules.md` contains the cross-target rule — satisfies mission criterion "rules documented"

**Context:** Research found extensive cross-target infrastructure (802 lines in `cross.rs`):
- `ori_llvm/tests/aot/cross.rs`: 30+ tests for target triple parsing, platform detection, CPU features, data layout, pointer sizes across 8 supported targets
- `ori_llvm/src/aot/target_features.rs` + `tests.rs`: `Arch` enum, `TargetTripleComponents`, alias canonicalization (arm64→aarch64)
- The compiler CAN create `TargetConfig` for non-host targets and query data layouts
- Cross-target LLVM IR emission is structurally possible (target triple flows to `TargetMachine`)
- **Gap**: no tests verify actual LLVM IR emission for non-host targets — only triple parsing and data layout queries. No `--target` CLI end-to-end test.
- Related bug: BUG-04-045 (arm64 vs aarch64 mismatch in cross-compilation)

**Reference implementations:**
- **Go** `test/codegen/README:17-26`: "host-architecture testing is the default for performance reasons, but comprehensive all-architecture checks are recommended after compiler changes"
- **Rust**: cross-compile test suite with FileCheck for IR verification

**Depends on:** Section 01.

---

## 06.1 Non-Host FileCheck Tests

**File(s):** `compiler/ori_llvm/tests/codegen/`, `compiler/ori_llvm/tests/aot/cross.rs`

Add FileCheck tests that emit LLVM IR for a non-host target and verify ABI-sensitive attributes (sret, calling conventions, struct passing, signedness).

- [ ] Write FileCheck tests targeting aarch64 (assuming x86_64 host) or vice versa:
  - Test 1: Verify `sret` attribute on ARM64 for structs >16 bytes (`codegen-rules.md §AB-4`)
  - Test 2: Verify `c_char` signedness handling in FFI declarations
  - Test 3: Verify calling convention attributes for the target
  - Test 4: Verify aggregate passing mode (direct vs indirect threshold at 16 bytes)

- [ ] Use the existing `ori_test_harness` FileCheck infrastructure (`// CHECK:`, `// CHECK-NOT:`, etc.) with a target override mechanism:
  - The test creates a `TargetConfig::from_triple("aarch64-unknown-linux-gnu")` (or equivalent)
  - Emits LLVM IR using the non-host target
  - Runs FileCheck against the emitted IR

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 06.2 --target End-to-End Test

**File(s):** `compiler/oric/`, `compiler/ori_llvm/tests/`

Add an integration test that exercises the full compile path for a non-host target (compile-only, not link/execute).

- [ ] Write integration test:
  - Compile a simple Ori program targeting a non-host triple
  - Verify: compilation succeeds (exit code 0)
  - Verify: emitted LLVM IR contains the correct target triple
  - Verify: data layout matches the target (not the host)
  - This is compile-only — no linking, no execution (we don't have a cross-linker in CI)

- [ ] If `ori build --target aarch64-unknown-linux-gnu` doesn't exist yet, add the `--target` flag to the CLI:
  - Pass the triple through to `TargetConfig::from_triple()`
  - Wire through to `TargetMachine` creation in `ori_llvm`
  - This may already work — check if the existing cross-compilation infrastructure supports it end-to-end

- [ ] **Subsection close-out (06.2)** — MANDATORY before starting 06.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 06.3 Cross-Target Verification Rule

**File(s):** `.claude/rules/codegen-rules.md`, `.claude/rules/tests.md`

- [ ] Add rule to `codegen-rules.md` (new subsection in §9 Verification or §2 ABI):
  - ABI-, layout-, calling-convention-, or signedness-sensitive changes MUST include at least one non-host target FileCheck assertion
  - The assertion must verify the specific ABI attribute being changed (sret, calling convention, struct passing, c_char signedness)
  - Use the existing `ori_test_harness` FileCheck harness with `TargetConfig::from_triple()`
  - The rule applies to changes touching: `codegen-rules.md §AB-*` (ABI classification), `§NR-*` (narrowing), `§TM-*` (trampolines), `§RT-*` (runtime contract)

- [ ] Add cross-reference in `tests.md` linking to the cross-target rule in `codegen-rules.md`

- [ ] **Subsection close-out (06.3)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] All subsections (06.1-06.3) complete
- [ ] Non-host FileCheck tests pass
- [ ] --target end-to-end test passes
- [ ] Rule documented in codegen-rules.md
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
