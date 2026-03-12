---
section: "03"
title: "FIPTree — The Functional Essence of Imperative Binary Search Trees"
status: complete
goal: "Determine if constructor-context opportunity creation is strong enough and whether Stage 3 should be mandatory architecture"
paper:
  title: "The Functional Essence of Imperative Binary Search Trees"
  url: "https://www.microsoft.com/en-us/research/publication/fiptree-full/"
  doi: "https://doi.org/10.1145/3656398"
  venue: "PLDI 2024"
  authors: "Lorenzen, Leijen, Swierstra, Lindley"
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "Paper Thesis"
    status: complete
  - id: "03.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "03.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "03.4"
    title: "Plan Edits"
    status: complete
  - id: "03.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "03.6"
    title: "Lens Shift"
    status: complete
  - id: "03.7"
    title: "Open Risk"
    status: complete
---

# Section 03: FIPTree — The Functional Essence of Imperative Binary Search Trees

**Status:** Complete
**Goal:** Determine if constructor-context opportunity creation is architecturally strong
enough in the AIMS plan, whether Stage 3 (TRMC/context normalization) should move earlier
or be mandatory rather than optional, and whether "unfinished structure around a hole" is
represented explicitly enough.

**Paper:** Lorenzen et al., "The Functional Essence of Imperative Binary Search Trees,"
PLDI 2024. [Full paper](https://www.microsoft.com/en-us/research/publication/fiptree-full/)
[DOI: 10.1145/3656398](https://doi.org/10.1145/3656398)

**Why read this third:** This is where constructor contexts stop being an abstract trick
and become a structural front-end for in-place execution. It shows how FIP + constructor
contexts enable O(1) top-down tree algorithms.

**Pause questions:**
- Is constructor-context opportunity creation strong enough in the plan?
- Should Stage 3 move earlier or be framed as mandatory architecture, not optional enhancement?
- Are you representing "unfinished structure around a hole" explicitly enough?

**AIMS context:**
- `ShapeClass::ContextHole` exists in the lattice as a variant
- Stage 3 (Opportunity Creation) is designed as pre-analysis normalization
- `aims/normalize/` is planned but not yet implemented
- Section 09 plans shape activation as part of dimensional fusion

---

## 03.1 Paper Thesis

The paper's central claim is that imperative tree algorithms (move-to-root, splay, zip
trees) have a *functional essence* that can be recovered by two key structural
transformations, and that the resulting functional programs match or beat the best C
implementations when compiled with Perceus RC and FIP.

**The two transformations are:**

1. **Bottom-up via defunctionalized CPS / zippers.** A recursive tree function is CPS-
   transformed and defunctionalized. The resulting continuation data type IS a zipper --
   a one-hole context stored as a reversed path from the hole back to the root. The zipper
   reconstructs the tree bottom-up via `rebuild()`. At runtime, under Perceus, the zipper
   constructors reuse the memory of the destructed tree nodes (same arity, same layout).
   This compiles to pointer-reversal traversal -- no allocation, no stack growth.

2. **Top-down via first-class constructor contexts.** Instead of accumulating a reversed
   path, the algorithm accumulates a Minamide-style context tuple `{root_ptr, hole_ptr}`.
   Composition (`c1 ++ c2`) and application (`c ++. v`) are O(1) operations that write
   the new subtree root into the hole and advance the hole pointer. The tree is built in
   forward order as the traversal descends, so there is no "unwind" phase.

**The key insight for AIMS:** Both transformations are mechanical. Given a recursive
function `f` that matches on a tree and recurses into a subtree, the bottom-up version
is derived by CPS + defunctionalization, and the top-down version is derived by
tail-recursion-modulo-context (TRMC). The paper proves these are equivalent to the
original by simple structural induction. The loop invariants of the imperative algorithms
are *exactly* the constructor contexts / zippers of the functional versions.

**Performance results:** On 10M insertions, Koka's FIP functional implementations
outperform standard C for move-to-root, splay, and zip trees (both top-down and
bottom-up). Against "equalized" C (linked with mimalloc, header word added), the
functional versions are at most 15% slower for top-down, and faster for bottom-up
(because zippers compile to pointer-reversal while C uses parent pointers).

**Context representation (Section 3.2):** The paper introduces runtime context paths as
an alternative to Minamide's affine type requirement. Each heap object's header stores
an 8-bit field recording which child leads to the hole. Context composition follows the
path from root to hole, then links the new context at the hole's position. When a context
is shared (RC > 1), the path is copied on demand. When unique, all operations are O(1)
in-place. This requires neither a linear type system nor a special-purpose analysis --
precise reference counting is sufficient.

**Formal results:** All correctness proofs are mechanized in Coq/Iris. The key theorems
have the form:
- `down-td(t,k,accl,accr) = val Node(l,x,r) = insert(t,k) in Node(accl ++. l, x, accr ++. r)`
- `down-bu(t,k,z) = rebuild(z, insert(t,k))`

These equalities hold by structural induction on the tree and require the three
context laws: identity (`ctx _ ++. v = v`), associativity (`(c1 ++ c2) ++ c3 = c1 ++ (c2 ++ c3)`),
and distributivity (`(c1 ++ c2) ++. v = c1 ++. (c2 ++. v)`).

---

## 03.2 What AIMS Should Adopt

### Keep

**K1. Context representation as Minamide tuple with runtime path.**
The paper's representation -- `{root_ptr, hole_ptr}` with child-index metadata in
object headers -- is the right runtime model. AIMS does not need to invent a different
representation. The `aims/normalize/context.rs` module (planned) should extract exactly
this metadata: which constructor, which child position holds the hole, and what the
path length is. The runtime representation maps directly to LLVM codegen: a struct of
two pointers, with the index stored in the existing header word (Ori already has a
header byte available in `ori_rt`).

**K2. Context laws as proof obligations.**
The three laws (identity, associativity, distributivity) are the MINIMUM proof
obligations for context correctness. Any normalization in `aims/normalize/trmc.rs` that
introduces constructor contexts MUST verify these laws hold for the specific constructor
and data type. In practice, this means:
- The constructor must have exactly one recursive field position (the hole position).
- The hole must be of the same type as the constructor's output (so composition is
  type-safe).
- No effectful instructions may appear between context creation and context fill/compose
  (purity within the context region).

**K3. FIP check as reuse credit accounting.**
The paper defines FIP as: "in each branch, constructors matched provide reuse credits
of size k (written diamond-k), consumed by constructors on the right-hand side. No net
allocation." This is *exactly* what `EffectClass.may_alloc == false` combined with
allocation balance tracking should compute. The reuse credit is the `ShapeClass::ReusableCtor`
with matching arity. AIMS Section 09.3 Rule 7 (`EffectClass::NONE + alloc-balanced -> FIP-natural`)
already captures this, but it should be made precise: the balance must be per-branch,
not just per-function, because FIP is a property of each match arm independently.

**K4. Dual accumulator pattern.**
The top-down splay and move-to-root algorithms use TWO constructor contexts (`accl`, `accr`)
for left and right subtrees. The normalize module must support multi-context accumulation,
not just single-context TRMC. This means `ContextRegion` (the planned Stage 3 data
structure) needs to handle N accumulator parameters, not just one. The common case is
N=1 (standard TRMC), the splay/MTR case is N=2, and zip tree unzip is N=2 as well.

**K5. Bounded FIP contract from allocation balance (not user annotation).**
The paper uses `fip(1)` to mark functions that allocate at most 1 constructor (for the
newly inserted key). AIMS should support a bounded FIP contract: `FipContract::Bounded(n)`
where n is the maximum net allocation. This is strictly more informative than `FipContract::Fip`
(n=0) vs `FipContract::Never`. The current `FipContract` enum in `aims/contract/mod.rs`
should grow a `Bounded(u16)` variant. This falls out of allocation balance tracking:
`allocs - reuses = n`. Note: this is a compiler-inferred classification, not a user
annotation (consistent with Section 02 R1 rejecting `fip` as a user-facing keyword
and R5 rejecting `fbip(n)` as a separate programmer-declared tier).

### New Invariants

**I1. Context region purity invariant.**
Between the point where a constructor context is captured and the point where it is
filled (composed or applied), no instruction may:
- Allocate heap memory (would break FIP guarantee)
- Share the context (would force a copy of the context path)
- Throw an exception (would leave the context dangling with an unfilled hole)

This maps to: within a context region, `EffectClass` must be `NONE`. The
`_context_regions: &[ContextRegion]` parameter already reserved in
`aims/intraprocedural/mod.rs` line 74 is the right place to enforce this.

**I2. Context hole type compatibility invariant.**
The value filling a context hole must have the same type (and therefore same ARC
layout) as the hole's expected type. This is a type-system property, not an analysis
property, so it should be checked at normalization time (in `aims/normalize/trmc.rs`),
not during intraprocedural analysis. Emit a diagnostic if the recursive call's return
type doesn't match the constructor's child field type.

**I3. Context uniqueness invariant.**
For O(1) operations, the context must be unique at every composition and application
point. The paper handles shared contexts by copying the context path -- this is correct
but degrades to O(depth) for each shared operation. AIMS should track context
uniqueness through the `Uniqueness` dimension. If a context variable has
`Uniqueness::Unique` at all composition/application points, the context operations are
guaranteed O(1). If `MaybeShared`, AIMS must emit a uniqueness check before composition
(analogous to the COW `IsShared` check). This means `ShapeClass::ContextHole` needs
to participate in the same uniqueness-driven optimization as `ReusableCtor`.

**I4. Allocation credit balance per match arm.**
The paper's FIP check counts reuse credits per branch: "in each branch, the
constructors matched provide credits consumed by the right-hand side." AIMS currently
tracks allocation balance at the function level (Section 09.3 Rule 7). For FIP
certification, the balance must be checked per match arm / per branch of a `Switch`
terminator. A function is FIP if and only if EVERY branch has non-negative credit
balance. One branch with a deficit (net allocation > 0) disqualifies the entire function
from FIP, even if other branches are balanced.

---

## 03.3 What AIMS Should Not Adopt

### Reject

**R1. Koka's `fip` keyword as a user-facing annotation.**
The paper relies on Koka's `fip` and `fip(n)` keywords as programmer annotations that
the compiler checks statically. Ori does not (and should not) require users to annotate
functions as FIP. AIMS infers FIP status from the converged lattice state. The compiler
should report FIP status as a diagnostic/attribute, not require it as an input. The
`FipContract` in `MemoryContract` is an OUTPUT of analysis, not a user declaration.

**R2. The 8-bit context path index in headers.**
The paper stores the context path child index in an existing 8-bit header field
used by Koka for "stackless freeing." Ori's runtime (`ori_rt`) has a different header
layout. The specific encoding (which header bits, how many bits per index) is a
runtime/codegen detail that should be designed when Stage 3 implementation begins,
not adopted wholesale from Koka. The invariant (each node on the context path stores
which child leads to the hole) should be adopted; the encoding should not.

**R3. AddressC / HeapLang formalization.**
The paper's Coq/Iris proofs use a custom embedded language (AddressC) built on
HeapLang. This formalization framework is specific to the paper's verification goals.
AIMS does not need to adopt Iris or HeapLang. The proof obligations (context laws,
FIP credit balance) can be checked by AIMS's own analysis rather than by a separate
proof assistant. The invariants are what matter, not the proof framework.

**R4. Constructor context as a first-class language feature.**
The paper introduces `ctx` as a user-visible keyword and constructor contexts as
first-class values that can be composed, applied, and returned from functions. Ori
should NOT expose constructor contexts in the surface language. They are a compiler-
internal optimization: the normalize pass rewrites recursive functions to use contexts
internally, but the user writes standard recursive code. This is consistent with
the AIMS principle that opportunity creation (Phase A) is a compiler transformation,
not a language feature.

**R5. Affine type system for contexts.**
The paper explicitly rejects the need for Minamide's affine type system, using
runtime reference counting instead. AIMS should follow this approach: context
uniqueness is tracked through the existing `Uniqueness` dimension, not through a
separate linear/affine type mode.

---

## 03.4 Plan Edits

### Stage 3 framing: mandatory architecture, not optional enhancement

The current plan (Section 00, lines 391-402) frames Stage 3 as:
> "Stage 3 -- Opportunity Creation (TRMC normalization)"
> "Benefits from Stage 2: active shape dimension identifies ContextHole"
> "Deliverable: more opportunities for reuse, FIP, and tail-call lowering"

After reading FIPTree, this framing understates the structural importance. Constructor
contexts are not just "more opportunities." They are the mechanism by which an entire
class of algorithms (top-down tree traversals, list accumulations, any recursive
function with a constructor in tail-modulo position) becomes FIP-eligible. Without
Stage 3, these algorithms cannot be FIP, period -- no amount of dimensional fusion
in Stage 2 can recover what normalization provides.
<!-- reviewed: completeness fix — Cross-dependency: Section 04 (TRMC) P5 also proposes
adding a "law before optimization" design principle to 00-overview, and P1 adds proof
obligations to Stage 3 scope. These three edits (03 reframing, 04 P1, 04 P5) all target
the same Stage 3 description in 00-overview.md and should be applied together. The current
AIMS plan Stage 3 text does NOT say "optional" — it says "Deliverable: more opportunities"
which is accurate but understated per this review. -->

**Recommended edit to `plans/aims/00-overview.md`:**
- Reframe Stage 3 as "Opportunity Creation (required for FIP on recursive algorithms)"
- Add: "Stage 3 is not an optimization pass. It is a structural prerequisite. Without
  it, self-recursive constructor functions cannot be FIP or FBIP. Stage 2 makes
  the analysis *ready* to exploit contexts; Stage 3 creates the contexts to exploit."
- Keep the staging order (Stage 3 after Stage 2) because Stage 2's active shape/effect
  dimensions genuinely help identify TRMC candidates. But remove language suggesting
  Stage 3 is optional.

### Section 09.2 Shape Activation: ContextHole needs richer metadata

The current plan (Section 09, lines 398-402) says:
> "`ContextHole` shape means the value has a hole to be filled by a recursive call.
> When shape analysis identifies `ContextHole + FunctionLocal`, the function is a
> TRMC candidate."

This is correct but insufficient. FIPTree shows that a context carries three pieces
of metadata that `ShapeClass::ContextHole` alone does not capture:

1. **Hole position** -- which child index of the constructor holds the hole.
2. **Context depth** -- how many constructors are on the path from root to hole
   (determines copy cost when shared).
3. **Number of accumulators** -- 1 for standard TRMC, 2 for splay/MTR-style dual
   contexts, N for multi-arm accumulation.
<!-- reviewed: completeness fix — Cross-dependency: Section 04 (TRMC) P3 also proposes
strengthening ContextHole in 09.2 with Unique + EffectClass requirements. The metadata
proposed here (hole position, depth, accumulator count) is complementary to Section 04's
soundness conditions (Unique + may_share==false). Both should be applied: 04's conditions
are soundness gates, 03's metadata is structural information. Together they become:
ContextHole(ContextMeta) + Unique + FunctionLocal + EffectClass::may_share==false. -->

**Recommended edit to `plans/aims/section-09-dimensional-fusion.md`:**
- Under "Shape for constructor contexts (TRMC preparation)" (line 398), add a note
  that `ContextHole` as a bare enum variant is a placeholder. When Stage 3 is
  implemented, it should become `ContextHole(ContextMeta)` where `ContextMeta` carries
  hole position, depth estimate, and accumulator count.
- Cross-reference with the `aims/normalize/context.rs` module plan, which is where
  this metadata is extracted.

### `aims/normalize/context.rs` scope expansion

The current plan (`00-overview.md` line 460) lists: <!-- reviewed: accuracy fix, was line 461 -->
> `context.rs -- constructor-context metadata extraction`

FIPTree reveals that this module needs to handle:
- Single-context TRMC (list map, tree rebuild)
- Dual-context accumulation (splay, move-to-root, zip tree unzip)
- Context composition validation (the three context laws)
- Context region boundary detection (where does the context region start/end?)
- Purity verification within context regions (EffectClass::NONE check)

This is not a single-file module. Recommend splitting:
- `context/mod.rs` -- `extract_context_metadata()` entry point
- `context/detect.rs` -- identify constructor-context candidates from the IR
- `context/validate.rs` -- verify context laws, purity, type compatibility
- `context/multi.rs` -- dual/multi-accumulator detection (splay pattern)
<!-- reviewed: completeness fix — Cross-dependency: Section 04 (TRMC) P2 proposes a
different normalize/ expansion with lift.rs, rewrite.rs, verify.rs. These two proposals
overlap on verify.rs (context law verification) but differ on the rest. Reconciliation:
Section 04's structure covers the TRMC rewrite pipeline (lift -> detect -> rewrite ->
verify), while Section 03's structure covers context metadata (detect -> validate ->
multi). The context/ subdirectory proposed here should be a subdirectory OF normalize/,
not a replacement for it. Combined layout:
  normalize/
    mod.rs, lift.rs, trmc.rs, rewrite.rs, verify.rs, collections.rs  (from Section 04)
    context/mod.rs, context/detect.rs, context/validate.rs, context/multi.rs  (from Section 03)
-->

### `FipContract` enum expansion

`aims/contract/mod.rs` currently has `FipContract::Never` as the only variant in
Stage 1. FIPTree shows the useful variants are:
- `Never` -- function is not FIP (net allocation > 0 in some branch)
- `Fip` -- function is FIP (zero net allocation in all branches)
- `Bounded(u16)` -- function allocates at most N constructors (FIPTree's `fip(n)`)
- `Fbip` -- function is FBIP (allocation matches deallocation over function lifetime,
  but not necessarily per-branch balanced)
<!-- reviewed: completeness fix — CONFLICTS with existing AIMS plan. The current
FipContract enum (Section 03.1) has three variants: Never, Conditional{requires_unique_params},
Certified. This proposal replaces that with four different variants: Never, Fip, Bounded(u16),
Fbip. The Conditional variant (parameterized on which params must be unique) has no
equivalent in the proposed expansion. Reconciliation: keep Conditional (it encodes a
real AIMS concept — call-site-dependent FIP), add Bounded alongside Certified, and treat
Fbip as a separate diagnostic flag (not a FipContract variant) since FBIP is a post-pipeline
check, not a contract property. Proposed merged enum:
  Never | Conditional{requires_unique_params} | Certified | Bounded(u16)
with a separate `is_fbip: bool` on MemoryContract. -->

---

## 03.5 Code Changes (Later)

### `ShapeClass::ContextHole` enrichment

**File:** `compiler/ori_arc/src/aims/lattice/dimensions.rs`

Current (line 181):
```rust
/// A constructor-context hole (Stage 3 TRMC).
ContextHole,
```

When Stage 3 arrives, this should become:
```rust
/// A constructor-context hole (Stage 3 TRMC).
/// Metadata records the hole position, estimated context depth,
/// and accumulator count (1 = standard TRMC, 2 = dual-context splay pattern).
ContextHole(ContextMeta),
```

Where `ContextMeta` is a small struct:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContextMeta {
    /// Which child index of the constructor holds the hole (0-based).
    pub hole_child_index: u8,
    /// Estimated depth of the context path (number of constructors root-to-hole).
    /// Used to estimate copy cost when the context is shared.
    pub estimated_depth: u16,
    /// Number of accumulator contexts in the pattern (1 = TRMC, 2 = splay).
    pub accumulator_count: u8,
}
```

The `ShapeClass::join` for `ContextHole` needs updating: two `ContextHole` values with
different `ContextMeta` should join to `NonReusable` (flat lattice semantics preserved).

**Timing:** Do NOT make this change now. Make it when `aims/normalize/context.rs` is
implemented in Stage 3. The current bare `ContextHole` variant is correct for Stages 1-2.

### `FipContract` expansion

**File:** `compiler/ori_arc/src/aims/contract/mod.rs`

Add `Bounded(u16)` variant when Stage 3 begins. This enables `fip(1)` style insertion
algorithms where one allocation is permitted but the function is otherwise in-place.

### Context region infrastructure

**Files (new, Stage 3):**
- `compiler/ori_arc/src/aims/normalize/mod.rs`
- `compiler/ori_arc/src/aims/normalize/trmc.rs`
- `compiler/ori_arc/src/aims/normalize/context/mod.rs`
- `compiler/ori_arc/src/aims/normalize/context/detect.rs`
- `compiler/ori_arc/src/aims/normalize/context/validate.rs`
- `compiler/ori_arc/src/aims/normalize/context/multi.rs`

The `ContextRegion` struct (consumed by `_context_regions` in
`aims/intraprocedural/mod.rs`) needs at minimum:
```rust
pub struct ContextRegion {
    /// The block range this context spans (entry block to fill block).
    pub blocks: Range<usize>,
    /// The variable holding the context accumulator.
    pub context_var: ArcVarId,
    /// Which constructor and child position form the context.
    pub hole_info: ContextMeta,
    /// Whether this is a single or multi-accumulator pattern.
    pub pattern: ContextPattern,
}

pub enum ContextPattern {
    /// Standard TRMC: one accumulator, one recursive call.
    SingleAccumulator,
    /// Dual accumulator (splay/MTR): two contexts for left/right subtrees.
    DualAccumulator { left: ArcVarId, right: ArcVarId },
}
```

### Allocation balance tracking per branch

**File:** `compiler/ori_arc/src/aims/intraprocedural/block.rs`

The paper's FIP check requires per-branch credit accounting. During backward analysis
of a `Switch` terminator, each successor block's allocation credit (constructors
consumed by pattern match) must balance against the constructors allocated in that
branch's body. Track this as part of the sparse event table
(`AimsStateMap.events`) with a new event variant:
```rust
AllocCreditBalance {
    block: BlockIdx,
    branch_idx: usize,
    credits_provided: u16,  // destructed constructors
    credits_consumed: u16,  // allocated constructors
}
```

---

## 03.6 Lens Shift

### Reading Paper 04 (TRMC) through FIPTree's lens

FIPTree reveals that TRMC is not the whole story for top-down algorithms. TRMC handles
the single-accumulator case (one context, one recursive call in tail-modulo position).
But FIPTree demonstrates patterns that go beyond TRMC:

1. **Dual-context accumulation** -- splay trees use `accl` and `accr`. Standard TRMC
   produces one accumulator. The TRMC paper (Paper 04) should be read with the
   question: does it address multi-accumulator patterns, or does that require a
   separate extension?

2. **Context returned as value** -- The `append-td` function returns a context as its
   result (not just filling and returning a tree). The `flatten-td` function composes
   returned contexts. This is beyond TRMC (which fills the context at the base case).
   Paper 04 should be checked for whether its "equational approach with context laws"
   covers this case.

3. **The zipper alternative** -- FIPTree shows that bottom-up algorithms with zippers
   can BEAT top-down algorithms with contexts (bottom-up splay beats top-down in
   some restructuring-heavy cases). Paper 04 focuses on top-down / TRMC. The AIMS
   normalize module should support BOTH the zipper derivation (CPS +
   defunctionalization) and the context derivation (TRMC), choosing the better one
   based on the algorithm structure.

4. **Equivalence proofs** -- FIPTree proves that bottom-up and top-down versions are
   equivalent to the recursive specification. Paper 04 should give the equational
   laws that make this proof work for the TRMC transformation specifically. Look for:
   does TRMC guarantee the same three context laws (identity, associativity,
   distributivity)?

### Recalibration for subsequent papers

The remaining papers should be read knowing that:
- Constructor contexts are a runtime representation with concrete metadata
  requirements, not just an abstract transformation.
- FIP is a per-branch property, not just per-function.
- The "opportunity" in "opportunity creation" is precisely: making the context
  structure explicit so that the analysis (Phase B) can prove uniqueness, purity,
  and allocation balance for each context region.

---

## 03.7 Open Risk

### R1. `ShapeClass::ContextHole` is too coarse

The current `ContextHole` variant is a single unit value in a flat lattice. It tells
the analysis "this is a context" but nothing about the context's structure. FIPTree
shows that the compiler needs to know: hole position, path depth, accumulator count,
and which constructor forms each context node. Without this metadata, the analysis
cannot:
- Verify context law compliance (needs hole position and constructor identity)
- Decide between O(1) and O(depth) operations (needs uniqueness at composition points)
- Match context allocation credits against pattern match credits (needs constructor
  arity for credit accounting)

**Risk level:** Medium. This is a Stage 3 problem, not a Stage 1-2 problem. The
current placeholder is acceptable for now. But the transition from `ContextHole`
to `ContextHole(ContextMeta)` will touch `ShapeClass::join`, `canonicalize`, every
test that matches on `ShapeClass`, and the transfer functions that set shape. Plan
for this as a cross-cutting change, not a local edit.

### R2. AIMS has no explicit representation of "unfinished structure around a hole"

This is the deepest concern. FIPTree's key contribution is showing that constructor
contexts -- unfinished structures with a hole -- are the EXACT data structure that
captures loop invariants of imperative tree algorithms. The paper says:

> "the functional algorithms capture the key loop invariants required to verify the
> imperative algorithms"

AIMS currently has no IR-level representation of a context. `ShapeClass::ContextHole`
is a lattice dimension value attached to a variable, not a structural entity in the
IR. There is no `ArcInstr::ContextCompose` or `ArcInstr::ContextApply`. The normalize
module is planned to rewrite the IR before analysis, but the rewritten IR still uses
standard `Construct` and `Project` instructions.

**The question is:** after normalization, can AIMS's existing IR instructions fully
represent context operations, or does the IR need new instruction variants?

If contexts are lowered to a pair of pointer variables (root, hole) with standard
`Construct` and `Set` instructions to thread them, then the existing IR suffices and
the analysis can track them through the dimensions. But if context composition
requires an atomic "write value to hole, advance hole pointer" operation, then a new
IR instruction is needed because `Set` alone doesn't capture the "advance" semantics.

**Risk level:** High. This is an architectural question that determines whether
Stage 3 is a normalization-only change or requires IR extension. The answer
depends on whether `Construct` + `Set` + `Project` can express `ctx_compose(c, new_node)`
without losing information that the analysis needs. Resolve this question BEFORE
starting Stage 3 implementation.

### R3. Per-branch FIP certification is not planned

The current AIMS plan tracks allocation balance at the function level (Section 09.3
Rule 7, Section 09.2 Effect Activation). FIPTree requires per-branch balance: each
arm of a match must independently balance its constructor destructions against its
constructor allocations. A function where branch A is balanced and branch B allocates
net-1 should get `FipContract::Bounded(1)`, not `FipContract::Fip`. The intraprocedural
analysis infrastructure (block-level backward pass) naturally provides per-block
information, but the rollup into a function-level `FipContract` currently loses the
per-branch granularity.

**Risk level:** Low. This is a precision issue, not a soundness issue. A conservative
`FipContract::Never` is always safe. But fixing it requires the per-branch credit
tracking described in 03.5, which adds a new event type to the sparse event table.

### R4. No support for zipper derivation

FIPTree shows that BOTH top-down (context) and bottom-up (zipper) derivations are
valuable. The current AIMS plan focuses entirely on TRMC (top-down). The zipper
derivation (CPS + defunctionalization) is a separate transformation that produces
a different data type (zipper) and a different traversal pattern (rebuild). AIMS
has no plan for this transformation.

For tree algorithms specifically, the zipper-based bottom-up approach can be faster
than the context-based top-down approach (the paper shows this for splay and
move-to-root). If AIMS only supports top-down normalization, it will miss the
faster derivation for some algorithms.

**Risk level:** Low for Stage 3 v1 (which explicitly limits scope to self-recursive
constructor contexts). But worth tracking: a future Stage 3 v2 should consider
whether the CPS-defunctionalization / zipper path is worth automating.
