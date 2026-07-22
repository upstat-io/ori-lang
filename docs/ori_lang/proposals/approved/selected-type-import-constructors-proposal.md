# Proposal: Selected Type Import — Constructor and Variant Binding

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Approved:** 2026-07-22
**Affects:** Compiler (import resolver, evaluator, LLVM backend), spec (Clause 15, Clause 18)
**Related:** newtype-pattern-proposal.md (approved — governs newtype construction `T(v)` and the always-public `.inner` accessor; this proposal extends it into the cross-module-import dimension only), module-system-details-proposal.md (approved — governs re-export chains, visibility-through-chains, and `::` private access, which this proposal's re-export and visibility rules build on), no-circular-imports-proposal.md
**Depends On:** module-system-details-proposal.md

---

## Summary

A selected import — `use "./m" { T }` — brings an exported type `T` into the consumer's scope. The specification defines that a type may be exported (`pub type`), but does not define what a selected type import *binds* beyond the bare type name. This proposal settles that: a selected type import binds the type name (for annotations and struct-literal construction) and, for a newtype, the newtype's constructor callable under the same name. Sum-type variant *constructors* are NOT auto-bound in value position; a variant is imported explicitly by naming it in the import list. Variant *patterns* resolve type-directed against the scrutinee and are never import-gated. The binding is carried by a single resolver-emitted descriptor that every backend consumes, so the evaluator, VM, and compiled backends bind identically.

---

## Motivation

Ori already lets a module export a type (`pub type User = { .. }`, Clause 18.4) and lets a consumer select it (`use "./m" { User }`, Clause 18.3.1). But the specification never states what that import makes usable.

> **Grounding note.** This Motivation describes committed state as of the draft date (2026-07-21). A reader inspecting a working tree that carries in-flight resolver work may observe the type-name binding already present; that does not change the semantics this proposal decides, which no committed clause answers.

### The Problem in Practice

Given a provider module:

```ori
// shapes.ori
pub type Widget = { size: int };
pub type Meters = int;
pub type Shape = Circle(radius: int) | Square(side: int);
```

A consumer imports and uses them:

```ori
use "./shapes" { Widget, Meters, Shape };

let w = Widget { size: 7 };      // struct literal — resolves via the type
let m = Meters(42);              // newtype construction — CALL-shaped
let c = Circle(radius: 3);       // sum-variant construction — CALL-shaped
```

At the committed implementation, **every one of those three lines fails identically on every backend.** `compiler/oric/src/imports/mod.rs` models a module's importable surface as exactly two namespaces — functions and constants — with no type namespace at all, so a selected import naming a type falls into the function bucket and is reported as a missing function:

```
error[E2003]: import error: function 'Widget' not found in module './shapes.ori'
```

The resolver is a single shared artifact consumed by both the evaluator and the compiled path, so there is no per-backend divergence here: the failure is uniform.

Binding the type NAME is a straightforward resolver defect (add the third namespace) and is tracked as a compiler bug — it is **out of scope for this proposal**. What that fix does *not* settle, and what no clause of the specification answers, is the question this proposal exists to decide:

> Once a selected import binds the type name, what else does it bind?

- `Widget { size: 7 }` needs only the type — struct-literal construction is type-driven, so it follows from the name binding alone.
- `Meters(42)` is **call**-shaped. It needs a constructor *value* in the consumer's scope. Nothing in the specification says a type import provides one.
- `Circle(radius: 3)` needs the *variant* constructors of an imported sum type. Nothing says whether importing `Shape` brings `Circle` and `Square` into scope, or whether they must be imported by name.

Leaving those two questions unspecified is what invites backends to answer them differently — each one independently deciding whether to bind a constructor value is exactly how an evaluator-versus-compiled divergence gets created. This proposal fixes the semantics before that divergence can be baked in.

### When This Matters

Any program that imports a newtype or sum type from another module and constructs a value of it. This is ordinary cross-module code — a `Meters` unit type in a `units` module, a `Shape` sum in a `geometry` module. Absent a rule, once the type-name binding lands, each backend would independently decide whether a type import supplies a constructor value — and that per-backend decision is precisely how a program would come to mean different things on the evaluator versus the compiled path, violating the cross-executor parity invariant (`missions.md §ori_eval`). Settling the binding at the resolver, once, forecloses that divergence before it can arise.

---

## Goals and Non-Goals

**Goals:**

- Define exactly what a selected import of a type binds into the consumer's scope.
- Prevent an evaluator-versus-compiled divergence on newtype construction, by settling the semantics at the resolver — via a single binding descriptor every backend consumes — before each backend answers the question independently.
- Define how a consumer constructs an imported sum type's variants in value position, and how those variants resolve in pattern position.
- Define how aliasing (`T as U`) interacts with a bound constructor.
- Define within-import-rank name-collision handling for imported variants.
- Resolve how the binding propagates through `pub use` re-export.
- State the backend-neutral parity requirement and its carrier.

**Non-Goals:**

- Glob / wildcard imports (`use "./m" { * }` or a `Shape.*` variant-glob form). Named imports only.
- Method / trait-impl import semantics (governed by Clause 18.3.5 default-binding rules).
- Qualified variant construction syntax (`Shape.Circle(..)`) — deferred to a named successor proposal (see Alternative 3). This proposal defines *what a bare selected import binds*, not new qualified-access syntax.

---

## Design

A selected import item that names an exported type binds, into the consumer's scope:

1. **The type name** — usable in type annotations, `type` aliases, generic arguments, and struct-literal construction (`T { .. }`). This is the resolver behavior already landed for every type kind.
2. **For a newtype** (`type T = Underlying`) — the constructor callable `T(value)`, under the same name `T`. The newtype's type name and its constructor share one identifier, so binding the type binds the constructor.
3. **For a struct or sum type** — no additional value-namespace callable. A struct is constructed by literal syntax (rule 1). A sum type's variants are separate value-namespace items, imported explicitly (see below).

### Sum-type variants in value position are imported explicitly

Importing a sum type binds the type name only. To *construct* a variant, the consumer names the variant in the import list, exactly as a function is named:

```ori
use "./shapes" { Shape, Circle, Square };   // type + two variant constructors

@classify (s: Shape) -> int = 1;            // Shape: type annotation
let c = Circle(radius: 3);                   // Circle: variant constructor (value position)
```

Importing `Shape` alone binds the type for annotations but not the `Circle`/`Square` constructors in value position. A variant is exported when its parent type is `pub`; a variant of a non-`pub` type is imported with the `::` private-access prefix like any other private item (Clause 18.4.1).

**Mechanism — value-position variant resolution is import-scoped.** Registering an imported sum *type* supports type annotations and pattern typing (the type's variant set is known to the checker), but a variant name enters the consumer's *value* namespace only via an explicit import entry. Registering the type MUST NOT, by itself, make its variant names bare-resolvable in expression position. This requires the value-position variant-resolution path to consult import scope rather than a flat global variant map: a bare `Circle(..)` in expression position resolves only when `Circle` was named in an import (or is a local declaration), never merely because `Shape` was imported.

### Variant patterns resolve type-directed, never import-gated

A bare identifier in a `match` arm is resolved against the **scrutinee's type**, not against import scope. A variant is matchable in pattern position whenever the scrutinee's type is known — independent of whether the variant name was imported. Explicit import (above) governs only value/call position.

```ori
use "./shapes" { Shape };                    // type only — no variant value-imports

@arity (s: Shape) -> int =
    match s {
        Circle(radius:) -> 1,                // Circle resolves against Shape, no import needed
        Square(side:) -> 1,
    };
```

This decouples pattern resolution from the value-namespace import rule and codifies the existing type-directed behavior of the pattern checker (`check_binding_pattern` / `try_resolve_unit_variant`). Without this rule, an import-gated pattern position would turn an unimported nullary variant into an *irrefutable binding* that silently swallows a match arm, leaving later arms unreachable while exhaustiveness still passes — a wrong-answer-with-no-error hazard.

**Collision guard.** When a binding-shaped pattern name (a bare lowercase-or-any identifier intended as a fresh binding) matches a variant of the scrutinee's type, that is a **hard error**, not a silent fresh binding. The error directs the author to rename the binding or match the variant explicitly. (This is the error form of Rust's `bindings_with_variant_name` lint; Ori makes it an error because the silent-binding outcome is a correctness hazard, not a style nit.)

### Generic newtypes

An imported generic newtype (`type Box<T> = ..`) binds its constructor with its type parameters: `use { Box }` binds `Box` such that `Box(v)` constructs a `Box<T>` with `T` inferred from `v` at the call site, exactly as at the declaring module. An explicit type argument is accepted where inference is insufficient (`Box<int>(v)`), following ordinary generic-call rules.

### Orphan-variant import (variant without parent type)

Importing a variant without importing its parent type is permitted (Error Handling, below). The constructed value's variant identity and parent-type identity are fully determined by the imported variant; the parent type's inherent methods and trait impls are reachable on the resulting value through ordinary method/trait resolution (which is type-directed, not import-directed). The parent *type name* is required only where the author must *name* the type — a type annotation, a generic argument, an explicit type ascription.

### Syntax

No new syntax. The existing selected-import list (`{ item, item, .. }`) and alias form (`{ item as name }`) carry every case:

```ori
use "./shapes" { Widget };                   // struct type
use "./shapes" { Meters };                    // newtype: binds Meters(..) constructor
use "./shapes" { Meters as M };               // aliased newtype: M(..) constructs a Meters
use "./shapes" { Shape, Circle, Square };     // sum type + explicit variant constructors
use "./internal" { ::Hidden };                // private type via :: (Clause 18.4.1)
```

### Semantics

- **Newtype constructor binding.** `use { T }` where `T` is a newtype binds `T` in the value namespace to the newtype constructor. `T(v)` produces a value whose runtime type is `T` wrapping `v`. This mirrors `register_newtype_constructors`, which already binds newtype constructors for the *local* module — the import path binds the *imported* newtype's constructor the same way.
- **Aliasing.** `use { T as U }` binds the constructor under `U`; the constructed value's runtime type remains `T`. `use { Meters as M }` makes `M(42)` produce a `Meters`. For a variant, `use { Circle as Ring }` binds the `Circle` constructor under `Ring`; the value's variant identity remains `Circle` of `Shape`.
- **Visibility — construction is public with the type.** A type's constructor / variants are exported iff the type is `pub`. A private type's constructor / variants require the `::` prefix at the import site (Clause 18.4.1). This makes a `pub` newtype's constructor callable by any importer: `pub type Meters = int` allows `Meters(raw)` at every consumer, so a validating smart-constructor cannot be enforced by hiding construction while exporting the type. This is a deliberate continuation of Ori's existing newtype transparency: the approved `newtype-pattern-proposal.md` already makes the `.inner` accessor *always public*, so a `pub` newtype is transparent for access; making construction transparent with the type is the coherent extension of that decision, not an accidental collision with it. A module that needs validated construction exposes a `pub @make (..) -> Result<T, E>` factory function and does not export the raw type's constructor path — but the language does not *enforce* that boundary at the type level.
- **Re-export propagation (`pub use`).** Consistent with the re-export-chain rules of `module-system-details-proposal.md`: a `pub use "./m" { Meters }` of a newtype re-exports the type name AND its constructor (they are one identifier — re-exporting the name re-exports the binding). A `pub use "./m" { Shape, Circle }` of a sum type re-exports the type name and each *explicitly-listed* variant constructor; a `pub use "./m" { Shape }` re-exports the type name only (matching the value-position import rule — the re-export carries exactly what the import binds). Visibility-through-chains and aliasing follow `module-system-details-proposal.md` unchanged.
- **Backend neutrality — REQUIRED, carried by a single descriptor.** The import resolver decides the binding once and emits it as a *constructor-binding descriptor* in `ResolvedImports` — carrying the imported type reference, its kind (struct / newtype / sum-variant), the local (aliased) name, and the resolved visibility. Every backend (the type checker's registry registration, the evaluator's environment binding, and every compiled/JIT backend) CONSUMES that descriptor rather than re-deriving the binding from the provider AST. A backend that performs its own visibility check, or its own newtype/sum classification, or lets a same-named function shadow a bound constructor, is a layering violation. Because the binding is decided once at the resolver, `Meters(42)` is accepted identically on every executor and a program that constructs an imported newtype or variant produces the same observable result everywhere.

### Error Handling

- Importing a variant whose parent sum type is not imported is permitted (the variant is a self-standing value-namespace item, like Gleam). Constructing it yields a value of the parent type; the parent type name is only needed for annotations (see Orphan-variant import, above).
- **Name collisions at the import site are errors, disambiguated with `as`.** Cross-category resolution order (locals > params > module items > imports > prelude) is already settled by Clause 18.7 and is unchanged — an import of `None` shadowing the prelude `None` follows that order. This proposal defines the *within-rank* cases the explicit-variant model newly introduces, each an error at the import site:
  - Importing the same name from two providers (`use "./a" { Node }; use "./b" { Node }`) — error; disambiguate with `use "./b" { Node as BNode }`.
  - Importing a variant whose name duplicates another imported variant *from the same provider* (two sum types in one provider sharing a variant name) — error; disambiguate with `as`.
  - Importing a variant whose name collides with an already-imported function, constant, or type in the same rank — error; disambiguate with `as`.
  These are hard errors rather than silent last-wins: rejecting Alternative 1 (below) on collision grounds while silently last-wins-shadowing the explicit model's own collisions would be self-contradictory.
- Naming an item that is neither a function, constant, type, nor variant in the provider remains the existing "not found" import error (Clause 18.3).
- Importing a non-`pub` type / variant without `::` remains the existing private-access error.

---

## Drawbacks

- **A new resolver classification dimension.** The import resolver classifies an imported name across the function, constant, and type namespaces. A *variant* name is in none of those — it is nested inside a sum type's declaration, not in the provider's top-level function or type inventory — so supporting `use { Circle }` requires the resolver to build a new *variant inventory* by walking each provider sum type's variant list. This is a fourth classification dimension, not a refinement of the existing type dimension, and it carries the within-provider variant-name collision surface handled under Error Handling.
- **Explicit-variant ergonomics.** Requiring `use { Shape, Circle, Square }` is more verbose than an auto-binding `use { Shape }`. This is the deliberate cost of the explicit model (see Alternatives) — the same cost Rust, Gleam, and Haskell accept. With qualified construction deferred (Alternative 3) and globs excluded, explicit enumeration is currently the only value-position access path for a wide sum type; the successor proposal named there is the intended relief.
- **Two identifiers, one name (newtype).** Binding a newtype name into both the type and value namespaces means `Meters` denotes a type in type position and a constructor in call position. This dual role already exists for local newtypes; the proposal only extends it across the import boundary.
- **Construction transparency is not encapsulation.** As noted under Semantics, a `pub` newtype's constructor is public with the type, so the type system does not enforce a validated-construction boundary. This is a deliberate coherence choice with the always-public `.inner` accessor, but it means smart-constructor patterns rely on a factory-function convention, not a language-enforced private constructor.

---

## Alternatives Considered

### Alternative 1: Auto-bind sum variants on type import

`use { Shape }` would bind `Shape`, `Circle`, and `Square` all at once (variants bare in value position).

Rejected. Every surveyed language rejects this: importing a type does not silently inject its variant names into the consumer's value namespace, because variant names (`Ok`, `Error`, `None`, `Node`, `Leaf`) collide readily across modules and the injection is invisible at the import site. Rust requires `use Enum::*` or explicit variant paths; Gleam requires each constructor named; Haskell requires `Type(..)` or `Type(Ctor)`. Auto-binding trades a small keystroke saving for silent namespace pollution and cross-module name collisions.

### Alternative 2: Require explicit constructor import even for newtypes

`use { Meters }` would bind only the type; `Meters(42)` would need a separate value-namespace import.

Rejected. A newtype's constructor is not a separate name — it *is* the type's name. There is no second identifier to import. Rust binds a tuple-struct's constructor with its type *when the tuple field is visible to the importer* — a `pub struct Meters(f64)` exports the type but not the constructor if the field is private, and `pub struct Meters(pub f64)` exports both. Ori's newtype makes the wrapped value transparent (always-public `.inner`), so the Ori analogue of "field visible" always holds: binding the newtype constructor with the type is the faithful adaptation of the Rust rule under Ori's transparency. Forcing a phantom second import of the same identifier is ceremony with no disambiguation benefit.

### Alternative 3: Qualified-only variant construction (`Shape.Circle(..)`)

Instead of importing variants, always construct them qualified through the type.

Deferred to a named successor proposal, not rejected. Qualified variant access is a plausible independent feature, but it is new *syntax* and orthogonal to *what a selected import binds*. This proposal defines the import-binding rule; a follow-up proposal (`qualified-variant-construction`, to be drafted) adds qualified variant construction as an additional value-position access path — the committed relief for the explicit-enumeration ergonomics cost noted in Drawbacks. The two compose: explicit import gives bare `Circle(..)`, qualified access gives `Shape.Circle(..)`. This proposal treats that successor as its named follow-up dependency, not as an open possibility.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:** Import resolution is a compiler responsibility — it maps import syntax to name bindings across namespaces before type checking. It cannot be expressed in library code. The evaluator's runtime-constructor binding and the compiled backend's codegen are likewise compiler-internal.

**Missing features that would enable purity:** None applicable; this is definitionally a compiler concern.

**Recommendation:** Proceed as a compiler + spec feature. The change is a resolver rule plus matching evaluator/VM/compiled binding, plus a spec clause defining the rule. No new syntax.

---

## Spec & Grammar Impact

- **Grammar:** none. The import-list grammar (`import`, item list, `as` alias) already parses every case.
- **Spec Clause 18.3 (Imports):** add a subclause defining what a selected import of a type binds — the type name; for a newtype, its constructor; for a sum type, the type name only, with variant *constructors* imported explicitly by name for value position. State the aliasing rule, the value-position import-scoped resolution mechanism, and the backend-neutral parity requirement carried by the resolver-emitted binding descriptor.
- **Spec Clause 18.4 (Visibility):** clarify that a variant / constructor is exported iff its parent type is `pub`, and is importable with `::` otherwise; state that construction is public with the type (no type-level encapsulation of construction).
- **Spec Clause 18.5 / 18.5.1 (Re-exports and re-export chains):** state that a `pub use` carries exactly what the corresponding import binds — a newtype's constructor with its name, and each explicitly-listed variant constructor for a sum type.
- **Spec Clause 18.7 (Resolution):** add the within-rank collision rule (imported-name collisions across providers, intra-provider variant duplication, variant-vs-other-item collisions are errors, `as`-disambiguated); the existing cross-category ordering is unchanged.
- **Spec Clause 15 (Patterns):** clarify that variant-pattern resolution is type-directed against the scrutinee and is not import-gated, and that a binding-shaped pattern name colliding with a scrutinee variant is an error.

---

## Prior Art

Languages surveyed against their cloned reference repositories (`~/projects/reference_repos/lang_repos/`) where available; rows marked *(external)* are established behavior verified against language references rather than repo source.

| Language | Type import | Variant / constructor import |
|----------|-------------|------------------------------|
| **Rust** (repo) | `use m::Enum;` imports the type only | Variants need `use m::Enum::{V1, V2}` or `use m::Enum::*`. A tuple-struct `use m::Meters;` binds the constructor *only when the tuple field is visible to the importer* — `pub struct Meters(f64)` exports the type but not the private-field constructor; `pub struct Meters(pub f64)` exports both. |
| **Gleam** (repo) | `import m.{type Shape}` imports the type | Constructors imported separately by name: `import m.{Circle, Square}`. Types and constructors are distinct import items. |
| **Haskell** *(external)* | `import M (Shape)` imports the type only | `import M (Shape(..))` for all constructors; `import M (Shape(Circle))` for specific ones. |
| **Swift** *(external)* | `import M` brings the enum type | Cases are accessed qualified (`Shape.circle`); no bare case binding at import. |
| **OCaml** *(external)* | Constructors are brought into scope by `open M` (module-wide), not by a selected type import | No per-type selective variant import; `open` is all-or-nothing. |

The consistent finding — for the specific question of a *selective type import* — across every surveyed language: importing a sum type does **not** silently bind its variants in value position; variant/constructor binding is explicit (named) or module-wide (`open`/glob), never an implicit side effect of importing the type. The surveyed languages split on the *alternative* access path they provide (Rust/Haskell offer a glob; Swift offers qualified cases; OCaml offers module-wide `open`), which is why this proposal names qualified construction as a committed successor rather than closing every path. Rust's tuple-struct rule — the single-constructor case binds with the type *because they share one name, subject to field visibility* — is the precedent this proposal adopts for newtypes, under Ori's always-transparent newtype (the field-visibility precondition always holds).

Ori-internal precedent — **specification-level only**: `docs/ori_lang/v2026/spec/18-modules.md §18.3.5` already defines an import that binds more than the bare name (a *trait* import binds the trait name **plus its `def impl`**, with `without def` to opt out). "An import binds the name and its associated construct" is therefore an established shape in the *specification*, which is what this proposal reasons from when it gives a newtype import its constructor.

This is cited as a design precedent, NOT as a claim about current behavior: the import resolver carries no `without def` handling, so §18.3.5's binding is not verified as implemented. The precedent argues for consistency of the specification's shape; it is not evidence of what the compiler does today.

---

## Unresolved Questions

- **Variant visibility granularity.** This proposal ties a variant's exportability to its parent type's `pub`. Whether individual variants could ever carry independent visibility is left open (no current use case; every surveyed language ties variant visibility to the type).
- **Language-enforced private construction.** Construction is public with the type (Semantics). Whether Ori should later add a mechanism to hide a `pub` type's constructor while exporting the type (restoring a language-enforced validated-construction boundary) is left open; it interacts with the always-public `.inner` decision and would be its own proposal.
