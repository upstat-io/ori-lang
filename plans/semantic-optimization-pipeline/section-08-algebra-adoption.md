---
section: "08"
title: "Algebra Law Adoption"
status: not-started
reviewed: false
goal: "Install algebraic law declarations on stdlib operator traits and impls so the normalization pass has laws to consume"
success_criteria:
  - "Add trait has algebra { supports [associative, commutative]; identity: Zero::zero; inverse: Neg::negate }"
  - "Mul trait has algebra { supports [associative, commutative]; identity: One::one; distributes_over [Add] }"
  - "impl Add for int has laws [associative, commutative, identity, inverse]"
  - "impl Add for str has laws [associative, identity] (not commutative — string concat is not commutative)"
  - "impl Add for float explicitly does NOT declare associative (float reassociation is unsound by default)"
  - "impl Add for float does NOT declare identity (signed zero: -0.0 + 0.0 = +0.0 != -0.0)"
  - "Logical bool `&&` / `||` are NOT modeled as trait laws (they are control-flow IR, not PrimOps, and not optimizable at ARC IR level)"
  - "Satisfies mission criterion: stdlib types have algebraic laws installed"
inspired_by:
  - "Tang 2013 Table 5.1: Monoid models (int+0, int*1, string+'', bool&&true, set∪∅)"
  - "Codex round 4: float reassociation must be opt-in, not default"
depends_on: ["06", "07"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "08.1"
    title: "Operator Trait Algebra Blocks"
    status: not-started
  - id: "08.2"
    title: "Stdlib Impl Laws"
    status: not-started
  - id: "08.3"
    title: "Soundness Validation"
    status: not-started
  - id: "08.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "08.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Algebra Law Adoption

**Status:** Not Started
**Goal:** Install algebra blocks on stdlib operator traits (Add, Mul, Sub, Neg, etc.) and laws on stdlib impls (int, float, str, bool, list) so the normalization pass (Section 09) has concrete laws to consume. This is where Tang's Monoid/Group/Ring models are expressed — not as a rigid hierarchy, but as opt-in law declarations on individual impls.

**Critical soundness decisions (from consensus rounds 3-4):**
- `float: Add` does NOT declare `associative` — float reassociation changes results
- `float: Mul` does NOT declare `associative` — same reason
- `str: Add` declares `associative` and `identity` but NOT `commutative` — string concat is not commutative
- `[T]: Add` declares `associative` and `identity` but NOT `commutative` — list concat is ordered
- `bool: &&` and `||` are hardcoded language operators, not overloadable traits. Section 09 may still normalize them for builtin `bool`, but Sections 07-08 do NOT model them as trait laws.

**Depends on:** Section 06 (runtime identity fixes — must land first so identity laws are sound) and Section 07 (algebra schema must exist).

---

## 08.1 Operator Trait Algebra Blocks

**File(s):** `library/std/prelude.ori`

Add `algebra { }` blocks to operator traits declaring which laws are *available* for impls to opt into.

- [ ] Add to `Add` trait:
  ```ori
  pub trait Add<Rhs = Self> {
      type Output
      @add (self, rhs: Rhs) -> Self.Output

      algebra {
          supports [associative, commutative]
          identity: Zero::zero
          inverse: Neg::negate
      }
  }
  ```
- [ ] Add to `Mul` trait:
  ```ori
  pub trait Mul<Rhs = Self> {
      type Output
      @multiply (self, rhs: Rhs) -> Self.Output

      algebra {
          supports [associative, commutative]
          identity: One::one
          distributes_over [{ inner: Add::add, side: both }]
      }
  }
  ```
- [ ] Add to `Neg` trait:
  ```ori
  pub trait Neg {
      type Output
      @negate (self) -> Self.Output

      algebra {
          supports [involutive]
      }
  }
  ```
- [ ] Add to `Not` trait:
  ```ori
  pub trait Not {
      type Output
      @not (self) -> Self.Output

      algebra {
          supports [involutive]
      }
  }
  ```
- [ ] Sub, Div, Rem, FloorDiv — no algebra blocks (subtraction is not associative, division is not commutative)
- [ ] BitAnd, BitOr, BitXor — algebra blocks for associative + commutative + identity (all/zero/zero respectively)
- [ ] Do NOT add algebra blocks for logical `&&` / `||`: `BinaryOp::And` / `Or` do not map through `BinaryOp::trait_name()` and are validated as hardcoded boolean operators in `ori_types`
- [ ] Verify: all modified trait definitions parse correctly
- [ ] Verify: `timeout 150 ./test-all.sh` passes (algebra blocks with no law consumers are inert)

---

## 08.2 Stdlib Impl Laws

**File(s):** `library/std/prelude.ori` (or wherever stdlib impls live)

Add `laws [ ]` declarations to stdlib operator impls. Each law declaration is the programmer's assertion that the algebraic property holds for this type+operation combination. The compiler trusts this.

**IMPORTANT: Builtin operator impls do NOT exist as `.ori` impl blocks.** Currently, `prelude.ori` defines operator traits (e.g., `trait Add<Rhs = Self>`) but NOT the impls for builtin types (int, float, str, etc.). Builtin operator dispatch is handled entirely by the type checker (via well-known trait bitfield and `infer_binary_op`) and the evaluator/codegen (via hardcoded dispatch). This means we have TWO options for installing laws:

**Option A (`.ori` impl blocks):** Add explicit `impl Add for int { @add (self, rhs: int) -> int = self + rhs; laws [associative, commutative, identity, inverse] }` to `prelude.ori`. This creates circular dispatch (the `+` in the body would resolve to the same `Add::add`). The type checker would need to handle this without infinite recursion. This is the cleaner long-term approach but requires careful compiler work.

**Option B (Rust-side registration):** Install laws during trait registration in `compiler/ori_types/src/check/registration/traits.rs` by populating `ImplEntry.declared_laws` for builtin type operator impls. This avoids `.ori` syntax issues but means laws are hardcoded in Rust rather than declared in the language.

**Recommendation:** Option B for this plan (pragmatic, matches how builtin operator impls already work). Option A is a future improvement when the language supports `intrinsic` impl bodies.

NOTE: All `impl` syntax in `.ori` below uses the CURRENT parser syntax (`impl Trait for Type`), not the planned `impl Type: Trait` syntax (roadmap Section 3.23, not yet implemented). For Option B (Rust-side registration), syntax is irrelevant.

- [ ] **Option B path:** In `compiler/ori_types/src/check/registration/traits.rs`, during builtin operator impl registration, add `declared_laws` for each type:
- [ ] int + Add: `laws [associative, commutative, identity, inverse]`
- [ ] int + Mul: `laws [associative, commutative, identity, distributes_over]`
- [ ] int + Neg: `laws [involutive]` — `-(-x) == x` for all int (checked arithmetic: only panics if original x panics, which -(-x) also would)
- [ ] float + Add: `laws [commutative]` — NOT associative (float reassociation unsound). NOT identity: `-0.0 + 0.0 = +0.0` which differs from `-0.0` (Codex round 2 finding). Float Add gets ONLY commutative.
- [ ] float + Mul: `laws [commutative]` — NOT associative, NOT identity (`0.0 * Inf = NaN`, so even multiplicative identity has edge cases). Float Mul gets ONLY commutative.
- [ ] str + Add: `laws [associative, identity]` — string concat is associative and has empty string identity (after Section 06 runtime fix)
- [ ] [T] + Add: `laws [associative, identity]` — list concat is associative and has empty list identity (after Section 06)
- [ ] bool + Not: `laws [involutive]` — `!!x == x`
- [ ] int + Not (BitNot): `laws [involutive]` — `~~x == x` for bitwise not on int
- [ ] int + BitAnd: `laws [associative, commutative, identity]` — identity is -1 (all bits set)
- [ ] int + BitOr: `laws [associative, commutative, identity]` — identity is 0
- [ ] int + BitXor: `laws [associative, commutative, identity]` — identity is 0
- [ ] Do NOT install laws for logical `&&` / `||`: there is no `And` / `Or` trait in `prelude.ori`, and `BinaryOp::And` / `Or` are lowered to control-flow IR (branches), not PrimOps — they do not participate in the trait-law schema
- [ ] Verify: all law declarations validate against their trait's algebra block
- [ ] Verify: `timeout 150 ./test-all.sh` passes

---

## 08.3 Soundness Validation

**File(s):** Tests in `tests/spec/algebra/` (NEW directory)

Write tests that verify the algebraic properties actually hold for each declared law. These are NOT compiler tests — they're mathematical property tests executed by the Ori runtime.

- [ ] Create `tests/spec/algebra/` directory
- [ ] For each impl with laws, write property tests. **NOTE: every `.ori` test file MUST include `use std.testing { assert_eq }` (or `{ assert, assert_eq }`) — `assert_eq` is NOT in the prelude.**
  - `int_add_assoc.ori`: `assert_eq(actual: (a + b) + c, expected: a + (b + c))` for various a, b, c
  - `int_add_commutative.ori`: `assert_eq(actual: a + b, expected: b + a)`
  - `int_add_identity.ori`: `assert_eq(actual: a + 0, expected: a)` and `assert_eq(actual: 0 + a, expected: a)`
  - `str_add_identity.ori`: `assert_eq(actual: s + "", expected: s)` and `assert_eq(actual: "" + s, expected: s)`
  - `str_add_assoc.ori`: `assert_eq(actual: (a + b) + c, expected: a + (b + c))`
  - `list_add_identity.ori`: list + [] and [] + list
  - `bool_and_identity.ori`: `b && true == b` (runtime semantics validation — NOT an ARC IR optimization target since `&&` is short-circuit control flow, not PrimOp)
  - `bool_or_identity.ori`: `b || false == b` (runtime semantics validation — NOT an ARC IR optimization target since `||` is short-circuit control flow, not PrimOp)
  - `int_neg_involutive.ori`: `-(-x) == x` for various x
  - `bool_not_involutive.ori`: `!!b == b`
- [ ] Negative tests verifying we correctly EXCLUDED:
  - `float_add_not_assoc.ori`: demonstrate `(1e15 + 1.0) + (-1e15) != 1e15 + (1.0 + (-1e15))` — proves associativity doesn't hold for float
  - `str_add_not_commutative.ori`: `"ab" + "cd" != "cd" + "ab"` — proves commutativity doesn't hold for str
- [ ] Verify: `timeout 150 cargo st tests/spec/algebra/` passes
- [ ] **Trust model note**: User-declared `laws []` are trusted without runtime verification (same model as GHC RULES, C++ copy elision). An incorrect `laws [associative]` declaration on a user type will produce silently wrong optimized code. This is by design — runtime verification would eliminate the optimization benefit. Future work (not this plan): a `--verify-algebra` mode that inserts runtime property tests for declared laws during development builds.

**Matrix dimensions:**
- Types: int, float, str, [int], bool
- Laws: associative, commutative, identity, inverse, involutive
- Coverage: positive (law holds) + negative (law correctly excluded)
- Semantic pin: `int_add_assoc.ori` only meaningful with associative law declared
- Negative pin: `float_add_not_assoc.ori` proves exclusion is correct

---

## 08.R Third Party Review Findings

- None.

---

## 08.N Completion Checklist

- [ ] All operator traits have algebra blocks
- [ ] All stdlib impls have appropriate laws (including correct exclusions for float)
- [ ] Soundness test suite covers all declared laws
- [ ] Negative tests prove excluded laws are correctly excluded
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/algebra/` passes (law declarations are inert metadata, must not change observable behavior)
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
  - [ ] This section `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria updated
  - [ ] `index.md` status updated
  - [ ] Section 09 `depends_on` verified
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** Every overloadable stdlib operator impl has explicit algebraic law declarations. Soundness tests verify all declared properties hold. Float is correctly excluded from associativity and identity. Builtin bool logic (`&&` / `||`) is explicitly kept outside the trait-law schema (they are control-flow IR, not PrimOps, and not optimizable at ARC IR level). All tests pass in debug and release.
