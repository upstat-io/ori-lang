# Proposal: Declaration-Driven Ecosystem Imports (`use <lang>`)

**Status:** Draft (research)
**Author:** Eric (with AI assistance)
**Created:** 2026-07-15
**Affects:** grammar, type system, capability system, FFI, stdlib, spec
**Depends On:** `js-interop-typescript-bindings-proposal.md` (draft — first instance), `deep-ffi-proposal.md` (approved), `ffi-boundary-safety-proposal.md` (approved), `platform-ffi-proposal.md` (approved)
**Related:** `capability-propagation-completion-proposal.md`, `negative-effect-without-proposal.md` (drafts; inherited transitively via the js-interop instance)

---

## Summary

- One import surface for every foreign ecosystem: `use <lang> "<source>" { ... }`. The compiler reads the ecosystem's OWN authoritative machine-readable declarations; a sidecar supplies what the declarations lack; the compiler generates the boundary; types, capabilities, and AIMS ownership are checked at the seam.
- This proposal is the UMBRELLA: the shared grammar generalization, the shared core (sidecar family, translation cache, binding generation, capability tiers), the per-language backend contract, and the enforcement-strength taxonomy.
- The architecture is EXTENSION-BASED (§3): a language is a registered backend against a FIXED compiler surface — translators are pure Ori executed under const-eval (resource-bounded, no IO), binding specs are data the generic consumer already understands. Adding a LABELED ABI language with an existing corpus reader requires ZERO compiler changes.
- `use js` (per `js-interop-typescript-bindings-proposal.md`) is the first registered instance. This proposal adds the second: **`use c`** — C headers parsed via libclang, sidecar-supplied ownership/error/capability metadata, output materialized as generated Deep-FFI bindings.
- `extern` FFI is NOT replaced: it is demoted from user-facing surface to compilation target and escape hatch. `use c` generates what developers hand-write today.
- Doctrine: other languages have an FFI; Ori imports ecosystems.

---

## Motivation

### The Problem in Practice

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

### When This Matters

- Every C library a program touches (libsodium, sqlite3, zlib, PCRE2, platform APIs) today costs a hand-authored extern block; the stdlib's own FFI-backed modules pay it too.
- The `use js` draft already proved the pattern's four parts (declaration corpus, sidecar, generated boundary, effect/ownership propagation at the seam). Without an umbrella, each future ecosystem re-derives the machinery; with one, each new language is a backend.
- Uniform surface is a language pitch, not just plumbing: the developer learns ONE import model for JS, C, and future ecosystems.

---

## Goals and Non-Goals

**Goals:**

- The `use <lang> "<source>"` grammar generalization with a registered-language-tag table.
- The EXTENSION architecture: a fixed compiler surface + the `LanguageBackend` contract (§3.2), such that a new LABELED language with an existing corpus reader adds zero compiler code, and every backend class has a named, bounded compiler footprint (§3.3).
- The shared core every backend inherits: sidecar schema family, translation-cache keying, binding/trampoline generation, capability metadata tiers (§3.4).
- The enforcement-strength taxonomy (capability MEDIATION vs capability LABELING), stated per instance.
- `use c` as the second registered instance: libclang-parsed headers, sidecar ownership/error/capability metadata, generated Deep-FFI bindings.

**Non-Goals:**

1. **Not `use cpp`.** C++ (templates, overloads, mangling, no stable ABI) is a separate research program; Swift's multi-year partial C++ interop is the scale evidence. A subset-feasibility study is a named future proposal, never an implicit extension of `use c`.
2. **Not `use py` / `use java` / further instances.** Each future instance (`.pyi` stubs + embedded CPython; `.class` metadata + embedded JVM) is its own proposal registering into this umbrella's backend contract.
3. **Not a replacement for `extern`.** The hand-bound surface (per `platform-ffi-proposal.md` + `deep-ffi-proposal.md`) remains the substrate `use c` compiles TO and the escape hatch for anything the translator cannot express.
4. **Not full C preprocessor support.** Object-like constant macros translate; function-like macros do NOT (escape hatch: a wrapper header or hand-bound extern). Subset boundaries are explicit, never silent.
5. **Not capability mediation for native code.** Native libraries execute their own syscalls; `use c` delivers capability LABELING (audited claims), never enforcement (per §5).

---

## Design

### §1. The Doctrine — Four Parts, Per-Language Answers

Every instance of `use <lang>` supplies the same four parts:

| Part | `use js` (first instance) | `use c` (this proposal) |
|---|---|---|
| Declaration corpus | `.d.ts` TypeScript declarations | `.h` C headers |
| Sidecar (what declarations lack) | capabilities per export/callback | ownership (`owned`/`borrowed`/`#free`), error protocol, capabilities |
| Boundary mechanism | embedded engine + generated trampolines | direct C ABI — generated Deep-FFI bindings, no runtime |
| AIMS integration | `JsRef` 8th lattice dimension (shared GC) | EXISTING machinery — `Locality::Borrowed(p)` + Deep-FFI ownership annotations; no new dimension |

- A new instance MUST answer all four parts in its registering proposal; a partial answer (types without lifetime story, boundary without capability story) is refused at review.
- The C row needs LESS new machinery than the JS row: no second runtime, no job queue, no GC handshake, no lattice extension.

### §2. Grammar — Generalizing `js_import` to `foreign_import`

The `js_import` production from the js-interop draft generalizes without changing its disambiguation:

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

### §3. Extension Architecture — a Language Is a Backend, Not a Compiler Patch

The system is EXTENSION-BASED. Adding a language means registering a backend against a fixed compiler surface; the compiler is NEVER modified per language except for the two rare, shared components named below. The split follows the js-interop draft's own precedent (its `.d.ts` parser is pure Ori executed under const-eval — that mechanism IS the extension point, generalized here).

#### §3.1. The three layers, by mutability

| Layer | Owner | Changes per new language? |
|---|---|---|
| **Compiler surface** (fixed) | `foreign_import` grammar + tag registry, const-eval translation bridge, `TypeRegistry` injection + deserializer, capability plumbing, generic Deep-FFI binding consumer, trampoline machinery | NO — generic over every backend |
| **Corpus readers** (small shared set) | `oric` (impure driver): raw-text reader (`.d.ts`, `.pyi`), libclang reader (C headers → serialized language-neutral C-declaration form) | RARELY — only when a new language needs a corpus format no existing reader produces (e.g. a future `.class`-metadata reader). Readers are shared infrastructure, never per-language logic |
| **Backend** (the extension) | Translator + sidecar schema + (mediated class only) runtime binding library | YES — this is ALL a new language adds |

#### §3.2. The backend contract

A backend registers:

```ori
trait LanguageBackend {
    @tag () -> str;                        // "js", "c" — claims the `use <tag>` surface
    @corpus_reader () -> CorpusReaderId;   // which shared reader feeds @translate
    @enforcement () -> EnforcementClass;   // Mediated | Labeled (§5)
    // Pure, const-evaluable: serialized corpus + sidecar + build config in,
    // serialized declarations + capability metadata + binding specs out
    // (the dts_serial.ori format family; deserialized + validated by the fixed compiler surface)
    @translate (corpus: str, sidecar: Option<str>, config: str) -> str;
}
```

- **Translators are pure Ori, executed at build time under const-eval** — resource-limited (the existing const-function limits), NO IO, NO capabilities. The compiler hands them strings and validates what comes back. A translator cannot read the filesystem, reach the network, or exceed its step budget — the build-time supply-chain surface of a backend is bounded by construction.
- **Binding specs are data.** For LABELED backends the emitted specs are Deep-FFI extern declarations the fixed surface already knows how to consume; for MEDIATED backends, boundary-trampoline specs. Either way the compiler's consumer is generic.
- **Runtime binding (MEDIATED class only)**: an engine embedding written as library Ori over Deep FFI (plus any vendored native shim, catalogued like `ori_rt`). Library-space, not compiler-space.
- **AIMS lattice extension (rare)**: required ONLY when a mediated backend embeds a collector-owning runtime (the `JsRef` case, per §6). The only per-language compiler change that can exist, and most backends never need it.

#### §3.3. Cost of adding a language, by class

| New language | Compiler changes | Extension work |
|---|---|---|
| LABELED, existing corpus reader (another header-driven ABI language) | ZERO | translator (pure Ori) + sidecar schema |
| LABELED, new corpus format | one shared corpus reader | translator + sidecar schema |
| MEDIATED (embedded runtime) | usually zero; lattice dimension only if the runtime owns a collector | translator + sidecar schema + engine binding library (+ vendored shim) |

#### §3.4. Shared services every backend inherits

| Component | Contract |
|---|---|
| Sidecar family | Per-source metadata resolved beside the corpus: `<pkg>.ori-caps.json` (JS), `<lib>.ori-ffi.json` (C). One resolution rule, one precedence model (author sidecar > package metadata > heuristic list > conservative default); field vocabulary declared by the backend's sidecar schema |
| Translation cache | `target/<lang>-bindings/<sha256>.ori-types` keyed on: corpus hash + sidecar hash + relevant build configuration + backend version + compiler version (the js-interop §2.5 key composition, generalized). Backend version in the key makes backend upgrades self-invalidating |
| Binding materialization | Bindings are compiler-internal; `--emit-bindings` renders them as readable `.ori` for inspection |
| Capability tiers | The js-interop §3.1 precedence table, per-language: tier-0 ground truth where the runtime is owned, sidecar/metadata/heuristic/conservative tiers otherwise |
| Diagnostics | The E1520-range foreign-import diagnostics, parameterized by tag; E1520 (unregistered tag) lists the installed backend tags |

- v1 ships two in-tree backends (`js`, `c`) registered through this contract — the contract is proven by having two conforming instances from day one, not by speculation.
- Third-party backend distribution (out-of-tree tags) is deliberately deferred (§Unresolved Questions): the interface makes it possible; trust, tag namespacing, and packaging decide WHEN.

### §4. The `use c` Instance

#### §4.1. Resolution and build configuration

| Source form | Resolution |
|---|---|
| `use c "sqlite3.h" { ... }` | Header on the configured include path |
| `use c "./vendor/foo.h" { ... }` | Workspace-relative header |
| `use c "<system>/socket.h" { ... }` | System include (angle-bracket semantics) |

- Include paths, `-D` defines, and target sysroot are BUILD CONFIGURATION, never source-file content — declared in the build manifest, not in the `use c` statement. (Zig accepted the same conclusion for `@cImport` — zig#20630 moves it into their build system; Ori starts there instead of migrating later.)
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
| `struct` (complete definition) | Ori record with `#repr("c")` | Field-by-field translation; bitfields → `JsAny`-analog opaque fallback (`COpaque`) |
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
- A symbol with NO sidecar entry imports maximally conservatively: opaque `CPtr` params/returns, `borrowed` default, `Result<T, FfiError>` via `#error(none)` absent, `uses FFI("<lib>"), UnknownEffects`. Conservative-by-default mirrors the js-interop §3.4 posture.

#### §4.5. Output — generated Deep-FFI bindings

- The translator's output for `use c` IS a set of Deep-FFI extern bindings (compiler-internal; `--emit-bindings` renders them as `.ori`). Semantics are DEFINED by equivalence: `use c` + sidecar produces exactly what a correct hand-authored `extern "c"` block with the same annotations produces.
- One semantic authority: the Deep-FFI proposals remain the SSOT for boundary behavior; this proposal adds zero new runtime semantics for C.

### §5. Enforcement-Strength Taxonomy — Mediation vs Labeling

Every registered instance declares its class; documentation and diagnostics state it plainly:

| Class | Meaning | Instances |
|---|---|---|
| MEDIATED | Ori owns the foreign runtime; denied capabilities are enforced (compile-time primary, runtime engine gate as defense) | `use js` |
| LABELED | Foreign code executes native syscalls Ori cannot intercept; capability claims are sidecar-audited declarations; the honest gate is `uses FFI("<lib>")` + sandbox `denied: [FFI]` wholesale | `use c` (and every native-ABI instance) |

- A LABELED import NEVER presents itself as sandboxed: `without Net` on a function calling a mislabeled C library is a documentation-strength guarantee, not a mediation guarantee. Diagnostics for capability denials on LABELED imports carry the class in the message.
- This is the same honesty split the js-interop draft's §9.3 established for N-API addons; the umbrella promotes it to a named, per-instance property.

### §6. AIMS Integration — Reuse First, Extend Only for Owned Runtimes

- `use c` adds NO lattice dimension: C has no collector to root against; lifetimes are contractual and the Deep-FFI ownership annotations + `Locality::Borrowed(p)` + existing verification already model them.
- A lattice extension is justified ONLY when the instance embeds a runtime with its own collector (the js-interop `JsRef` case). Naming guidance for that machinery: keep the handle side-table and obligation types instance-scoped (`JsHandleObligation`) but design field/table shapes so a future sibling (e.g. a CPython instance) is a parallel dimension, not a refactor — do not foreclose, do not generalize prematurely.

---

## Drawbacks

- libclang becomes a compiler-build dependency for the `use c` path. Mitigated: the LLVM toolchain is already vendored; still, it widens the build surface and version-couples header parsing to the shipped LLVM.
- Headers underspecify by design; the sidecar carries real authoring burden for rich libraries. Mitigated by conservative defaults (everything works untyped-ish and opaque without a sidecar) — but "works well" requires sidecar investment, and a wrong sidecar is a soundness lie at the boundary (same trust class as a wrong hand-written extern annotation today).
- The preprocessor subset (Non-Goal 4) will disappoint on macro-heavy libraries; the escape hatch is explicit but is still a cliff.
- Doctrine gravity: once `use js` and `use c` exist, every ecosystem invites an instance. The per-instance-proposal gate (Non-Goal 2) is the containment; the §3 extension architecture is what keeps each granted instance from being a compiler rewrite.
- Extension-interface rigidity: the `LanguageBackend` contract (§3.2) becomes API the moment a second backend conforms; evolving it after N backends exist costs N migrations. Mitigated: backend version participates in the cache key (§3.4), and the serialized-declaration format family is versioned with the translator (js-interop const-eval bridge precedent).

---

## Alternatives Considered

### Alternative 1: Per-language ad-hoc designs, no shared core

Rejected. The js-interop draft already contains the four-part shape; a second free-standing design re-derives grammar, sidecar, cache, and capability tiers with drift guaranteed. The umbrella makes language N+1 a backend registration.

### Alternative 2: Status quo — hand-bound `extern` only

Rejected for the same reason the js-interop draft rejected manual binding (its Alt 2): per-symbol transcription cost, boundary visible everywhere, the authoritative declaration corpus sitting unread beside the hand-copy.

### Alternative 3: Bespoke C parser + preprocessor (the Zig path)

Rejected. Zig's `translate-c` demonstrates both feasibility and cost: a whole C frontend to maintain, still a semantic subset. libclang rides Ori's already-vendored LLVM; the maintenance asymmetry decides it.

### Alternative 4: Offline bindgen tool (rust-bindgen shape)

Rejected as the USER surface. Generated-file workflows drift from their inputs, keep the boundary visible, and integrate with neither capabilities nor AIMS. Internally the translator IS a bindgen with a content-addressed cache — the difference is invisibility and seam integration, not mechanism.

### Alternative 5: Fold `use c` into the js-interop proposal

Rejected. The instances share the surface and core but nothing at the runtime layer (embedded engine vs direct ABI); the js-interop draft is already large; and the umbrella needs a home that outlives any one instance.

---

## Purity Analysis

**Can be pure Ori?** PARTIALLY.
**If not, why:** grammar (parser), TypeRegistry injection, capability propagation, and binding emission are compiler-resident; header parsing binds libclang (native, `oric`-side, IO-confined like the js-interop const-eval bridge).
**Missing features that would enable purity:** none realistic for the libclang leg; a pure-Ori C parser is Alternative 3 (rejected).
**Recommendation:** Hybrid — compiler feature for grammar/translation/registry; sidecar PARSING as pure Ori stdlib (`library/std/ffi/sidecar_parser.ori`, mirroring `dts_parser.ori`); generated bindings target the existing Deep-FFI surface.

---

## Spec & Grammar Impact

| Spec target | Change |
|---|---|
| `grammar.ebnf` (Annex A) | `js_import` generalizes to `foreign_import` + registered-tag table; disambiguation note unchanged |
| Clause 18 (Modules) | Sub-clause: declaration-driven imports — tag registry, resolution per tag, sidecar resolution + precedence |
| Clause 26 (FFI) | `use c` defined by equivalence to Deep-FFI extern bindings; `extern` documented as substrate + escape hatch |
| Clause 20 (Capabilities) | Enforcement-strength taxonomy (MEDIATED / LABELED) as a named per-instance property |
| Diagnostic codes | E1520–E1539 (foreign-import resolution/parse: E1520 unregistered tag, E1530 function-like macro, ...) |

The js-interop draft amends on this proposal's acceptance: its `js_import` grammar section re-anchors as the first `foreign_import` instance (mechanical; semantics unchanged).

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
| Go (rejected) | go#77386 `x/tools/ffi` binding-generation proposal closed not-planned; cgo remains the manual path | Intel graph, verified issue | Evidence the ecosystem gap is real and that incumbent languages decline the surface |
| `js-interop-typescript-bindings-proposal.md` | The first instance: `.d.ts` + sidecar + embedded engine + `JsRef` | In-tree draft | This umbrella generalizes its pattern; that draft retains all JS-specific design |

**Novelty claim:** header-driven C import is well-precedented (Zig, Swift, D); sidecar-configured binding generation is precedented (Kotlin/Native, Win32Metadata). Unprecedented is the UNIFORM multi-ecosystem surface where each instance carries compile-time capability semantics (mediated or labeled, honestly classed) and ownership integrated into one compile-time calculus — plus the doctrine that hand-written FFI is the generated substrate, not the user surface.

---

## Unresolved Questions

1. Build-manifest shape for include paths / defines / sysroot per target (interacts with the toolchain/config proposal surface; resolves at review).
2. Sidecar distribution: vendored-in-repo only, or a shared community registry for popular C libraries (v2 question; vendored-only for v1).
2a. Third-party backend distribution: the §3.2 contract makes out-of-tree backends structurally possible (pure-Ori translators are const-eval-sandboxed — no IO, resource-bounded — so the trust surface is narrow); tag namespacing (two packages claiming `py`), packaging, and the trust policy for MEDIATED backends' native shims decide WHEN (v2; in-tree-only for v1).
3. `--emit-bindings` output stability: inspectable `.ori` renders — stable API or debug aid (leaning debug aid; resolves during implementation).
4. Trigger condition for the `use cpp` subset-feasibility study (out of scope; recorded so Non-Goal 1 has a revisit anchor).
5. Whether `use c` items support `* as NS` namespace form over a header's full export set (parser accepts; translator support resolves during implementation).
