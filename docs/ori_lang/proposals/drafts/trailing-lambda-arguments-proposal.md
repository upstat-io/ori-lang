# Proposal: Trailing Lambda Arguments and Deferred-Pattern Unification

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-06-16
**Affects:** Compiler (parser, type system), standard library, spec (Clause 14, Clause 15), grammar
**Amends:** single-lambda-positional-proposal.md
**Depends On:** capability-propagation-completion-proposal.md

---

## Summary

Two coupled changes that collapse two redundant ways of writing "code to run later" into one:

1. **Generalize positional lambdas** (amends `single-lambda-positional-proposal.md`): a lambda *literal* in the **trailing** argument position may omit its parameter name in a call of *any* arity, binding by position to the last declared parameter. Today the name may be omitted only for single-parameter calls.
2. **Re-specify the deferred-evaluation function-expression patterns** (`catch`, `cache`, `timeout`, `with`) so their deferred arguments are ordinary lambda literals rather than bare by-name expressions, and convert those patterns from compiler-special `FunctionExpKind` constructs into ordinary higher-order functions wherever doing so is sound. Patterns that genuinely require compiler/runtime support (`recurse`, `parallel`, `spawn`, `nursery`, channel constructors) retain that support but adopt the same lambda-literal surface.

The result is one uniform surface for callbacks: a lambda literal, nameless when it is the trailing argument — exactly how `list.map(x -> x * 2)` already reads.

---

## Motivation

### The Problem in Practice

Ori currently has **two** surface forms for "an argument that is code to be run later," and they are visually indistinguishable from eager code:

```ori
// Bare by-name expression — LOOKS eager, is actually deferred:
catch(expr: may_panic())
cache(key: `user-{id}`, op: db.query(id: id), ttl: 5m)
timeout(op: slow_call(), after: 5s)

// Lambda — explicitly deferred, in the SAME feature family:
with(acquire: open_file(path), action: f -> read_all(f), release: f -> close(f))
nursery(body: () -> spawn_workers(), on_error: handler)
list.map(transform: x -> x * 2)
```

Three problems compound here:

1. **Deception.** `cache(op: db.query(id:))` reads as if `db.query` is called eagerly at that point. It is not — `op:` is evaluated zero times on a cache hit, once on a miss (Clause 15 §15.x cache semantics). The bare-expression spelling hides the deferral that is the whole point of the construct.
2. **Inconsistency.** `with(action: f -> ...)` spells its deferred argument as a lambda, while `cache(op: ...)` spells it as a bare expression — within the same family of control-abstraction patterns. `nursery(body: () -> ...)` already mandates a lambda (grammar `nursery_args`), while its sibling `catch(expr: ...)` mandates a bare expression.
3. **Redundant labels.** `expr:` and `op:` name the single obvious callback of a control wrapper. The label adds ceremony without disambiguation — the exact redundancy `single-lambda-positional-proposal.md` already removed for `map`/`filter`/`find`.

### Why these are compiler-special at all

`catch`, `cache`, `timeout`, `with` are not language primitives in any deep sense — they are deferred-evaluation control wrappers. The compiler models them as 15 hard-coded `FunctionExpKind` variants dispatched through `ori_patterns` purely so their arguments can be evaluated lazily. A first-class deferred value (a lambda) expresses the same thing without bespoke compiler machinery, which is what the **lean-core** mission (design principle #6: "compiler implements only features needing special syntax / static analysis; stdlib in pure Ori") asks for.

---

## Goals and Non-Goals

**Goals:**

- One surface for deferred callbacks: lambda literals.
- A trailing lambda literal may drop its parameter name in any-arity calls (generalizing the approved single-parameter rule).
- Move `catch` / `cache` / `timeout` / `with` out of the compiler's pattern machinery into ordinary functions wherever sound.

**Non-Goals:**

- **No `{ }` trailing-closure block syntax.** `single-lambda-positional-proposal.md` explicitly rejected block syntax ("requires new syntax"); this proposal reuses the existing `x -> e` / `() -> e` lambda forms only.
- **No multiple nameless trailing lambdas.** Only the single trailing argument may be nameless. Patterns with several lambda parameters (`with`, `recurse`) keep names on all but at most the last. (Swift's multiple-trailing-closure feature, swift#38625, is the complexity this avoids.)
- **No removal of compiler/runtime support** for `recurse` (needs `self`), `parallel` / `spawn` / `nursery` (need scheduler + `Suspend`), or channel constructors. These adopt the lambda surface but stay compiler-aware.
- **No new effect-system surface syntax** is defined here beyond what the capability dependencies provide; see Unresolved Questions.

---

## Design

### Part 1 — Trailing positional lambda (generalizes the approved single-param rule)

`14-expressions.md` §14.1.3 today permits a positional (nameless) argument in three cases, the third being "single-parameter functions called with inline lambda expressions." This proposal **replaces** the single-parameter restriction of case 3 with a trailing-position rule:

> The **final** argument of a call may be written positionally, without its parameter name, **if and only if** it is a lambda literal (`x -> e`, `(a, b) -> e`, `() -> e`, or a typed-parameter form). It binds to the **last declared parameter**, which shall be of function type. All preceding arguments shall be named. Function references and variables holding functions are not lambda literals and continue to require the name.

This is strictly more permissive and subsumes the approved rule: a single-parameter call's only parameter is also its last parameter, so `list.map(x -> x * 2)` remains valid as a special case.

**Examples:**

```ori
// Single trailing callback — fully clean:
catch(() -> may_panic())
timeout(after: 5s, () -> slow_call())
cache(key: `user-{id}`, ttl: 5m, () -> db.query(id: id))

// Named form ALWAYS remains valid (parity with single-lambda-positional):
catch(expr: () -> may_panic())
timeout(after: 5s, op: () -> slow_call())

// Function reference still requires the name (unchanged):
let attempt = () -> may_panic()
catch(attempt)              // error: name required (not a lambda literal)
catch(expr: attempt)        // OK
```

### Part 2 — Signature reordering (callback declared last)

By-position binding requires each affected signature to declare its primary callback **last**. The reorders:

| Pattern | Today | Reordered |
|---|---|---|
| `timeout` | `timeout(op:, after:)` | `timeout(after:, op:)` |
| `cache` | `cache(key:, op:, ttl:)` | `cache(key:, ttl:, op:)` |
| `recurse` | `recurse(condition:, base:, step:, memo:, parallel:)` | `recurse(condition:, base:, memo:, parallel:, step:)` |
| `with` | `with(acquire:, action:, release:)` | unchanged — multi-lambda, stays fully named (see below) |
| `catch` | `catch(expr:)` | unchanged — single argument |

`with` has three lambda parameters; only one could ever be nameless, and its conceptual "primary" callback (`action:`) is not naturally last (cleanup `release:` is). The ergonomic gain is marginal and the names aid clarity, so `with` retains named arguments for all three. This is the honest boundary of the trailing-lambda benefit: it is large for single-callback patterns and negligible for multi-lambda ones.

### Part 3 — Deferred arguments become lambda literals

The deferred arguments formerly written as bare expressions become lambda literals:

```ori
// catch / cache / timeout: zero-arg thunks
catch(() -> may_panic())
cache(key: k, ttl: 5m, () -> db.query(id:))

// recurse: step receives `self` as its parameter instead of a magic binding
@factorial (n: int) -> int =
    recurse(condition: () -> n <= 1, base: () -> 1, self -> n * self(n - 1));
```

For `recurse`, the previously-magic `self(...)` binding inside `step` becomes an explicit lambda parameter (`self -> ...`). `condition` and `base` become zero-argument thunks that close over the enclosing parameters as before.

### Part 4 — Conversion to library functions (as far as sound)

With deferred arguments now first-class lambdas, the patterns that are *pure deferred-control-flow* become ordinary functions. The conversion is honest about what each pattern actually needs:

| Pattern | Becomes | Requires |
|---|---|---|
| `cache` | pure-Ori library HOF | `Cache` capability + effect-polymorphism over `op` |
| `with` | pure-Ori library HOF | a deterministic cleanup primitive (`Drop`/defer) for the run-on-all-paths guarantee |
| `timeout` | library HOF | `Suspend` + a cancellation intrinsic |
| `catch` | library-signature HOF with an **intrinsic body** | runtime panic-boundary intrinsic |
| `recurse` | stays compiler construct | `self` recursion + memo/parallel lowering |
| `parallel` / `spawn` / `nursery` | stay compiler constructs | scheduler + `Suspend` |
| `channel*` | stay constructors | channel runtime |

Illustrative library signatures (effect notation tentative — see Unresolved Questions):

```ori
pub @cache<K, V, E> (key: K, ttl: Duration, op: () -> V uses E) -> V uses Cache, E
    where K: Hashable + Eq = ...;

pub @with<R, T, E> (acquire: () -> R uses E, action: (R) -> T uses E, release: (R) -> void uses E) -> T uses E = ...;

// catch keeps an intrinsic body but presents an ordinary signature:
pub @catch<T> (expr: () -> T) -> Result<T, str> = <intrinsic panic boundary>;
```

The load-bearing requirement is **effect transparency**: a passed closure's `uses` effects must surface to the calling function's obligations (the `E` variable above). Ori's type checker does not do this today (capability checks are lexical at named-call sites; lambda types carry no effect information), which is why this proposal depends on `capability-propagation-completion-proposal.md` and on effect-row polymorphism over function-typed parameters.

### Semantics (unchanged observable behavior)

Each converted construct preserves its current evaluation semantics exactly:

- `cache`: compute `key`; on hit return the stored value (clone); on miss invoke `op` once, store, return.
- `with`: invoke `acquire`; if it succeeds, invoke `action`, then invoke `release` on every exit path including panic.
- `timeout`: invoke `op` under the deadline; cancel on expiry.
- `catch`: invoke `expr` under a panic boundary; `Ok(v)` on success, `Err(msg)` on panic.
- `recurse`: evaluate `condition`; if true return `base`; else evaluate `step` with `self` bound to the recursive invocation.

### Error Handling

- A positional non-lambda-literal argument in non-trailing position, or a positional final argument whose target parameter is not of function type: `E20xx` (extends the existing "positional outside permitted cases" error of `14-expressions.md` §14.1.3).
- A nameless trailing lambda whose arity does not match the last parameter's function type: existing lambda-arity mismatch diagnostic.
- Calling a converted HOF whose closure requires a capability not available in the caller: existing capability error (`E1200`), now reached through the closure rather than a lexical call — this is precisely the behavior `capability-propagation-completion-proposal.md` must deliver.

---

## Drawbacks

- **Effect-system dependency is large.** The conversion is only sound once closure effects propagate to callers. Until then, only Part 1 (surface syntax) and Part 3 (lambda spelling, kept as compiler patterns) can ship; Part 4 (library conversion) is gated.
- **Two ways to pass a callback persist.** `catch(() -> e)` and `catch(expr: () -> e)` are both legal — the same tolerated redundancy `single-lambda-positional-proposal.md` already accepted for `map`. It does not add a *new* axis of redundancy, but it does extend an existing one.
- **Signature reorders are a (small) breaking change** for any existing code calling `timeout` / `cache` / `recurse` with positional or reordered-named arguments. Named-argument calls in the current order continue to work because names are order-independent; only positional callers break.
- **`with` gains little.** The benefit is uneven across patterns; the proposal must not oversell uniformity.
- **`() ->` ceremony on zero-arg thunks.** `catch(() -> e)` is marginally noisier than today's `catch(expr: e)` on length, trading brevity for explicit-deferral honesty.

---

## Alternatives Considered

### Alternative 1: `{ }` trailing-closure block syntax (Swift/Kotlin)

`cache(key: k, ttl: 5m) { db.query(id:) }`. Rejected for consistency with `single-lambda-positional-proposal.md`, which already rejected block syntax as unnecessary new surface. Reusing `() ->` keeps one lambda syntax.

### Alternative 2: `@autoclosure`-style implicit thunking (Swift, Scala by-name)

Let the call site write `cache(op: db.query(id:))` and have the compiler wrap it in a thunk automatically. Rejected because it re-introduces the deception this proposal removes — the deferral becomes invisible again — conflicting with "explicit over implicit" (design principle #1).

### Alternative 3: Bind trailing lambda to the sole remaining lambda-typed parameter (no reorders)

Avoids signature churn but binds by type rather than position, is more implicit, and needs a tiebreak when two lambda parameters are unfilled. Rejected in favor of by-position binding, consistent with how Ori positional arguments already bind and with Swift SE-0286 forward-matching.

### Alternative 4: Keep patterns compiler-special; change surface only

Ship Parts 1–3 and never convert to library functions. This is the fallback if the capability dependencies do not land; it captures the ergonomic and consistency wins but leaves the lean-core debt. Recorded as the staged-delivery floor rather than the end state.

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY.

- `cache` — YES given a `Cache` capability handler and effect propagation; the control flow is pure.
- `with` — YES given a deterministic run-on-all-paths cleanup primitive (the panic-path guarantee needs `Drop`/defer support).
- `timeout` — NO without `Suspend` + a cancellation intrinsic.
- `catch` — NO; a panic boundary is a runtime intrinsic. It can present an ordinary HOF signature over an intrinsic body.
- `recurse`, `parallel`, `spawn`, `nursery`, `channel*` — NO; intrinsic compiler/runtime support.

**Missing features that would enable purity:** effect-row polymorphism over function-typed parameters; capability propagation through closures (`capability-propagation-completion-proposal.md`); a defer/`Drop`-based cleanup primitive for `with`; a panic-boundary intrinsic exposed to library code for `catch`.

**Recommendation:** Proceed as a staged proposal. Part 1 (trailing-lambda syntax) and Part 3 (lambda-literal spelling) are independent of the capability work and can land first. Part 4 (library conversion) is gated on the capability dependencies and lands per-pattern as each prerequisite resolves.

---

## Spec & Grammar Impact

- **`grammar.ebnf`**: `function_exp` / `catch_expr` / `nursery_args` either fold into ordinary `call_args` (for converted patterns) or have their `pattern_arg` expression positions constrained to `lambda` (for retained patterns). `call_arg` gains the trailing-positional-lambda rule.
- **`14-expressions.md` §14.1.3**: replace the single-parameter clause of the positional-argument rule with the trailing-parameter rule (Part 1).
- **`15-patterns.md`**: re-specify `catch` / `cache` / `timeout` / `with` deferred arguments as lambdas; document which patterns become library functions vs. retain compiler support; restate `recurse`'s `self` as an explicit `step` lambda parameter.
- **Clause 20 (capabilities)**: cross-reference the effect-transparency requirement (owned by the capability dependencies).

---

## Prior Art

- **Swift — SE-0286 "Forward matching of trailing closure arguments"** (swift#32891, swift#32644): trailing closures bind to parameters by forward position matching — the direct precedent for Part 1's by-position binding. **Multiple trailing closures** (swift#38625) are the complexity this proposal deliberately avoids by allowing only one nameless trailing lambda.
- **Kotlin** — a trailing lambda may be moved outside the parentheses and is unlabeled; inline lambdas avoid allocation. Validates the single-trailing-callback ergonomic.
- **Koka** — effect rows make a higher-order function polymorphic over the effects of its function parameters (koka#873 / koka#875 on higher-rank effect handlers). This is the model for the effect transparency Part 4 requires.
- **Scala (by-name `=> T`) / Swift (`@autoclosure`)** — implicit deferral mechanisms; considered and rejected (Alternative 2) for hiding the deferral.

*(All issue/PR references verified against the intelligence graph; SE-0286 corresponds to Swift's adopted forward-matching rule.)*

---

## Unresolved Questions

- **Effect notation.** What is the surface syntax (if any) for effect-polymorphic HOF signatures (`op: () -> V uses E`), and is the `E` inferred at each call site (Rust-closure style) or written explicitly (Koka effect-row style)? Resolve before Part 4; may warrant a dedicated effect-polymorphism proposal beyond `capability-propagation-completion-proposal.md`.
- **`with` cleanup primitive.** Does the run-on-all-paths `release` guarantee ride on `Drop`, a `defer` construct, or a retained intrinsic? Decided during `with` conversion.
- **`catch` exposure.** Should the panic-boundary intrinsic be a general library-callable primitive, or remain a single dedicated `catch` builtin with an ordinary signature?
- **Migration window.** Should the named pre-reorder forms of `timeout` / `cache` / `recurse` be accepted during a deprecation window, or is the rename atomic?
