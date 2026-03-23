# Proposal: Negative Effects — The `without` Clause

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-22
**Affects:** Compiler (parser, IR, type checker, evaluator, LLVM), capabilities spec, grammar
**Related:** `unsafe-semantics-proposal.md` (approved), `capset-proposal.md` (approved), `capability-composition-proposal.md` (approved)
**Research:** `plans/deep-safety/negative-effects-research.md` (672 lines, 8 approaches surveyed)

---

## Summary

Extend Ori's capability system with a `without` clause that expresses **capability denial** — the ability to declare that a function and all its transitive callees are forbidden from using a specific capability. This is the compile-time mechanism for expressing context rules like "interrupt handlers cannot allocate memory" and "RCU read-side critical sections cannot sleep."

```ori
// Interrupt handler: cannot allocate, cannot sleep
@handle_irq (frame: InterruptFrame) -> void
    uses InterruptCtx
    without Allocator, Suspend = {
    acknowledge_irq(frame:)
    schedule_bottom_half()   // OK — doesn't use Allocator or Suspend
    // allocate(size: 1024)  // ERROR E1260: Allocator is denied
}

// Polymorphic: "works in any context that doesn't allocate"
@process_fast<E> (data: [byte]) -> int
    uses E without Allocator = {
    data.fold(initial: 0, op: (a, b) -> a + int(b))
}
```

---

## Problem Statement

### The Gap

Ori's current capability system can express "this function REQUIRES capability X" (`uses X`). It cannot express "this context FORBIDS capability Y." This is a critical gap for systems programming:

| Kernel Rule | Required Expression | Current Ori |
|------------|-------------------|-------------|
| Interrupt handlers cannot allocate | `without Allocator` | Cannot express |
| RCU read-side cannot sleep | `without Suspend` | Cannot express |
| Spinlock-held code cannot sleep | `without Suspend, SleepingLock` | Cannot express |
| Per-CPU access needs preemption disabled | `without Suspend` | Cannot express |

These rules are currently enforced in the Linux kernel by:
- **Klint** — compile-time preemption count tracking (Rust-for-Linux)
- **lockdep** — runtime lock ordering and context verification
- **sparse** — static annotation checking (`__must_hold`, `__acquires`)
- **`CONFIG_DEBUG_ATOMIC_SLEEP`** — runtime sleep-in-atomic detection

All of these are either external tools, runtime checks, or optional annotations. None are part of the language's type system. Ori can do better.

### Why Capability Absence Is Not Sufficient

A capability that is merely *absent* can be provided via `with...in`:

```ori
// Currently: if Allocator is not in scope, you can add it
with Allocator = KernelAllocator {} in {
    let buf = allocate(size: 1024)  // Works — Allocator was provided
}
```

For kernel safety, this is wrong. An interrupt handler that somehow acquires an Allocator capability and uses it will crash the kernel. We need *denial* — a capability that is explicitly forbidden and cannot be overridden.

### Prior Art

The Boolean effect algebra from Lutze, Madsen, Schuster, and Brachthäuser ("With or Without You: Programming with Effect Exclusion," ICFP 2023) provides the theoretical foundation. Key results:

1. **Effect Safety Theorem**: "No excluded effect is ever performed" — proven sound
2. **Principal types**: Algorithm W + Boolean unification preserves principal types
3. **Complete inference**: Boolean unification is decidable (NP-hard in general, fast for small sets)
4. **Validated**: 59 real-world code fragments successfully recoded

Implemented in the Flix programming language with syntax `ef - Block`.

See `plans/deep-safety/negative-effects-research.md` for the full 8-approach survey and analysis.

---

## Design

### Syntax

The `without` clause appears after the `uses` clause in function signatures:

```ebnf
function_sig    = "@" identifier [ type_params ] params "->" type
                  [ uses_clause ] [ without_clause ] [ where_clause ] .
uses_clause     = "uses" capability_list .
without_clause  = "without" capability_list .
capability_list = capability { "," capability } .
```

`without` is already a context-sensitive keyword in Ori (used in `use "m" { Trait without def }`). This proposal adds it as a context-sensitive keyword in function signature position.

### Examples

```ori
// Basic denial
@irq_handler () -> void
    uses InterruptCtx
    without Allocator, Suspend = ...

// Denial without specific uses (any function can deny capabilities)
@fast_path (data: [byte]) -> int
    without Allocator = ...

// Denial with capset
capset AtomicDenials = Allocator, Suspend, SleepingLock
@atomic_operation () -> void without AtomicDenials = ...

// Polymorphic denial: effect variable minus denied capabilities
@transform<E> (data: [byte], f: (byte) -> byte) -> [byte]
    uses E without Allocator = ...

// Denial in trait methods
trait InterruptHandler {
    @handle (self, frame: InterruptFrame) -> void
        uses InterruptCtx
        without Allocator, Suspend
}
```

### Semantic Rules

#### Rule 1: Denial Introduction

When a function declares `without X`, `X` is added to the **denied set** for the function body and all transitive callees.

```
[DENY-INTRO]
If f declares "without E₁, E₂, ..., Eₙ",
then denied(f.body) = { E₁, E₂, ..., Eₙ }.
```

#### Rule 2: Denial Propagation

Callees inherit the caller's denied set. The effective denied set at any point is the union of all denials in the call chain.

```
[DENY-PROP]
If caller has denied set D and calls callee,
then denied(callee) = D ∪ callee's own "without" set.
```

#### Rule 3: Denial Violation Check

Calling a function that `uses X` where `X` is in the current denied set is a compile error.

```
[DENY-CHECK]
If f uses capability E, and E ∈ denied(call_site),
then emit E1260: "capability E is denied in this context."
```

#### Rule 4: Denial Cannot Be Overridden

`with X = impl in expr` inside a `without X` context is a compile error. Denial is strictly monotonic — it can be added but never removed within a scope.

```
[DENY-NO-OVERRIDE]
If "with E = impl in expr" appears where E ∈ denied(current),
then emit E1261: "cannot provide capability E — it is denied in this context."
```

This is the critical distinction from capability *absence*. Absence is the default state that `with...in` can remedy. Denial is an invariant that `with...in` cannot break.

#### Rule 5: Polymorphic Denial

When a function has an effect variable `E` with `without X`, any instantiation of `E` at a call site must not include `X`.

```
[DENY-POLY]
If f<E> has "uses E without X", and call site instantiates E = {A, B, X},
then emit E1262: "effect variable E instantiated with denied capability X."
```

#### Rule 6: Denial and Marker Capabilities

Marker capabilities (`Unsafe`, `Suspend`) can be denied. `without Unsafe` means no unsafe operations are permitted — not even wrapped in `unsafe { }` blocks. This enables verified-safe modules.

```ori
// This module is provably free of unsafe operations
@verified_sort<T: Comparable> (list: [T]) -> [T]
    without Unsafe = ...
```

### Interaction with Existing Features

#### Interaction with `uses`

`uses` and `without` are independent. A function can both require and deny capabilities:

```ori
@irq_handler () -> void
    uses InterruptCtx, VolatileIO    // requires these
    without Allocator, Suspend       // forbids these
```

The denied set applies to the *body* and *callees*, not to the function's own declared capabilities. You can require `InterruptCtx` and deny `Allocator` — these are different capabilities.

#### Interaction with `with...in`

`with Cap = impl in expr` provides a capability within a scope. If `Cap` is in the current denied set, this is a compile error (Rule 4). Otherwise, `with...in` works normally.

```ori
@f () -> void without Allocator = {
    // with Allocator = impl in { ... }  // ERROR E1261
    with Http = MockHttp {} in { ... }   // OK — Http is not denied
}
```

#### Interaction with Capsets

Capsets expand before denial checking. `without NetCapset` where `capset NetCapset = Http, Dns, Tls` denies all three.

```ori
capset AtomicDenials = Allocator, Suspend, SleepingLock

@irq_handler () -> void without AtomicDenials = ...
// Equivalent to: without Allocator, Suspend, SleepingLock
```

#### Interaction with Trait Methods

Trait method signatures can include `without` clauses. Implementations must respect them:

```ori
trait InterruptHandler {
    @handle (self, frame: InterruptFrame) -> void
        without Allocator, Suspend
}

impl MyDriver: InterruptHandler {
    @handle (self, frame: InterruptFrame) -> void
        without Allocator, Suspend = {
        // Implementation inherits denial
    }
}
```

An implementation may add *additional* denials (stricter) but not remove them (weaker).

#### Interaction with `unsafe { }`

`unsafe { }` discharges the `Unsafe` marker capability. If `Unsafe` is in the denied set, `unsafe { }` is a compile error — the denial cannot be overridden, not even by `unsafe`.

### IR Representation

#### Function Type Extension

Add a `denied` field to the capability representation in function types:

```rust
pub struct CapabilitySet {
    pub required: BTreeSet<CapabilityId>,  // existing: uses X
    pub denied: BTreeSet<CapabilityId>,    // new: without X
}
```

#### ExprKind — No New Variant

`without` is a signature-level annotation, not an expression. No new `ExprKind` variant is needed.

### Error Codes

| Code | Description | Example |
|------|------------|---------|
| E1260 | Capability denied in current context | Calling `uses Allocator` from `without Allocator` |
| E1261 | Cannot provide denied capability | `with Allocator = impl in` where Allocator is denied |
| E1262 | Effect variable instantiated with denied capability | `f<{Allocator, Http}>()` where `E without Allocator` |
| E1263 | Implementation weakens trait denial | `impl` omits `without X` that trait requires |

### Error Message Design

```
error[E1260]: capability `Allocator` is denied in this context
  --> drivers/net/e1000/irq.ori:42:5
   |
42 |     let buffer = allocate(size: 1024);
   |                  ^^^^^^^^ requires `Allocator` capability
   |
note: `Allocator` is denied by the enclosing function
  --> drivers/net/e1000/irq.ori:30:1
   |
30 | @handle_rx_irq () -> void
31 |     uses InterruptCtx
32 |     without Allocator, Suspend = {
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^ denial established here
   |
help: use pre-allocated buffers or deferred allocation via bottom-half processing
```

```
error[E1261]: cannot provide denied capability `Allocator`
  --> drivers/net/e1000/irq.ori:50:5
   |
50 |     with Allocator = KernelAllocator {} in {
   |     ^^^^ `Allocator` is denied in this context and cannot be overridden
   |
note: denial established here
  --> drivers/net/e1000/irq.ori:32:5
   |
32 |     without Allocator, Suspend = {
   |     ^^^^^^^^^^^^^^^^^
```

### Evaluation Semantics

`without` is purely compile-time. At runtime, the denied set is erased — it has zero cost. The evaluator ignores `without` annotations entirely.

```rust
// In the evaluator: nothing changes
// without is checked during type checking, not evaluation
```

### LLVM Codegen

No LLVM IR changes. `without` is erased before codegen.

---

## Grammar Changes

Additive change to `grammar.ebnf`:

```ebnf
(* Updated function signature *)
function_sig    = "@" identifier [ type_params ] params "->" type
                  [ uses_clause ] [ without_clause ] [ where_clause ] .

(* New production *)
without_clause  = "without" capability { "," capability } .
```

`without` is a context-sensitive keyword (already reserved in import context for `use "m" { Trait without def }`). This adds it as context-sensitive in function signature position. No ambiguity — it appears only between `uses` and `where`/`=`.

---

## Implementation Phases

### Phase 1: Parser + IR (1 week)

1. Parse `without` clause in function signatures
2. Add `denied: BTreeSet<CapabilityId>` to `CapabilitySet` in IR
3. Round-trip: parse → IR → display
4. Tests: parsing positive/negative cases

### Phase 2: Type Checker — Basic Checking (1-2 weeks)

1. Propagate denied set from caller to callee during type checking
2. Check Rule 3 (denial violation): calling `uses X` in `without X` context → E1260
3. Check Rule 4 (no override): `with X = impl in` in `without X` context → E1261
4. Tests: basic denial checking, propagation, override prevention

### Phase 3: Type Checker — Trait + Capset Integration (1 week)

1. Expand capsets in `without` clauses
2. Check Rule 6 (trait implementation respects denials) → E1263
3. Marker capability denial (`without Unsafe`)
4. Tests: capset expansion, trait conformance, marker denial

### Phase 4: Polymorphic Denial (2-3 weeks)

1. Extend capability inference to handle denied sets in effect variables
2. Boolean unification for `uses E without X` patterns
3. Check Rule 5 (effect variable instantiation) → E1262
4. Tests: polymorphic denial inference, instantiation checking

### Phase 5: Evaluator + LLVM (< 1 day)

1. Evaluator: ignore `without` (already compile-time only)
2. LLVM: no changes (erased before codegen)

---

## Design Decisions

### Why `without` and not `denies`?

`without` reads naturally in English: "this function uses InterruptCtx without Allocator." It matches Flix's syntax convention and Ori's existing use of `without` in import declarations (`use "m" { Trait without def }`).

### Why not closed capability sets (Koka model)?

Koka can express absence by listing a closed set of *allowed* effects. But this is non-modular — adding a new capability to the system requires updating all closed sets. `without` is modular — you name only what is forbidden.

### Why can't denial be overridden?

Denial represents an invariant, not a default. An interrupt handler that allocates memory will crash the kernel regardless of what capabilities are in scope. `with Allocator = impl in` would allow bypassing the invariant, defeating the purpose.

If a developer genuinely needs to override a denial (e.g., in a test), they should use a different function signature that does not include the denial.

### Why is denial monotonic (can only add, never remove)?

This follows the "principle of least authority" from capability-secure systems (Mark Miller). Narrowing authority is safe; widening authority is dangerous. If a caller denies `Allocator`, no callee should be able to re-enable it.

### Why not an integer-based model (like Klint)?

Klint tracks preemption count as an integer range. This is more precise for nested spinlocks (count = 2 vs count = 1) but less composable and requires per-capability arithmetic. Boolean denial (denied or not) is simpler, composes naturally, and handles all kernel context rules.

If integer-valued denial proves necessary in the future, it can be added as a refinement without changing the core model.

---

## Spec Changes Required

1. **§20 Capabilities**: Add `without` clause to capability syntax, define denial semantics
2. **§20.9 Marker Capabilities**: Note that markers can be denied (`without Unsafe`)
3. **§20.11 Capsets**: Capsets expand in `without` clauses
4. **grammar.ebnf**: Add `without_clause` production
5. **Error codes**: Add E1260, E1261, E1262, E1263

---

## Open Questions

1. **Should `without` be inferrable?** If a function's body never uses `Allocator` and never calls anything that uses `Allocator`, should the compiler infer `without Allocator`? **Tentative answer: No.** Denial is an *assertion by the programmer*, not a computed property. Inference would make signatures unstable — adding an allocation to a transitive callee would silently remove the denial.

2. **Should there be a `#[deny_check]` lint for functions that declare `without` but whose bodies would succeed without it?** (I.e., the denial is vacuous because no callee uses the denied capability anyway.) **Tentative answer: Yes, as a warning** — helps catch stale denials after refactoring.

3. **Should denial interact with default implementations?** If a trait has a `def impl` that `uses Allocator`, and a function calls a trait method in a `without Allocator` context, should the default impl be rejected? **Tentative answer: Yes** — the denial applies to all code executed, including defaults.

---

## Non-Goals

- **Runtime denial checking**: `without` is purely compile-time. No runtime cost.
- **Full Boolean effect algebra**: The ICFP 2023 paper defines union, intersection, and complement on effects. This proposal implements only the complement (`without` = `~`). Full Boolean operations can be added later if needed.
- **Automatic denial from context types**: Future work may allow `InterruptCtx` to automatically establish a denial set. This proposal requires explicit `without` declarations.
- **Denial across module boundaries**: Denial propagates through function calls, not module imports. A module that exports a function with `without Allocator` enforces denial on callers of that function, not on the entire module.
