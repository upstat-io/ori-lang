---
section: "10"
title: "Concurrent Immediate Reference Counting"
status: complete
goal: "Define runtime abstraction points for future concurrent RC without contaminating current AIMS core"
paper:
  title: "Concurrent Immediate Reference Counting"
  doi: "https://doi.org/10.1145/3656383"
  venue: "PLDI 2024"
  authors: "Jaehwang Jung, Jeonghyeon Kim, Matthew J. Parkinson, Jeehoon Kang"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09"]
sections:
  - id: "10.1"
    title: "Paper Thesis"
    status: complete
  - id: "10.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "10.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "10.4"
    title: "Plan Edits"
    status: complete
  - id: "10.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "10.6"
    title: "Lens Shift"
    status: complete
  - id: "10.7"
    title: "Open Risk"
    status: complete
---

# Section 10: Concurrent Immediate Reference Counting

**Status:** Complete
**Goal:** Define what runtime abstraction points should exist *now* so that future
concurrent RC is possible, and what complexity must be explicitly kept out of the
current AIMS branch. This is a boundary-setting review.

**Paper:** Jung, Kim, Parkinson, & Kang, "Concurrent Immediate Reference Counting,"
PLDI 2024. [DOI: 10.1145/3656383](https://doi.org/10.1145/3656383)
Associated with Microsoft Research Project Verona.

**Why read this tenth:** Mostly a boundary-setting read. It tells you what should stay
out of AIMS core for now -- deferred decrements, epoch-based reclamation, atomic operations
-- while identifying abstraction points that future concurrent RC would need.

**Pause questions:**
- What runtime abstraction points should exist now so future concurrent RC is possible?
- What complexity must be explicitly kept out of the current branch?

**AIMS context:**
- Stage 5 lists "Concurrent runtime strategies" as a separate effort
- `ori_rt` currently uses atomic RC by default (`AtomicI64`), non-atomic under `single-threaded` feature
- AIMS analysis is single-threaded (Salsa handles parallelism)
- Section 07 (Advanced) mentions CIRC and concurrent RC as a future follow-on
- Ori has `Sendable` trait for cross-thread safety
- `RcStrategy` enum in `ir/repr.rs` describes how to perform inc/dec per type shape

**Note on AIMS context correction:** The original text stated "`ori_rt` currently uses
non-atomic reference counting." This is incorrect. `ori_rt` uses `AtomicI64` with
`fetch_add(Relaxed)` for increment and `fetch_sub(Release)` for decrement by default.
The non-atomic path is behind `#[cfg(feature = "single-threaded")]`. The default build
is already thread-safe for reference count manipulation.

---

## 10.1 Paper Thesis

CIRC addresses a fundamental tension in concurrent memory management: **traditional
reference counting provides deterministic, immediate reclamation of garbage but performs
poorly in concurrent settings, while safe memory reclamation (SMR) algorithms like
epoch-based reclamation (EBR) and hazard pointers (HP) handle concurrency well but
suffer from unbounded reclamation delays and memory growth.**

The paper's core contribution is a hybrid scheme combining SMR with reference counting
that achieves two properties simultaneously:

1. **Immediate recursive reclamation of linked structures.** When a reference count
   reaches zero, CIRC does not merely mark the object for later collection. It
   identifies outgoing references (via the `RcObject::pop_edges()` trait method),
   decrements them immediately, and recursively reclaims any transitively-unreachable
   chain. This avoids the "garbage pileup" problem where deferred-decrement schemes
   (OrcGC, CDRC) accumulate unbounded garbage during long critical sections.

2. **Immediate application of decrements without deferred buffering.** Unlike OrcGC
   (which buffers decrements in thread-local logs and applies them lazily) or CDRC
   (which defers decrements until epoch advancement), CIRC applies decrements at the
   point of the `drop`/`store` that triggers them. Only *reclamation* (the actual
   `free()`) is deferred to epoch-safe points -- not the reference count manipulation
   itself.

The key architectural insight is the separation of two concerns that prior work conflated:

- **Counted references** (`Rc<T>`): Owning pointers that participate in reference
  counting. Incrementing and decrementing happen immediately and atomically.
- **Uncounted references** (`Snapshot<'g, T>`): Temporary local pointers protected by
  EBR guard lifetimes, not by reference counting. Loading a `Snapshot` from an
  `AtomicRc` does NOT increment the reference count, avoiding the atomic-increment
  overhead during traversals.

This split means that read-heavy workloads (traversals, lookups) avoid all reference
count manipulation -- they use `Snapshot` pointers whose validity is guaranteed by the
EBR critical section. Only ownership transfers (store, swap, clone-for-retention)
touch the reference count.

**Performance claims:** CIRC avoids the memory growth instability of competing
approaches (OrcGC, Fast Reference Counter, CDRC) while matching or exceeding their
throughput on update-heavy workloads. The stability comes from immediate reclamation:
garbage does not accumulate across epoch boundaries.

**Scope:** CIRC targets concurrent lock-free data structures (linked lists, trees,
hash maps) in unmanaged languages. It is a library-level mechanism, not a
compiler-level one. There is no discussion of static analysis, compiler-guided RC
elision, or borrow inference. The compiler's role is limited to ensuring `RcObject`
is correctly implemented (exposing outgoing edges for recursive reclamation).

**Proof obligations:** CIRC establishes correctness through the following invariants:
1. *Counted-reference safety:* Every `Rc<T>` increment/decrement is atomic and
   the standard Release/Acquire ordering contract holds (writes visible before
   deallocation, deallocating thread sees all prior writes).
2. *Snapshot validity:* An uncounted `Snapshot<'g, T>` is valid for the duration
   of the EBR guard `'g`. The Rust type system enforces this at compile time.
3. *Recursive reclamation completeness:* When refcount reaches zero, `pop_edges()`
   is called before `free()`, ensuring all transitively-reachable zero-count
   objects are reclaimed in the same epoch. No garbage chain survives across epochs.
4. *Deferred-free safety:* The actual `free()` is deferred until the epoch advances
   past all active guards, ensuring no `Snapshot` holder accesses freed memory.

### Comparison with Related Concurrent RC Approaches

CIRC is one of several concurrent RC strategies. Understanding the landscape
informs which abstraction points AIMS needs.

**Biased Reference Counting** (Choi, Shull, & Torrellas, PACT 2018): Assigns each
object an "owner thread." The owner's counter is updated non-atomically; a second
"shared counter" is updated atomically by other threads. Deallocation happens when
both counters reach zero. BRC achieves 22.5% execution-time reduction on average
by avoiding atomics on the common (owner-thread) path. The insight: most objects
are accessed by a single thread. This is directly relevant to Ori because Ori's
`Sendable` trait gates cross-thread transfer -- objects that never cross a channel
boundary are de facto single-threaded. A biased-RC scheme could use AIMS's
`Locality` dimension to identify never-sent objects and skip atomic operations
entirely, without needing EBR or epoch machinery. Biased RC is a smaller step
than CIRC and could be a Stage 5a intermediate target.

**Lean 4's `m_rc` encoding** (Ullrich & de Moura, IFL 2019; Lean 4 runtime): Lean
encodes the threading mode *in the reference count field itself*: `m_rc > 0` means
single-threaded (non-atomic inc/dec), `m_rc < 0` means multi-threaded (atomic
inc/dec), `m_rc == 0` means RC-free (scalar/persistent). The sign bit serves as a
per-object mode selector with zero overhead -- no feature flag, no global mode
switch. When an object is sent to another thread, its `m_rc` is negated (flipped
to multi-threaded mode), and all future operations use atomics. This is a more
granular version of Ori's current `single-threaded` feature flag: instead of a
global compile-time choice, it is a per-object runtime choice. Lean's approach
is notable because the borrow-inference pass (which AIMS replaces) operates
identically regardless of the threading mode -- the analysis is about ownership
and liveness, not about atomicity. The atomicity decision is deferred to the
runtime, exactly as AIMS should do.

**Swift's `InlineRefCounts`**: Swift stores strong + unowned reference counts
inline in the object header (64-bit packed). When weak references or overflow
occur, a side-table is allocated. The `isUniquelyReferenced` fast path checks
`strong_extra == 0 && !has_side_table && !is_deiniting` -- all non-atomic when
the object is provably thread-local. Swift always uses atomic operations for the
refcount itself but has a non-atomic fast path for the uniqueness check when
conditions allow. This is relevant to Ori because AIMS's `StaticUnique` COW mode
already eliminates the runtime uniqueness check entirely -- a stronger optimization
than Swift's conditional non-atomic check.

**Koka's Perceus runtime**: Koka compiles to C and its `kklib` runtime uses
atomic RC operations by default. Perceus's static analysis (borrow inference +
reuse) reduces the number of RC operations so aggressively that the overhead of
atomicity on the remaining operations is negligible. This validates AIMS's
approach: invest in reducing RC operation count through analysis rather than
optimizing the cost per operation through runtime tricks.

### Section 09 Lens Question: Does Concurrent Execution Change the Demand Algebra?

Section 09.6 raised this question. The answer from CIRC is: **no, the demand
algebra is unchanged.** Concurrent execution does not change whether a variable is
used `Once` or `Many` -- cardinality is a per-thread, per-execution-path property.
`seq_add(Once, Once) = Many` remains correct: if a thread uses a value twice
sequentially, it needs RC >= 2 regardless of other threads. What changes under
concurrency is the *implementation* of RC operations (atomic vs non-atomic), not
the *analysis* of how many operations are needed. AIMS's backward cardinality
inference is thread-model-independent. This confirms Section 09's prediction.

---

## 10.2 What AIMS Should Adopt

### Keep

**K1. The `ori_rt` function-call boundary is already the correct abstraction point.**
AIMS emits `ArcInstr::RcInc` and `ArcInstr::RcDec` instructions that the LLVM emitter
translates to calls to `ori_rc_inc(data_ptr)` and `ori_rc_dec(data_ptr, drop_fn)`.
These are extern "C" function calls, not inlined atomic operations. This indirection
is exactly the right boundary for future concurrent RC: replacing `ori_rc_inc`'s
implementation (from an atomic `fetch_add` to a CIRC-style counted-reference
increment) requires zero changes to `ori_arc`, `ori_llvm`, or any AIMS analysis code.
The ARC IR, the LLVM call sites, and all analysis stay unchanged. Only `ori_rt`
changes. **This boundary must be preserved.**

The current function signatures:
- `ori_rc_inc(data_ptr: *mut u8)` -- increment
- `ori_rc_dec(data_ptr: *mut u8, drop_fn: Option<extern "C" fn(*mut u8)>)` -- decrement + drop
- `ori_rc_is_unique(data_ptr: *const u8) -> bool` -- COW uniqueness check

These three functions are the entire RC API surface between compiler-generated code
and the runtime. A CIRC-style backend would replace their implementations while
preserving the signatures. The `drop_fn` parameter on `ori_rc_dec` maps naturally
to CIRC's `RcObject::pop_edges()` -- the drop function already traverses child
fields and calls `ori_rc_dec` on each, which is the recursive-reclamation pattern
CIRC requires.

**K2. The `RcStrategy` enum correctly separates "what to RC" from "how to RC."**
`RcStrategy` (`compiler/ori_arc/src/ir/repr.rs`) describes the *shape* of the value
(HeapPointer, FatPointer, Closure, AggregateFields, InlineEnum) so the LLVM emitter
knows how to extract the data pointer and which drop function to use. It says nothing
about the *mechanism* of reference counting (atomic vs non-atomic, immediate vs
deferred, counted vs uncounted). This is correct. A concurrent RC backend would use
the same `RcStrategy` values -- it still needs to know "extract field 1 for the data
pointer" regardless of how the reference count is manipulated.

**K3. AIMS correctly avoids embedding reference-count-value assumptions in analysis.**
The AIMS lattice dimensions (`Uniqueness`, `Cardinality`, `Consumption`, `AccessClass`)
reason about abstract ownership properties, not concrete reference count values. AIMS
never reads `ori_rc_count()` during analysis. It never assumes "if uniqueness is
Unique, then refcount == 1." The uniqueness dimension is a *static over-approximation*
of dynamic uniqueness, used to make COW decisions (`StaticUnique` vs `Dynamic`). This
abstraction is compatible with CIRC, where a `Snapshot` pointer to an object does not
affect its reference count -- the object might have refcount == 1 (statically unique)
while multiple threads hold uncounted `Snapshot` references to it. AIMS's analysis
would remain sound because `Uniqueness::Unique` is never claimed when aliasing is
possible.

**K4. The `single-threaded` feature flag pattern is the right runtime-selection
mechanism for now.** `ori_rt` already has `#[cfg(feature = "single-threaded")]` /
`#[cfg(not(...))]` dual paths in `ori_rc_inc`, `ori_rc_dec`, `ori_rc_is_unique`,
and all collection-buffer RC functions. A future concurrent RC backend would be
another feature variant, not a replacement of the default path. The compile-time
feature-flag approach matches CIRC's design: the choice between `Rc` (counted) and
`Snapshot` (uncounted) is made at type level, not dynamically.

**Comparison with Lean 4's per-object approach:** Lean 4 encodes the threading
mode in the sign bit of `m_rc` (positive = single-threaded, negative =
multi-threaded), giving per-object granularity. Ori could adopt this approach in
Stage 5 as an alternative to global feature flags: objects that never cross
`Sendable` channel boundaries stay in non-atomic mode, while sent objects flip to
atomic mode. This would require (a) adding a mode bit to the RC header or
repurposing the sign bit, (b) updating `ori_rc_inc`/`ori_rc_dec` to branch on the
mode, and (c) the LLVM emitter to emit a "mark as shared" call when values are
sent through channels. The analysis (AIMS) would be unchanged -- the mode
decision is purely a runtime concern. The current feature-flag approach is correct
for Stage 1-4 and should not be replaced prematurely.

**K5. The `drop_fn` parameter on `ori_rc_dec` is the recursive-reclamation hook.**
CIRC's key innovation is recursive reclamation via `pop_edges()`. In Ori, this is
already implemented: the compiler generates per-type drop functions that traverse
RC'd child fields and call `ori_rc_dec` on each. The `drop_fn: Option<extern "C"
fn(*mut u8)>` parameter on `ori_rc_dec` is the mechanism. When refcount reaches zero,
the drop function is called, which decrements children, which may trigger their drop
functions, producing exactly the recursive reclamation chain CIRC describes. No
changes needed to this pattern.

### New Invariants

**N1. AIMS must not emit RC operations that assume synchronous reclamation ordering.**
Currently, AIMS emits `RcDec` and assumes the memory is freed (or drop function
called) immediately when the count reaches zero. If Ori later adopts CIRC-style
deferred reclamation (where `free()` is delayed to epoch-safe points even though the
decrement is immediate), the ordering assumption "memory is freed before the next
instruction executes" would break. Today this is not a problem because AIMS never
reads freed memory. But any future optimization that assumes "after RcDec, the
allocation is available for reuse in the next instruction" (reuse tokens) must be
aware that deferred reclamation means the allocation might not actually be freed yet.

**Current AIMS reuse emission** (`compiler/ori_arc/src/aims/emit_reuse/`) already
handles this correctly: `Reset`/`Reuse` instructions are emitted only when the
value's uniqueness is statically proven (`Uniqueness::Unique`), meaning the current
thread is the sole owner. In a CIRC world, `Unique` would still mean sole *counted*
owner -- uncounted `Snapshot` references are protected by EBR and would not be
present when a reuse token is created (the EBR guard would prevent the reuse from
being visible to snapshot holders). This is compatible, but the invariant should be
documented: **reuse tokens require sole counted ownership, not merely zero uncounted
references.**

**N2. AIMS must not assume that `ori_rc_is_unique` returning true means "no other
thread can see this object."** Currently, `ori_rc_is_unique` checks `refcount == 1`
with `Relaxed` ordering. In a CIRC world, an object could have refcount == 1 while
uncounted `Snapshot` references exist on other threads. The `Relaxed` read could
observe refcount == 1 as a stale value after a concurrent decrement. The existing
`ori_rc_is_unique` documentation already acknowledges this:

> A stale read of RC=2 when the true value is 1 is safe (we take the slow copy path
> unnecessarily). A stale read of RC=1 when the true value is 2 is impossible: the
> incrementing thread must have cloned from an existing reference.

This reasoning holds for counted references but not for uncounted `Snapshot`
references. If CIRC-style snapshots are adopted, `ori_rc_is_unique` would need to be
redefined to mean "sole counted owner" (which is sufficient for COW: if you are the
sole counted owner, no other thread will modify the object through a counted
reference, and snapshot holders are read-only by CIRC's design). AIMS's COW emission
(`compiler/ori_arc/src/aims/emit_rc/cow.rs`) uses `ori_rc_is_unique` only for the
`Dynamic` COW mode -- `StaticUnique` skips the runtime check entirely. This is
compatible: static uniqueness means no aliasing at all, which is stronger than
CIRC's counted-uniqueness.

**N3. The `MAX_REFCOUNT` immortal sentinel must remain compatible with any future
RC scheme.** CIRC uses epoch timestamps stored in pointer tag bits, not in the
reference count field. The immortal sentinel (`MAX_REFCOUNT = isize::MAX`) occupies
a value in the refcount space. A future concurrent RC implementation must preserve
the invariant that `MAX_REFCOUNT` means "skip all RC operations." This is trivially
compatible with CIRC (the sentinel check is a fast-path early return in
`ori_rc_inc`/`ori_rc_dec`, before any atomic operation or epoch interaction).

---

## 10.3 What AIMS Should Not Adopt

### Reject

**R1. Reject epoch-based reclamation in `ori_rt`.** CIRC's EBR component (derived
from `crossbeam-epoch`) introduces per-thread epoch counters, a global epoch that
advances periodically, thread-local garbage bags timestamped with the current epoch,
and a `pin()`/`unpin()` critical-section API that all code accessing shared pointers
must use. This adds:
- A global `AtomicUsize` for the epoch counter (contention point under high thread counts)
- Per-thread `LocalEpoch` state (thread-local storage, initialization, cleanup)
- Garbage bags (`Vec<Deferred>`) that accumulate deferred `free()` calls
- A `pin()` call before every shared-pointer access, returning a RAII `Guard`
- Epoch advancement logic (scan all threads' local epochs, advance global epoch
  when all threads have observed the previous epoch)

This machinery is justified only when multiple threads share heap objects protected
by non-counted references. Ori's current concurrency model uses `Sendable`-gated
channels with message-passing semantics, not shared-heap access. Even if Ori later
adds shared-heap concurrency (nursery/task model), the EBR complexity should live
in a dedicated concurrent-data-structure library, not in the core `ori_rt` RC
functions. **Reason:** EBR changes the *API contract* of all pointer accesses (must
be inside a critical section), which would require LLVM codegen changes to emit
guard acquisition/release around every pointer load. This is a pervasive change
that should not be made speculatively.

**R2. Reject uncounted `Snapshot`-style references in the compiler IR.** CIRC's
`Snapshot<'g, T>` is a pointer valid only within an EBR guard scope. Introducing
uncounted references into ARC IR would require:
- A new `ArcInstr` variant for "load uncounted reference" (no `RcInc`)
- Lifetime tracking in the ARC IR to ensure snapshots do not escape guards
- A new lattice dimension or modification to `Uniqueness` to distinguish "counted
  unique" from "unique among counted refs but snapshots may exist"
- LLVM codegen changes to emit guard enter/exit

This is a fundamental IR redesign, not a bolt-on optimization. **Reason:** AIMS's
entire analysis is built on the invariant that every non-scalar, non-borrowed
reference is counted. Removing this invariant would invalidate `Cardinality`
(which counts uses, assuming each use holds a counted reference), `Consumption`
(which tracks ownership transfer of counted references), and reuse eligibility
(which requires sole counted ownership). The cost of re-deriving these properties
for a mixed counted/uncounted model far exceeds the benefit for a language that
does not yet have shared-heap concurrency.

**R3. Reject deferred-decrement buffering.** OrcGC and CDRC buffer decrements in
thread-local logs and apply them lazily. CIRC explicitly rejects this approach
(its title says "immediate") because deferred decrements cause garbage accumulation
and unpredictable reclamation latency. AIMS should likewise reject deferred
decrements: the current `ori_rc_dec` applies the decrement immediately and calls
the drop function inline when the count reaches zero. This is the correct behavior
for a language with deterministic destruction semantics (`Drop` trait). Deferring
decrements would violate Ori's destruction ordering guarantee: `Drop` implementations
may have observable side effects (flushing buffers, releasing locks), and deferring
their execution to an arbitrary later point would be a semantic change visible to
user code. **Reason:** Ori's `Drop` is synchronous and ordered. Deferred decrements
break this contract.

**R4. Reject the `RcObject::pop_edges()` trait requirement.** CIRC requires every
reference-counted type to implement `pop_edges()`, which returns the list of outgoing
`Rc` pointers for recursive reclamation. In Ori, this is already handled by the
compiler-generated drop functions: each type's drop function knows its field layout
and calls `ori_rc_dec` on each RC'd field. The drop function *is* the edge-popping
mechanism. Introducing a separate `pop_edges()` API would duplicate what drop
functions already do. **Reason:** Drop functions are already generated, already
correct, and already recursive. A second edge-enumeration mechanism adds complexity
with no benefit.

**R5. Reject atomic memory orderings stronger than currently used.** The current
`ori_rt` already uses the correct orderings for ARC:
- `Relaxed` for `ori_rc_inc` (increment only needs atomicity, not ordering --
  the incrementing thread already holds a valid reference)
- `Release` for `ori_rc_dec`'s `fetch_sub` (ensures writes through this reference
  are visible before any thread deallocates)
- `Acquire` fence before calling the drop function (ensures the deallocating thread
  sees all prior writes from all threads)

These are the standard Rust `Arc` / Swift `swift_retain`/`swift_release` orderings.
CIRC uses similar orderings for its counted references. Strengthening to
`SeqCst` or adding additional fences would add overhead with no correctness benefit
for Ori's current single-threaded-with-atomic-RC model. **Reason:** The current
orderings are already correct for multi-threaded RC. Changing them would only be
justified if Ori adopted a relaxed-memory-model concurrency primitive (like
CIRC's `Snapshot`) that requires additional synchronization.

**R6. Reject `AtomicRc`-style compare-and-swap operations on reference counts.**
CIRC's `AtomicRc` supports `load`, `store`, and `compare_exchange` on shared
pointer locations. These are needed for lock-free concurrent data structures
(lock-free linked lists, lock-free hash maps). Ori does not have lock-free data
structures in its standard library or language model. Collection mutations go
through `ori_rt` runtime functions that hold exclusive access to the collection's
internal buffer. Adding CAS on reference-counted pointers would serve no current
use case. **Reason:** No consumer exists. Adding infrastructure without consumers
violates the "no speculative complexity" principle.

**R7. Reject biased reference counting in Stage 1-4.** Biased RC (Choi et al.,
PACT 2018) is a strong candidate for Stage 5 but should not be adopted now. The
dual-counter design (owner-thread non-atomic counter + shared atomic counter)
requires per-object owner-thread tracking, which would add a word to the RC header
layout. The current 16-byte header (`[data_size: i64 | strong_count: i64]`) would
become 24 bytes (`[data_size | strong_count | owner_thread_id]`). This layout
change affects every allocation and every `ori_rc_alloc`/`ori_rc_free` call site.
It should be evaluated as part of a Stage 5 runtime redesign, not retrofitted into
the current header. **Reason:** Header layout changes are pervasive. The current
layout is correct and efficient for single-threaded and channel-based concurrency.

**R8. Reject Lean 4-style per-object mode bits in Stage 1-4.** Lean 4's `m_rc`
sign-bit encoding (positive = single-threaded, negative = multi-threaded) adds a
branch to every RC operation to check the threading mode. This branch is well-
predicted on modern CPUs (most objects stay in one mode for their lifetime), but
it is overhead that Ori does not need until shared-heap concurrency exists. The
current feature-flag approach eliminates this branch entirely at compile time.
**Reason:** Runtime branching on RC hot path is only justified when the program
actually uses multiple threading modes. Current Ori programs do not.

---

## Pause Question Answers

**Q1: What runtime abstraction points should exist now so future concurrent RC
is possible?**

Five abstraction points already exist and must be preserved:
1. The `ori_rc_inc` / `ori_rc_dec` / `ori_rc_is_unique` extern "C" function
   boundary (K1) -- the sole interface between compiler-generated code and the
   RC mechanism. Swapping implementations requires only relinking `ori_rt`.
2. The `RcStrategy` enum (K2) -- describes value shape, not RC mechanism.
3. The `drop_fn` parameter on `ori_rc_dec` (K5) -- the recursive-reclamation
   hook, equivalent to CIRC's `pop_edges()`.
4. The `single-threaded` feature flag (K4) -- compile-time variant selection.
5. The AIMS lattice's abstract ownership reasoning (K3) -- never embeds concrete
   refcount values, never assumes synchronous reclamation.

No new abstraction points are needed. The existing boundaries are sufficient.

**Q2: What complexity must be explicitly kept out of the current branch?**

Eight categories of complexity are excluded (R1-R8):
- Epoch-based reclamation (R1) -- pervasive API change
- Uncounted `Snapshot`-style references in IR (R2) -- fundamental IR redesign
- Deferred-decrement buffering (R3) -- violates `Drop` ordering
- Separate `pop_edges()` trait (R4) -- drop functions already serve this role
- Stronger atomic orderings (R5) -- current orderings are correct
- Compare-and-swap on RC pointers (R6) -- no consumer exists
- Biased RC dual counters (R7) -- header layout change, Stage 5 concern
- Per-object mode bits (R8) -- runtime branching on hot path, premature

---

## 10.4 Plan Edits

### Section 07 (`plans/aims/section-07-advanced.md`)

**E1. Expand the CIRC bullet in 07.4 with boundary documentation.** The current
text (07.4, "Concurrent RC Strategies") is three sentences plus an AIMS prerequisite
paragraph. It should be expanded to document:
- The specific `ori_rt` functions that constitute the RC API boundary:
  `ori_rc_inc`, `ori_rc_dec`, `ori_rc_is_unique`, `ori_rc_is_unique_or_null`
- The invariant that AIMS analysis must not embed concrete refcount-value assumptions
- The invariant that drop functions serve as the recursive-reclamation hook
- The compatibility of the current `RcStrategy` enum with concurrent backends
- The explicit exclusion of EBR, uncounted references, and deferred decrements
  from AIMS core (with reference to this review)

**E2. Add a "Runtime Abstraction Boundary" subsection to 07.4.** Currently 07.4 is
a flat list of future items. Add a structured subsection documenting the five
`ori_rt` functions that are the RC API boundary, their signatures, their memory
orderings, and the invariant that they must remain the sole interface between
compiler-generated code and the RC mechanism. This makes the boundary explicit
rather than implicit.

### Stage 5 scope (`plans/aims/00-overview.md`)

**E3. Refine Stage 5 "Concurrent runtime strategies" to be more specific.** The
current text is two bullet points: "SCC-based frozen-cycle RC" and "Concurrent
runtime strategies." Replace the second with:
- "Concurrent runtime strategies: evaluate CIRC-style counted/uncounted split
  for `Sendable` channel-based concurrency. Requires: (a) shared-heap access
  model defined, (b) `ori_rt` RC API boundary preserved, (c) EBR guard
  emission in LLVM codegen. Should NOT require changes to `ori_arc` analysis
  or AIMS lattice."

This scopes the effort precisely and documents the preconditions.

**E3a. Add a biased-RC intermediate target to Stage 5.** The CIRC paper and the
biased-RC paper (PACT 2018) suggest a natural progression for concurrent RC:
1. Stage 5a: Lean 4-style per-object mode bits (sign-bit encoding in `m_rc`).
   No EBR, no epoch machinery. Objects default to non-atomic; flip to atomic
   when sent through a `Sendable` channel. Requires: header layout decision,
   `ori_rc_inc`/`ori_rc_dec` branch on mode, LLVM emitter emits "mark shared"
   at channel send sites.
2. Stage 5b: Biased RC (dual counters, owner-thread fast path). Requires:
   24-byte header, per-object owner tracking, deallocation protocol when both
   counters reach zero.
3. Stage 5c: CIRC-style EBR integration (only if lock-free data structures are
   added to the standard library). Requires: EBR guard emission, `Snapshot`
   type in the runtime, fundamental API changes.

Each stage is independently valuable and independently deployable. Document
this progression in the Stage 5 section of `00-overview.md` so the path from
current state to full concurrent RC is explicit.

### `ori_rt` module documentation

**E4. Add a "Concurrent RC Compatibility" section to `compiler/ori_rt/src/rc/mod.rs`
module doc.** The current module doc lists the RC functions but does not document
which properties the compiler relies on. Add a section stating:
- The compiler emits calls to `ori_rc_inc`/`ori_rc_dec`/`ori_rc_is_unique` without
  assumptions about the implementation mechanism
- The signatures are stable API: changing them requires updating all LLVM call
  sites in `ori_llvm/src/codegen/arc_emitter/`
- The `drop_fn` parameter on `ori_rc_dec` serves as the recursive-reclamation
  hook and must continue to be called synchronously when the refcount reaches zero
  (Ori's `Drop` ordering guarantee)
- The `single-threaded` feature flag is the mechanism for runtime variant selection
- `MAX_REFCOUNT` is the immortal sentinel and must be preserved across implementations

---

## 10.5 Code Changes (Later)

These are implementation items for after the full literature review is complete.
None are on the critical path for Stage 1-2.

### `compiler/ori_rt/src/rc/mod.rs`

**C1. Add runtime-boundary documentation to the module doc.** Implement E4 above.
The doc comment at the top of `mod.rs` currently lists functions by category. Add
a "Stability and Compatibility" section documenting:
```rust
//! ## Stability and Compatibility
//!
//! The following functions constitute the RC API boundary between
//! compiler-generated code and the runtime. Their signatures are consumed
//! by `ori_llvm/src/codegen/arc_emitter/` and must not change without
//! updating all LLVM call sites:
//!
//! - `ori_rc_inc(data_ptr)` — increment (Relaxed ordering)
//! - `ori_rc_dec(data_ptr, drop_fn)` — decrement + synchronous drop (Release/Acquire)
//! - `ori_rc_is_unique(data_ptr)` — COW uniqueness check (Relaxed)
//! - `ori_rc_is_unique_or_null(data_ptr)` — COW with sentinel handling
//!
//! The `drop_fn` on `ori_rc_dec` is called synchronously when refcount
//! reaches zero. This is a semantic guarantee (Ori's `Drop` is ordered
//! and deterministic). Future concurrent RC implementations must preserve
//! this property or document the semantic change.
//!
//! The `MAX_REFCOUNT` sentinel marks immortal objects. All RC function
//! implementations must check for this value and skip (no-op).
//!
//! The `single-threaded` feature selects non-atomic implementations.
//! Future concurrent RC variants should use additional feature flags,
//! not replace the default path.
```

**C2. No vtable or function-pointer indirection needed.** The question was raised
whether `ori_rt`'s RC functions should be behind a vtable for swappability. The
answer is no. The functions are already behind an extern "C" call boundary. Swapping
implementations is done at link time (different `ori_rt` builds for different
concurrency models) or at compile time (feature flags). A vtable would add an
indirect call on every RC operation -- an unacceptable overhead for what is already
one of the hottest paths in the runtime. Feature flags provide zero-cost selection
at compile time.

### `compiler/ori_arc/src/ir/repr.rs`

**C3. Add a doc comment to `RcStrategy` noting concurrent-RC compatibility.**
The enum is already correctly shaped (describes value layout, not RC mechanism).
Add a note making this explicit:
```rust
/// Describes the *shape* of an RC'd value for the LLVM emitter.
///
/// This enum tells the emitter how to extract the data pointer and which
/// drop function to use. It says nothing about the RC *mechanism* (atomic
/// vs non-atomic, immediate vs deferred). This separation is intentional:
/// a concurrent RC backend would use the same `RcStrategy` values.
```

### `compiler/ori_arc/src/aims/emit_reuse/`

**C4. Add AIMS-independent RC abstraction trait for Stage 5 module refactor.**
When a third RC variant is needed (O3), the inline `#[cfg]` blocks should be
replaced with a module-based dispatch. The trait signature would be:
```rust
/// RC operations trait. Selected at compile time via feature flag.
/// All implementations must preserve:
/// - Null pointer is no-op (all functions)
/// - MAX_REFCOUNT is no-op (immortal sentinel)
/// - drop_fn called synchronously when refcount reaches zero
/// - Release/Acquire ordering on dec/drop boundary
pub trait RcOps {
    fn inc(data_ptr: *mut u8);
    fn dec(data_ptr: *mut u8, drop_fn: Option<extern "C" fn(*mut u8)>);
    fn is_unique(data_ptr: *const u8) -> bool;
    fn is_unique_or_null(data_ptr: *const u8) -> bool;
}
```
Implementations: `rc/atomic.rs` (current default), `rc/non_atomic.rs` (current
`single-threaded`), `rc/biased.rs` (Stage 5a), `rc/circ.rs` (Stage 5c). The
extern "C" functions (`ori_rc_inc`, etc.) would delegate to the selected
implementation via monomorphization (type alias, not dynamic dispatch). This is
a Stage 5 refactoring task -- the trait definition is recorded here for future
reference, not for current implementation.

**C5. Document the "sole counted ownership" invariant on reuse tokens.** In
`detect.rs` and `mod.rs`, the reuse eligibility check requires
`Uniqueness::Unique`. Add a comment noting that in a future concurrent-RC world,
`Unique` means "sole counted owner" (no other `Rc` exists), which is sufficient
for reuse because uncounted `Snapshot` references are read-only and protected by
EBR (they cannot observe the reuse). This is a documentation-only change that
prevents future confusion.

#### Fix along the way (when touching emit_reuse/)

- [ ] **[NOTE]** `aims/emit_reuse/mod.rs:508` — 508 lines, marginally over the 500-line limit. Proactive split recommended at ~450 lines when next adding Stage 2+ features (e.g., cross-type size-class matching). Extract `apply_static_reuse_same_block`/`apply_static_reuse_cross_block` and helpers into `static_reuse.rs`.

**C6. Add `ori_rc_mark_shared(data_ptr: *mut u8)` as a future API stub.** When
Lean 4-style per-object mode bits are adopted (Stage 5a), channel send sites need
to call a function that flips the object from single-threaded to multi-threaded
mode. Define the stub signature now (as a no-op behind a feature flag) so that
the LLVM emitter's channel-send codegen can be designed with this call site in
mind. The function signature: `extern "C" fn ori_rc_mark_shared(data_ptr: *mut u8)`.
Current behavior: no-op (everything is already atomic by default). Under
`single-threaded` + `per-object-mode`: negates `m_rc` to flip to atomic mode.
This is a Stage 5 implementation item -- recorded here for interface planning.

---

## 10.6 Lens Shift

This paper changes how we read Paper 11 (Cyclic RC) in two ways:

**L1. Cycle detection must be compatible with the immediate-decrement invariant.**
CIRC explicitly applies decrements immediately (no deferral). Paper 11 (Parkinson
et al., "RC Deeply Immutable Cycles," ISMM 2024 -- same Parkinson as CIRC co-author)
addresses cyclic structures by lifting RC to the SCC level for frozen/immutable
graphs. When reading Paper 11, the key question becomes: does its cycle-handling
strategy require deferred decrements, or is it compatible with immediate decrements?
If it requires deferral, it conflicts with both CIRC's design and Ori's synchronous
`Drop` guarantee. If it works with immediate decrements (likely, given the shared
authorship), it composes cleanly with CIRC and with AIMS's current RC emission.

**L2. The "counted vs uncounted" split reframes cycle detection.** In CIRC, only
counted references (`Rc`) participate in reference counting. Uncounted `Snapshot`
references are invisible to the reference count. For cycle detection, this means
cycles can only form through counted references. If Paper 11's cycle detection
operates on counted references only, the uncounted-reference mechanism is orthogonal.
When reading Paper 11, check whether its SCC-based frozen-cycle approach operates
at the RC level (compatible with CIRC's counted/uncounted split) or at the pointer
level (would need to account for uncounted references).

**L3. `pop_edges()` is the shared primitive between CIRC and Paper 11.** Both
papers need to enumerate outgoing references from a reclaimed object: CIRC for
recursive reclamation, Paper 11 for SCC identification. In Ori, the compiler-
generated drop function serves this role. When reading Paper 11, look for whether
it requires a richer edge-enumeration API (e.g., distinguishing "strong" from "weak"
edges, or enumerating edges without triggering destruction). If so, Ori's drop
functions may need to be augmented with a separate edge-enumeration entry point.
This would be a `ori_rt` API addition, not an AIMS analysis change.

**L4. Read Paper 12 (Double-Ended Bit-Stealing) with tag-bit awareness.** CIRC
stores epoch timestamps in pointer tag bits. Paper 12 (Elsman, ICFP 2024) uses
both low and high pointer bits for ADT representation. If Ori later adopts both
CIRC-style concurrent RC and bit-stealing representation optimization, the tag-bit
budgets must be coordinated. AIMS's `ShapeClass::ReusableCtor` and `Locality` facts
would need to inform which bits are available for each purpose. This is not an
immediate concern but should be noted when reading Paper 12.

---

## 10.7 Open Risk

**O1. AIMS assumes synchronous drop execution after `RcDec`.** The reuse emission
pass (`compiler/ori_arc/src/aims/emit_reuse/`) emits `Reset`/`Reuse` instructions
that assume the allocation from a `RcDec`'d variable is available for reuse in the
same basic block. If Ori later adopts deferred reclamation (even CIRC's
deferred-free-only variant, where `free()` is delayed to epoch boundaries), the
allocation may not actually be freed when the reuse instruction executes. The
`Reset` instruction specifically resets an allocation's fields for reuse, which
requires the allocation to be both unreachable to other threads and not yet freed.
Under CIRC, this would require the `Reset` to happen *before* the EBR-deferred
`free()`, which means CIRC would need a special path for reused allocations that
skips the deferred-free queue entirely.

**Mitigation:** This is sound today because Ori is effectively single-threaded for
heap access. When shared-heap concurrency is designed, the reuse-emission invariant
("unique counted owner implies safe to reuse") must be re-validated against the
concurrent RC model. The AIMS lattice's `Uniqueness::Unique` would need to imply
not just "sole counted owner" but "sole owner of any kind, counted or uncounted."
This may require the EBR guard to be released before reuse can proceed, which
would be a scheduling constraint on the LLVM emitter, not on AIMS analysis.

**O2. `ori_rc_is_unique` has a CIRC-incompatible semantic under concurrent access.**
The function checks `refcount == 1` with `Relaxed` ordering. Its documentation
argues that a stale read of RC=1 when the true value is 2 is impossible because
"the incrementing thread must have cloned from an existing reference." This
argument holds for counted references but breaks for CIRC's uncounted `Snapshot`
references: a thread could hold a `Snapshot` (no refcount increment) while
another thread sees refcount == 1 and concludes unique ownership. The `Snapshot`
holder could then read stale data if the "unique" owner mutates in place.

**Mitigation:** Under CIRC's design, `Snapshot` holders are explicitly read-only
(no mutation through uncounted references). COW mutation by the counted owner is
safe because snapshot holders will see the pre-mutation version through their
EBR-protected view, and the EBR mechanism ensures the old version is not freed
until all snapshots are released. However, this relies on CIRC's constraint that
`Snapshot` never provides mutable access. If Ori's future concurrency model allows
mutable shared access (unlikely given the `Sendable` design, but worth documenting),
this assumption breaks. **Document in `ori_rc_is_unique`'s doc comment that the
uniqueness check is about counted references only, and that future concurrent RC
must preserve the read-only invariant on uncounted references.**

**O3. The `single-threaded` feature flag covers a binary choice, not a spectrum.**
The current design has two modes: fully atomic (default) and fully non-atomic
(`single-threaded`). CIRC suggests a third mode: "mostly non-atomic with
epoch-protected critical sections." A biased-RC scheme (thread-local fast path,
atomic slow path for cross-thread transfers) would be yet another mode. The
feature-flag mechanism supports this (add `circ`, `biased-rc` features), but the
`ori_rt` code structure -- with `#[cfg(not(feature = "single-threaded"))]` blocks
inline in each function -- would become unwieldy with three or more variants.

**Mitigation:** When a third RC variant is needed, refactor `ori_rt/src/rc/` to
use a trait-based or module-based dispatch rather than inline `#[cfg]` blocks.
Define the RC operations as a trait (`RcOps`) with implementations in separate
modules (`rc/atomic.rs`, `rc/non_atomic.rs`, `rc/circ.rs`), selected at compile
time via feature-flag-driven type alias. This refactor should happen at the
beginning of Stage 5, not now. The current two-variant inline approach is
manageable and avoids premature abstraction.

**O4. AIMS's `MemoryContract` does not distinguish thread-local from cross-thread
parameter ownership.** When Ori adds concurrency, a function parameter might be
received from another thread (requiring atomic RC) or from the same thread (safe
for non-atomic fast path). CIRC's design exploits this distinction via
counted-vs-uncounted references. AIMS's `ParamContract` has `access: AccessClass`
(Owned/Borrowed) and `consumption: Consumption` (Linear/Unrestricted/Dead) but no
"provenance: ThreadLocal/CrossThread" dimension. Adding this would be a Stage 5
concern (new `Locality` refinement or new `ParamContract` field), not a current
AIMS change.

**Mitigation:** Document in Section 07.4 that `ParamContract` may need a
thread-provenance field when concurrent RC is designed. This is informational;
no current code change.

**O5. Drop-function panics interact with concurrent reclamation.** `ori_rt`'s
`call_drop_fn` wraps drop function calls in `catch_unwind` and aborts on panic
(because `ori_rc_dec` is `nounwind` in LLVM IR). Under CIRC's recursive
reclamation, a panic in a drop function would abort the process during a
potentially deep chain of recursive decrements. This is already the correct
behavior (panic during drop is unrecoverable in Ori's model), but it should be
documented as an explicit design choice: recursive reclamation chains are
all-or-nothing. A panic at any point in the chain aborts the process.

**Mitigation:** Already handled by `call_drop_fn`'s `catch_unwind` + `abort`.
No code change needed. Add a note to the drop-function documentation explaining
that recursive reclamation assumes drop functions do not panic.
