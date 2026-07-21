# Proposal: Final-Binary Generation Lifecycle

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** Build system, compiler driver (`oric`), `ori_llvm` AOT/incremental layer, CI, release packaging
**Depends On:** cache-lifecycle-proposal.md (draft — supplies the cache root, budget, and collection pass)
**Amends:** aot-compilation-proposal.md (approved) — its build-output publication behavior
**Related:** toolchain-philosophy-proposal.md (approved — T4 is the outcome contract), multi-file-aot-proposal.md (approved)

---

## Summary

`ori build` publishes each final binary as an **immutable generation** made current by an atomic metadata switch, and reclaims superseded generations under a **lease protocol** that never removes an artifact a live process is executing or linking. This closes the half of toolchain-philosophy invariant T4 that `cache-lifecycle-proposal.md` deliberately excluded: bounding the *cache* does not stop N rebuilds from retaining N binaries, because a binary is keyed by build identity rather than content and therefore has no supersession signal to prune against. The design problem is concurrency, not policy — the shipped publication primitive is correct for content-addressed entries and provably incorrect for current pointers, and this proposal is the place that gets fixed.

---

## Motivation

### The outcome T4 requires, and what is still missing

The approved umbrella states T4 at outcome level:

> The build/cache is bounded and automatically self-evicting: the tool that wrote it reclaims it under a defined policy, ON BY DEFAULT, so cache never accumulates without limit.

`cache-lifecycle-proposal.md` delivers that for **content-addressed, hermetically regenerable** entries — intermediate objects, IR, metadata, incremental state — where eviction is safe by construction because the key that named an entry also reconstructs it. It explicitly excludes final binaries and states the residual outcome they must satisfy:

> A rebuild MUST NOT cause the cache to accumulate one retained copy per build. A final binary MUST NOT be mutated underneath a process reading or executing it. A binary's debug sidecar MUST share its artifact's lifetime.

That is this proposal's charter.

### Why final binaries are a different problem, not a bigger one

In the motivating measurement, `target/debug/deps/` held **ten hash-suffixed copies of the same binary, none collected**. Content-addressing did not prevent that — it *caused* it. Content-addressing is a naming scheme: two builds with different content are simply two valid entries, so retention has nothing to prune against and every distinct-content build coexists forever.

A binary needs the opposite property: a **supersession** signal saying *this build replaces that one for this build identity*. And the moment supersession exists, so does the hazard that eviction of the superseded copy races a process still using it. The three problems below are all consequences of that one structural difference.

### Problem 1 — the shipped publication primitive is unsound for current pointers

`ori_llvm/src/aot/incremental/cache/atomic.rs` publishes via:

```rust
fn publish_temp_file(temp: &Path, destination: &Path) -> Result<(), CacheError> {
    match fs::rename(temp, destination) {
        Ok(()) => Ok(()),
        // Windows does not replace an existing destination. A writer that won
        // the race has already published a complete destination.
        Err(_) if destination.exists() => {
            let _ = fs::remove_file(temp);
            Ok(())
        }
        ...
```

For a content-addressed destination this is **correct**: both writers computed the same content for the same key, so the loser discarding its temp loses nothing. For a **current pointer** it is **unsound in two distinct ways**:

1. The two writers wrote *different* content. Discarding the loser's temp and reporting `Ok` silently drops a newer generation — a rebuild appears to succeed while the pointer still names the old binary. This is a wrong-artifact bug that no test of the content-addressed path can catch.
2. The guard is `Err(_) if destination.exists()`, which swallows **every** rename failure whenever the destination happens to exist — permission denied, cross-device link, an I/O error — and reports success. On the content-addressed path the destination existing is genuine evidence the work is done; on the pointer path it is not evidence of anything.

*Verified by direct source read of `cache/atomic.rs`.*

### Problem 2 — a current generation cannot be evicted, so the current set is unbounded

Any correct design must refuse to reclaim the *current* generation for a build identity, or the next `ori run` finds nothing. But that makes the set of current generations structurally exempt from the budget: one per (project × profile × target × output kind × output path), retained indefinitely, growing with every distinct configuration a developer ever builds. A budget that cannot touch its largest class is not a bound.

This is the failure the cache-side budget does not reach, and stating "the cache is bounded" without resolving it would be false.

### Problem 3 — resolve-then-use is a race, and materialization is platform-divergent

A reader resolves a build identity to a generation, then uses it. Collection can run between those two steps. Any lease taken *after* resolution closes nothing — the window is exactly the gap the lease was meant to cover.

Materialization has an independent hazard. Placing the deliverable by hard link or copy means the user's `./build/app` is a *second* reference to generation content; POSIX symlink-swap semantics, which would make the switch atomic, do not carry to Windows, where an in-use executable is locked against both replacement and deletion. A design that specifies one mechanism and assumes the other platform follows is the shape that produced this proposal.

---

## Goals and Non-Goals

**Goals:**

- Define **build identity** precisely enough that no two distinguishable build outputs share a generation slot.
- Define a publication protocol whose current-pointer switch is atomic and **correct on Windows**, not merely correct on POSIX with a Windows caveat.
- Define reclaim that provably never removes a generation a live process is executing or linking, with a race-free acquisition order.
- Bound the current-generation set, so T4's "never accumulates without limit" holds for the whole cache and not only its regenerable part.
- Define deliverable materialization for a path the user may be executing, per platform.
- Prototype the concurrency protocol against the real primitives before specifying it as final, per `.claude/rules/prototype_strategy.md`.

**Non-Goals:**

- Not the cache root, budget policy, eviction ordering, or `ori cache` surface — `cache-lifecycle-proposal.md` owns those and this proposal consumes them.
- Not debug-info emission format — that is D5 of the cache proposal (what the compiler emits); this governs where a sidecar is stored and when it is reclaimed.
- Not a change to where deliverables *appear*. `build/obj/` and `--out-dir` semantics from `aot-compilation-proposal.md` are unchanged; only the publication mechanism behind them changes.
- Not a distributed or multi-machine build system.

---

## Design

### D1 — Build identity

A generation slot is keyed by the full tuple that distinguishes an output a user can ask for:

| Component | Why it must be in the key |
|---|---|
| Project root (canonicalized) | Two projects must not share a slot |
| Profile (`debug` / `release` / named) | Different optimization, different artifact |
| Target triple | Cross-compilation produces distinct artifacts |
| Output kind (`bin` / `--lib` / `--dylib` / `--wasm` / each `--emit=TYPE`) | `ori build --lib` and `ori build --dylib` produce different files for identical sources |
| Logical output path (`-o` / `--out-dir` relative to project root) | Two `--out-dir` targets are two deliverables |

A bare (project, profile, target) key — the shape that first suggests itself — conflates a `--lib` build with a `--dylib` build into one slot, so each would evict the other and a project building both would rebuild on every alternation. Identity is derived from the resolved invocation, never from the command string.

### D2 — Publication: content first, visibility second, with an explicit replace primitive

Publication keeps the shipped two-step shape, which is already the right one:

1. **Write generation content** to an exclusively-created path (`create_new`, no rename). Safe without coordination because the generation is not discoverable until step 2.
2. **Switch the current pointer** by atomically replacing the identity's metadata record.

Step 2 requires a primitive `publish_temp_file` does not provide: **replace-or-fail**, never replace-or-assume-success.

| Platform | Primitive | Contract |
|---|---|---|
| POSIX | `rename(2)` | Already atomically replaces an existing destination |
| Windows | `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` (or `ReplaceFileW`) | Atomically replaces; a genuine failure surfaces as an error |

The distinction from the existing primitive is that **failure is reported**. `Err(_) if destination.exists() => Ok(())` is retained only for the content-addressed path, where destination-exists genuinely means the work is done; it is banned on the pointer path. Two concurrent rebuilds of the same identity produce a last-writer-wins pointer where *both* generations exist and one is current — never a silently-discarded newer build.

**Open for prototyping:** whether the pointer is a metadata file replaced atomically, a directory entry, or a POSIX symlink with a Windows metadata-file fallback. The requirement is fixed (atomic switch, errors surfaced, no in-place mutation of published content); the mechanism is validated empirically before it is specified.

### D3 — Reclaim: acquire-then-verify, never resolve-then-acquire

Reclaim must never remove a generation a live process holds. The ordering is the whole design:

1. Reader **acquires** a lease on the generation it intends to use.
2. Reader **re-reads** the current pointer and verifies it still names that generation.
3. On mismatch, the reader releases and retries from step 1.

Acquiring before verifying is what closes the window; resolve-then-acquire leaves exactly the gap it was introduced to cover. Collection takes the mirror discipline: it may reclaim a generation only after observing it is both non-current **and** unleased, with the lease check inside the same critical section that removes it.

Leases are **per-generation, never cache-wide**. A cache-wide lease would let one live reader anywhere block reclaim of every unrelated generation, which cannot satisfy a bounded contract on a busy machine.

**Crash recovery:** a lease records an owning process identity and an expiry. A lease whose owner is gone, or whose expiry has passed, is reclaimable — a crashed build never permanently pins a generation. Expiry alone is insufficient (a long link can outlive a conservative expiry); owner-liveness alone is insufficient (a PID can be recycled). Both are required, and the interaction is a prototype target.

`go#43645` ("build cache not safe for concurrent builds") and `zig#9258` ("Shared Cache Locking") are the precedents this protocol must answer; both north stars shipped bugs in exactly this area, which is the argument for prototyping rather than specifying from first principles.

### D4 — Bounding the current-generation set

The current generation for an identity is exempt from reclaim while that identity is live. The set of *identities* is therefore what must be bounded, and it is bounded by retiring identities rather than by evicting current generations:

- An identity whose project root no longer exists is **dead**; its current generation is reclaimable immediately. This alone collects the dominant term, since deleted and moved workspaces are what accumulate.
- An identity not built within the cache's recency floor is **cold**; its current generation becomes eligible for reclaim under budget pressure, ranked after every non-current generation and every regenerable entry.
- Reclaiming a cold identity's current generation costs a full rebuild, which is why it ranks last. It is never *forbidden*, because a class that can never be reclaimed is the unbounded-growth failure this whole workstream exists to prevent.

`ori cache info` reports the current-generation set separately from reclaimable entries, so the exempt-by-default portion is visible rather than implicit.

### D5 — Deliverable materialization

The deliverable at the user's output path is materialized from the current generation. The switch happens on cache metadata; the user-visible path is never a file swapped underneath a process that may be executing it.

| Platform | Mechanism | Constraint answered |
|---|---|---|
| POSIX | Hard link where the cache and output share a filesystem, copy otherwise | An executing process holds an inode, not a path; replacing the link does not disturb it |
| Windows | Copy, published via the D2 replace primitive | An in-use executable is locked; a build targeting a running binary must fail with an actionable diagnostic naming the holding process, never partially overwrite |

A hard link makes the deliverable a second reference to generation content, so reclaim must treat an outstanding materialized link as a reference — the link count, not the cache's own bookkeeping, is the authority on POSIX. Whether that is sufficient on every supported filesystem is a prototype target.

### D6 — Sidecar co-lifetime

A debug sidecar is published **inside its artifact's generation**, so it shares that generation's identity and reclaim by construction. It is not a separately keyed entry that could be evicted while its binary survives. Asserting "sidecars are never orphaned" across two different retention keys, with no mechanism to enforce it, is the shape this replaces.

---

## Prototype Obligations

Per `.claude/rules/prototype_strategy.md`, the concurrency protocol is validated before it is specified as final. Each item names its falsification signal:

| Hypothesis | Falsified by |
|---|---|
| The D2 replace primitive is atomic on Windows under concurrent rebuild | Any observed torn or stale pointer under N concurrent publishers |
| Acquire-then-verify closes the reclaim race | Any observed reclaim of a leased generation under adversarial interleaving |
| Owner-liveness + expiry never permanently pins a generation | Any leaked lease surviving a killed build |
| Owner-liveness + expiry never reclaims a live generation | Any reclaim under a link slower than the expiry, or under PID reuse |
| Hard-link reference counting suffices on POSIX | Any supported filesystem where the link count does not reflect an outstanding deliverable |

Correctness is the admission gate; a mechanism that fails any row is replaced, not accommodated.

---

## Drawbacks

- **This is the highest-risk part of the T4 workstream.** Both north stars shipped concurrency bugs here. Prototyping reduces the risk but does not remove it.
- **A lease protocol is real machinery.** Process-liveness checks, expiry, and crash recovery are code that exists only to make reclaim safe, on a path most builds never contend.
- **Reclaiming a cold current generation costs a rebuild.** Bounded by ranking it last, but the surprise is real when it happens.
- **Windows and POSIX diverge in the design, not only the implementation.** Different materialization mechanisms mean a class of bug can exist on one platform and not the other; CI must exercise both.
- **It amends an approved proposal's publication behavior.** `aot-compilation-proposal.md` did not contemplate generations; deliverable *location* is unchanged, but the mechanism behind it is not.

---

## Alternatives Considered

### Alternative 1: In-place overwrite of the deliverable

Write each new binary directly over the old path. **Rejected:** it corrupts any process currently executing or linking that file, and on Windows it cannot even be attempted against a running executable. It also makes the reclaim hazard `ArtifactCache::remove` documents unavoidable rather than solvable.

### Alternative 2: Content-address binaries too, with no current pointer

**Rejected:** this is the observed Cargo failure. Content-addressing provides no supersession signal, so retention has nothing to prune against and every distinct-content build coexists — the ten retained copies in the motivating measurement.

### Alternative 3: Content-address binaries *and* add a current pointer

**Rejected as redundant, not as wrong.** It would work; it is D1–D3 with an extra hash in the path and no added benefit, since build identity already names the slot a user asks for. The concurrency design is identical either way, which is the point: the hard part is supersession and reclaim, not naming.

### Alternative 4: Never reclaim current generations

**Rejected:** it is the simplest correct-looking rule and it reintroduces unbounded growth, since the current set has no natural bound. D4 ranks current generations last rather than exempting them permanently.

### Alternative 5: A single cache-wide lock instead of per-generation leases

**Rejected:** it is simpler and it serializes unrelated builds. One live reader would block reclaim of every unrelated generation, so the cache could not stay bounded on a machine doing concurrent work.

### Alternative 6: Fold this into `cache-lifecycle-proposal.md`

**Rejected on review evidence.** The combined draft went through three review rounds; findings concentrated entirely in this half while the eviction and migration half stopped generating them. Splitting lets each half be reviewed against its own hazards — policy questions against measurement, concurrency questions against adversarial interleaving.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:** This governs on-disk artifact publication, cross-platform atomic-replace primitives, process-level lease coordination, and process-liveness detection. It lives in `oric` and the `ori_llvm` AOT/incremental layer.

**Missing features that would enable purity:** N/A — build-artifact publication is toolchain infrastructure by nature.

**Recommendation:** Proceed as a compiler/toolchain feature completing T4 of `toolchain-philosophy-proposal.md`, alongside `cache-lifecycle-proposal.md`.

---

## Spec & Grammar Impact

- **No grammar changes.** No new productions, keywords, or syntax.
- **No normative language-spec clause changes.** Artifact publication is toolchain behavior.
- **No new CLI surface.** `ori cache info` gains a current-generation line; the subcommand family itself is `cache-lifecycle-proposal.md`'s.
- **Amendment:** on approval, `aot-compilation-proposal.md` receives an errata entry recording that final-binary publication goes through generations rather than direct output writes, per the errata format in the proposals rule.

---

## Prior Art

- **Go.** `go#43645` ("build cache not safe for concurrent builds") is the direct precedent: a shared cache whose concurrency model was under-specified shipped a real bug. *Verified against the `go` issue corpus indexed in the intelligence graph.*
- **Zig.** `zig#9258` ("Shared Cache Locking") is the closest prior design discussion for the locking model this proposal needs, and Zig's in-place incremental direction is the argument that N rebuilds should not retain N artifacts. *Verified against the `zig` issue corpus indexed in the intelligence graph.*
- **Rust / Cargo.** `target/debug/deps/` retaining hash-suffixed historical copies with no collection is the anti-pattern being corrected, and the measured evidence that content-addressing without supersession accumulates. *Verified against the measured `target/` breakdown in `cache-lifecycle-proposal.md`.*
- **Ori's own shipped cache.** `ori_llvm/src/aot/incremental/cache/atomic.rs` already implements the content-then-metadata two-step this proposal keeps, and its `publish_temp_file` already documents the Windows non-replacing-rename behavior that makes the current primitive unsuitable for pointers. *Verified by direct source read.*

---

## Unresolved Questions

- **Pointer representation.** Metadata file, directory entry, or POSIX symlink with a Windows fallback — resolved by prototype, not by argument.
- **Lease expiry window.** Must exceed the slowest realistic link; the concrete value comes from measurement.
- **Process-liveness detection portability.** Whether PID-plus-start-time is sufficient on every supported platform, and the fallback where it is not.
- **Recency floor for cold identities.** Shares the cache proposal's floor or takes its own; a rebuild is more expensive than an object recompile, which argues for a longer one.
- **Concurrent materialization.** Whether two builds writing different `--out-dir` deliverables from the same generation need coordination beyond the lease.
- **CI applicability.** Whether ephemeral CI containers can skip the lease protocol entirely, and how that interacts with the cache proposal's open CI-locality question.
