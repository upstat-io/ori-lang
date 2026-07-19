# Proposal: Trait Philosophy — Traits as Type Properties

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-04-14
**Affects:** Spec (Clauses 8, 9, 10, 12), standard library documentation, language guide, compiler documentation
**Related:** `value-trait-proposal.md` (approved), `sendable-interior-mutability-proposal.md` (approved), `clone-trait-proposal.md` (approved), `derived-traits-proposal.md` (approved), `drop-trait-proposal.md` (approved), `limbs-trait-proposal.md` (draft)

---

## Summary

Codify Ori's unified trait model as a first-class language principle: **a trait is a declared property of a type — a fact about what the type *is* — which the compiler consumes at every pipeline stage to make decisions about memory management, code generation, algorithm selection, and optimization.**

This proposal does not introduce new syntax or behavior. It articulates the philosophy that already underlies Ori's trait system — and makes explicit the design invariants that distinguish Ori's traits from the interface/typeclass models of Rust, Java, C#, and Haskell.

The central claim:

> In Ori, there is no distinction between "marker traits" and "method-bearing traits." **All traits are properties.** Some properties imply operations; some don't. The compiler treats them uniformly.

---

## Motivation

### The Philosophy Exists But Is Fragmented

Ori's approved proposals already treat traits as compiler-consumable properties, not interface method tables:

- **Value proposal**: "the compiler **verifies** a type satisfies `Value`" — verification language, not implementation language
- **Sendable proposal**: "Users CANNOT manually implement `Sendable`" and "a safety property verified by the compiler"
- **Derived traits proposal**: Describes derivation as compiler-synthesized from structure, not as macro code-gen
- **Limbs proposal (draft)**: Uses trait declaration to drive arithmetic algorithm selection, SIMD width, and codegen

But this philosophy is **not articulated anywhere as a unified principle**:

- **Spec Clause 9 (Properties of Types)** frames traits as "what types implement" — interface language
- **Spec Clause 10 (Declarations)** describes trait declarations syntactically but not philosophically
- **Core principles doc** (`archived-design/01-philosophy/02-core-principles.md`) does not mention the trait-as-property concept
- **No document** states that `Value`, `Sendable`, `Eq`, `Iterator`, `Limbs`, `Drop` are all instances of the same underlying model

The result: proposal authors, spec readers, and contributors encounter traits as if they were Rust-style interfaces and then discover — proposal by proposal — that Ori's traits behave differently. The absence of a unifying statement forces each proposal to re-litigate the philosophy.

### The Cost of Fragmentation

Without a codified philosophy, the following problems arise:

1. **New trait proposals must re-justify the model.** Every proposal introducing a "marker-like" trait has to explain why it isn't just a method bag. This duplicates rationale across proposals.

2. **Users coming from Rust/Java transfer the wrong mental model.** They read the spec, see "trait" written the same way as in Rust, and assume interface semantics — missing the fact that Ori's traits drive codegen, memory model, and algorithm selection.

3. **The `Value` / `Sendable` / `Limbs` pattern looks like a special case.** In fact it is the general case — method-bearing traits like `Eq` and `Iterator` are just traits that happen to imply operations. But without articulation, the "marker traits" feel like exceptions.

4. **Future design decisions drift.** Without a stated principle, designers may add compiler-magic escape hatches (like Rust's `auto trait`, `unsafe trait`, `?Sized`) when the existing uniform model would suffice.

### The Ori Distinction

Other languages have drifted from what "trait" means:

| Language | Trait concept | Dispatch model | Compiler integration |
|---|---|---|---|
| **Smalltalk (origin)** | Composable behavior unit (mixin) | Method lookup | None |
| **Rust** | Interface with static dispatch | vtable or monomorphization | Minimal (marker traits as special case) |
| **Haskell** | Typeclass with constraint solving | Dictionary passing | Constraint propagation only |
| **Java/C#** | Interface | vtable | None |
| **Ori** | **Property of the type** | **Not the primary concern** | **Full — every pipeline stage** |

In all the other models, the trait is fundamentally about **how to call methods on the type**. In Ori, the trait is fundamentally about **what the type is** — and the compiler consumes that information everywhere, not just at method call sites.

This is not a refinement of the interface model. It is a categorically different relationship between programmer and compiler.

---

## Design

This proposal is **definitional**, not mechanistic. It codifies principles, not new compiler behavior. The principles below describe the model that already exists in Ori's approved proposals and current implementation — making explicit what has been implicit.

### Principle 1: Traits Are Properties, Not Interfaces

A trait declaration asserts a **property of types that satisfy it**. The property may or may not imply operations.

- `Value` asserts: "this type is inline, bitwise-copyable, and ARC-free"
- `Sendable` asserts: "this type is safe to transfer across tasks"
- `Eq` asserts: "this type supports equality comparison"
- `Iterator` asserts: "this type produces a sequence of elements"
- `Limbs` asserts: "this type is a fixed-width multi-word integer"
- `Drop` asserts: "this type has cleanup semantics at end-of-lifetime"

Each assertion is a **fact the compiler can reason about**. Methods, when present, are the *access pattern* for the property — not the definition of it.

### Principle 2: No Marker / Non-Marker Distinction

Ori has a single category: **traits**. There is no syntactic, semantic, or compiler-pipeline distinction between traits with methods and traits without.

```ori
// All of these are traits. Same system. Same processing.
trait Value: Clone, Eq {}
trait Sendable {}
trait Limbs: Value {
    type Limb: Value
    let $WIDTH: int
    @limb (self, index: int) -> Self.Limb
    // ...
}
trait Eq {
    @equals (self, other: Self) -> bool
}
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)
}
```

The absence of methods on `Value` and `Sendable` is not a special case — it is simply what those properties look like. A property that does not imply an operation has no methods. A property that does imply operations has methods. Both are traits.

This is in deliberate contrast to Rust, which distinguishes `trait`, `auto trait`, `unsafe trait`, `?Sized` traits, and has special compiler treatment for `Copy`, `Send`, `Sync`, `Sized`, `Unpin`. Ori rejects this stratification.

### Principle 3: The Compiler Consumes Traits at Every Stage

A trait declaration is information that flows through the entire compiler pipeline, not just method lookup:

| Pipeline stage | What it consumes from traits |
|---|---|
| **Parser** | Trait names in type declarations (structural) |
| **Type checker** | Trait bounds, conformance, method resolution |
| **Canonicalization** | Operator → trait method desugaring |
| **AIMS (memory model)** | `Value` for ARC elimination; `Drop` for lifetime scheduling; `Sendable` for escape analysis |
| **ARC lowering** | Trait conformance determines RC emission |
| **Code generation** | `Limbs.$WIDTH` for algorithm selection; `Value` for register allocation; SIMD width from trait type parameters |
| **Optimization** | Trait-derived invariants enable dead-code elimination in `$if`/`$for` expansion |

A trait is not "seen" only by the type checker. Every stage of the compiler has access to trait facts about types and uses them to make decisions.

### Principle 4: Methods Are Consequences, Not Definitions

When a trait has methods, those methods exist *because the property implies them*, not because the trait is *defined as* "a collection of methods."

- A type that is `Eq` has `equals` **because** being equatable means you can check equality. The method is the *expression* of the property.
- A type that is `Limbs` has `limb` **because** being a multi-word integer means you can access individual limbs. The method is the *expression* of the property.

This is why Ori's derived implementations are semantic inferences, not macro substitutions. When the compiler derives `Eq` for a struct, it is not "running a code-gen macro." It is **computing the correct implementation** from the fact that "this type is equatable" plus the fact that "this type has these fields." The derivation is the only possible implementation consistent with the property. The compiler writes it because it is the logical consequence of the declaration.

### Principle 5: Derivation Is Semantic Inference

Ori's derived trait implementations (`#derive(Eq, Hashable, Comparable, Clone, Default, Debug, Printable)` and the structural derivations driven by `Value`, `Sendable`, `Limbs`) are **not macro expansions**. They are semantic inferences:

- The compiler knows what `Eq` means (reflexivity, symmetry, transitivity)
- The compiler knows what the type is (a struct with fields `x: int, y: int`)
- The compiler knows the fields are themselves `Eq`
- Therefore the compiler knows the only correct implementation: `self.x == other.x && self.y == other.y`

This is inference, not code generation. The distinction matters because inference is **guaranteed correct** by construction — if the types satisfy the premises, the derived implementation satisfies the trait's semantics. Rust's `#[derive(...)]` is a proc macro that walks fields and emits code; if the macro has a bug, the code is wrong and the compiler shrugs. Ori's derivation is a proof that produces the implementation as a byproduct.

### Principle 6: Trait Declaration Is Trait Conformance

When a type declares a trait in its header, that declaration is the **assertion of conformance**. The compiler's job is to verify the assertion, not to accept it on faith.

```ori
type Point: Value, Eq = { x: float, y: float }
```

This says: "Point is a Value, and Point is Eq." The compiler verifies:
- All fields of Point are `Value` → assertion valid → `Value` conformance granted
- All fields of Point are `Eq` → derivation possible → `Eq` conformance granted, implementation synthesized

If verification fails, the declaration is rejected at the type level — the type does not exist in an inconsistent state. This is stronger than Rust's "you can `impl Trait for Type` and the compiler checks later" model. In Ori, the trait list on a type *is* the conformance set, and it is verified at declaration time.

### Principle 7: Traits Compose as Facts

When a type declares multiple traits, each trait adds an independent fact the compiler consumes. Facts compose; they do not interact through method-resolution precedence.

```ori
type U256: Value, Limbs, Eq, Comparable, Hashable, Add, Mul = {
    digits: [int, max 4],
}
```

This is seven facts about U256. The compiler composes them:

- `Value` + `Limbs` → ARC-free multi-word integer → register-allocatable, no heap
- `Limbs` + `Add` → can derive carry-chained addition
- `Limbs.$WIDTH = 4` + `Add` → unroll via `$for`, no loop overhead
- `Value` + `Limbs.$WIDTH <= 8` → fits in SIMD registers → auto-vectorize
- `Eq` + `Comparable` + `Hashable` → structural comparison and hashing derived from limb layout

The programmer does not choreograph these interactions. The programmer states facts. The compiler composes them into an implementation.

This is the deepest consequence of the trait-as-property model: **trait stacking is optimization stacking**. More facts about a type give the compiler more information to optimize with, and the composition is automatic.

---

## Semantic Rules

### Rule 1: Trait Uniformity

Every trait declaration is parsed, type-checked, and pipeline-consumed by the same machinery. There shall be no syntactic or semantic category that distinguishes "marker traits" from "method-bearing traits." A trait with zero methods is a valid, ordinary trait.

### Rule 2: Declared Conformance

A type's trait list on declaration shall be the authoritative conformance set for that type. Conformance shall be verified at declaration time, not deferred to usage. Types with failing conformance shall be rejected at type-checking, not at method-call sites.

### Rule 3: Derivation Is Inference

When the compiler synthesizes a trait implementation for a type, that implementation shall be derivable from:
1. The trait's declared semantics, and
2. The type's structural properties and its other declared trait conformances.

Implementations that cannot be so derived shall require explicit user implementation. Implementations that can be so derived shall not require explicit user implementation except when the user wishes to override behavior (where the trait permits override).

### Rule 4: Trait Facts Flow Through the Pipeline

Trait conformance shall be derived once by the frontend and exposed through typed, stable identities to the downstream phase that owns each decision. AIMS may consume the exact logical properties needed for ownership/effect proofs; physical backends receive only the frozen executable and physical-plan facts needed for faithful projection.

LLVM, VM, native, compiled-WebAssembly, and JIT code shall not inspect the full trait set to reconstruct semantics, ownership, layout, or helper policy already decided upstream.

### Rule 5: No Auto/Unsafe/Special Traits

Ori shall not introduce `auto trait`, `unsafe trait`, `default trait`, or similar compiler-magic subcategories for traits. When a new trait requires special treatment, that treatment shall be expressed through:
- Supertrait requirements (e.g., `trait Limbs: Value`)
- Associated types and constants (e.g., `type Limb: Value`, `let $WIDTH: int`)
- Verification rules in the trait's own definition

not through a new syntactic trait category.

### Rule 6: Method Presence Is Not Definitional

Whether a trait has zero methods, one method, or many methods shall not affect how the trait is handled by the compiler or the language. A method-less trait is not a "marker" — it is simply a trait whose property does not imply operations.

### Rule 7: Operator Desugaring Respects Trait Semantics

Operators that desugar to trait methods (e.g., `a + b → Add::add(a, b)`) shall do so because the operator expresses the property the trait declares. Operator traits are not a special category — they are ordinary traits whose methods happen to be the desugar targets of syntactic operators.

---

## Comparison With Other Languages

### Rust

Rust's trait system is fundamentally interface-based with marker traits as a stratified special case:

- **Normal traits**: Method bags with static or dynamic dispatch
- **Auto traits** (`Send`, `Sync`): Compiler infers conformance structurally; opt-out with `!`
- **Unsafe traits** (`Send`, `Sync`): Require `unsafe impl` acknowledgment
- **Built-in traits** (`Sized`, `Copy`, `Drop`): Special compiler knowledge
- **Marker traits** (`Copy`, `Sized`): Empty trait bodies with semantic meaning

The fact that Rust needs five subcategories is evidence that the base model (interfaces) cannot express the full range of trait use cases without escape hatches. Ori collapses these into one category.

### Haskell

Haskell's typeclasses are closer to Ori's traits in spirit — they describe properties, drive derivation (`deriving Eq`), and feed into constraint solving. But:

- Typeclass membership does not feed into codegen decisions (no memory model impact, no algorithm selection)
- Typeclass dispatch is dictionary-passing, which still treats classes as method tables at the machine level
- No equivalent of `Value` or `Sendable` as memory-model-affecting properties

Haskell understood the property model better than Rust did, but did not extend it to the full compiler pipeline.

### Java/C#

Interfaces are pure method contracts with dispatch via vtable. No structural implications, no compiler-pipeline integration beyond dispatch. The word "trait" does not apply.

### Smalltalk (Origin)

The original "trait" concept (Schärli, Ducasse, Nierstrasz, Black, 2003) described traits as composable units of behavior — essentially mixins with conflict resolution. This is behavioral, not property-based. Rust adopted the term from this lineage but stripped the composability; Ori returns to the original intent of "trait" (characteristic / property) while going further than the 2003 paper by making the characteristic a compiler-consumable fact.

---

## Implications

### For Proposal Authors

New trait proposals no longer need to justify the "this is a property, not an interface" framing from scratch. They can cite this proposal as the foundation and focus on:
- What property the new trait expresses
- What the compiler can do with that property
- What supertraits and associated items are required

### For Spec Authors

Spec Clause 9 (Properties of Types) should be updated to frame traits as properties, with method lists as the expression of properties where applicable. The spec should not describe any trait as "a marker trait" — only as "a trait" with or without methods.

### For Language Learners

A single mental model suffices: **traits are declared properties of types that the compiler verifies and consumes.** No special cases, no stratification. A learner who understands `Eq` understands `Value` and `Limbs` — same model, different property.

### For Future Features

Features that might be tempted to introduce a new trait category instead extend the existing model:
- **Constant-time crypto traits**: `ConstantTime` is a trait whose property constrains codegen
- **Allocator-aware types**: `AllocatorAware` is a trait whose property modifies memory layout
- **Effect-tracking traits**: Capabilities (`Http`, `FileSystem`) are already modeled this way

---

## Spec & Grammar Impact

### Spec Changes

- **Clause 8 (Types)**: Add a short subsection titled "Traits as Type Properties" stating Principle 1 and Principle 2
- **Clause 9 (Properties of Types)**: Reframe as "Properties of Types (via Traits)." Remove language that distinguishes "marker traits" from "other traits." Group trait listings by property category (memory, identity, iteration, arithmetic) rather than by method presence.
- **Clause 10 (Declarations)**: Clarify that trait declarations assert conformance at declaration time and that conformance is verified structurally.
- **Clause 12 (Traits)**: Add a preamble paragraph stating the trait philosophy and citing this proposal.
- **No grammar changes** — the philosophy does not introduce new syntax.

### Documentation Changes

- Update `.claude/rules/ori-syntax.md` to describe traits as properties in its header comment
- Update language guides and tutorials to lead with "traits are properties" rather than "traits are interfaces"
- Archive or update `archived-design/01-philosophy/02-core-principles.md` to include the trait philosophy as a core principle

---

## Design Decisions

### Why codify now?

The `Limbs` proposal (draft, 2026-04-14) makes the trait-as-property model load-bearing in a way that previous proposals did not. Without codifying the philosophy, `Limbs` reads as a novel and surprising use of traits. With the philosophy codified, `Limbs` is the natural extension of the existing model. Future proposals (Montgomery arithmetic, constant-time operations, allocator awareness) will benefit from the same codification.

### Why not just update the spec?

The spec describes *what* the language is. This proposal describes *why* it is that way — the design principle behind the spec's decisions. Both are needed. Once this proposal is approved, the spec updates follow naturally; the spec cites the principle.

### Why no new syntax or compiler changes?

The philosophy already exists in the implementation. This proposal does not change behavior — it names the behavior. Every compiler pass that currently consumes trait information continues to do so; every approved trait (Value, Sendable, Clone, Drop, Eq, etc.) continues to work identically. What changes is how future proposals and spec sections are written.

### Why is this a proposal and not just a design doc?

Proposals are versioned, reviewed, and authoritative. A design doc in `archived-design/` or an informal guide in `docs/` would be easier to ignore or contradict in future proposals. Proposals must be explicitly superseded, giving the philosophy the durability it needs to shape future work.

---

## Related Proposals

This proposal retroactively unifies the trait philosophy established across:

- **`value-trait-proposal.md`** (approved 2026-03-05) — First clear articulation of "compiler-verified property"
- **`sendable-interior-mutability-proposal.md`** (approved 2026-01-30) — Auto-derived property, no manual implementation
- **`clone-trait-proposal.md`** (approved 2026-01-28) — Method-bearing trait with deep-copy semantics
- **`drop-trait-proposal.md`** (approved 2026-01-30) — Lifecycle property with compiler-scheduled operations
- **`derived-traits-proposal.md`** (approved 2026-01-30) — Structural derivation as inference
- **`capability-unification-generics-proposal.md`** (approved) — `:` syntax for structural properties
- **`limbs-trait-proposal.md`** (draft 2026-04-14) — Arithmetic properties drive algorithm selection

None of these proposals are amended by this one. This proposal only articulates the shared model.

---

## Open Questions

### Should operator traits be explicitly grouped?

`Add`, `Sub`, `Mul`, etc. are ordinary traits whose methods happen to be operator desugar targets. Should the spec explicitly group them as "operator traits" for pedagogical purposes, or should it resist the grouping to reinforce uniformity? Proposal recommendation: group them pedagogically but explicitly note that the grouping is presentational, not semantic.

### Should the spec include a "trait taxonomy"?

There may be value in a non-normative table that classifies existing traits by the category of property they express (memory, identity, iteration, arithmetic, lifecycle, capability). This would aid learning without introducing compiler-visible categories. Proposal recommendation: include as an informative annex, clearly marked non-normative.

### How does this interact with future effect/capability traits?

Ori's capability system (`Http`, `FileSystem`, etc.) is already property-like — a function "has" a capability it uses. Should capabilities be explicitly framed as traits under this philosophy, or do they remain a distinct concept? Proposal recommendation: leave for a future proposal; the current capability model is working, and unification can be considered separately.

---

## Summary Table

| Concept | Rust / Java / C# | Haskell | Ori |
|---|---|---|---|
| What a trait *is* | Interface (method contract) | Typeclass (method + constraint) | **Property of the type** |
| Marker traits | Special case (`auto`, empty) | Not a concept | **Not distinguished — traits are traits** |
| Method presence | Required (except markers) | Required | **Optional — method is consequence, not definition** |
| Derivation | Macro (proc macro in Rust) | Compiler-built-in (`deriving`) | **Semantic inference** |
| Compiler pipeline integration | Dispatch only (+ marker special cases) | Dispatch + constraint solving | **Every stage consumes trait facts** |
| Memory model impact | None (except `Copy`, `Drop`) | None | **Yes — `Value` eliminates ARC, `Drop` schedules cleanup** |
| Codegen impact | None (except via monomorphization) | None | **Yes — `Limbs.$WIDTH` selects algorithms and SIMD** |
| Conformance model | `impl Trait for Type` | `instance Typeclass Type` | **Declared on type, verified at declaration** |

---

## Non-Goals

- This proposal does **not** change any existing trait's behavior
- This proposal does **not** introduce new syntax, keywords, or compiler passes
- This proposal does **not** deprecate any existing approved proposal
- This proposal does **not** address the capability system (left for future work)
- This proposal does **not** prescribe a taxonomy that the compiler enforces — the uniformity principle forbids compiler-visible categories beyond "trait"

## Goals

- **Articulate** the trait-as-property philosophy as a first-class language principle
- **Unify** the scattered framing across existing approved proposals
- **Guide** future proposals and spec updates to use consistent language
- **Differentiate** Ori's trait model clearly from Rust/Haskell/Java/C# for learners and contributors
- **Anchor** the design rationale for proposals like `Limbs` that depend on the property-model
