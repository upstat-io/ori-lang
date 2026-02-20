---
section: "05"
title: Cleanup & Verification
status: not-started
goal: Remove all dead code, run full test suite, benchmark, update docs
sections:
  - id: "05.1"
    title: Dead Code Elimination
    status: not-started
  - id: "05.2"
    title: Full Test Suite
    status: not-started
  - id: "05.3"
    title: Performance Validation
    status: not-started
  - id: "05.4"
    title: Documentation & Spec
    status: not-started
  - id: "05.5"
    title: Consistency Audit
    status: not-started
---

# Section 05: Cleanup & Verification

**Status:** 📋 Planned
**Goal:** The codebase is clean, all tests pass, no performance regressions, documentation is updated.

**BE EXHAUSTIVE.** This is the final verification pass. Leave nothing behind. Run every check. Delete every dead import. Update every doc comment that mentions `run()`, `SeqBinding`, or the old patterns. This section is what separates a clean refactor from one that leaves a trail of debris.

---

## 05.1 Dead Code Elimination

- [ ] Run `./clippy-all.sh` and fix ALL warnings:
  - Unused imports (from removed `SeqBinding`, `SeqBindingRange`, `FunctionSeq::Run`)
  - Unused functions (former Run-specific handlers)
  - Unused variables
  - Dead code warnings
- [ ] Run `cargo c` with `--cfg test` to check test-only code
- [ ] Grep for orphaned references to old concepts:
  ```bash
  grep -rn 'run_seq\|run_block\|run_with_checks\|run_rule' compiler/
  grep -rn 'seq_binding\|seq_bindings' compiler/
  grep -rn 'SeqBinding\|SeqBindingRange' compiler/
  ```
- [ ] Check for orphaned test helpers that only served `FunctionSeq::Run`:
  - Test factories, assertion helpers, mock constructors
- [ ] Delete `compiler/ori_ir/src/ast/patterns/seq/` directory if now empty (or reduce to just FunctionSeq definition)
  - If `FunctionSeq` is the only remaining type, consider moving it to a simpler location
  - Evaluate: does `FunctionSeq` still warrant its own sub-module, or should it move up?
- [ ] Delete `compiler/ori_fmt/src/rules/run_rule.rs` if entirely dead
- [ ] Remove any `#[allow(dead_code)]` or `#[expect(dead_code)]` that were added as temporary workarounds
- [ ] Run `./clippy-all.sh` again — MUST pass clean

---

## 05.2 Full Test Suite

Run EVERY test suite. No exceptions. No skips.

- [ ] `cargo t` — all Rust unit tests across all crates
- [ ] `cargo st` — all Ori spec tests (3,842+)
- [ ] `./test-all.sh` — comprehensive test suite (10,219+ tests)
- [ ] `./llvm-test.sh` — LLVM/AOT tests (394+)
- [ ] `cargo t -p ori_parse -- incremental` — incremental parsing
- [ ] `cargo t -p ori_fmt` — formatter tests
- [ ] `cargo st tests/spec/patterns/try.ori` — try blocks specifically
- [ ] `cargo st tests/spec/expressions/block_scope.ori` — block scoping
- [ ] `cargo st tests/spec/patterns/match.ori` — match expressions
- [ ] `cargo st tests/spec/patterns/run.ori` — run pattern tests (should these be renamed/updated?)
- [ ] `cargo st tests/compile-fail/` — all compile-fail tests

If ANY test fails, investigate and fix. Do not proceed to 05.3 with failing tests.

---

## 05.3 Performance Validation

The parser and type checker are performance-critical. Block parsing changed — verify no regression.

- [ ] Run parser benchmark BEFORE changes (already have baseline from prior work):
  ```bash
  cargo bench -p oric --bench parser -- "raw/throughput"
  ```
  Record baseline: ______ MiB/s

- [ ] Run parser benchmark AFTER changes:
  ```bash
  cargo bench -p oric --bench parser -- "raw/throughput"
  ```
  Record result: ______ MiB/s

- [ ] Verify no regression >5% vs baseline
- [ ] If regression detected:
  - Profile with `cargo bench -p oric --bench parser -- --profile-time=5`
  - Likely cause: shared block-parsing helper adding overhead
  - Fix: `#[inline]` on the shared helper, or keep it monomorphized
- [ ] Run type checker benchmark if available:
  ```bash
  cargo bench -p oric --bench type_check
  ```
- [ ] Document results in this section (fill in the blanks above)

---

## 05.4 Documentation & Spec

- [ ] Update grammar spec: `docs/ori_lang/0.1-alpha/spec/grammar.ebnf`
  - Remove any `run_expression` or `run_block` production rules if present
  - Verify `block_expression` is the canonical production
- [ ] Update doc comments throughout:
  - `compiler/ori_ir/src/ast/patterns/seq/mod.rs` — update `FunctionSeq` module docs (no more Run)
  - `compiler/ori_ir/src/ast/stmt.rs` — update `StmtKind` docs (it's now the canonical statement type)
  - `compiler/ori_types/src/infer/expr/sequences.rs` — update module docs (no more run_seq)
  - `compiler/ori_canon/src/lower/sequences.rs` — update module docs
  - `compiler/ori_fmt/src/formatter/stacked.rs` — update module docs
- [ ] Update design docs if they reference `FunctionSeq::Run`:
  - `grep -rn 'FunctionSeq::Run\|SeqBinding\|run_seq' docs/`
  - `grep -rn 'FunctionSeq::Run\|SeqBinding\|run_seq' plans/`
- [ ] Update error code docs if E-codes referenced Run:
  - `compiler/ori_diagnostic/src/error_code/mod.rs`
  - `docs/compiler/design/appendices/C-error-codes.md`
- [ ] Update `.claude/rules/` files:
  - `compiler.md` — if it mentions FunctionSeq/SeqBinding
  - `ir.md` — update IR description
  - Any others: `grep -rn 'SeqBinding\|FunctionSeq::Run' .claude/`
- [ ] Update MEMORY.md if it references old patterns
- [ ] Run `/sync-spec` skill if grammar changed
- [ ] Run `/sync-docs` skill if design docs changed

---

## 05.5 Consistency Audit

Final paranoia check. Run every automated consistency test the codebase has.

- [ ] `./fmt-all.sh` — formatting consistency
- [ ] `./clippy-all.sh` — lint consistency
- [ ] `./build-all.sh` — full build including LLVM
- [ ] Check `compiler/oric/tests/` consistency tests:
  - `oric/tests/phases/common/parse/tests.rs` — parser consistency
  - `oric/tests/phases/common/typecheck/tests.rs` — type checker consistency
- [ ] Verify the `TYPECK_BUILTIN_METHODS` sorted test still passes (alphabetical sort)
- [ ] Verify the `TYPECK_METHODS_NOT_IN_EVAL` consistency test still passes
- [ ] Check that no `.ori` test file references `run(` as a block construct:
  ```bash
  grep -rn 'run(' tests/ library/ --include='*.ori' | grep -v '// ' | head -20
  ```
  (Some hits may be legitimate — `run` as a function name in user code, not as the old block syntax)

---

## 05.6 Completion Checklist

- [ ] `./test-all.sh` passes — ALL 10,219+ tests
- [ ] `./clippy-all.sh` passes — zero warnings
- [ ] `./fmt-all.sh` passes — zero formatting changes
- [ ] `./build-all.sh` passes — including LLVM
- [ ] No performance regression >5%
- [ ] Zero references to `SeqBinding`, `SeqBindingRange`, `FunctionSeq::Run` in entire repo
- [ ] All doc comments updated
- [ ] Grammar spec updated
- [ ] CLAUDE.md / MEMORY.md updated if needed

**Exit Criteria:** The refactor is invisible to users. All tests pass. All docs are accurate. Performance is unchanged. The codebase is cleaner than before.

---

## Final Verification Command

Run this single command to verify the entire refactor:

```bash
./test-all.sh && ./clippy-all.sh && ./fmt-all.sh && echo "BLOCK UNIFICATION COMPLETE"
```
