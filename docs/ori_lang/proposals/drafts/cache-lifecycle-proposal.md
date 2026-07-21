# Proposal: Build Cache Lifecycle and Garbage Collection

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** Build system, compiler driver (`oric`), CI, release packaging, developer disk footprint
**Depends On:** toolchain-philosophy-proposal.md (approved)
**Amends:** aot-compilation-proposal.md (approved) — its Incremental Compilation Cache decision AND its Debug Format table (Linux row)
**Related:** multi-file-aot-proposal.md (approved), self-contained-toolchain-proposal.md (draft — its installed components are excluded from GC by D3; the exclusion stands on its own terms and does not depend on that draft's approval), test-driven-pgo-proposal.md (draft — its profile cache is classified by D3), binary-generation-lifecycle-proposal.md (successor — depends on this proposal's cache root, budget, and D3a reference primitive; extends them to final-binary generations)

---

## Summary

Ori's build cache gains an **automatic, bounded eviction lifecycle** it does not have today. This proposal migrates the existing single-source object cache — already global, already content-addressed, already atomic — into the general build-artifact cache, adds the garbage collection it currently lacks, and separates debug information so a shipped artifact is code rather than symbols. Cache administration becomes `ori cache` subcommands. Scope is deliberately bounded to **content-addressed, hermetically regenerable entries**, where eviction is safe by construction; final-binary publication and reclaim — which need supersession and live-reader coordination, not content-addressing — are the successor proposal `binary-generation-lifecycle-proposal.md`.

This proposal owns the **mechanism** for invariant **T4 ("The Creator Owns the Lifecycle")** of the approved `toolchain-philosophy-proposal.md`, which states the outcome contract: the cache is bounded and self-evicting, debug info is separable, and a per-project never-evicted cache is the explicitly rejected anti-pattern.

It **amends** the approved `aot-compilation-proposal.md`, whose Incremental Compilation Cache section decided a project-local cache in `build/cache/`. That decision is superseded below with its rationale addressed directly, not silently reversed.

---

## Motivation

### What already exists

Ori already ships a cross-invocation, content-addressed object cache for the single-source `ori build` path:

- `oric/src/commands/build/incremental_cache.rs` — `CacheProbe::{Hit, Miss, Bypass}` over a `CacheKey { source_hash, deps_hash, flags_hash, combined }`, publishing an `ObjectManifest { schema, request_sha256, object_sha256, object_size }` under `CACHE_FORMAT_VERSION = "ori-build-object-v2"`.
- `ori_llvm/src/aot/incremental/cache/` — `ArtifactCache` with a two-step publication discipline in `atomic.rs`: generation *content* is written to an exclusively-created path (`publish_generation_bytes` / `publish_generation_file`, `create_new`, no rename), and the *metadata* that makes a generation discoverable is published by write-then-rename (`publish_bytes_atomically` → `fs::rename`). Content creation and visibility are already distinct steps.
- **The shipped cache runs on Linux and Windows only.** `llvm_runtime_digest()` is `#[cfg]`-split three ways: Linux fingerprints the loaded LLVM library by path, size, and digest; Windows hashes a fixed domain constant (LLVM is statically linked, covered by the compiler digest); **every other platform — macOS included — returns `Err("the dynamically loaded LLVM library cannot yet be fingerprinted on this platform; incremental reuse is disabled")`**, which becomes a quiet `CacheBypass`, so the object cache never runs there at all.

  This is load-bearing and easy to miss: it would mean D5's macOS dSYM commitments and the successor's `.dSYM` materialization design apply to a platform whose *object* cache never runs. Shipping a cache lifecycle that skips a supported platform is not acceptable, so **D1b below closes the gap as part of this proposal** rather than deferring it.
- The cache root is **global and per-user**, resolved in three tiers: `$XDG_CACHE_HOME/ori/build/<version>`, then `~/.cache/ori/build/<version>`, then — when `HOME` is unset or relative — `$TMPDIR/ori-cache/build/<version>`. The third tier is neither per-user nor under the `ori/` root the collector spans; D1 removes it (see below), which is why D2's collector root can be stated as `…/ori/build/`. It **detects** a relative `XDG_CACHE_HOME` and emits a diagnostic — but it does **not reject the build**: `cache_directory()`'s `Err` becomes a `CacheBypass { warn: true }`, printing "rebuild proceeds without reuse" and continuing **uncached**. D1 strengthens detection into rejection; the detection is the part that already exists.

So the foundation this proposal needs is largely built. What is missing is the **lifecycle**: there is no eviction, no budget, and no `gc`. `ArtifactCache::remove`'s doc comment records the residual reclaim hazard: "A concurrent linker may still hold the object path returned by [`Self::get_verified`]. [`Self::clear`] reclaims retained generations." The `clear` it directs to is a blunt `remove_dir_all` over both `objects/` and `meta/`.

*Absence-claim grounding, corrected.* The claim is **not** that no budget surface exists — one does: `CacheConfig.max_size: u64` (documented `0 = unlimited`) with a `with_max_size` builder. It has **zero consumers**; nothing reads it. D2 activates that field rather than adding a second knob beside it. The accurate claim is that no budget is *enforced*, no eviction policy exists, and there is no `gc`. Reclaim is worse than blunt: `ArtifactCache::clear` — the path `remove`'s doc comment directs to — has no production caller at all, appearing only in tests. Established by reading `oric/src/commands/build/incremental_cache.rs` and `ori_llvm/src/aot/incremental/cache/{mod,atomic,tests}.rs` in full and searching `compiler/` for eviction, budget, and collection consumers.

**Two further shipped caches this proposal must account for**, found by widening the same search past the two files above:

- `ori_llvm/src/aot/incremental/arc_cache/` — `ArcIrCache`, homed at the project-local `build/cache/functions/arc_ir/` layout D0 supersedes, caching serialized `Vec<ArcFunction>` (incremental-compilation state — the class D4 calls the dominant term in the motivating failure). It has no atomic publication, no read verification, and no eviction. It is currently **dormant** (no consumer outside its own re-export). It migrates to this cache under D1 when it gains one; shipping it live at the superseded layout is banned.
- `incremental/mod.rs`'s module documentation describes the project-local `build/cache/` tree (`hashes.json`, `deps/`, `objects/<hash>.o`) that D0 amends away. That documentation is updated as part of the migration.

### The gap this closes

Nothing bounds the cache. Ori is early enough that no user has hit the wall yet, but the failure mode is fully characterized by the ecosystem it borrows from. Measured on a working developer machine, Rust's per-project `target/` reached:

```
63G     target/
40G     target/debug/incremental/     # ~63% — incremental cache, never pruned
215 MB  target/debug/ori              # of which ~185 MB is DWARF; ~25 MB is code
        target/debug/deps/            # 10 hash-suffixed copies of the same binary, none collected
```

This is not one pathological workspace. Enumerating every Rust `target/` directory on the same machine (`du -s --block-size=1M` over `find ~/projects -maxdepth 3 -type d -name target`) gives 65159 + 63636 + 29375 + 3902 + 12 MiB across five workspaces — **158.3 GiB** (`(65159+63636+29375+3902+12)/1024` → `158.3`) held by a single toolchain's build caches, none of it reclaimed by anything. The root cause is not *having* a cache — it is that **no component owns the cache's eviction lifecycle**, so "disposable" never becomes "automatically disposed". Ori has the same structural exposure today: an unbounded global cache plus a not-yet-written multi-file cache.

### Why now

The multi-file build path has no on-disk artifact cache yet. Extending the existing single-source cache to cover it is imminent work. Establishing eviction and debug-info separation **as part of that extension** avoids retrofitting a lifecycle onto a format that did not plan for one — precisely the position Cargo is in.

---

## Goals and Non-Goals

**Goals:**

- Migrate the existing single-source object cache into the general build-artifact cache, preserving its input-addressing, atomicity, and absolute-path discipline.
- Make cache entries **project-neutral** (D1a): remove project-location identity from the request digest, add stable module identity, and land the DWARF path normalization and diagnostic re-anchoring that make a shared entry correct rather than merely deduplicated.
- Make the cache **run on every supported platform** (D1b): add the missing macOS LLVM-fingerprinting arm, so the lifecycle is not codified for a cache that is inert on one of the three.
- Add eviction that is **on by default** and bounded, with a policy that does not depend on unreliable filesystem metadata.
- Separate debug information via the **platform-native** mechanisms the AOT proposal already chose.
- Expose cache administration as `ori cache` subcommands (per T1/T2).
- Amend the approved project-local `build/cache/` decision explicitly, addressing its stated rationale.

**Non-Goals:**

- Not a distributed or remote/shared-team cache.
- Not a redesign of incremental compilation itself — this governs the *storage and lifecycle* of its outputs.
- Not a package/registry download-cache policy.
- Not a change to `build/obj/` or the AOT output layout — deliverables stay where `aot-compilation-proposal.md` puts them; only the *cache* moves.
- Does not re-decide T4's outcome contract, which the approved umbrella owns.
- **Not the final-binary publication and reclaim mechanism.** Generation identity, current-pointer switching, cross-platform replace semantics, deliverable materialization, and live-reader coordination belong to `binary-generation-lifecycle-proposal.md`. This proposal states the outcome those must satisfy (D4) and stops there.

---

## Design

### D0 — Amendment: the cache is global, deliverables stay project-local

`aot-compilation-proposal.md` decided a **project-local cache in `build/cache/`**, on three stated grounds: simpler cache invalidation, no cross-project coherency issues, and per-project cache lifetime. This proposal supersedes that decision, addressing each ground:

- **Invalidation simplicity** — input-addressing already provides it, via two cooperating layers rather than the key alone. The live `CacheKey` carries four fields with three different derivations: `source_hash` and `deps_hash` are a **128-bit truncation** of the request digest (bytes `[0..8]` and `[8..16]`); `flags_hash` is a separate hash of `"{compiler_version}:{opt_level}:{target}"`, not a digest slice at all; and `combined` is a 64-bit `combine_hashes` over the other three, used as the filename index. None of these is a content address. Full input identity is re-established on every read by `verify_cache_entry`, which compares the manifest's stored `request_sha256` against the recomputed digest and treats a mismatch as a miss — that SHA-256 comparison, not the key, is what makes reuse safe. There is still no invalidation logic to get wrong — but the guarantee rests on that read-time verification, not on the key being a content address. This holds today and is unaffected by the move.
- **Cross-project coherency** — coherency is not the obstacle; **reachability is**, and it is a change this proposal must make rather than a property it inherits. Two projects resolving to the same key did resolve to the same inputs, so sharing is correct. But under the shipped digest they *cannot* resolve to the same key: `request_digest` folds **`source-spelling`** (the path as written) and **`canonical-source`** (the canonicalized absolute path) into the hash, so byte-identical source at two paths yields two keys. Cross-project reuse is therefore a **change this proposal makes** — D1a, in scope — not a property it inherits.
- **Per-project cache lifetime** — this is the property being deliberately traded. Per-project lifetime is exactly what leaves each project's cache unbounded, and what would produce N copies of one dependency across N projects once multi-file builds have shared dependencies to duplicate. Lifetime moves to the budget-and-recency policy in D2.

**What the amendment buys, and when.** Moving the cache out of `build/cache/` delivers **bounded, self-evicting storage under one owner** immediately; that alone satisfies T4. Cross-project *sharing* is the second benefit, and it is **in scope here** — D1a below is part of this proposal's delivered work, not sequenced follow-on. One honest timing note: sharing has no observable effect until multi-file caching lands, because `has_unfingerprinted_imports` bypasses the cache for any source carrying `use` imports, so the shipped cache is single-file-only today. D1a therefore lands *ready*, and the shared-dependency duplication Alternative 1 rejects becomes reachable the moment multi-file caching does. Landing the keying change with the amendment rather than after it means the approved decision is overturned once, with both benefits in the approved scope, instead of amended now and amended again later.

### D1a — Path-independent keying (IN SCOPE — delivered with this proposal)

Cross-project reuse requires removing `source-spelling` and `canonical-source` from the request digest. That is a real behavioral change, and its consequences are **deliverables of this proposal**, not warnings about future work. Removing the path fields without the two mechanisms below would ship a cache that silently serves objects carrying another project's paths — so they land together or not at all.

- **DWARF path normalization — REQUIRED, lands with the keying change.** Debug info records source paths, so an object shared across projects would otherwise carry whichever path first produced it. Emission normalizes source paths to a **project-root-relative form** before they enter DWARF, with the consuming project's root supplied at link time — the role `-fdebug-prefix-map` plays for C/C++ toolchains. An object is then genuinely project-neutral rather than merely deduplicated. This also improves reproducibility independently of caching, and it must agree with `SOURCE_DATE_EPOCH`'s existing treatment: both are "strip machine-and-location identity from the artifact" concerns and neither may reintroduce what the other removes.
- **Diagnostic rendering — REQUIRED, lands with the keying change.** A diagnostic resolved against a shared object renders the **consuming** project's path. Because normalization makes stored paths root-relative, rendering re-anchors them against the consuming project's root at display time. A user must never see another project's directory in their own diagnostic; if that appears, the normalization is incomplete and the cache entry is not shareable.
- **Negative gate.** Path normalization is the *precondition* for dropping the path fields. An implementation that removes `source-spelling` and `canonical-source` before normalization is in place is a wrong-artifact bug, not a partial delivery — the entry becomes shareable while its contents are still project-specific.
- **Module-qualified symbol mangling — the one that can produce a WRONG artifact.** `multi-file-aot-proposal.md` §3 mangles emitted symbols as `_ori_<module-path>_<function-name>`, so the module path is baked into an object's *linker symbols*, not only into its debug info. Stripping path identity from the digest without replacing it would let two projects with byte-identical source at different logical module paths collide on one key while legitimately requiring differently-mangled symbols — a wrong-artifact hazard, strictly worse than the cosmetic path classes above. **Therefore D1a removes the two filesystem-path fields but ADDS the stable logical module identity to the digest.** The *project's own* filesystem location stops being part of the key; module identity does not.

**Residual path-derived inputs remain, and they are platform-dependent.** `request_digest` folds `dependency-sha256` and `llvm-sha256`. `prelude_digest()` hashes the prelude source path under the label `prelude-path` on every platform. `llvm_runtime_digest()` is `#[cfg]`-split three ways and only its **Linux** arm hashes a library path (`library-path`); the **Windows** arm hashes a fixed domain constant and no path at all. Removing `source-spelling` and `canonical-source` therefore does **not** make the digest path-free on Linux, and makes it nearly so on Windows.

- These paths are properties of the **installed toolchain**, not of the project, so they are constant across projects on one machine — which is exactly the scope cross-project sharing needs, and is why D1a does not strip them.
- They correctly *do* participate in the key: a different prelude or a different LLVM is a different compilation input, and an entry produced under one must not be reused under the other.
- The limitation this creates is on **cross-machine** sharing (a future remote-cache concern, outside this proposal's Non-Goals), not on the cross-project sharing D1a targets.

The precise claim is therefore: D1a removes *project-location* identity from the key while preserving *toolchain* identity. The earlier unqualified phrasing overstated it.
- **Reproducibility interaction.** `SOURCE_DATE_EPOCH` is already in the unfingerprinted-environment bypass list; path normalization is the same class of concern and the two must agree on what a reproducible artifact means.

D1a ships as one unit — path fields removed, module identity added, normalization and rendering in place. Its *effect* is gated by multi-file caching (single-file-only builds have no shared dependencies to reuse), but its *correctness* is not: a single-file build under D1a produces a project-neutral entry that simply has no second consumer yet.

**Unchanged:** `build/obj/` and the AOT output layout stay exactly as approved. A project directory holds its *deliverables*; the cache holds *reusable intermediate work*. Deleting a project never destroys reusable work; deleting the cache never destroys deliverables.

### D1 — One global content-addressed cache (migration, not greenfield)

The existing cache root, key, manifest, and atomic publication are adopted as the foundation and extended to cover multi-file builds.

- **Location:** the current per-user root (`$XDG_CACHE_HOME/ori/build/<version>`), with a new `ORI_CACHE` override. **`ORI_CACHE` does not exist today** — it is introduced here, and introducing it requires a coordinated change (below).
- **`ORI_CACHE` must be exempted from the unfingerprinted-environment bypass.** `is_unfingerprinted_environment` currently bypasses the cache for *any* variable starting with `ORI_`, exempting only `ORI_SANITIZE` and `ORI_NO_REPR_OPT`. Without an exemption, setting `ORI_CACHE` to redirect the cache would **disable** it, emitting a diagnostic telling the user to unset the very flag that selects the root. `ORI_CACHE` and any budget variable this proposal adds join the exemption list in the same change that introduces them. The exemption is correct on the merits: the cache *root* is not a compilation input, so it must not fingerprint an entry.
- **Absolute-path discipline — mandatory, and it fixes a live silent-fallthrough.** `ORI_CACHE` and the OS-conventional default are each resolved to an absolute path before any use; a relative path is **rejected with an explicit diagnostic naming the variable and the requirement**, never silently normalized, ignored, or joined against the current directory. This is stronger than shipped behavior in two places: a relative `XDG_CACHE_HOME` currently *warns and proceeds uncached* rather than failing, and a relative-or-unset `HOME` currently falls through **silently** to a third fallback (`std::env::temp_dir().join("ori-cache")`) with no diagnostic at all. That silent fallthrough is the `go#69997` shape ("silently ignores a relative `GOCACHE`") sitting inside the function cited here as precedent; it is fixed, not inherited. `zig#20129` / `zig#19284` / `zig#25307` (closed) and `zig#9215` (open) are the same defect class in the other north star.
- **Cross-project reuse:** an entry whose key matches is reusable by any project on the machine, per D1a's project-neutral keying and path normalization.
- **Format versioning:** the existing `CACHE_FORMAT_VERSION` / `CACHE_MANIFEST_SCHEMA` pair gates migration; an unrecognized version is a cold cache, never a misparse. `CACHE_FORMAT_VERSION` is a **path component** of the root, so a version bump creates a sibling directory — see D2's collector-root rule, which is what stops that from stranding the previous cache.

### D1b — LLVM fingerprinting on macOS (IN SCOPE — closes the platform gap)

The cache is disabled on macOS solely because `llvm_runtime_digest()` has no arm for it. The two shipped arms establish the pattern; macOS needs the third, and both of its linkage modes are answerable with mechanisms already in the file.

| Platform | Linkage | Mechanism |
|---|---|---|
| Linux (shipped) | Dynamic | Enumerate `/proc/self/maps`, select images whose basename starts with `libLLVM`, canonicalize, then fold `library-path`, `library-size`, `library-sha256` per image under domain `ori-build-llvm-runtime-v1` |
| Windows (shipped) | Static | LLVM is linked into the binary, so it is already covered by the compiler digest; fold the fixed domain `ori-build-static-llvm-covered-by-compiler-v1` and nothing else |
| **macOS (D1b)** | **Either** | **Enumerate loaded images via `_dyld_image_count()` / `_dyld_get_image_name()`, select basenames starting with `libLLVM`, and fold the identical three fields under the identical Linux domain. If no such image is loaded, LLVM is statically linked and the Windows arm's reasoning applies verbatim — fold the static domain constant.** |

- **It is a port, not a new design.** The identity being captured is unchanged; only the OS mechanism for enumerating loaded images differs (`/proc/self/maps` has no macOS equivalent, `dyld` is the direct analogue). Reusing the Linux domain string for the dynamic case is deliberate: the same LLVM produces the same digest regardless of which OS enumerated it.
- **Both linkage modes are real on macOS** — a Homebrew LLVM is dynamic, a vendored or bundled one is static — so the arm dispatches on what is actually loaded rather than on a build-time assumption.
- **Failure stays a bypass, never a wrong artifact.** If enumeration fails, the arm returns `Err` exactly as today and the build proceeds uncached with a diagnostic. D1b removes the *unconditional* disablement; it does not remove the safety property that an unfingerprintable toolchain must not be cached against.
- **Verification:** a macOS build produces a cache hit on an unchanged rebuild, and swapping the LLVM installation produces a miss. The second half is the one that matters — a fingerprint that never changes would be worse than no cache at all.

### D2 — Eviction on by default, budget + recency, never `atime`

- **Collector root spans versions — ABSOLUTE.** The collector is rooted at `…/ori/build/`, **not** at `…/ori/build/<version>`. Because `CACHE_FORMAT_VERSION` is a path component, a version-scoped collector would never see prior-version trees, and every compiler upgrade that bumps the format would strand a full cache permanently — the exact unbounded accumulation this proposal exists to prevent, produced by its own migration mechanism. Prior-version roots are evicted first, ahead of every in-version class: they can never be hit again.
- **Budget:** the cache targets a maximum total size, configured via `ori cache budget <size|unlimited>` (persisted per-user) or the `ORI_CACHE_BUDGET` environment override. Exceeding it triggers collection. **The budget is hard and always wins over the recency floor** — that comparison only; the one admission-time exception, and the exact resulting bound, are stated in the *Terminal case* bullet below, which this sentence does not override.
- **`unlimited` is expressible, and saying so is the honest answer to `go#69565`.** The shipped `CacheConfig.max_size` already defines `0 = unlimited`; this proposal activates that field rather than adding a second knob beside it. A user who sets `unlimited` has opted out of bounding — that is the escape valve north-star users are asking for, and denying it silently would reproduce the pressure rather than answer it. T4 governs the **default**, which is bounded and on; it does not require the bound to be user-immovable. `ori cache info` reports an unlimited budget as such, so the state is never invisible.
- **Recency floor:** entries used within a recency window are preferentially retained, so the active working set survives ordinary collection. The floor is a *preference*, never an unconditional exemption.
- **Budget-vs-floor interaction — the budget is authoritative.** When the recency-protected set *alone* would exceed the budget, collection does not stop at the floor: it (a) narrows the recency window until the protected set fits, evicting the oldest protected entries first, and (b) stops admitting new entries for that build if the budget still cannot be met. A cache that stayed over budget because everything was recently used would not be bounded, and T4 requires bounded. The floor bounds *surprise*, the budget bounds *size*, and size wins.
- **Use-stamping is tool-owned, and lives OUTSIDE the verified manifest.** Last-use is recorded in a per-entry sidecar stamp file, written by the build tool when it reads the entry. `atime` is **not** consulted — it is disabled or coarsened on most Linux mounts (`relatime`/`noatime`).
- **The stamp is deliberately not in `ObjectManifest`.** Putting it there would make every cache *hit* a manifest *write*, with three consequences: it contradicts the no-lock-on-the-read-path guarantee below; it breaks read-only and shared-mount cache roots (a real CI shape, adjacent to `go#64721`); and it mutates a digest-verified envelope — `ObjectManifest` carries `deny_unknown_fields` and is validated against `request_sha256` on every read, so a mutable field would either cold-start every existing entry or change what the entry *is* on each write. A sidecar avoids all three and keeps the shipped ownership split intact ("the caller owns the manifest schema and semantic request identity").
- **A stamp write is best-effort and never fails a build.** An unwritable stamp (read-only root) degrades that entry to its publication time for ranking purposes; it does not disable caching.
- **The cache's own bookkeeping is inside the budget, not beside it.** Manifests, use-stamps, and any index or log the collector maintains count toward the budget and are themselves bounded and collected. A bookkeeping file that grows without limit and that the reclaim path skips is the same failure at one remove — Go shipped exactly that (`go#31068`, "cache log grows unbounded and `go clean -cache` ignores it"). Per-entry metadata is reclaimed with its entry; any shared index is compacted by the same collection pass. Nothing in the cache is exempt from the collector on the grounds that the collector wrote it.
- **When it runs:** opportunistically after a build when over budget, doing bounded work per invocation; and on demand via `ori cache gc`.
- **Guaranteed progress — the collector cannot starve.** Opportunistic collection with a non-blocking lock has no progress guarantee on a busy machine: every invocation may legitimately skip, and composed with admission-stop the steady state would be *over budget, admission off, collection never running, caching silently dead*. That is the Alternative-2 failure this proposal rejects by name, reached by accident. Therefore: when the cache has been over budget across `N` consecutive skipped attempts (`N` small, implementation-chosen), the next build **blocks** on the collection lock rather than skipping. Blocking on the collector is bounded work; unbounded growth is not.
- **Admission-stop is never silent.** Refusing to admit new entries emits a diagnostic naming the budget, the current size, and `ori cache gc` / `ori cache budget`. A permanent silent performance cliff is a worse failure than a slow build.
- **Terminal case.** If a single entry alone exceeds the budget it is admitted, because refusing it would make the cache useless for the workload that most needs it. A user is told which entry and what budget would hold it.

#### The composed bound — ONE invariant over the whole cache root (SSOT)

The budget is bounded, not absolute, and the exceptions **stack**. Two are irreducible at any instant, they arise from different classes, and nothing prevents both being active at once — an oversized incremental-state blob admitted under the terminal case while a CI matrix holds N concurrent build leases is an ordinary scenario, not a contrived one. Stating each exception separately as its own `max(budget, X)` understates the true worst case, because the collector can drive every *other* class to zero while both floors remain.

> **Total cache-root size never exceeds `max(budget, OVERSIZED + LEASED)`**, where
> `OVERSIZED` = the size of any single entry admitted under the terminal case above (`0` when none is live), and
> `LEASED` = the aggregate size of the concurrently-leased set (D3a references and `binary-generation-lifecycle-proposal.md` D3 generation leases together).

This is the **single** budget invariant for the whole cache root under D4 clause 3's one-budget mandate. `binary-generation-lifecycle-proposal.md` D4 cites it rather than restating it; two parallel invariants that must be kept in sync is exactly how they came to disagree.

Both terms are self-limiting rather than accumulating: `OVERSIZED` covers one entry, never a growing set, and `LEASED` is bounded by concurrent in-flight builds and released on D3's owner-gone-AND-expiry-passed rule. That is what "the budget is hard" means — a finite, stated, checkable ceiling, not an absolute one.
- **Order:** prior-version roots first; then least-recently-used within artifact class, respecting the recency floor.
- **Orphaned generations are collected by a sweep, not by the manifest walk.** `remove()` deliberately preserves the immutable object generation it invalidates, and each publish creates a *new* generation file rather than replacing one, so invalidation cycles and lost publication races both leave objects referenced by no manifest. Those orphans have no manifest (invisible to a stamp-keyed walk) and no live key (outside D3's regenerability argument as stated), so a manifest-driven collector would never reclaim them — "N rebuilds retain N copies" reappearing inside the class this proposal calls safe. Collection therefore includes a generation sweep: any generation file reachable from no manifest and holding no in-flight reference (D3a) is reclaimable regardless of age.

### D3 — Eviction is safe by construction — for regenerable entries only

For a **content-addressed, hermetically regenerable** entry, collection never yields a **wrong artifact**: an evicted entry is a cache miss whose content is reproducible from the same inputs that named it. There is no reachability analysis to get wrong, and no risk of a stale or mismatched result.

**That is narrower than "safe", and the difference is load-bearing.** Regenerability protects a consumer that has *not yet resolved* — it misses and rebuilds. It does **not** protect a consumer that already resolved and holds a path: `get_verified` returns a `PathBuf` that `oric` hands to the linker, and a linker given a path does not re-resolve. Reclaiming that entry mid-link yields `ENOENT`. This is the hazard `ArtifactCache::remove`'s doc comment records (quoted in full under *What already exists*), and it applies to the **collector**, not only to `ori cache clean`. Go has the same class open today (`go#31948`, `go#69566`).

An earlier draft claimed "no reader depends on a particular copy being the current one." That is false: a linker holding a path does. The claim is withdrawn.

**D3a — the in-flight-reference primitive (owned here).** This proposal therefore owns a per-entry in-flight-reference check, and the collector consumes it:

- **Acquire-then-verify, never resolve-then-acquire.** A build takes the reference on the key **before** resolution returns a path, then verifies the entry still exists before using it; on a lost race it releases and retries. Registering *after* resolution would leave open exactly the window the primitive exists to close — resolution completes, the collector reclaims, the path is handed to the linker, `ENOENT`. This is the same ordering `binary-generation-lifecycle-proposal.md` D3 states for generations, and it is stated here because this is where the primitive lives.
- The reference is held for as long as the build may still hand that path to a subprocess, released when the last such use completes.
- The collector reclaims an entry only after observing it is both eviction-eligible **and** unreferenced, with the check inside the same critical section that removes it.
- **Crash recovery requires BOTH signals, matching the successor.** A reference records an owning process identity (PID plus start time, against PID reuse) and an expiry. It is reclaimable only when the owner is proven gone **and** the expiry has passed. Either signal alone is insufficient: a long link can outlive a conservative expiry, and a recycled PID can impersonate a dead owner. A crashed build therefore never permanently pins an entry, and a slow one is never reclaimed underneath.
- `ori cache clean` (D6) is a second consumer of the same primitive, not a separate mechanism.

This is the minimum coordination that makes on-by-default collection correct, so it belongs to the proposal that turns collection on. `binary-generation-lifecycle-proposal.md` **extends** this primitive to current-generation reclaim, where a reader additionally depends on a *specific* generation being current — matching its declared `Depends On:` on this proposal. The dependency runs one way: primitive here, extension there.

This is the property that lets collection be on by default, and it is **scoped deliberately** — it does not extend to every artifact the cache touches:

| Class | Safe to evict freely? | Why |
|---|---|---|
| Intermediate objects, IR, metadata, incremental state | **Yes** | Content-addressed, hermetic, regenerable from their key |
| Final binaries | **Out of scope** | Keyed by build identity, not content; needs supersession + live-reader coordination (successor proposal) |
| Debug sidecars for cached intermediates (D5) | **Yes — jointly with their object** | Same content-addressed key, so they cannot be orphaned |
| Measured profile data (`test-driven-pgo`) | **No — a third class; expensive-to-regenerate** | Reproducible only by re-executing tests, not from a cache key. Retained under budget, reclaimed at **rank 6** of the global order (`binary-generation-lifecycle-proposal.md` D4) — after every regenerable class, before live final-binary generations — and never by the recency floor alone |
| Installation-managed components (bundled linker/SDK) | **No — excluded from GC entirely** | Installed, not built; not regenerable from a cache key. Installed alongside `ori`, outside the cache root, so the collector cannot reach them regardless |

Where a per-project non-content-addressed layout must reason about which artifacts a future configuration might still need, Ori's regenerable classes avoid that problem rather than solving it: no reachability analysis is required, because the key that named the entry also reconstructs it. **This is the seam the proposal is scoped to.** Classes without that property — final binaries, measured profiles, installed components — do not inherit the argument and get explicit treatment instead of being covered by prose.

`test-driven-pgo-proposal.md` (draft) is the case that forced the third row: it places profile data at a project-local `.ori/pgo/` with no lifecycle owner and an open question about invalidation across LLVM versions. Reconciling it here rather than letting it accumulate beside a bounded cache is what the classification criterion is for.

### D4 — Per-artifact-class retention

| Artifact class | Storage | Rationale |
|---|---|---|
| Intermediate objects, IR, metadata | Content-addressed, retained under budget | Small, high reuse across projects and configurations |
| Incremental-compilation state | Content-addressed, retained under budget, evicted first | Large and regenerable; the dominant term in the motivating failure |
| Debug sidecars for cached intermediates | Content-addressed with their object; reclaimed with it | Same key, same lifetime — no cross-key orphaning |
| Measured profile data (`test-driven-pgo`) | **Relocated into the cache root**, keyed by the profile's producing inputs; retained under budget, reclaimed at global rank 6 | It cannot stay at a project-local `.ori/pgo/`: D4 clause 3 requires one budget over one cache root, and a store the collector cannot reach is by definition outside the bound. Relocation is the condition of the classification, not a consequence of it |
| **Final binaries and their sidecars** | **Out of scope — mechanism owned by `binary-generation-lifecycle-proposal.md`** | See below |

**Final-binary retention is deliberately out of scope.** This proposal states the *outcome* T4 requires and defers the mechanism:

> 1. A rebuild MUST NOT cause the cache to accumulate one retained copy per build.
> 2. **The set of retained final-binary generations MUST itself be bounded, and every generation in it MUST be reclaimable under budget pressure — including current ones.** No class may be permanently exempt from the collector; a budget that cannot reach its largest class is not a bound.
> 3. Final-binary storage counts against **this proposal's budget**. There is one budget over one cache root, not two.
> 4. A final binary MUST NOT be mutated underneath a process reading or executing it.
> 5. A binary's debug sidecar MUST share its artifact's lifetime AND remain discoverable from the materialized deliverable (see D5a).

Clause 2 is the one that does the binding work. Without it a successor could retain exactly one generation per build identity forever, across unboundedly many identities, and satisfy every other clause verbatim while leaving the cache unbounded — the precise failure T4 exists to prevent.

The mechanism satisfying that outcome — generation identity (which must distinguish `--lib` / `--dylib` / `--wasm` / `--emit` / `-o` outputs, not merely project/profile/target), current-pointer publication with correct cross-platform replace semantics, deliverable materialization for a path a user may be executing, reclaim coordination against live readers, and the budget interaction for the set of current generations — is owned by `binary-generation-lifecycle-proposal.md`.

That split is deliberate. Those problems are a concurrent-systems design with real platform divergence (Windows locks in-use executables and its `rename` does not replace); they are not settled by prose in a cache-policy proposal. Keeping them here produced three successive rounds of second-order races. Everything above this row is content-addressed and idempotent, where the existing publication primitive is already sound — which is precisely why the seam falls here.

One principle does stay here, because it governs eviction policy for every class: **history is not a build-cache responsibility.** Version control owns history. Retention exists to make the next build fast and to protect live readers — never to preserve past builds. An entry no build will ask for again has no claim on disk.

### D5 — Debug info separated via platform-native mechanisms

D5 governs what the compiler **emits**, not how a binary is retained. It stays here because "debug information is separable" is a T4 outcome about artifact composition, and because it is settled by adopting existing platform conventions rather than by a concurrency design. Where a separated sidecar is then *stored and reclaimed* alongside a final binary is the successor proposal's concern.

`aot-compilation-proposal.md`'s Debug Format table reads, verbatim: `| Linux | DWARF 4 | Default |`; `| macOS | DWARF 4 + dSYM | Split debug |`; `| Windows | CodeView/PDB | MSVC standard |`; `| WASM | DWARF 4 | Source maps |`. It also carries the explicit Design Decision "Separate dSYM files by default on macOS".

- The Linux row's Standard column is literally `Default`; reading it as **embedded** is this proposal's interpretation by contrast with the macOS `Split debug` row, not the table's wording.
- macOS and Windows are adopted **unchanged**. This proposal introduces no parallel discovery mechanism for them.
- The WASM row's `Source maps` value is **unchanged and out of scope** — this proposal makes no claim about WASM debug format.

**Linux is a new decision this proposal makes, not one it inherits.** Embedded DWARF does not satisfy T4's "debug information is separable" outcome — macOS and Windows already do by construction, Linux does not. This proposal therefore **amends the Linux row** to default to split debug info (`.dwo` / `.debug` companion located by the platform's own `build-id` / `.gnu_debuglink` linkage). The frontmatter `Amends:` field names this row explicitly, and Spec & Grammar Impact plans its own errata entry.

- Debug info is emitted into the **platform-native** sidecar and located by the platform's own linkage (build ID / `.gnu_debuglink` on Linux, the dSYM bundle convention on macOS, the PDB path record on Windows).
- **The cache manifest is never the debugger's discovery path.** An external `gdb`/`lldb` cannot consult an Ori-internal manifest; the earlier "located through the cache manifest" mechanism was incompatible with both the approved decision and standard tooling interop.
- **Detail level stays `--debug=0|1|2`** exactly as approved (`--debug=0` already means no debug info). This proposal adds **no** second flag governing an overlapping state; the earlier `--debug-info=none` duplicated `--debug=0` and left `--debug=2 --debug-info=none` undefined.
- What this proposal adds is only the **default**: split rather than embedded, plus `--debug-info=split|embedded` for placement where a workflow requires a self-contained artifact. `--debug=0` means no debug info is emitted, so `--debug-info` has nothing to place: the combination is a **no-op, not an error**, and `--debug-info` is silently inert under `--debug=0`. (The rejected `--debug-info=none` was different — it duplicated `--debug=0`'s meaning rather than being subsumed by it.)

### D5a — Sidecar discovery must survive materialization

Split debug info only works if the debugger can find the sidecar from the binary it actually opened. That composition crosses the seam and is stated here so it is owned rather than assumed:

- `.gnu_debuglink` and the Windows PDB path record resolve relative to — or as an absolute path baked beside — **the binary the debugger opened**, which is the materialized deliverable at the user's output path, not the cache generation.
- A sidecar that lives only inside a cache generation is therefore unreachable from the deliverable, and on Windows a PDB path fixed at link time would point into reclaimable cache storage.
- **Requirement:** a sidecar is materialized alongside its deliverable by the same mechanism that materializes the deliverable, and its recorded discovery path resolves against the deliverable's location. Build-ID lookup is the preferred Linux mechanism precisely because it is location-independent.
- `binary-generation-lifecycle-proposal.md` D5 materializes the deliverable; per D4 clause 5 it materializes the sidecar with it. Neither proposal may treat the sidecar as materialized "for free".

### D6 — `ori cache` subcommands

Per T1 (one tool) and T2 (canonical defaults, operational flags permitted):

- `ori cache info` — location, total size, budget, entry counts by class, reclaimable amount.
- `ori cache gc [--dry-run]` — force collection; `--dry-run` reports without removing.
- `ori cache budget <size|unlimited>` — set the budget (D2); with no argument, report it.
- `ori cache clean` — remove **everything in the cache root**, including final-binary storage once the successor lands. It does not reach installation-managed components, which live outside the root. A `clean` that left the largest class behind while reporting success would be a lie to a user out of disk space; the successor's generations are reclaimed here under D3a's reference check like any other class.

**`clean` is not bulk eviction, and it does not inherit D3's safety argument.** D3 establishes that evicting a *regenerable entry* is safe because a consumer that loses the race re-resolves or rebuilds. That argument covers the collector; it does **not** cover `clean`, because a build that has already resolved an entry and handed its path to the linker does not re-resolve — it gets `ENOENT` mid-link. Go has this bug open today (`go#31948`, "cmd/go: concurrent build and cache clean is unsafe"), alongside `go#69566` ("invoking `go run` from `go test` can corrupt build cache"), so it is a demonstrated failure of the north star, not a hypothetical.

Therefore:

- `clean` acquires the collection lock and **blocks unconditionally** rather than skipping — unlike the collector, which blocks only after D2's `N` consecutive over-budget skips. A user asking to reclaim everything wants it to happen, not to be silently skipped.
- `clean` refuses while any build holds an in-flight entry reference, reporting which build and exiting non-zero rather than removing under it. `--force` overrides for the wedged-state case and says plainly what it may break.
- The mechanism is D3a's in-flight-reference primitive, owned by this proposal. `clean` and the collector are both its consumers; neither invents its own.

### Concurrency and error handling

- **Entry publication** is atomic write-then-rename (the shipped `atomic.rs` discipline); readers never observe a partial entry, and no lock is required on the read path.
- **Reclaim coordination is D3a's in-flight-reference primitive, and it applies to the collector.** An unreferenced entry is reclaimable; a referenced one is not, checked inside the same critical section that removes it. This closes the `ArtifactCache::remove` hazard quoted under *What already exists*. `go#43645` (closed) and `go#31948` / `go#69566` (both open) are the north-star precedents; `zig#9258` (a closed PR, cited as design discussion) is the prior-art locking design.
- **Nested and recursive invocations share one reference set.** A build that spawns a child `ori` invocation (the `go#69566` shape — `go run` from `go test`) must not have the child's collection reclaim entries the parent holds. References are keyed by the resolving process and inherited by descendants for the parent's duration.
- **Lock contention is non-blocking by default, with one mandatory exception.** A build that cannot acquire the collection lock normally skips collection for that invocation and proceeds. The exception is D2's guaranteed-progress rule: after `N` consecutive over-budget skips the next build **blocks** on the lock rather than skipping. "Never stalls" is the common case, not an invariant — an unconditional non-blocking collector cannot guarantee the bound, which is why D2 overrides it.
- Cache directory unwritable or full → the build proceeds without caching, emitting a diagnostic naming the path and the `ORI_CACHE` override.
- Corrupt or truncated entry (digest mismatch on read) → treated as a miss, entry removed, artifact rebuilt. A corrupt cache never yields a wrong build.

---

## Drawbacks

- **This amends an approved decision.** `aot-compilation-proposal.md` chose project-local caching deliberately; overturning it costs churn and requires the migration path above. The trade is accepted because per-project lifetime is the specific property that produces unbounded growth and cross-project duplication.
- **A global cache is a shared failure domain.** Corruption or a bad eviction affects every project on the machine. Mitigated by digest-verified reads and atomic writes, but the blast radius is genuinely wider than per-project isolation.
- **Eviction can surprise.** A collection between two builds turns an expected fast rebuild slow. Bounded by the recency floor and budget, but the experience is a real cost of any automatic GC.
- **The T4 outcome is only partly delivered here.** Bounding the cache does not by itself stop N rebuilds retaining N binaries; that half waits on the successor proposal. Splitting the work is what makes each half reviewable, but it does mean T4 is satisfied by the pair, not by this proposal alone.
- **Split debug info adds a moving part.** A lost or mismatched sidecar degrades debuggability; embedding stays available for workflows that need it.

---

## Alternatives Considered

### Alternative 1: Keep the approved project-local `build/cache/` and add GC to it

Retain the approved decision and bound each project's cache independently. **Rejected:** it bounds growth but keeps the duplication (N projects still compile a shared dependency N times), and it forces exactly the reachability question a per-project non-content-addressed layout creates. It also diverges from the *already-shipped* global cache, leaving two cache models in one toolchain.

### Alternative 2: No automatic GC; ship a maintenance command

Provide `ori cache gc` but leave it off by default. **Rejected:** this is the failure being corrected. An opt-in cleanup nobody runs is how a disposable cache reaches hundreds of gigabytes. T4 requires the creator to own the lifecycle.

### Alternative 3: `atime`-based LRU eviction

Use filesystem access time to drive LRU. **Rejected:** `atime` is disabled or coarsened on most modern Linux mounts (`relatime`/`noatime`). Tool-owned use-stamping gives an accurate, portable record — and the shipped cache already owns its manifest.

### Alternative 4: Content-address everything, and keep final binaries in scope

Extend the content-addressed scheme to final binaries and let this proposal cover them too. **Rejected — and the rejection is what defines this proposal's boundary.** Content-addressing is a *naming* scheme: it gives no "this generation supersedes that one" signal, because two builds with different content are simply two valid entries. Retention then has nothing to prune against, and every distinct-content build coexists — exactly the "N rebuilds retain N copies" term in the motivating measurement.

Final binaries need supersession and live-reader coordination, which are a different problem with different failure modes (platform-divergent replace semantics, in-use executable locks, resolve-then-acquire races). Three review rounds on a combined draft produced findings concentrated entirely in that half. Splitting is therefore not deferral — it separates a problem where eviction is safe by construction from one where safety must be *established*, and lets each be reviewed against its own hazards.

### Alternative 5: Solve final-binary retention here with in-place overwrite

Write each new binary directly over the old path, avoiding generations entirely. **Rejected:** it corrupts any process currently executing or linking that file, and it converts the reclaim hazard `ArtifactCache::remove` documents from solvable into unavoidable. Rejecting it does not make the problem this proposal's, though — it establishes that the problem needs a real concurrency design, which is the successor proposal's subject.

---

## Purity Analysis

**Can be pure Ori?** NO.

**If not, why:** This governs the compiler driver's on-disk artifact storage, cache-key derivation from compilation inputs, and artifact emission (including debug-info separation at the codegen/link boundary). It lives in `oric` and the `ori_llvm` AOT/incremental layer.

**Missing features that would enable purity:** N/A — build-artifact storage is toolchain infrastructure by nature.

**Recommendation:** Proceed as a compiler/toolchain feature realizing T4 of `toolchain-philosophy-proposal.md`, amending `aot-compilation-proposal.md`'s cache-location decision.

---

## Spec & Grammar Impact

- **No grammar changes.** No new productions, keywords, or syntax.
- **No normative language-spec clause changes.** Cache layout and lifecycle are toolchain behavior.
- **New CLI surface:** the `ori cache` subcommand family (`info`, `gc`, `budget`, `clean`) and the `--debug-info=split|embedded` placement flag. `--debug=0|1|2` is unchanged.
- **New environment variables:** `ORI_CACHE` (root override) and `ORI_CACHE_BUDGET`. Both join the `is_unfingerprinted_environment` exemption list in the same change that introduces them (per D1) — otherwise setting either disables the cache.
- **Umbrella errata:** `toolchain-philosophy-proposal.md`'s facet map records T4's owner as "*new proposal to follow (no owner yet)*", singular. T4 is satisfied by this proposal **and** `binary-generation-lifecycle-proposal.md` together; the facet-map row receives an errata entry naming both when the pair is approved.
- **Amendments — TWO errata entries, both required.** On approval, `aot-compilation-proposal.md` receives an errata entry for its **Incremental Compilation Cache decision** (superseded by D0) AND a second entry for its **Debug Format table, Linux row** (superseded by D5), per the errata format in the proposals rule. That proposal currently has no `## Errata` section; both entries are new. Recording only the cache entry would leave an approved decision superseded with no errata trail.

---

## Prior Art

- **Go — the primary north star.** One shared build cache (`$GOCACHE`) reused across all projects, content-addressed, automatically trimmed. Go instrumented cache age and reuse distribution to inform policy (`go#22990`), evidence that eviction policy should be measured rather than guessed. Go also shipped the concurrency bug this design must avoid (`go#43645`, "build cache not safe for concurrent builds") and the relative-path bug (`go#69997`, `go clean -cache` silently ignoring a relative `GOCACHE`). Sandboxed environments losing the shared cache degrade to slow compiles (`go#64721`), bearing on the CI question below.

  **Counter-signal, recorded rather than argued away:** `go#69565` ("proposal: cmd/go: allow disabling build cache trimming") is *open*, and it is users of the north star asking for an opt-out from exactly the always-on trimming this proposal adopts. It does not overturn T4 — the umbrella settled that the creator owns the lifecycle — but it is evidence that a hard, non-overridable budget generates real pressure, and it argues that the default must be *well-chosen* rather than merely present. D2 answers it directly: `unlimited` is expressible, T4 governs the default rather than forbidding the override. Go also still has `go#31948` (concurrent build vs `clean` — answered by D3a + D6) and `go#69566` (nested invocation corrupting the cache — answered by the descendant-inheritance rule in Concurrency) open. *Every `go#` number above verified by title against the `go` issue corpus indexed in the intelligence graph; open/closed state as recorded there.*
- **Zig — content-addressed manifest, in-place incremental direction, and the locking design.** Zig's cache is content-addressed with an explicit manifest; shared-cache concurrency is a recognized design concern (`zig#9258`, "Shared Cache Locking"). Zig also hit the relative-cache-path family this proposal preempts: `zig#20129` ("setting global cache directory to relative path causes build failure of dependencies", closed), `zig#19284` ("Cross compile to Windows fails with relative global cache directory path", closed), `zig#25307` ("Global build cache directory", closed), and `zig#9215` ("windows: overriding cache dirs with build-exe failing", **still open**). `zig#20073` ("build: use absolute paths to local and global cache dirs") is the corresponding PR; the graph records it as a *closed* PR, which is not by itself evidence it merged, so it is cited as the attempted remedy rather than as a shipped fix. `zig#9258` is likewise a closed PR, cited as design discussion, not as shipped machinery. *Every `zig#` number above verified by title against the `zig` corpus indexed in the intelligence graph; open/closed state as recorded there.*
- **Rust / Cargo — the anti-pattern.** Per-project `target/`, never automatically collected, retaining the incremental cache, full debug info, and every historical build artifact. Cargo's *global registry* cache gained scheduled automatic collection before per-`target/` collection did — evidence that the eviction problem is tractable and that the per-project, non-content-addressed layout is what made `target/` hard. `cargo-sweep` exists to fill the missing lifecycle owner. *Verified against the `rust` issue corpus and the measured `target/` breakdown above.*
- **Ori's own shipped cache.** `oric/src/commands/build/incremental_cache.rs` + `ori_llvm/src/aot/incremental/cache/` already implement the global root, the input-derived key, atomic publication, and relative-path *detection* this proposal builds on. It does not implement absolute-path *enforcement* — detection currently bypasses the cache rather than failing (see D1). `ArtifactCache::remove`'s doc comment records the concurrent-reader hazard verbatim: "A concurrent linker may still hold the object path returned by [`Self::get_verified`]. [`Self::clear`] reclaims retained generations." The hazard note is on `remove`, not on `clear`; both are named here because the split matters to who must close it. *Verified by direct source read of `cache/mod.rs`, `cache/atomic.rs`, and `incremental_cache.rs`.*

---

## Unresolved Questions

- **Default budget value.** Should be derived from measured reuse distribution (the data Go gathered in `go#22990`) rather than guessed; the concrete default and whether it scales with available disk resolve during implementation.
- **Recency-floor window.** Needs an empirical value; too short reintroduces surprise rebuilds, too long weakens the budget.
- **Incremental-state granularity.** Per-crate, per-module, or per-function caching of incremental state determines both reuse rate and eviction granularity; resolves with the incremental design.
- **CI cache locality.** Whether CI shares the global cache or uses a project-local root (favoring reproducibility and external cache restore, per `go#64721`) is open. What is **not** open: whichever root CI uses stays bounded and self-evicting per T4, unless the CI environment is provably ephemeral (a container discarded per job) or a named external owner bounds it. "Collection disabled" is not an available CI default.
- **Migration of existing caches.** Whether an existing `ori-build-object-v2` cache is adopted in place, re-keyed, or cold-started when the multi-file cache format lands.
- **DWARF normalization form.** D1a's path normalization is in scope and required; the concrete encoding (project-root-relative rewrite at emission versus a link-time mapping table) resolves during implementation. What is **not** open: normalization precedes removal of the path fields, and diagnostics render the consuming project's paths.
- **Other unsupported platforms.** D1b closes macOS. Any remaining target without a loaded-image enumeration mechanism still falls to the `Err` arm and builds uncached — correct but unaccelerated. Which platforms that leaves depends on the supported-target set at implementation time.
- **Reference-expiry window.** D3a's in-flight reference needs an expiry longer than the slowest realistic link; the concrete value comes from measurement. **Settled, not open:** the reference carries PID plus start time and reclaim requires owner-gone AND expiry-passed (D3a). What is open is only the *portable detection mechanism* for owner-liveness across supported platforms.
- **Default budget value.** Open per the note above; `unlimited` is expressible but is not the default.
