---
paths:
  - "**ori_types**"
  - "**typeck**"
---

# Type Checker Formal Ruleset

This document defines the **laws** of the Ori type checker — the algorithm that converts a parsed AST into a typed IR with every type resolved, every trait dispatched, and every capability accounted for. The spec (`docs/ori_lang/v2026/spec/`) defines **what** programs are well-typed; this document defines **how** the checker decides. If the code violates a rule stated here, the code has a bug.

**Relationship to other rulesets**: The parser (`parse.md`) produces the AST consumed here. The type representation (`types.md`) defines the pool, tags, flags, schemes, and registries the checker operates on. Evaluation (`ori_eval`), ARC analysis (`aims-rules.md`), and codegen (`codegen-rules.md`) consume the typed IR this checker emits. `types.md` is about the type representation; this document is about the algorithm that fills it in. Together they govern the `ori_types` crate.

**Relationship to compiler.md and impl-hygiene.md**: Those files are *operational* guides. This document is *normative* — what the checker must guarantee. When they conflict, this document is authoritative for checker-specific rules. Cross-crate hygiene principles (SSOT, no side logic, algorithmic DRY) apply unchanged.

**Scope**: This ruleset covers pipeline ordering, the `InferEngine` and `ModuleChecker`, unification, bidirectional checking, generalization and instantiation, expression typing per spec form, trait resolution and coherence, capability checking, control-flow / pattern / test / cfg typing, error recovery, and the diagnostic catalog. Type storage and pool invariants live in `types.md`. Diagnostic rendering lives in `ori_diagnostic` (`diagnostic.md`).

**Target-only rules**: Rules marked **(target-only)** describe the COMPLETE target system per the spec. The implementation may not have shipped them yet. The spec is authoritative; code divergences are bugs to file, not spec inaccuracies.

---

## Notation

- **SHALL** = mandatory requirement (violation = implementation bug)
- **SHOULD** = recommended practice (violation = design smell, may be justified)
- **Idx** / **Pool** / **Tag** / **TypeFlags** — defined in `types.md`
- **Γ** = type environment: a stack of scopes, each binding `Name → Idx`
- **ρ** = current rank (scope nesting depth, SC-2 in `types.md`)
- **Expected<T>** = checker-mode context with an expected type; `Infer` = inference-mode context
- **Synth(e) → T** = synthesis: infer type of `e` bottom-up
- **Check(e, T)** = checking: verify `e` has type `T` with `T` propagated inward
- Rules are numbered `CATEGORY-N`. Categories: `CK` (module checker / pipeline), `EN` (inference engine state), `UN` (unification), `BD` (bidirectional checking), `GN` (generalization & instantiation), `EX` (expression typing), `TR` (trait resolution), `CP` (capability checking), `CF` (control flow / patterns / cfg), `ER` (error recovery), `DI` (diagnostics catalog), `PC` (phase contracts), `SG` (scope / save-restore helpers), `SL` (Salsa), `TRG` (tracing). Registration-pass rules live in `types.md` under `TYPES:RG-*` — this crate reads, never re-registers.
- Cross-references: `types.md` rules prefixed with `TYPES:` (e.g., `TYPES:TI-2`), `parse.md` with `PARSE:`, `aims-rules.md` with `AIMS:`, `codegen-rules.md` with `CG:`, `impl-hygiene.md` with `HYG:`, spec clauses with `Spec:`

---

## §1 Module Check Pipeline

Type checking a module is a strictly-ordered multi-pass pipeline driven by `check_module_impl()`. Passes are organized into three groups — registration, signatures, bodies — followed by export finalization through `finish()` / `finish_with_pool()`. Passes within a group are ordered; no pass reads state that a later pass produces.

Source: `ori_types/src/check/api/mod.rs` (`check_module_impl`), `ori_types/src/check/mod.rs` (`ModuleChecker`, `finish`).

### CK-1 — Pass Order

`check_module_impl()` SHALL execute these passes in order. Each row names the pass, its source file, and the group it belongs to:

| # | Pass | Group | Purpose | Source |
|---|------|-------|---------|--------|
| 0a | `register_builtin_types` | Registration | Register prelude / builtin types (str, int, List, Option, …) | `check/registration/builtin_types.rs` |
| 0b | `register_user_types` | Registration | Register struct / sum / newtype declarations into `TypeRegistry` | `check/registration/user_types.rs` |
| 0c | `register_traits` + `register_impls` | Registration | Register trait defs and `impl` blocks; run coherence (`TR-2`) | `check/registration/traits.rs`, `check/registration/impls.rs` |
| 0d | `register_derived_impls` | Registration | Generate derived trait impls (`TR-8`) | `check/registration/derived.rs` |
| 0e | `register_consts` | Registration | Register module-level constants | `check/registration/consts.rs` |
| 1 | `collect_signatures` | Signatures | Collect function / method signatures; enforce `CK-4` no-Var | `check/signatures/mod.rs` |
| 2 | `check_function_bodies` | Bodies | Check top-level function bodies | `check/bodies/mod.rs` |
| 3 | `check_test_bodies` | Bodies | Check `@t tests @f` and `tests _` bodies (`CF-6`) | `check/bodies/mod.rs` |
| 4 | `check_impl_bodies` | Bodies | Check method bodies in `impl Type { @m }` and `impl Type: Trait { @m }` | `check/bodies/mod.rs` |
| 5 | `check_def_impl_bodies` | Bodies | Check method bodies in `def impl Trait { @m }` | `check/bodies/mod.rs` |
| — | `finish()` / `finish_with_pool()` | Export | Collect typed IR, diagnostics, registries into `TypeCheckResult` | `check/mod.rs` |

The Registration group (0a–0e) SHALL freeze the registries before Signatures begins; a later pass SHALL NOT add registry entries. The Signatures pass SHALL freeze signatures before Bodies begins. Each pass reads its predecessors' output through explicit accessors — no hidden mutation. Export finalization (`finish`) is invoked by the driver AFTER `check_module_impl` completes; it does not participate in type checking itself, only in packaging output.

Rationale: Freezing earlier groups turns "what a later pass sees" into a deterministic function of "what an earlier pass produced" — the foundation for Salsa caching and for reasoning about the checker's fixpoints. Splitting body-checking into four passes (functions / tests / impl methods / def-impl methods) lets each pass operate on a homogeneous input and keeps trace output grouped by the construct being checked.

### CK-2 — Rank Discipline

A rank counter (`ρ`, `TYPES:SC-2`) SHALL be pushed on entry into any polymorphic scope and popped on exit. Polymorphic scopes are:

- Function body (parameter list + body)
- Method body (including `self`)
- `let` binding RHS (for `let-polymorphism`)
- Scheme instantiation (ephemeral — see CK-3)

A variable allocated by `fresh_var()` at rank ρ SHALL be generalizable only at scopes with outer rank `< ρ`. Exit without pop is a soundness bug; pop without exit is a monomorphism bug.

Cross-reference: `TYPES:SC-2`, `GN-1`.

### CK-3 — Parser `Infer` → Fresh `Var`

Every AST type reference carrying `Tag::Infer` SHALL be replaced by a fresh unification variable (`Tag::Var`) on entry into the checker. The replacement happens once, at signature-collection or body-entry time, before any unification runs.

Rationale: The checker operates on `Var`s; `Infer` is the parser's placeholder. Mixing them is a `TYPES:TK-5` violation.

### CK-4 — Signature Contract at Bodies Entry

By end of the Signatures pass (#1), every function / method signature SHALL carry pool-resolved `Idx` values for each parameter type and the return type. Signatures SHALL NOT contain `Tag::Infer` (parser placeholders for user-elided annotations — these are replaced by fresh `Tag::Var`s at signature-collection time per `CK-3`), unnormalized `Tag::Projection`, or unresolved `Tag::Named` references.

Fresh `Tag::Var`s for unannotated parameters or unannotated return types ARE permitted in `FunctionSig`; they are the checker-owned unification targets that the Bodies group resolves. Source: `compiler/ori_types/src/check/signatures/mod.rs` (`collect_signatures`) allocates a fresh `Var` for any parameter without an annotation. The Bodies pass for that function then unifies those fresh `Var`s against call-site and body constraints before emission.

A signature reaching the Bodies group with `Tag::Infer`, `Tag::Projection`, or unresolved `Tag::Named` is a `PC-2` (output contract) violation of the signatures sub-phase. A signature carrying a fresh `Tag::Var` is NOT — that is the normal hand-off shape.

Rationale: Body-checking relies on knowing call-site types. Unresolved signatures would create an ordering dependency between bodies, making the Bodies-group passes non-parallelizable and non-Salsa-cacheable.

### CK-5 — Coherence Check Location

Coherence (no-overlap of impls) SHALL be enforced during the Registration group, pass 0c, at registration time (`TR-5`). Coherence violations discovered later (in the Bodies group) are bugs in the `register_impls` pass, not new findings — the registration pass is the SSOT for impl coherence.

---

## §2 Inference Engine

The `InferEngine` holds all mutable state used by expression typing: the type pool, the type environment, unification state, rank counter, capability scope, `self` scope, and accumulated errors. It is a borrowed reference, not a clone — one engine per module-checking session.

Source: `ori_types/src/infer/mod.rs`, `ori_types/src/infer/env/`, `ori_types/src/infer/context.rs`.

### EN-1 — Engine Components

`InferEngine` SHALL own or borrow:

| Component | Role | Stored as |
|-----------|------|-----------|
| `Pool` | Type storage (`TYPES:§1`) | `&mut` borrow (one pool per engine lifetime) |
| `TypeEnv` | `Name → Idx` bindings, scoped | owned field |
| `UnifyEngine` | Union-find, fresh var allocation | owned field |
| `TraitRegistry` / `TypeRegistry` / signature map / const map | Registration + Signatures output | `&` borrows |
| `StringInterner` | `Name` interning | `&` borrow |
| Capability sets | current required / provided | owned fields |
| Self / impl-self types | `Self` scope | owned `Option<Idx>` stack |
| Loop break stack | inferred loop type | owned `Vec<Idx>` |
| Diagnostic accumulator | accumulated `TypeError` | owned field |

Rationale: Single-threaded, single-pool scope keeps all checker state on one borrow — no locking, no sharing, deterministic access order.

### EN-2 — No Cross-Module Engine Reuse

One `InferEngine` SHALL serve exactly one module-checking session. Reusing an engine across modules leaks state (loop break stack, capability sets, self types) and is forbidden.

Rationale: Registration + Signatures output is module-scoped. Mixing modules into one engine breaks Salsa caching per `HYG:§Salsa & Caching` (SL-3 in `types.md`).

### EN-3 — Fresh Variable Allocation

`fresh_var()` SHALL allocate a `Tag::Var` with rank equal to the current rank counter and append it to `var_states`. Allocation SHALL be deterministic — `fresh_var()` produces `Var(0), Var(1), …` in call order within a checking session.

Rationale: Deterministic allocation makes snapshot tests stable and makes pool output reproducible for Salsa.

### EN-4 — No Interior Mutability on Registry Borrows

Registry borrows (`TypeRegistry`, `TraitRegistry`, signature map) SHALL be immutable `&` references. The engine SHALL NOT mutate registries mid-checking. The Registration group (passes 0a–0e) freezes registries; the Bodies group (passes 2–5) may only read.

Cross-reference: `TYPES:RG-1`, `TYPES:RG-2`.

---

## §3 Unification

Unification is the core operation that turns constraints (`a ≡ b`) into bindings (`Var i := Idx j`). It is implemented as union-find over variable states with occurs checking, path compression, and tag-aware sub-unification.

Source: `ori_types/src/unify/mod.rs`, `ori_types/src/unify/substitute.rs`, `ori_types/src/unify/error/`, `ori_types/src/unify/rank/`.

### UN-1 — Unification Contract

`unify(a, b)` SHALL produce one of:

- `Ok(())` — `a` and `b` are now equivalent; all relevant vars are bound
- `Err(TypeError::Mismatch { ... })` — a concrete-vs-concrete conflict; the error is accumulated (ER-1) and the types are left as-is
- `Err(TypeError::Infinite { ... })` — occurs-check failure (UN-5); types left as-is

Unification SHALL NOT have side effects beyond pool reads, variable binding, and diagnostic accumulation.

### UN-2 — Structural Unification

Concrete-vs-concrete unification SHALL proceed by tag:

- **Same tag, same data** → trivially equal (via `TYPES:TI-2`), return `Ok`.
- **Same tag, different data** → recurse on children per the tag's extra layout (`TYPES:TY-4`).
- **Different tag, both concrete** → `E2001` type mismatch.

Cross-tag exceptions: `Alias` transparently unwraps (`TYPES:TK-9`). `Never` unifies with anything (UN-3). `Error` unifies with anything (UN-4).

Rationale: Tag-driven dispatch reuses the pool's existing structural vocabulary. Adding a new tag requires updating this section's coverage.

### UN-3 — Never Absorbs

`Tag::Never` SHALL unify with any type `T` by binding direction-preserving: `Never ⊑ T`. Use sites expecting `T` accept `Never` (`break`, `panic()`, infinite `loop`). Use sites expecting `Never` accept only `Never` or a producer (`panic()`, `todo()`, `unreachable()`).

Cross-reference: `TYPES:TK-4`.

Spec: Clause 8.1.1 (Never Semantics).

### UN-4 — Error Absorbs

`Tag::Error` SHALL unify with any type and SHALL NOT emit a diagnostic. `Error` is poison per `HYG:§Error Recovery Monotonicity`. Any operation on an error-typed subexpression silently propagates the error — no cascading diagnostics.

Cross-reference: `TYPES:TK-3`.

### UN-5 — Occurs Check

Unifying `Var(α)` with any type `T` SHALL perform the occurs check: if `α` appears transitively in `T`, reject with `E2008` (infinite type). The check SHALL be gated by `TYPES:TF-5` — skip when `T` has `!HAS_VAR`.

Path compression during union-find SHALL NOT bypass the occurs check. Compressed chains still represent real bindings; compressing around a cycle is a soundness bug.

### UN-6 — Rigid Variable Rejection

Unifying `Tag::RigidVar(α)` with a non-variable type SHALL fail with `E2003` (via `TypeErrorKind::RigidMismatch`; see `message.rs` mapping at `compiler/ori_types/src/type_error/check_error/message.rs`). Rigid variables are parametric — they cannot be narrowed to a concrete type, only to themselves or another rigid variable of matching identity.

Two distinct rigid variables SHALL NOT unify with each other. A `RigidVar(α)` and a `RigidVar(β)` where `α != β` is a `E2003` `RigidMismatch` (parametricity violation).

Cross-reference: `TYPES:TK-6`.

### UN-7 — Var-to-Var Union-Find

Unifying two `Var`s SHALL use rank-weighted union-find: bind the higher-rank var to the lower-rank var (the lower-rank is closer to the outer scope and safer to keep as the root). Path compression SHALL follow standard union-find amortization.

Rationale: Rank-weighted union keeps later generalization correct (CK-2) by biasing binding direction toward the outer, shared representative.

### UN-8 — Projection Normalization

Before unifying a type containing `Tag::Projection`, the checker SHALL attempt to normalize the projection: if the receiver's concrete type has a registered trait impl with the relevant associated type, replace the projection with the impl's binding. Unresolved projections at unification time are kept symbolic and carried forward; they SHALL be resolved by end of the Bodies group or rejected with `E2003`.

Cross-reference: `TYPES:TK-8`.

---

## §4 Bidirectional Checking

Expressions are typed in one of two modes: `Synth` (bottom-up inference) or `Check` (top-down against an expected type). The mode determines the error-message direction and enables expected-type propagation into subexpressions.

Source: `ori_types/src/infer/context.rs` (`Expected`, `ExpectedOrigin`), `ori_types/src/infer/expr/`.

### BD-1 — Mode Selection

For each expression form, the rule set specifies whether it is **naturally synth** (inferable without context) or **naturally check** (benefits from an expected type). The driver SHALL pick the mode based on the call site:

- Function argument with known signature → `Check(param_ty)`
- Function return with declared return type → `Check(return_ty)`
- `let x: T = e` → `Check(T)` on `e`
- `let x = e` → `Synth(e)`, then conditionally generalize per `GN-3` Value Restriction (only non-capturing lambdas)
- Operator operand with known other-operand type → `Check(other_ty)`
- Pattern expected by match scrutinee → `Check(scrutinee_ty)` on each arm
- No context available → `Synth`

### BD-2 — Expected Propagation

In `Check(T)` mode, the expected type SHALL propagate to structurally matching subexpressions:

- `Check(List<U>)` on `[e1, e2, ...]` → each `ei` checked with `Check(U)`
- `Check(Tuple<T1, T2>)` on `(a, b)` → `a` with `Check(T1)`, `b` with `Check(T2)`
- `Check((P) -> R)` on a lambda → parameters typed `Check(P)`, body typed `Check(R)`
- `Check(Option<U>)` on `Some(x)` → `x` checked with `Check(U)`
- `Check(Result<U, E>)` on `Ok(x)` → `x` checked with `Check(U)` (analogously for `Err`)
- `Check(Map<K, V>)` on `{k: v, ...}` → keys checked with `Check(K)`, values with `Check(V)`

Propagation stops at opaque constructs (named types without extractable parameter info, functions) where the expected is matched in aggregate.

### BD-3 — Synth then Subsume

When a subexpression is naturally synth inside a `Check(T)` context, the driver SHALL synthesize the subexpression's type `U` and then unify `U ≡ T`. Subsumption failure produces `E2001` with the expected type as "expected" and the synthesized as "got".

Rationale: Synth-then-subsume gives the correct error direction — the expected type is "what the context demands", the synthesized is "what the subexpression produces".

### BD-4 — ExpectedOrigin Carries Blame

The `Expected<T>` value SHALL be paired with an `ExpectedOrigin` that records WHY the type is expected — annotation, return type, parameter, match scrutinee, operator operand, etc. Diagnostics SHALL use `ExpectedOrigin` to produce imperative, context-aware messages.

Cross-reference: `HYG:§Error Handling — Expected context`.

### BD-5 — Mode Discipline

Mixing `Synth` and `Check` within one expression typing function without an explicit switch is a correctness risk. Each case of each expression form SHALL declare its mode. A function that ambiguously mixes modes inverts the error-direction convention (BD-3) and is a correctness bug.

---

## §5 Generalization & Instantiation

Let-polymorphism lets bindings quantify over their free type variables. Generalization turns a monotype into a scheme at scope exit; instantiation turns a scheme back into a monotype at each use site with fresh variables.

Source: `ori_types/src/unify/generalization.rs`, `ori_types/src/pool/substitute/`.

### GN-1 — Generalization Rule

At the exit of a polymorphic scope (function body return, `let` RHS binding, method body), the checker SHALL:

1. Resolve the type being generalized via `Pool::resolve_fully`.
2. Collect free `Var`s with rank strictly greater than the current outer rank.
3. Check that each collected var does not escape through the environment (if it does, it is not generalizable — this prevents skolem escape).
4. Construct a `Tag::Scheme` with one `BoundVar` per collected `Var`, substituting in the body.

Under-generalization (collecting too few vars) produces monomorphic bindings where polymorphism was intended. Over-generalization (collecting vars that escape) is unsoundness — a skolem escaping its binder.

Cross-reference: `TYPES:SC-1`, `TYPES:SC-2`.

### GN-2 — Instantiation at Use Site

Every reference to a scheme `∀α. body` SHALL instantiate the scheme at the use site:

1. Allocate one fresh `Var` per bound var (at the current rank).
2. Substitute each `BoundVar(α_i)` in the body with the corresponding fresh `Var`.
3. Return the instantiated body as the type of the reference.

Instantiation SHALL NOT mutate the scheme — schemes are immutable pool entries. The fresh vars live in the use site's rank.

### GN-3 — Value Restriction

Ori applies an AST-based Value Restriction to local let bindings: only direct non-capturing lambda initializers (`x -> x`, `(a, b) -> a + b`) are generalizable. All other initializers — empty containers (`[]`, `{}`), block-wrapped lambdas (`{ x -> x }`), variable aliases (`let alias = id`), conditionals, function calls, struct literals, constants — are monomorphic and SHALL NOT have their type variables generalized.

The restriction is enforced by `should_generalize(arena: &ExprArena, init: ExprId) -> bool` in `compiler/ori_types/src/infer/expr/blocks.rs` — the SSOT for the generalization decision. All three let-binding generalization sites (`infer_block` block-statement let, `infer_let` ExprKind::Let, `infer_try_stmt` try-block let) SHALL call this function rather than inlining equivalent logic.

This matches Rust's approach (no let-polymorphism for locals) while preserving the most common polymorphic use case. The choice is AST-based rather than type-based because type-tag heuristics fail when the resolved type is still `Tag::Var` awaiting bidirectional unification.

### GN-4 — Rank Restoration

On scope exit, the checker SHALL restore the pre-entry rank. Failure to restore produces spurious generalization in unrelated expressions. The restoration SHALL be enforced by matching `enter_rank_scope` / `exit_rank_scope` calls per `SG-5` (and the dual `UnifyEngine::enter_scope` / `exit_scope` pair). These are manually-paired helpers, NOT RAII guards — a dangling push is a monomorphism bug; a dangling pop is a skolem-escape bug per `GN-1`.

---

## §6 Expression Typing

Every expression form in `docs/ori_lang/v2026/spec/14-expressions.md` SHALL have a corresponding typing rule. This section summarizes the rules and anchors each to the spec clause and the source file that implements it.

Source: `ori_types/src/infer/expr/` (per-form files).

### EX-1 — Literal Expressions

| Literal | Type | Source |
|---------|------|--------|
| integer literal | `int` | `infer/expr/sequences.rs` |
| float literal | `float` | same |
| `true` / `false` | `bool` | same |
| string literal `"..."` | `str` | same |
| char literal `'c'` | `char` | same |
| byte literal `b'x'` | `byte` | same |
| duration literal `1s` | `Duration` | same |
| size literal `1kb` | `Size` | same |
| interpolated string `` `{x}` `` (no format spec) | `str`; requires `T: Printable` on each interpolant | checked via `resolve_builtin_method` / `TraitRegistry` (TR-1, `TYPES:RG-3`) |
| interpolated string `` `{x:spec}` `` (format spec present) | `str`; requires `T: Formattable` on the interpolant — validates the spec per Spec Clause 14.18 / Annex D | produces `E2034` on invalid spec, `E2035` on spec/type mismatch, `E2038` on missing `Printable` for the no-spec form |

Spec: Clause 14.2.

### EX-2 — Arithmetic, Bitwise, Comparison Operators

Binary operator expressions SHALL desugar to trait method calls per `spec/operator-rules.md`:

- `a + b` → `Add::add(a, b)` (trait method from the registry — `TYPES:BI-4`)
- `a == b` → `Eq::equals(a, b)`
- `a < b` → `Comparable::compare(a, b).is_less()`
- `a & b` → `BitAnd::bit_and(a, b)` (on operator-supporting types)

The checker SHALL NOT hardcode "if type == int do X else if type == float do Y" — all operator dispatch goes through the registry (`TYPES:RG-4`).

Shift count overflow (`1 << 63`, negative count) produces `E2020` (unsupported operator) at static-analysis time when the count is constant; runtime counts panic per spec.

Spec: Clause 14.3, `spec/operator-rules.md`.

### EX-3 — Call Expressions

`f(x, y)` SHALL type as:

1. Synthesize `f`'s type. If it's a scheme, instantiate (`GN-2`).
2. Require `f` to be a function type `(P1, P2, ...) -> R uses Caps`.
3. Check each argument against its parameter type (`Check(Pi)`).
4. Verify capability set (`CP-2`).
5. Result type is `R`.

**(Target-only)** Spec Clauses 10 / 14.5 specify named-argument matching by parameter name (`f(x: 1, y: 2)`, order independent), argument punning (`f(x:)` = `f(x: x)`), variadic collection (`...int` collects remaining positional arguments into `[T]`), and spread (`sum(...list)`). The shipped call checker validates arguments positionally (`compiler/ori_types/src/infer/expr/calls/call_inference.rs`, `compiler/ori_types/src/infer/expr/calls/method_call.rs`); argument names flow into diagnostics but not into matching logic. The AST preserves `Param.is_variadic` and `CallArg.is_spread` fields for the parser's benefit, but `ori_types` does not yet consume them during inference.

Spec: Clauses 10, 14.5.

### EX-4 — Method Calls

`x.m(args)` SHALL resolve through the split method-lookup surface defined in `TYPES:RG-3`:

1. Compute receiver type `Tx`.
2. Try **builtin** resolution first via `resolve_builtin_method()` consulting `ori_registry::BUILTIN_TYPES`. On hit, return the builtin method's return type.
3. On miss, consult `TraitRegistry::lookup_method()` for **inherent** impls (`impl Tx { @m }`) first, then **trait** impls (`impl Tx: Trait { @m }`).
4. On trait-method hit, verify `Tx` satisfies the trait's bounds (`TR-4`).
5. Instantiate the method's scheme (if generic).
6. Check each argument as in `EX-3`.
7. Result type is the resolved method's return type.

Aggregate resolution order is therefore **builtin → inherent → trait**, identical across every call site and matching `TYPES:RG-3`. Ambiguity between multiple matching trait impls produces `E2023` (ambiguous method); user disambiguates with `Trait.method(x, args)`.

Extension dispatch (`extend T { @m }`) is **(target-only)** — extension registration has not yet been threaded through `ori_types::registry`, so no extension-method path executes today.

Spec: Clause 14.6.

### EX-5 — Block Expressions

`{ s1; s2; e }` SHALL type as:

1. Check each statement `si` in sequence (each gets its own scope slot; no shadowing across statements unless the source shadows explicitly).
2. If the block ends with an expression `e` (no trailing `;`), the block's type is `e`'s type.
3. If the block ends with `;` (all statements), the block's type is `()` (unit).

An empty block `{}` is the empty map literal, not an empty block (disambiguated by context). A block with only `;`s is void.

Spec: Clause 11 (blocks and scope), Clause 14.4.

### EX-6 — Conditional Expressions

`if c then t else e` SHALL:

1. Check `c` with `Check(bool)`.
2. Synthesize `t` and `e`; unify them to a common type.
3. Block a `then`-only form (no `else`) from producing a non-unit value — `if c then e` with no `else` SHALL have type `()`.

`NO_STRUCT_LIT` context flag per `PARSE:CF-3` is already in force during condition parsing; the checker does not re-enforce.

Spec: Clause 16 (control flow).

### EX-7 — Match Expressions

`match scrutinee { P1 -> e1, P2 -> e2, ... }` SHALL:

1. Synthesize `scrutinee`'s type `Ts`.
2. For each arm: check pattern `Pi` against `Ts` (binding new scope vars), then check `ei` with the expected type (propagated if available).
3. Unify all arm expressions to a common result type.
4. Check exhaustiveness (`CF-3`).

Guards (`P if cond`) narrow the arm's reachability but do NOT narrow exhaustiveness — a match with only guarded arms requires a `_` catch-all (`E2001` if missing).

Spec: Clauses 15 (patterns), 16.

### EX-8 — Let Binding

`let x = e` SHALL:

1. Enter a rank scope (`CK-2`, push rank).
2. Synthesize `e`'s type `Te`.
3. If `should_generalize(arena, e)` returns `true` (per `GN-3` Value Restriction — only direct non-capturing lambdas qualify), generalize `Te` to a scheme (`GN-1`) while still at the elevated rank. Otherwise, `Te` remains a monotype.
4. Exit the rank scope.
5. Bind `x : scheme_or_monotype` in the environment.

`let x: T = e` SHALL check `e` against `T`, then bind `x : T`. (`T` may itself be generic; bound vars in `T` are rigid.)

`let $x = e` (immutable) is identical to `let x = e` in type checking; mutability is tracked separately for assignment checks.

Spec: Clause 12 (constants), Clause 13 (variables).

### EX-9 — Pattern Bindings in Let

`let { x, y } = p` (struct destructure) SHALL check that `p` has a struct type with at least fields `x`, `y`, then bind each destructured name to its field's type. Tuple, list, and nested-pattern destructures follow analogous rules.

Refutable patterns in `let` SHALL be rejected — `let Some(x) = option` is `E2040` (unsupported — destructure a sum type via `match`, not `let`).

Spec: Clause 15.

### EX-10 — Loop Expressions

Shipped loop typing covers `for` and `loop`:

- `for x in iter do body` — Synthesize `iter`'s type; require `Iterable`; bind `x : Iter.Item`; check body with expected `()`; result type is `()`.
- `for x in iter yield e` — analogous, but collect each yielded `e` into a result collection; result type is `[E]` (or `{K: V}` if `e` is `(K, V)`).
- `loop { body }` — Check body; allow `break value` to supply the loop's result type via the loop-break stack (`EN-1`); result type is inferred from `break value`s (or `Never` if none).

`for-do` SHALL NOT support `break value` (`E0860` from parse per `PARSE:DI` — the checker does not re-emit).

**(Target-only)** Spec defines `while c do body` — `Check(c, bool)`, body `Check(())`, result `()`, `break value` forbidden — and the parser reference (`PARSE:CF` target-only annotations) marks the `while` production target-only. No `While` AST node or inference entry point exists in `ori_types` today; the shipped checker does not type `while` expressions.

Spec: Clause 16.

### EX-11 — Closure Expressions

`x -> e`, `(a, b) -> e`, `() -> e` SHALL:

1. Enter a rank scope.
2. If `Check((P) -> R)` context, type each parameter from `P` and check body with `Check(R)`.
3. Else (`Synth`), allocate fresh `Var` per parameter, synthesize body with those bindings.
4. Exit rank scope.
5. Capture analysis: every free variable in the body SHALL be captured by value (Ori's ARC semantics — no mutable references across closure boundaries).
6. Result type is `(P1, P2, ...) -> R uses CapsBody`.

Captured variables SHALL NOT include `self` unless the enclosing method binds `self` by value (standard method receiver).

Spec: Clause 14.7 (closures).

### EX-12 — Cast Expressions

Shipped behavior: `infer_cast` in `compiler/ori_types/src/infer/expr/operators.rs` resolves the target type annotation and returns that type (for `e as T`) or `Option<T>` (for `e as? T`). The shipped checker does NOT validate that the cast is legal — the in-source comment "we don't check cast validity here" documents the absence. All casts type-check today regardless of whether the source/target pair is a lossless primitive coercion, a supported `As`/`TryAs` impl, or a nonsensical pair.

Implicit coercion SHALL NOT be inserted by the checker — casts are always explicit (`e as T` / `e as? T`).

**(Target-only)** Per Spec Clause 14.9 and Clause 8.11, full cast typing validates:

- `e as T` (infallible) — legal when the conversion is a spec-listed built-in (e.g. `int → float`, `char → int`, numeric widening) OR when a user-defined `As<T>` impl exists on `e`'s type. Otherwise `E2001` (type mismatch).
- `e as? T` (fallible) — legal when a built-in fallible conversion table entry exists (e.g. `int → byte` with range check, `str → int` via parsing) OR when a user-defined `TryAs<T>` impl exists. Result type is `Option<T>`.

The full built-in conversion table is in Spec Clause 14.9 subclauses. `As` and `TryAs` trait definitions are Spec Clause 8.11; their registration in `ori_registry` is pending.

Spec: Clause 14.9 (conversions), Clause 8.11 (conversion traits).

### EX-13 — Pipe Expressions (target-only)

**(Target-only)** Per Spec Clause 14.8 and the parser reference (`PARSE:` — `|>` is marked target-only and not yet lexed/parsed), the pipe operator SHALL desugar as follows:

- `x |> f(a: v)` — desugar to `{ let $_tmp = x; f(a: v, _pipe_slot: _tmp) }` where `_pipe_slot` is the function's first parameter that lacks both a value AND a default. Missing parameter is a diagnostic (error code to be assigned when the feature ships — reusing `E2040` or `E2027` is NOT correct; those codes are already bound to `UnsupportedFeature` and `AmbiguousIndex`). Multiple matching parameters is a separate diagnostic (likewise, code to be assigned at ship time).
- `x |> .method()` — desugar to `_tmp.method()` with the same temporary binding.
- `x |> (y -> e)` — apply the lambda to `x`.

No pipe production is shipped in the parser today, and `compiler/ori_types/src/infer/expr/mod.rs` has no pipe `ExprKind` arm — the entire EX-13 surface activates only when the parser ships `|>`.

Spec: Clause 14.8.

### EX-14 — Struct Literal and Update

`Point { x: 1, y: 2 }` SHALL:

1. Resolve `Point` to a registered struct type (`TYPES:RG-1`).
2. Verify every required field is supplied; extras produce `E2003`; missing produce `E2003`.
3. Check each supplied value against the field's declared type.

`{ ...existing, x: 10 }` (update) SHALL require `existing` to have a struct type; the supplied fields override, others inherit from `existing`.

Spec: Clause 14.10 (struct literals).

### EX-15 — Collection Literals

- `[1, 2, 3]` — element types unified; result `[T]`.
- `[...a, ...b]` — spread requires `a` and `b` to have list types with unifiable elements.
- `{k: v}` — keys unified; values unified; result `{K: V}`; `K` must satisfy `Hashable` (`E2031`).
- `{[expr]: v}` — computed-key form; `expr` typed as `K`.

**(Target-only — fixed-capacity)** Spec supports `[1, 2, 3]: [int, max 4]` fixed-capacity annotations; the spec-target typing is bidirectional expected-type propagation where `Check([int, max 4])` propagates `Check(int)` to each element and the capacity is checked at the literal. Today the shipped checker erases `[T, max N]` to `[T]` at type resolution (`TYPES:PT-2`), so a fixed-capacity-annotated literal is checked as an ordinary `[int]` literal; no capacity-aware bidirectional path is active. The target bidirectional path activates when fixed-capacity erasure is lifted.

Spec: Clause 14.11.

### EX-16 — Try Expressions

`e?` SHALL require `e` to have type `Result<T, E>` or `Option<T>`:

- `Result<T, E>?` — on `Err(e)`, propagate by short-circuit; on `Ok(x)`, result is `x`. The enclosing function's return type SHALL unify with a `Result<_, E>`.
- `Option<T>?` — on `None`, propagate; on `Some(x)`, result is `x`. Enclosing function SHALL return `Option<_>`.

Mixing `Result?` and `Option?` in one function is `E2001` at the second kind's use site.

Spec: Clause 17 (errors and panics).

### EX-17 — Assignment Statements

`x = v` SHALL type as `()` (void — the statement form per Spec Clause 16) and require `x` to be a mutable binding — immutable `let $x` rejects with `E2039` (assign to immutable). Compound assignment (`x += y`) desugars to `x = x + y` at parse time (`PARSE:PR-5`); the checker types the desugared form.

Index assignment (`list[i] = v`) SHALL desugar to `list = list.updated(key: i, value: v)`; field assignment (`state.f = v`) desugars to `state = { ...state, f: v }`. The root binding must be mutable.

Spec: Clause 14.12.

---

## §7 Trait Resolution

Trait resolution picks an `impl` at each method-call site, respecting coherence (no ambiguous impls) and specificity (more-specific impls win). The rules below are the checker's discipline; the registry (`TYPES:RG-2`) holds the impl table.

### TR-1 — Dispatch Priority

Method-call resolution SHALL follow this priority, identical to `TYPES:RG-3` and to the `resolve_builtin_method()` + `TraitRegistry::lookup_method()` two-path split:

1. **Builtin methods** — `resolve_builtin_method()` consulting `ori_registry::BUILTIN_TYPES` — highest priority. Builtin method resolution runs before any user-defined lookup.
2. **Inherent methods** — `impl T { @m }` blocks — consulted via `TraitRegistry::lookup_method()` after builtin miss.
3. **Trait methods** — `impl T: Trait { @m }` blocks — consulted within the same trait-registry call; ambiguity between concurrent trait impls is `E2023`.

A `Trait.method(receiver, args)` qualified form disambiguates across tiers.

**(Target-only)** Extensions (`extend T { @m }`) are a language-level feature not yet threaded into the type-checker's method-resolution surface. When shipped, they will sit at priority 4 (below trait impls).

### TR-2 — Coherence

For each `(Trait, ImplType)` pair, at most one impl SHALL be registered per module. Duplicate impls (two impls with identical keys) produce `E2010` (duplicate implementation / DuplicateImpl, checked at `CK-5`). Overlapping but non-duplicate impls (two impls whose keys are distinct but whose domains intersect — blanket vs specific without specificity ranking) produce `E2021` (OverlappingImpls).

Blanket impls (`impl<T: Bound> T: Trait`) SHALL NOT overlap with specific impls without an explicit specificity ranking — overlap produces `E2021`.

### TR-3 — Specificity

When multiple impls match a call site (e.g., a blanket and a specific), the more specific SHALL win. The shipped specificity model has three levels, stored as `ImplSpecificity::{Generic, Constrained, Concrete}` in `compiler/ori_types/src/registry/traits/mod.rs`:

1. **Concrete** — no type parameters and no `where` clause (most specific).
2. **Constrained** — type parameters present AND a `where` clause or bound constrains them.
3. **Generic** — type parameters present with no additional constraints (least specific).

Specificity is computed from `type_params.is_empty()` and `where_clause.is_empty()` at registration (`check/registration/impls.rs`); competing impls are resolved by taking the `max` of the three-level `ImplSpecificity`. Ties at the same level produce `E2021` (overlapping implementations).

**(Target-only)** Finer specificity distinctions — e.g., separating `Applied(T, concrete_args)` from fully-generic `Applied(T, type_params)`, or ordering bound-constrained parameters by bound count — are specified but not yet modeled. Finer ranking ships together with richer impl metadata.

### TR-4 — Bound Satisfaction

Every call to a method `m : (self, ...) -> R` where `Self: Trait + Bounds` SHALL verify that the receiver's type satisfies every bound. Failure is `E2001` with the unsatisfied bound as the expected-context origin.

Bound satisfaction is transitively resolved: if `T: Hashable`, then `T: Eq` (Hashable's supertrait) SHALL also hold, verified via the registered supertrait chain.

### TR-5 — Coherence at Registration Time

Coherence (TR-2) SHALL be enforced during the Registration group at pass 0c (`CK-1`, `CK-5`). A coherence violation discovered during body-checking is a registration bug — the registration pass failed to catch a conflict it should have caught at registration time.

### TR-6 — Object Safety Check

A trait used at a trait-object position (argument, return, field) SHALL be verified object-safe (`TYPES:BI-6`). Non-object-safe traits (`Clone`, `Eq`, `Iterator`, `Comparable`, `Hashable`) produce `E2024`. `Into<T>` IS object-safe — the generic parameter is on the trait, not the method; `into(self) -> T` has a `self` receiver and returns `T` (not `Self`); all three object-safety rules hold.

### TR-7 — Default Implementations

A trait method with a default body SHALL be inherited by every impl that doesn't override it. `def impl Trait { @m }` provides module-scoped default implementations for all types satisfying the trait; user `impl` bodies override.

A stateless `def impl` SHALL be unique per `(trait, module)` pair — multiple `def impl`s for the same trait are `E2022` (conflicting defaults).

### TR-8 — Derived Implementations

Derived trait impls (`#derive(Eq, Clone)` pre-proposal syntax; `type T: Eq, Clone = {...}` post-proposal) SHALL generate registered impls with canonical field-by-field semantics (`TYPES:BI-3`):

- `Eq` — all fields `Eq`; compare componentwise
- `Clone` — all fields `Clone`; clone componentwise
- `Debug` — all fields `Debug`; format as `TypeName { f1: v1, f2: v2 }`
- `Printable` — all fields `Printable`; format same as Debug without escaping
- `Default` — all fields `Default`; produce zero value
- `Comparable` — all fields `Comparable`; compare lexicographically (declaration order)
- `Hashable` — all fields `Hashable`; hash via `hash_combine` (FNV-1a base)

A missing field trait produces `E2032` (field missing trait in derive).

### TR-9 — Extensions (target-only)

**(Target-only)** Extensions (`extend T { @m }`) are defined in Spec Clause 8.8 but are not yet registered or dispatched by the shipped `ori_types` registry surface. When the feature ships, extensions will:

- Add `m` to `T`'s method surface at module-local priority 4 (below builtin / inherent / trait in TR-1).
- SHALL NOT add fields, SHALL NOT override existing methods, and SHALL NOT declare static methods on `T`.
- Cross traits via `extend<T: Bound> [T]` — valid only when the bound does not create impl ambiguity with pre-existing impls.

Spec: Clause 8.8 (extensions section).

---

## §8 Capability Checking

Ori's effect system is expressed via capabilities — named, structured effects (`Http`, `FileSystem`, `Print`, `Suspend`, …) declared in function signatures (`uses Http`) and satisfied at call sites via `with Http = handler in expr`.

Source: `ori_types/src/infer/mod.rs` (capability fields), `ori_types/src/infer/env/` (provided/required sets).

### CP-1 — Capability Set Propagation

Every function declaration SHALL carry a capability set (the `uses Cap1, Cap2` clause). In the shipped representation, the capability set lives on `FunctionSig.capabilities` as a metadata sidecar — the pool entry for `Tag::Function` stores only `[param_count, param_tys..., return_ty]` (`TYPES:TL-2`) with no capability payload. **(Target-only)** In-pool capability encoding and `TypeFlags::HAS_CAPABILITY` propagation (`TYPES:TF-1`, `TYPES:TF-3`) are specified but not yet shipped; when they do, compound types containing a function with non-empty capabilities will inherit the flag.

### CP-2 — Call-Site Capability Requirement

A call `f(...)` where `f : ... uses Caps` SHALL require that every capability in `Caps` is available at the call site. Availability is the set of capabilities provided by enclosing `with Cap = handler in { ... }` blocks plus the current function's own `uses` declaration.

Missing capabilities produce `E2014` (missing capability) with the unavailable cap names listed in the diagnostic.

### CP-3 — Handler Provision (shipped surface)

`with Cap = provider in expr` SHALL, as shipped:

1. Infer `provider`'s type (validating the provider expression).
2. Enter a child environment scope and bind the capability name to the provider type (so the body can reference `Cap` as an identifier).
3. Add `Cap` to the provided-capability set for the duration of `expr` via `with_provided_capability` (SG-3's inference-engine variant).
4. Type `expr`; the result is `expr`'s type — handlers do not wrap the result.
5. Exit the child scope.

Source: `ori_types/src/infer/expr/constructors.rs::infer_with_capability`.

**(Target-only)** Full spec semantics of `with Cap = handler in expr` additionally include:

- Verifying `handler` implements every trait method of `Cap` (trait-shape validation).
- Stateful handlers `with Cap = handler(state: init) { op: (s) -> (s', val), ... } in expr` where state replaces `self` and each operation has type `(S) -> (S', R)` (Spec Clause 20, errors E1204–E1207).

These checks are defined in Spec Clause 20 but are not yet performed by `infer_with_capability`.

Spec: Clause 20 (capabilities).

### CP-4 — Capset Expansion (target-only)

**(Target-only)** Spec Clause 20 defines capsets — `capset Net = Http, Dns, Tls` — as structural aliases expanded before type checking, and forbids `impl` or `with Net = handler` forms on capsets. `ori_types` does not yet contain a capset expansion pass; a `uses Net` clause today is parsed as a single capability name with no member resolution. Capset support is target-only until the expansion pass ships.

Spec: Clause 20 (capsets).

### CP-5 — Unsafe Capability (target-only)

**(Target-only)** Spec declares `uses Unsafe` as a marker capability that cannot be bound via `with...in` — its presence marks a function as using `unsafe { ... }` blocks, and propagation follows CP-1. The shipped checker treats `Unsafe` as a plain capability name with no special marker-rejection for `with Unsafe = ...` forms.

### CP-6 — Suspend Capability (target-only)

**(Target-only)** Spec mandates that non-`Suspend` functions SHALL NOT call `Suspend` functions directly, and that concurrency combinators (`parallel(tasks:)`, `nursery(body:)`) SHALL provide `Suspend` to their bodies. The shipped checker does not yet enforce this calling-context discipline, and the relevant function-expression kinds (`Parallel`, `Nursery`) are currently rejected with `E2040` as unsupported.

### CP-7 — Purity Inference (target-only)

**(Target-only)** Per the spec, a function with no `uses` clause and no calls to capability-requiring callees can be inferred pure and flagged `IS_PURE` (`TYPES:TF-1`). The `TypeFlags::IS_PURE` bit exists in the flags schema (`TYPES:TF-1`) but the shipped checker does not yet compute or set it — purity inference is target-only. When shipped, purity will be used by `aims-rules.md` (effect classification) and `codegen-rules.md` (`CG:AT-3` purity attributes).

### CP-8 — FFI Capability (target-only)

**(Target-only)** Spec Clause 26 defines `uses FFI` / `uses FFI("lib")` as required by functions that call into `extern "c"` / `extern "js"` blocks, with per-library capabilities (`FFI("sqlite3")`) distinct from the general `FFI` marker. The shipped checker does not yet enforce FFI capability requirements at extern-call sites; FFI capability support is target-only.

Spec: Clause 26 (FFI).

---

## §9 Control Flow / Patterns / Conditional Compilation

Language constructs whose typing depends on structure beyond the pure expression calculus.

### CF-1 — Pattern Typing

For a pattern `P` checked against type `T`:

- Literal `lit` — `T` SHALL unify with `lit`'s type.
- Binding `x` — introduces `x: T` in the arm scope.
- Wildcard `_` — always succeeds, no binding.
- Constructor `K(p1, ..., pn)` — `T` SHALL unify with the constructor's result; each `pi` checks against the corresponding field/payload type.
- Struct `{ x, y }` — `T` SHALL be a struct with at least `x`, `y` fields; binds `x`, `y` to those types.
- List `[p1, p2, ..rest]` — `T` SHALL unify with `[U]`; each `pi` checks against `U`; `rest` binds to `[U]`.
- Range `1..10` — `T` SHALL unify with `int` (Range patterns are int-only in shipped surface).
- Or `A | B` — both `A` and `B` typecheck against `T`; bindings in both sides SHALL have identical names and types.
- At `x @ pat` — `pat` checks against `T`; `x` binds to `T` in addition.
- Guard `pat if cond` — `cond` SHALL have type `bool`; the arm body sees bindings introduced by `pat`.

Spec: Clause 15.

### CF-2 — Variant Punning

`Some(value:)` SHALL desugar to `Some(value: value)` when `value` is a simple-name pattern. Analogous for struct patterns.

Spec: Clause 15.3.

### CF-3 — Exhaustiveness

A `match` expression SHALL be exhaustive: the union of its arms' patterns SHALL cover every possible value of the scrutinee's type. Non-exhaustiveness produces `E2001` with the uncovered pattern listed.

`let <pat> = expr` SHALL require an irrefutable pattern — patterns that could fail at runtime are rejected (use `match` instead).

Spec: Clause 15.4, Clause 16.

### CF-4 — Guards and Exhaustiveness

Guards do NOT contribute to exhaustiveness coverage. A guarded arm `P if cond -> e` is treated as if `cond` could fail, so the exhaustiveness checker considers the arm unreachable under that guard. A match with only guarded arms SHALL require a `_` catch-all.

### CF-5 — Conditional Compilation (driver-layer pruning)

Per Spec Clause 25, `#target(os: "linux")` and `#cfg(debug)` attributes at declaration sites SHALL gate the declaration at the parse/driver boundary: unsatisfied branches are dropped before type checking begins; satisfied branches enter the normal `ori_types` pipeline. The `ori_types` checker SHALL assume its input `Module` has already been filtered — it does NOT walk `target_attr` / `cfg_attr` fields during any `CK-1` pass.

**(Target-only)** The driver-layer / parser-layer pruning pass that removes unsatisfied branches is not yet wired into the shipped `ori_types` entry path (`compiler/ori_types/src/check/api/mod.rs` walks `module.types`, `module.impls`, `module.functions`, `module.tests` unconditionally). When it ships, pruning runs before the Registration group (pass 0a) and is transparent to every rule below.

The `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release` constants SHALL have type `str` or `bool` as appropriate and SHALL be compile-time foldable (ownership: const-evaluator / driver, not `ori_types`).

Spec: Clause 25 (conditional compilation).

### CF-6 — Test Declarations

`@t tests @fn () -> void` (attached test) and `tests _` (floating test) SHALL be checked as normal function bodies but tagged in the typed IR for test-harness consumption (`check/bodies/mod.rs` `check_test_bodies`, pass 3 per `CK-1`).

`#skip("reason")` SHALL NOT suppress type errors — only type-correct bodies are skippable.

`#compile_fail("expected_error")` expectation matching is owned by the test harness (`oric`'s test runner), not the `ori_types` checker. Per Spec Clause 19, a `#compile_fail` test passes when compilation fails with a diagnostic whose message contains the requested substring. `ori_types` type-checks the test body as any normal body; failure-expectation comparison happens at the harness layer that consumes the checker's emitted diagnostics.

Spec: Clause 19 (testing).

### CF-7 — Contracts

`pre(cond)` / `post(result -> cond)` on a function declaration SHALL be checked as `bool`-typed expressions with access to the function's parameters (for `pre`) or parameters plus bound result name (for `post`).

Spec: Clause 10 (declarations, contract clauses).

---

## §10 Error Recovery

Type errors SHALL NOT stop the check. The checker accumulates every error it can surface per pass, under the constraint that recovery is *monotonic*: recovering from error A does not manufacture new errors downstream.

Source: `ori_types/src/type_error/`, `ori_types/src/reporting/`.

### ER-1 — Accumulate, Don't Bail

Every typing function SHALL accumulate errors into the engine's diagnostic accumulator and continue with a poison type (`Tag::Error`) when recovery is needed. A single typing function SHALL NOT return early on the first error.

Exception: `debug_assert!` violations of phase contracts (`PC-2`) — those are internal compiler errors, not user errors, and may propagate a hard panic in debug builds.

### ER-2 — TyError as Poison

`Tag::Error` unifies with everything (UN-4) without emitting a new diagnostic. Any further type checking on a subexpression that produced `Tag::Error` SHALL silently propagate the error type without adding to the diagnostic accumulator.

### ER-3 — Error Nodes Are Terminal

AST nodes marked with a parser error marker (`PARSE:DI-*`) SHALL be skipped by the checker with no cascading errors. The checker does not re-diagnose syntactic issues.

### ER-4 — Follow-On Suppression

If an error at span `S` produces `Tag::Error`, subsequent errors at child spans involving `Tag::Error` SHALL be suppressed. Users see the root cause, not the cascade.

Cross-reference: `HYG:§Aspirational Patterns — Deduplication by (Code, Span) with Follow-On Suppression`.

### ER-5 — Diagnostic Dedup

Diagnostics SHALL be deduplicated by `(error_code, primary_span)` before rendering. The same logical error discovered through two paths produces one diagnostic.

### ER-6 — Similarity Suggestions

Unknown-name diagnostics (`E2003`) SHALL compute Damerau-Levenshtein distance to in-scope names with threshold `distance ≤ min(name.len() - 1, max(2, name.len() / 3))` and emit "did you mean?" suggestions.

---

## §11 Diagnostic Catalog

The checker's diagnostic codes are stable identifiers — their numeric identity is part of the public API and SHALL NOT be renumbered. Ranges: `E2000..E2099` = type/semantic errors.

Source: `ori_types/src/type_error/check_error/`.

### DI-1 — Error Code Table

| Code | Kind | Meaning | Source variant |
|------|------|---------|----------------|
| E2001 | Error | Type mismatch (including failed unification, pattern mismatch, non-exhaustive match, unsatisfied bounds) | `TypeErrorKind::Mismatch`, `PatternMismatch`, `NonExhaustiveMatch`, `UnsatisfiedBound` |
| E2003 | Error | Unknown identifier / undefined field / rigid mismatch / import error / not-a-struct | `UnknownIdent`, `UndefinedField`, `RigidMismatch`, `ImportError`, `NotAStruct` |
| E2004 | Error | Arity mismatch (call expects N args, got M) / missing struct fields | `ArityMismatch`, `MissingFields` |
| E2005 | Error | Ambiguous type (inference stuck) | `AmbiguousType` |
| E2008 | Error | Infinite type (occurs check failure) | `InfiniteType` |
| E2010 | Error | Duplicate impl / duplicate field / missing associated type | `DuplicateImpl`, `DuplicateField`, `MissingAssocType` |
| E2014 | Error | Missing capability | `MissingCapability` |
| E2019 | Error | Uninhabited struct field (`Never` as field) | `UninhabitedStructField` |
| E2020 | Error | Unsupported operator for type | `UnsupportedOperator` |
| E2021 | Error | Overlapping implementations | `OverlappingImpls` |
| E2022 | Error | Conflicting default implementations | `ConflictingDefaults` |
| E2023 | Error | Ambiguous method | `AmbiguousMethod` |
| E2024 | Error | Trait not object-safe | `NotObjectSafe` |
| E2025 | Error | Type not indexable | `NotIndexable` |
| E2026 | Error | Index key type mismatch | `IndexKeyMismatch` |
| E2027 | Error | Ambiguous index | `AmbiguousIndex` |
| E2028 | Error | Cannot derive Default for sum type | `CannotDeriveForSumType` |
| E2029 | Error | Cannot derive trait without required supertrait | `CannotDeriveWithoutSupertrait` |
| E2030 | Warning | Hash invariant violation (Hashable/Eq skew) | `HashInvariantViolation` |
| E2031 | Error | Non-hashable map key type | `NonHashableMapKey` |
| E2032 | Error | Field missing required trait in derive | `FieldMissingTraitInDerive` |
| E2033 | Error | Trait not derivable — emitted by the derive-registration path for traits that cannot be derived (markers `Sendable`/`Value`, intent-dependent traits `Iterator`/`Into`/`Drop`/`Iterable`) | `TraitNotDerivable` |
| E2034 | Error | Invalid format specification | `InvalidFormatSpec` |
| E2035 | Error | Format type mismatch | `FormatTypeMismatch` |
| E2036 | Error | `Into<T>` not implemented | `IntoNotImplemented` |
| E2037 | Error | Ambiguous `Into<T>` conversion | `AmbiguousInto` |
| E2038 | Error | Missing `Printable` for string interpolation | `MissingPrintable` |
| E2039 | Error | Cannot assign to immutable binding | `AssignToImmutable` |
| E2040 | Error | Feature not yet supported | `UnsupportedFeature` |
| E2041 | Error | Invalid `#repr` attribute | `InvalidReprAttribute` |

New error codes SHALL be appended in order. Renumbering or reuse is forbidden by `HYG:§Error Handling — Error codes are stable API`.

### DI-2 — Imperative Suggestions

Every diagnostic SHALL either include a concrete fix ("try changing `int` to `float`") or a `// SAFETY:`-style rationale. No "unexpected X" without "expected Y because Z".

### DI-3 — Warnings vs Errors

Warnings (E2030 currently) SHALL NOT suppress code generation. Errors SHALL prevent codegen (`PC-4`). The warning-vs-error classification SHALL be attached to the `ErrorCode`, not computed dynamically.

---

## §12 Phase Contracts

The checker's input and output invariants are the contract with surrounding phases. Violations are silent miscompilations unless validated.

### PC-1 — Input Contract

On entry, the AST SHALL satisfy `PARSE:DD-*`:

- All tokens consumed; no residual parser state
- Error nodes present only where recovery happened; marked with error flag
- `TypeId::INFER` permitted; other `TypeId`s either pre-interned primitives or pending-resolution nominal refs
- Spans present on every node

### PC-2 — Output Contract (to Eval / ARC / Codegen)

On successful check, the typed IR SHALL satisfy (mirrors `TYPES:PC-2`):

- No `Tag::Var` in any type position
- No `Tag::Infer`
- No `Tag::Projection` (all normalized)
- No `Tag::SelfType` (all substituted)
- All `Tag::Named` resolved to `TypeRegistry` entries
- All method calls resolved statically via builtin-first `resolve_builtin_method()` or `TraitRegistry::lookup_method()` — no runtime lookup remains (`TYPES:§RG-3`)
- All capability requirements satisfied at their use sites

Consumers SHALL `debug_assert!` these on entry. Release builds SHALL produce an internal compiler error (not silent miscompilation) on violation.

**Producer-side enforcement:** `validate_body_types` in `compiler/ori_types/src/check/validators/mod.rs` (re-exported at the crate root as `pub use check::validators::validate_body_types;`) walks the body's `expr_types` and `FunctionSig` (`param_types` + `return_type`) after `InferEngine` body-checking completes, identifies any surviving unbound `Tag::Var` per expression/signature position, and emits `E2005` (`AmbiguousType`) once per `ExprIndex`. The validator's gate order is `resolve_fully → HAS_ERROR → HAS_VAR` (`§ER-4` cascade suppression precedes `TYPES:§TF-5` fast-path) and the walk delegates compound-tag child traversal to the canonical `Pool::visit_children` helper (`compiler/ori_types/src/pool/descriptor.rs`, `TYPES:§TF-3` propagation + the `Tag::Scheme` fix) rather than cloning a parallel tag-dispatch ladder (`HYG:§Algorithmic DRY`). Without this producer-side enforcement the `PC-2` contract is only checked by consumer `debug_assert!`s, which strip in release and surface as silent miscompilation.

Cross-reference: `HYG:§Cross-Phase Invariant Contracts`.

### PC-3 — Error-Typed Output

On failed check, the typed IR is produced with `Tag::Error` at failure sites. Downstream phases SHALL skip error-typed nodes. This is NOT a contract violation — it is the documented error-recovery carrier.

### PC-4 — Codegen Gate

Codegen SHALL refuse to emit if any typeck error was produced (as opposed to warning). The gate lives in the driver (`oric`), not in the checker — the checker produces output regardless; the driver decides whether to feed it to codegen.

---

## §13 Salsa & Caching

The checker's tracked queries participate in Salsa's incremental computation. Determinism and input hygiene are non-negotiable.

### SL-1 — Tracked Query Purity

Every `#[salsa::tracked]` typeck query SHALL be pure: same inputs → same outputs. No global state, no clock reads, no env-var reads, no filesystem.

Cross-reference: `TYPES:SL-1`, `HYG:§Salsa & Caching`.

### SL-2 — Deterministic Var Allocation

`fresh_var()` (EN-3) SHALL produce the same sequence of var ids for the same module input. Non-determinism here breaks Salsa memoization — two runs of the same query produce different outputs.

### SL-3 — Error Accumulation

Errors SHALL flow through Salsa's diagnostic accumulator, not returned through `Result`. Each phase appends; consumers collect at the end.

### SL-4 — Registry Key Stability

Keys used by tracked queries (names, type ids, span-derived keys) SHALL have stable identity across revisions. Key identity changing while content stays the same forces re-execution and is a performance cliff (`HYG:§Salsa & Caching — Query-key stability`).

---

## §14 Scope / Save-Restore Helpers

Mutable scope context in `ModuleChecker` is managed by closure-based save/restore helpers. These are NOT RAII guard objects (no `Drop` impl); they are `with_*` methods that save prior state, invoke a closure, then restore state after the closure returns. Panic safety is NOT guaranteed — an unwinding panic inside the closure will NOT restore the saved state. Manual scope manipulation outside these helpers is forbidden.

Source: `ori_types/src/check/scope.rs` (module-checker helpers), `ori_types/src/infer/mod.rs` (inference-engine helpers), `ori_types/src/unify/mod.rs` (unify-engine scope).

### SG-1 — Function Scope

`ModuleChecker::with_function_scope<T, F>(self, fn_type: Idx, capabilities: FxHashSet<Name>, f: F) -> T` SHALL:

1. Save `current_function` and `current_capabilities`.
2. Set them to the supplied values.
3. Invoke `f(self)`.
4. After `f` returns, restore the saved values.
5. Return `f`'s result.

This helper manages ONLY the `current_function` and `current_capabilities` fields. It does NOT push a type-environment scope, does NOT push a rank, and does NOT bind parameters. Environment-scope push/pop lives in SG-4; rank push/pop lives in SG-5; parameter binding is the responsibility of the caller's body-checking entry point.

### SG-2 — Impl Scope

`ModuleChecker::with_impl_scope<T, F>(self, self_ty: Idx, f: F) -> T` SHALL save the prior `current_impl_self`, set it to `self_ty`, invoke `f`, restore the saved value, and return `f`'s result. `self` references inside `f` resolve to `self_ty`.

### SG-3 — Provided-Capabilities Scope

`ModuleChecker::with_provided_capabilities<T, F>(self, caps: FxHashSet<Name>, f: F) -> T` SHALL save the prior `provided_capabilities`, replace it with `caps`, invoke `f`, restore the saved set, and return `f`'s result. Used to implement `with Cap = handler in expr`.

### SG-4 — Environment Scope (InferEngine)

`InferEngine::enter_scope(self)` SHALL push a new scope onto the underlying `TypeEnv`; `InferEngine::exit_scope(self)` SHALL pop it. These calls SHALL match one-to-one. A push without a matching pop leaks bindings into the enclosing scope; a pop without a matching push is a scope-underflow bug.

This is NOT a closure-based helper — callers are responsible for manual pairing. Body-checking entry points typically enter a scope at the start and exit it on completion; the error-recovery rule (`ER-1`) SHALL NOT skip the matching `exit_scope` call when recovering from a nested error.

### SG-5 — Rank Scope (InferEngine + UnifyEngine)

`InferEngine::enter_rank_scope(self)` SHALL push the rank counter (`CK-2`); `InferEngine::exit_rank_scope(self)` SHALL pop it. The rank scope for unification lives separately at `UnifyEngine::enter_scope(self)` / `UnifyEngine::exit_scope(self)`, which tracks the rank of unification-variable introductions.

Calls SHALL match one-to-one across both layers. A dangling rank push is a monomorphism bug (over-eager generalization); a dangling rank pop is a soundness bug (skolem escape per `GN-1`).

---

## §15 Tracing

Source: `compiler/ori_types/src/lib.rs` (tracing target), `compiler/oric/src/tracing_setup.rs`.

### TRG-1 — Target

Checker events SHALL trace under target `ori_types`. Recommended:

- `ORI_LOG=ori_types=debug` — module-level phase boundaries, type errors
- `ORI_LOG=ori_types=trace ORI_LOG_TREE=1` — per-expression call tree

### TRG-2 — Phase Dump

`ORI_DUMP_AFTER_TYPECK=1` SHALL dump the typed IR to stderr after the checker exits. The dump format SHALL include every expression's inferred type, rendered with `Pool::format`.

### TRG-3 — Error-Path Tracing

Error construction SHALL emit `tracing::trace!` events at construction time. Error recovery (poisoning with `Tag::Error`) SHALL emit `tracing::warn!`. These allow post-mortem reconstruction of error paths without littering output with `println!`.

Cross-reference: `HYG:§Tracing & Logging`.

---

## §15.5 Graph-first, manual second

Before reading §16's reference-repo citations, query the intelligence graph:

- `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,koka,lean4,gleam --limit 5`
  — semantic equivalents across HM / bidirectional reference compilers
- `scripts/intel-query.sh --human callers "<symbol>" --repo ori` — blast radius
  for changes in this domain
- `scripts/intel-query.sh --human file-symbols "<path-fragment>" --repo ori` — the
  module inventory before editing
- `scripts/intel-query.sh --human ori-inference --limit 5` — pre-curated subsystem
  view for inference, unification, trait resolution, capability checking

The graph covers Ori plus 10 reference compilers, synced on every commit. Manual reference-repo reading
stays authoritative — but only AFTER the graph narrows the search. Never
cite a graph result without verifying against the actual source. See
`.claude/rules/intelligence.md` for the canonical when-to-query workflow and subcommand reference and
`.claude/skills/dual-tpr/compose-intel-summary.md` for the canonical
query protocol used by review-family skills.

## §16 Prior Art Cross-Reference

| System | Relevant Pattern | Ori Correspondence |
|--------|-----------------|-------------------|
| **Rust `rustc_hir_typeck`** | Bidirectional check + synth modes, expected-type propagation | BD-1..BD-5 |
| **Rust `rustc_trait_selection`** | Coherence at impl-registration, specificity ordering | TR-2, TR-3, CK-5 |
| **Rust `FnCtxt`** | Per-body context bundling env + inference state | `InferEngine` (EN-1) |
| **Koka `Type.Infer`** | HM-inference extended with effect (capability) typing | CP-1..CP-8 |
| **Gleam `compiler-core::analyse`** | Whole-module registration before body checking | CK-1 four-phase pipeline |
| **Swift `Sema`** | Request-based incremental type check | SL-1..SL-4 (Salsa analog) |
| **Zig `Sema.zig`** | Job queue per declaration | CK-1 explicit phase ordering (aspirational: explicit job list) |
| **Lean 4 `elabTerm`** | Bidirectional elaboration with expected types | BD-1..BD-5 |
| **Roc `check`** | Rank-based generalization, let-polymorphism | GN-1, CK-2 |
| **Elm `Reporting`** | Imperative diagnostic suggestions | DI-2 |

### Interface with types.md

| typeck.md rule | Uses types.md rule | Interface |
|----------------|--------------------|-----------|
| `UN-*` (unification) | `TYPES:TI-2`, `TYPES:TF-5`, `TYPES:SC-4` | Compares `Idx`, gates occurs check via flags |
| `GN-*` (generalization) | `TYPES:SC-1`, `TYPES:SC-2` | Scheme construction / rank tracking |
| `TR-*` (trait resolution) | `TYPES:RG-2`, `TYPES:RG-3` | Dispatches through registries |
| `CP-*` (capability checking) | `TYPES:TL-2`, `TYPES:TF-1 HAS_CAPABILITY` | Reads capability sets from function types |
| `PC-*` (phase contracts) | `TYPES:PC-1`, `TYPES:PC-2` | Output contract enforced here |
| `CK-*` (pipeline) | `TYPES:RG-1` (freeze after Registration group, passes 0a–0e) | Registry lifecycle contract |

### Interface with aims-rules.md

| aims-rules.md consumer | Produced by typeck.md rule | Data |
|------------------------|----------------------------|------|
| `AIMS:§5 IC-5` (EffectSummary, inline under `§5 Interprocedural Contracts`) | CP-1, CP-7 | Capability set + purity → effect base |
| `AIMS:§1.3 Cardinality` | EX-10 (for loops) | Loop structure informs once/many classification |
| `AIMS:§1.6 Shape` | EX-14, EX-15 | Struct / collection construction shape |
| `AIMS:§7 Pipeline` | PC-2 | Typed IR with resolved types is AIMS' input |

### Interface with codegen-rules.md

| codegen-rules.md consumer | Produced by typeck.md rule | Data |
|---------------------------|----------------------------|------|
| `CG:TR-2` (full resolution before translation) | PC-2 | No `Tag::Var` in typed IR |
| `CG:AT-3` (purity attributes) | CP-7 | `IS_PURE` flag on function types |
| `CG:§2 ABI` | EX-3 (call typing) | Argument/return type layout |
| `CG:TR-3` (aggregate field ordering) | EX-14 (struct literal) | Struct type with field list |

---

## §17 Key Files

| Path | Role |
|------|------|
| `ori_types/src/check/mod.rs` | `check_module` entry point, phase coordination (CK-1) |
| `ori_types/src/check/api/` | Public query surface for the driver |
| `ori_types/src/check/registration/mod.rs` | Registration-group dispatch (passes 0a–0e) |
| `ori_types/src/check/registration/user_types.rs` | Register struct / sum / newtype user types (`TYPES:RG-1`) — note: user surface does not include transparent aliases |
| `ori_types/src/check/registration/traits.rs` | Register trait definitions (TYPES:RG-2) |
| `ori_types/src/check/registration/impls.rs` | Register impls with coherence check (TR-2, TR-5) |
| `ori_types/src/check/registration/derived.rs` | Register derived trait impls (TR-8) |
| `ori_types/src/check/registration/consts.rs` | Register module constants |
| `ori_types/src/check/registration/builtin_types.rs` | Register builtin methods / prelude traits (TYPES:RG-3) |
| `ori_types/src/check/registration/type_resolution.rs` | Resolve `Tag::Named` to `TypeRegistry` entries |
| `ori_types/src/check/signatures/mod.rs` | Signatures pass 1 — collect function / method signatures (CK-4) |
| `ori_types/src/check/bodies/mod.rs` | Bodies-group passes 2–5 — function / test / impl / def-impl body checking |
| `ori_types/src/check/scope.rs` | Closure-based save/restore scope helpers for `ModuleChecker` (SG-1..SG-3); NOT RAII guards |
| `ori_types/src/check/exports.rs` | Typed IR emission after `check_module_impl` via `finish()` / `finish_with_pool()` (PC-2) |
| `ori_types/src/check/imports.rs` | Import resolution |
| `ori_types/src/check/object_safety.rs` | Object-safety check (TR-6) |
| `ori_types/src/check/validators/mod.rs` | Producer-side `PC-2` enforcement — `validate_body_types` walks `expr_types` + `FunctionSig` positions, emits `E2005` per surviving `Tag::Var` (`DI-1`). Gate order: `resolve_fully → HAS_ERROR → HAS_VAR`. Delegates compound-tag child traversal to `Pool::visit_children` (`TYPES:§TF-3`). |
| `ori_types/src/check/validators/tests.rs` | 12-cell matrix covering negative/positive/cascade/determinism/nesting/dedup axes of the validator's fast-path gates |
| `ori_types/src/check/well_known/` | Well-known type / trait identifiers |
| `ori_types/src/infer/mod.rs` | `InferEngine` (EN-1) |
| `ori_types/src/infer/context.rs` | `Expected` / `ExpectedOrigin` (BD-1..BD-4) |
| `ori_types/src/infer/env/` | `TypeEnv` (scoped `Name → Idx`) |
| `ori_types/src/infer/expr/mod.rs` | Expression typing dispatch |
| `ori_types/src/infer/expr/identifiers.rs` | Variable / identifier reference typing |
| `ori_types/src/infer/expr/operators.rs` | Operator typing (EX-2) |
| `ori_types/src/infer/expr/calls/` | Call typing (EX-3) |
| `ori_types/src/infer/expr/methods/` | Method-call resolution (EX-4, TR-1) |
| `ori_types/src/infer/expr/blocks.rs` | Block typing (EX-5) |
| `ori_types/src/infer/expr/control_flow.rs` | If / match / loop typing (EX-6, EX-7, EX-10) |
| `ori_types/src/infer/expr/collections.rs` | List / map / set literal typing (EX-15) |
| `ori_types/src/infer/expr/sequences.rs` | Literal and sequence typing (EX-1) |
| `ori_types/src/infer/expr/constructors.rs` | Sum-variant construction typing |
| `ori_types/src/infer/expr/structs/` | Struct literal / update typing (EX-14) |
| `ori_types/src/infer/expr/concurrency.rs` | Nursery / parallel / spawn typing |
| `ori_types/src/infer/expr/type_resolution.rs` | In-body type-reference resolution |
| `ori_types/src/infer/expr/registry_bridge/` | Bridge from `Tag` to `ori_registry::TypeTag` for builtin registry queries (bypasses `MethodRegistry`) |
| `ori_types/src/unify/mod.rs` | Unification driver (UN-1..UN-8) |
| `ori_types/src/unify/generalization.rs` | Scheme construction (GN-1) |
| `ori_types/src/unify/substitute.rs` | Body substitution during instantiation (GN-2) |
| `ori_types/src/unify/rank/` | Rank counter and var rank tracking (CK-2) |
| `ori_types/src/unify/error/` | Unification error shapes |
| `ori_types/src/type_error/check_error/` | `TypeErrorKind` variants and error construction (§11) |
| `ori_types/src/type_error/context/` | Error context (expected origin, span chains) |
| `ori_types/src/type_error/diff/` | Type difference rendering for mismatch errors |
| `ori_types/src/type_error/expected/` | `ExpectedOrigin` metadata |
| `ori_types/src/type_error/problem/` | Problem-level (diagnostic) construction |
| `ori_types/src/type_error/suggest/` | "did you mean?" suggestions (ER-6) |
| `ori_types/src/type_error/warning.rs` | Warning-severity diagnostics (DI-3) |
| `ori_types/src/reporting/` | Per-crate diagnostic formatting (routed to `ori_diagnostic`) |

---

## Appendix A: Pipeline Pass Decision Table

For each language construct, which pass (from the `CK-1` table) is responsible:

| Construct | Pass(es) | Rule | Rationale |
|-----------|----------|------|-----------|
| Built-in / prelude types | 0a (Registration) | CK-1, `TYPES:RG-1` | Must be registered before user types reference them |
| Struct / sum / newtype declarations | 0b (Registration) | CK-1, `TYPES:RG-1` | `TypeRegistry` frozen before Signatures |
| Trait declarations + default methods | 0c (Registration) | CK-1, `TYPES:RG-2` | `TraitRegistry` frozen before Signatures |
| Impl blocks (inherent, trait, default) | 0c (Registration) | CK-1, TR-2 | Coherence checked at registration (CK-5) |
| Derived impls (Eq, Clone, etc.) | 0d (Registration) | TR-8 | Generated once with canonical componentwise semantics |
| Module-level constants | 0e (Registration) + 1 (Signatures) | CK-1 | Declarations first, then RHS typed |
| Function / method signatures | 1 (Signatures) | CK-4 | Fresh `Tag::Var` permitted for elided annotations (checker-owned, resolved by Bodies); no `Tag::Infer` / `Tag::Projection` / unresolved `Tag::Named` allowed |
| Function bodies | 2 (Bodies — functions) | CK-1 | Uses frozen registries + signatures |
| Contracts (pre/post) | 2 (Bodies — functions) | CF-7 | Typed in function-body scope |
| Attached / floating tests | 3 (Bodies — tests) | CF-6 | Bodies like any function |
| Method bodies in `impl Type { ... }` and `impl Type: Trait { ... }` | 4 (Bodies — impl) | CK-1 | Typed with `Self` bound via SG-2 |
| Method bodies in `def impl Trait { ... }` | 5 (Bodies — def impl) | CK-1, TR-7 | Stateless default bodies typed last |
| Typed IR packaging | `finish()` / `finish_with_pool()` | CK-1, PC-2 | Validates output contract at driver boundary |

## Appendix B: Error Code Catalog — Quick Reference

See §11 for the full table with source variants. Summary:

- **E2001** — Type mismatch
- **E2003** — Unknown name / field
- **E2004** — Arity mismatch
- **E2005** — Ambiguous type
- **E2008** — Infinite type
- **E2010 / E2021 / E2022** — Impl coherence family
- **E2014** — Missing capability
- **E2019** — Uninhabited struct field
- **E2020** — Unsupported operator
- **E2023** — Ambiguous method
- **E2024** — Not object-safe
- **E2025 / E2026 / E2027** — Indexing family
- **E2028 / E2029 / E2032 / E2033** — Derive family
- **E2030** — Hash invariant (warning)
- **E2031** — Non-hashable map key
- **E2034 / E2035** — Format-spec family
- **E2036 / E2037** — Into conversion family
- **E2038** — Missing Printable for interpolation
- **E2039** — Assign to immutable
- **E2040** — Feature not yet supported
- **E2041** — Invalid #repr

## Appendix C: Bidirectional Mode Decision Table

For each expression form, whether the natural mode is `Synth` or `Check`:

| Form | Natural mode | Expected propagation |
|------|--------------|----------------------|
| Literal | Synth | — |
| Variable reference | Synth | — |
| Call | Synth (result type known from signature) | Args use `Check(param_ty)` |
| Method call | Synth | Args use `Check(param_ty)` |
| Block | matches terminal expression | Propagate to terminal |
| If/else | Synth from branches | Each branch `Check` if expected present |
| Match | Synth (join arms) | Each arm `Check` if expected present |
| Let binding RHS | Check (if annotated) else Synth | — |
| Lambda | Check (if expected function type) else Synth | Params + body from expected |
| List literal | Check (if expected `[T]`) else Synth | Elements use `Check(T)` |
| Map literal | Check (if expected `{K:V}`) else Synth | Keys `Check(K)`, values `Check(V)` |
| Struct literal | Check (resolves field types) | Field values use declared field types |
| Tuple literal | Check (if expected tuple) else Synth | Components use expected component types |
| Loop | Synth | Body always `Check(())` |
| Try (`e?`) | Check against outer Result/Option | Inner `e` synthesized |
| Cast (`e as T`) | Check the cast legality | `e` synthesized |
| Pipe (`x \|> f`) | Desugar then Synth | Desugared form drives mode |
