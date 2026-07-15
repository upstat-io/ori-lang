# Proposal: Ecosystem Interop Architecture (`use <lang>`)

**Status:** Draft (research)
**Author:** Eric (with AI assistance)
**Created:** 2026-07-15
**Affects:** grammar, type system, capability system, AIMS, runtime, FFI, stdlib, build system, spec
**Depends On:** `js-interop-typescript-bindings-proposal.md` (draft — first MEDIATED instance), `deep-ffi-proposal.md` (approved), `ffi-boundary-safety-proposal.md` (approved), `platform-ffi-proposal.md` (approved)
**Related:** `capability-propagation-completion-proposal.md`, `negative-effect-without-proposal.md` (drafts; inherited transitively via the js-interop instance)

---

## Summary

- This proposal defines HOW interop works in Ori — the single architecture by which ANY foreign ecosystem plugs into Ori's type, capability, lifetime, execution, and packaging systems. One import surface: `use <lang> "<source>" { ... }`.
- Doctrine: **other languages have an FFI; Ori imports ecosystems.** Declaration files are one instance's type-authority input, never the mechanism's identity — the corpus is whatever a registered reader serializes (declarations, runtime module graphs, package metadata; §2).
- Every instance answers a SIX-PART doctrine (§1): type authority, semantic gap-filling, capability authority, execution authority, lifetime authority, packaging authority. A partial answer is refused at review.
- The architecture is EXTENSION-BASED (§3): a language is a registered backend against a FIXED compiler surface. The enforcement CLASS carries the contract: a LABELED backend (direct native ABI) binds the compile-time shared core; a MEDIATED backend (embedded runtime) binds the compile-time shared core PLUS the runtime shared core (§3.5) — engine trait, scheduler integration, payload embedding, sandbox surface, handle-lifetime pattern, compat-layer pattern.
- Two v1 in-tree instances prove the contract: **`use js`** (MEDIATED; per `js-interop-typescript-bindings-proposal.md`, which instantiates the §3.5 contracts rather than owning them) and **`use c`** (LABELED; this proposal, §4 — C headers via libclang, sidecar-supplied ownership/error/capability metadata, output materialized as generated Deep-FFI bindings).
- The doctrine is BIDIRECTIONAL: the export direction — Ori as the imported ecosystem (C-ABI library emission, declaration emission from Ori public API) — is a named doctrine leg (§6). v1 ships no export instance; the slot exists so export proposals register into this architecture instead of re-deriving it.
- `extern` FFI is NOT replaced: it is demoted from user-facing surface to compilation target and escape hatch.

---

## Motivation

### The Problem in Practice — import leg

Binding a C library today is hand-written, per-symbol, and carries invariants the compiler never sees until the author transcribes them:

```ori
// Today: hand-bound, per-symbol, ownership transcribed by hand from the docs
extern "c" from "sqlite3" #error(nonzero) {
    @_open (filename: CPtr, db: out CPtr) -> c_int as "sqlite3_open"
    @_close (db: owned CPtr) -> c_int as "sqlite3_close"
    @_errmsg (db: CPtr) -> borrowed CPtr as "sqlite3_errmsg"
}
```

Every symbol is transcribed; every transcription is a defect opportunity; the library's own declaration of its interface (`sqlite3.h`) sits unread next to the hand-copy.

```ori
// This proposal: the header IS the binding source; the sidecar carries what it lacks
use c "sqlite3.h" { sqlite3_open, sqlite3_close, sqlite3_errmsg };
```

### The Problem in Practice — architecture leg

The `use js` draft proves that importing a real ecosystem is a whole-system property, not a declaration-translation feature. Its load-bearing machinery spans SIX systems: type translation, sidecar semantics, capability grounding, an embedded engine driven by the Ori scheduler, cross-runtime lifetime management, and AOT payload packaging. Without an umbrella that names all six, each future ecosystem (`use py`, `use java`) re-derives five of them ad hoc — the exact drift this architecture exists to prevent. A future embedded-CPython instance wants nearly every runtime contract the js instance built: interface-stub corpus (`.pyi`), thread-confined context (the GIL story mirrors the js single-context model), scheduler-driven pending-call pumping, payload embedding, and a capability-gated reimplementation of enough of the ecosystem's runtime API to get witnessed capability grounding.

### When This Matters

- Every C library a program touches (libsodium, sqlite3, zlib, PCRE2, platform APIs) today costs a hand-authored extern block; the stdlib's own FFI-backed modules pay it too.
- Every embedded-runtime ecosystem after JS pays the five-systems re-derivation tax unless the runtime contracts are umbrella-owned.
- Uniform surface is a language pitch, not just plumbing: the developer learns ONE interop model for JS, C, and future ecosystems — in both directions.

---

## Goals and Non-Goals

**Goals:**

- The `use <lang> "<source>"` grammar generalization with a registered-language-tag table (§2.2).
- The six-part doctrine (§1) every instance answers: type authority, semantic gap-filling, capability authority, execution authority, lifetime authority, packaging authority.
- The corpus generalization (§2.1): corpus = whatever the registered reader serializes — declarations, runtime module graphs, package metadata — so witnessed capability grounding (the js tier-0 `node:` import-graph computation) is representable in the backend contract.
- The EXTENSION architecture with CLASS-CARRIED contracts (§3): LABELED binds the compile-time shared core; MEDIATED binds compile-time + runtime shared cores. Every backend class has a named, bounded footprint (§3.6).
- The runtime shared core (§3.5), generalized out of the js instance: engine-trait shape with job-queue surface, scheduler-integration contract with quiescence/deadlock semantics, payload-embedding machinery, two-layer sandbox pattern, handle side-table + per-instance lattice-dimension pattern, compat-layer pattern with falsifiable pass-rate gating.
- Packaging as a doctrine part (§1 row 6): every instance states how its foreign payload ships in the AOT artifact.
- The export-direction doctrine leg (§6): Ori as importable library, declaration emission, callback trampolines as the shared mini-export surface — named, with instances deferred to companion proposals.
- `use c` as the second registered instance (§4): libclang-parsed headers, sidecar ownership/error/capability metadata, generated Deep-FFI bindings.

**Non-Goals:**

1. **Not `use cpp`.** C++ (templates, overloads, mangling, no stable ABI) is a separate research program; Swift's multi-year partial C++ interop is the scale evidence. A subset-feasibility study is a named future proposal, never an implicit extension of `use c`.
2. **Not `use py` / `use java` / further instances.** Each future instance (`.pyi` stubs + embedded CPython; `.class` metadata + embedded JVM) is its own proposal registering into this umbrella's backend contract — and inheriting the §3.5 runtime shared core when MEDIATED.
3. **Not a replacement for `extern`.** The hand-bound surface (per `platform-ffi-proposal.md` + `deep-ffi-proposal.md`) remains the substrate `use c` compiles TO and the escape hatch for anything a translator cannot express — including compiler-internal seams the developer owns on both sides.
4. **Not full C preprocessor support.** Object-like constant macros translate; function-like macros do NOT (escape hatch: a wrapper header or hand-bound extern). Subset boundaries are explicit, never silent.
5. **Not capability mediation for native code.** Native libraries execute their own syscalls; `use c` delivers capability LABELING (audited claims), never enforcement (per §5).
6. **Not any export instance in v1.** §6 names the export legs and their registration slot; each ships as its own proposal (`pub extern` C-ABI emission first, per §6.4 ordering).
7. **Not third-party backend distribution in v1.** The §3.2 contract makes out-of-tree backends structurally possible; tag namespacing, packaging, and trust policy decide WHEN (§Unresolved Questions).

---

## Design

### §1. The Doctrine — Six Parts, Per-Language Answers

Every instance of `use <lang>` supplies the same six parts:

| Part | Question it answers | `use js` (first instance, MEDIATED) | `use c` (this proposal, LABELED) |
|---|---|---|---|
| 1. Type authority (corpus) | Where do boundary types come from? | `.d.ts` TypeScript declarations | `.h` C headers via libclang |
| 2. Semantic gap-filling (sidecar) | What do the corpus artifacts structurally lack? | capabilities per export/callback | ownership (`owned`/`borrowed`/`#free`), error protocol, capabilities |
| 3. Capability authority | How are effects known, and how strongly? | MEDIATED; tier-0 GROUND TRUTH computed from the package's `node:` import graph (compat-layer builtins declare their own `uses`) | LABELED; sidecar-audited claims; honest gate is `uses FFI("<lib>")` |
| 4. Execution authority | What runs the foreign code, and who drives it? | embedded engine, Ori-scheduler-driven job queue (§3.5 contracts) | direct C ABI — no runtime, no driver |
| 5. Lifetime authority (AIMS) | How do cross-boundary lifetimes integrate? | `JsRef` lattice dimension + handle side-table (per-instance §3.5 pattern) | EXISTING machinery — `Locality::Borrowed(p)` + Deep-FFI ownership annotations; no new dimension |
| 6. Packaging authority | How does the foreign payload ship in the AOT artifact? | engine static-linked + package module graphs embedded as precompiled bytecode; pay-for-what-you-use linkage; lazy startup | native library linked per build-manifest configuration; no payload embedding |

- A new instance MUST answer all six parts in its registering proposal; a partial answer (types without lifetime story, boundary without packaging story) is refused at review.
- The LABELED row needs LESS new machinery than the MEDIATED row: no second runtime, no scheduler integration, no payload embedding, no lattice extension. The class-carried contracts (§3.3) make this difference explicit instead of accidental.

### §2. Corpus and Grammar

#### §2.1. Corpus — what the reader serializes

- A backend's corpus is the serialized machine-readable evidence set its registered corpus reader extracts from the ecosystem. Declaration files are the common case, NOT the definition. Corpus content classes:
  - **Interface declarations** — `.d.ts`, `.h`, `.pyi`, `.class` metadata.
  - **Runtime module graphs** — the package's resolved import graph (the js instance's tier-0 capability grounding is COMPUTED from each package's transitive `node:` import graph at translation time; that graph is corpus content, not sidecar content).
  - **Package metadata** — manifest fields (`package.json` `types`/`exports`/`main`, capability opt-in fields).
- The reader serializes; the backend's translator (pure Ori under const-eval, §3.2) consumes the serialized corpus + sidecar + build config and emits declarations, capability metadata, and binding specs.
- A corpus definition narrowed to "declaration files" cannot represent witnessed capability grounding — the strongest capability tier the architecture supports — and is therefore wrong by construction.

#### §2.2. Grammar — `foreign_import`

```ebnf
(* Amended source_file — foreign_import replaces js_import in the alternation *)
source_file    = [ file_attribute ] { import | foreign_import | reexport | extension_import } { declaration } .

foreign_import = "use" foreign_lang_tag string_literal foreign_import_list ";" .
foreign_lang_tag = identifier .   (* MUST be a tag claimed by an installed backend (§3.2); v1 in-tree
                                   * backends: "js", "c". Unregistered tag after `use` followed by
                                   * string_literal = E1520 (message lists installed tags). *)
foreign_import_list = "{" foreign_import_item { "," foreign_import_item } "}" .
foreign_import_item = "default" "as" identifier
                    | "*" "as" identifier
                    | "type" foreign_named_identifier [ "as" identifier ]
                    | foreign_named_identifier [ "as" identifier ] .
foreign_named_identifier = identifier .
```

- Disambiguation is UNCHANGED from the js-interop draft: after `use`, peek 2 tokens; `identifier` + `string_literal` selects `foreign_import`; the identifier is then checked against the tag registry (parse-time, E1520 with the registered-tag list on miss).
- Language tags are context-sensitive identifiers (only after `use`, before a string literal); `js` and `c` remain ordinary identifiers everywhere else.
- Per-tag item-form support varies (`default as` is JS-only; `type` applies to both); the parser accepts the superset, the per-language translator rejects unsupported forms with a tag-specific diagnostic.

### §3. Extension Architecture — a Language Is a Backend; the Class Carries the Contract

The system is EXTENSION-BASED. Adding a language means registering a backend against a fixed compiler surface; the compiler is NEVER modified per language except for the rare, shared components named below. The split follows the js-interop draft's own precedent (its `.d.ts` parser is pure Ori executed under const-eval — that mechanism IS the extension point, generalized here).

#### §3.1. The three layers, by mutability

| Layer | Owner | Changes per new language? |
|---|---|---|
| **Compiler surface** (fixed) | `foreign_import` grammar + tag registry, const-eval translation bridge, `TypeRegistry` injection + deserializer, capability plumbing, generic Deep-FFI binding consumer, trampoline machinery | NO — generic over every backend |
| **Corpus readers** (small shared set) | `oric` (impure driver): raw-text reader (`.d.ts`, `.pyi`), libclang reader (C headers → serialized language-neutral C-declaration form), module-graph resolver (package import graphs → serialized graph form) | RARELY — only when a new language needs a corpus format no existing reader produces. Readers are shared infrastructure, never per-language logic |
| **Backend** (the extension) | Translator + sidecar schema + (MEDIATED class) the §3.5 runtime-core instantiations | YES — this is ALL a new language adds |

#### §3.2. The backend contract

A backend registers:

```ori
trait LanguageBackend {
    @tag () -> str;                          // "js", "c" — claims the `use <tag>` surface
    @corpus_readers () -> [CorpusReaderId];  // which shared readers feed @translate (declarations,
                                             // module graphs, package metadata — per §2.1)
    @enforcement () -> EnforcementClass;     // Mediated | Labeled (§3.3 — the class binds the contract set)
    // Pure, const-evaluable: serialized corpus + sidecar + build config in,
    // serialized declarations + capability metadata + binding specs out
    // (the dts_serial.ori format family; deserialized + validated by the fixed compiler surface)
    @translate (corpus: str, sidecar: Option<str>, config: str) -> str;
}
```

- **Translators are pure Ori, executed at build time under const-eval** — resource-limited (the existing const-function limits), NO IO, NO capabilities. The compiler hands them strings and validates what comes back. A translator cannot read the filesystem, reach the network, or exceed its step budget — the build-time supply-chain surface of a backend is bounded by construction.
- **Binding specs are data.** For LABELED backends the emitted specs are Deep-FFI extern declarations the fixed surface already knows how to consume; for MEDIATED backends, boundary-trampoline specs. Either way the compiler's consumer is generic.

#### §3.3. Class-carried contracts

The enforcement class is not a documentation label — it is the contract selector:

| Class | Binds | Instances |
|---|---|---|
| **LABELED** (direct native ABI) | Compile-time shared core (§3.4) | `use c`; every future header-driven ABI language |
| **MEDIATED** (embedded runtime Ori owns) | Compile-time shared core (§3.4) + runtime shared core (§3.5) | `use js`; every future embedded-runtime ecosystem |

A MEDIATED registering proposal MUST state its instantiation of every §3.5 contract; a LABELED registering proposal MUST state that none applies (no runtime, no payload, no sandbox mediation).

#### §3.4. Compile-time shared core (every backend inherits)

| Component | Contract |
|---|---|
| Sidecar family | Per-source metadata resolved beside the corpus: `<pkg>.ori-caps.json` (JS), `<lib>.ori-ffi.json` (C). One resolution rule, one precedence model (author sidecar > package metadata > heuristic list > conservative default); field vocabulary declared by the backend's sidecar schema |
| Translation cache | `target/<lang>-bindings/<sha256>.ori-types` keyed on: corpus hash + sidecar hash + relevant build configuration + backend version + compiler version (the js-interop §2.5 key composition, generalized). Backend version in the key makes backend upgrades self-invalidating |
| Binding materialization | Bindings are compiler-internal; `--emit-bindings` renders them as readable `.ori` for inspection |
| Capability tiers | The js-interop §3.1 precedence table, per-language: tier-0 ground truth where a witnessed source exists (owned-runtime import graphs), sidecar/metadata/heuristic/conservative tiers otherwise |
| Diagnostics | The E1520-range foreign-import diagnostics, parameterized by tag; E1520 (unregistered tag) lists the installed backend tags |

#### §3.5. Runtime shared core (MEDIATED backends inherit; js instantiates, never owns)

Generalized out of the js-interop draft's §5, §5.1, §6, §7, §8, §9. Each row is an umbrella-owned contract; the js draft's corresponding sections become that instance's instantiation on this proposal's acceptance.

| Contract | Shape (umbrella-owned) | js instantiation |
|---|---|---|
| Engine trait | Per-instance engine trait with value/error assoc types, context creation, eval/call/property surface, to/from-Ori conversion, ref inc/dec, capability gate, AND the job-queue surface: `run_pending_job () -> Result<bool, Error>`, `has_pending_jobs () -> bool`, `next_timer_deadline () -> Option<Duration>` | `JsEngine` (js §5); JSC v1, trait seam keeps later engines a projection |
| Scheduler integration | The Ori scheduler is the SOLE driver of any embedded runtime; no foreign runtime owns a thread or event loop. Binding-site resolution loop (normative): drain pending jobs → settled converts / rejected errors; pending + armed timers → cooperative suspend until earliest deadline; pending + quiescent (no jobs, no timers, no in-flight host callbacks) → diagnosable deadlock error, NEVER a hang. Host timers and I/O completions arm on the Ori scheduler | js §5.1 (`JsPromise<T>` resolution; `setTimeout` shims; nextTick-ordering via compat layer) |
| Context threading model | Per-instance foreign context is thread-confined by default; values carrying foreign handles are blocked from crossing `Nursery`/channel boundaries at compile time; multi-context concurrency is per-instance v2 work | js §6 (thread-local `JsContext`; E4032; CN-12) |
| Handle lifetime pattern | Foreign-handle obligations tracked per-SSA in a side-table keyed by SSA index; interprocedural obligations by parameter/return slot in `MemoryContract`; a NEW orthogonal AIMS lattice dimension is added ONLY when the embedded runtime owns a collector — instance-scoped naming, field/table shapes designed so a sibling instance is a parallel dimension, not a refactor | js §4 (`JsRef` 8th dimension; `JsHandleObligation`; CN-9..CN-13) |
| Sandbox surface | Two-layer authority split: compile-time capability filtering is the type checker's claim, authoritative for statically-visible foreign code; runtime engine gates are authoritative for dynamically-loaded code; dynamic loading disabled by default at sandbox construction; memory/CPU quotas runtime-only. Per-instance API surface is FIXED at registration; additions require amendment proposals | js §7 (`JsSandbox`; E2058; `disable_dynamic_imports` default) |
| Payload embedding | The foreign payload ships inside the AOT executable: engine static-links beside `ori_rt`; payload embeds as precompiled engine-format bytecode (default; engine name+version keyed into the translation cache) or verbatim source (engine-version-independent fallback); pay-for-what-you-use — a program with no `use <lang>` import links neither engine nor payload; engine startup is lazy | js §8 (JSC static link; `bun build --bytecode` technique; three-layer artifact) |
| Compat-layer pattern | When the ecosystem's packages assume a runtime API surface (`node:*`, Python stdlib), reimplement that surface natively on Ori's capability-gated primitives — NEVER embed the ecosystem's own runtime (its syscalls bypass capability mediation). Each reimplemented builtin declares the Ori capabilities its implementation uses; those declarations are the tier-0 witnessed ground truth. Completion is gated on the ecosystem's OWN test suite: per-module pass-rates published, support claims checkable | js §9 (the Bun way; per-builtin `uses`; Node-test-suite gate) |

#### §3.6. Cost of adding a language, by class

| New language | Compiler changes | Extension work |
|---|---|---|
| LABELED, existing corpus reader (another header-driven ABI language) | ZERO | translator (pure Ori) + sidecar schema |
| LABELED, new corpus format | one shared corpus reader | translator + sidecar schema |
| MEDIATED (embedded runtime) | usually zero compiler-crate changes; ONE lattice dimension iff the runtime owns a collector (the only per-language compiler change that can exist) | translator + sidecar schema + the FULL §3.5 instantiation: engine binding library (+ vendored native shim, catalogued like `ori_rt`), scheduler integration, context-threading enforcement, payload embedding, sandbox instance, compat layer scoped to the claimed package tiers |

- A MEDIATED instance is bounded, named work — each §3.5 row is a checklist entry with an existing reference instantiation — but it is NEVER "zero plus a translator." Registering proposals that price a MEDIATED instance as LABELED-shaped are refused at review.

### §4. The `use c` Instance

#### §4.1. Resolution and build configuration

| Source form | Resolution |
|---|---|
| `use c "sqlite3.h" { ... }` | Header on the configured include path |
| `use c "./vendor/foo.h" { ... }` | Workspace-relative header |
| `use c "<system>/socket.h" { ... }` | System include (angle-bracket semantics) |

- Include paths, `-D` defines, and target sysroot are BUILD CONFIGURATION, never source-file content — declared in the build manifest, not in the `use c` statement. (Zig accepted the same conclusion for `@cImport` — zig#20630 moves it into their build system; Ori starts there instead of migrating later.)
- Packaging (doctrine part 6): the resolved library links into the AOT artifact per the build manifest's `link` configuration (sidecar `@library.link` names the library); no payload embedding, no runtime.
- Resolution + parse failures use a dedicated range: E1520–E1539.

#### §4.2. Parsing — libclang, not a bespoke C frontend

- Headers are parsed with libclang. `oric` already vendors the LLVM toolchain; the C-frontend dependency rides the existing ship.
- Writing a bespoke C parser + preprocessor (Zig's `translate-c` path, `zig/lib/compiler/translate-c`) is rejected per Alternative 3 — Zig's maintenance history of that component is the cautionary evidence.
- Parse happens at build time in `oric` (the impure driver crate), mirroring the js-interop const-eval bridge's IO confinement: core crates see translated declarations, never libclang.

#### §4.3. Type translation (v1 table)

| C construct | Ori translation | Notes |
|---|---|---|
| `int`, `long`, `size_t`, ... | `c_int`, `c_long`, `c_size`, ... | Existing C-type vocabulary per `platform-ffi-proposal.md` |
| `float` / `double` | `c_float` / `c_double` | |
| `char *` (param, by convention a string) | `CPtr`; sidecar `"@string": true` upgrades to `str` marshalling | Headers cannot distinguish string vs byte-buffer vs out-param — sidecar decides |
| `T *` | `CPtr` (opaque default); sidecar refines (`owned`/`borrowed`/`out`/`#borrow_from(p)`) | Ownership is NEVER inferred from the header |
| `struct` (complete definition) | Ori record with `#repr("c")` | Field-by-field translation; bitfields → opaque fallback (`COpaque`) |
| `struct` (forward-declared / incomplete) | opaque newtype over `CPtr` | The common handle pattern (`sqlite3 *`) |
| `enum` | Ori sum type with explicit discriminants | |
| `union` | `COpaque` (accessor functions via sidecar) | No safe direct translation |
| function pointer | `(args) -> R` boundary-thunked callback | Callback capability rules per js-interop §2.4 apply verbatim |
| variadic (`...`) | translated ONLY with sidecar opt-in; requires `unsafe` per existing C-variadic rules | |
| object-like macro (`#define N 4096`) | `let $n` constant when the expansion is a constant expression | |
| function-like macro | NOT translated (E1530 names the escape hatch) | Non-Goal 4 |

#### §4.4. Sidecar — `<lib>.ori-ffi.json`

The sidecar carries exactly the three things headers structurally lack:

```json
{
  "sqlite3_open":   { "@error": "nonzero", "@oriParams": { "db": "out" } },
  "sqlite3_close":  { "@params": { "db": "owned" }, "@free-role": "sqlite3" },
  "sqlite3_errmsg": { "@returns": "borrowed", "@borrow_from": "db" },
  "@library": { "capabilities": ["FileSystem"], "link": "sqlite3" }
}
```

- **Ownership**: `owned` / `borrowed` / `out` / `#free(<fn>)` / `#borrow_from(p)` — the Deep-FFI Phase-2 vocabulary, supplied as data instead of hand-authored annotations.
- **Error protocol**: the Deep-FFI `#error(...)` variants (`errno` / `nonzero` / `null` / `negative` / `success: N` / `none`), producing `Result<T, FfiError>` exactly as hand-bound Deep FFI does.
- **Capabilities**: library-level + per-symbol capability claims feeding §5's labeling tier.
- A symbol with NO sidecar entry imports maximally conservatively: opaque `CPtr` params/returns, `borrowed` default, `uses FFI("<lib>"), UnknownEffects`. Conservative-by-default mirrors the js-interop §3.4 posture.

#### §4.5. Output — generated Deep-FFI bindings

- The translator's output for `use c` IS a set of Deep-FFI extern bindings (compiler-internal; `--emit-bindings` renders them as `.ori`). Semantics are DEFINED by equivalence: `use c` + sidecar produces exactly what a correct hand-authored `extern "c"` block with the same annotations produces.
- One semantic authority: the Deep-FFI proposals remain the SSOT for boundary behavior; this proposal adds zero new runtime semantics for C.

### §5. Enforcement-Strength Taxonomy — Mediation vs Labeling

Every registered instance declares its class; the class binds the contract set (§3.3); documentation and diagnostics state the class plainly:

| Class | Meaning | Instances |
|---|---|---|
| MEDIATED | Ori owns the foreign runtime; denied capabilities are enforced (compile-time primary, runtime engine gate as defense) | `use js` |
| LABELED | Foreign code executes native syscalls Ori cannot intercept; capability claims are sidecar-audited declarations; the honest gate is `uses FFI("<lib>")` + sandbox `denied: [FFI]` wholesale | `use c` (and every native-ABI instance) |

- A LABELED import NEVER presents itself as sandboxed: `without Net` on a function calling a mislabeled C library is a documentation-strength guarantee, not a mediation guarantee. Diagnostics for capability denials on LABELED imports carry the class in the message.
- This is the same honesty split the js-interop draft's §9.3 established for N-API addons; the umbrella promotes it to the contract-binding per-instance property.

### §6. Export Direction — Ori as the Imported Ecosystem

Interop is bidirectional. The import legs above make foreign ecosystems consumable from Ori; the export legs make Ori consumable from foreign hosts. v1 ships NO export instance; this section names the doctrine slots so export proposals register into the architecture instead of re-deriving it.

#### §6.1. Ori as a C-ABI library

- `pub extern` exported functions: Ori AOT emits a static/shared library plus exported C-ABI symbols; `#repr("c")` boundary types; the Deep-FFI ownership vocabulary (`owned`/`borrowed`/`out`) mirrored to the export direction so a foreign caller knows who frees what.
- Runtime-ownership hazards are first-class design surface, not afterthoughts: an embedded Ori runtime rides in a host process it does not own — signal-handler installation, thread creation, and TLS interaction must be embedder-configurable (Go's c-shared mode hit exactly this class: go#13042, SIGCHLD interception crashing host processes; go#71099 records sustained demand for the library-embedding shape).
- Prior art: Go `-buildmode=c-shared`/`c-archive` (+ generated header), Rust `crate-type = ["cdylib"]` + cbindgen, Kotlin/Native frameworks, Haskell `foreign export`.
- Companion-proposal slot: "Ori as a C-ABI library" — the highest-leverage export leg; it is also the substrate for embedding Ori components inside existing native hosts.

#### §6.2. Declaration emission

- The inverse of the corpus leg: emit `.d.ts` (or `.h`) from an Ori module's public API so foreign consumers get typed access — the js-interop draft's full-stack type-sharing motivation, inverted (its Non-Goal 5).
- Pure compile-time emission; no engine involvement; reuses the translation machinery's type-mapping tables in reverse.

#### §6.3. Callback trampolines — the mini-export every MEDIATED instance already needs

- Every MEDIATED instance registers host functions and passes Ori closures into the foreign runtime (js-interop §2.4 callbacks; §9.2 builtin registration). That machinery IS an export surface: foreign code calling into Ori through generated thunks with capability subset-checking at registration.
- The export legs generalize machinery that must exist anyway; an export proposal extends the trampoline substrate rather than inventing a parallel one.

#### §6.4. Ordering

- §6.1 (C-ABI library emission) is the prerequisite leg: §6.2 emits declarations FOR it, and §6.3's trampolines share its calling-convention substrate. An export instance proposal that starts anywhere else must justify the inversion.

---

## Drawbacks

- libclang becomes a compiler-build dependency for the `use c` path. Mitigated: the LLVM toolchain is already vendored; still, it widens the build surface and version-couples header parsing to the shipped LLVM.
- Headers underspecify by design; the sidecar carries real authoring burden for rich libraries. Mitigated by conservative defaults (everything works opaque without a sidecar) — but "works well" requires sidecar investment, and a wrong sidecar is a soundness lie at the boundary (same trust class as a wrong hand-written extern annotation today).
- The preprocessor subset (Non-Goal 4) will disappoint on macro-heavy libraries; the escape hatch is explicit but is still a cliff.
- Doctrine gravity: once `use js` and `use c` exist, every ecosystem invites an instance. The per-instance-proposal gate (Non-Goal 2) is the containment; the §3 extension architecture is what keeps each granted instance from being a compiler rewrite.
- Contract-interface rigidity, now on TWO cores: the `LanguageBackend` contract (§3.2) and the runtime shared core (§3.5) become API the moment a second backend conforms; evolving either after N backends exist costs N migrations. Mitigated: backend version participates in the cache key (§3.4), the serialized-declaration format family is versioned with the translator, and §3.5 contracts are trait-shaped (per-instance assoc types) rather than concrete-type-shaped.
- The runtime shared core is generalized from a corpus of ONE (the js instance). Contracts generalized from one instance risk js-shaped assumptions; the mitigation is the checklist discipline — the second MEDIATED instance's registering proposal is REQUIRED to flag every §3.5 row it cannot instantiate as stated, and that review amends the umbrella before the instance lands.

---

## Alternatives Considered

### Alternative 1: Per-language ad-hoc designs, no shared core

Rejected. The js-interop draft demonstrates the full six-system anatomy of a real ecosystem import; a second free-standing design re-derives grammar, sidecar, cache, capability tiers, AND the five runtime systems (engine driving, scheduler integration, packaging, sandboxing, compat-layer grounding) with drift guaranteed. The umbrella makes language N+1 a backend registration with a class-bound checklist.

### Alternative 2: Status quo — hand-bound `extern` only

Rejected for the same reason the js-interop draft rejected manual binding (its Alt 2): per-symbol transcription cost, boundary visible everywhere, the authoritative declaration corpus sitting unread beside the hand-copy.

### Alternative 3: Bespoke C parser + preprocessor (the Zig path)

Rejected. Zig's `translate-c` demonstrates both feasibility and cost: a whole C frontend to maintain, still a semantic subset. libclang rides Ori's already-vendored LLVM; the maintenance asymmetry decides it.

### Alternative 4: Offline bindgen tool (rust-bindgen shape)

Rejected as the USER surface. Generated-file workflows drift from their inputs, keep the boundary visible, and integrate with neither capabilities nor AIMS. Internally the translator IS a bindgen with a content-addressed cache — the difference is invisibility and seam integration, not mechanism.

### Alternative 5: Fold `use c` into the js-interop proposal

Rejected. The instances share the surface and compile-time core but nothing at the runtime layer (embedded engine vs direct ABI); the js-interop draft is already large; and the umbrella needs a home that outlives any one instance.

### Alternative 6: Keep the umbrella declaration-scoped (a header-import feature)

The prior form of this draft. Rejected — the frame was generalized from the thin (LABELED) instance:

- Its flagship instance (`use js`) spends its largest design sections on machinery a declaration-scoped umbrella has no vocabulary for (engine driving, scheduler integration, payload packaging, sandbox authority, compat-layer grounding); an umbrella that cannot describe its own first instance forces every MEDIATED ecosystem to re-derive those systems ad hoc.
- A declaration-scoped corpus definition cannot represent witnessed (tier-0) capability grounding, which is computed from runtime module graphs, not declarations.
- Its cost table priced MEDIATED backends as "usually zero compiler changes" plus a translator, misrepresenting the named §3.5 work as free.
- It had no packaging doctrine part and no export-direction slot, leaving both homeless.

### Alternative 7: Embed each ecosystem's own runtime for MEDIATED instances (libnode-shape, generalized)

Rejected at the umbrella level (the js draft's Alt 9, promoted): an ecosystem's own runtime performs its own syscalls, bypassing capability mediation entirely and leaving no seam for per-builtin effect declarations. The §3.5 compat-layer pattern (reimplement the runtime API surface on Ori's capability-gated primitives) is the only shape under which the MEDIATED class's enforcement claim is true.

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY.
**If not, why:** grammar (parser), `TypeRegistry` injection, capability propagation, binding emission, and any per-instance lattice dimension are compiler-resident; corpus readers bind native components (libclang) `oric`-side, IO-confined like the js-interop const-eval bridge.
**Missing features that would enable purity:** none realistic for the libclang leg; a pure-Ori C parser is Alternative 3 (rejected).
**Recommendation:** Hybrid — compiler feature for grammar/translation/registry; sidecar PARSING as pure Ori stdlib (`library/std/ffi/sidecar_parser.ori`, mirroring `dts_parser.ori`); translators pure Ori under const-eval; §3.5 runtime-core instantiations live in stdlib + vendored native shims (catalogued like `ori_rt`), never in core compiler crates.

---

## Spec & Grammar Impact

| Spec target | Change |
|---|---|
| `grammar.ebnf` (Annex A) | `js_import` generalizes to `foreign_import` + registered-tag table; disambiguation note unchanged |
| Clause 18 (Modules) | Sub-clause: ecosystem imports — tag registry, resolution per tag, corpus definition, sidecar resolution + precedence |
| Clause 20 (Capabilities) | Enforcement-strength taxonomy (MEDIATED / LABELED) as the contract-binding per-instance property; capability-tier model |
| Clause 21 (Memory Model) | The per-instance lattice-dimension pattern (§3.5 handle-lifetime row): when an embedded runtime owns a collector, the instance adds one orthogonal dimension + side-table; instance-scoped naming |
| Clause 26 (FFI) | `use c` defined by equivalence to Deep-FFI extern bindings; `extern` documented as substrate + escape hatch; export-direction cross-reference (§6) |
| Annex E (System Considerations) | NOTE on MEDIATED packaging: engine static-linking, payload embedding modes, pay-for-what-you-use linkage, lazy startup |
| Diagnostic codes | E1520–E1539 (foreign-import resolution/parse: E1520 unregistered tag, E1530 function-like macro, ...) |

The js-interop draft amends on this proposal's acceptance: its `js_import` grammar section re-anchors as the first `foreign_import` instance, and its §5/§5.1/§6/§7/§8/§9 re-anchor as the first instantiation of the §3.5 runtime shared core (mechanical re-anchoring; semantics unchanged).

---

## Prior Art

| System | What it does | Verified | What this proposal differs on |
|---|---|---|---|
| Zig `@cImport` / `translate-c` | Direct C-header import in-language; bespoke C frontend (`zig/lib/compiler/translate-c`) | Local source + zig#1596; zig#20630 (open, +91) moves `@cImport` into the build system — include paths/defines are build config | Ori: libclang not bespoke; build config in manifest from day one; sidecar adds ownership/error/capability semantics Zig imports do not carry |
| Swift ClangImporter | Seamless C/Obj-C import via embedded Clang (`swift/lib/ClangImporter`); C++ interop partial after years | Local source | Same import ergonomics; Ori adds effect labeling + Deep-FFI ownership at the seam; Swift's C++ timeline is the Non-Goal 1 evidence |
| D ImportC | C sources imported via bundled C11 compiler | docs | Same direction; no sidecar semantics |
| Kotlin/Native cinterop | Headers + `.def` sidecar file → Kotlin bindings | docs | The closest sidecar precedent; `.def` is per-library config, not ownership/effect semantics |
| Java Panama `jextract` | Headers → Java FFM bindings via libclang | docs | Offline tool, not a language surface; no effects/ownership |
| rust-bindgen | Headers → Rust bindings, offline codegen | docs | Alternative 4 shape: visible generated files, no seam integration |
| CsWin32 / Win32Metadata | Formalized API metadata sidecar driving codegen | docs | The strongest "sidecar as first-class artifact" precedent, single-vendor scope |
| GraalVM polyglot | Multi-language runtime embedding with `HostAccess` policy | docs | The MEDIATED-class precedent; runtime-checked policy vs Ori's compile-time effect propagation + scheduler-owned driving; VM tax vs pay-for-what-you-use linkage |
| Go (rejected import leg) | go#77386 `x/tools/ffi` binding-generation proposal closed not-planned; cgo remains the manual path | Intel graph, verified issue | Evidence the ecosystem gap is real and that incumbent languages decline the surface |
| Go c-shared / c-archive (export leg) | `-buildmode=c-shared`/`c-archive` emits a C-consumable library + header from Go code | Intel graph: go#71099 (demand for c-shared libraries consumable by independent Go apps), go#13042 (embedded-runtime signal-handler hazard — SIGCHLD interception crashing hosts) | The §6.1 precedent set: both the demand signal and the runtime-ownership hazard class an Ori export instance must design for |
| Rust cdylib + cbindgen; Kotlin/Native frameworks; Haskell `foreign export` | Language-as-C-ABI-library emission with generated headers | docs | Same §6.1 precedent family; none carries capability/ownership metadata in the emitted interface |
| `js-interop-typescript-bindings-proposal.md` | The first MEDIATED instance: `.d.ts` + sidecar + embedded engine + scheduler-driven job queue + `JsRef` + payload embedding + compat layer | In-tree draft | This umbrella generalizes its runtime machinery into the §3.5 contracts; that draft retains all JS-specific design as the first instantiation |

**Novelty claim:** header-driven C import is well-precedented (Zig, Swift, D); sidecar-configured binding generation is precedented (Kotlin/Native, Win32Metadata); language-as-C-library export is precedented (Go, Rust, Kotlin/Native). Unprecedented is the UNIFORM multi-ecosystem architecture where each instance carries compile-time capability semantics (mediated or labeled, honestly classed), ownership integrated into one compile-time calculus, a class-bound runtime contract set (scheduler-owned driving, witnessed capability grounding, payload packaging), and a bidirectional doctrine — plus the doctrine that hand-written FFI is the generated substrate, not the user surface.

---

## Unresolved Questions

1. Build-manifest shape for include paths / defines / sysroot per target (interacts with the toolchain/config proposal surface; resolves at review).
2. Sidecar distribution: vendored-in-repo only, or a shared community registry for popular C libraries (v2 question; vendored-only for v1).
3. Third-party backend distribution: the §3.2 contract makes out-of-tree backends structurally possible (pure-Ori translators are const-eval-sandboxed — no IO, resource-bounded — so the trust surface is narrow for LABELED; MEDIATED backends' native shims need a trust policy); tag namespacing (two packages claiming `py`), packaging, and that trust policy decide WHEN (v2; in-tree-only for v1).
4. `--emit-bindings` output stability: inspectable `.ori` renders — stable API or debug aid (leaning debug aid; resolves during implementation).
5. Trigger condition for the `use cpp` subset-feasibility study (out of scope; recorded so Non-Goal 1 has a revisit anchor).
6. Whether `use c` items support `* as NS` namespace form over a header's full export set (parser accepts; translator support resolves during implementation).
7. Runtime-shared-core contract versioning: how a §3.5 contract evolves once two MEDIATED instances conform (amendment-proposal gate vs versioned trait family; resolves when the second MEDIATED instance registers).
8. Export-leg proposal ordering is fixed by §6.4 (C-ABI library emission first); the trigger condition for authoring it (embedder demand vs compiler-internal seam demand) resolves at review.
