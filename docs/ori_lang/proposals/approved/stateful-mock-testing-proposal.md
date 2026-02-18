# Proposal: Stateful Mock Testing in the Capability System

**Status:** Approved
**Approved:** 2026-02-18
**Author:** Eric (with AI assistance)
**Created:** 2026-02-17
**Affects:** Capability system, `with...in` semantics, testing, type system, grammar

---

## Summary

Ori's capability system uses `with...in` to provide mock implementations for testing. However, some tests require mocks that accumulate state across multiple calls (e.g., a counter that increments). Because Ori has value semantics and no shared mutable references, user-defined mock types cannot carry mutable state. This proposal introduces **stateful effect handlers** -- a `handler(state: expr) { ... }` construct for `with...in` that threads local mutable state through handler operations, preserving value semantics while enabling stateful capability mocking.

---

## Problem Statement

The spec at `14-capabilities.md` shows `MockClock.advance()` with interior mutability, but `MockClock` is a runtime-provided type. User-defined types in Ori cannot have interior mutability -- this is a language invariant (see `15-memory-model.md` and `sendable-interior-mutability-proposal.md`).

A test in `tests/spec/expressions/with_expr.ori` is skipped because it expects stateful behavior from a value-semantic mock:

```ori
#skip("test expects mutable state but Counter.increment is pure")
@test_with_expression_body tests @with_expression_body () -> void = run(
    let mock = MockCounter { value: 0 },
    let result = with Counter = mock in run(
        let a = mock.increment(),  // expects 1
        let b = mock.increment(),  // expects 2 (accumulated!)
        a + b,
    ),
    assert_eq(actual: result, expected: 3), // 1 + 2
)
```

Since `MockCounter` is a value type, `mock.increment()` returns `self.value + 1` every time -- always `1` for a mock initialized with `value: 0`. There is no mechanism for `increment()` to modify `mock` in-place across calls.

### Where the Problem Arises

This affects any capability mock that must:
1. **Count calls** -- how many times was a method invoked?
2. **Sequence responses** -- return different values on successive calls
3. **Record interactions** -- log what was called with what arguments
4. **Accumulate state** -- maintain a running total, buffer, or queue

These are common testing patterns in every language. Ori currently has no answer for them in user code.

### What the Spec Says Today

The `14-capabilities.md` spec acknowledges this gap implicitly:

> `MockClock` uses interior mutability for its time state, allowing `advance()` without reassignment.

This is documented as a property of the runtime-provided `MockClock` type. No mechanism exists for user-defined types to achieve the same behavior. Stateful handlers eliminate this gap.

---

## How Reference Languages Solve This

### Koka: Stateful Effect Handlers

Koka solves this directly through its effect handler system. An effect handler can carry local mutable state via `var`:

```koka
effect state<s>
  fun get() : s
  fun put(x : s) : ()

fun state(init, action)
  var s := init
  handle action
    fun get()  s
    fun put(x) s := x
```

The `var` keyword introduces a mutable local variable scoped to the handler. The handler's operations (`get`, `put`) read and write this variable. The key insight: **the state lives in the handler frame, not in the effect type.** The effect operations are pure from the caller's perspective -- state is an implementation detail of the handler.

Koka's approach is relevant because Ori's `with...in` is semantically similar to Koka's `handle...with`. Both provide an implementation for an abstract effect within a lexical scope.

### Haskell: IORef / STRef

Haskell uses monadic mutable references for testing:

```haskell
test :: IO ()
test = do
    ref <- newIORef 0
    let increment = modifyIORef ref (+1) >> readIORef ref
    a <- increment  -- 1
    b <- increment  -- 2
    assertEqual (a + b) 3
```

`IORef` provides interior mutability within the `IO` monad. `STRef` provides the same within the pure `ST` monad (via rank-2 types ensuring references cannot escape). Both require monadic sequencing.

This is clean but relies on reference types that Ori explicitly forbids in user code.

### Roc: Platform-Provided Effects

Roc treats all effects as opaque operations provided by the platform (runtime). User code describes effects declaratively; the platform implements them. Testing is done by swapping the platform. There is no user-level mechanism for stateful mocks -- effects are always opaque.

This is similar to Ori's current situation with `MockClock`: the runtime provides it, users cannot build their own.

### Swift: Classes with Reference Semantics

Swift solves this trivially because classes have reference semantics:

```swift
class MockCounter: Counter {
    var value = 0
    func increment() -> Int { value += 1; return value }
}
```

Multiple references to the same `MockCounter` instance share state. Ori has no equivalent -- all user-defined types have value semantics.

### Elm: No Side Effects in Tests

Elm avoids the problem entirely. Tests are pure functions that assert on pure transformations. There is no concept of mocking stateful interactions because there are no stateful interactions in user code. Effects happen at the boundary via `Cmd`/`Sub`.

---

## Rejected Alternatives

### Alternative 1: TestCell\<T\> -- Test-Only Mutable Cell

A special type `TestCell<T>` available only in test code that provides interior mutability.

**Rejected because:**
- Violates value semantics -- `TestCell` has reference semantics by definition
- Creates a precedent for interior mutability in user code
- "Test-only" restrictions tend to leak -- users will demand `TestCell` in production
- Shared mutable state breaks the ARC safety invariant
- Requires exemption from `Sendable` checks, adding a special case to the type system

### Alternative 2: Event Recording Pattern (Pure)

Record events and assert on the log after execution.

**Rejected because:**
- Has the same fundamental problem it claims to solve -- the log must be shared between mock and test
- Reduces to Alternative 1 (shared mutable log) or to the chosen approach (stateful handler with log as state)
- Not a standalone solution

### Alternative 3: Monadic State Threading

Thread state explicitly through each call, returning `(updated_mock, result)`.

**Rejected because:**
- Works for direct calls but **cannot work through `with...in`** -- trait method signatures are fixed
- `@increment (self) -> int` cannot return `(MockCounter, int)` while satisfying the trait
- Only works when `with...in` capability provision is not involved, which is the exact case that needs solving

---

## Design: Stateful Effect Handlers

### Overview

Extend `with...in` to support stateful handlers. The `handler(state: expr) { ... }` construct creates a handler frame with local mutable state that its operations can read and modify:

```ori
@test_counter () -> void = run(
    let result = with Counter = handler(state: 0) {
        increment: (s) -> (s + 1, s + 1),
        get: (s) -> (s, s),
    } in run(
        let a = Counter.increment(),  // handler state: 0 -> 1, returns 1
        let b = Counter.increment(),  // handler state: 1 -> 2, returns 2
        a + b,
    ),
    assert_eq(actual: result, expected: 3),
)
```

### How It Works

1. The `handler(state: S)` expression creates a stateful handler frame
2. Each operation receives the current state as its first argument (replacing `self`)
3. Each operation returns a tuple `(next_state, return_value)`
4. The state is threaded through operations sequentially (left to right in the `run` block)
5. The `with...in` expression returns the body's result type (state is internal to the handler)

The state is local to the handler frame. It is not shared, not aliased, and not accessible outside the `with...in` scope. This is exactly how Koka's `var`-based handlers work -- the mutable state lives in the handler's activation frame, invisible to the type system's value semantics.

### Semantic Model

This is equivalent to CPS-transforming the handler state through each operation call:

```
// Desugared (conceptual, not surface syntax)
let s0 = 0
let (s1, a) = increment_handler(s0)  // (1, 1)
let (s2, b) = increment_handler(s1)  // (2, 2)
let result = a + b                    // 3
```

The state threading is implicit -- the programmer writes `Counter.increment()` and the compiler/runtime handles the plumbing. This preserves the purity of the calling code while allowing the handler to maintain state.

### Keyword Treatment

`handler` is a **context-sensitive keyword**, valid only in the expression position of a `capability_binding` (the RHS of `with X = ...`). It is consistent with Ori's treatment of other context-sensitive names like `run`, `try`, `match`, `cache`, etc. `handler` as an identifier remains valid in all other positions.

### State Value

Handlers support a **single state value**. For multiple independent state values, compose them into a tuple or struct:

```ori
with Counter = handler(state: { count: 0, log: [] }) {
    increment: (s) -> run(
        let new_count = s.count + 1,
        ({ count: new_count, log: [...s.log, "inc"] }, new_count),
    ),
    get: (s) -> (s, s.count),
    calls: (s) -> (s, s.log),
} in ...
```

### Return Type

`with...in` with a stateful handler returns the **body's type only**. The handler's final state is not included in the return type. To observe final state, call a handler operation within the body:

```ori
// Need final count? Call get() at the end of the body
let (result, final_count) = with Counter = handler(state: 0) {
    increment: (s) -> (s + 1, s + 1),
    get: (s) -> (s, s),
} in run(
    let a = Counter.increment(),
    let b = Counter.increment(),
    (a + b, Counter.get()),
),
```

This preserves backward compatibility of `with...in` semantics -- no type system special-casing is needed.

### Scope

Stateful handlers are **available everywhere**, not restricted to test code. Any `with...in` scope may use a `handler(...)` expression. The mechanism is sound for all code because:
- State is scoped to the `with...in` block (no leaks)
- No shared mutable references (frame-local)
- No violation of ARC invariants

`def impl` **cannot be stateful**. Default implementations are stateless by design -- they have no `self` and no instance scope. A `def impl` exists at module level with no clear state lifetime. Stateful handlers require a `with...in` scope to bound their state's lifetime.

---

## Type Checking Rules

### Handler-Trait Signature Mapping

For a handler operation named `op` implementing trait method `@op (self, p1: T1, ..., pN: TN) -> R`:

1. The handler operation receives `(state: S, p1: T1, ..., pN: TN)` -- **state replaces `self`**
2. The handler operation must return `(S, R)` -- a tuple of `(next_state, return_value)`
3. The state type `S` is inferred from the `state:` initializer expression
4. All handler operations must use the **same state type `S`**
5. Every required trait method must have a corresponding handler operation
6. Default trait methods are used if not overridden in the handler
7. Handler operations for non-existent trait methods are an error

### Handlers Are Not Impl Blocks

A `handler(...)` expression is a **distinct dispatch mechanism** from `impl Trait for Type`. The handler:
- Has **no `self`** -- state replaces self in the operation signature
- Is **not a type** -- it is a handler frame with scoped lifetime
- Does **not require** an `impl` block -- the handler declaration itself satisfies the trait for the scope of the `with...in`

The type checker treats `handler(...)` as satisfying the trait's interface for the duration of the `with...in` scope.

### Multi-Parameter Methods

When a trait method takes additional parameters beyond `self`, the handler operation receives them after state:

```ori
trait Cache {
    @get (self, key: str) -> Option<str>
    @set (self, key: str, value: str) -> void
}

with Cache = handler(state: {str: str} {}) {
    get: (s, key: str) -> (s, s[key]),
    set: (s, key: str, value: str) -> ({...s, [key]: value}, ()),
} in ...
```

### Verification Order

1. Infer state type `S` from the `state:` initializer
2. For each handler operation:
   a. Look up the corresponding trait method
   b. Verify parameter types match (after state-replaces-self substitution)
   c. Verify return type is `(S, R)` where `R` is the trait method's return type
3. Verify all required trait methods are covered (accounting for defaults)
4. Verify no extra operations reference non-existent trait methods

---

## Nested Handler Semantics

Each `handler(...)` maintains its own state, independent of other handlers in the same or enclosing `with...in` scopes. State is not shared between handlers.

An inner handler operation may invoke outer handler operations. The outer handler's state threading is independent:

```ori
with Logger = handler(state: []) {
    log: (s, msg: str) -> ([...s, msg], ()),
    entries: (s) -> (s, s),
} in
    with Counter = handler(state: 0) {
        increment: (s) -> run(
            Logger.log(msg: "increment called"),  // invokes outer handler
            (s + 1, s + 1),
        ),
        get: (s) -> (s, s),
    } in run(
        Counter.increment(),
        Counter.increment(),
        assert_eq(actual: Logger.entries(), expected: ["increment called", "increment called"]),
    )
```

Each handler's state is threaded independently through its own operations. Cross-handler calls dispatch through the normal capability resolution chain.

---

## Error Codes

| Code | Description |
|------|-------------|
| E1204 | Handler missing required operation (trait method not defined in handler) |
| E1205 | Handler operation signature mismatch (parameters or return type don't match trait) |
| E1206 | Handler state type inconsistency (operations return different state types) |
| E1207 | Handler operation for non-existent trait method |

### Diagnostic Examples

```
error[E1204]: handler missing required operation `get`
  --> src/test.ori:5:20
   |
5  |     with Counter = handler(state: 0) {
   |                    ^^^^^^^ missing `get`
   |
   = note: trait `Counter` requires: increment, get
   = help: add `get: (s) -> (s, s)` to the handler

error[E1205]: handler operation `increment` has wrong return type
  --> src/test.ori:6:9
   |
6  |         increment: (s) -> s + 1,
   |         ^^^^^^^^^ expected (int, int), got int
   |
   = note: handler operations must return (next_state, return_value)
   = help: change to `increment: (s) -> (s + 1, s + 1)`

error[E1206]: handler state type inconsistency
  --> src/test.ori:7:9
   |
6  |         increment: (s) -> (s + 1, s + 1),
   |                            --- state type inferred as `int` here
7  |         reset: (s) -> ("zero", 0),
   |                        ^^^^^^ expected `int`, got `str`
   |
   = note: all handler operations must use the same state type

error[E1207]: handler operation `nonexistent` does not match any method in trait `Counter`
  --> src/test.ori:8:9
   |
8  |         nonexistent: (s) -> (s, 0),
   |         ^^^^^^^^^^^ `Counter` has no method `nonexistent`
   |
   = note: trait `Counter` methods: increment, get
```

---

## Alignment with Ori's Design

This approach aligns with Ori's existing design trajectory:

1. **Capabilities are already effect-like** -- `with...in` is a handler
2. **Value semantics preserved** -- state is frame-local, not shared
3. **Explicit effects** -- the `handler(state: ...)` syntax makes statefulness visible
4. **No new types** -- reuses existing capability machinery
5. **Koka precedent** -- well-studied semantics from effect handler research
6. **ARC invariants maintained** -- no shared mutable references, no interior mutability

### What About MockClock?

The spec's `MockClock.advance()` pattern (interior mutability in a runtime-provided type) becomes a stateful handler:

```ori
@test_expiry tests @is_expired () -> void = run(
    let result = with Clock = handler(state: Instant.from_unix_secs(secs: 1700000000)) {
        now: (s) -> (s, s),
        advance: (s, by: Duration) -> (s + by, ()),
    } in run(
        assert(!is_expired(token: token)),
        Clock.advance(by: 1h),
        assert(is_expired(token: token)),
    ),
)
```

This eliminates the need for a runtime-provided `MockClock` type entirely. Users can build their own stateful clock mock using the handler mechanism.

---

## Comparative Analysis

| Criterion | TestCell (rejected) | **Stateful Handlers** (chosen) | Event Log (rejected) | State Threading (rejected) |
|-----------|--------------------|-----------------------------|---------------------|-----------------------------|
| Value semantics | Violates | **Preserves** | Requires TestCell or Handlers | Preserves |
| Works with `with...in` | Yes | **Yes** | Requires TestCell or Handlers | No |
| Implementation cost | Moderate | **High** | N/A | None |
| Ergonomics | Good | **Good** | N/A | Poor |
| Language changes | Yes | **Yes** | N/A | None |
| Precedent | Rust's Cell | **Koka's handlers** | N/A | Haskell's State monad |
| Shared mutable state | Yes (problematic) | **No (frame-local)** | N/A | No |

---

## Impact on Existing Spec

### Changes to `14-capabilities.md`

1. **Add "Stateful Handlers" section** after "Providing Capabilities":
   - Handler syntax and semantics
   - State threading model
   - Type checking rules
   - Scope and lifetime rules
   - Interaction with nested handlers

2. **Update `MockClock` example** to use stateful handlers instead of asserting interior mutability for a runtime-provided type.

3. **Add error codes** E1204-E1207.

### Changes to `grammar.ebnf`

Extend with handler expression productions:

```ebnf
handler_expr       = "handler" "(" "state" ":" expression ")" "{" handler_operations "}" .
handler_operations = handler_operation { "," handler_operation } .
handler_operation  = identifier ":" expression .
```

Add `handler` to context-sensitive keywords list.

### Changes to `15-memory-model.md`

Add a note clarifying that handler frame state is frame-local mutable state (similar to `var` in loop bodies) and does not violate the "no shared mutable references" invariant.

---

## Migration: Rewriting the Skipped Test

### Current (skipped):

```ori
#skip("test expects mutable state but Counter.increment is pure")
@test_with_expression_body tests @with_expression_body () -> void = run(
    let mock = MockCounter { value: 0 },
    let result = with Counter = mock in run(
        let a = mock.increment(),
        let b = mock.increment(),
        a + b,
    ),
    assert_eq(actual: result, expected: 3),
)
```

### With Stateful Handlers:

```ori
@test_with_expression_body tests @with_expression_body () -> void = run(
    let result = with Counter = handler(state: 0) {
        increment: (s) -> (s + 1, s + 1),
        get: (s) -> (s, s),
    } in run(
        let a = Counter.increment(),
        let b = Counter.increment(),
        a + b,
    ),
    assert_eq(actual: result, expected: 3),
)
```

---

## Phased Implementation

**Phase 1: Grammar and parsing**
- Add `handler` to context-sensitive keywords
- Parse `handler(state: expr) { op: expr, ... }` within `capability_binding`
- Produce AST nodes for handler expressions

**Phase 2: Type checker support**
- Verify handler operations match trait signature (state-replaces-self mapping)
- Infer state type from initializer
- Verify state type consistency across operations
- Emit E1204-E1207 diagnostics

**Phase 3: Evaluator support**
- Implement handler frame state in the evaluator
- Thread state through capability dispatch calls within `with...in` scope
- Handle nested handler state independently

**Phase 4: LLVM codegen**
- Generate handler frame state allocation
- State threading through operation calls in AOT compilation

---

## Summary

| Aspect | Decision |
|--------|----------|
| Approach | Stateful effect handlers |
| Keyword | `handler` (context-sensitive) |
| Value semantics | Preserved -- state is frame-local |
| Shared mutable state | None -- state lives in handler frame |
| State convention | `(next_state, return_value)` tuple |
| State count | Single state value; compose via structs/tuples |
| Return type | `with...in` returns body result only |
| Trait mapping | State replaces `self`; handler ops return `(S, R)` |
| Scope | Available everywhere (not test-only) |
| `def impl` | Cannot be stateful (no scope for state lifetime) |
| Nested handlers | Independent state scopes |
| Compiler changes | Grammar, type checker, evaluator, LLVM codegen |
| New types | None |
| Error codes | E1204-E1207 |
| Precedent | Koka's effect handlers with `var` |
| Primary use case | Stateful mock testing via `with...in` |
| Secondary use case | Any capability that benefits from handler-local state |
| Replaces | Runtime-only `MockClock` interior mutability |
