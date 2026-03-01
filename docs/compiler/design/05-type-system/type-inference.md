---
title: "Type Inference"
description: "Ori Compiler Design — Type Inference"
order: 502
section: "Type System"
---

# Type Inference

## What Is Type Inference?

In an explicitly typed language like C or Go, the programmer writes the type of every variable:

```c
int x = 42;
float y = 3.14;
char* name = "hello";
```

In an inferred language like Ori, the compiler deduces types from usage:

```ori
let x = 42           // compiler infers: int
let y = 3.14         // compiler infers: float
let name = "hello"   // compiler infers: str
```

**Type inference** is the algorithm that makes this work. Given an expression and the types of its sub-expressions, the inference algorithm determines the type of the whole expression — without requiring the programmer to write it down.

This is more than a convenience feature. Inference enables polymorphism without annotation burden. A function like `@identity (x) = x` has type `forall T. T -> T` — it works on any type. The programmer writes no types at all; the inference algorithm discovers the most general type automatically.

### A Brief History

The story of type inference spans six decades and connects logic, programming languages, and automated theorem proving.

**Haskell Curry** (1958) observed that the types of combinators in combinatory logic could be determined mechanically from their structure. His work on the correspondence between proofs and programs (the Curry-Howard correspondence) laid the theoretical foundation.

**Roger Hindley** (1969) proved that the principal type (most general type) of a term in the simply typed lambda calculus is unique and computable. He gave the first algorithm for finding it, based on Robinson's unification algorithm (1965).

**Robin Milner** (1978) independently developed the same algorithm for the ML programming language, calling it Algorithm W. Milner's contribution was practical: he showed how to extend the algorithm with `let`-polymorphism, allowing local definitions to be used at multiple types without explicit annotation. This was the key insight that made type inference practical for real programming.

**Luis Damas and Robin Milner** (1982) formalized the [type system and proved Algorithm W correct](https://dl.acm.org/doi/10.1145/582153.582176). The resulting system — **Damas-Milner** or **Hindley-Milner** (HM) — became the foundation for type inference in ML, Haskell, OCaml, F#, Elm, Roc, and now Ori.

### Inference Strategies

Modern compilers use several inference strategies, often in combination:

**Algorithm W (bottom-up synthesis).** Walk the AST bottom-up, synthesizing a type for each expression from its sub-expressions. When two types must agree (e.g., both branches of an `if`), *unify* them — find a substitution that makes them equal. This is Milner's original algorithm and the basis for most HM implementations.

**Algorithm J (with mutable state).** A variant of Algorithm W that uses mutable union-find cells for type variables instead of explicit substitutions. More efficient in practice (no substitution composition), but harder to reason about formally. Most production implementations, including Ori's, use this approach.

**Bidirectional type checking.** Combine bottom-up synthesis with top-down checking. When the expected type is known (from an annotation, a function signature, or context), *check* the expression against it top-down rather than synthesizing bottom-up. This gives better error messages ("expected `int` but found `str`" instead of "can't unify `int` and `str`") and supports features that pure synthesis cannot (like checking lambda parameters against known types). Introduced by Pierce and Turner (2000), now widely used — [Elm](https://github.com/elm/compiler), [Roc](https://github.com/roc-lang/roc), and Ori all combine synthesis with checking.

**Constraint-based inference.** Generate all type constraints first (e.g., "expression E3 must have the same type as parameter P1"), then solve them in a separate pass. This separates the "what constraints exist?" question from the "what types satisfy them?" question. Used by GHC (for its complex constraint language involving type classes, type families, and GADTs) and some Rust trait-solving machinery. More powerful but more complex than immediate solving.

Ori uses **Algorithm J with bidirectional checking** — immediate solving via mutable union-find, enhanced with top-down type checking when expected types are available. This provides the simplicity and efficiency of Algorithm J with the error quality benefits of bidirectional checking.

## The InferEngine

The `InferEngine` orchestrates type inference for individual expressions. It is created fresh for each function body, receiving the pool, environment, and registries from `ModuleChecker`:

```rust
pub struct InferEngine<'pool> {
    // Core inference machinery
    unify: UnifyEngine<'pool>,              // Unification engine (borrows Pool)
    env: TypeEnv,                           // Name → scheme bindings
    expr_types: FxHashMap<ExprIndex, Idx>,  // Expression → inferred type

    // Error reporting
    context_stack: Vec<ContextKind>,        // For error context
    errors: Vec<TypeCheckError>,            // Accumulated errors
    warnings: Vec<TypeCheckWarning>,        // Accumulated warnings

    // External data (borrowed from ModuleChecker)
    interner: Option<&'pool StringInterner>,
    well_known: Option<&'pool WellKnownNames>,
    trait_registry: Option<&'pool TraitRegistry>,
    signatures: Option<&'pool FxHashMap<Name, FunctionSig>>,
    type_registry: Option<&'pool TypeRegistry>,
    const_types: Option<&'pool FxHashMap<Name, Idx>>,

    // Inference state
    self_type: Option<Idx>,                 // Current function type (for recursion)
    impl_self_type: Option<Idx>,            // Self in impl blocks
    loop_break_types: Vec<Idx>,             // Stack of break value types
    current_capabilities: FxHashSet<Name>,  // uses clause capabilities
    provided_capabilities: FxHashSet<Name>, // with...in capabilities
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,
    mono_instances: Vec<MonoInstance>,       // Generic instantiations
}
```

### Why Fresh Per Function?

Each function body gets a fresh `InferEngine` rather than reusing one across the module. This simplifies state management — the expression type map, error list, and context stack are always empty at the start of each function. It also enables potential parallelization: two function bodies could be checked concurrently with separate engines (though Ori doesn't currently exploit this).

The external data (registries, signatures, interner) is shared via immutable borrows from `ModuleChecker`, which owns the long-lived state. The pool is borrowed mutably through the `UnifyEngine`, since unification mutates variable state.

### Key State Fields

**`self_type`** — The current function's own type, enabling recursive calls. When checking `@factorial(n: int) -> int`, this is set to `(int) -> int` so that the recursive call `factorial(n: n - 1)` can resolve its type without the function being fully checked.

**`impl_self_type`** — The concrete type for `Self` in `impl` blocks. When checking `impl Point { @distance(self) -> float }`, this is set to `Point` so that `Self` in the body resolves correctly.

**`loop_break_types`** — A stack of expected break value types for nested loops. Each `loop` expression pushes a fresh type variable; `break expr` unifies the expression type with the top of the stack. The stack enables nested loops to have independent break types.

**`well_known`** — Pre-interned `Name` handles for common type names (`int`, `str`, `bool`, `Option`, `Result`, etc.). This avoids repeated string interning lookups during type annotation resolution, providing O(1) access to the ~30 most frequently referenced type names.

## Bidirectional Type Checking

Two top-level functions drive expression type checking:

```rust
/// Synthesize a type for an expression (bottom-up).
pub fn infer_expr(engine: &mut InferEngine<'_>, arena: &ExprArena, expr_id: ExprId) -> Idx

/// Check an expression against an expected type (top-down).
pub fn check_expr(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    expr_id: ExprId,
    expected: &Expected,
    span: Span,
) -> Idx
```

**`infer_expr`** (synthesis mode) walks the expression bottom-up, building a type from its components. A literal `42` synthesizes `int`. A binary operation `a + b` synthesizes the return type of the `+` operator for the operand types. A function call synthesizes the return type of the called function.

**`check_expr`** (checking mode) takes an `Expected` type and verifies the expression produces a compatible type. This is used when context provides a known type: function return types, annotated let bindings, `if`/`else` branch unification. Checking mode can provide better error messages because it knows what type was expected and why.

The interplay between these two modes is what makes bidirectional checking powerful. Consider:

```ori
@process (items: [int]) -> [str] = {
    items.map(transform: x -> str(value: x))
}
```

The return type annotation `[str]` flows top-down into the body via `check_expr`. The `map` method knows its return type must be `[str]`, which constrains the lambda's return type to `str`. The lambda parameter `x` is inferred bottom-up as `int` from the input list type. Neither direction alone could infer the complete type — the bidirectional flow makes them collaborate.

### Expression Dispatch

| Expression Kind | Inference Rule |
|----------------|---------------|
| `Literal(Int)` | `Idx::INT` |
| `Literal(Float)` | `Idx::FLOAT` |
| `Literal(Str)` | `Idx::STR` |
| `Literal(Bool)` | `Idx::BOOL` |
| `Ident(name)` | Lookup in `TypeEnv`, instantiate if polymorphic scheme |
| `Binary { op, left, right }` | Infer operands, check operator type rules |
| `Call { func, args }` | Infer function type, unify args with params |
| `FieldAccess { expr, field }` | Infer receiver, look up field or method |
| `Index { expr, index }` | Infer collection type, return element type |
| `If { cond, then, else }` | Cond must be `bool`, unify branch types |
| `Match { scrutinee, arms }` | Infer scrutinee, check patterns, unify arm types |
| `For { var, iter, body }` | Infer iterable element type, bind loop variable |
| `Loop { body }` | Fresh break type variable, infer body |
| `Let { pattern, value, body }` | Infer value, bind pattern, infer body |
| `Lambda { params, body }` | Fresh vars for params, infer body, construct function type |
| `List { elems }` | Unify all elements, return `[T]` |
| `Map { entries }` | Unify all keys and values, return `{K: V}` |
| `Tuple { elems }` | Infer each element, return tuple type |
| `Block { stmts, result }` | Infer statements in sequence, return last expression |
| `Try { body }` | Like block, wraps result in `Result` and enables `?` |

### Identifier Resolution and Instantiation

When an identifier is looked up, the result may be a polymorphic type scheme. The engine must instantiate it with fresh variables to ensure each use site gets independent type variables:

```rust
let ty = engine.env.lookup(name)?;
if engine.pool.tag(ty) == Tag::Scheme {
    engine.unify.instantiate(ty)  // Fresh vars for each quantified variable
} else {
    ty
}
```

This ensures that `id(42)` and `id("hello")` don't interfere — each instantiation creates separate fresh variables that are unified independently.

## Worked Examples

### Simple Let Binding

```ori
let x = 42
let y = x + 1
```

```
Step 1: x — create fresh var T0 at current rank
Step 2: 42 — synthesize int (literal rule)
Step 3: unify(T0, int) → Link T0 → int
Step 4: y — create fresh var T1
Step 5: x + 1 — look up x: int, look up (+): (int, int) -> int
Step 6: unify(T1, int) → Link T1 → int
Result: x : int, y : int
```

### Let Polymorphism

```ori
let id = x -> x
let a = id(42)
let b = id("hello")
```

This example demonstrates why let-polymorphism matters — without it, `id` could only be used at one type.

```
Step 1: Enter rank 3 for let-binding of id
Step 2: Lambda x -> x:
   - x : T0 at rank 3 (fresh var)
   - body: T0 (returns x)
   - type: (T0) -> T0
Step 3: Exit rank 3 — generalize:
   - T0 is unbound at rank 3 → generalize to Generalized
   - id : forall T. T -> T (scheme)
Step 4: id(42):
   - Instantiate scheme: (T1) -> T1 (fresh vars at rank 2)
   - Unify arg: T1 = int
   - Return: int
Step 5: id("hello"):
   - Instantiate scheme: (T2) -> T2 (new fresh vars)
   - Unify arg: T2 = str
   - Return: str
```

The critical step is generalization: because `T0` was created at rank 3 and remains unbound when exiting rank 3, it becomes universally quantified. Each use of `id` then gets independent variables.

### Generic Function with Annotation

```ori
@identity<T> (x: T) -> T = x
identity(42)
identity("hello")
```

```
Step 1: identity — signature collected in Pass 1:
   - T is a rigid variable (from annotation)
   - type scheme: forall T. (T) -> T
Step 2: identity(42):
   - Instantiate: (T0) -> T0 (fresh vars replace rigid T)
   - Unify arg: unify(T0, int) → T0 = int
   - Return: int
Step 3: identity("hello"):
   - Instantiate: (T1) -> T1 (new fresh vars)
   - Unify arg: unify(T1, str) → T1 = str
   - Return: str
```

### Collection Inference

```ori
let xs = [1, 2, 3]
let ys = xs.map(transform: x -> x * 2)
```

```
Step 1: [1, 2, 3]:
   - Fresh element var T0
   - Unify T0 = int (from literal 1)
   - Check 2, 3: both int, unification succeeds
   - Result: [int]
Step 2: xs.map(transform: x -> x * 2):
   - Receiver type: [int]
   - Method lookup: map<A, B>(self: [A], transform: (A) -> B) -> [B]
   - Instantiate: A = T1, B = T2
   - Unify self: [T1] = [int] → T1 = int
   - Lambda x -> x * 2:
     - x : int (from T1)
     - x * 2 : int
     - lambda type: (int) -> int
   - Unify transform: (T1) -> T2 = (int) -> int → T2 = int
   - Return: [int]
```

### Match Expression

```ori
@describe (value: Option<int>) -> str =
    match value {
        Some(n) -> `The value is {n}`,
        None -> "No value",
    }
```

```
Step 1: value : Option<int> (from parameter annotation)
Step 2: match — scrutinee type: Option<int>
Step 3: Arm 1 — Some(n):
   - Pattern Some(n) matches Option<int> → n : int
   - Body: template literal → str
Step 4: Arm 2 — None:
   - Pattern None matches Option<int> → no bindings
   - Body: "No value" → str
Step 5: Unify arm types: str = str ✓
Step 6: Exhaustiveness check: Some + None covers Option ✓
Result: str
```

## Capability Tracking

Functions declare required capabilities with `uses`. The inference engine verifies capability availability at every call site:

```ori
@fetch_data (url: str) -> str uses Http = { ... }

@process () -> str uses Http =
    let data = fetch_data(url: "/api")  // Ok: Http is in current_capabilities
    data

@main () -> void =
    with Http = MockHttp { } in
        process()  // Ok: Http is provided by with...in
```

The `InferEngine` tracks capabilities in two sets:

- **`current_capabilities`** — from the current function's `uses` clause. Set via `set_capabilities()` when creating the engine for each function body.
- **`provided_capabilities`** — from `with...in` expressions in scope. Updated when entering a `with...in` block.

When a called function requires a capability, the engine checks both sets. If the capability is in neither, a type error is emitted with a clear message: "function `fetch_data` requires capability `Http`, but `Http` is not available in this scope."

## Pattern Resolutions

During `match` expression inference, the engine records how patterns resolve to variant indices. This information is needed by the LLVM backend for generating `switch` instructions on enum discriminants:

```rust
pub enum PatternResolution {
    UnitVariant {
        type_name: Name,
        variant_index: u8,  // Tag value for LLVM discriminant
    },
}
```

Pattern resolutions are accumulated in `InferEngine::pattern_resolutions` and emitted as part of `TypedModule`. The evaluator doesn't need them (it dispatches on variant values directly), but LLVM codegen needs the numeric discriminant for each variant.

## Error Accumulation

The engine accumulates errors rather than bailing on the first failure, enabling the compiler to report multiple issues in a single pass:

```rust
pub struct TypeCheckError {
    pub kind: TypeErrorKind,
    pub span: Span,
    pub context: ErrorContext,
    pub severity: Severity,
    pub suggestions: Vec<Suggestion>,
}
```

The `ErrorContext` tracks the origin of type expectations — "2nd argument to `foo`", "return type of function", "condition of if-expression" — giving users actionable error messages that explain *why* a type was expected, not just that it was wrong.

When a type error occurs, the engine records it and assigns the `Error` type to the failing expression. The `Error` type unifies with everything (like `Never`), preventing cascading errors: one genuine mistake doesn't trigger dozens of downstream "mismatched types" errors.

## Prior Art

### Damas-Milner / Algorithm W

The [Damas-Milner paper](https://dl.acm.org/doi/10.1145/582153.582176) (1982) established the theoretical foundation. Algorithm W is a syntax-directed algorithm: for each syntactic form, there's a rule that computes the type. Ori's expression dispatch table is a direct descendant of Algorithm W's inference rules, adapted for a richer expression language.

### Pierce and Turner — Bidirectional Checking

["Local Type Inference"](https://www.cis.upenn.edu/~bcpierce/papers/lti-toplas.pdf) by Pierce and Turner (2000) introduced the synthesis/checking distinction that Ori uses. Their key insight was that many expressions have a "natural" direction: literals synthesize, function arguments check. Combining both directions gives better error messages and handles cases that pure synthesis cannot.

### Oleg Kiselyov — Efficient Generalization

Oleg Kiselyov's work on [efficient generalization](http://okmij.org/ftp/ML/generalization.html) describes the rank-based approach to let-polymorphism that Ori implements. The key insight is that tracking scope depth via ranks avoids the need to explicitly collect and compare free variables during generalization — a variable can be generalized if its rank matches the current scope depth.

### Elm and Roc — Error-Focused Inference

[Elm](https://github.com/elm/compiler)'s type checker prioritizes error message quality, tracking rich context about *why* types were expected. [Roc](https://github.com/roc-lang/roc) extended this with type diffing — when two complex types don't unify, the error highlights only the differing parts. Ori's `ErrorContext` and suggestion system draw from both approaches.

## Design Tradeoffs

**Immediate solving vs. constraint generation.** Ori unifies constraints immediately during traversal rather than collecting them. This simplifies the implementation (no constraint store, no separate solver) and gives error locality (errors at the point of occurrence). The tradeoff is that error quality can depend on traversal order — a constraint-based system could choose the "best" error to report from the full constraint set.

**Fresh engine per function vs. shared state.** Creating a fresh `InferEngine` for each function body adds allocation overhead but simplifies state management and prevents accidental state leakage between functions. The per-function approach also makes the system more amenable to future parallelization.

**Bidirectional vs. pure synthesis.** Adding top-down checking increases implementation complexity (two modes instead of one) but significantly improves error messages and enables features like checking lambda parameters against known function types. The added complexity is well-justified by the UX improvement.

**Capability tracking in inference vs. separate pass.** Embedding capability checking in the inference engine means it shares the same traversal and error infrastructure, avoiding a separate pass. The tradeoff is that the inference engine is more complex (two capability sets to track) and capability errors are interleaved with type errors rather than reported separately.
