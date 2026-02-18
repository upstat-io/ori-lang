---
title: "Capabilities"
description: "Ori Language Specification — Capabilities"
order: 14
section: "Verification"
---

# Capabilities

Capabilities are traits representing effects or suspension.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (uses_clause), EXPRESSIONS (with_expr)

## Declaration

```ori
@fetch (url: str) -> Result<Response, Error> uses Http = Http.get(url)

@save (data: str) -> Result<void, Error> uses FileSystem, Async =
    FileSystem.write(path: "/data.txt", content: data)
```

## Capability Traits

Capabilities are traits with default implementations:

```ori
pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    @post (url: str, body: str) -> Result<Response, Error> = ...
}
```

Import the trait to use the default:

```ori
use std.net.http { Http }

@fetch () -> Result<str, Error> uses Http =
    Http.get(url: "https://api.example.com/data")
```

Other standard capability traits:

```ori
trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
}

trait Print {
    @print (msg: str) -> void
    @println (msg: str) -> void
    @output () -> str
    @clear () -> void
}
```

## Suspend Capability

`Suspend` is a marker capability indicating a function may suspend. A function with `uses Suspend` requires a suspending context to execute.

| With `uses Suspend` | Without |
|---------------------|---------|
| Non-blocking, may suspend | Blocking, synchronous |

```ori
@fetch_suspending (url: str) -> Result<Data, Error> uses Http, Suspend = ...  // may suspend
@fetch_sync (url: str) -> Result<Data, Error> uses Http = ...                 // blocks
```

No `async` type modifier. No `.await` expression. Return type is final value type.

Concurrency via `parallel` pattern:

```ori
parallel(tasks: [fetch(a), fetch(b)], max_concurrent: 10)
```

See [Concurrency Model](23-concurrency-model.md) for task definitions, suspending context semantics, and suspension points.

## Providing Capabilities

Default implementations are automatically bound when importing a trait. Override with `with...in` when custom configuration or mocking is needed:

```ori
with Http = ConfiguredHttp { timeout: 5s } in
    fetch("/data")
```

### Multi-Binding Syntax

Multiple capabilities may be bound in a single `with` expression using comma-separated bindings:

```ori
with Http = mock_http, Cache = mock_cache in
    complex_operation()
```

This is equivalent to nested `with` expressions:

```ori
with Http = mock_http in
    with Cache = mock_cache in
        complex_operation()
```

### Partial Provision

When a function requires multiple capabilities, some may be explicitly provided while others use defaults:

```ori
def impl Http { ... }
def impl Cache { ... }
def impl Logger { ... }

@test_with_mock_http () -> void = run(
    let mock = MockHttp { ... },

    with Http = mock in
        complex_operation(),  // MockHttp + default Cache + default Logger
)
```

Only `Http` is overridden; `Cache` and `Logger` use their `def impl`.

### Nested Binding Semantics

Inner bindings shadow outer bindings within their scope:

```ori
with Http = OuterHttp in run(
    use_http(),  // OuterHttp

    with Http = InnerHttp in
        use_http(),  // InnerHttp (shadows Outer)

    use_http(),  // OuterHttp again
)
```

`with` creates a lexical scope — bindings are visible only within:

```ori
let result = with Http = mock in fetch()
// mock is NOT bound here
fetch()  // Uses default Http, not mock
```

## Stateful Handlers

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § EXPRESSIONS (handler_expr)
>
> **Proposal:** [stateful-mock-testing-proposal.md](../../../proposals/approved/stateful-mock-testing-proposal.md)

A _stateful handler_ is a `with...in` binding that threads local mutable state through handler operations. The `handler(state: expr) { ... }` construct creates a handler frame with frame-local state, enabling stateful capability mocking while preserving value semantics.

`handler` is a context-sensitive keyword, valid only in the expression position of a `capability_binding`.

### Syntax

```ori
with Counter = handler(state: 0) {
    increment: (s) -> (s + 1, s + 1),
    get: (s) -> (s, s),
} in run(
    let a = Counter.increment(),  // state: 0 -> 1, returns 1
    let b = Counter.increment(),  // state: 1 -> 2, returns 2
    a + b,                        // 3
)
```

### Semantics

1. The `state:` initializer determines the initial state value and its type `S`
2. Each handler operation receives the current state as its first argument, replacing `self`
3. Each handler operation must return `(S, R)` where `S` is the next state and `R` is the trait method's return type
4. State is threaded through operations sequentially within the `with...in` scope
5. The `with...in` expression returns the body's type; handler state is internal

### Type Checking Rules

For a handler operation named `op` implementing trait method `@op (self, p1: T1, ..., pN: TN) -> R`:

- The handler operation receives `(state: S, p1: T1, ..., pN: TN)`
- The handler operation must return `(S, R)`
- The state type `S` is inferred from the `state:` initializer
- All handler operations must use the same state type
- Every required trait method must have a corresponding handler operation; default trait methods are used if not overridden
- Handler operations for non-existent trait methods are an error

A stateful handler is not a type and has no `self`. It satisfies the trait's interface for the duration of the `with...in` scope through a distinct dispatch mechanism from `impl` blocks.

### State Composition

Handlers support a single state value. Multiple independent state values are composed into a struct or tuple:

```ori
with Counter = handler(state: { count: 0, log: [] }) {
    increment: (s) -> ({ ...s, count: s.count + 1, log: [...s.log, "inc"] }, s.count + 1),
    get: (s) -> (s, s.count),
} in ...
```

### Nested Handlers

Each handler maintains independent state. Cross-handler calls dispatch through normal capability resolution:

```ori
with Logger = handler(state: []) {
    log: (s, msg: str) -> ([...s, msg], ()),
} in
    with Counter = handler(state: 0) {
        increment: (s) -> run(
            Logger.log(msg: "increment"),  // invokes outer handler
            (s + 1, s + 1),
        ),
    } in ...
```

### Restrictions

- `def impl` cannot be stateful (stateless by design, no scope for state lifetime)
- Stateful handlers are available in all `with...in` scopes (not restricted to test code)

## Capability Variance

A context with more capabilities may call functions requiring fewer:

```ori
@needs_http () -> void uses Http = ...
@needs_both () -> void uses Http, Cache = ...

@caller () -> void uses Http, Cache = run(
    needs_http(),  // OK: caller has Http
    needs_both(),  // OK: caller has both
)
```

A function requiring more capabilities cannot be called from one with fewer:

```ori
@needs_both () -> void uses Http, Cache = ...

@caller () -> void uses Http = run(
    needs_both(),  // ERROR: caller lacks Cache
)
```

## Propagation

Capabilities propagate: if A calls B with capability C, A must declare or provide C.

```ori
@helper () -> str uses Http = Http.get("/").body

// Must declare Http
@caller () -> str uses Http = helper()

// Or provide it
@caller () -> str = with Http = MockHttp {} in helper()
```

## Standard Capabilities

| Capability | Purpose | May Suspend |
|------------|---------|-------------|
| `Http` | HTTP client | Yes |
| `FileSystem` | File I/O | Yes |
| `Cache` | Caching | Yes |
| `Clock` | Time | No |
| `Random` | RNG | No |
| `Crypto` | Cryptographic operations | No |
| `Print` | Standard output | No |
| `Logger` | Structured logging | No |
| `Env` | Environment | No |
| `Intrinsics` | Low-level SIMD and bit operations | No |
| `FFI` | Foreign function interface | No |
| `Suspend` | Suspension marker | Yes |

### Cache Capability

The `Cache` capability provides key-value caching with TTL-based expiration:

```ori
trait Cache {
    @get<K: Hashable + Eq, V: Clone> (self, key: K) -> Option<V>
    @set<K: Hashable + Eq, V: Clone> (self, key: K, value: V, ttl: Duration) -> void
    @invalidate<K: Hashable + Eq> (self, key: K) -> void
    @clear (self) -> void
}
```

Used by the `cache` pattern (see [Patterns § cache](10-patterns.md#cache)).

Cache implementations differ in suspension behavior:

| Implementation | Description | Suspends |
|----------------|-------------|----------|
| `InMemoryCache` | Process-local | No |
| `DistributedCache` | Shared across nodes | Yes |
| `NoOpCache` | Disable caching (always miss) | No |

When using a suspending cache implementation, the calling function must have `uses Suspend` or be called from a suspending context.

### Clock Capability

The `Clock` capability provides access to the current time:

```ori
trait Clock {
    @now () -> Instant
    @local_timezone () -> Timezone
}
```

`Instant` and `Timezone` are defined in `std.time`. Functions requiring current time must declare `uses Clock`:

```ori
@log_timestamp (msg: str) -> void uses Clock, Print =
    print(msg: `[{Clock.now()}] {msg}`)
```

Mock clocks enable deterministic testing via _stateful handlers_:

```ori
@test_expiry tests @is_expired () -> void = run(
    let start = Instant.from_unix_secs(secs: 1700000000),
    with Clock = handler(state: start) {
        now: (s) -> (s, s),
        advance: (s, by: Duration) -> (s + by, ()),
    } in run(
        assert(!is_expired(token: token)),
        Clock.advance(by: 1h),
        assert(is_expired(token: token)),
    ),
)
```

The `handler(state: expr) { ... }` construct creates a stateful handler frame with local mutable state threaded through operations. State is frame-local and does not violate value semantics. See [Stateful Handlers](#stateful-handlers) for the full specification.

### Crypto Capability

The `Crypto` capability provides cryptographic operations:

```ori
trait Crypto {
    @hash (data: [byte], algorithm: HashAlgorithm) -> [byte]
    @hash_password (password: str) -> str
    @verify_password (password: str, hash: str) -> bool
    @generate_key () -> SecretKey
    @encrypt (key: SecretKey, plaintext: [byte]) -> [byte]
    @decrypt (key: SecretKey, ciphertext: [byte]) -> Result<[byte], CryptoError>
    @random_bytes (count: int) -> [byte]
    // ... additional methods defined in std.crypto
}
```

Types and functions are defined in `std.crypto`. The `Crypto` capability is non-suspending — cryptographic operations are CPU-bound and complete synchronously.

Key types are separated by purpose to prevent misuse at compile time:
- `SigningPrivateKey` / `SigningPublicKey` — for digital signatures
- `EncryptionPrivateKey` / `EncryptionPublicKey` — for asymmetric encryption
- `KeyExchangePrivateKey` / `KeyExchangePublicKey` — for Diffie-Hellman key exchange

Private key types (`SecretKey`, `SigningPrivateKey`, `EncryptionPrivateKey`, `KeyExchangePrivateKey`) automatically zero their memory when dropped.

### Intrinsics Capability

The `Intrinsics` capability provides low-level SIMD operations, bit manipulation, and hardware feature detection:

```ori
trait Intrinsics {
    // SIMD operations (examples for 4-wide float)
    @simd_add_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_mul_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_sum_f32x4 (a: [float, max 4]) -> float
    // ... additional widths: f32x8, f32x16, i64x2, i64x4

    // Bit operations
    @count_ones (value: int) -> int
    @count_leading_zeros (value: int) -> int
    @count_trailing_zeros (value: int) -> int
    @rotate_left (value: int, amount: int) -> int
    @rotate_right (value: int, amount: int) -> int

    // Hardware queries
    @cpu_has_feature (feature: str) -> bool
}
```

SIMD operations work on fixed-capacity lists representing vector registers:

| Width | Float Type | Int Type | Platforms |
|-------|------------|----------|-----------|
| 128-bit | `[float, max 4]` | `[int, max 2]` | SSE, NEON, SIMD128 |
| 256-bit | `[float, max 8]` | `[int, max 4]` | AVX, AVX2 |
| 512-bit | `[float, max 16]` | — | AVX-512 |

The default `def impl Intrinsics` uses native SIMD instructions when available and falls back to scalar emulation otherwise. For testing, `EmulatedIntrinsics` always uses scalar operations.

Feature detection via `cpu_has_feature` accepts platform-specific feature strings:

| Platform | Features |
|----------|----------|
| x86_64 | `"sse"`, `"sse2"`, `"sse3"`, `"sse4.1"`, `"sse4.2"`, `"avx"`, `"avx2"`, `"avx512f"` |
| aarch64 | `"neon"` |
| wasm32 | `"simd128"` |

Unknown feature strings cause a panic.

## Default Capabilities

`Print` has a default implementation via `def impl`. Programs may use `print` without declaring `uses Print`:

```ori
@main () -> void = print(msg: "Hello, World!")
```

The default is `StdoutPrint` for native execution, `BufferPrint` for WASM.

### Name Resolution

When resolving a capability name, the compiler checks in order:

1. **Innermost `with...in` binding** — highest priority
2. **Outer `with...in` bindings** — in reverse nesting order
3. **Imported `def impl`** — from the module where the trait is defined
4. **Module-local `def impl`** — defined in the current module
5. **Error** — capability not provided

When both an imported `def impl` and a module-local `def impl` exist for the same capability, imported takes precedence.

### Suspend Binding Prohibition

`Suspend` is a marker capability — it has no methods and cannot be provided via `with...in`. Attempting to bind `Suspend` is a compile-time error:

```ori
with Suspend = SomeImpl in  // ERROR: Suspend cannot be bound
    suspending_fn()
```

Suspending context is provided by:
- The runtime for `@main () uses Suspend`
- Concurrency patterns: `parallel`, `spawn`, `nursery`

## Testing

```ori
@test_fetch tests @fetch () -> void =
    with Http = MockHttp { responses: {"/users/1": "{...}"} } in
    run(
        assert_ok(result: fetch(url: "/users/1")),
    )
```

Mock implementations are synchronous; test does not need `Suspend`.

## Named Capability Sets (`capset`)

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (capset_decl)

A _capset_ is a named, transparent alias for a set of capabilities. Capsets are expanded to their constituent capabilities during name resolution, before type checking. A capset is not a trait, not a type, and has no runtime representation.

```ori
capset Net = Http, Dns, Tls
capset Runtime = Clock, Random, Env
capset WebService = Net, Runtime, Database, Suspend
```

### Usage

Capsets may appear in `uses` clauses and other `capset` declarations. Capsets and individual capabilities may be mixed:

```ori
@fetch (url: str) -> Result<str, Error> uses Net, Logger, Suspend = ...
```

After expansion, `uses Net, Logger, Suspend` is equivalent to `uses Http, Dns, Tls, Logger, Suspend`.

### Expansion

The compiler expands capsets transitively and deduplicates the result. The expanded set uses set semantics — duplicates are eliminated and order is irrelevant:

```ori
capset Net = Http, Dns
capset Infra = Net, Logger

// Valid — `uses Infra, Http` expands to `uses Http, Dns, Logger`
@fn () -> void uses Infra, Http = ...
```

### Restrictions

A capset declaration:

- Must contain at least one member
- Must not form a cycle with other capset declarations
- Must not share a name with a trait in the same scope
- Members must be capability traits or other capsets

A capset is not a trait. It cannot be used in `impl` blocks, `def impl` declarations, or `with...in` bindings:

```ori
// Invalid — capsets cannot be bound
with Net = something in ...  // error

// Invalid — capsets are not traits
impl Net for SomeType { ... }  // error
def impl Net { ... }           // error
```

### Visibility

Capsets follow standard visibility rules. A `pub` capset must not reference non-accessible capabilities:

```ori
pub capset Net = Http, Dns, Tls       // Valid — all members accessible
pub capset Bad = SomePrivateTrait      // Invalid — member not accessible
```

### Variance Interaction

Capability variance operates on the expanded set:

```ori
capset Runtime = Clock, Random, Env

@needs_clock () -> void uses Clock = ...

@caller () -> void uses Runtime = run(
    needs_clock(),  // Valid — Runtime includes Clock
)
```

## Purity

Functions without `uses` are pure: no side effects, cannot suspend, safely parallelizable.

## Errors

| Code | Description |
|------|-------------|
| E0600 | Function uses capability without declaring it |
| E1000 | Conflicting default implementations for same trait |
| E1001 | Duplicate default implementation in same module |
| E1002 | `def impl` methods cannot have `self` parameter |
| E1200 | Missing capability (callee requires capability caller lacks) |
| E1201 | Unbound capability (no `with` or `def impl` available) |
| E1202 | Type does not implement capability trait |
| E1203 | `Suspend` capability cannot be explicitly bound |
| E1204 | Handler missing required operation (trait method not defined in handler) |
| E1205 | Handler operation signature mismatch (parameters or return type) |
| E1206 | Handler state type inconsistency (operations return different state types) |
| E1207 | Handler operation for non-existent trait method |
| E1220 | Cyclic capset definition |
| E1221 | Empty capset |
| E1222 | Capset name collides with trait name |
| E1223 | Capset member is not a capability trait or capset |

```
error[E0600]: function uses `Http` without declaring it
```

Capabilities must be explicitly declared or provided.
