# Proposal: Stateful Mock Testing in the Capability System

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-17
**Affects:** Capability system, `with...in` semantics, testing, potentially type system

---

## Summary

Ori's capability system uses `with...in` to provide mock implementations for testing. However, some tests require mocks that accumulate state across multiple calls (e.g., a counter that increments). Because Ori has value semantics and no shared mutable references, user-defined mock types cannot carry mutable state. This proposal evaluates four approaches to stateful mock testing and recommends **Approach 2: Stateful Effect Handlers**.

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

This is documented as a property of the runtime-provided `MockClock` type. No mechanism exists for user-defined types to achieve the same behavior.

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

## Approach 1: TestCell\<T\> -- Test-Only Mutable Cell

### Design

Introduce a special type `TestCell<T>` available only in test code that provides interior mutability:

```ori
@test_counter () -> void = run(
    let counter = TestCell(0),
    let mock = MockCounter { cell: counter },
    with Counter = mock in run(
        mock.increment(),  // modifies counter via TestCell
        mock.increment(),
        assert_eq(actual: counter.get(), expected: 2),
    ),
)
```

`TestCell<T>` would support:
- `TestCell(value)` -- constructor
- `.get() -> T` -- read current value
- `.set(value: T) -> void` -- write new value
- `.update(f: (T) -> T) -> T` -- read-modify-write, returns new value

### Mock Implementation

```ori
type MockCounter = { cell: TestCell<int> }

impl Counter for MockCounter {
    @increment (self) -> int = self.cell.update(v -> v + 1)
    @get (self) -> int = self.cell.get()
}
```

### Implementation Requirements

- **Compiler change**: New built-in type with special semantics
- **Type system**: `TestCell<T>` only valid in `@test` functions (enforced by type checker)
- **Runtime**: Backed by a heap-allocated mutable cell with single-owner semantics
- **Sendable**: `TestCell<T>` is NOT `Sendable` (cannot cross task boundaries)

### Evaluation

| Criterion | Assessment |
|-----------|------------|
| Value semantics consistency | **Violates.** `TestCell` has reference semantics by definition. Two copies of a struct containing a `TestCell` would share state, which contradicts Ori's memory model. |
| Implementation complexity | **Moderate.** New built-in type, test-only restriction in type checker. |
| Ergonomics | **Good.** Familiar pattern for Rust/Swift users. |
| Language changes | **Yes.** New type, new semantics. |
| Library-only | **No.** Requires compiler support. |

### Risks

- Creates a precedent for interior mutability in user code, even if test-only
- "Test-only" restrictions tend to leak -- users will want `TestCell` in production code for "just this one case"
- Shared mutable state breaks the ARC safety invariant that no shared mutable references exist
- The `TestCell` would need to be exempted from `Sendable` checks, adding a special case to the type system

---

## Approach 2: Stateful Effect Handlers (Koka-style)

### Design

Extend `with...in` to support stateful handlers. The handler carries local mutable state that its operations can read and modify:

```ori
@test_counter () -> void = run(
    let (result, final_state) = with Counter = stateful(
        initial: 0,
        handlers: {
            increment: (state) -> run(
                let new_state = state + 1,
                (new_state, new_state),  // (next_state, return_value)
            ),
            get: (state) -> (state, state),  // state unchanged, return current
        },
    ) in run(
        let a = Counter.increment(),  // handler state: 0 -> 1, returns 1
        let b = Counter.increment(),  // handler state: 1 -> 2, returns 2
        a + b,
    ),
    assert_eq(actual: result, expected: 3),
    assert_eq(actual: final_state, expected: 2),
)
```

### Alternative Syntax: Inline State Declaration

A cleaner syntax that integrates with existing `with...in`:

```ori
@test_counter () -> void = run(
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

### How It Works

1. The `handler(state: S)` expression creates a stateful handler frame
2. Each operation receives the current state as its first argument
3. Each operation returns a tuple `(next_state, return_value)`
4. The state is threaded through operations sequentially (left to right in the `run` block)
5. The `with...in` expression can optionally return `(body_result, final_state)`

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

### Implementation Requirements

- **Grammar change**: Extend `with_expr` to accept `handler(state: expr) { ... }` as the binding expression
- **Type checker**: Verify handler operations match trait signature, infer state type
- **Evaluator**: Thread state through handler calls within the `with...in` scope
- **No new types**: No `TestCell`, no interior mutability

### Evaluation

| Criterion | Assessment |
|-----------|------------|
| Value semantics consistency | **Preserves.** State is hidden in the handler frame, not shared. No aliasing. The calling code sees pure operations. |
| Implementation complexity | **High.** Requires changes to `with...in` evaluation, state threading, new syntax. |
| Ergonomics | **Good.** Once learned, the pattern is concise and expressive. |
| Language changes | **Yes.** Grammar, type checker, evaluator. |
| Library-only | **No.** Requires compiler support. |

### Alignment with Ori's Design

This approach aligns with Ori's existing design trajectory:

1. **Capabilities are already effect-like** -- `with...in` is a handler
2. **Value semantics preserved** -- state is frame-local, not shared
3. **Explicit effects** -- the `handler(state: ...)` syntax makes statefulness visible
4. **No new types** -- reuses existing capability machinery
5. **Koka precedent** -- well-studied semantics from effect handler research

### Risks

- Significant implementation effort
- New syntax to learn
- The `(next_state, return_value)` convention may be unfamiliar
- Interaction with nested `with...in` must be carefully specified

---

## Approach 3: Event Recording Pattern (Pure)

### Design

Instead of mutable state, record events and assert on the log:

```ori
@test_counter () -> void = run(
    let (mock, log) = recording_counter(initial: 0),
    let result = with Counter = mock in run(
        let a = Counter.increment(),
        let b = Counter.increment(),
        a + b,
    ),
    assert_eq(actual: log.calls(), expected: ["increment", "increment"]),
)
```

### The Fundamental Problem

This approach has the same fundamental problem it claims to solve. The `log` must be shared between the mock (which writes to it) and the test (which reads from it). In a value-semantic language, the `log` returned by `recording_counter` is a snapshot -- it will not reflect calls made through `mock` after the binding.

For this to work, `log` would need to be... a `TestCell<[str]>`. Which is Approach 1.

Alternatively, `recording_counter` could return the log as part of the `with...in` result:

```ori
@test_counter () -> void = run(
    let (result, log) = with Counter = recording(initial: 0) in run(
        let a = Counter.increment(),
        let b = Counter.increment(),
        a + b,
    ),
    assert_eq(actual: result, expected: 3),
    assert_eq(actual: log.calls(), expected: ["increment", "increment"]),
)
```

But this is just Approach 2 (stateful handler) with the state being a call log instead of a counter.

### Evaluation

| Criterion | Assessment |
|-----------|------------|
| Value semantics consistency | **Impossible without another mechanism.** Requires shared mutable log. |
| Implementation complexity | **N/A.** Reduces to Approach 1 or 2. |
| Ergonomics | **N/A.** |
| Language changes | **N/A.** |
| Library-only | **N/A.** |

### Verdict

Approach 3 is not a standalone solution. It is a use case for either Approach 1 or 2.

---

## Approach 4: Monadic State Threading

### Design

Thread state explicitly through each call, returning updated state alongside each result:

```ori
@test_counter () -> void = run(
    let mock = MockCounter { value: 0 },
    let (mock, a) = mock.increment(),  // returns (MockCounter { value: 1 }, 1)
    let (mock, b) = mock.increment(),  // returns (MockCounter { value: 2 }, 2)
    assert_eq(actual: a + b, expected: 3),
)
```

### Mock Implementation

```ori
type MockCounter = { value: int }

impl Counter for MockCounter {
    @increment (self) -> (MockCounter, int) = run(
        let new_value = self.value + 1,
        (MockCounter { value: new_value }, new_value),
    )
    @get (self) -> (MockCounter, int) = (self, self.value)
}
```

### The Fundamental Problem

This approach works for direct calls but **cannot work through `with...in`**. Capability trait methods have fixed signatures defined by the trait:

```ori
trait Counter {
    @increment (self) -> int
    @get (self) -> int
}
```

The return type is `int`, not `(MockCounter, int)`. The mock implementation must conform to the trait signature. There is no way to thread updated state back through the capability dispatch.

The test in the "Design" section above does not use `with...in` at all -- it calls methods directly on the mock. This works today without any language changes, but it does not test capability provision.

### When This Pattern Is Sufficient

This pattern works when:
- Testing pure logic that happens to use a data structure resembling a mock
- No `with...in` capability provision is involved
- The test author is willing to manually thread state

### Evaluation

| Criterion | Assessment |
|-----------|------------|
| Value semantics consistency | **Preserves.** Pure state threading. |
| Implementation complexity | **None.** Works today. |
| Ergonomics | **Poor.** Manual state threading is tedious and error-prone. |
| Language changes | **None.** |
| Library-only | **Yes.** No compiler changes needed. |
| Works with `with...in` | **No.** Cannot thread state through capability dispatch. |

### Verdict

Approach 4 is a workaround for simple cases that does not solve the actual problem (stateful mocks via `with...in`).

---

## Comparative Analysis

| Criterion | Approach 1: TestCell | Approach 2: Stateful Handlers | Approach 3: Event Log | Approach 4: State Threading |
|-----------|---------------------|------------------------------|----------------------|---------------------------|
| Value semantics | Violates | Preserves | Requires 1 or 2 | Preserves |
| Works with `with...in` | Yes | Yes | Requires 1 or 2 | No |
| Implementation cost | Moderate | High | N/A | None |
| Ergonomics | Good | Good | N/A | Poor |
| Language changes | Yes | Yes | N/A | None |
| Precedent | Rust's Cell | Koka's handlers | N/A | Haskell's State monad |
| Test-only restriction | Needs enforcement | Natural (handler scope) | N/A | N/A |
| Shared mutable state | Yes (problematic) | No (frame-local) | N/A | No |

---

## Recommendation: Approach 2 -- Stateful Effect Handlers

### Rationale

**Approach 2 is the only option that both solves the problem and preserves Ori's value semantics invariants.**

1. **Value semantics preserved.** The state lives in the handler frame, not in a shared mutable cell. No aliasing, no shared mutable references, no violation of ARC safety invariants. The calling code (`Counter.increment()`) is pure -- the state threading is an implementation detail of the handler.

2. **Natural extension of existing design.** Ori's `with...in` already functions as an effect handler. Adding state to handlers is the natural next step, well-studied in the effect handler literature (Koka, Eff, Frank, Multicore OCaml).

3. **No special types or exceptions.** Unlike `TestCell`, stateful handlers do not introduce a new kind of type with different semantics. They extend existing capability machinery.

4. **Not test-only.** Stateful handlers are useful beyond testing -- any `with...in` scope could benefit from handler-local state. This avoids the "test-only feature" problem where restrictions inevitably leak.

5. **Scope-bounded.** Handler state is scoped to the `with...in` block. When the block ends, the state is gone. No lifecycle management, no cleanup, no leaks.

### What About MockClock?

The spec's `MockClock.advance()` pattern (interior mutability in a runtime-provided type) would become a special case of stateful handlers:

```ori
@test_expiry tests @is_expired () -> void = run(
    let result = with Clock = handler(state: Instant.from_unix_secs(secs: 1700000000)) {
        now: (s) -> (s, s),
        advance: (s, by: Duration) -> run(
            let new_time = s + by,
            (new_time, ()),
        ),
    } in run(
        assert(!is_expired(token: token)),
        Clock.advance(by: 1h),
        assert(is_expired(token: token)),
    ),
)
```

This eliminates the need for a runtime-provided `MockClock` type entirely. Users can build their own stateful clock mock using the handler mechanism.

### Phased Implementation

The full stateful handler system is a significant undertaking. A phased approach reduces risk:

**Phase 1: Design and specification** (this proposal)
- Finalize syntax
- Specify state threading semantics
- Specify interaction with nested `with...in`
- Specify type checking rules

**Phase 2: Evaluator support**
- Implement handler frame state in the evaluator
- Thread state through capability dispatch calls
- Return `(result, final_state)` from `with...in`

**Phase 3: Type checker support**
- Verify handler operations match trait signature
- Infer state type from `initial` expression
- Verify state type consistency across operations

**Phase 4: Syntax refinement**
- Finalize surface syntax based on experience
- Add sugar for common patterns (e.g., call recording)

---

## Impact on the Capability Spec

### Changes to `14-capabilities.md`

1. **Add "Stateful Handlers" section** after "Providing Capabilities":
   - Handler syntax and semantics
   - State threading model
   - Scope and lifetime rules
   - Interaction with nested handlers

2. **Update `MockClock` example** to use stateful handlers instead of asserting interior mutability for a user-defined type.

3. **Add error codes** for handler-related diagnostics:
   - Handler operation signature mismatch
   - Missing operations in handler
   - State type inconsistency

### Changes to `grammar.ebnf`

Extend `with_expr` to accept handler expressions:

```ebnf
with_expr = "with" capability_binding { "," capability_binding } "in" expression .
capability_binding = identifier "=" expression .
handler_expr = "handler" "(" "state" ":" expression ")" "{" handler_operations "}" .
handler_operations = handler_operation { "," handler_operation } .
handler_operation = identifier ":" expression .
```

### Changes to `05-variables.md`

No changes. Handler state is internal to the handler frame and does not affect the variable binding model.

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

### Under Approach 1 (TestCell):

```ori
@test_with_expression_body tests @with_expression_body () -> void = run(
    let cell = TestCell(0),
    let mock = MockCounter { cell: cell },
    let result = with Counter = mock in run(
        let a = Counter.increment(),
        let b = Counter.increment(),
        a + b,
    ),
    assert_eq(actual: result, expected: 3),
    assert_eq(actual: cell.get(), expected: 2),
)
```

### Under Approach 2 (Stateful Handlers):

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

### Under Approach 4 (State Threading, no `with...in`):

```ori
@test_counter_threading () -> void = run(
    let mock = MockCounter { value: 0 },
    let (mock, a) = mock.increment_stateful(),
    let (mock, b) = mock.increment_stateful(),
    assert_eq(actual: a + b, expected: 3),
)
// Note: this does NOT test capability provision via with...in
```

---

## Open Questions

1. **Syntax**: Should stateful handlers use `handler(state: expr) { ... }` or a different syntax? Should the state parameter be named or positional?

2. **Multiple state values**: Should handlers support multiple independent state variables, or should users compose them into a single struct/tuple?

3. **State return**: Should `with...in` with a stateful handler always return `(result, final_state)`, or should final state access be opt-in?

4. **Non-test usage**: Should stateful handlers be restricted to test code, or available everywhere? The recommendation is "available everywhere" since the mechanism is sound and useful for capability configuration beyond testing.

5. **Interaction with `def impl`**: Can a `def impl` be stateful? Probably not -- `def impl` is stateless by design (no `self`). Stateful handlers are for `with...in` scopes.

6. **Trait method signature**: Handler operations receive state as a hidden first argument. How does this interact with trait methods that already have `self`? The state and `self` serve different purposes -- `self` is the handler instance, state is the handler's mutable context. For stateful handlers, `self` may not be needed since there is no handler instance.

---

## Summary

| Aspect | Decision |
|--------|----------|
| Recommended approach | Stateful effect handlers (Approach 2) |
| Value semantics | Preserved -- state is frame-local |
| Shared mutable state | None -- state lives in handler frame |
| Compiler changes | Grammar, type checker, evaluator |
| New types | None |
| Test-only restriction | Not needed -- mechanism is sound for all code |
| Precedent | Koka's effect handlers with `var` |
| Primary use case | Stateful mock testing via `with...in` |
| Secondary use case | Any capability that benefits from handler-local state |
| Replaces | Runtime-only `MockClock` interior mutability |
