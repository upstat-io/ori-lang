# Negative Effects Research: Expressing that a Context FORBIDS Operations

The single biggest unresolved design question for deep safety. Ori can express "this function REQUIRES capability X" (`uses X`). For kernel safety, we need "this context FORBIDS capability Y."

Concrete kernel examples:
- Interrupt handlers cannot allocate memory (`no Allocator`)
- RCU read-side critical sections cannot sleep (`no Suspend`)
- Per-CPU access requires preemption disabled (`no Preemption`)
- Hardirq context cannot acquire sleeping locks (`no SleepingLock`)

This document surveys eight approaches from across PL research.

---

## 1. Pony Deny Capabilities

### The Model

Pony's reference capabilities are defined by what they DENY, not what they allow. Each of the six capabilities (`iso`, `trn`, `val`, `ref`, `box`, `tag`) is formally specified by what operations it forbids to other aliases, both locally (within the same actor) and globally (across actors).

**The Deny Matrix:**

| Capability | Deny Local Read | Deny Local Write | Deny Global Read | Deny Global Write |
|------------|----------------|-----------------|-----------------|------------------|
| `iso`      | Yes            | Yes             | Yes             | Yes              |
| `trn`      | No             | Yes             | Yes             | Yes              |
| `ref`      | No             | No              | Yes             | Yes              |
| `val`      | No             | No              | No              | Yes              |
| `box`      | No             | Yes             | No              | No               |
| `tag`      | No             | No              | No              | No               |

The key insight from the "Deny Capabilities for Safe, Fast Actors" paper (Clebsch et al.): "what a subject is *not able* to do (denied)" defines the capability, inverting the traditional model that lists permissions. Each capability is a pair of (local deny, global deny) properties. Global properties are always at least as restrictive as local properties.

### Soundness

The system provides data-race freedom as a consequence of deny properties. When a reference's local and global deny properties are identical, the reference can safely be sent to another actor. This is proven sound through an operational semantics where actors see themselves as `ref` internally, guaranteeing no other actor can observe their fields.

The capabilities are transitive -- write capability on an object implies write access to all reachable objects through that reference.

### Adaptation to Effect Denial

Pony's deny model is about *aliasing* denial, not *effect* denial. However, the structural insight transfers directly:

Instead of: "iso denies global read/write aliases"
We could have: "InterruptCtx denies Allocator/Suspend effects"

The matrix structure maps naturally:

| Context          | Deny Allocator | Deny Suspend | Deny SleepingLock | Deny Preemption |
|------------------|----------------|--------------|-------------------|-----------------|
| `InterruptCtx`   | Yes            | Yes          | Yes               | Yes             |
| `RcuReadSide`    | No             | Yes          | Yes               | No              |
| `AtomicCtx`      | No             | Yes          | Yes               | No              |
| `PreemptDisabled` | No            | No           | Yes               | No              |
| `NormalCtx`      | No             | No           | No                | No              |

### Feasibility for Ori

**Strengths:** The deny matrix is elegant, finite, and statically checkable. It maps well to kernel context hierarchies. The transitive deny property (if a context denies X, all callees inherit the denial) matches Ori's capability propagation.

**Weaknesses:** Pony's model is about aliasing a single object -- it's a 6-element lattice. Kernel contexts create a potentially open-ended deny set. The model needs adaptation from a fixed set of deny properties to a composable set.

**Implementation complexity:** Low-medium. The deny matrix is essentially a compile-time lookup table. The main complexity is in propagation and checking.

**Mapping to Ori:** A context could carry both positive capabilities (`uses X`) and negative capabilities (`denies Y`). The deny set propagates downward through calls. A function requiring `Allocator` called in a context that `denies Allocator` is a compile error.

---

## 2. Row-Polymorphic Effect Exclusion

### The Landscape

There are two fundamentally different approaches to expressing effect absence in row-polymorphic effect systems:

#### 2a. Koka's Approach: Closed Rows (No Direct Absence)

Koka uses row-polymorphic effect types where effects are tracked as labeled rows. An effect row is either:
- **Closed**: `<exn, div>` -- exactly these effects, nothing else
- **Open**: `<exn, div | e>` -- these effects plus whatever `e` represents

Absence in Koka can only be enforced by listing a closed row of allowed effects. To say "no io", you must list everything that IS allowed. As Leijen explicitly notes: "with flags one can state specifically that a certain effect must be absent, while [Koka] can only enforce absence of an effect by explicitly listing a closed row of the allowed effects which is less modular."

**Handler masking:** When a handler handles effect `E`, it removes `E` from the effect row of the handled computation. The `mask` function can hide an effect from the row. But this is effect *discharge*, not effect *prohibition* -- it says "I handled this effect" rather than "this effect is forbidden."

The fundamental limitation: Koka cannot say "this function may do anything EXCEPT io" without listing every other possible effect. This is inherently non-modular -- adding a new effect to the system requires updating all such closed rows.

#### 2b. Links/Lindley-Cheney Approach: Presence/Absence Flags

The Links programming language (Lindley and Cheney, "Row-based Effect Types for Database Integration") uses a different row structure where each effect label carries a **presence flag** (Present or Absent):

```
(exn: Present, io: Absent | rho)
```

This can express "must not have io" modularly. The typing rule for database queries in Links enforces that the `wild` (io) effect must be flagged Absent, ensuring database queries never perform side effects.

**Key typing rule:** A handler that removes effect F sets its presence flag from Present to Absent in the output row. A function type can require Absent flags on specific effects.

**Cost:** Requires a kind system that tracks for each type variable which labels cannot be present. This adds complexity to type inference and the constraint solver.

### Adaptation for Ori

The presence/absence flag approach maps directly to negative effects:

```ori
// Hypothetical syntax
@irq_handler () -> void uses InterruptCtx denies Allocator, Suspend = ...
```

In row terms: `InterruptCtx: Present, Allocator: Absent, Suspend: Absent | rho`

**Implementation complexity:** Medium-high. Requires extending Ori's capability inference to track both presence and absence. The constraint solver must handle absence constraints during unification.

**Key question:** Ori's capabilities are not currently row-polymorphic. Adding row polymorphism to the capability system is a significant architectural change. But the *concept* of presence/absence flags could be adapted without full row polymorphism -- see Section 5 (effect exclusion via Boolean algebra).

---

## 3. Scala 3 Capture Checking

### The Model

Scala 3's experimental capture checking tracks which capabilities a value retains (captures). Types are annotated with capture sets: `T^{c1, c2}` means "value of type T that captures capabilities c1 and c2."

**Key mechanisms:**

- **Universal capability:** `cap` represents "any capability." `T^` abbreviates `T^{cap}`.
- **Pure functions:** `A -> B` captures nothing. `A => B` = `A ->{cap} B` captures everything.
- **Subcapturing:** `C1 <: C2` holds when C2 accounts for every element in C1.
- **Escape checking:** Prevents capabilities from outliving their scope.

**Restriction through bounding:**
```scala
def runSecure[C^ >: {trusted} <: {trusted}](block: () ->{C} Unit): Unit
```
This bounds the capture set to exactly `{trusted}`, meaning only capabilities derived from `trusted` are permitted. This is a form of *positive restriction* -- "only these capabilities" rather than "not these capabilities."

**`@constructorOnly`:** An annotation that prevents a capability parameter from being retained as a field. This is the closest to a negative constraint: "this value must NOT capture the parameter."

### What Scala 3 Cannot Do

Scala 3 capture checking has **no general mechanism for negative constraints**. You cannot say "this function must not capture capability X." You can:
- Bound capture sets to specific capabilities (positive restriction)
- Prevent escape through scoping rules
- Use `@constructorOnly` for constructor-specific non-retention

But you cannot write: "any capabilities except FileSystem."

### Adaptation for Ori

Scala 3's approach is essentially **positive restriction through bounding**, not negative restriction. For Ori's kernel use case:

Instead of "denies Allocator", Scala would express: "allowed capabilities are exactly {InterruptCtx, VolatileIO, InlineAsm}" -- a closed positive set.

This is equivalent but less ergonomic for the kernel case, where you typically want to say "everything is allowed except these dangerous things." It has the same modularity problem as Koka's closed rows.

**Implementation complexity:** Medium. Capture set tracking is well-understood. The restriction to positive bounds simplifies inference.

**Verdict:** Insufficient for the kernel problem. Positive-only bounds force listing all allowed capabilities, which is fragile and non-modular.

---

## 4. Typestate Systems for Context Restrictions

### The Model

Typestate-oriented programming (Strom and Yemini, 1986; Aldrich et al., Plaid language) encodes state in the type, restricting which operations are available in each state. A `File<Reading>` has `read()` but no `write()`; a `File<Closed>` has neither.

**Core mechanism:** State-dependent method availability through phantom type parameters and state-specific `impl` blocks. Transitions consume the old state and produce a new state (ownership transfer).

**Plaid language (CMU, Aldrich):** Pioneered typestate-oriented programming where:
- Each typestate has its own interface, representation, and behavior
- Typestate transitions change the type at compile time
- Access permissions (`unique`, `immutable`, `shared`, `none`) enforce aliasing constraints
- The permission system ensures that only the holder of the appropriate permission can trigger a state transition

**Session types:** Related formalism for communication protocols. A session type describes what operations are valid at each point in a protocol. Applied to APIs rather than communication channels, this becomes typestate.

### Application to Kernel Contexts

Kernel contexts map naturally to typestates:

```
InterruptCtx  --[return from IRQ]-->  NormalCtx
NormalCtx     --[rcu_read_lock()]--> RcuReadSide
RcuReadSide   --[rcu_read_unlock()]--> NormalCtx
NormalCtx     --[preempt_disable()]--> PreemptDisabled
```

A function operating in `InterruptCtx` typestate simply does not have `allocate()` in its interface. This is prohibition-by-absence rather than prohibition-by-annotation.

**Rust's typestate pattern:** Demonstrates this approach using phantom types:
```rust
struct Context<S: ContextState> { _state: PhantomData<S> }
impl Context<InterruptCtx> {
    fn volatile_read(&self, addr: PhysAddr) -> u32 { ... }
    // No allocate() method -- not available in this state
}
impl Context<NormalCtx> {
    fn allocate(&self, size: usize) -> *mut u8 { ... }
}
```

### Strengths and Weaknesses

**Strengths:**
- Prohibition is structural, not annotated -- you cannot call what does not exist
- Compile-time enforcement with zero runtime cost
- Well-understood formal foundations (Aldrich's TOPLAS 2014 paper proves soundness)
- Natural fit for state machines (lock acquire/release, context entry/exit)

**Weaknesses:**
- Requires explicit state threading -- every function must take and return the context
- Poor composability -- combining two typestate protocols is complex
- Cannot express "any context except InterruptCtx" polymorphically
- Function signatures become cluttered with state parameters
- Does not compose with Ori's effect/capability system without significant design work

### Adaptation for Ori

Typestate could model context transitions (entering/exiting interrupt handlers, acquiring/releasing locks). But as the sole mechanism for negative effects, it is insufficient:

1. It handles *state-specific method availability* but not *effect prohibition*
2. A function deep in the call chain needs the context threaded through every caller
3. It cannot express the polymorphic pattern "works in any context that doesn't allocate"

**Verdict:** Useful as a complementary mechanism for lock/context protocols, but insufficient as the primary negative-effect system.

---

## 5. Effect Exclusion via Boolean Algebra (The "With or Without You" Approach)

### The Model

**This is the most directly relevant prior work.** Lutze, Madsen, Schuster, and Brachthaeuser (ICFP 2023) present a type and effect system with **union, intersection, and complement** effects, implemented in the Flix programming language.

**Core mechanism:** Effects form a Boolean algebra with three operations:
- **Union:** `ef1 + ef2` -- has both effects
- **Intersection:** `ef1 & ef2` -- has only effects in both
- **Complement:** `~ef` -- has any effect EXCEPT ef
- **Difference:** `ef1 - ef2` -- equivalent to `ef1 & ~ef2`

**Concrete syntax in Flix:**
```flix
// This function can have any effect EXCEPT Block
def onClick(listener: KeyEvent -> Unit \ (ef - Block)): Unit

// An exception handler that must not throw
def handle(h: ErrMsg -> a \ (ef - Throw)): a \ ef
```

The complement `~Block` means "any effect except Block." The difference `ef - Block` means "whatever effects ef has, minus Block." This is exactly the negative-effect mechanism kernel safety needs.

### Formal Foundations

The system is formalized as the **lambda-C calculus** (lambda-complement). Key results:

1. **Effect Safety Theorem:** "No excluded effect is ever performed." If a function has type `... \ (ef - Block)`, the Block effect will never execute. This is a *non-standard* soundness property beyond standard progress/preservation.

2. **Principal types:** The system preserves principal types modulo Boolean equivalence. This means type inference is complete -- Algorithm W extended with Boolean unification finds the most general type.

3. **Boolean unification:** Effect inference requires solving equations over Boolean algebras (sets with union, intersection, complement). Boolean unification is decidable but NP-hard in general. The Flix implementation uses the Boole library for Boolean unification. In practice, effect sets are small and inference is fast.

4. **Case study:** The authors identified 59 open-source code fragments that require effect exclusion for correctness and successfully recoded them using their system.

### How It Maps to Ori

This is the most natural fit for Ori's capability system. The mapping:

| Flix concept | Ori equivalent |
|-------------|---------------|
| Effect | Capability |
| `ef - Block` | `uses (ef - Allocator)` or `denies Allocator` |
| `~Block` | `denies Allocator` (complement of Allocator) |
| `ef1 + ef2` | `uses Ef1, Ef2` (already exists) |
| `handle` removes effect | `with X = impl in` discharges capability |

**Proposed Ori syntax options:**

```ori
// Option A: explicit denies clause
@irq_handler () -> void uses InterruptCtx denies Allocator, Suspend = ...

// Option B: set subtraction in uses clause
@irq_handler () -> void uses InterruptCtx, ~Allocator, ~Suspend = ...

// Option C: without keyword (matching Flix)
@irq_handler () -> void uses InterruptCtx without Allocator, Suspend = ...
```

### Implementation Complexity

**Medium.** The core algorithm is Boolean unification on effect sets. The Flix implementation demonstrates feasibility. Key costs:

1. Extend capability representation from sets to Boolean expressions (sets with complement)
2. Extend capability inference/checking to handle complement constraints
3. Boolean unification solver (can use existing libraries; sets are typically small)
4. Error messages for denial violations ("E1250: function requires Allocator, but current context denies it")

### Proven Sound?

**Yes.** Progress, preservation, AND the effect safety theorem are formally proven. This is the strongest theoretical guarantee of any approach surveyed.

### Does It Solve the Kernel Problem?

**Yes, directly.** The Boolean complement/difference mechanism expresses exactly what kernel safety needs:

- Interrupt handler: `uses InterruptCtx without Allocator, Suspend, SleepingLock`
- RCU read-side: `uses RcuReadSide without Suspend, SleepingLock`
- Preempt-disabled: `uses PreemptDisabled without SleepingLock`
- Normal context: no restrictions (no `without` clause)

A function that `uses Allocator` called from an `InterruptCtx without Allocator` context is a type error. The compiler statically prevents the violation.

---

## 6. Capability-Secure Languages and Authority Restriction

### Object-Capability Model (Mark Miller)

Miller's E language and thesis ("Robust Composition," 2006) define capabilities through the object-capability model: authority is the ability to cause effects, and a capability is an unforgeable reference that confers specific authority.

**Key mechanisms for restriction:**

1. **Attenuation:** Creating a new capability that is strictly less powerful than the original. A read-only file handle is an attenuation of a read-write handle.

2. **Caretaker pattern:** A pair of objects -- a forwarding facet and a revoking facet -- that share a mutable reference. The forwarding facet delegates to the capability; the revoking facet can set the reference to null, permanently disabling access.

3. **Membrane pattern:** Transitive attenuation. Every capability flowing through a membrane is wrapped in an attenuating proxy. Used for revocation and read-only enforcement over entire object graphs.

4. **Facets:** Multiple views of the same underlying capability with different authority levels.

**Critical insight:** Authority in the ocap model is ALWAYS narrowed, never amplified. A module can only use capabilities it was explicitly given. This is the "principle of least authority" (POLA).

### Wyvern Language (CMU, Aldrich et al.)

Wyvern formalizes authority control through its module system (ECOOP 2017):

- Modules are first-class, statically typed capabilities
- The `bind` construct restricts what a module can access -- only explicitly imported capabilities
- Effect system tracks HOW resources are used (not just which ones are accessed)
- Authority defined non-transitively: wrappers can provide attenuated versions
- Proven capability-safe and authority-safe

### Adaptation for Ori

The ocap model's authority restriction is fundamentally **positive** -- you restrict by providing a narrower capability, not by annotating what is forbidden. This maps to:

```ori
// Instead of: @f () -> void denies Allocator
// Provide a restricted context:
with Allocator = PanicOnAlloc {} in
    irq_handler()
```

Where `PanicOnAlloc` is a dummy implementation that panics if called. The restriction is enforced by the handler, not the type system.

**Weakness:** This is runtime enforcement (the panic happens at runtime), not compile-time prohibition. For kernel safety, compile-time enforcement is essential.

**Wyvern's approach** is closer: if the module system restricts what capabilities are *available*, then a function in a restricted module simply cannot reference `Allocator`. But this requires capability propagation through modules, not just functions.

### Verdict

The ocap model provides the philosophical foundation ("authority is always narrowed") but not the compile-time enforcement mechanism. The positive-restriction approach (provide only what is needed) is sound but less ergonomic than negative restriction (forbid what is dangerous) for the kernel use case.

However, the insight that authority narrowing is the fundamental operation is valuable. A `without` clause is semantically equivalent to narrowing the available capabilities -- it is attenuation expressed as a type constraint.

---

## 7. Linux Kernel's Actual Enforcement Tools

### Sparse: Context Counting

Sparse (Linus Torvalds / Josh Triplett) performs static analysis on the Linux kernel using annotations. For lock/context checking:

**`__attribute__((context(expression, in_context, out_context)))` :**
- `in_context`: required context count on function entry
- `out_context`: context count on function exit
- Each `spinlock` counts as +1, each `spinunlock` as -1

**Derived macros:**
- `__must_hold(x)` = `context(x, 1, 1)` -- lock held on entry and exit
- `__acquires(x)` = `context(x, 0, 1)` -- lock NOT held on entry, held on exit
- `__releases(x)` = `context(x, 1, 0)` -- lock held on entry, NOT held on exit

Sparse checks that entry/exit contexts match and no path has conflicting contexts. This is essentially **integer-valued typestate** -- the preemption count is an integer that must satisfy bounds at each call site.

**Limitation:** Sparse does no real data flow analysis. It cannot handle conditional locking (`spin_trylock` with a success check) and produces false positives with complex control flow.

### Klint: Compile-Time Preemption Count Tracking

Klint (Gary Guo, 2022-2026) is a custom `rustc` driver that performs compile-time atomic context violation detection for Rust kernel code.

**Model:** Each function is assigned:
- **adjustment:** how the preemption count changes after calling this function
- **expected range:** what preemption count values are permitted when calling this function

For example:
```rust
#[klint::preempt_count(adjust = 1, expect = 0..)]
pub fn rcu_read_lock() -> RcuReadGuard

// mutex_lock: adjustment 0, expects preemption count == 0
// spin_lock: adjustment +1, expects any
// schedule: adjustment 0, expects preemption count == 0
```

The critical rule: `schedule()` (and anything that can sleep) expects preemption count == 0. `rcu_read_lock()` increments preemption count to 1. Therefore, calling `schedule()` inside an RCU read-side critical section is flagged as an error: the preemption count is 1, but `schedule` expects 0.

**Inference:** Klint propagates preemption count ranges through call chains. Explicit annotation is often unnecessary due to inference.

**Limitations:**
- Cannot handle conditional locking (`spin_trylock` returning `Option`)
- Struggles with compiler-injected drop code
- Cannot automatically derive FFI function annotations
- No formal model beyond interval arithmetic on integers

### lockdep: Runtime Lock Ordering

lockdep validates lock ordering at runtime, detecting potential deadlocks. It tracks which locks are held at each point and validates that acquisition order is consistent. Not a compile-time tool.

### How These Map to Type-Level Enforcement

The kernel tools use **integer tracking** (preemption count) with **range constraints** (expected values). This maps to Ori as:

| Kernel tool | Ori type-level equivalent |
|------------|--------------------------|
| `preempt_count >= 1` | Context carries denied capabilities |
| `__must_hold(lock)` | Typestate: `Context<LockHeld<L>>` |
| `__acquires(lock)` | Function returns `Context<LockHeld<L>>` |
| `schedule() expects count==0` | `schedule` uses `Suspend`, denied in atomic contexts |
| Klint range checking | Capability denial propagation through call chain |

The integer model is **more precise** than Boolean effects for certain patterns (e.g., nested RCU read locks increment to 2), but **less composable** and requires manual annotation at FFI boundaries.

---

## 8. Theoretical Analysis: Can Negative Effects Exist in Algebraic Effect Systems?

### The Core Question

Standard algebraic effect systems (Plotkin & Power, 2003; Plotkin & Pretnar, 2009) model effects as algebraic operations with handlers. A handler for effect E removes E from the effect row:

```
handle e with { op -> k -> ... } : tau ! (rho \ E)
```

This is effect *masking/discharge* -- the handler provides an interpretation for E and removes it from the type. But this is fundamentally different from effect *prohibition*.

**Effect masking says:** "I am handling E here, so callees need not worry about it."
**Effect prohibition says:** "E must never occur, even transitively."

### The Difference Between Absence and Prohibition

| Property | Absence | Prohibition |
|----------|---------|-------------|
| Expression | `total` or closed row without E | `denies E` or `~E` in row |
| Handler interaction | A handler CAN discharge E | No handler CAN introduce E |
| Polymorphism | Fixed -- lists what IS present | Open -- says what is NOT present |
| Violation | Runtime (unhandled effect) | Compile-time type error |
| Modularity | Poor (must list all present effects) | Good (only names the forbidden ones) |

### Three Approaches to Formalization

**Approach 1: Closed rows (Koka, standard algebraic effects)**
Express absence by listing a closed row of present effects. Cannot express "everything except E" modularly. Koka's `mask` function can hide effects but not prohibit them.

**Approach 2: Presence/absence flags (Links, Lindley-Cheney)**
Each effect label carries a flag: `Present` or `Absent`. Can express `{io: Absent | rho}` meaning "no io, regardless of what else is present." Requires a kind system to track which labels have which flags. Used in Links for database query safety.

**Approach 3: Boolean algebra (Flix, Lutze et al. ICFP 2023)**
Effects form a Boolean algebra with union, intersection, complement. Effect exclusion is `ef - E` (set difference) or `~E` (complement). Complete type inference via Boolean unification. Proven sound with an effect safety theorem.

### Which Approach Is Sound?

All three are sound in their respective formalizations:
- **Closed rows:** Standard progress/preservation (Koka, Leijen 2014)
- **Presence/absence flags:** Standard progress/preservation (Links, Lindley-Cheney 2012)
- **Boolean algebra:** Progress, preservation, AND effect safety theorem (Lutze et al. 2023)

The Boolean algebra approach has the **strongest** guarantee because the effect safety theorem explicitly proves that excluded effects never execute, beyond standard type safety.

### Handler Interaction with Negative Effects

A key subtlety: what happens when a handler tries to introduce a forbidden effect?

```ori
// Context denies Allocator
with Allocator = SomeImpl in
    // Does the denial still hold inside the with...in?
    do_something()
```

There are two possible semantics:
1. **Handler overrides denial:** The `with` introduces the capability, overriding the denial. The denial is scoped.
2. **Denial cannot be overridden:** Attempting to provide a denied capability is a compile error.

For kernel safety, **option 2 is required.** The whole point is that interrupt handlers CANNOT allocate, period. A `with Allocator = ...` inside an interrupt handler must be a compile error. The denial is not a default that can be overridden -- it is an invariant that must hold.

This means denial is **more powerful than capability absence.** A capability that is merely absent can be provided via `with...in`. A capability that is *denied* cannot be provided at all within the denial scope.

---

## Synthesis: Recommendation for Ori

### The Design Space

| Approach | Can express "not X"? | Polymorphic? | Sound proof? | Composable? | Impl. complexity |
|----------|---------------------|-------------|-------------|------------|-----------------|
| Pony deny | Yes (matrix) | No (fixed 6) | Yes | Limited | Low |
| Koka closed rows | Indirectly | No | Yes | Poor | Low |
| Links flags | Yes | Yes | Yes | Medium | Medium-high |
| Scala 3 capture | Positive only | Limited | Partial | Medium | Medium |
| Typestate | By absence | No | Yes | Poor | Medium |
| Boolean algebra | **Yes (direct)** | **Yes** | **Yes (strongest)** | **Good** | Medium |
| Ocap attenuation | Positive only | N/A | Informal | Good | Low |
| Kernel tools | Integer ranges | No | No | Poor | Low |

### Recommended Approach: Boolean Effect Algebra with `without` / `denies`

The Boolean algebra approach from Lutze et al. (ICFP 2023, implemented in Flix) is the strongest candidate:

1. **Direct expressiveness:** `denies Allocator` maps to complement `~Allocator` in the Boolean algebra
2. **Polymorphic:** `uses (ef - Allocator)` works with effect variables
3. **Sound:** Proven with progress, preservation, AND the effect safety theorem
4. **Complete inference:** Algorithm W + Boolean unification gives principal types
5. **Composable:** Denial sets compose naturally via Boolean operations
6. **Practical:** Implemented and validated in Flix with 59 real-world examples

### Proposed Ori Design

#### Syntax

```ori
// Denial in function signature
@irq_handler () -> void
    uses InterruptCtx
    without Allocator, Suspend, SleepingLock = ...

// Denial inherited from context
@helper () -> void uses InterruptCtx without Allocator = {
    // Cannot call any function that uses Allocator
    volatile_write(addr: mmio_base, value: 0xFF)
}

// Polymorphic denial: "any context that doesn't allocate"
@process_fast<E> (data: [byte]) -> void
    uses E without Allocator = ...

// Context entry point establishes denial
@handle_interrupt (ctx: InterruptFrame) -> void
    uses InterruptCtx
    without Allocator, Suspend, SleepingLock = {
    // Everything called from here inherits the denial
    acknowledge_irq(ctx:)
    process_data(buffer:)
}
```

#### Semantics

1. `without X` in a function signature means: within this function and all transitive callees, capability X is forbidden
2. Calling a function that `uses X` from a context that has `without X` is a compile error
3. `with X = impl in expr` inside a `without X` context is a compile error (denial cannot be overridden)
4. Denial propagates downward through the call chain automatically
5. A function can add new denials but never remove them (monotonic restriction)

#### Typing Rules (Informal)

```
[DENY-INTRO]
If f declares "without E", then in the body of f,
the denied set includes E.

[DENY-PROP]
If the current denied set is D, calling g that "uses E"
where E in D, is a type error.

[DENY-INHERIT]
If the current denied set is D, any function called inherits D.
The callee's denied set is D union callee's own "without" set.

[DENY-NO-OVERRIDE]
"with E = impl in expr" where E is in the current denied set
is a type error.

[DENY-POLYMORPHIC]
If f has effect variable e "without E", then at any call site,
the instantiation of e must not include E.
```

#### Error Messages

```
error[E1250]: capability Allocator is denied in this context
  --> drivers/net/e1000/irq.ori:42:5
   |
42 |     let buffer = allocate(size: 1024);
   |                  ^^^^^^^^ requires Allocator capability
   |
note: Allocator is denied because this function is in interrupt context
  --> drivers/net/e1000/irq.ori:30:1
   |
30 | @handle_rx_irq () -> void
31 |     uses InterruptCtx
32 |     without Allocator, Suspend, SleepingLock = {
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ denial established here
   |
help: use pre-allocated buffers from the driver's buffer pool instead
```

### Complementary Mechanisms

The Boolean algebra approach can be supplemented with:

1. **Typestate for lock/context protocols:** `rcu_read_lock()` transitions context type, automatically adding denials. The denial set is derived from the context typestate, not manually annotated.

2. **Capset integration:** `capset InterruptDenials = Allocator, Suspend, SleepingLock` defines reusable denial sets.

3. **Context types that encode denials:**
```ori
// The type InterruptCtx automatically denies certain capabilities
// This is defined in the standard library, not user code
type InterruptCtx: Value = { frame: InterruptFrame }
capset InterruptCtx.denies = Allocator, Suspend, SleepingLock
```

### Implementation Roadmap

1. **Phase 1:** Parse `without` clause, represent denied set in function types
2. **Phase 2:** Check denial violations (calling `uses X` from `without X` context)
3. **Phase 3:** Propagate denials through call chains (simple set union)
4. **Phase 4:** Prevent `with X = impl in` from overriding denials
5. **Phase 5:** Boolean unification for polymorphic denial inference
6. **Phase 6:** Integrate with typestate for automatic denial derivation from context types

Phases 1-4 provide the core kernel safety mechanism. Phase 5 adds polymorphism. Phase 6 adds ergonomics.

---

## Sources

### Papers
- Clebsch et al., "Deny Capabilities for Safe, Fast Actors" (2015) -- Pony deny model
- Leijen, "Koka: Programming with Row Polymorphic Effect Types" (2014) -- Row-polymorphic effects
- Lindley & Cheney, "Row-based Effect Types for Database Integration" (2012) -- Presence/absence flags
- Lutze, Madsen, Schuster, Brachthaeuser, "With or Without You: Programming with Effect Exclusion" (ICFP 2023) -- Boolean effect algebra
- Aldrich et al., "Foundations of Typestate-Oriented Programming" (TOPLAS 2014) -- Plaid typestate
- Miller, "Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control" (2006) -- E language, ocap model
- Melicher et al., "A Capability-Based Module System for Authority Control" (ECOOP 2017) -- Wyvern modules
- Plotkin & Pretnar, "Handlers of Algebraic Effects" (2009) -- Algebraic effect handlers
- Bauer & Pretnar, "Programming with Algebraic Effects and Handlers" (2012) -- Eff language
- Brachthaeuser, Schuster, Ostermann, "Effects as Capabilities" (OOPSLA 2020) -- Effekt language
- Leroy, "Type and Effect Systems" (lecture notes) -- Formal effect system foundations
- Hillerstrom & Lindley, "Liberating Effects with Rows and Handlers" (2016) -- Links effect system

### Tools and Languages
- Pony: https://tutorial.ponylang.io/reference-capabilities/capability-matrix.html
- Flix effect system: https://doc.flix.dev/effects.html
- Flix effect polymorphism: https://doc.flix.dev/effect-polymorphism.html
- Koka: https://koka-lang.github.io/koka/doc/book.html
- Scala 3 capture checking: https://docs.scala-lang.org/scala3/reference/experimental/cc.html
- Scala 3 advanced CC: https://docs.scala-lang.org/scala3/reference/experimental/cc-advanced.html
- Effekt: https://effekt-lang.org/docs/concepts/effect-polymorphism
- Klint: https://github.com/Rust-for-Linux/klint
- Klint blog: https://www.memorysafety.org/blog/gary-guo-klint-rust-tools/
- Sparse annotations: https://sparse.docs.kernel.org/en/latest/annotations.html
- Rust typestate pattern: https://cliffle.com/blog/rust-typestate/
- Stanford CS 242 typestate: https://stanford-cs242.github.io/f19/lectures/08-2-typestate.html
- The Morning Paper on deny capabilities: https://blog.acolyer.org/2016/02/17/deny-capabilities/
- Klint atomic context article: https://lwn.net/Articles/951550/
- Wyvern: https://wyvernlang.github.io/
- Plaid: http://www.cs.cmu.edu/~aldrich/plaid/
