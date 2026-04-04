---
section: "05"
title: "Structural Derive Normalization"
status: not-started
reviewed: false
goal: "Mark derived methods as memory(read) and leave a clean handoff for ARC-level peepholes to Section 09"
success_criteria:
  - "Derived Eq/Comparable/Hashable methods have memory(read) LLVM attribute"
  - "Section 09 explicitly owns reflexive/boolean peepholes at the ARC IR layer; Section 05 leaves no duplicate implementation work"
  - "Satisfies mission criterion: derived methods marked memory(read) enabling GVN/CSE"
inspired_by:
  - "DerivedTrait is_nounwind_derived() pattern (ori_ir/src/derives/mod.rs:218)"
  - "Nounwind attribute application at declaration time (derive_codegen/mod.rs:268)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "memory(read) on Derived Methods"
    status: not-started
  - id: "05.2"
    title: "Reflexive Peepholes"
    status: not-started
  - id: "05.3"
    title: "Boolean and Bitwise Simplifications"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Structural Derive Normalization

**Status:** Not Started
**Goal:** Derived trait methods (Eq, Comparable, Hashable) have known properties: they only read their arguments, never allocate, and have deterministic results. Marking them `memory(read)` lets LLVM's GVN eliminate redundant calls (e.g., calling `x.eq(y)` twice in the same basic block without intervening writes). The reflexive and boolean peepholes remain documented here because they motivated the section, but their executable implementation anchor is Section 09's shared ARC normalization pass.

**Success Criteria:**

- [ ] `ORI_DUMP_AFTER_LLVM=1` shows `memory(read)` on derived Eq/Comparable/Hashable method functions
- [ ] LLVM GVN eliminates second call when `point.eq(other)` is called twice without intervening store
- [ ] Section 09 is the sole implementation anchor for reflexive/boolean peepholes; Section 05 requires no duplicate ARC peephole pass

**Context:** The DerivedTrait system already has `is_nounwind_derived()` (derives/mod.rs:218) which marks derived Eq/Comparable/Clone/Hashable/Default as nounwind. The same pattern applies for `memory(read)` — these methods read fields but never write or allocate. For reflexive peepholes: derived structural Eq always returns true when both operands are the same variable (because field-by-field comparison of identical values is trivially true). Float Eq is excluded because `NaN != NaN`.

**Depends on:** Nothing for 05.1 (memory(read) attributes). Subsections 05.2 and 05.3 operate at the ARC IR level and their peepholes overlap with Section 09's normalization pass. **Implementation order: 05.1 in Phase 1, 05.2/05.3 deferred to Phase 4 and implemented AS PART OF Section 09** (identity elimination subsumes boolean identity; involutive rewrite subsumes double negation; reflexive peephole shares the same ARC IR walker). This avoids building redundant peephole infrastructure that Section 09 immediately replaces. The items remain documented in Section 05 for completeness but the implementation anchor is in Section 09.

**Implementation note (05.2/05.3):** These peepholes operate on ARC IR (`ArcValue::PrimOp`), NOT LLVM codegen (`ValueId`). See subsection details for why.

---

## 05.1 memory(read) on Derived Methods

**File(s):** `compiler/ori_ir/src/derives/mod.rs`, `compiler/ori_llvm/src/codegen/derive_codegen/mod.rs`

Follow the `is_nounwind_derived()` pattern: add `is_readonly_derived()` metadata predicate to `DerivedTrait`, then apply the attribute at function declaration time in derive codegen.

- [ ] Add `is_readonly_derived(&self) -> bool` method to `DerivedTrait` in `derives/mod.rs`:
  - Returns `true` for: Eq, Comparable, Hashable (read fields only, never allocate)
  - Returns `false` for: Clone (allocates), Default (allocates), Printable (allocates strings), Debug (allocates strings)
- [ ] Write failing AOT test FIRST (TDD): call `point.eq(other)` twice in sequence → before the fix, IR shows two calls. This test will verify GVN elimination after the fix.
- [ ] In `derive_codegen/mod.rs` `setup_derive_function()` (at line 272 where `is_nounwind_derived` is checked): add `if trait_kind.is_readonly_derived() { fc.builder_mut().add_memory_read_attribute(func_id); }`
- [ ] Verify `add_memory_read_attribute` exists on IrBuilder (or add it in `attributes.rs` — currently `attributes.rs` has function-level attribute methods at 287 lines; check if a `memory(read)` attribute method already exists)
- [ ] Verify AOT test now passes: IR shows single call (GVN eliminated the duplicate)
- [ ] Verify: `timeout 150 cargo t -p ori_llvm` passes (debug AND release)

---

## 05.2 Reflexive Peepholes

**File(s):** `compiler/ori_arc/src/aims/normalize/` (NEW, ARC IR level), NOT `ori_llvm` codegen

**IMPORTANT**: This peephole CANNOT be implemented at the LLVM codegen level (`emit_binary_op()`). The codegen receives `ValueId` (opaque LLVM values), NOT `ArcVarId` (original variable identity). By the time `emit_binary_op()` runs, the original variable identity that would let us detect `x == x` (same source variable) is lost — two `ValueId`s could be the same underlying variable but the codegen has no way to know.

The reflexive peephole must operate on ARC IR, where `ArcValue::PrimOp { op: PrimOp::Binary(Eq), args: [v1, v2] }` still carries `ArcVarId` values. When `v1 == v2` (same ArcVarId), we can safely replace with a literal.

**Derived-vs-custom provenance**: At ARC IR level, ALL `==` / `!=` operators (both primitive and user-defined types) lower to `PrimOp::Binary(Eq)` / `PrimOp::Binary(NotEq)`. There is no `Apply` path for operator syntax. The type checker validates Eq trait satisfaction but the AST retains `ExprKind::Binary` through canonicalization and lowering.

For the reflexive peephole (`x == x → true`), this means:
  - **Primitive types** (int, bool, char, byte, str): reflexive is trivially correct (non-float). Safe.
  - **User types with derived Eq**: structural field-by-field comparison of identical values is trivially true. Safe.
  - **User types with custom Eq**: custom `equals` method could do ANYTHING (side effects, non-reflexive semantics). The reflexive peephole would be UNSOUND.
  - **Float**: `NaN == NaN` is false. UNSOUND.

The safe approach for this plan is narrower: only fire for builtin non-float primitive comparisons (`int`, `bool`, `char`, `byte`, `str`). User structs and custom/derived Eq remain excluded unless a future plan exports explicit Eq provenance into ARC IR. This is conservative but sound, and matches Section 09's execution scope.

- [ ] In the ARC IR normalization pass (or a dedicated peephole pass at step 3b): scan for `ArcInstr::Let { value: ArcValue::PrimOp { op: PrimOp::Binary(Eq), args: [v1, v2] }, .. }` where `v1 == v2`
- [ ] If same ArcVarId AND the type is a builtin non-float primitive → replace value with `ArcValue::Literal(LitValue::Bool(true))`
- [ ] Similarly for `PrimOp::Binary(NotEq)` with same ArcVarId → `LitValue::Bool(false)`
- [ ] For comparison operators with same operand:
  - `x < x` → `false`, `x <= x` → `true`, `x > x` → `false`, `x >= x` → `true`
  - Note: `compare(x, x)` goes through `Apply` (trait method call), not `PrimOp` — skip for now
- [ ] **Soundness guard**: ONLY for builtin non-float primitives. Float NaN makes `NaN == NaN` → false, and user-defined/custom Eq is not proven reflexive by `PrimOp` alone.
- [ ] Write Ori spec test (**include `use std.testing { assert_eq }`**): `assert_eq(actual: (x == x), expected: true)` for int → verify it compiles to constant
- [ ] Write negative test: float `x == x` where `x = 0.0 / 0.0` (NaN) → must remain as PrimOp, not constant true

**Matrix dimensions:**
- Types: `int`, `bool`, `char`, `byte`, `str` (safe — builtin non-float primitives), `float` (excluded — NaN), user struct (excluded — custom Eq possible; future work)
- Patterns: `x == x`, `x != x`, `x < x`, `x <= x`, `x > x`, `x >= x`
- Semantic pin: int `x == x` produces `LitValue::Bool(true)` in ARC IR (only passes with peephole)
- Negative pin: float NaN `x == x` is NOT optimized to true

---

## 05.3 Boolean and Bitwise Simplifications

**File(s):** `compiler/ori_arc/src/aims/normalize/` (ARC IR level peepholes)

Simple boolean/bitwise algebra rewrites that fire at ARC IR level (step 3b).

**CRITICAL IR FACT: `&&` / `||` are NEVER PrimOps.** The `lower_binary()` function in `compiler/ori_arc/src/lower/expr/mod.rs` intercepts `BinaryOp::And` and `BinaryOp::Or` BEFORE they reach PrimOp emission and routes them to `lower_short_circuit_and()` / `lower_short_circuit_or()`, producing control-flow IR (branches). Only bitwise `&`/`|` (`BitAnd`/`BitOr`) reach PrimOp. Therefore:
- `x && true → x` / `x && false → false` / `x || false → x` / `x || true → true` target **non-existent IR** and are **removed from this section**.
- `!!x → x` (`UnaryOp::Not` via PrimOp) IS valid and remains.
- BitAnd/BitOr identity patterns (`x & 0xFF...FF → x`, `x | 0 → x`) are valid PrimOp patterns and are added.

- [ ] `!!x → x`: scan for `ArcValue::PrimOp { op: PrimOp::Unary(Not), args: [y] }` where `y` is defined by `ArcValue::PrimOp { op: PrimOp::Unary(Not), args: [x] }` → replace with `ArcValue::Var(x)`
- [ ] `x & 0xFF...FF → x` (BitAnd with all-ones identity): scan for `PrimOp::Binary(BitAnd)` where one arg is `LitValue::Int(-1)` (all bits set) → replace with `ArcValue::Var(other_arg)`
- [ ] `x | 0 → x` (BitOr with zero identity): scan for `PrimOp::Binary(BitOr)` where one arg is `LitValue::Int(0)` → replace with `ArcValue::Var(other_arg)`
- [ ] `x ^ 0 → x` (BitXor with zero identity): scan for `PrimOp::Binary(BitXor)` where one arg is `LitValue::Int(0)` → replace with `ArcValue::Var(other_arg)`
- [ ] `x & 0 → 0` (BitAnd annihilator): scan for `PrimOp::Binary(BitAnd)` where one arg is `LitValue::Int(0)` → replace with `ArcValue::Literal(LitValue::Int(0))`
- [ ] `x | 0xFF...FF → 0xFF...FF` (BitOr annihilator): scan for `PrimOp::Binary(BitOr)` where one arg is `LitValue::Int(-1)` → replace with `ArcValue::Literal(LitValue::Int(-1))`
- [ ] Write AOT tests for each simplification pattern
- [ ] **Interaction with Section 09**: These peepholes are a subset of the algebraic normalization pass. `!!x → x` is the involutive rewrite for Not. BitAnd/BitOr/BitXor identity elimination falls under Section 09's identity elimination. If Section 09 is done first, this subsection is a no-op. If this section is done first, the peepholes should be placed in a location that Section 09 can reuse.

**Matrix dimensions:**
- Types: `bool` (Not), `int` (BitAnd, BitOr, BitXor)
- Patterns: `!!x`, `x & -1`, `x | 0`, `x ^ 0`, `x & 0`, `x | -1`, `!true`, `!false`
- Semantic pin: `!!flag` compiles to `flag` (no Not instructions in IR)
- Negative pin: `x && true` does NOT appear as PrimOp (it's control-flow IR from short-circuit lowering — no ARC IR peephole possible)

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] `is_readonly_derived()` added to `DerivedTrait`
- [ ] `memory(read)` applied to derived Eq/Comparable/Hashable functions
- [ ] GVN test: repeated derived method call is eliminated
- [ ] Section 09 explicitly owns 05.2/05.3 without duplicate infrastructure in Section 05
- [ ] `timeout 150 ./test-all.sh` green (debug AND release)
- [ ] Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/traits/derive/` passes
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** Derived methods are marked `memory(read)`. LLVM can CSE repeated derived calls. The plan cleanly hands ARC-level reflexive/boolean peepholes to Section 09 without duplicated pass infrastructure. Zero regressions.
