# Proposal: Final-Binary Generation Lifecycle

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Approved:** 2026-07-21
**Affects:** Build system, compiler driver (`oric`), `ori_llvm` AOT/incremental layer, CI, release packaging
**Depends On:** cache-lifecycle-proposal.md (approved — supplies the cache root, the budget, the collection pass, and the D3a in-flight-reference primitive this proposal extends)
**Amends:** aot-compilation-proposal.md (approved) — its build-output publication behavior
**Related:** toolchain-philosophy-proposal.md (approved — T4 is the outcome contract), multi-file-aot-proposal.md (approved)

---

## Summary

`ori build` publishes each final binary as an **immutable generation** made current by an atomic metadata switch, and reclaims superseded generations under a **lease protocol** that never removes an artifact a live process is executing or linking. This closes the half of toolchain-philosophy invariant T4 that `cache-lifecycle-proposal.md` deliberately excluded: bounding the *cache* does not stop N rebuilds from retaining N binaries, because a binary is keyed by build identity rather than content and therefore has no supersession signal to prune against. The design problem is concurrency, not policy — the shipped publication primitive is correct for content-addressed entries and provably incorrect for current pointers, and this proposal is the place that gets fixed.

---

## Motivation

### The outcome T4 requires, and what is still missing

The approved umbrella states T4 at outcome level:

> The build/cache is **bounded and automatically self-evicting** — the tool that wrote it reclaims it under a defined policy on by default, so cache never accumulates without limit.

`cache-lifecycle-proposal.md` delivers that for **content-addressed, hermetically regenerable** entries — intermediate objects, IR, metadata, incremental state — where eviction is safe by construction because the key that named an entry also reconstructs it. It explicitly excludes final binaries and states the residual outcome they must satisfy:

> 1. A rebuild MUST NOT cause the cache to accumulate one retained copy per build.
> 2. **The set of retained final-binary generations MUST itself be bounded, and every generation in it MUST be reclaimable under budget pressure — including current ones.** No class may be permanently exempt from the collector; a budget that cannot reach its largest class is not a bound.
> 3. Final-binary storage counts against **this proposal's budget**. There is one budget over one cache root, not two.
> 4. A final binary MUST NOT be mutated underneath a process reading or executing it.
> 5. A binary's debug sidecar MUST share its artifact's lifetime AND remain discoverable from the materialized deliverable (see D5a).

Clause 2 is the binding constraint on D4 below; clause 5 is the binding constraint on D5/D6.

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

For the shipped content-addressed path this is **tolerable**, though not for the reason it first appears. Same key does **not** imply same content: `concurrent_publication_keeps_object_manifest_pairs_coherent` (`cache/tests.rs`) publishes two *different* objects under one `CacheKey` with distinct object ids, and asserts **both** generation files survive (`read_dir(objects).count() == 2`). The real shipped contract is **object-manifest pair coherence** — a reader always sees one complete, self-consistent pair — not content equality. The loser of a metadata race loses only its *pointer*, while its generation persists and stays reachable if re-published.

For a **current pointer** the same arm is **unsound in two distinct ways**, and the pair-coherence contract is exactly why:

1. The two writers wrote *different* content. Discarding the loser's temp and reporting `Ok` silently drops a newer generation — a rebuild appears to succeed while the pointer still names the old binary. This is a wrong-artifact bug that no test of the content-addressed path can catch.
2. The guard is `Err(_) if destination.exists()`, which swallows **every** rename failure whenever the destination happens to exist — permission denied, cross-device link, an I/O error — and reports success. On the content-addressed path a published destination is genuine evidence that *some complete, self-consistent pair* is readable, which is all that path promises. On the pointer path the promise is stronger — *this* generation is current — and destination-exists is no evidence of it at all.

*Verified by direct source read of `cache/atomic.rs`.*

### Problem 2 — a current generation cannot be evicted, so the current set is unbounded

The obvious design refuses to reclaim the *current* generation for a build identity, since the next `ori run` would otherwise find nothing. But that exempts the current-generation set from the budget: one per (project × profile × target × output kind × output path), retained indefinitely, growing with every distinct configuration a developer ever builds. A budget that cannot touch its largest class is not a bound, so the obvious design is wrong. D4 keeps current generations reclaimable and ranks them last instead.

This is the failure the cache-side budget does not reach, and stating "the cache is bounded" without resolving it would be false.

### Problem 3 — resolve-then-use is a race, and materialization is platform-divergent

A reader resolves a build identity to a generation, then uses it. Collection can run between those two steps. Any lease taken *after* resolution closes nothing — the window is exactly the gap the lease was meant to cover.

Materialization has an independent hazard, and it is platform-divergent in the design, not merely the implementation. POSIX symlink-swap semantics, which would make the switch atomic, do not carry to Windows, where an in-use executable is locked against both replacement and deletion; and a macOS debug sidecar is a directory bundle rather than a file, so a mechanism stated for one artifact shape does not carry to the other. A design that specifies one mechanism and assumes the rest follow is the shape that produced this proposal.

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

This **extends** `cache-lifecycle-proposal.md` D3a's in-flight-reference primitive; it does not introduce a second mechanism. D3a establishes that an unreferenced entry is reclaimable and a referenced one is not, with the check inside the removing critical section — sufficient for content-addressed entries, where a loser simply rebuilds. A final binary needs one thing more: a reader depends on a *specific* generation being **current**, so the reference must be verified against the current pointer, not merely held.

Reclaim must never remove a generation a live process holds. The ordering is the whole design:

1. Reader **acquires** a lease on the generation it intends to use.
2. Reader **re-reads** the current pointer and verifies it still names that generation.
3. On mismatch, the reader releases and retries from step 1.

Acquiring before verifying is what closes the window; resolve-then-acquire leaves exactly the gap it was introduced to cover. Collection takes the mirror discipline: it may reclaim a generation only after observing it is unleased, with the lease check inside the same critical section that removes it.

**Two reclaim cases, not one — and conflating them makes D4 clause 2 unenforceable.**

| Case | Precondition | Pointer action |
|---|---|---|
| **Superseded** generation | A newer generation is already current | None. D2's atomic switch retired it at publication; reclaim only removes storage |
| **Current** generation (any of D4's DEAD / COLD / RECENT tiers) | The generation the identity's pointer still names is being reclaimed | **Retire the pointer as part of the same critical section that removes the generation** |

The distinction is **superseded vs still-pointed-to**, not which tier. A superseded generation was de-pointed at publication, so reclaim only frees storage. Every one of D4's three tiers reclaims a generation an identity's pointer *still names* — a DEAD identity's root is gone but its pointer record persists, a COLD identity's is stale, a RECENT identity's is fresh — so all three perform the identical retire-and-remove transition. The tiers set reclaim *rank*, never reclaim *mechanism*. Removing a still-pointed-to generation's storage without atomically retiring its pointer would leave a dangling pointer into freed storage, which is the bug this row exists to prevent.

An earlier draft gated *all* reclaim on "non-current **and** unleased". That gate can never admit a never-superseded generation — a single-build identity has no successor to make it non-current — so its storage would be permanently unreclaimable regardless of budget pressure, and clause 2's "including current ones" would be a promise the mechanism cannot keep.

The current-generation case therefore performs a **retire-and-remove** transition atomically: clear the identity's current-pointer record and remove the generation inside one critical section, using D2's replace-or-fail primitive against the pointer. Afterwards the identity resolves to **no current generation** — a legitimate, explicitly-representable state meaning "this identity must rebuild before it can run". `ori run` on such an identity rebuilds rather than erroring, which is the full-rebuild cost D4's RECENT tier already names.

Leases are **per-generation, never cache-wide**. A cache-wide lease would let one live reader anywhere block reclaim of every unrelated generation, which cannot satisfy a bounded contract on a busy machine.

**Crash recovery:** a lease records an owning process identity (PID plus start time) and an expiry. A lease is reclaimable only when the owner is proven gone **AND** the expiry has passed — never on either signal alone. Expiry alone is insufficient (a long link can outlive a conservative expiry); owner-liveness alone is insufficient (a PID can be recycled). This is `cache-lifecycle-proposal.md` D3a's criterion unchanged; the portable detection mechanism is a prototype target, the conjunction is not. A crashed build therefore never permanently pins a generation, and a slow one is never reclaimed underneath.

`go#43645` ("build cache not safe for concurrent builds") and `zig#9258` ("Shared Cache Locking") are the precedents this protocol must answer; both north stars shipped bugs in exactly this area, which is the argument for prototyping rather than specifying from first principles.

### D4 — Bounding the current-generation set

**No generation is permanently exempt** — required by `cache-lifecycle-proposal.md` D4 clause 2, which admits no permanently-exempt class. Current generations are ranked *last*, never excluded.

**One global reclaim order, spanning both proposals' classes.** Because there is a single budget over a single cache root (clause 3), "evicted last" cannot mean two different things in two documents. The total order under budget pressure is:

| Rank | Class | Owner |
|---|---|---|
| 1 | Prior-version cache roots | cache-lifecycle D2 |
| 2 | Dead final-binary generations (project root gone) | this D4 |
| 3 | Superseded (non-current) generations | this D3 |
| 4 | Incremental-compilation state, then intermediate objects / IR / metadata, LRU within class | cache-lifecycle D2 / D4 |
| 5 | Cold final-binary generations (identity past the recency floor) | this D4 |
| 6 | Measured profile data | cache-lifecycle D3 / D4 |
| 7 | **Recent** final-binary generations, least-recently-built first | this D4 |

Ranks 6 and 7 are where the two proposals' local "evicted last" claims meet: profile data is reclaimed *before* a recent current generation because re-measuring a profile costs test execution while losing a recently-built binary costs a full rebuild *and* leaves the identity unrunnable until it happens. Both remain reclaimable; neither is exempt.

The three final-binary tiers, **named rather than numbered** — the global table above owns the numbering, and reusing local numbers for different classes is how "rank 3" came to mean two things:

- **DEAD tier** — the identity's project root no longer exists. Reclaimable immediately, ahead of everything (global rank 2). Deleted and moved workspaces are a monotonically-discoverable class that nothing else reclaims, which is why they go first. How large a share they represent is unmeasured and is **not** claimed here; the ranking rests on their being free to reclaim (zero rebuild cost), not on an asserted share.
- **COLD tier** — the identity has not been built within the recency floor. Reclaimed after every superseded generation and every regenerable entry (global rank 5).
- **RECENT tier** — the identity was built recently. Reclaimed **last of everything** (global rank 7), and only when the budget cannot be met by exhausting the DEAD and COLD tiers and every class ranked above them. Reclaiming a recently-built current generation costs that identity a full rebuild, which is why it is last; it is not forbidden. ("RECENT", not "LIVE" — "live" is reserved throughout D3 for a live *reader/process* holding a lease, an unrelated sense.)

An earlier draft exempted recently-built identities from reclaim entirely. That is the loophole `cache-lifecycle-proposal.md` D4 clause 2 exists to close: a developer holding N configurations warm would pin N generations permanently, and a budget that cannot reach them is not a bound. The exemption is withdrawn.

**Thrash guard.** Reclaiming a recent current generation the next build immediately rebuilds is churn, not bounding. The RECENT tier therefore reclaims only when the budget remains unmet after the DEAD and COLD tiers are exhausted, and it reclaims the least-recently-built recent identity first.

The guard **defers** reclaim; it never **cancels** it. The RECENT tier proceeds down to the budget even when every remaining generation is recently-built — the alternative would be a de-facto exemption for a degenerate case, which clause 2 forbids.

**This proposal states no separate bound.** The budget invariant for the whole cache root is `cache-lifecycle-proposal.md` D2's composed bound — `max(budget, OVERSIZED + LEASED)` — which already carries this proposal's contribution as its `LEASED` term. Restating it here as a second `max(budget, X)` formula was how the two documents came to disagree: each named one exception as if it were the only one, when both are independently triggerable within the single budget clause 3 mandates.

What this proposal contributes to that invariant is the `LEASED` term's shape. Leases are per-generation (never cache-wide) precisely so concurrent builds do not block each other — so N concurrent builds hold N distinct leases and none of those generations is reclaimable while leased; a CI matrix fanning out N builds is the ordinary way to reach it. The set is nonetheless self-limiting: a lease is held only for the window in which a build may still hand a path to a subprocess, and it expires under D3's owner-gone-AND-expiry-passed rule, so `LEASED` is bounded by concurrent in-flight builds rather than by history.

There is no per-class allocation and no final-binary sub-budget — `cache-lifecycle-proposal.md` D4 clause 3 forbids a second budget ("one budget over one cache root, not two"). Final-binary generations compete for the same single budget as every other class; what distinguishes them is only their *rank* in the reclaim order, not a reserved share. The leased-generation floor and D2's oversized-entry floor are **not alternatives** — both can be active at once, which is precisely why the composed bound sums them rather than taking a maximum over them.

One generation may exceed the budget (refusing it would leave a build with no output); an accumulating *set* may not. A budget so small that steady-state work does not fit is reported by `ori cache info` as sustained RECENT-tier reclaim, so the user sees rebuild churn as a budget signal rather than as unexplained slowness.

`ori cache info` reports the current-generation set separately from other reclaimable entries, so its size and rank are visible rather than implicit.

### D5 — Deliverable materialization

The deliverable at the user's output path is materialized from the current generation. The switch happens on cache metadata; the user-visible path is never a file swapped underneath a process that may be executing it.

| Platform | Mechanism | Constraint answered |
|---|---|---|
| POSIX | Hard link where the cache and output share a filesystem, copy otherwise | An executing process holds an inode, not a path; replacing the link does not disturb it |
| Windows | Copy, published via the D2 replace primitive | An in-use executable is locked; a build targeting a running binary must fail with an actionable diagnostic naming the holding process, never partially overwrite |

**A materialized deliverable NEVER blocks reclaim, and this is what keeps clause 2's bound achievable.** An earlier draft said reclaim "must treat an outstanding materialized link as a reference." That is wrong, and wrong in the direction that breaks the bound: a deliverable at `./build/app` is a file the user keeps and may run at any time, with no expiry and no liveness signal — unlike a D3 lease, which is bounded by both. Treating it as a blocking reference would let any retained deliverable pin its generation indefinitely, re-creating the permanently-exempt class clause 2 forbids.

The correct model follows POSIX link semantics exactly:

- Reclaim removes **the cache's own directory entry**, always, unconditionally. That operation is safe with any number of outstanding deliverable links: `unlink` on one name never disturbs another name for the same inode, and an executing process holds the inode regardless.
- Once the cache's entry is gone, the generation is **no longer cache-accounted**. If the user's deliverable holds the last link, those blocks are the user's build output — the same as any file `ori build` wrote — and they are outside the cache budget by definition. The budget bounds the *cache*, not the user's output directory.
- Consequently `ori cache gc` can always reach the bound, and disk that survives is disk the user is deliberately holding.

**Windows diverges and must be stated separately**: with copy-based materialization the deliverable is an independent file from the start, so reclaim of the cache's copy is trivially unblocked. No link-count reasoning applies.

The prototype obligation is therefore *not* "does link counting suffice" (the cache no longer depends on it) but the narrower question of whether any supported filesystem fails to free blocks on the cache's own unlink once its entry is the last cache-side name.

**The debug sidecar is materialized with the deliverable**, per `cache-lifecycle-proposal.md` D4 clause 5 and D5a. A sidecar left only inside the generation is unreachable from the binary a debugger actually opens: `.gnu_debuglink` and the Windows PDB path record resolve against the deliverable's location, and a PDB path fixed at link time would otherwise point into reclaimable cache storage. Build-ID lookup is preferred on Linux precisely because it is location-independent.

**"By the same mechanism" is insufficient — the pair needs a transactional protocol, and one platform's sidecar is not a file.**

- **The sidecar is not always file-shaped.** A macOS `.dSYM` is a *directory bundle*, which cannot be hard-linked as a unit. Per-platform mechanism: POSIX single-file sidecars (`.dwo` / `.debug`) hard-link like the binary; a `.dSYM` bundle is materialized by recursive link-or-copy into a staging directory; Windows `.pdb` copies.
- **Materialization is two writes and must not be observable half-done — but it does NOT rename a directory onto the output location.** The deliverable stays a plain file at its documented path (`aot-compilation-proposal.md`'s output layout is unchanged per this proposal's Non-Goals); renaming a staging *directory* onto that path would turn the executable into a directory, and renaming it onto the enclosing directory would clobber unrelated sibling artifacts. Neither is intended. The protocol is: stage binary and sidecar into a temporary sibling directory, `fsync` both, then publish with **per-object atomic renames from the staging directory onto their final paths** — binary first, sidecar second — each using D2's replace-or-fail primitive.
- **The ordering is what makes the pair safe, since two renames are not one transaction.** Publishing the binary first would briefly pair a new binary with an old sidecar; publishing the **sidecar first** pairs an old binary with a new sidecar, which the discovery mechanisms reject rather than mis-resolve (build-ID and PDB signature both fail to match, so a debugger reports missing symbols instead of showing wrong ones). Sidecar-then-binary is therefore the required order, and the residual window degrades to *no* debug info rather than *wrong* debug info.
- **Failure is all-or-nothing.** If the sidecar step fails, the staged pair is discarded and the previously-materialized pair is left untouched; the build reports the failure rather than publishing a binary whose sidecar is stale or missing. Silently shipping a binary with a mismatched sidecar is worse than failing the materialization, because the mismatch surfaces later as wrong debug info.
- **Consequence for D4 clause 5**: "shares its artifact's lifetime" is enforced by the pair being published and reclaimed as one unit, not by two independent operations that usually agree.

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
| The cache's own `unlink` always frees its accounting, regardless of outstanding deliverable links | Any supported filesystem where removing the cache-side name fails or does not release cache-accounted space |
| Binary+sidecar materialization is atomic, including a directory-shaped `.dSYM` | Any observed half-published pair, or a debugger resolving a sidecar that does not match its binary |

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
- **No new CLI surface.** `ori cache info` gains a current-generation line; the subcommand family itself is `cache-lifecycle-proposal.md`'s. `ori cache gc` and `ori cache clean` reach final-binary generations through D3's reference check — no separate command exists or is needed, and none is withheld.
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
