---
section: "01"
title: "Foundation & Policy Rules"
status: not-started
reviewed: false
goal: "Establish shared policy rules that all subsequent sections reference — compile-time performance gates, crash regression test discipline, and explicit phase documentation"
success_criteria:
  - "impl-hygiene.md contains a compile-time performance measurement gate rule"
  - "tests.md contains a crash/ICE regression test rule with tests/crashes/ directory"
  - "compiler.md or the Salsa query module contains explicit phase execution documentation"
  - "All rules are enforceable — each has a concrete verification mechanism"
inspired_by:
  - "TypeScript --extendedDiagnostics (compiler/commandLineParser.ts:435-455)"
  - "Rust tests/crashes/ minimized crash reproduction directory"
  - "Zig Compilation.zig explicit job queue with stage ordering"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Compile-Time Performance Measurement Gate"
    status: not-started
  - id: "01.2"
    title: "Crash/ICE Regression Test Rule"
    status: not-started
  - id: "01.3"
    title: "Explicit Phase Execution Documentation"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Foundation & Policy Rules

**Status:** Not Started
**Goal:** Establish the shared policy rules that all subsequent sections in this plan and all downstream plans reference. Three rules that production compilers enforce but Ori doesn't yet mandate.

**Success Criteria:**
- [ ] `impl-hygiene.md` has a compile-time performance measurement gate rule requiring measurement evidence for hot-path changes — satisfies mission criterion "All new rules documented"
- [ ] `tests.md` has a crash/ICE regression test rule requiring minimized reproducers in `tests/crashes/` — satisfies mission criterion "All new rules documented"
- [ ] `compiler.md` has an explicit phase execution graph listing all Salsa tracked functions in execution order with their contracts — satisfies mission criterion "All new rules documented"
- [ ] `tests/crashes/` directory exists with a README explaining the crash regression discipline

**Context:** The dual-source TPR review identified that existing coding guidelines (`#[cold]`, `#[inline]`, no hot-path alloc) don't mandate measurement evidence. Benchmarks exist (`oric/benches/type_check.rs`, `parser.rs`, `borrow_inference.rs`) but no rule requires their use. Similarly, Rust's `tests/crashes/` pattern for minimized ICE reproducers has no Ori equivalent. And while Salsa handles phase ordering correctly via demand-driven queries, the ordering is invisible without tracing — a documented phase graph makes it auditable.

**Reference implementations:**
- **TypeScript** `src/compiler/executeCommandLine.ts:1153-1195`: reports files, identifiers, symbols, types, instantiations, cache sizes, and phase timings via `--extendedDiagnostics`
- **Rust** `tests/crashes/`: directory of minimized ICE reproducers, each a `.rs` file with `//@ known-bug:` annotation
- **Zig** `src/Compilation.zig`: compilation defined as a queue of jobs tagged with stages

---

## 01.1 Compile-Time Performance Measurement Gate

**File(s):** `.claude/rules/impl-hygiene.md`

Add a rule in the Performance Annotations section (`impl-hygiene.md:551`) requiring changes to hot queries, inference, lowering, codegen, or cache structure to include compile-time measurement evidence.

- [ ] Add rule text to `impl-hygiene.md` §Performance Annotations after the `#[must_use]` rule. The rule must specify:
  - **When it applies**: changes to Salsa tracked queries, unification/inference hot paths, ARC pipeline passes, LLVM emission, or cache/interning structure
  - **What evidence is required**: before/after benchmark output from `cargo bench -p oric --bench {relevant}` or `ORI_LOG=ori_types=info` phase timing, or tracing output showing query counts
  - **Existing tools**: cite `perf-baseline.sh`, `cow-benchmark.sh`, `oric/benches/type_check.rs`, `oric/benches/parser.rs`, `oric/benches/borrow_inference.rs`
  - **When exempt**: cold paths (error handling, CLI parsing, test setup), documentation, rule file changes
  - File size constraint: `impl-hygiene.md` is at 697 lines — well under the 500-line limit for rule files (rule files are exempt from the source file limit, but be concise)

- [ ] Add reference to the new rule in `CLAUDE.md` §Compiler Coding Guidelines Performance bullet

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [ ] All tasks above are `[x]` and the rule text is verified in the file
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 01.2 Crash/ICE Regression Test Rule

**File(s):** `.claude/rules/tests.md`, `tests/crashes/README.md`

Add a crash/ICE regression discipline modeled on Rust's `tests/crashes/` pattern. When a compiler crash or internal compiler error (ICE) is discovered and fixed, a minimized reproducer is added to `tests/crashes/` as a permanent regression guard.

- [ ] Create `tests/crashes/` directory with a `README.md` explaining:
  - One `.ori` file per crash/ICE bug
  - File naming: `{BUG-ID}-{short-description}.ori` (e.g., `BUG-04-045-cross-target-linker.ori`)
  - Each file must reproduce the original crash in under 20 lines
  - Files are compiled (not executed) by the test harness — the test passes if compilation does NOT crash
  - `#compile_fail("error message")` for bugs that should produce a diagnostic instead of crashing

- [ ] Add rule text to `tests.md` requiring:
  - Every fixed compiler crash/ICE MUST add a minimized reproducer to `tests/crashes/`
  - The reproducer MUST be small enough to bisect (target: <20 lines)
  - The `/fix-bug` completion checklist MUST include "crash reproducer added if the bug was a crash/ICE"
  - Cite Rust's `tests/crashes/` pattern as prior art

- [ ] Add `tests/crashes/` to `test-all.sh` test runner (compile-only, no execution)

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting 01.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 01.3 Explicit Phase Execution Documentation

**File(s):** `.claude/rules/compiler.md`, `.claude/rules/canon.md`

Add a documented phase graph to `compiler.md` that explicitly lists all Salsa tracked functions in execution order with their input/output types and contracts. This makes the implicit Salsa ordering auditable without tracing.

- [ ] Add a "Phase Execution Graph" section to `compiler.md` §Architecture listing:
  - Every Salsa `#[tracked]` function in `oric/src/query/`
  - For each: function name, input type, output type, invariants guaranteed
  - The demand-driven execution order (which queries trigger which)
  - Cross-reference to `canon.md §1` (pipeline table) for the full phase picture
  - Note: this is documentation, not code — Salsa handles execution; the doc makes it inspectable

- [ ] Verify the documented graph matches actual code by reading `compiler/oric/src/query/` and listing all `#[tracked]` functions

- [ ] Cross-reference: update `canon.md §1` Pipeline Overview if the new documented graph reveals any inaccuracies in the existing table

- [ ] **Subsection close-out (01.3)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] All subsections (01.1, 01.2, 01.3) complete
- [ ] All new rules are in the correct canonical rule files
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean (after TPR)
- [ ] **`/improve-tooling`** — section-close sweep: verify subsection retrospectives ran, add cross-cutting improvements
- [ ] **`/sync-claude`** — section-close doc sync: verify all Claude artifacts reflect code changes from this section
