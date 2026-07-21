# Proposal: Standard Library Random Number Generation (`std.math.rand`)

**Status:** Draft
**Author:** Eric
**Created:** 2026-07-20
**Affects:** stdlib, capabilities, spec
**Depends On:** (none — `stdlib-crypto-ffi-native-proposal.md` covers cryptographic randomness separately)

---

## Summary

Specify a general-purpose random number generation API for the standard library. Today `library/std/math/rand/mod.ori` is an empty `// TODO` sketch and the prelude declares `trait Random { @rand_int; @rand_float }` with no `def impl`, so no Ori program can generate random numbers. This proposal defines: (1) a pure, seedable `Rng` value type with a specified, reproducible PRNG algorithm; (2) capability-gated global convenience functions for system-seeded randomness; and (3) the `Random` capability provider shape.

---

## Motivation

Random number generation is table-stakes for simulations, shuffles, sampling, procedural generation, randomized algorithms, and property-based testing. Ori cannot express any of them today.

### The Problem in Practice

```ori
// Today: impossible. rand() does not exist; `trait Random` has no implementation.
@shuffle_deck () -> [int] uses Random = {
    let deck = for i in 1..=52 yield i;
    shuffle(items: deck)          // no such function
}
```

The `100 prisoners` problem, Monte Carlo integration, reservoir sampling, and Fisher-Yates shuffles are all blocked. There is no way to write a probabilistic program in Ori.

### When This Matters

Any simulation, game, statistical method, or randomized data structure. It also blocks reproducible testing: without a *seedable* generator, tests cannot pin a deterministic random sequence.

---

## Goals and Non-Goals

**Goals:**
- A pure, value-semantic, **seedable** `Rng` whose sequence is deterministic and bit-identical across runs and platforms for a given seed.
- Capability-gated global helpers (`random`, `random_int`, `shuffle`, `choice`, `sample`) for system-seeded convenience.
- A specified default PRNG algorithm (not "implementation-defined") so seeds reproduce exactly.

**Non-Goals:**
- **Cryptographic** randomness — owned by `std.crypto.rand` (`stdlib-crypto-ffi-native-proposal.md`). This generator is explicitly NOT cryptographically secure and must document that.
- Distributions beyond uniform (normal, Poisson, etc.) — a follow-up.
- Parallel/splittable streams — noted as an unresolved question, not specified here.

---

## Design

Two layers: a **pure seeded core** (`Rng`) and a **capability** (`Random`) for system entropy. Value semantics mean an `Rng` is threaded explicitly (returned alongside each draw), so pure code stays pure.

### Syntax

Pure, seedable core (no capability — reproducible + testable):

```ori
type Rng = { ... }   // opaque PRNG state, a Value type (inline, bitwise-copy)

impl Rng {
    @new (seed: int) -> Rng                              // seed -> initial state
    @next_int (self, min: int, max: int) -> (int, Rng)   // uniform in [min, max)
    @next_float (self) -> (float, Rng)                   // uniform in [0.0, 1.0)
    @next_bool (self) -> (bool, Rng)
}
```

Because `Rng` is a `Value` and Ori has no in-place mutation, each draw returns the drawn value **and the advanced generator**:

```ori
@roll_two (seed: int) -> (int, int) = {
    let rng = Rng.new(seed: seed);
    let (a, rng) = rng.next_int(min: 1, max: 7);
    let (b, _)   = rng.next_int(min: 1, max: 7);
    (a, b)
}
```

Capability-gated global helpers (system-seeded; require `uses Random`):

```ori
@random () -> float uses Random                    // [0.0, 1.0)
@random_int (min: int, max: int) -> int uses Random
@random_bool () -> bool uses Random
@shuffle<T> (items: [T]) -> [T] uses Random        // Fisher-Yates
@choice<T> (items: [T]) -> T uses Random
@sample<T> (items: [T], n: int) -> [T] uses Random
```

Provided the standard way capabilities are (per spec clause 20):

```ori
with Random = SystemRandom in {
    let deck = shuffle(items: for i in 1..=52 yield i)   // system-seeded
}
// or seed it for reproducibility:
with Random = SeededRandom(seed: 42) in { ... }
```

### Semantics

- **PRNG algorithm (specified, not implementation-defined).** Recommended: seed expansion via **SplitMix64**, main stream via **xoshiro256\*\*** (both public-domain, fast, well-tested, and small enough to specify exactly). The exact constants and update function are pinned in the spec so a given seed reproduces bit-identically on every platform. (Final algorithm selection is an Unresolved Question — PCG64 is the main alternative.)
- `Rng.new(seed:)` runs SplitMix64 to fill the 256-bit xoshiro state from the 64-bit seed (never zero-state).
- `next_int(min:, max:)` uses Lemire's unbiased bounded-integer method (rejection-free fast path) — no modulo bias. `min == max` or `min > max` is an error (see below).
- `next_float()` produces `[0.0, 1.0)` by taking the top 53 bits of a 64-bit draw (uniform double).
- The `Random` capability's `SystemRandom` handler seeds from the OS at `with...in` entry (needs IO); `SeededRandom(seed:)` is a pure deterministic handler with the same algorithm as `Rng`.

### Error Handling

- `next_int(min:, max:)` with `min >= max`: compile-error where statically known (both const), else a runtime panic `E6xxx: rand: empty or inverted range [min, max)`. (Exact code allocated at spec time.)
- `sample(items:, n:)` with `n > len(items)`: panic `E6xxx: rand: sample size exceeds population`.
- `choice(items:)` / `sample(...)` on an empty list: panic.

---

## Drawbacks

- **Surface-area growth**: a new stdlib module + a prelude capability. Mitigated by keeping the core small (one `Rng` type + a handful of methods).
- **Value-semantics threading is verbose**: `let (x, rng) = rng.next_int(...)` everywhere. This is the honest cost of no-in-place-mutation; the capability-gated globals hide it for the common case, and it is exactly the same pattern iterators already use (`@next (self) -> (Option<T>, Self)`).
- **Pinning the algorithm is a forward commitment**: once a seed's sequence is spec'd, it cannot change without a versioned break. This is intended (reproducibility is a goal), but it means the algorithm choice is load-bearing.

---

## Alternatives Considered

### Alternative 1: Implementation-defined PRNG

Leave the algorithm unspecified (like C `rand()`). Rejected — it defeats reproducible testing and cross-platform determinism, both explicit goals. A seed must reproduce a sequence exactly.

### Alternative 2: Capability-only (no pure `Rng`)

Expose only `uses Random` globals, no pure seedable type. Rejected — pure code (const evaluation, deterministic tests, referentially-transparent simulations) needs a generator that is not an ambient effect. The pure `Rng` is the idiomatic core; the capability is the convenience layer over it.

### Alternative 3: Mutable RNG handle

A mutable `Rng` that advances in place. Rejected — violates value semantics / no-in-place-mutation. The `(value, Rng)` return is the Ori-idiomatic shape (mirrors `Iterator`).

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY.
**If not, why:** The seeded core `Rng` (PRNG arithmetic, `next_*`) is **fully pure Ori** — no compiler or capability support needed. Only *system seeding* (reading OS entropy) requires IO, expressed through the `Random` capability handler, not a compiler feature.
**Missing features that would enable purity:** None for the core. System seeding intentionally uses the existing capability mechanism (spec clause 20), not a new compiler primitive.
**Recommendation:** **Hybrid, library-first.** Implement the pure `Rng` + the `Random` `def impl` entirely in stdlib Ori; no compiler change. The capability wiring reuses existing capability infrastructure. This is a pure stdlib addition — the compiler is untouched.

---

## Spec & Grammar Impact

- **New/updated spec content** for `std.math.rand` (currently a stub): the `Rng` type, its methods, the specified PRNG algorithm + constants, and the bounded-integer method.
- **Capability registration**: `Random` joins the standard capability set in clause 20 (`Http`, `FileSystem`, `Clock`, `Random`, ...) — the syntax reference already lists `Random` as a standard capability, so this formalizes it.
- **No grammar change** — `uses Random`, `with Random = ... in ...`, and `impl`/`def impl` are existing productions.

---

## Prior Art

- **Go `math/rand/v2`** — moved to a specified, seedable PCG generator as the default; kept a top-level convenience API plus an explicit `Rand` value seeded via a `Source`. Directly parallels the pure-`Rng`-plus-globals split here.
- **Rust `rand` / `rand_core`** — `RngCore` trait + concrete generators (`StdRng`, `SmallRng` = xoshiro/PCG family); `SeedableRng::seed_from_u64` uses SplitMix64-style expansion — the exact seeding idiom proposed here. Separates crypto (`OsRng`) from fast PRNGs, matching our crypto/math split.
- **Swift `RandomNumberGenerator`** protocol + `SystemRandomNumberGenerator` — the capability-like "system generator vs your own seeded generator" split; the pure-generator-passed-explicitly shape.
- **Java `SplittableRandom` / `RandomGenerator`** (JEP 356) — SplitMix64 core; a specified, reproducible algorithm behind a value-like generator.
- **xoshiro / SplitMix64 (Blackman & Vigna)** — public-domain reference algorithms, small enough to specify exactly, which is why they are the recommended default here.

(Prior-art entries are discovery context, to be verified against the reference sources during `/review-draft-proposal`.)

---

## Unresolved Questions

- **Exact PRNG choice** — xoshiro256\*\* + SplitMix64 seeding (recommended) vs PCG64. Resolve during review; whichever is chosen must be spec'd to bit-level determinism.
- **Idiomatic default** — is the pure `Rng` the primary API with the capability as sugar, or vice versa? (This proposal treats the pure `Rng` as the core and the capability as the convenience layer.)
- **Splittable / parallel streams** — deferred; note whether the chosen algorithm admits a `split()` for independent substreams (xoshiro `jump()` / SplitMix `split()` both do), to avoid a design that forecloses it.
- **Distributions** (normal, exponential, weighted `choice`) — explicitly a follow-up proposal, not this one.
- **`float` precision** — 53-bit `[0.0, 1.0)` double is proposed; confirm no need for `[0.0, 1.0]` inclusive or 64-bit variants.
