# Proposal: Build Cache Lifecycle and Garbage Collection

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-07-21
**Affects:** Build system, compiler driver (`oric`), CI, release packaging, developer disk footprint
**Depends On:** toolchain-philosophy-proposal.md (approved)
**Amends:** aot-compilation-proposal.md (approved) — its Incremental Compilation Cache decision
**Related:** multi-file-aot-proposal.md (approved), self-contained-toolchain-proposal.md (draft), binary-generation-lifecycle-proposal.md (successor — owns final-binary publication and reclaim)

---

## Summary

Ori's build cache gains an **automatic, bounded eviction lifecycle** it does not have today. This proposal migrates the existing single-source object cache — already global, already content-addressed, already atomic — into the general build-artifact cache, adds the garbage collection it currently lacks, and separates debug information so a shipped artifact is code rather than symbols. Cache administration becomes `ori cache` subcommands. Scope is deliberately bounded to **content-addressed, hermetically regenerable entries**, where eviction is safe by construction; final-binary publication and reclaim — which need supersession and live-reader coordination, not content-addressing — are the successor proposal `binary-generation-lifecycle-proposal.md`.

This proposal owns the **mechanism** for invariant **T4 ("The Creator Owns the Lifecycle")** of the approved `toolchain-philosophy-proposal.md`, which states the outcome contract: the cache is bounded and self-evicting, debug info is separable, and a per-project never-evicted cache is the explicitly rejected anti-pattern.

It **amends** the approved `aot-compilation-proposal.md`, whose Incremental Compilation Cache section decided a project-local cache in `build/cache/`. That decision is superseded below with its rationale addressed directly, not silently reversed.

---

## Motivation

### What already exists

Ori already ships a cross-invocation, content-addressed object cache for the single-source `ori build` path:

- `oric/src/commands/build/incremental_cache.rs` — `CacheProbe::{Hit, Miss, Bypass}` over a `CacheKey { source_hash, deps_hash, flags_hash }`, publishing an `ObjectManifest { schema, request_sha256, object_sha256, object_size }` under `CACHE_FORMAT_VERSION = "ori-build-object-v2"`.
- `ori_llvm/src/aot/incremental/cache/` — `ArtifactCache` with a two-step publication discipline in `atomic.rs`: generation *content* is written to an exclusively-created path (`publish_generation_bytes` / `publish_generation_file`, `create_new`, no rename), and the *metadata* that makes a generation discoverable is published by write-then-rename (`publish_bytes_atomically` → `fs::rename`). Content creation and visibility are already distinct steps.
- The cache root is already **global and per-user** (`$XDG_CACHE_HOME/ori/build/<version>`, falling back to `~/.cache/ori/build/<version>`), and already **rejects a relative `XDG_CACHE_HOME`** with an explicit diagnostic.

So the foundation this proposal needs is largely built. What is missing is the **lifecycle**: there is no eviction, no budget, and no `gc`. `ArtifactCache::remove`'s doc comment records the residual reclaim hazard — "a concurrent linker may still hold the object path returned by `get_verified`" — and directs reclaim to `clear`, which is a blunt directory removal.

*Absence-claim grounding:* the "no eviction / no budget / no `gc`" claim was established by reading `oric/src/commands/build/incremental_cache.rs` and `ori_llvm/src/aot/incremental/cache/{mod,atomic,tests}.rs` in full and searching `compiler/` for eviction, budget, and collection entry points; no such surface exists. This is a claim about implementation, verified against implementation.

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

- Migrate the existing single-source object cache into the general build-artifact cache, preserving its content-addressing, atomicity, and absolute-path discipline.
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

- **Invalidation simplicity** — content-addressing already provides it. The live `CacheKey { source_hash, deps_hash, flags_hash }` makes an entry's identity its inputs; there is no invalidation logic to get wrong, project-local or not. The shipped global cache demonstrates this in production today.
- **Cross-project coherency** — content-addressing removes the coherency question by construction: two projects resolving to the same key resolved to the same inputs, so sharing the entry is correct, not risky. A differing input yields a different key, not a conflict.
- **Per-project cache lifetime** — this is the property being deliberately traded. Per-project lifetime is exactly what produces N copies of one dependency across N projects and what leaves each project's cache unbounded. Lifetime moves to the budget-and-recency policy in D2.

**Unchanged:** `build/obj/` and the AOT output layout stay exactly as approved. A project directory holds its *deliverables*; the cache holds *reusable intermediate work*. Deleting a project never destroys reusable work; deleting the cache never destroys deliverables.

### D1 — One global content-addressed cache (migration, not greenfield)

The existing cache root, key, manifest, and atomic publication are adopted as the foundation and extended to cover multi-file builds.

- **Location:** the current per-user root (`$XDG_CACHE_HOME/ori/build/<version>`), with an `ORI_CACHE` override.
- **Absolute-path discipline — mandatory.** `ORI_CACHE`, the project-local override, and the OS-conventional default are each resolved to an absolute path before any use; a relative path is **rejected with an explicit diagnostic naming the variable and the requirement**, never silently normalized, ignored, or joined against the current directory. This generalizes the rule `incremental_cache.rs` already enforces for `XDG_CACHE_HOME` ("must be an absolute path; set it to a writable absolute directory"), and it is the specific defect that shipped in both north stars (`zig#20129`, `zig#19284`, `zig#20073`, and `go#69997`, where `go clean -cache` *silently ignores* a relative `GOCACHE`).
- **Cross-project reuse:** an entry whose key matches is reused by any project on the machine.
- **Format versioning:** the existing `CACHE_FORMAT_VERSION` / `CACHE_MANIFEST_SCHEMA` pair gates migration; an unrecognized version is a cold cache, never a misparse.

### D2 — Eviction on by default, budget + recency, never `atime`

- **Budget:** the cache targets a configurable maximum total size. Exceeding it triggers collection. **The budget is hard and always wins** (see below).
- **Recency floor:** entries used within a recency window are preferentially retained, so the active working set survives ordinary collection. The floor is a *preference*, never an unconditional exemption.
- **Budget-vs-floor interaction — the budget is authoritative.** When the recency-protected set *alone* would exceed the budget, collection does not stop at the floor: it (a) narrows the recency window until the protected set fits, evicting the oldest protected entries first, and (b) stops admitting new entries for that build if the budget still cannot be met. A cache that stayed over budget because everything was recently used would not be bounded, and T4 requires bounded. The floor bounds *surprise*, the budget bounds *size*, and size wins.
- **Use-stamping is tool-owned.** Each entry's manifest records last-use, stamped by the build tool when it reads the entry. `atime` is **not** consulted — it is disabled or coarsened on most Linux mounts (`relatime`/`noatime`).
- **When it runs:** opportunistically after a build when over budget, doing bounded work per invocation; and on demand via `ori cache gc`.
- **Order:** least-recently-used within artifact class, respecting the recency floor.

### D3 — Eviction is safe by construction — for regenerable entries only

For a **content-addressed, hermetically regenerable** entry, collection is a **cost** decision, never a correctness one: an evicted entry is a cache miss whose content is reproducible from the same inputs that named it. There is no reachability analysis to get wrong.

This is the property that lets collection be on by default, and it is **scoped deliberately** — it does not extend to every artifact the cache touches:

| Class | Safe to evict freely? | Why |
|---|---|---|
| Intermediate objects, IR, metadata, incremental state | **Yes** | Content-addressed, hermetic, regenerable from their key |
| Final binaries | **Out of scope** | Keyed by build identity, not content; needs supersession + live-reader coordination (successor proposal) |
| Debug sidecars for cached intermediates (D5) | **Yes — jointly with their object** | Same content-addressed key, so they cannot be orphaned |
| Installation-managed components (bundled linker/SDK per `self-contained-toolchain`) | **No — excluded from GC entirely** | Installed, not built; not regenerable from a cache key |

Where a per-project non-content-addressed layout must reason about which artifacts a future configuration might still need, Ori's regenerable classes avoid that problem rather than solving it: eviction is safe *by construction*, because the key that named the entry also reconstructs it. **This is the seam the proposal is scoped to.** Classes that do not have that property — final binaries, installed components — do not inherit the safety argument and are therefore excluded rather than covered by prose.

### D4 — Per-artifact-class retention

| Artifact class | Storage | Rationale |
|---|---|---|
| Intermediate objects, IR, metadata | Content-addressed, retained under budget | Small, high reuse across projects and configurations |
| Incremental-compilation state | Content-addressed, retained under budget, evicted first | Large and regenerable; the dominant term in the motivating failure |
| Debug sidecars for cached intermediates | Content-addressed with their object; reclaimed with it | Same key, same lifetime — no cross-key orphaning |
| **Final binaries and their sidecars** | **Out of scope — mechanism owned by `binary-generation-lifecycle-proposal.md`** | See below |

**Final-binary retention is deliberately out of scope.** This proposal states the *outcome* T4 requires and defers the mechanism:

> A rebuild MUST NOT cause the cache to accumulate one retained copy per build. A final binary MUST NOT be mutated underneath a process reading or executing it. A binary's debug sidecar MUST share its artifact's lifetime.

The mechanism satisfying that outcome — generation identity (which must distinguish `--lib` / `--dylib` / `--wasm` / `--emit` / `-o` outputs, not merely project/profile/target), current-pointer publication with correct cross-platform replace semantics, deliverable materialization for a path a user may be executing, reclaim coordination against live readers, and the budget interaction for the set of current generations — is owned by `binary-generation-lifecycle-proposal.md`.

That split is deliberate. Those problems are a concurrent-systems design with real platform divergence (Windows locks in-use executables and its `rename` does not replace); they are not settled by prose in a cache-policy proposal. Keeping them here produced three successive rounds of second-order races. Everything above this row is content-addressed and idempotent, where the existing publication primitive is already sound — which is precisely why the seam falls here.

One principle does stay here, because it governs eviction policy for every class: **history is not a build-cache responsibility.** Version control owns history. Retention exists to make the next build fast and to protect live readers — never to preserve past builds. An entry no build will ask for again has no claim on disk.

### D5 — Debug info separated via platform-native mechanisms

D5 governs what the compiler **emits**, not how a binary is retained. It stays here because "debug information is separable" is a T4 outcome about artifact composition, and because it is settled by adopting existing platform conventions rather than by a concurrency design. Where a separated sidecar is then *stored and reclaimed* alongside a final binary is the successor proposal's concern.

`aot-compilation-proposal.md`'s Debug Format table decides: **Linux — DWARF 4, default (embedded)**; **macOS — DWARF 4 + dSYM, split** (also an explicit Design Decision, "Separate dSYM files by default on macOS"); **Windows — CodeView/PDB**; **WASM — DWARF 4**. This proposal adopts macOS and Windows unchanged and does **not** introduce a parallel discovery mechanism.

**Linux is a new decision this proposal makes, not one it inherits.** The approved table has Linux at embedded DWARF 4, which does not satisfy T4's "debug information is separable" outcome — macOS and Windows already do by construction, Linux does not. This proposal therefore **amends the Linux row** to default to split debug info (`.dwo` / `.debug` companion located by the platform's own `build-id` / `.gnu_debuglink` linkage). That amendment is stated here explicitly rather than presented as an existing decision; the `Amends:` header covers it.

- Debug info is emitted into the **platform-native** sidecar and located by the platform's own linkage (build ID / `.gnu_debuglink` on Linux, the dSYM bundle convention on macOS, the PDB path record on Windows).
- **The cache manifest is never the debugger's discovery path.** An external `gdb`/`lldb` cannot consult an Ori-internal manifest; the earlier "located through the cache manifest" mechanism was incompatible with both the approved decision and standard tooling interop.
- **Detail level stays `--debug=0|1|2`** exactly as approved (`--debug=0` already means no debug info). This proposal adds **no** second flag governing an overlapping state; the earlier `--debug-info=none` duplicated `--debug=0` and left `--debug=2 --debug-info=none` undefined.
- What this proposal adds is only the **default**: split rather than embedded, plus `--debug-info=split|embedded` for placement where a workflow requires a self-contained artifact.

### D6 — `ori cache` subcommands

Per T1 (one tool) and T2 (canonical defaults, operational flags permitted):

- `ori cache info` — location, total size, budget, entry counts by class, reclaimable amount.
- `ori cache gc [--dry-run]` — force collection; `--dry-run` reports without removing.
- `ori cache clean` — remove everything (the blunt escape hatch). Scoped to this proposal's classes; it does not reach installation-managed components (D3) and does not reach final-binary storage, which the successor proposal governs.

### Concurrency and error handling

- **Entry publication** is atomic write-then-rename (the shipped `atomic.rs` discipline); readers never observe a partial entry, and no lock is required on the read path.
- **Reclaim coordination is scoped to content-addressed entries.** Collection here reclaims content-addressed entries, which are idempotent: a reader that loses a race simply re-resolves or rebuilds, and no reader depends on a *particular* copy being the current one. `ArtifactCache::remove`'s doc comment records the residual hazard for object generations — "a concurrent linker may still hold the object path returned by `get_verified`" — and directs reclaim of retained generations to `clear`. Closing that hazard for **final binaries**, where a reader does depend on a specific current generation, requires the lease protocol owned by `binary-generation-lifecycle-proposal.md`; `go#43645` ("build cache not safe for concurrent builds") is the north-star precedent and `zig#9258` ("Shared Cache Locking") the prior art it must answer.
- **Lock contention** is non-blocking: a build that cannot acquire the collection lock skips collection for that invocation and proceeds, never stalls.
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
- **New CLI surface:** the `ori cache` subcommand family and the `--debug-info=split|embedded` placement flag. `--debug=0|1|2` is unchanged.
- **Amendment:** on approval, `aot-compilation-proposal.md` receives an errata entry recording that its Incremental Compilation Cache decision is superseded here, per the errata format in the proposals rule.

---

## Prior Art

- **Go — the primary north star.** One shared build cache (`$GOCACHE`) reused across all projects, content-addressed, automatically trimmed. Go instrumented cache age and reuse distribution to inform policy (`go#22990`), evidence that eviction policy should be measured rather than guessed. Go also shipped the concurrency bug this design must avoid (`go#43645`, "build cache not safe for concurrent builds") and the relative-path bug (`go#69997`, `go clean -cache` silently ignoring a relative `GOCACHE`). Sandboxed environments losing the shared cache degrade to slow compiles (`go#64721`), bearing on the CI question below. *Verified against the `go` issue corpus indexed in the intelligence graph.*
- **Zig — content-addressed manifest, in-place incremental direction, and the locking design.** Zig's cache is content-addressed with an explicit manifest; shared-cache concurrency is a recognized design concern (`zig#9258`, "Shared Cache Locking"). Zig also shipped the relative-cache-path family this proposal preempts (`zig#20129`, `zig#19284`, fixed by `zig#20073` "use absolute paths to local and global cache dirs", plus `zig#25307`, `zig#9215`). *Verified against the `zig` issue corpus indexed in the intelligence graph.*
- **Rust / Cargo — the anti-pattern.** Per-project `target/`, never automatically collected, retaining the incremental cache, full debug info, and every historical build artifact. Cargo's *global registry* cache gained scheduled automatic collection before per-`target/` collection did — evidence that the eviction problem is tractable and that the per-project, non-content-addressed layout is what made `target/` hard. `cargo-sweep` exists to fill the missing lifecycle owner. *Verified against the `rust` issue corpus and the measured `target/` breakdown above.*
- **Ori's own shipped cache.** `oric/src/commands/build/incremental_cache.rs` + `ori_llvm/src/aot/incremental/cache/` already implement the global root, content-addressed key, atomic publication, and absolute-path enforcement this proposal builds on. `ArtifactCache::remove`'s doc comment records the concurrent-reader hazard verbatim — "a concurrent linker may still hold the object path returned by `get_verified`" — and directs reclaim of retained generations to `clear`. The hazard note is on `remove`, not on `clear`; both are named here because the split matters to who must close it. *Verified by direct source read of `cache/mod.rs`, `cache/atomic.rs`, and `incremental_cache.rs`.*

---

## Unresolved Questions

- **Default budget value.** Should be derived from measured reuse distribution (the data Go gathered in `go#22990`) rather than guessed; the concrete default and whether it scales with available disk resolve during implementation.
- **Recency-floor window.** Needs an empirical value; too short reintroduces surprise rebuilds, too long weakens the budget.
- **Incremental-state granularity.** Per-crate, per-module, or per-function caching of incremental state determines both reuse rate and eviction granularity; resolves with the incremental design.
- **CI cache locality.** Whether CI shares the global cache or uses a project-local root (favoring reproducibility and external cache restore, per `go#64721`) is open. What is **not** open: whichever root CI uses stays bounded and self-evicting per T4, unless the CI environment is provably ephemeral (a container discarded per job) or a named external owner bounds it. "Collection disabled" is not an available CI default.
- **Migration of existing caches.** Whether an existing `ori-build-object-v2` cache is adopted in place, re-keyed, or cold-started when the multi-file cache format lands.
- **Ordering against the successor.** Whether `binary-generation-lifecycle-proposal.md` must be approved before this one implements, or the two implement independently. This proposal has no dependency on the successor's mechanism; the successor depends on this proposal's cache root and budget. Approval order is therefore free, implementation order is not.
