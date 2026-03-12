---
section: "11"
title: "Reference Counting Deeply Immutable Data Structures with Cycles"
status: complete
goal: "Identify future assumptions to preserve in runtime/interface without contaminating current AIMS design"
paper:
  title: "Reference Counting Deeply Immutable Data Structures with Cycles: an Intellectual Abstract"
  doi: "https://doi.org/10.1145/3652024.3665507"
  venue: "ISMM 2024"
  authors: "Matthew J. Parkinson, Sylvan Clebsch, Tobias Wrigstad"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10"]
sections:
  - id: "11.1"
    title: "Paper Thesis"
    status: complete
  - id: "11.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "11.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "11.4"
    title: "Plan Edits"
    status: complete
  - id: "11.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "11.6"
    title: "Lens Shift"
    status: complete
  - id: "11.7"
    title: "Open Risk"
    status: complete
---

# Section 11: Reference Counting Deeply Immutable Data Structures with Cycles

**Status:** Complete
**Goal:** Identify future assumptions that should be preserved in the runtime/interface
for frozen cyclic heaps, without contaminating current AIMS design (Ori does not yet
have frozen cyclic graphs).

**Paper:** Parkinson, Clebsch, Wrigstad, "Reference Counting Deeply Immutable Data
Structures with Cycles: an Intellectual Abstract," ISMM 2024.
[DOI: 10.1145/3652024.3665507](https://doi.org/10.1145/3652024.3665507)

**Why read this eleventh:** Another boundary-setting paper, useful for understanding what
future frozen-cycle support would require. The key output is: what interface assumptions
should we preserve now so this remains possible later?

**Pause questions:**
- What future assumptions should be preserved in the runtime/interface?
- What should not contaminate current AIMS design because Ori does not yet have frozen cyclic graphs?

**AIMS context:**
- Stage 5 lists "SCC-based frozen-cycle RC" as a separate effort
- `ori_rt` has no cycle detection or backup tracing
- AIMS assumes acyclic reference graphs (Ori's value semantics prevent most cycles)
- `EffectClass` could eventually track cycle-forming operations

---

## 11.1 Paper Thesis

The paper proposes that memory underlying deeply immutable data structures can be
efficiently managed using reference counting, even when the structures contain
cycles. The key insight is that cycles in immutable data are themselves immutable --
the reference graph is frozen and will never change -- so it can be analyzed once.

The approach combines two classical algorithms:

1. **Strongly Connected Component (SCC) calculation.** After an object graph is
   frozen (transitioned from mutable to deeply immutable), Tarjan's SCC algorithm
   partitions the graph into SCCs in near-linear time. Each SCC becomes the unit
   of reference counting rather than individual objects.

2. **Union-find for equivalence classes.** Objects within the same SCC are
   collapsed into a single equivalence class using a union-find (disjoint-set)
   data structure. One object in each SCC is designated the representative; the
   SCC's single reference count is stored on (or associated with) this
   representative.

The key property exploited: since the graph is unchanging (deeply immutable), the
SCCs are computed exactly once, in O(n * alpha(n)) time (Tarjan's SCC is O(n+m);
union-find operations are amortized near-constant). After that, reference counting
operates at SCC granularity. An external reference to any object in an SCC
increments/decrements the SCC's count. When the SCC's count reaches zero, the
entire SCC is deallocated.

**Deep immutability** means: once frozen, an object and everything it transitively
refers to can never be mutated again. This is the critical invariant -- it
guarantees the reference graph is stable, so the SCC decomposition remains valid
forever. The concept originates in Pony's `val` reference capability (Clebsch is
Pony's creator) and Project Verona's region/freeze model (Parkinson leads Verona).

**What this eliminates.** Traditional RC systems that encounter cycles must use
either: (a) a backup cycle collector (tracing GC), sacrificing RC's promptness
and determinism; or (b) weak references, requiring programmer discipline. This
paper's approach avoids both by leveraging the immutability guarantee to make
cycle detection a one-time structural operation rather than an ongoing runtime
cost.

**Nature of the contribution.** The paper is explicitly subtitled "an Intellectual
Abstract" -- it presents the conceptual framework and correctness argument, not a
full implementation with benchmarks. The runtime overhead analysis is theoretical
(one-time near-linear SCC + union-find, then standard RC at SCC granularity).

---

## 11.2 What AIMS Should Adopt

### Keep

**K1. Do not hardcode acyclicity into the RC header layout.**
The current `ori_rt` V3 header (`compiler/ori_rt/src/rc/mod.rs`) is:
```
[data_size: i64 | strong_count: i64 | data bytes ...]
```
This 16-byte header has no bits that encode "this object is acyclic" or "this
object participates in a cycle." That is the correct state of affairs. The header
layout must remain cycle-agnostic -- it should neither assert acyclicity nor
include cycle-detection machinery. If Ori later adds frozen cyclic graphs, the
SCC representative's `strong_count` can serve as the SCC's refcount without
header changes. Objects in the same SCC that are NOT the representative would
need a way to redirect RC operations to the representative (see K5).

**K2. Keep `ori_rc_inc` / `ori_rc_dec` as thin dispatch points.**
The current RC functions (`compiler/ori_rt/src/rc/mod.rs:106-261`) have clean
fast paths (null check, immortal sentinel check, atomic inc/dec, underflow check,
drop dispatch). This structure is compatible with future cycle support: an SCC-
aware runtime would add a "frozen-SCC redirect" check alongside the existing
immortal sentinel check, not restructure the entire function. The immortal
sentinel pattern (check-and-skip at `MAX_REFCOUNT`) already demonstrates the
abstraction: a per-object metadata check that short-circuits the normal RC path.

**K3. Preserve the `drop_fn` callback architecture.**
`ori_rc_dec` takes a `drop_fn: Option<extern "C" fn(*mut u8)>` that handles
child field decrements and deallocation. For SCC-frozen objects, the drop function
would instead decrement the SCC representative's count and, only when the SCC
count reaches zero, deallocate all objects in the SCC. The callback architecture
is the correct extension point -- no restructuring needed.

**K4. Keep `ori_rc_is_unique` cycle-unaware.**
`ori_rc_is_unique` (`compiler/ori_rt/src/rc/mod.rs:345-363`) checks `refcount == 1`.
For frozen-SCC objects, uniqueness is defined at SCC granularity: the entire SCC
is unique when only one external reference exists. The current function's contract
("am I the sole owner?") remains correct -- a future SCC-aware path would redirect
the check to the SCC representative's count. No semantic change needed.

**K5. Preserve spare header capacity for future metadata.**
The V3 header uses 16 bytes (8 for `data_size`, 8 for `strong_count`). If the
paper's approach is adopted, objects in an SCC need a way to find their
representative. Two plausible future approaches, neither requiring header
expansion:
- **In-band**: repurpose `strong_count` of non-representative objects as a
  pointer/offset to the representative (detectable by a flag bit or sentinel
  value).
- **Side table**: an external map from object address to SCC representative,
  consulted only for frozen objects (identified by a flag bit in `strong_count`
  or a separate per-object tag).

Neither approach requires adding fields to the current header. The critical
constraint is: **do not pack new semantics into the existing `strong_count` or
`data_size` fields in a way that would conflict with either approach.** The
current immortal sentinel (`MAX_REFCOUNT = isize::MAX`) uses the top of the
value range; a future frozen-SCC sentinel could use a different sentinel value
or bit, as long as the ranges do not overlap. Document this in the runtime.

### New Invariants

**I1. Document the acyclicity assumption in AIMS analysis.**
AIMS currently assumes acyclic reference graphs implicitly -- no analysis pass
checks for or handles cycles. This assumption should be made explicit:
- In `compiler/ori_arc/src/aims/mod.rs`: document that AIMS assumes all reference
  graphs are acyclic (DAGs). Backward dataflow converges because the lattice has
  finite height (15) and transfer functions are monotone, not because of graph
  acyclicity. However, the RC emission logic (inc/dec placement, drop ordering)
  assumes that decrementing a parent's refcount will eventually reach all
  children via the drop chain -- cycles would cause this chain to loop.
- The assumption holds today because Ori's value semantics (capture by value, no
  shared mutable references, no interior mutability) structurally prevent most
  cycles. The only future cycle source would be an explicit `freeze` operation
  on mutable graphs built via unsafe FFI or a future graph-building API.

**I2. Document the drop-chain acyclicity invariant in `ori_rt`.**
`ori_rc_dec` calls `drop_fn` which recursively decrements child fields. If there
is a cycle, this chain would loop infinitely (or stack-overflow). Add a comment
to `call_drop_fn` (`compiler/ori_rt/src/rc/mod.rs:269`) documenting that drop
functions assume acyclic reference graphs. This invariant would be relaxed by
SCC-frozen cycle support (frozen objects bypass the normal drop chain).

**I3. Ori's value semantics as the primary cycle prevention mechanism.**
Ori's design pillars (capture by value, no shared mutable references, no GC, no
interior mutability) make cycles structurally impossible in safe Ori code:
- **Capture by value**: closures copy their environment, not reference it.
- **No shared mutable refs**: cannot create back-edges in an object graph.
- **Immutable by default with `let $x`**: rebinding creates new values, not
  graph mutations.
- **No self-referential types**: struct fields cannot reference the enclosing
  struct (no `Rc<RefCell<Self>>` pattern).

This should be documented as a language-level invariant that the runtime relies
on. If any future language feature breaks this (e.g., `freeze` for mutable
graphs, explicit cycle construction), the runtime must be updated simultaneously.

---

## 11.3 What AIMS Should Not Adopt

### Reject

**R1. Reject SCC computation in the current pipeline.**
The paper's SCC-at-freeze approach requires: (a) a freeze operation that
transitions mutable graphs to deeply immutable; (b) a graph traversal at freeze
time to compute SCCs; (c) union-find to collapse SCCs; (d) modified RC ops to
redirect to SCC representatives. None of these exist in Ori. There is no `freeze`
operation, no mutable graph construction API, no cyclic data, and no mechanism
to transition objects from mutable to immutable at a well-defined program point.
Building any of this machinery now would be pure waste.

**Reasoning:** Ori's value semantics prevent cycles in safe code. The only
plausible cycle source would be a future `freeze` operation on explicitly-
constructed mutable graphs (similar to Verona regions). That feature is not on
any Ori roadmap. Building SCC machinery for hypothetical cycles would violate
AIMS's "no bolted-on passes" principle -- the machinery has no analysis
dimension to participate in and no converged state to read from.

**R2. Reject cycle detection flags in the type system or `ArcClass`.**
Some RC systems (e.g., Python, CPython's `gc` module) mark types as
"potentially cyclic" to enable/disable cycle collection. The paper's approach
eliminates this need (SCC computation at freeze time handles all shapes), and
Ori's type system provides no cycle-relevant information today (all heap types
are `DefiniteRef`; no `Cyclic` / `Acyclic` distinction). Adding such a
distinction to `ArcClass` (`compiler/ori_arc/src/lib.rs`) would add complexity
with zero benefit.

**R3. Reject `may_create_cycle` flag in `EffectClass`.**
The current `EffectClass` (`compiler/ori_arc/src/aims/lattice/dimensions.rs:204`)
has three boolean flags: `may_alloc`, `may_share`, `may_throw`. Adding
`may_create_cycle` was considered. Reject: cycles are structurally prevented by
Ori's value semantics, so the flag would be `false` for every operation in every
program. A flag that is always false carries no information and wastes a bit of
lattice height. If Ori later adds a cycle-forming primitive, `EffectClass` is
the right place for the flag, but that addition should be made simultaneously
with the language feature, not speculatively now.

**R4. Reject union-find data structures in `ori_rt` or `ori_arc`.**
The paper uses union-find to collapse SCCs into equivalence classes. This is a
runtime data structure that only makes sense alongside a freeze operation. Adding
it now would be dead code. The AIMS plan's existing `graph/scc/mod.rs` (Tarjan's
SCC for the call graph) is a compile-time algorithm for interprocedural analysis,
not a runtime object-graph algorithm -- it should not be repurposed or confused
with the paper's approach.

**R5. Reject lazy mark-scan or backup cycle collection.**
The paper explicitly positions itself as an alternative to backup cycle
collectors (tracing GC, lazy mark-scan). Ori should not adopt any form of
backup cycle collection:
- No tracing GC (violates Ori's deterministic memory model).
- No lazy mark-scan (runtime overhead for a problem that does not exist).
- No weak-reference workarounds (adds programmer burden for impossible cycles).
The correct Ori approach, if cycles become possible, is the paper's freeze+SCC
approach, not a traditional cycle collector.

---

## 11.4 Plan Edits

**PE1. Stage 5 scope clarification in `plans/aims/00-overview.md`.**
The current Stage 5 entry (line 411) reads:
```
Stage 5 -- Runtime follow-ons (separate efforts)
  SCC-based frozen-cycle RC
```
This should be expanded to clarify the prerequisites and the paper's contribution:
- **Prerequisite**: A `freeze` operation or equivalent language feature that
  transitions mutable object graphs to deeply immutable.
- **Prerequisite**: Ori must have a mechanism for constructing cyclic graphs
  (currently impossible in safe code).
- **Paper contribution**: SCC + union-find at freeze time; RC at SCC granularity.
- **Not on any current roadmap**: This is a contingency plan, not a planned
  feature.

No code changes to Stage 5 scope are needed -- the existing plan correctly
identifies this as a "separate effort" that "should NOT block AIMS-core work."

**PE2. Section 07.4 future item is sufficient as-is.**
The `SCC-Frozen Cyclic RC` entry in `plans/aims/section-07-advanced.md` (lines
369-376) already has the correct AIMS prerequisites documented:
```
ShapeClass and Locality dimensions must be precise enough to
identify frozen/immutable subgraphs. Stage 1 conservative defaults
are insufficient -- this needs Stage 4+ locality precision.
```
No edits needed. The entry correctly identifies this as a future extension that
reads AIMS facts rather than participating in AIMS analysis.

**PE3. No changes to Section 09 (Dimensional Fusion).**
The dimensional fusion plan does not need to accommodate cycle-related
dimensions. The current 7-dimension lattice is correct for acyclic reference
graphs. If cycles are introduced, a new analysis dimension (or an `EffectClass`
flag) would be added at that time, not preemptively.

---

## 11.5 Code Changes (Later)

These are implementation notes for if/when Ori adds frozen cyclic data structures.
None of these are actionable now.

**C1. `compiler/ori_rt/src/rc/mod.rs` -- document sentinel value ranges.**
Add a comment near `MAX_REFCOUNT` (line 87) documenting the reserved sentinel
value ranges and their interaction with future extensions:
- `MAX_REFCOUNT` (`isize::MAX`): immortal sentinel (current).
- Range `[MAX_REFCOUNT - 1, MAX_REFCOUNT - 256]` (hypothetical): reserved for
  future SCC-frozen sentinel values. Not yet allocated; merely a note that
  future extensions should not collide with the immortal sentinel.

This is a documentation-only change.

**C2. `compiler/ori_rt/src/rc/mod.rs` -- document drop-chain acyclicity.**
Add a `// INVARIANT:` comment to `call_drop_fn` (line 269) documenting that the
drop chain assumes acyclic reference graphs. SCC-frozen cycle support would
replace the drop function for frozen objects with an SCC-aware deallocator.

**C3. `compiler/ori_arc/src/aims/mod.rs` -- document acyclicity assumption.**
Add a `//!` module doc paragraph to the AIMS root module stating:
> AIMS assumes all reference graphs are acyclic (DAGs). This holds because Ori's
> value semantics (capture by value, no shared mutable references) structurally
> prevent cycles in safe code. If a future language feature introduces cycles
> (e.g., `freeze` on mutable graphs), AIMS emission must be updated to handle
> cyclic drop chains (SCC-frozen objects bypass normal `ori_rc_dec` drop).

**C4. `compiler/ori_arc/src/aims/lattice/dimensions.rs` -- EffectClass future flag.**
If Ori adds a cycle-forming primitive, add `may_cycle: bool` to `EffectClass`.
This would participate in FIP certification (cyclic allocation blocks FIP) and
inform the AIMS emission layer to use SCC-aware RC ops. Chain height increases
from 3 to 4 (four independent booleans). No structural changes to the lattice
framework needed -- `EffectClass::join` is already componentwise OR.

**C5. `compiler/ori_rt/src/rc/mod.rs` -- SCC-redirect in `ori_rc_inc` / `ori_rc_dec`.**
If SCC-frozen objects are introduced, the RC fast path gains one additional
check (after the immortal sentinel check):
```
// Fast path: immortal
if refcount == MAX_REFCOUNT { return; }
// Fast path: SCC-frozen (hypothetical)
if is_scc_frozen(data_ptr) {
    let rep = scc_representative(data_ptr);
    ori_rc_inc(rep);  // redirect to SCC representative
    return;
}
// Normal path: atomic inc
```
The `is_scc_frozen` check could be a sentinel value in `strong_count`, a bit
flag, or a side-table lookup. The paper does not mandate a specific encoding.

---

## 11.6 Lens Shift

**What this changes about reading Paper 12 (Double-Ended Bit-Stealing).**

Paper 12 (Elsman, ICFP 2024) proposes using both low and high pointer bits for
compact ADT representation. This paper (11) introduces a potential future
constraint on those bits:

1. **High bits in `strong_count`.** If SCC-frozen objects are identified by a
   sentinel value or flag bit in `strong_count`, the bit-stealing paper's use of
   high pointer bits must not conflict with the SCC encoding. The current
   `strong_count` is a full `i64`; the immortal sentinel uses the maximum positive
   value. A future SCC sentinel would use a different value or a high bit. Paper
   12's bit-stealing targets data pointers and tag fields, not the RC header, so
   there is likely no conflict -- but the reviewer should verify that Paper 12's
   representation changes do not extend into the 16-byte RC header prefix.

2. **Object identity under union-find.** The paper's SCC approach collapses
   multiple objects into one equivalence class. If Paper 12's bit-stealing
   encodes type discriminants into pointers, those encoded pointers must survive
   the SCC redirect (the pointer to an object must still decode correctly even
   if RC operations are redirected to a different object's header). This is
   likely a non-issue (bit-stealing encodes the pointer itself, not where its
   RC is stored), but should be explicitly verified.

3. **Frozen objects are read-only.** The paper's deeply immutable guarantee means
   frozen objects' bit-encoded representations can never change after freeze.
   This is favorable for bit-stealing: once the representation is committed, it
   is stable forever. No COW checks needed for frozen data. Paper 12 should note
   that frozen-immutable data is the ideal target for aggressive representation
   optimization (no runtime representation changes possible).

**Cumulative lens shift:** Papers 10-12 form a trilogy of boundary-setting
reviews. Paper 10 (Concurrent RC) set the boundary for runtime atomicity. Paper
11 (this one) sets the boundary for cyclic data. Paper 12 (Bit-Stealing) sets
the boundary for representation optimization. The shared theme: AIMS core should
produce facts (uniqueness, shape, locality, effect) that these future systems
consume, but none of these systems should feed back into AIMS analysis. The data
flow is strictly one-directional: AIMS analysis -> runtime/representation
decisions.

---

## 11.7 Open Risk

**OR1. Ori's value semantics are the sole cycle-prevention mechanism.**
There is no runtime check, no type-system enforcement, and no analysis pass
that verifies acyclicity. The guarantee comes entirely from language-level
design decisions (no shared mutable references, capture by value). If any
future feature weakens these guarantees -- even partially -- the entire RC
system would silently corrupt:
- `ori_rc_dec` drop chains would loop infinitely on cycles.
- AIMS backward dataflow would still converge (lattice height is the bound),
  but emission decisions would be unsound (drop ordering assumes DAG structure).
- No diagnostic would fire. No runtime check would catch it. Silent UB.

**Mitigation:** If a feature is proposed that could introduce cycles (FFI
graph construction, `freeze` on mutable regions, interior mutability), the
proposal must include a cycle-safety analysis or runtime check. The paper's
SCC-at-freeze approach is the cleanest option (one-time cost, then standard RC).

**OR2. The `drop_fn` callback has no depth limit.**
`call_drop_fn` (`compiler/ori_rt/src/rc/mod.rs:269`) catches panics but has no
recursion depth limit. A deeply nested acyclic graph could also stack-overflow
the drop chain (not just cycles). This is a pre-existing risk unrelated to
cycles, but the paper's approach (SCC-aware deallocation that iterates rather
than recurses) would also address it. For now, the LLVM codegen generates
iterative drop functions for simple cases; deeply nested types still recurse.

**OR3. The immortal sentinel may be too coarse for SCC extension.**
The immortal sentinel uses `MAX_REFCOUNT` (a single value in a 64-bit space).
If frozen-SCC objects need their own sentinel, they need a different value.
Two sentinels in the RC fast path means two branches per `ori_rc_inc` /
`ori_rc_dec`. This is acceptable (branch prediction handles it), but should
be planned as a single extension rather than accumulating sentinels
incrementally.

**OR4. No runtime-level object graph traversal exists.**
The paper assumes the ability to traverse an object's reference graph at freeze
time (to compute SCCs). Ori's runtime has no such capability -- `ori_rt` has
no `trace` / `visit_children` callback on RC'd objects. The `drop_fn` callback
*decrements* children but does not *enumerate* them. Adding a
`visit_children_fn` callback would be a prerequisite for implementing the
paper's approach. This callback would need to be generated per-type by LLVM
codegen, similar to `drop_fn` but without the deallocation step.
