# Proposal: Standard Library Random Number Generation (`std.math.rand`)

**Status:** Draft
**Author:** Eric
**Created:** 2026-07-20
**Revised:** 2026-07-20 (dual-harness review — reconciled with the approved `std.crypto` random surface, the prelude `Random` trait, and the existing `rand.md` module doc; PRNG pinned; range convention fixed; pure-core collection methods added)
**Affects:** stdlib, capabilities, spec
**Related:** `stdlib-crypto-api-proposal.md` (approved — owns cryptographic randomness at `std.crypto`; this proposal draws its non-goal boundary against it, see `## Relationship to std.crypto`)

---

## Summary

Specify a general-purpose (non-cryptographic) random number generation API for the standard library. Today `library/std/math/rand/mod.ori` is an empty `// TODO` sketch, the prelude declares `trait Random { @rand_int; @rand_float }` with no `def impl`, and `modules/std.math/rand.md` documents a third, incompatible surface — so no Ori program can generate random numbers and the three declarations disagree. This proposal defines: (1) a pure, seedable `Rng` value type with a pinned, reproducible PRNG algorithm and pure collection methods (`shuffle`/`choice`/`sample`); (2) the canonical `Random` capability trait (superseding the prelude and `rand.md` declarations) plus `SystemRandom` / `SeededRandom` providers; and (3) capability-gated global convenience helpers named to match the prelude trait (`rand_int` / `rand_float` / `rand_bool` / `rand_choice` / `rand_sample` / `rand_shuffle`) so they never collide with `std.crypto`'s `random_int` / `random_bytes`.

---

## Motivation

Random number generation is table-stakes for simulations, shuffles, sampling, procedural generation, randomized algorithms, and property-based testing. Ori cannot express any of them today.

### The Problem in Practice

```ori
// Today: impossible. rand_int() does not exist; `trait Random` has no `def impl`.
@shuffle_deck () -> [int] uses Random = {
    let deck = for i in 1..=52 yield i;
    rand_shuffle(items: deck)          // no such function
}
```

The `100 prisoners` problem, Monte Carlo integration, reservoir sampling, and Fisher-Yates shuffles are all blocked. There is no way to write a probabilistic program in Ori.

### When This Matters

Any simulation, game, statistical method, or randomized data structure. It also blocks reproducible testing: without a *seedable, pure* generator, tests cannot pin a deterministic random sequence.

---

## Goals and Non-Goals

**Goals:**
- A pure, value-semantic, **seedable** `Rng` whose sequence is deterministic and bit-identical across runs and platforms for a given seed.
- Pure `Rng` collection methods (`shuffle` / `choice` / `sample`) so deterministic, capability-free shuffles and sampling (the motivating use cases) are expressible on the pure core.
- The canonical `Random` capability trait (`@rand_int` / `@rand_float` / `@rand_bool`) that supersedes the divergent prelude and `rand.md` declarations, with `SystemRandom` and `SeededRandom` providers.
- Capability-gated global helpers (`rand_int` / `rand_float` / `rand_bool` / `rand_choice` / `rand_sample` / `rand_shuffle`) for system-seeded convenience, named distinctly from `std.crypto`'s `random_*` surface.
- A **pinned** default PRNG algorithm (xoshiro256\*\* + SplitMix64 seeding, exact constants) so seeds reproduce exactly.

**Non-Goals:**
- **Cryptographic** randomness — owned by `std.crypto` (`random_bytes` / `random_int` / `random_uuid` under `uses Crypto`, per the approved `stdlib-crypto-api-proposal.md`). This generator is explicitly NOT cryptographically secure and documents that prominently.
- Distributions beyond uniform (normal, Poisson, etc.) — a follow-up.
- Parallel/splittable streams — noted as an unresolved question, not specified here.

---

## Design

Two layers: a **pure seeded core** (`Rng`) and a **capability** (`Random`) for system entropy. Value semantics mean an `Rng` is threaded explicitly (returned alongside each draw), so pure code stays pure. The capability-gated globals are thin wrappers over the `Random` trait methods.

### The pure seeded core (`Rng`)

Reproducible + testable, no capability. `Rng` is a `Value` type with module-private state (opaque):

```ori
type Rng: Value = { ... }   // module-private xoshiro256** state (256 bits = 4 x int); opaque, inline, bitwise-copy

impl Rng {
    @new (seed: int) -> Rng                              // SplitMix64-expand seed -> 256-bit state
    @next_int (self, min: int, max: int) -> (int, Rng)   // uniform in [min, max] INCLUSIVE
    @next_float (self) -> (float, Rng)                   // uniform in [0.0, 1.0)
    @next_bool (self) -> (bool, Rng)
    @shuffle<T> (self, items: [T]) -> ([T], Rng)          // pure Fisher-Yates
    @choice<T> (self, items: [T]) -> (Option<T>, Rng)     // None on empty
    @sample<T> (self, items: [T], n: int) -> (Option<[T]>, Rng)   // without replacement; None when n<0 or n>len
}
```

Because `Rng` is a `Value` and Ori has no in-place mutation, each draw returns the drawn value **and the advanced generator**:

```ori
@roll_two (seed: int) -> (int, int) = {
    let rng = Rng.new(seed: seed);
    let (a, rng) = rng.next_int(min: 1, max: 6);   // inclusive: 1..6
    let (b, _)   = rng.next_int(min: 1, max: 6);
    (a, b)
}

// Deterministic, capability-free shuffle (what property-based tests want):
@shuffled_deck (seed: int) -> [int] = {
    let rng = Rng.new(seed: seed);
    let (deck, _) = rng.shuffle(items: for i in 1..=52 yield i);
    deck
}
```

### The `Random` capability (canonical trait)

This proposal defines the canonical `Random` capability trait and **supersedes** both the current `prelude.ori` declaration (`{ @rand_int, @rand_float }` — missing `@rand_bool`) and the `modules/std.math/rand.md` declaration (`{ @int, @float, @bool, @bytes }` — wrong names, and `@bytes` belongs to `std.crypto`):

```ori
pub trait Random {
    @rand_int (self, min: int, max: int) -> (int, Self)   // uniform [min, max] inclusive
    @rand_float (self) -> (float, Self)                   // [0.0, 1.0)
    @rand_bool (self) -> (bool, Self)
}
```

Two providers implement it; state is threaded frame-locally via the stateful-handler mechanism (spec clause 20):

```ori
// System-seeded: reads OS entropy ONCE at `with...in` entry (needs IO). NON-cryptographic.
pub type SystemRandom = { ... }
pub impl SystemRandom: Random { ... }

// Deterministic: pure, same xoshiro256** algorithm as `Rng`, seeded explicitly.
pub type SeededRandom = { seed: int }
pub impl SeededRandom: Random { ... }
```

### Capability-gated global helpers

Thin wrappers over the ambient `Random` provider's trait methods (require `uses Random`). Named `rand_*` to match the prelude trait and to avoid collision with `std.crypto`'s `random_*`:

```ori
@rand_int (min: int, max: int) -> int uses Random      // uniform [min, max] inclusive
@rand_float () -> float uses Random                    // [0.0, 1.0)
@rand_bool () -> bool uses Random
@rand_shuffle<T> (items: [T]) -> [T] uses Random        // Fisher-Yates
@rand_choice<T> (items: [T]) -> Option<T> uses Random   // None on empty
@rand_sample<T> (items: [T], n: int) -> Option<[T]> uses Random   // without replacement; None when n<0 or n>len
```

Provided the standard way capabilities are (per spec clause 20):

```ori
with Random = SystemRandom in {
    let deck = rand_shuffle(items: for i in 1..=52 yield i)   // system-seeded
}
// or seed it for reproducibility:
with Random = SeededRandom(seed: 42) in { ... }
```

**No ambient default, no fixed-seed global.** A `uses Random` function invoked with no `with Random = ...` provider in scope is a capability-resolution error (the standard clause-20 behavior for an unprovided capability) — there is NO implicit ambient `def impl` and NO fixed-seed global default. This forecloses the Go `math/rand` pre-1.20 regret (a global RNG defaulting to a fixed seed silently returns the same sequence every run; Go switched to auto-seeding and deprecated `Seed()`). If a future revision adds a module default provider, it MUST auto-seed from OS entropy; a fixed-seed ambient default is forbidden by this proposal.

### Semantics

- **PRNG algorithm — PINNED (not implementation-defined).** Seed expansion via **SplitMix64**; main stream via **xoshiro256\*\*** (Blackman & Vigna; public-domain, fast, small enough to specify exactly). The exact constants and update function are pinned in the spec so a given seed reproduces bit-identically on every platform. The 256-bit xoshiro state is filled from the 64-bit seed by iterating SplitMix64 four times (never an all-zero state).
- `next_int(min:, max:)` / `rand_int(min:, max:)` draw uniformly in **`[min, max]` inclusive** (matching the shipped `rand.md` d6/coin idioms and the approved `std.crypto` `random_int` convention), using Lemire's unbiased bounded-integer method over the range size `span = max - min + 1` (result = `min + lemire(span)`). Rejection-free fast path — no modulo bias.
- `next_float()` / `rand_float()` produce `[0.0, 1.0)` by taking the top 53 bits of a 64-bit draw (a uniform double).
- `shuffle` is Fisher-Yates over `rand_int`. `sample` draws `n` distinct elements **without replacement**.
- `SystemRandom` seeds from the OS at `with...in` entry (needs IO) and is **non-cryptographic**; `SeededRandom(seed:)` is a pure deterministic provider using the same algorithm as `Rng`.

### Error Handling

- `next_int(min:, max:)` / `rand_int(min:, max:)` with `min > max`: runtime panic `E6xxx: rand: inverted range [min, max]` (exact code allocated at spec time). `min == max` is the **valid singleton draw** (returns `min`), not an error — a consequence of the inclusive convention. No compile-time diagnostic is emitted (that would require compiler static analysis, contradicting the library-first purity claim; see Purity Analysis).
- `choice` / `rand_choice` on an empty list: returns `None` (total, no panic).
- `sample` / `rand_sample` with `n < 0` or `n > len(items)`: returns `None` (total, no panic).

---

## Drawbacks

- **Surface-area growth**: a new stdlib module + the `Random` capability trait + two providers. Mitigated by keeping the core small (one `Rng` type + a handful of methods) and by the globals being thin wrappers.
- **Value-semantics threading is verbose**: `let (x, rng) = rng.next_int(...)` everywhere. This is the honest cost of no-in-place-mutation; the capability-gated globals hide it for the common case, and it mirrors the pattern iterators already use (`@next (self) -> (Option<T>, Self)`).
- **Pinning the algorithm is a forward commitment**: once a seed's sequence is spec'd, it cannot change without a versioned break. This is intended (reproducibility is a goal), but it means the algorithm choice is load-bearing.
- **Supersedes existing declarations**: the current `prelude.ori` `Random` trait and the `rand.md` module doc are both replaced, which is a doc + prelude edit (see Spec & Grammar Impact).

---

## Alternatives Considered

### Alternative 1: Implementation-defined PRNG

Leave the algorithm unspecified (like C `rand()`). Rejected — it defeats reproducible testing and cross-platform determinism, both explicit goals. A seed must reproduce a sequence exactly.

### Alternative 2: Capability-only (no pure `Rng`)

Expose only `uses Random` globals, no pure seedable type. Rejected — pure code (const evaluation, deterministic tests, referentially-transparent simulations) needs a generator that is not an ambient effect, AND it needs the collection operations (shuffle/sample) on that pure generator, not only on the capability layer. The pure `Rng` with its own `shuffle`/`choice`/`sample` is the idiomatic core; the capability is the convenience layer over it.

### Alternative 3: Mutable RNG handle

A mutable `Rng` that advances in place. Rejected — violates value semantics / no-in-place-mutation. The `(value, Rng)` return is the Ori-idiomatic shape (mirrors `Iterator`).

### Alternative 4: Exclusive `[min, max)` range

Match Rust's `gen_range` half-open convention (more composable for length-based indexing). Rejected — the shipped `rand.md` (d6 = `rand_int(1, 6)`, coin = `rand_int(0, 1)`) and the approved `std.crypto` `random_int(min: 100000, max: 999999)` both already use the **inclusive** `[min, max]` convention; a silent flip to exclusive would change the meaning of existing documented idioms and diverge from the crypto sibling. Consistency across the stdlib random surface wins.

### Alternative 5: Share the `random_int` name with `std.crypto`

Keep `random_int` for both the non-crypto (`uses Random`) and crypto (`uses Crypto`) globals. Rejected — Ori resolves free functions by name, so a program importing both would collide, and the two most safety-divergent RNGs would share an identifier. `rand_*` (matching the prelude trait) vs `random_*` (crypto) keeps them distinct.

---

## Purity Analysis

**Can be pure Ori?** YES (library-first).
**If not, why:** N/A — the entire feature is pure-Ori stdlib. The seeded core `Rng` (PRNG arithmetic, `next_*`, `shuffle`/`choice`/`sample`) is fully pure Ori. Only *system seeding* (reading OS entropy) requires IO, expressed through the `Random` capability's `SystemRandom` provider, which uses the existing capability mechanism (spec clause 20), not a new compiler primitive. Error paths are runtime panics (no compile-time diagnostic on argument values), so no compiler static-analysis dependency is introduced.
**Missing features that would enable purity:** None.
**Recommendation:** **Library-first — the compiler is untouched.** Implement the pure `Rng` + the `Random` trait + `SystemRandom` / `SeededRandom` providers entirely in stdlib Ori. The capability wiring reuses existing capability infrastructure.

---

## Spec & Grammar Impact

- **`Random` capability is already registered.** Spec clause 20.8 already lists `Random | RNG | No` (`20-capabilities.md:277`) and `capset Runtime = Clock, Random, Env` already uses it. This proposal does NOT add the capability; it **formalizes the already-registered `Random` capability** by specifying its trait method surface (`@rand_int` / `@rand_float` / `@rand_bool`) and the PRNG algorithm clause.
- **New/updated spec content** for `std.math.rand` (currently a stub): the `Rng` type, its methods (including `shuffle`/`choice`/`sample`), the pinned xoshiro256\*\* + SplitMix64 algorithm + exact constants, the inclusive Lemire bounded-integer method, and the 53-bit float construction.
- **Supersede the prelude + module doc.** The current `prelude.ori` `trait Random { @rand_int, @rand_float }` is replaced by the canonical three-method trait; `modules/std.math/rand.md` is rewritten (its `SystemRandom = "Cryptographically secure RNG (default)"` and `random_bytes` documentation is INCORRECT for this design — `SystemRandom` here is explicitly non-cryptographic, and byte generation lives at `std.crypto`). This security-guarantee change from crypto-secure to non-crypto is made explicit, not silent.
- **No grammar change** — `uses Random`, `with Random = ... in ...`, and `impl`/`def impl`/`trait` are existing productions.

---

## Relationship to std.crypto

The approved `stdlib-crypto-api-proposal.md` owns cryptographic randomness at the `std.crypto` root (NOT a `std.crypto.rand` submodule): `random_int` / `random_bytes` / `random_uuid`, all `uses Crypto` (a CSPRNG). This proposal owns *fast, non-cryptographic, seedable* randomness at `std.math.rand`. The two are kept disjoint:

- **Distinct names**: `std.math.rand` uses `rand_*`; `std.crypto` uses `random_*`. No identifier collision, so `use std.math.rand { rand_int }` and `use std.crypto { random_int }` coexist.
- **Same range convention**: both `rand_int` and `std.crypto.random_int` are inclusive `[min, max]`.
- **Explicit safety boundary**: `std.math.rand` documents prominently that it is NOT cryptographically secure; security-sensitive callers use `std.crypto`.

---

## Prior Art

- **Go `math/rand/v2`** — moved to a specified, seedable generator as the default; auto-seeds the top-level convenience API (fixing the pre-1.20 fixed-seed-global regret, go#54880 / go#59331) and keeps an explicit `Rand` value seeded via a `Source`. Directly parallels the pure-`Rng`-plus-globals split here; the auto-seed / no-fixed-seed-default discipline is adopted.
- **Rust `rand` / `rand_core`** — `RngCore` trait + concrete generators (`StdRng`, `SmallRng` = xoshiro/PCG family); `SeedableRng::seed_from_u64` uses SplitMix64-style expansion — the exact seeding idiom used here. Separates crypto (`OsRng`) from fast PRNGs, matching the `std.crypto` / `std.math.rand` split.
- **Swift `RandomNumberGenerator`** protocol + `SystemRandomNumberGenerator` — the capability-like "system generator vs your own seeded generator" split; the pure-generator-passed-explicitly shape.
- **Java `SplittableRandom` / `RandomGenerator`** (JEP 356) — SplitMix64 core; a specified, reproducible algorithm behind a value-like generator.
- **xoshiro / SplitMix64 (Blackman & Vigna)** — public-domain reference algorithms, small enough to specify exactly, which is why they are the pinned default here.

(Prior-art entries are discovery context, to be verified against the reference sources during `/review-draft-proposal`.)

---

## Unresolved Questions

- **Splittable / parallel streams** — deferred; the chosen xoshiro256\*\* admits `jump()` and SplitMix64 admits `split()` for independent substreams, so the design does not foreclose a future `Rng.split()`; whether to expose it now is open.
- **Distributions** (normal, exponential, weighted `choice`) — explicitly a follow-up proposal, not this one.
- **`float` precision** — 53-bit `[0.0, 1.0)` double is proposed; confirm no need for a `[0.0, 1.0]` inclusive or 64-bit variant.
- **`SystemRandom` naming** — the name is reused from the (incorrect) `rand.md` doc with an inverted security guarantee; confirm whether to keep `SystemRandom` (with the doc rewrite making non-crypto explicit) or rename to avoid confusion with a crypto-secure connotation.
