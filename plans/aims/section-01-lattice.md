---
section: "01"
title: "Unified Lattice Design"
status: in-progress
goal: "Define the AimsState product lattice with correct join, meet, and transfer functions"
inspired_by:
  - "Perceus lambda_1 calculus (Reinking et al., PLDI 2021)"
  - "GHC demand analysis (Sergey et al., POPL 2014)"
  - "Lean 4 borrow inference (Ullrich & de Moura, IFL 2019)"
  - "Quantitative Type Theory (Atkey, LICS 2018)"
  - "Linearity vs Uniqueness (Marshall et al., ESOP 2022)"
  - "Oxidizing OCaml (Lorenzen et al., ICFP 2024) — locality dimension"
depends_on: []
sections:
  - id: "01.1"
    title: "The Product Lattice"
    status: complete
  - id: "01.2"
    title: "Access and Consumption Dimensions"
    status: complete
  - id: "01.3"
    title: "Uniqueness Dimension"
    status: complete
  - id: "01.3a"
    title: "Dimension Interactions"
    status: complete
  - id: "01.4"
    title: "Cardinality Dimension"
    status: complete
  - id: "01.4a"
    title: "Locality Dimension"
    status: complete
  - id: "01.4b"
    title: "ShapeClass Dimension"
    status: complete
  - id: "01.4c"
    title: "EffectClass Dimension"
    status: complete
  - id: "01.5"
    title: "Join and Transfer Functions"
    status: complete
  - id: "01.6"
    title: "Lattice Properties and Proofs"
    status: complete
  - id: "01.7"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Unified Lattice Design

**Status:** Complete
**Goal:** Define an `AimsState` type that encodes access class, consumption,
uniqueness, cardinality, locality, shape class, and effect class in a single product lattice,
with formally correct join and transfer functions. The lattice must be finite-height
(guaranteeing convergence), monotonic under all transfer functions, and sound with
respect to concrete RC semantics.

**Context:** The current `ori_arc` maintains 6+ separate data structures to track
overlapping properties of the same variables: `Ownership` (Owned/Borrowed),
`DerivedOwnership` (Owned/BorrowedFrom/Fresh), `Uniqueness` (Unique/MaybeShared/Shared),
`LiveSet` (`FxHashSet<ArcVarId>`), `CowMode` (Dynamic/StaticUnique/StaticShared),
`DropHints`. These encode different projections of a single underlying state —
"what is the ownership situation of this variable at this program point?" — but they
can't communicate. AIMS unifies them into one lattice element per variable per program
point.

The original plan focused on three dimensions (ownership, uniqueness, cardinality).
The expanded lattice adds three more (locality, shape class, effect class) to support
future stack allocation, FIP certification, and representation optimization — all
reading from the same unified fact structure. The new dimensions begin conservative
in v1 and do not need to be precise for the initial AIMS replacement to work.

**Reference implementations:**
- **Lean 4** `src/Lean/Compiler/IR/Borrow.lean`: SCC-based borrow inference with
  monotonic `Borrowed → Owned` promotion — the ownership dimension
- **Koka** `src/Core/CheckFBIP.hs`: Linear resource calculus with dup/drop as
  structural rules — the substructural mode dimension
- **GHC** `compiler/GHC/Core/Opt/DmdAnal.hs`: Demand analysis with `{Absent, Once, Many}`
  cardinality — the cardinality dimension
- **OxCaml** (Lorenzen et al., ICFP 2024): Modal memory management with affinity,
  uniqueness, and locality as mode axes — the locality dimension

---

## 01.1 The Product Lattice

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`, `compiler/ori_arc/src/aims/lattice/dimensions.rs`

> **Warning: File size.** This section defines 8 enums (7 dimensions + ReuseCtorKind) + 1 struct +
> join/transfer functions + canonicalization + tests. **Actual split (implemented):**
> `lattice/dimensions.rs` for dimension enums (AccessClass, Consumption, Cardinality, Uniqueness,
> Locality, ShapeClass, ReuseCtorKind), `lattice/mod.rs` for AimsState, EffectClass, SizeClass,
> BorrowSource, join/canonicalize, `transfer/mod.rs` for transfer functions.
> Tests in `lattice/tests.rs` and `transfer/tests.rs`.

The core `AimsState` is a product of seven dimensions, each a small finite lattice.
The product lattice inherits join/meet componentwise. The four core dimensions
(access, consumption, uniqueness, cardinality) drive the fixed-point solver and RC
emission. The three auxiliary dimensions (locality, shape, effect) are conservative
in v1 and provide the architectural foundation for future optimizations (stack
allocation, FIP certification, representation optimization) without requiring a
second pass. v1 treats all dimensions uniformly in the worklist
The core/auxiliary distinction is for documentation
and future optimization — it does not affect v1 solver behavior.
(Originally an early design decision, now superseded by this section.)

- [x] Define `AimsState` as a struct with seven fields:
  ```rust
  /// Unified ownership state for a variable at a program point.
  ///
  /// Product of seven dimensions. Join is componentwise. Transfer
  /// functions update one or more dimensions simultaneously.
  ///
  /// **Core dimensions** (drive the fixed point — worklist always reacts
  /// to changes in these):
  ///   access, consumption, cardinality, uniqueness
  ///
  /// **Auxiliary dimensions** (conservative in v1 — all dimensions are
  /// treated identically by the worklist, with no selective reprocessing.
  /// v1 does not distinguish core from auxiliary for iteration purposes):
  ///   locality, shape, effect
  ///
  /// This is a product lattice with a core/auxiliary distinction for documentation
  and future optimization, but v1 treats all dimensions uniformly in the
  worklist (historical design decision).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub struct AimsState {
      /// Aliasing: owned allocation vs borrowed view.
      pub access: AccessClass,
      /// Substructural: how the value is consumed.
      pub consumption: Consumption,
      /// Forward usage count.
      pub cardinality: Cardinality,
      /// Runtime reference count knowledge.
      pub uniqueness: Uniqueness,
      /// Escape analysis (auxiliary, conservative in v1).
      pub locality: Locality,
      /// Structural shape (auxiliary, conservative in v1).
      pub shape: ShapeClass,
      /// Memory effects (auxiliary, conservative in v1).
      pub effect: EffectClass,
  }
  ```

- [x] Define `AimsState::TOP` (most conservative: `Owned, Unrestricted, Many, Shared,
  Unknown, NonReusable, EffectClass::ALL`)
- [x] Define `AimsState::BOTTOM` (most optimistic: `Borrowed, Dead, Absent, Unique,
  BlockLocal, NonReusable, EffectClass::NONE`).
  Note: `Dead` consumption and `Absent` cardinality are redundant (both mean "not live").
  This is intentional for the product lattice (componentwise bottom); infeasible
  states like `(*, Dead, Once, *)` are documented in 01.6.
  Note: ShapeClass uses `NonReusable` as bottom because ShapeClass is a flat lattice
  where the initial value is set by the defining instruction's transfer function —
  BOTTOM is never used as an initial state for real variables.
- [x] Define `AimsState::SCALAR` (special: no RC ever, terminates analysis).
  **Note:** `SCALAR` is an analysis fast-path, not a lattice element. Variables
  classified as scalar are excluded from analysis entirely — they are never
  placed on the worklist and never participate in join or transfer operations.
  Lattice property proofs (idempotence, monotonicity, etc.) apply only to
  the product lattice states. `SCALAR` is an implementation optimization that
  short-circuits the analysis for non-RC types.
- [x] Define `AimsState::FRESH` — `(Owned, Linear, Once, Unique, FunctionLocal,
  NonReusable, EffectClass::NONE)` convenience base for freshly constructed values.
  Note: `FRESH` uses `NonReusable` as a default — individual transfer functions
  (e.g., `Construct`) override `shape` with the appropriate `ShapeClass` derived
  from the constructor kind. `FRESH` is a convenience starting point, not the final
  state for any instruction that produces an allocation.
- [x] Implement `AimsState::join(&self, other: &Self) -> Self` (componentwise join)
- [x] Implement `AimsState::canonicalize(&mut self)` — enforce feasibility invariants
  (historical design decision).
  **Call sites**: canonicalize is called in exactly two places:
  1. At the END of every `join` operation (inside `AimsState::join`)
  2. At the END of every transfer function application (inside each transfer fn)
  This ensures no infeasible state ever propagates into the state map or across
  a worklist iteration. The worklist change-detection compares post-canonicalized
  states, so canonicalization cannot cause spurious non-convergence.

  **Exhaustive canonicalization rules**:
  - `consumption == Dead` → force `cardinality = Absent` (dead = zero uses)
  - `cardinality == Absent` → force `consumption = Dead` (zero uses = dead)
  - `consumption == Linear` + `cardinality == Absent` → collapse to
    `consumption = Dead, cardinality = Absent` (infeasible: linear requires use)
  - `uniqueness == Shared` + `shape == ReusableCtor(_)` → collapse shape to
    `NonReusable` (shared values cannot be reused)
  - `shape == NonReusable` → no reuse candidate regardless of other dimensions
  - ~~locality == BlockLocal + access == Owned + function return → promote
    locality to FunctionLocal~~ — **Removed**: this rule requires knowing we are
    at a return point, which is not derivable from the state value alone.
    `canonicalize()` must be a pure function on `AimsState`. Locality promotion
    at return points is handled by the `Return` terminator's transfer function
    in Section 02 (`transfer/mod.rs`), not by `canonicalize()`.
  **Note:** Effect promotion (`Pure` → `MayAlloc` etc.) is handled by
  transfer functions in `transfer/mod.rs`, not by `canonicalize()`.
  `canonicalize()` is a pure function on `AimsState` — it never reads
  control-flow position, instruction context, or caller knowledge.

  **NOT canonicalized** (valid states that look surprising but are correct):
  - `(Borrowed, Dead, Absent, Unique)` — an expired borrow of a unique source;
    no action needed, the source handles cleanup
  - `(Owned, Linear, Once, MaybeShared)` — owned value used once but might be
    shared; valid during iteration (may narrow to Unique or widen to Affine)

  **Note on `(Owned, Linear, *, Shared)` from feasibility table (01.6)**:
  This state CAN appear legitimately in converged output. A function that
  receives a shared parameter (RC > 1 — Shared) but uses it only once on a
  given path (Linear) is a valid converged state. The emission handles it
  correctly: `rc_dec` at last use (cleaning up this path's reference), no
  `rc_inc` (only one use on this path). This is precisely the (Linear, Shared)
  combination from Marshall et al.'s analysis: linearity (future demand) and
  uniqueness (past aliasing) are independent properties. A shared value used
  linearly still needs `rc_dec` at its single use point — the other references
  exist but are not this code path's concern.
  (See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))
  Canonicalizing this state away would be unsound — it would either lose the
  consumption information (widening Linear to Unrestricted) or lose the
  uniqueness information (narrowing Shared to Unique). Neither is correct.
  The state is NOT canonicalized; it is valid as-is.
- [x] Implement `AimsState::is_rc_needed(&self) -> bool` — true unless Dead or Scalar,
  AND access is `Owned` (borrowed values never need RC)
- [x] Implement `AimsState::needs_cow_check(&self) -> bool` — true only for MaybeShared
- [x] Implement `AimsState::is_reuse_candidate(&self) -> bool` — true for consumed owned
  values with a reusable shape AND uniqueness `Unique` or `MaybeShared`.
  `Unique` sources get static reuse (direct Reset); `MaybeShared` sources get
  dynamic reuse (IsShared check + conditional). `Shared` sources are never
  reuse candidates.
- [x] Implement `AimsState::is_local(&self) -> bool` — true for FunctionLocal or BlockLocal
- [x] Implement `AimsState::from_arc_class(ArcClass) -> Self` — map `Scalar` → `SCALAR`,
  `DefiniteRef` → `TOP` (conservative starting point for analysis),
  `PossibleRef` → `TOP` (conservative; encountering PossibleRef post-mono is a compiler bug
  but analysis must not crash)

---

## 01.2 Access and Consumption Dimensions

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Tracks whether a value is consumed, shared, or dead, and separately whether it
is an owned value or a borrowed view. This replaces both `Ownership` and
`DerivedOwnership` from the current system.

Based on Chirimar et al.'s insight that RC operations ARE structural rules:
- `rc_inc` = contraction (duplication) — value used more than once
- `rc_dec` = weakening (drop) — value goes out of scope unused

**Design decision:** `Borrowed` is NOT part of
the ordered consumption lattice. Borrowed is an alias/access property, not a
consumption mode. Placing it in the same ordering as `Dead`/`Linear`/`Affine`/
`Unrestricted` causes `join(Linear, Borrowed)` to lose consumption information.
Instead, borrowing is tracked via a separate `AccessClass` dimension.

- [x] Define `AccessClass` enum (the aliasing dimension):
  ```rust
  /// Whether a value is an owned allocation or a borrowed view.
  ///
  /// This is orthogonal to consumption: a borrowed value may be used
  /// Once or Many times; an owned value may be Linear or Unrestricted.
  /// RC emission depends on access: only Owned values carry RC obligations.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum AccessClass {
      /// Temporary view of another value. No RC operations.
      /// The source value's state is unaffected by the borrow.
      Borrowed,
      /// The value owns its allocation. RC operations may be needed.
      Owned,
  }
  ```

- [x] Define `Consumption` enum (the substructural dimension):
  ```rust
  /// How a value participates in the substructural discipline.
  ///
  /// Ordered: Dead < Linear < Affine < Unrestricted
  /// (Dead is bottom, Unrestricted is top)
  ///
  /// **Temporal direction: FUTURE-facing.** Consumption encodes what structural
  /// rules this value obeys going forward (will it be used once? may it be
  /// dropped? freely copied?). This is NOT to be confused with `Uniqueness`,
  /// which is PAST-facing (has this value been duplicated?). See Marshall et
  /// al. (ESOP 2022) for the formal distinction. A value can be `Linear`
  /// (future: consumed once) and `Shared` (past: already aliased) — these
  /// are independent axes.
  ///
  /// Note: Borrowed is NOT in this ordering (see AccessClass).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum Consumption {
      /// Value is not live at this point. No RC operations needed.
      Dead,
      /// Value is consumed exactly once (moved). No RC inc/dec needed.
      /// Corresponds to a linear use — the value is transferred, not copied.
      Linear,
      /// Value may be dropped without use (e.g., in an else branch).
      /// RC dec may be needed, but no RC inc.
      Affine,
      /// Value may be freely copied and dropped. Full RC required.
      Unrestricted,
  }
  ```

- [x] Implement join rules:
  - `AccessClass::join`: `Owned` if either side is `Owned`; `Borrowed` only if both are `Borrowed`
  - `Consumption::join`: componentwise max with ordering `Dead < Linear < Affine < Unrestricted`
  - Example: `join((Owned, Linear), (Borrowed, Linear)) = (Owned, Linear)` — consumption survives
- [x] Map to current system: `Owned` → `(Owned, Linear)` or `(Owned, Affine)`,
  `Borrowed` → `(Borrowed, Linear)` or `(Borrowed, Affine)`
- [x] RC emission rule: emit `RcInc`/`RcDec` only when `access == Owned`.
  Borrowed values never own the RC obligation.

- [x] Define `BorrowSource` for provenance tracking (sparse side table):
  ```rust
  /// Tracks where a borrowed value comes from. Stored in a sparse side
  /// table, NOT in the finite lattice (provenance is auxiliary data).
  pub enum BorrowSource {
      /// Known exact source variable.
      Exact(ArcVarId),
      /// Multiple sources or unknown origin.
      Unknown,
  }
  ```
- [x] Provenance rules:
  - `Project(dst, src, ...)`: set `dst.access = Borrowed`, `borrow_source[dst] = Exact(src)`,
    `dst.uniqueness = src.uniqueness` (uniqueness-preserving borrow)
  - Join of two borrowed values: same source → keep `Exact(source)`;
    different source → promote to `Unknown`
  - Join of borrowed and owned: `access = Owned`, clear borrow provenance

---

## 01.3 Uniqueness Dimension

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Tracks whether the runtime reference count is provably 1, provably > 1, or unknown.
This replaces the current `Uniqueness` enum and `CowMode`.

Key insight from Marshall et al.: uniqueness is a PAST guarantee ("has not been
duplicated"), distinct from linearity which is a FUTURE guarantee ("will be consumed
once"). A value can be unique but unrestricted (many future uses, but only one
current reference).

- [x] Define `Uniqueness` enum:
  ```rust
  /// Runtime reference count knowledge.
  ///
  /// Ordered: Unique < MaybeShared < Shared
  /// (Unique is bottom/most-optimistic, Shared is top/most-conservative)
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum Uniqueness {
      /// Provably RC == 1. COW fast path, reset/reuse candidate.
      /// Sources: Construct, Fresh allocation, COW result (both paths
      /// produce unique output), return from function with unique summary.
      Unique,
      /// Unknown RC. Runtime check needed for COW.
      MaybeShared,
      /// Provably RC > 1. COW always takes slow path.
      Shared,
  }
  ```

- [x] Implement `Uniqueness::join` — max of the two (most conservative)
- [x] Note: COW operations ALWAYS produce `Unique` output (both fast and slow paths
  allocate a uniquely-owned result)

**Linearity vs. Uniqueness — full property mapping (Marshall et al., ESOP 2022):**
The three-way split of resource properties in AIMS corresponds to Marshall et al.'s
analysis: `Consumption` + `Cardinality` encode linearity (future demand — what
structural rules does this value obey going forward?), while `Uniqueness` encodes
uniqueness (past aliasing — has this value been duplicated?). These are different
type-theoretic properties with different information flow: linearity restricts what
CAN be done with a value, uniqueness guarantees what HAS been done to a value. They
interact (a unique value used linearly needs no RC at all) but must never be
conflated (a shared value used linearly still needs `rc_dec` at its single use
point; a unique value used many times needs `rc_inc` at each additional use).
(See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))

---

## 01.3a Dimension Interactions

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs` (within `canonicalize()`)

The seven dimensions interact in specific ways. Canonicalization enforces these
invariants after every join and transfer:

**Consumption × Uniqueness — central insight (Marshall et al., ESOP 2022):**
The interaction between Consumption (future-facing: linearity) and Uniqueness
(past-facing: aliasing) encodes the paper's central result — these are independent
type-theoretic properties that interact but must not be conflated. Each combination
is meaningful and drives a different optimization:
- **Linear + Unique**: No RC operations at all. The value has a single reference
  (Unique) and will be consumed exactly once (Linear). This is the ideal case.
- **Linear + Shared**: `rc_dec` at last use, no `rc_inc`. The value has multiple
  references but this code path consumes it only once. Still needs cleanup.
- **Unrestricted + Unique**: `rc_inc` at each additional use, no COW check.
  The value is sole-referenced but will be used multiple times. COW is always
  fast-path (unique → mutate in place).
- **Unrestricted + Shared**: Full ARC treatment. Multiple references, multiple
  uses. `rc_inc` at each use, COW needs runtime check.
(See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))

- [x] **Access × Consumption**:
  - `access == Borrowed` → `is_rc_needed()` always returns false regardless of consumption.
    The access check is the primary RC filter; borrowed values never carry RC obligations.
    The consumption dimension for borrowed values is retained without canonicalization
    because it still carries useful information: `Borrowed + Linear` means the borrow
    is consumed once (informative for callee demand inference), `Borrowed + Unrestricted`
    means the borrow is used freely (valid — no RC contradiction because `is_rc_needed()`
    gates all RC emission on `access == Owned`).

- [x] **Consumption × Cardinality**:
  - `consumption == Dead` → `cardinality = Absent` (dead means zero future uses)
  - `cardinality == Absent` → `consumption = Dead` (zero uses means dead)
  - `consumption == Linear` + `cardinality == Absent` is infeasible → collapse to Dead

- [x] **Access × Uniqueness**:
  - `access == Borrowed` preserves the source's uniqueness (key insight: borrowing
    doesn't duplicate the reference, so the source stays unique if it was unique)
  - This enables uniqueness-preserving borrows: `Project(dst, src)` with `src: Unique`
    gives `dst: (Borrowed, *, *, Unique)` — the borrow knows the source is unique

- [x] **Uniqueness × ShapeClass**:
  - `uniqueness == Shared` → `shape` irrelevant for reuse (shared values can't be reused)
  - `shape == ReusableCtor` + `uniqueness == Unique` → static reuse candidate (direct Reset)
  - `shape == ReusableCtor` + `uniqueness == MaybeShared` → dynamic reuse candidate
    (IsShared check + conditional: fast path reuses, slow path allocates fresh)

- [x] **Locality × Access**:
  - `access == Borrowed` → locality inherits from the source (borrow doesn't change
    where the value lives)
  - `locality == HeapEscaping` has no impact on RC correctness (just on future stack
    allocation optimization)

- [x] **EffectClass × FipContract**:
  - A function with any `MayAlloc` effect on the fast path cannot be `FipContract::Certified`
  - This interaction is computed in Section 03 (interprocedural), not in the lattice itself

---

## 01.4 Cardinality Dimension

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Tracks how many times a value is used going forward. This is the novel dimension
borrowed from GHC's demand analysis that the current `ori_arc` does not have.

This enables new optimizations:
- `Absent` values can have RC dec elided (they're dead — liveness subsumption)
- `Once` values don't need RC inc at the use site (consumed, not shared)
- `Many` values need full RC treatment

- [x] Define `Cardinality` enum:
  ```rust
  /// Forward usage count for a value.
  ///
  /// Ordered: Absent < Once < Many
  /// (Absent is bottom, Many is top)
  ///
  /// Inspired by GHC's demand analysis (Sergey et al., POPL 2014).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum Cardinality {
      /// Value is never used after this point. Dead — no RC needed.
      /// Equivalent to "not in live set" in current liveness analysis.
      Absent,
      /// Value is used exactly once. Linear consumption — no RC inc.
      /// The single use consumes the value.
      Once,
      /// Value is used multiple times (or in a loop). RC inc needed
      /// at each use site beyond the first.
      Many,
  }
  ```

- [x] Implement `Cardinality::join` — max of the two
- [x] Implement `Cardinality::seq_add` — for sequential composition along one path:
  `Absent + x = x`, `Once + Once = Many`, `Many + _ = Many`.
  See also `Cardinality::alt_join` (= `max`) in Section 02.2a for alternative control flow.
  **QTT correspondence:** `(Cardinality, seq_add, Absent)` is a commutative monoid
  with absorbing element `Many`, directly analogous to QTT's 0-1-omega resource
  semiring (Atkey, LICS 2018). `seq_add` corresponds to QTT's resource accumulation
  (+). `seq_add` distributes over `alt_join` — the key soundness property for
  fixed-point analysis over CFGs with diamonds.
  (See: [Literature Review §07 — QTT](../aims-literature-review/section-07-quantitative-type-theory.md))
- [x] Test semiring laws for `seq_add` and `alt_join` (Decision 2):
  - **Associativity of seq_add**: `a.seq_add(b.seq_add(c)) == a.seq_add(b).seq_add(c)`
    for all (a, b, c) — 27 cases exhaustive
  - **Commutativity of seq_add**: `a.seq_add(b) == b.seq_add(a)` for all pairs — 9 cases
  - **Identity of seq_add**: `a.seq_add(Absent) == a` and `Absent.seq_add(a) == a`
  - **Associativity of alt_join**: `a.alt_join(b.alt_join(c)) == a.alt_join(b).alt_join(c)`
  - **Idempotence of alt_join**: `a.alt_join(a) == a` (max is idempotent)
    **QTT correspondence:** `alt_join` is the lattice lub (idempotent), NOT QTT semiring
    addition. It combines usages from mutually exclusive paths (branch join), not
    sequential accumulation. The idempotence distinguishes it from `seq_add`.
    (See: [Literature Review §07 — QTT](../aims-literature-review/section-07-quantitative-type-theory.md))
  - **Distributivity**: `a.seq_add(b.alt_join(c)) == a.seq_add(b).alt_join(a.seq_add(c))`
    — required for sound fixed-point over CFGs with diamonds. 27 cases exhaustive.
  - **Absorbing element**: `Many.seq_add(x) == Many` for all x (Many absorbs)
  - **Positivity**: `a.seq_add(b) == Absent` implies both `a == Absent` and `b == Absent`
    (no non-trivial cancellation — usage can only accumulate, never cancel out)
  - **Right-distributivity**: `(a.alt_join(b)).seq_add(c) == a.seq_add(c).alt_join(b.seq_add(c))`
    — symmetric to left-distributivity; required for commutativity + distributivity
    consistency. 27 cases exhaustive.

---

## 01.4a Locality Dimension

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Tracks whether a value escapes its defining scope. This dimension does not affect
RC emission in v1 but provides the architectural foundation for future stack
allocation hints (Stage 4 in the implementation sequence).

- [x] Define `Locality` enum:
  ```rust
  /// Escape analysis: does this value outlive its defining scope?
  ///
  /// Ordered: BlockLocal < FunctionLocal < HeapEscaping < Unknown
  /// (BlockLocal is most-optimistic, Unknown is most-conservative)
  ///
  /// Justified by the OxCaml locality mode (Lorenzen et al., ICFP 2024):
  /// locality as an inferable mode axis for safe stack allocation.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub enum Locality {
      /// Value does not escape its defining basic block.
      BlockLocal,
      /// Value does not escape its defining function.
      FunctionLocal,
      /// Value may escape to the heap (stored in a data structure, returned, etc.)
      HeapEscaping,
      /// Unknown — conservative default.
      Unknown,
  }
  ```

- [x] Implement `Locality::join` — max of the two (most conservative)
- [x] **v1 behavior**: Initialize all non-scalar variables to `Unknown`. Refine to
  `FunctionLocal` only when provable (e.g., not returned, not stored in a captured
  closure, not passed to an owned-consuming callee). This is safe — overapproximation.

---

## 01.4b ShapeClass Dimension

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Tracks the structural shape of a value for reuse compatibility and future
representation optimization. Conservative in v1.

- [x] Define `ShapeClass` enum:
  ```rust
  /// Structural shape classification for reuse compatibility.
  ///
  /// Forms a **flat lattice** with `NonReusable` as the top element:
  /// any two distinct non-`NonReusable` values join to `NonReusable`.
  /// This is a valid lattice (idempotent, commutative, associative join)
  /// with chain height 1 (any value reaches `NonReusable` in one step).
  /// The product lattice's componentwise join is well-defined because
  /// ShapeClass::join is a valid join operation.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum ShapeClass {
      /// Not a candidate for allocation reuse. Top element of the flat lattice.
      NonReusable,
      /// A constructor allocation that may be reusable.
      /// `ReuseCtorKind` identifies struct vs enum variant for size matching.
      ReusableCtor(ReuseCtorKind),
      /// A collection buffer (list, map, set) — separate reuse path via
      /// CollectionReuse instructions.
      CollectionBuffer,
      /// A constructor-context hole (Stage 3 TRMC normalization).
      ContextHole,
  }

  /// Constructor kind for reuse size matching.
  ///
  /// Named `ReuseCtorKind` to avoid collision with `ir::CtorKind` (which
  /// has 7 variants: Struct, EnumVariant, Tuple, ListLiteral, MapLiteral,
  /// SetLiteral, Closure). This enum classifies allocation shape for reuse
  /// compatibility only — it is NOT the IR-level constructor kind.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub enum ReuseCtorKind {
      Struct,
      EnumVariant,
  }
  ```

- [x] Implement `ShapeClass::join` (exhaustive — flat lattice with `NonReusable` as top):
  - `NonReusable` + anything → `NonReusable` (absorbing/top element)
  - `ReusableCtor(k1)` + `ReusableCtor(k2)` where `k1 == k2` → `ReusableCtor(k1)`
  - `ReusableCtor(k1)` + `ReusableCtor(k2)` where `k1 != k2` → `NonReusable`
  - `CollectionBuffer` + `CollectionBuffer` → `CollectionBuffer`
  - `ContextHole` + `ContextHole` → `ContextHole`
  - `ReusableCtor(_)` + `CollectionBuffer` → `NonReusable`
  - `ReusableCtor(_)` + `ContextHole` → `NonReusable`
  - `CollectionBuffer` + `ContextHole` → `NonReusable`
  (All symmetric cases follow from commutativity.)
  This is a flat join (not a linear chain), so ShapeClass contributes
  chain height 1 (any value reaches `NonReusable` in at most one step).
- [x] **v1 behavior**: Set `ReusableCtor` for `Construct` instructions, `CollectionBuffer`
  for list/map/set allocations, `NonReusable` for everything else.

---

## 01.4c EffectClass Dimension

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs`

Tracks the memory effects of operations for FIP certification: a function certified
FIP must have no allocations on the fast path.

- [x] Define `EffectClass` as independent boolean flags (NOT a total order):
  ```rust
  /// Memory effect classification for FIP certification.
  ///
  /// Each flag is independent because FIP needs to know "may allocate"
  /// separately from "may throw". A total order (Pure < MayAlloc <
  /// MayShare < MayThrow) would lose information: joining MayAlloc
  /// with MayThrow would produce MayThrow, erasing the allocation
  /// fact that FIP certification depends on.
  ///
  /// Join is componentwise OR (each flag independently conservative).
  /// NONE is bottom (all false), ALL is top (all true).
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
  pub struct EffectClass {
      /// May allocate heap memory (blocks FIP certification).
      pub may_alloc: bool,
      /// May share references (refcount > 1).
      pub may_share: bool,
      /// May throw exceptions/panics.
      pub may_throw: bool,
  }

  impl EffectClass {
      pub const NONE: Self = Self { may_alloc: false, may_share: false, may_throw: false };
      pub const ALL: Self = Self { may_alloc: true, may_share: true, may_throw: true };
  }
  ```

- [x] Implement `EffectClass::join` — componentwise OR (each flag independently
  conservative). `NONE.join(x) == x`, `ALL.join(x) == ALL`.
  Chain height = 3 (each of 3 booleans can flip false→true once).
- [x] **v1 behavior**: Initialize most operations to `ALL` (conservative). Set `NONE` for
  scalar operations and known-pure builtins. Refine in Stage 2 for FIP certification.

---

## 01.5 Join and Transfer Functions

**File(s):** `compiler/ori_arc/src/aims/transfer/mod.rs`

Transfer functions define how each ARC IR instruction transforms the `AimsState`
of variables it touches. These are the core of the analysis.

**Code note (future):** The `transfer/mod.rs` module doc should include the
following design invariant: "No transfer function may derive Uniqueness from
Consumption or Cardinality alone. Uniqueness is about the past (has this value
been duplicated?); Consumption and Cardinality are about the future (how will
this value be used?). Any transfer rule that bridges this gap must involve a
past-facing dimension (locality, shape, or an interprocedural contract)."
(See: [Literature Review §06 — Linearity/Uniqueness](../aims-literature-review/section-06-linearity-uniqueness.md))

- [x] Define transfer functions for value-producing instructions (field names match `ArcInstr` enum):
  - `Construct { dst, ctor, args, .. }` → dst gets `FRESH` base then shape overridden
    from `ctor`: `ir::CtorKind::Struct` → `ReusableCtor(Struct)`,
    `ir::CtorKind::EnumVariant` → `ReusableCtor(EnumVariant)`,
    `ir::CtorKind::ListLiteral`/`SetLiteral` → `CollectionBuffer`,
    `ir::CtorKind::MapLiteral` → `CollectionBuffer`,
    `ir::CtorKind::Tuple`/`Closure` → `NonReusable`.
    In backward analysis: adds demand on each arg via `seq_add(Once)`.
    Note: `ir::CtorKind` (7 variants) maps to `ReuseCtorKind` (2 variants) — only
    Struct and EnumVariant are reuse-eligible; others remain `NonReusable`.
  - `Apply { dst, func, args, .. }` → dst gets state from callee's
    `MemoryContract.return_info.uniqueness` and access `Owned`;
    each arg's demand increases via `seq_add` per callee's `ParamContract.cardinality`
    (backward analysis: encountering a use ADDS demand when walking before the use)
  - `ApplyIndirect { dst, closure, args, .. }` → dst gets `TOP` (conservative — closure
    is unknown); closure gets demand bump (`Once` or `Many` from existing cardinality via
    `seq_add`); all args get `(Owned, Unrestricted)` (unknown callee may do anything)
  - `PartialApply { dst, func, args, .. }` → dst gets `FRESH`; captured args get
    `(Owned, Unrestricted, Many)` (stored in closure environment, may be used multiple
    times across invocations). Captured args' `locality` promoted to `HeapEscaping`
    (closure may outlive the defining function).
  - `Project { dst, value, field, .. }` → dst gets `(Borrowed, Linear, Once,
    value.uniqueness)` with `BorrowSource::Exact(value)`;
    value's uniqueness preserved (key insight: borrowing doesn't affect source uniqueness)
  - `CollectionReuse { old_var, dst, .. }` → dst gets `FRESH` with shape
    `CollectionBuffer`; old_var consumed (transitions to `(Owned, Dead, Absent, *)`)
    in forward semantics. In backward analysis: adds demand on old_var and args.
  - `Select { dst, cond, true_val, false_val, .. }` → dst gets join of true_val and
    false_val states; cond gets cardinality bump
  - `Let { dst, value, .. }` → for `ArcValue::Var(v)`: dst inherits v's state; for
    `ArcValue::Literal(_)`: dst gets `SCALAR`; for `ArcValue::PrimOp { .. }`: dst gets
    state based on result type (scalar → `SCALAR`, ref → `FRESH`)
  - `RcInc { var, .. }` / `RcDec { var, .. }` → these are OUTPUTS (AIMS generates them);
    if encountered during migration, treat as no-op for analysis purposes
  - `IsShared { dst, var }` → dst gets `SCALAR` (bool result); var unchanged
  - `Set { base, field, value }` → base must be `(Owned, *, *, Unique)` (mutation requires
    unique ownership); value consumed
  - `SetTag { base, tag }` → base must be `(Owned, *, *, Unique)`
  - `Reset { var, token }` → token gets `SCALAR`-like (reuse token); var consumed
  - `Reuse { token, dst, ctor, args, .. }` → dst gets `FRESH`; token consumed; args consumed

- [x] Define transfer functions for terminators (field names match `ArcTerminator` enum):
  - `Return { value }` → value must be at least `Once` cardinality; state contributes
    to function's return summary
  - `Jump { target, args }` → args flow into target block's params; standard flow
  - `Branch { cond, then_block, else_block }` → cond gets `Once`; split state; join at merge points
  - `Switch { scrutinee, cases: Vec<(u64, ArcBlockId)>, default }` → scrutinee gets
    cardinality bump (read for tag test). Note: ARC IR `Switch` only carries the
    discriminant value and target block IDs — it does NOT carry pattern bindings or
    destructuring info. Pattern bindings are lowered by the decision tree compiler
    into `Project` instructions in each target block's body. The analysis handles
    pattern-bound variables through the normal `Project` transfer function when
    processing target blocks.
  - `Invoke { dst, func, args, normal, unwind }` → like Apply but dst is defined only
    in `normal` successor (not `unwind`); unwind edge needs cleanup state for live
    variables across the invoke
  - `Resume` / `Unreachable` → terminal; no successor state contribution

- [x] Define transfer functions for RC operations (these are OUTPUTS, not inputs,
  but during analysis we reason about where they WOULD be needed):
  - If cardinality is `Absent` at a point where current system would emit `RcDec` →
    the dec is unnecessary (value already dead)
  - If cardinality is `Once` at a use site → no `RcInc` needed (single consumer)
  - If uniqueness is `Unique` at a COW point → `CowMode::StaticUnique`

- [x] Map `ArcClass::Scalar` → `AimsState::SCALAR` at variable definition
  (short-circuit: scalars never need analysis — `AimsState::from_arc_class`)

---

## 01.6 Lattice Properties and Proofs

**File(s):** `compiler/ori_arc/src/aims/lattice/mod.rs` (tests)

The lattice must satisfy formal properties for the analysis to be sound and terminate.
All proofs and tests below apply to the product lattice states only. `SCALAR` is
an analysis fast-path outside the product lattice (see 01.1) and is excluded from
lattice property verification — it never participates in join or transfer operations.

- [x] Test: **Idempotence** — `a.join(a) == a` for all states
- [x] Test: **Commutativity** — `a.join(b) == b.join(a)` for all pairs
- [x] Test: **Associativity** — `a.join(b.join(c)) == a.join(b).join(c)` for all triples
- [x] Test: **Monotonicity** — if `a ≤ b` then `f(a) ≤ f(b)` for all transfer functions
- [x] Test: **Finite height** — the chain height (max ascending steps per variable) is
  1+3+2+2+3+1+3 = 15 (AccessClass: 1, Consumption: 3, Uniqueness: 2, Cardinality: 2,
  Locality: 3, ShapeClass: 1, EffectClass: 3 — three independent booleans, each flips
  once). Fixed-point iteration converges in at most 15 × |variables| × |blocks| steps.
  The theoretical product state count is larger (AccessClass: 2, Consumption: 4,
  Uniqueness: 3, Cardinality: 3, Locality: 4, ShapeClass: 5 — NonReusable +
  ReusableCtor(Struct) + ReusableCtor(EnumVariant) + CollectionBuffer + ContextHole,
  EffectClass: 8 — three independent booleans = 2^3, total = 5,760) but this is
  irrelevant to convergence — only the chain height of 15 matters. After
  canonicalization (infeasible state collapsing), the reachable state space is
  dramatically smaller. In practice, convergence is typically fast because most
  dimensions stabilize quickly and canonicalization prunes impossible states.
- [x] Test: **Soundness** — the abstract state correctly approximates concrete RC behavior:
  - `Unique` in the lattice → runtime RC is 1 (never wrong)
  - `Once` in the lattice → the value IS used at most once (never wrong)
  - `Linear` in the lattice → no RC inc is needed for the consumption (moved,
    not copied). An RcDec IS still needed if the value dies without being consumed
    (e.g., dead parameter at function entry). Linear means "consumed at most once
    without duplication", not "no RC operations whatsoever".
  - `Borrowed` in the lattice → no RC operations on this variable (never wrong)
  - Conservative direction: the analysis may over-approximate (say `Shared` when
    actually `Unique`) but never under-approximate
- [x] Test: **Canonicalization** — `canonicalize()` is idempotent:
  `canonicalize(canonicalize(s)) == canonicalize(s)` for all states.
  All transfer functions produce canonicalized output:
  `canonicalize(transfer(s)) == transfer(s)`.
- [x] Test: **Feasibility** — no reachable state violates canonicalization invariants:
  - `consumption == Dead` implies `cardinality == Absent`
  - `access == Borrowed` implies no RC obligation (is_rc_needed returns false)

- [x] Enumerate all feasible states and document which are infeasible:
  | Access | Consumption | Uniqueness | Cardinality | Feasible? | Meaning |
  |--------|-------------|-----------|-------------|-----------|---------|
  | Owned | Dead | * | Absent | Yes | Dead variable, no RC |
  | Owned | Linear | Unique | Once | Yes | Fresh, consumed once, no RC |
  | Owned | Linear | Unique | Many | Yes | Fresh, used in loop — static COW |
  | Owned | Affine | Unique | Once | Yes | May drop early, no RC |
  | Owned | Affine | MaybeShared | Once | Yes | May drop, needs RC dec |
  | Borrowed | Linear | * | Once | Yes | Temporary view, one read, no RC |
  | Borrowed | Affine | * | Once | Yes | Temporary view, may drop, no RC |
  | Borrowed | Affine | * | Many | Yes | Multiple reads, no RC |
  | Owned | Unrestricted | Shared | Many | Yes | Full ARC (current default) |
  | Owned | Unrestricted | MaybeShared | Many | Yes | Full ARC with COW check |
  | * | Dead | * | Once/Many | No | Dead can't be used |
  | Owned | Linear | Shared | * | Yes | Linear + Shared: shared parameter used once on this path. Emission: `rc_dec` at last use, no `rc_inc`. Valid in converged output (Marshall et al.: linearity and uniqueness are independent). See 01.1 canonicalize note. |
  | Borrowed | Dead | * | Absent | Yes | Expired borrow — no RC, no use |

- [x] **Design decision: Borrowed resolved (historical design decision)**
  `Borrowed` is a separate `AccessClass` dimension, NOT part of the `Consumption`
  ordering. This resolves the `join(Linear, Borrowed)` problem: join now operates
  independently on each axis, so `join((Owned, Linear), (Borrowed, Linear)) =
  (Owned, Linear)` — consumption information survives. The borrow provenance is
  tracked in a sparse `BorrowSource` side table (see 01.2).

---

## 01.7 Completion Checklist

- [x] `AimsState` struct defined with all seven dimensions
- [x] `AccessClass` (2 variants) and `Consumption` (4 variants) enums defined with
  separate join operations (historical design decision)
- [x] `Uniqueness`, `Cardinality`, `Locality`, `ShapeClass`, `EffectClass` defined
- [x] All seven dimension enums have `join` operations
- [x] `AimsState::canonicalize()` enforces feasibility invariants (historical design decision)
- [x] Transfer functions defined for all `ArcInstr` variants (15 variants total:
  Let, Apply, ApplyIndirect, PartialApply, Project, Construct, RcInc, RcDec,
  IsShared, Set, SetTag, Reset, Reuse, CollectionReuse, Select)
- [x] Transfer functions defined for all `ArcTerminator` variants (7 variants total:
  Return, Jump, Branch, Switch, Invoke, Resume, Unreachable)
- [x] Unit tests pass for all lattice properties (idempotence, commutativity,
  associativity, monotonicity)
- [x] Canonicalization idempotence tested
- [x] Per-axis exhaustive lattice law tests (historical design decision)
- [x] Pairwise interaction tests: access × consumption, consumption × cardinality,
  uniqueness × shape, locality × effect
- [x] Cardinality semiring law tests: associativity, commutativity, identity of
  seq_add; associativity, idempotence of alt_join; distributivity of seq_add
  over alt_join (27 exhaustive cases each — see 01.4)
- [ ] Cardinality positivity test: `a.seq_add(b) == Absent` implies both `a == Absent`
  and `b == Absent` — 9 cases exhaustive (QTT correspondence: no non-trivial cancellation)
- [ ] Cardinality right-distributivity test: `(a.alt_join(b)).seq_add(c) == a.seq_add(c).alt_join(b.seq_add(c))`
  — 27 cases exhaustive (symmetric to left-distributivity)
- [x] Feasible/infeasible state table documented and tested
- [x] `AimsState::SCALAR` correctly short-circuits analysis for scalar types
- [x] `AimsState::from_arc_class` handles all three `ArcClass` variants
  (Scalar, DefiniteRef, PossibleRef)
- [x] `AimsState::FRESH` convenience constant defined and used in transfer functions
- [x] `BorrowSource` side table defined (sparse, not in finite lattice)
- [x] `Locality`, `ShapeClass`, `EffectClass` initialize conservatively for v1
- [x] No dependencies on other AIMS sections (this is the foundation)

- [x] **Non-convergence safety**: if the worklist exceeds a configurable iteration
  limit (default: `chain_height × num_variables × num_blocks`, i.e., 15 × V × B),
  stop iteration and widen all remaining non-converged variables to TOP. Log a
  `tracing::warn!` with the function name and iteration count. This is a safety net —
  the lattice properties guarantee convergence, so hitting this limit indicates a bug
  in transfer functions. The warning makes it visible during development.

**Exit Criteria:** `cargo t -p ori_arc -- aims::lattice` passes with 100% of lattice
property tests green. The chain height is verified as finite (15). All transfer
functions are monotonic. Canonicalization is idempotent. Pairwise interaction tests
cover core × core dimension pairs. The auxiliary dimensions (`Locality`, `ShapeClass`,
`EffectClass`) default to conservative values and do not affect RC emission in v1.
