---
section: "07"
title: "Algebra Law Schema"
status: not-started
reviewed: false
goal: "Add algebraic law declarations, identity-provider traits, and the exported law/provenance index that Section 09 consumes"
success_criteria:
  - "algebra { } block parses inside trait definitions"
  - "laws [ ] declaration parses inside impl blocks"
  - "AlgebraLaw enum exists in ori_ir with variants: Associative, Commutative, Identity, Inverse, DistributesOver, Involutive"
  - "TraitEntry and ImplEntry store algebraic law metadata queryable at type-check and codegen time"
  - "Zero and One traits defined in prelude.ori with zero() and one() methods"
  - "AlgebraLawIndex (or equivalent exported summary) reaches AimsPipelineConfig with exact operator provenance for Section 09 purity gating"
  - "Satisfies mission criterion: operator traits support algebra {} blocks and impls support laws []"
inspired_by:
  - "DerivedTrait metadata-first pattern (define_derived_traits! macro → strategy dispatch)"
  - "Tang 2013 concept/axiom framework (concept hierarchy with axiom-derived rewrite rules)"
  - "GHC RULES pragmas (trusted rewrite directives, no proof obligation)"
  - "Codex round 4: restricted declarative schema, not general axiom DSL"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "AlgebraLaw IR Types"
    status: not-started
  - id: "07.2"
    title: "Parser: algebra {} and laws []"
    status: not-started
  - id: "07.3"
    title: "Type Checker: Validate and Store Laws"
    status: not-started
  - id: "07.4"
    title: "Zero/One Traits in Prelude"
    status: not-started
  - id: "07.5"
    title: "Algebra Law Export & Operator Provenance"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Algebra Law Schema

**Status:** Not Started
**Goal:** Add the language surface for declaring algebraic properties on traits and opting into specific laws on impls. This follows the DerivedTrait metadata-first pattern: define a fixed `AlgebraLaw` enum, store law declarations as metadata in `TraitEntry`/`ImplEntry`, and let consumers (Section 09 normalization pass, Section 10 advanced rewrites) dispatch on the enum. This section also owns the exported law/provenance summary that the ARC pipeline consumes. It does NOT implement rewrites; it only builds the schema/foundation/export path.

The key design decision (from Codex consensus round 4): base operator traits declare which laws are *available* via `algebra { }`, but impls declare which laws *this type actually satisfies* via `laws [ ]`. `Add` is not always commutative (matrix multiplication is `Mul` but not commutative). The programmer asserts correctness — the compiler trusts and optimizes. This is the same trust model as GHC RULES, C++ copy elision, and Haskell monad laws.

**Success Criteria:**

- [ ] `algebra { supports [associative, commutative]; identity: Zero::zero }` parses inside a trait definition
- [ ] `laws [associative, commutative, identity]` parses inside an impl block
- [ ] `AlgebraLaw` enum with 6 variants exists in `ori_ir`
- [ ] `TraitEntry` has `algebra_laws: Vec<AlgebraDecl>` field
- [ ] `ImplEntry` has `declared_laws: Vec<AlgebraLawKind>` field
- [ ] Laws are validated: impl can only declare laws that the trait's `algebra {}` block lists as supported
- [ ] Zero and One traits exist in prelude.ori and are registered in the type checker

**Context:** Tang's dissertation showed that algebraic concept hierarchies (Magma → SemiGroup → Monoid → Group → Ring) enable generic compiler optimizations. Ori's approach is simpler: instead of a rigid hierarchy, each operator trait declares which algebraic properties it *could* have, and each impl opts into the ones that hold for its type. This gives the compiler the same optimization information without requiring users to navigate a complex trait hierarchy.

**Depends on:** Section 01 (traits/mod.rs must be split before adding fields to TraitEntry and export helpers).

---

## 07.1 AlgebraLaw IR Types

**File(s):** `compiler/ori_ir/src/algebra/mod.rs` (NEW)

Create the IR-level types for algebraic law declarations. These are data structures — no logic, no analysis, just representation.

- [ ] Create `compiler/ori_ir/src/algebra/mod.rs`
- [ ] Define core types:
  ```rust
  /// An algebraic property that a trait operation may have.
  /// Fixed enum — consumers match exhaustively.
  #[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
  pub enum AlgebraLawKind {
      /// op(op(a, b), c) == op(a, op(b, c))
      Associative,
      /// op(a, b) == op(b, a)
      Commutative,
      /// op(a, identity()) == a && op(identity(), a) == a
      /// Provider trait and method specified in AlgebraDecl.
      Identity,
      /// op(a, inverse(a)) == identity()
      /// Inverse trait and method specified in AlgebraDecl.
      Inverse,
      /// outer(a, inner(b, c)) == inner(outer(a, b), outer(a, c))
      /// Inner trait specified in AlgebraDecl.
      DistributesOver,
      /// unary(unary(a)) == a (e.g., -(-x) == x, !!x == x)
      Involutive,
  }

  /// Declaration of an algebraic property on a trait.
  /// Stored in TraitDef.algebra block.
  #[derive(Clone, Eq, PartialEq, Hash, Debug)]
  pub struct AlgebraDecl {
      pub kind: AlgebraLawKind,
      /// For Identity: the trait providing the identity element (e.g., Zero)
      pub provider_trait: Option<Name>,
      /// For Identity: the method name (e.g., "zero")
      pub provider_method: Option<Name>,
      /// For Inverse: the unary trait (e.g., Neg)
      pub inverse_trait: Option<Name>,
      /// For DistributesOver: the inner binary trait (e.g., Add)
      pub inner_trait: Option<Name>,
      /// For DistributesOver: left/right/both sides
      pub side: Option<DistributeSide>,
      pub span: Span,
  }

  #[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
  pub enum DistributeSide {
      Left,   // outer(a, inner(b, c)) == inner(outer(a, b), outer(a, c))
      Right,  // outer(inner(a, b), c) == inner(outer(a, c), outer(b, c))
      Both,   // both sides
  }
  ```
- [ ] Add `mod algebra;` to `compiler/ori_ir/src/lib.rs`
- [ ] Add `pub use algebra::*;` for external consumers
- [ ] Write tests in `compiler/ori_ir/src/algebra/tests.rs`:
  - `AlgebraLawKind` has exactly 6 variants
  - `AlgebraDecl` round-trips through Clone/Eq/Hash
- [ ] Verify: `timeout 150 cargo t -p ori_ir` passes

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.2 Parser: algebra {} and laws []

**File(s):** `compiler/ori_parse/src/grammar/item/trait_def.rs`, `compiler/ori_parse/src/grammar/item/impl_def/mod.rs`

Add parsing for the `algebra { }` block inside trait definitions and `laws [ ]` inside impl blocks.

- [ ] Add `algebra` as a context-sensitive keyword in `compiler/ori_lexer/src/keywords/mod.rs` (if not already present — check first)
- [ ] Add `laws` as a context-sensitive keyword in `compiler/ori_lexer/src/keywords/mod.rs`
- [ ] Add `supports` as a context-sensitive keyword in `compiler/ori_lexer/src/keywords/mod.rs` (used in `algebra { supports [...] }`)
- [ ] In `trait_def.rs` parsing: after parsing trait items (methods, assoc types), look for optional `algebra { }` block:
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
- [ ] Parse `supports [ ]` as list of `AlgebraLawKind` names
- [ ] Parse `identity:`, `inverse:`, `distributes_over` with their provider/method/side metadata
- [ ] Store as `Vec<AlgebraDecl>` on `TraitDef` (add field to `ori_ir/src/ast/items/traits.rs`)
- [ ] In `impl_def/mod.rs` parsing: after parsing impl methods, look for optional `laws [ ]`:
  ```ori
  // NOTE: Parser currently uses `impl Trait for Type` syntax.
  // The `impl Type: Trait` syntax is approved but NOT yet implemented
  // (roadmap Section 3.23). Use current syntax until parser is updated.
  impl Add for int {
      @add (self, rhs: int) -> int = self + rhs
      laws [associative, commutative, identity]
  }
  ```
- [ ] Parse `laws [ ]` as list of `AlgebraLawKind` names
- [ ] Store as `Vec<AlgebraLawKind>` on `ImplDef` (add field to `ori_ir/src/ast/items/traits.rs`)
- [ ] Write parser tests: trait with algebra block parses, impl with laws parses, empty algebra/laws parses, invalid law name produces error
- [ ] Verify: `timeout 150 cargo t -p ori_parse` passes

**Sync points:** `TraitDef` gains `algebra: Vec<AlgebraDecl>` field; `ImplDef` gains `declared_laws: Vec<AlgebraLawKind>` field. Both are in `ori_ir/src/ast/items/traits.rs`. All consumers of these structs must handle the new fields (Debug impl, visitors, serialization if any).

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.3 Type Checker: Validate and Store Laws

**File(s):** `compiler/ori_types/src/check/registration/traits.rs`, `compiler/ori_types/src/registry/traits/` (split in Section 01)

During trait/impl registration, validate and store algebraic law metadata.

- [ ] Add `algebra_laws: Vec<AlgebraDecl>` field to `TraitEntry` (in the resolution submodule from Section 01 split)
- [ ] Add `declared_laws: Vec<AlgebraLawKind>` field to `ImplEntry`
- [ ] During `register_trait()`: copy algebra declarations from `TraitDef` into `TraitEntry`
- [ ] During `register_impl()`: copy laws from `ImplDef` into `ImplEntry`
- [ ] Validate: each law in `ImplEntry.declared_laws` must be in the trait's `algebra_laws` supported list. If not → error (new diagnostic code, e.g., E2050 "law 'commutative' not supported by trait 'Add'")
- [ ] Validate: if impl declares `Identity` law, the trait must have an identity provider (`provider_trait` + `provider_method`), and the impl's self type must implement the provider trait. If not → error.
- [ ] Add query method: `TraitRegistry::has_law(&self, impl_idx: usize, law: AlgebraLawKind) -> bool` — checks if a specific impl declares a specific law
- [ ] Add query method: `TraitRegistry::identity_provider(&self, trait_idx: Idx) -> Option<(Name, Name)>` — returns (trait_name, method_name) for the identity element
- [ ] Write type checker tests:
  - Valid: impl with laws that match trait's algebra block → compiles
  - Invalid: impl with law not in trait's algebra → error E2050
  - Invalid: impl with Identity law but type doesn't implement Zero → error
- [ ] Verify: `timeout 150 cargo t -p ori_types` passes

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.4 Zero/One Traits in Prelude

**File(s):** `library/std/prelude.ori`

Add `Zero` and `One` traits that serve as identity element providers for additive and multiplicative operations.

- [ ] Add to prelude.ori:
  ```ori
  pub trait Zero {
      @zero () -> Self
  }

  pub trait One {
      @one () -> Self
  }
  ```
- [ ] Implement for built-in types (in prelude or stdlib). NOTE: uses current parser syntax (`impl Trait for Type`), not planned `impl Type: Trait`:
  ```ori
  impl Zero for int { @zero () -> int = 0 }
  impl One for int { @one () -> int = 1 }
  impl Zero for float { @zero () -> float = 0.0 }
  impl One for float { @one () -> float = 1.0 }
  impl Zero for bool { @zero () -> bool = false }  // additive identity for Or
  impl One for bool { @one () -> bool = true }      // multiplicative identity for And
  ```
- [ ] Add `Zero` and `One` to the well-known trait bitfield (`compiler/ori_types/src/check/well_known/trait_set.rs`) — next available bits are 28 (Zero) and 29 (One) after FORMATTABLE=27. Update `COUNT` to 30. Add entries to `trait_name_to_bit()` match. **NOTE: verify bit indices at implementation time — other work may have shifted them.**
- [ ] Write failing spec tests FIRST (TDD, **include `use std.testing { assert_eq }`**): `assert_eq(actual: int.zero(), expected: 0)`, `assert_eq(actual: float.one(), expected: 1.0)` — these fail before traits are registered
- [ ] Verify spec tests pass after registration
- [ ] Register in evaluator for interpreter execution: `Zero` and `One` are standard traits with regular methods — the evaluator dispatches to them through existing trait method resolution (`eval_method_call`). No special registration needed beyond type-checker registration. Verify by running spec tests with `cargo st`.
- [ ] Register in LLVM codegen for AOT execution: Same as evaluator — `Zero::zero()` and `One::one()` are regular associated functions compiled through the standard trait impl codegen path. No special codegen needed. Verify by running spec tests with `--backend=llvm`.
- [ ] Verify: `timeout 150 ./test-all.sh` passes

**This section provides to Section 08:** The algebra schema (types, parser, registration) and Zero/One traits. Section 08 installs actual algebra blocks on stdlib operator traits and laws on stdlib impls.

- [ ] **Subsection close-out (07.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.5 Algebra Law Export & Operator Provenance

**File(s):** `compiler/ori_types/src/registry/traits/`, `compiler/ori_llvm/src/codegen/function_compiler/`, `compiler/ori_arc/src/pipeline/`

**Critical export problem:** Section 09 runs in `ori_arc`, but the facts it needs live elsewhere:
- law declarations live in `ori_types::TraitRegistry`
- exact operator dispatch for user-defined types is only materialized later when `ori_llvm` resolves type-qualified methods
- `run_arc_pipeline()` / `AimsPipelineConfig` currently carry contracts, pool, interner, and builtins only

This subsection owns the bridge. Section 09 must not attempt to reconstruct callee identity from `PrimOp` alone.

- [ ] Create `AlgebraLawIndex` (or equivalent pair of exported structs) in `ori_ir` / shared compiler output — a compact, serializable summary:
  ```rust
  pub enum AlgebraPurity {
      BuiltinPure,
      CheckContracts { contract_key: Name },
  }

  pub struct AlgebraImplSummary {
      pub laws: Vec<AlgebraLawKind>,
      pub purity: AlgebraPurity,
  }

  pub struct AlgebraLawIndex {
      pub impls: FxHashMap<(Idx, Name), AlgebraImplSummary>,
      pub trait_decls: FxHashMap<Name, Vec<AlgebraDecl>>,
  }
  ```
- [ ] Add `TraitRegistry::build_algebra_law_index(&self) -> AlgebraLawIndex` in `ori_types`
- [ ] Seed builtin/stdlib operator entries with `BuiltinPure`; do NOT require Section 09 to look up contracts for primitive operators
- [ ] For user-defined operators, populate `CheckContracts { contract_key }` from the exact resolved impl identity during `FunctionCompiler` setup, where the type-qualified method dispatch tables already exist
- [ ] Do NOT derive or mangle operator callee names inside `ori_arc`; all exact-provenance work happens before the ARC pipeline call
- [ ] Thread `AlgebraLawIndex` through the compilation pipeline:
  - `ori_types` type checker output → `ori_llvm` `FunctionCompiler` (alongside `aims_contracts`)
  - `FunctionCompiler` → `run_arc_pipeline()` → `AimsPipelineConfig` → `normalize_algebra()`
- [ ] Verify conservative fallback: if a user-defined operator has no exact provenance entry, Section 09 skips the rewrite instead of guessing
- [ ] Without this plumbing, Section 09 is blocked — the normalization pass has no sound way to query both laws and purity for user-defined operators

- [ ] Verify: `timeout 150 ./test-all.sh` passes

- [ ] **Subsection close-out (07.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-07.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 07.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 07.R Third Party Review Findings

- None.

---

## 07.N Completion Checklist

- [ ] `AlgebraLawKind` enum with 6 variants in `ori_ir/src/algebra/mod.rs`
- [ ] `AlgebraDecl` type with provider/inverse/distributes metadata
- [ ] `algebra { }` block parses in trait definitions
- [ ] `laws [ ]` declaration parses in impl blocks
- [ ] `TraitEntry.algebra_laws` and `ImplEntry.declared_laws` populated during registration
- [ ] Validation: laws must match trait's supported set
- [ ] `TraitRegistry::has_law()` and `identity_provider()` query methods work
- [ ] `Zero` and `One` traits exist and work in both eval and LLVM
- [ ] `AlgebraLawIndex` built from `TraitRegistry`, enriched with operator provenance, and threaded to `AimsPipelineConfig`
- [ ] `timeout 150 ./test-all.sh` green (debug AND release — `cargo b --release && timeout 150 ./test-all.sh`)
- [ ] Dual-exec parity: `diagnostics/dual-exec-verify.sh tests/spec/` passes (new syntax and traits must not change observable behavior)
- [ ] Plan annotation cleanup
- [ ] **Plan sync** — update plan metadata
  - [ ] This section `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria updated
  - [ ] `index.md` status updated
  - [ ] Sections 08/09 `depends_on` verified
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** Algebra blocks parse, validate, and store correctly. Zero/One traits are usable in both eval and LLVM. Section 09 receives an exported law/provenance index instead of reconstructing operator identity from `PrimOp`. All tests pass in debug and release.
