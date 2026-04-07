---
section: "10"
title: "Advanced Rewrites & Verification"
status: not-started
reviewed: false
goal: "Add distributivity with cost model, multiplicative inverse, and comprehensive verification of the full optimization pipeline"
success_criteria:
  - "a*c + b*c → (a+b)*c fires when cost model determines it is profitable"
  - "Multiplicative inverse x * (1/x) → 1 fires for types with Inv trait and nonzero precondition"
  - "Full test matrix covers all type × law × pattern combinations"
  - "Dual-exec parity: eval and LLVM produce identical results for all test programs"
  - "ORI_CHECK_LEAKS=1 clean on all algebraic test programs"
  - "Code journey passes — progressively complex programs compiled correctly"
  - "Satisfies mission criterion: distributivity, inverse, verification all complete"
inspired_by:
  - "Tang 2013 Ch. 7: LICM, GVN, copy propagation for user-defined types"
  - "LLVM reassociate pass: cost-gated reassociation"
  - "GCC -fassociative-math: separate flag for unsafe math reassociation"
depends_on: ["09"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "10.1"
    title: "Distributivity with Cost Model"
    status: not-started
  - id: "10.2"
    title: "Multiplicative Inverse"
    status: not-started
  - id: "10.3"
    title: "Full Test Matrix"
    status: not-started
  - id: "10.4"
    title: "Dual-Exec Parity Verification"
    status: not-started
  - id: "10.5"
    title: "Code Journey & Safety"
    status: not-started
  - id: "10.6"
    title: "Documentation"
    status: not-started
  - id: "10.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "10.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Advanced Rewrites & Verification

**Status:** Not Started
**Goal:** Complete the algebraic optimization pipeline with distributivity (Ring law), multiplicative inverse, and comprehensive verification that the entire semantic optimization pipeline is correct, performant, and regression-free.

**Success Criteria:**

- [ ] Distributivity fires: `a*c + b*c → (a+b)*c` when profitable (fewer operations)
- [ ] Distributivity does NOT fire when unprofitable (e.g., when factoring increases live ranges)
- [ ] Multiplicative inverse: `x * recip(x) → 1` for types with `Inv` trait and nonzero proof
- [ ] Full test matrix: every type × law × pattern combination tested
- [ ] `diagnostics/dual-exec-verify.sh` passes on all algebraic test programs
- [ ] `ORI_CHECK_LEAKS=1` clean on all test programs
- [ ] `/code-journey` passes with escalating complexity

**Depends on:** Section 09 (normalization framework).

---

## 10.1 Distributivity with Cost Model

**File(s):** `compiler/ori_arc/src/aims/normalize/algebra.rs`

Distributivity rewrites factor common subexpressions: `a*c + b*c → (a+b)*c`. This reduces operation count (2 multiplications + 1 addition → 1 addition + 1 multiplication) but increases the live range of the intermediate `(a+b)` result. A cost model gates the rewrite.

- [ ] Detect pattern (indirect through ArcVarId def chains — all operators are PrimOp in ARC IR):
  - Outer: `Let { dst: d, value: PrimOp { op: Binary(Add), args: [mul1_dst, mul2_dst] } }`
  - `mul1_dst` defined by: `Let { value: PrimOp { op: Binary(Mul), args: [a, c1] } }`
  - `mul2_dst` defined by: `Let { value: PrimOp { op: Binary(Mul), args: [b, c2] } }`
  - Where `c1 == c2` (same ArcVarId) AND `Mul` has `DistributesOver { inner: Add }` law for this type
- [ ] Also detect left-distributed: `c*a + c*b → c*(a+b)` depending on `side: left/right/both`
- [ ] Cost model: count operations before and after. Only apply if post-rewrite has fewer operations (2 mul + 1 add → 1 add + 1 mul = saves 1 multiplication)
- [ ] **Live range consideration**: the factored common factor `c` must be live at both uses. If it's already live (same variable), factoring is free. If it requires an extra load/retain, cost may increase — gate on this.
- [ ] Rewrite: create new intermediate `Let { dst: sum_var, value: PrimOp { op: Binary(Add), args: [a, b] } }` then replace outer with `Let { dst: d, value: PrimOp { op: Binary(Mul), args: [sum_var, c] } }`
- [ ] **Reverse factoring** (expansion): `c*(a+b) → c*a + c*b` — useful when expansion enables other optimizations (e.g., constant folding on `c*a`). Only apply with explicit cost benefit.
- [ ] Write tests:
  - `a*c + b*c → (a+b)*c` for int (two muls → one)
  - `a*c + b*c` where c is different variables → NOT factored
  - `a*c + b*d` → NOT factored (different factors)
  - Cost model test: case where factoring would increase live range → NOT applied
- [ ] Verify: `timeout 150 cargo t -p ori_arc` passes

**Matrix dimensions:**
- Types: int (Mul distributes over Add)
- Patterns: right-factor, left-factor, both sides, no common factor, nested factoring
- Semantic pin: `a*c + b*c` produces ARC IR with 1 Mul + 1 Add (not 2 Mul + 1 Add)
- Negative pin: `a*c + b*d` is NOT factored

- [ ] **Subsection close-out (10.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.2 Multiplicative Inverse

**File(s):** `library/std/prelude.ori` (NEW trait), `compiler/ori_arc/src/aims/normalize/algebra.rs`

Add `Inv`/`Recip` trait for types with multiplicative inverse. Gate on nonzero precondition.

- [ ] Add to prelude.ori:
  ```ori
  pub trait Inv {
      @inv (self) -> Self
      // Precondition: self != Zero::zero()
      // Law: self.inv() * self == One::one()
  }
  ```
- [ ] Implement for `float` (using current parser syntax `impl Trait for Type`):
  ```ori
  impl Inv for float {
      @inv (self) -> float = 1.0 / self
  }
  ```
- [ ] Do NOT implement for `int` — integer division loses precision, `inv` is meaningless
- [ ] Add `Inverse` law to `Mul` trait algebra block: `inverse: Inv::inv` (multiplicative inverse)
- [ ] `impl Mul for float` can declare `laws [commutative, inverse]` — the `inverse` law asserts the mathematical property exists. BUT the normalization rewrite `x * inv(x) → 1.0` must be guarded by finite-nonzero proof (see precondition below)
- [ ] Normalization: detect `x * inv(x) → One::one()` (1.0 for float)
- [ ] **Precondition**: only fire if `x` is provably a finite nonzero value. For now, conservative: only fire if `x` is a literal that is nonzero AND finite (not NaN, not Inf, not -Inf). The guard must exclude ALL of: zero, NaN, Inf, -Inf.
  - `NaN * inv(NaN)` = `NaN * NaN` = `NaN` (not 1.0) — UNSOUND without guard
  - `Inf * inv(Inf)` = `Inf * 0.0` = `NaN` (not 1.0) — UNSOUND without guard
  - `-Inf * inv(-Inf)` = `-Inf * -0.0` = `NaN` (not 1.0) — UNSOUND without guard
  - `0.0 * inv(0.0)` = `0.0 * Inf` = `NaN` (not 1.0) — UNSOUND without guard
  - `-0.0 * inv(-0.0)` = `-0.0 * -Inf` = `NaN` (not 1.0) — UNSOUND without guard
  Future: integrate with range metadata and non-NaN/non-Inf proofs from type-level contracts.
- [ ] Write tests:
  - `x * (1.0 / x) → 1.0` where x is literal nonzero finite constant (e.g., `2.0`)
  - `x * (1.0 / x)` where x is variable → NOT optimized (can't prove finite nonzero)
  - `0.0 * (1.0 / 0.0)` → NOT optimized (zero)
  - `NaN * (1.0 / NaN)` → NOT optimized (NaN guard)
  - `Inf * (1.0 / Inf)` → NOT optimized (Inf guard)
- [ ] Verify: `timeout 150 ./test-all.sh` passes

- [ ] **Subsection close-out (10.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.3 Full Test Matrix

**File(s):** Tests across `tests/spec/algebra/`, `compiler/ori_llvm/tests/aot/`, `tests/valgrind/`

Build a comprehensive test matrix covering every type × law × pattern combination across all sections.

- [ ] **Identity elimination matrix:**

| Type | Operator | Identity | Left | Right | Both |
|------|----------|----------|------|-------|------|
| int | Add | 0 | x | x | x |
| int | Mul | 1 | x | x | x |
| str | Add | "" | x | x | x |
| [T] | Add | [] | x | x | x |
| int | BitAnd | -1 (all set) | x | x | x |
| int | BitOr | 0 | x | x | x |
| int | BitXor | 0 | x | x | x |
| float | Add | (excluded) | - | - | - |
| float | Mul | (excluded) | - | - | - |

Note: `&&`/`||` are NOT PrimOps (short-circuit control flow). No ARC IR identity elimination possible.

- [ ] **Involutive matrix:**

| Type | Operator | Test |
|------|----------|------|
| int | Neg | -(-x) → x |
| bool | Not | !!b → b |
| int | BitNot | ~~n → n |

- [ ] **Commutative ordering matrix:**

| Type | Operator | Before | After |
|------|----------|--------|-------|
| int | Add | 3 + x | x + 3 |
| int | Mul | 5 * y | y * 5 |
| int | BitAnd | -1 & x | x & -1 |
| int | BitOr | 0 \| x | x \| 0 |

Note: `&&`/`||` are NOT PrimOps — they produce control-flow IR, not commutable operands.

- [ ] **Associativity matrix:**

| Type | Operator | Input | Canonical |
|------|----------|-------|-----------|
| int | Add | (a+b)+c | a+(b+c) |
| int | Mul | (a*b)*c | a*(b*c) |
| str | Add | (s1+s2)+s3 | s1+(s2+s3) |

- [ ] **Inverse matrix:**

| Type | Operator | Input | Result |
|------|----------|-------|--------|
| int | Add/Neg | x+(-x) | 0 |
| int | Add/Neg | (-x)+x | 0 |

- [ ] **Distributivity matrix:**

| Type | Outer | Inner | Input | Result |
|------|-------|-------|-------|--------|
| int | Mul | Add | a*c+b*c | (a+b)*c |

- [ ] Verify: `timeout 150 cargo st tests/spec/algebra/` passes
- [ ] Verify: `timeout 150 cargo t -p ori_llvm` (AOT tests) passes

- [ ] **Subsection close-out (10.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.4 Dual-Exec Parity Verification

Verify that the normalization pass preserves behavioral equivalence between interpreter and LLVM paths.

- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/algebra/` — all programs produce identical output in eval vs AOT
- [ ] Run `diagnostics/dual-exec-verify.sh tests/spec/expressions/` — existing expression tests still produce identical output
- [ ] Any mismatch → investigate and fix before proceeding
- [ ] Track results in a verification report

- [ ] **Subsection close-out (10.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.5 Code Journey & Safety

- [ ] Run `/code-journey` with algebraic programs (matrix arithmetic, vector operations, numeric algorithms)
- [ ] All CRITICAL findings triaged
- [ ] Run `ORI_CHECK_LEAKS=1` on all algebraic test programs — zero leaks
- [ ] Run `diagnostics/valgrind-aot.sh tests/spec/algebra/` — zero memory errors
- [ ] Stress test: program with 1000+ algebraic operations → normalization pass completes in < 100ms

- [ ] **Subsection close-out (10.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.6 Documentation

- [ ] Update CLAUDE.md with new commands/flags:
  - `ORI_DUMP_AFTER_ALGEBRA=1` — dump ARC IR after algebraic normalization
  - `algebra {}` syntax in trait definitions
  - `laws []` syntax in impl blocks
  - `Zero`/`One`/`Inv` traits
- [ ] Update `.claude/rules/ori-syntax.md` with algebra/laws syntax: add `algebra`, `laws`, `supports` to context-sensitive keyword list; add `algebra {}` block in trait definition syntax; add `laws []` in impl syntax; add `Zero`/`One`/`Inv` in prelude traits
- [ ] Update `docs/ori_lang/v2026/spec/operator-rules.md` with algebraic law semantics
- [ ] Update `docs/ori_lang/v2026/spec/grammar.ebnf` with `algebra` and `laws` grammar rules
- [ ] Add architecture overview to `compiler/ori_arc/src/aims/normalize/algebra.rs` module docs
- [ ] Document in the module docs: `BinaryOp::And`/`Or`/`Coalesce` are intercepted before PrimOp emission and lowered to control-flow IR — they NEVER appear as `PrimOp::Binary(And)` etc. in ARC IR. Only bitwise `BitAnd`/`BitOr`/`BitXor` reach PrimOp.

- [ ] **Subsection close-out (10.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.R Third Party Review Findings

- None.

---

## 10.N Completion Checklist

- [ ] Distributivity with cost model implemented and tested
- [ ] Multiplicative inverse implemented for float with nonzero guard
- [ ] Full test matrix covers all type × law × pattern combinations
- [ ] Dual-exec parity verified — eval and LLVM match for all test programs
- [ ] `ORI_CHECK_LEAKS=1` clean on all algebraic programs
- [ ] Valgrind clean on all algebraic programs
- [ ] Code journey passes
- [ ] Stress test passes (1000+ ops, < 100ms)
- [ ] All documentation updated (CLAUDE.md, spec, grammar, rules)
- [ ] Plan annotation cleanup: all `semantic-optimization-pipeline` annotations removed from code — run `grep -rn "semantic-optimization-pipeline" compiler/ library/` to verify zero hits
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] **Plan sync** — update all plan metadata:
  - [ ] All sections `status` → `complete`
  - [ ] `00-overview.md` all mission success criteria checked, status → `complete`
  - [ ] `index.md` all section statuses updated
  - [ ] Cross-plan links updated (roadmap items resolved)
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** The full semantic optimization pipeline is operational: LLVM metadata emitted, AIMS facts exported, derived methods normalized, runtime identities fixed, algebraic laws declared and consumed, normalization pass canonicalizes and simplifies expressions, distributivity and inverse work with cost/precondition gates. `./test-all.sh` passes with 0 failures. Dual-exec parity holds for all programs. Memory safety verified by Valgrind and leak checker. Documentation updated to reflect new language surface and compiler capabilities.
