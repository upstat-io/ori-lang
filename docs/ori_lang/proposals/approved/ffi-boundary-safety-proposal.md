# Proposal: FFI Boundary Safety — Deep FFI Part 2

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-04-20
**Approved:** 2026-04-20
**Affects:** Language core (parser, type checker), compiler (AIMS, codegen, diagnostics), toolchain, standard library (`std.ffi`)
**Depends On:** `platform-ffi-proposal.md` (approved), `deep-ffi-proposal.md` (approved), `capability-unification-generics-proposal.md` (approved), `stateful-mock-testing-proposal.md` (approved), `capability-propagation-completion-proposal.md` (draft — required before §5 implements)
**Extends:** `deep-ffi-proposal.md` — completes deferred items; adds new items that emerged post-approval
**Amends:** `deep-ffi-proposal.md` §"Why not auto-import C headers (like Zig's @cImport)?" — see §Alternatives Considered below
**Supersedes:** `c-header-import-proposal.md` (draft) — the `#header(...)` design in §2 strictly dominates `@cImport(...)` by preserving boundary visibility AND eliminating signature boilerplate

> **Backend-neutrality clarification (2026-07-15):** RC-operation wording below
> describes the current compiled-counter adapter. AIMS itself freezes logical
> ownership, lifetime, transfer, cleanup, effect, unwind, and provenance facts
> across FFI. VM, LLVM/native, compiled-WebAssembly, and JIT projections choose
> and validate their own physical mechanisms against those same facts.

---

## Summary

Ori's approved Deep FFI proposal established declarative marshalling, ownership annotations, error protocols, `#free`, parametric FFI capabilities, and handler-based mocking. That work makes the surface of FFI calls safe and testable. This proposal extends the FFI story across three remaining axes: **(a)** the AIMS lattice analysis should propagate ownership, consumption, and uniqueness facts across the FFI boundary rather than stopping at it; **(b)** the compiler should verify C header compatibility at build time and auto-derive signatures from headers via an explicit-list `#header(...)` block attribute; **(c)** callbacks, borrow lifetimes, and diagnostic quality at the boundary should receive the same rigor as native Ori code. Together these make Ori the first production language whose memory-safety invariants survive the FFI boundary intact — fulfilling the mission claim of "memory safety across the entire program surface (including FFI)."

---

## Motivation

### Safety That Stops at the Boundary Is Not Safety

Ori's mission states that memory safety extends across the entire program surface, including FFI (`missions.md §Ori`, §AIMS). The approved Deep FFI proposal is a major step toward that claim — ownership annotations, `#free`, and automatic Drop implementations eliminate most FFI-induced leaks. But several gaps remain where the safety story stops at the boundary rather than crossing it:

1. **AIMS does not see the boundary.** The AIMS lattice analysis (`aims-rules.md §§1–7`) computes per-SSA ownership, consumption, cardinality, uniqueness, locality, shape, and effect dimensions. When an FFI call consumes an `owned CPtr`, AIMS has no fact that says "this ownership transferred out of Ori-managed memory" — it treats the value as having an unknown fate. This produces conservative logical owner-credit and release events; the current counter adapter consequently retains physical RC operations that an exact FFI contract would avoid.

2. **Borrow relationships at the boundary are not expressible.** An `owned CPtr` is clearly tracked; a `borrowed` str is copied immediately. But there is no way to express "this returned pointer borrows from THIS specific parameter and must not outlive it." Consider `sqlite3_column_text(stmt) -> const unsigned char*` — the returned pointer is valid only until the next call on `stmt`. Today, Ori either copies immediately (safe but slow) or treats the pointer as untracked (fast but dangerous). Neither is correct.

3. **Callback capabilities vanish at the boundary.** A C function that takes a callback (`qsort`, `libuv` event loops, GPU debug callbacks) receives a raw function pointer. If the Ori closure being registered uses capabilities (`uses Logger`, `uses Clock`), those capabilities are not propagated through the registration site. When the C runtime later invokes the callback, the capabilities are silently dropped — and any `uses`-requiring operation fails at runtime, not compile time.

4. **ABI drift is invisible until it crashes.** When a C library is upgraded and a struct layout changes, the Ori binding's `#repr("c")` struct becomes silently wrong. Today the compiler has no mechanism to detect this. The binding compiles successfully against headers from any library version; memory corruption surfaces at runtime, often in unrelated code.

5. **Diagnostic quality at the boundary is uneven.** When an FFI contract fails at compile time (mismatched repr, wrong error protocol, missing free function), the error message points at the Ori site but not at the C declaration that motivated the binding. The debugging loop requires switching between Ori errors and C headers manually. In aggregate this makes FFI errors feel like "language errors" rather than "boundary errors."

6. **Hand-writing signatures is boilerplate that risks drift.** Every extern declaration that redeclares a C function's signature duplicates information that lives authoritatively in the C header. Wrong transcription is a bug source. Out-of-date transcription after library upgrades is another. The approved Deep FFI proposal rejected Zig's `@cImport` (correctly — it hides the boundary) and deferred a separate `ori bindgen` tool (useful but orthogonal). A third option — header-assisted declaration with an explicit function list — preserves the boundary visibility principle while eliminating the boilerplate.

7. **Handler-mocking semantics are under-specified.** The approved proposal's §4 (Capability-Gated Testability) describes `with FFI("lib") = handler { ... } in { ... }` dispatch but leaves practical details open: how do stateful mocks interact with FFI that itself maintains state? What about partial mocks that intercept some functions but fall through to the real library for others? Can mocks record-and-replay real FFI traffic for regression testing?

### The Strategic Frame

Production languages fall into a 2×2 on (safety-across-boundary) × (interop-first-class). The top-left quadrant — safe AND interop-first-class — is vacant: Rust has safety but FFI is an `unsafe` escape hatch; Swift has Obj-C interop but C loses guarantees; Zig has interop but no safety to extend; Go/Java/Python all treat interop as an escape hatch. Closing the items above moves Ori into that vacant quadrant with a defensible architectural basis (capabilities + ownership annotations + AIMS lattice, all designed to extend through the boundary).

---

## Design

This proposal adds seven additions to the approved Deep FFI surface. Each is independently useful and opt-in; none breaks existing FFI code.

### 1. AIMS Lattice Extension Across the FFI Boundary

**Problem.** AIMS (`aims-rules.md`) computes seven lattice dimensions per SSA value. When an Ori function calls an extern function, the AIMS analysis today treats the boundary as opaque: the call's effect on ownership, consumption, and uniqueness is conservative (assume-the-worst). This produces extra logical ownership events on arguments; a counter-selecting adapter realizes them as retained RC operations that the declared ownership contract would otherwise avoid.

**Solution.** Each extern declaration with ownership annotations produces a `MemoryContract` fragment that AIMS consumes like any other `MemoryContract` (see `aims-rules.md §5 — Interprocedural Contracts`). Specifically:

- `owned` parameters generate `ParamContract::Consumed` — AIMS treats the argument as consumed at the call site; subsequent uses require a prior clone.
- `borrowed` parameters generate `ParamContract::Borrowed` — AIMS freezes no transferred owner credit or matching release for the call; a counter-selecting adapter therefore needs no RC pair, and reuse remains subject to the shared facts.
- `owned` returns generate `ReturnContract::Owned { release:
  ForeignReleaseId }`. AIMS treats the return as a unique logical ownership
  obligation and carries the typed foreign-release identity into the executable
  artifact. The selected FFI adapter resolves that identity to a concrete
  symbol, ABI, helper call, or inline sequence; `free_fn` is not AIMS
  vocabulary.
- `borrowed` returns (the default for `str`/`[byte]`) carry an explicit source
  lifetime and independent-result contract. The selected marshal adapter may
  copy into Ori-managed storage, but AIMS sees only the resulting logical
  freshness/ownership/lifetime facts, not a required heap representation.
- `#error(...)` protocols produce `EffectSummary` entries that mark the FFI call as potentially-returning (error) and potentially-terminating (success) — AIMS schedules cleanup operations accordingly.

**Effect.** An `owned CPtr` returned from C that is consumed by another FFI call
(the common pattern: `rsa = RSA_new(); sign(rsa, ...); RSA_free(rsa);`) needs no
Ori internal count events: AIMS sees one unique foreign ownership obligation
flowing through the chain, followed by its exact typed release. Today the same
pattern forces conservative RC retention.

**Verification.** The FIP certification pass (`aims-rules.md §VF-6`) extends to
verify that functions whose body is entirely FFI calls with `owned`/`borrowed`
contracts satisfy `FipContract::Certified`: zero unmatched logical
allocation/deallocation or foreign-release obligations in realized IR.

### 2. Header-Assisted Extern Blocks — `#header("file.h")`

**Problem.** Hand-transcribing C signatures is boilerplate and a drift risk. Zig's `@cImport` solves this by importing entire headers as namespaces, which the approved Deep FFI proposal correctly rejected as hiding the boundary. A separate `ori bindgen` tool was deferred.

**Solution.** A block-level `#header("path/to/header.h")` attribute instructs the compiler to read the named C header at build time, extract signatures for functions listed in the block body, and generate the Ori-visible signatures automatically. Annotations (`owned`, `borrowed`, `#error(...)`, `#free(...)`, `out`, `mut`) remain explicit in the block body; only the C type translation is automated.

```ori
extern "c" from "sqlite3" #header("sqlite3.h") #error(nonzero) #free(sqlite3_close) {
    @sqlite3_open                      // signature auto-derived from header
    @sqlite3_close (db: owned CPtr) -> c_int  // explicit signature still allowed
    @sqlite3_exec
    @sqlite3_prepare_v2                // auto-derived
    @sqlite3_errmsg #error(none)       // auto + per-function override
    @sqlite3_column_text #borrow_from(stmt)   // see §4
}
```

**Rules:**

1. Function list is required — no wildcard imports. Only functions listed in the block are importable.
2. Block declaration remains required — `extern "c" from "sqlite3"` is still the visible boundary marker.
3. Annotations and overrides remain explicit — ownership, error protocols, `#free` are never auto-inferred.
4. Explicit signatures take precedence — when a function has a hand-written signature, the compiler validates the header matches rather than deriving from it.
5. Header search path resolves in this deterministic order (first match wins):

    1. **Explicit `#header_path(...)` block attribute.** Always takes precedence. Hermetic build systems (Bazel, Nix, Buck) SHOULD use this to point at vendored headers; it makes header resolution independent of the host environment.
    2. **`pkg-config --cflags <lib>`** — invoked when `from "<lib>"` names a pkg-config-registered library. The `-I` paths from pkg-config output form the search list.
    3. **System default include paths** — compiler-default for the host platform (typically `/usr/include` and `/usr/local/include` on Unix-like systems; registry-driven on Windows). These are the same paths the host C toolchain would search.
    4. **Error E4020** if the named header cannot be resolved via any of the above.

    pkg-config is NOT required; its absence (Windows without MSYS2, custom installs, sandboxed builds) falls through to steps 1/3/4 as appropriate. The ordering guarantees that `#header_path(...)` always wins, which is what hermetic/reproducible builds need.

**Distinction from Zig's `@cImport`:** Zig imports every symbol in a header as a namespace `c.*`. Ori requires naming each imported symbol. The C boundary is visible at the block AND in the explicit function list; only the type-mapping tedium is automated.

**Distinction from `ori bindgen` tool:** Header assistance is evaluated at compile time by the Ori compiler against the current host's headers. There is no generated file to check in, no regeneration step to forget. Drift detection (§3) is automatic.

### 3. Compile-Time Struct Layout Verification

**Problem.** `#repr("c")` struct declarations in Ori can be silently wrong if the corresponding C library's layout changes after a library upgrade. Memory corruption surfaces at runtime in unrelated code.

**Solution.** When an `extern` block uses `#header(...)`, the compiler additionally verifies every `#repr("c")` type declared in the same module against the header's struct layout at compile time. Mismatches produce `E4xxx` FFI-family compile errors pinpointing:

- The Ori struct declaration line
- The C struct declaration line (file + line + column from the header)
- The specific mismatch (field count, field type, alignment, size)

```ori
#repr("c")
type sqlite3_module = {
    iVersion: c_int,
    xCreate: CPtr,
    // ... 20 more fields
}

extern "c" from "sqlite3" #header("sqlite3.h") {
    @sqlite3_create_module (...)       // uses sqlite3_module; verified against header
}
```

If `sqlite3.h` adds a field to `sqlite3_module` in a future SQLite version, recompilation fails with:

```
error[E4015]: struct layout mismatch with C header
  --> src/db/bindings.ori:12
   |
12 | type sqlite3_module = {
   | ^^^^^^^^^^^^^^^^^^^^^^ Ori declaration has 22 fields
   |
note: C header declares 23 fields
  --> /usr/include/sqlite3.h:8421
   |
8421 | struct sqlite3_module {
     | ^^^^^^^^^^^^^^^^^^^^^^
   = help: C header has additional field at index 23:
             xIntegrity: (*const fn(...) -> int)   (in header: `int (*xIntegrity)(...)`)
           Add the missing field to the Ori declaration, or remove #header(...) / #verify_layout(...) to silence.
```

Diagnostics report only what the compiler can derive from the current header: a field-by-field diff, the header line range, and the exact extra/missing field(s). They do NOT attribute changes to library versions — the compiler has no library-version-history database, and introducing one is explicitly out of scope. Upgrade attribution is a documentation task for library maintainers, not a compiler capability.

**Rules:**

1. Verification is opt-in: only active when `#header(...)` is specified on a block in the same module, OR an explicit `#verify_layout("header.h")` attribute is on the struct.
2. Ori struct field names must match C struct field names for the match to be checked by name; otherwise positional matching is attempted and a warning emitted.
3. Anonymous unions and bit-fields are reported as "complex layouts" with a pointer to the relevant clang AST node; user is responsible for verifying manually.
4. Verification runs via libclang (standard dependency of header parsing in §2); no separate tool required.

### 4. `#borrow_from(param)` — Lifetime Annotations at the Boundary

**Problem.** An `owned CPtr` has clear semantics: Ori takes responsibility. A `borrowed CPtr` default is "copy immediately" — safe but slow. There is no way today to express "this returned pointer borrows from THIS specific parameter and is valid only for the parameter's lifetime." Common in C:

- `sqlite3_column_text(stmt)` — returned pointer borrows from `stmt`; invalidated by next `sqlite3_step(stmt)`.
- `strchr(haystack, c)` — returned pointer borrows from `haystack`.
- `libxml2 xmlNodeGetContent(node)` — returned pointer borrows from `node`.

**Solution.** A per-function `#borrow_from(param_name)` attribute declares that the function's return value borrows from a named parameter. The type checker and AIMS enforce that the returned value does not outlive the parameter:

```ori
extern "c" from "sqlite3" #header("sqlite3.h") {
    @sqlite3_column_text #borrow_from(stmt)
    // effective signature:
    //   (stmt: borrowed CPtr, iCol: c_int) -> borrowed str valid_while(stmt)
}

// Compiler rejects:
let text = {
    let stmt = prepare(db, "SELECT ...");
    sqlite3_column_text(stmt: stmt, iCol: 0)
};  // ERROR: 'stmt' goes out of scope; borrowed value would outlive source
// Compiler accepts:
let stmt = prepare(db, "SELECT ...");
let text = sqlite3_column_text(stmt: stmt, iCol: 0);
use(text);  // OK — stmt still in scope
```

**Rules:**

1. `#borrow_from(p)` requires `p` to be a parameter of the same function.
2. The returned value's locality is `Locality::Borrowed(p)` in the AIMS lattice (`aims-rules.md §1.5`). This integrates with existing locality tracking; no new dimension.
3. Multiple `#borrow_from` are allowed: `#borrow_from(a, b)` means the return is valid while both `a` AND `b` are in scope — formally, the return value's lexical scope must be enclosed by the intersection of every named parameter's scope. Example — a hypothetical C join API returning a pointer into the shorter of two input buffers:

    ```ori
    extern "c" from "utf8lib" #header("utf8.h") {
        @utf8_common_prefix #borrow_from(a, b)
        // (a: borrowed str, b: borrowed str) -> borrowed str valid_while(a AND b)
    }

    let haystack = read_file(path: "big.txt");      // scope: H
    let shared = {
        let needle = compute_needle();              // scope: N
        utf8_common_prefix(a: haystack, b: needle)  // return valid while H ∩ N
    };  // ERROR: needle scope ends; return outlives `needle`
    ```

    Borrowing from a struct-field or array-element of a parameter (`#borrow_from(vec.data)`) is NOT supported in this revision; only top-level parameters may be named. That restriction may be lifted by a future proposal if a concrete use case demands it.
4. Implicit copy-to-owned is available at call sites: `sqlite3_column_text(stmt).to_owned()` extends the returned value's lifetime at the cost of a copy.
5. AIMS uses `#borrow_from` to freeze the returned value's lifetime relation without an independent owner credit; a counter-selecting adapter consequently emits no retain/release pair for that borrowed result.

### 5. Callback Capability Propagation

**Problem.** A C function that takes a callback (`qsort`, `libuv` handlers, GPU debug callbacks) receives a function pointer. If the Ori closure uses capabilities, those capabilities are silently dropped at the registration site. The C runtime later invokes the callback in a context where the capabilities are not available, and any `uses`-requiring operation fails at runtime rather than compile time.

**Solution.** Callback parameters in extern declarations can declare `uses` lists that describe which capabilities the C runtime will have available when invoking the callback:

```ori
extern "c" from "libuv" #header("uv.h") {
    @uv_timer_start (
        handle: CPtr,
        cb: (handle: CPtr) -> void uses Clock,  // callback has Clock access
        timeout: c_long,
        repeat: c_long,
    ) -> c_int
}

// At the call site, the compiler verifies the registered closure uses
// ONLY capabilities that the callback signature promises:
@on_tick (handle: CPtr) -> void uses Clock = {
    let now = Clock.now();   // OK — Clock in cb signature
    print(now);              // ERROR — Print not in cb signature
}

uv_timer_start(handle: timer, cb: on_tick, timeout: 1000, repeat: 1000);
```

**Rules:**

1. Callback parameter types declare `uses` explicitly: `(args) -> ret uses Cap1, Cap2`.
2. Missing `uses` on a callback parameter means the callback runs with NO Ori capabilities available (strictest default).
3. The compiler verifies at registration that the registered function's `uses` list is a subset of the callback parameter's `uses` list.
4. For libraries where the C runtime cannot provide a capability (e.g., signal handlers that must not allocate), the declaration uses no `uses` — the type system then forbids allocating closures from being registered.
5. Capability-providing libraries (event loops that pass an event-loop handle) propagate capabilities via handler-based capability provision at registration; this integrates with the existing `with Cap = handler in` mechanism.

**Effect.** Capability failures become compile-time errors at the registration site, not runtime errors in the C callback. This makes the FFI boundary hold the same capability-safety invariants as native Ori code.

### 6. FFI-Aware Diagnostic Quality

**Problem.** FFI errors today reference Ori sites only. Debugging requires switching between Ori compiler output and C headers manually.

**Solution.** Every FFI-family diagnostic (`E4xxx`) that originates from a `#header(...)`-enabled extern block adds a secondary span pointing at the relevant C declaration. Specifically:

- **Struct layout mismatch** (§3): Ori struct line + C header struct line + field-by-field diff.
- **Signature mismatch** (between explicit Ori signature and header): Ori declaration + C header declaration + diff.
- **Missing `#free` on `owned CPtr`**: Ori declaration + docstring excerpt from the C header if available (many C headers document ownership in comments).
- **Invalid error protocol** (`#error(errno)` on a function that doesn't set errno): Ori annotation + C header's documented return-value convention if discoverable.
- **Unresolved callback capability mismatch** (§5): Ori callback declaration + C callback-parameter type from header.

Diagnostic format follows existing `ori_diagnostic` patterns with the secondary span using `C header:` prefix to distinguish sources.

**Rules:**

1. Diagnostics degrade gracefully when `#header(...)` is not in use — single-span errors, no C references.
2. When the C header path is in a system location, diagnostics show the resolved absolute path and library origin (e.g., `/usr/include/sqlite3.h (libsqlite3-dev)`) rather than just the path.
3. Diagnostics never require libclang at runtime for error rendering — extracted C declarations are cached in build artifacts.

### 7. Handler-Mocking Semantics Formalization

**Problem.** The approved Deep FFI proposal's §4 establishes `with FFI("lib") = handler { ... } in { ... }` but leaves practical details unspecified.

**Solution.** Formalize three aspects:

#### 7a. Partial Mocking With Fall-Through

A handler that mocks only some functions of a library allows unmocked functions to call through to the real C implementation. This is already specified in the approved proposal; this proposal adds the explicit escape syntax for when fall-through is NOT wanted:

```ori
with FFI("sqlite3") = handler #strict {
    sqlite3_open: (filename: str) -> Result<CPtr, FfiError> = Ok(fake_ptr),
} in {
    // sqlite3_open is mocked
    // sqlite3_exec is NOT mocked → compile error in strict mode
    //   (default mode: falls through to real library)
    test_database_logic()
}
```

The `#strict` attribute on the handler rejects unmocked calls at compile time; helpful for tests that assert no real FFI traffic occurs.

#### 7b. Stateful Mock Integration With C State

When a C library maintains state (e.g., SQLite handle refers to an in-memory database), stateful handlers can model that state:

```ori
with FFI("sqlite3") = handler(state: MockDb.empty()) {
    sqlite3_open: (s, filename: str) -> (MockDb, Result<CPtr, FfiError>) = {
        let (db, handle) = s.open(filename: filename);
        (db, Ok(handle))
    },
    sqlite3_exec: (s, db: CPtr, sql: str) -> (MockDb, Result<void, FfiError>) = {
        let (new_state, result) = s.exec(db: db, sql: sql);
        (new_state, result)
    },
} in {
    test_database_logic()
}
```

This uses the `handler(state: S) { ... }` form from the approved stateful-mock-testing proposal; no new syntax required. This section just documents the FFI application.

#### 7c. Record-and-Replay Mode

A special handler form records real FFI traffic during a test run, producing a transcript that can replay without the library in subsequent runs:

```ori
// Recording run — requires real library present
with FFI("sqlite3") = record(to: "fixtures/sqlite_session.trace") in {
    test_database_logic()
}

// Replay run — no library needed; reads from recorded trace
with FFI("sqlite3") = replay(from: "fixtures/sqlite_session.trace") in {
    test_database_logic()
}
```

**Rules:**

1. `record` captures each FFI call's arguments and return value as serializable events.
2. `replay` deserializes the trace and returns recorded values for matching calls; diverging call sequences produce `ReplayDivergenceError` with a diff against the recorded transcript.
3. Traces are text-serialized (human-reviewable for test diffs) by default; binary format available via `record(to: ..., format: Binary)`.
4. `std.ffi.trace` module exposes `record` / `replay` / `Trace` type — stdlib, not built-in syntax.
5. **Replay is deterministic-by-construction.** `replay` returns recorded values verbatim for matching call sequences, including values from non-deterministic C calls (`time()`, `rand()`, `getpid()`, socket reads, etc.). This is the point of replay: deterministic test runs independent of wall-clock or OS state. Divergence from the recorded call sequence is surfaced by `ReplayDivergenceError` (rule 2); there is no separate "non-determinism detected" signal, because from replay's perspective all recorded values are treated the same. Tests that need to observe real non-deterministic behavior must drop out of replay mode and call the real library directly.

### 8. `std.ffi` Module Additions

The standard library's `std.ffi` module gains:

```ori
// Lifetime/borrow constructors (used by #borrow_from sugar)
type BorrowedView<T, ParamName>  // opaque; created by the compiler

// Record/replay support
type Trace                       // serializable FFI call transcript
type ReplayDivergenceError       // structured divergence report

@record (to: str, format: TraceFormat = TraceFormat.Text) -> FFIHandler
@replay (from: str) -> FFIHandler

// Layout verification (usually compiler-internal; exposed for advanced use)
@verify_struct_layout<T> (header: str, struct_name: str) -> Result<void, LayoutError>
```

All additions use existing Ori language features. No new stdlib-level syntax.

---

## Alternatives Considered

### Alternative 1: Keep FFI Opaque to AIMS

**Rejected.** AIMS requires every surviving logical ownership event to carry its exact fact and proof-obligation identity. A counter-selecting adapter must additionally trace each physical RC operation to its validated plan choice. FFI calls with declared ownership contracts already provide explicit ownership evidence; ignoring it would force conservative logical owner credits and releases into every physical projection. Extending the lattice across the boundary is the architecturally correct path.

### Alternative 2: Full Zig-Style `@cImport`

**Rejected** (and previously rejected in `deep-ffi-proposal.md`). Zig's approach imports an entire header as a namespace — you write `@cImport({@cInclude("sqlite3.h")})` once and every symbol becomes accessible via `c.*`. This hides the boundary at the call site: there's no Ori-code declaration naming which C functions are in use.

**The §2 design (header-assisted with explicit function list) is strictly superior** because:

- Boundary is visible at the block declaration (`extern "c" from "sqlite3" #header("sqlite3.h")`)
- Function list is visible (every imported function is named in Ori code)
- Annotations are still explicit (`owned`, `borrowed`, `#error`, `#free`)
- Drift detection is automatic (struct layout + signature verification)
- Manual signature override is still supported for unusual cases

This proposal supersedes the specific "why not `@cImport`" rejection in `deep-ffi-proposal.md` (see errata in §Spec & Grammar Impact) with a design that addresses the original concern (hidden boundary) while eliminating the approved proposal's admitted pain point (signature boilerplate).

### Alternative 3: External `ori bindgen` Tool Instead of Compiler Integration

**Rejected.** An external tool introduces a generate-check-in-regenerate workflow with the usual drift risks: generated files go stale, users forget to regenerate, CI must verify generated outputs match current headers. Build-time header reading (§2) gives the same ergonomic benefit without the workflow tax — the compiler reads the header on every build; drift is detected automatically; no generated files to check in.

For all build environments that ship libclang (Linux, macOS, Windows with LLVM-installed toolchain, standard CI runners), `#header(...)` is the canonical mechanism. `ori bindgen` is explicitly NOT a roadmap item at this time and would only be revisited if a libclang-less build environment becomes a supported Ori target. Until then, assume `#header(...)` is the one way to do header-assisted FFI binding.

### Alternative 4: Lifetime Parameters on FFI Types (Rust-Style)

**Rejected.** Ori deliberately avoids lifetime parameters (`deep-ffi-proposal.md` §"Why does `borrowed str` copy immediately?"). Introducing lifetimes at the FFI boundary would re-introduce the complexity Ori chose to avoid at the language level. `#borrow_from(param)` (§4) achieves the same safety guarantee via an attribute that references a parameter by name — single-level borrow tracking without lifetime variables.

### Alternative 5: Runtime Verification Instead of Compile-Time

**Rejected.** Runtime verification of struct layouts, callback capability availability, or FFI contracts reports bugs after corruption has already occurred. The mission-aligned path is compile-time verification where possible (struct layouts, callback `uses`, signatures) with runtime fall-back only for dynamically-loaded libraries (`dlopen` scenarios).

---

## Purity Analysis

**Can be pure Ori?** NO for all seven items. FFI is by definition the boundary between Ori and external code; making it safer requires compiler and analysis work that cannot live in user-level Ori code.

| Item | Why Compiler Support Required |
|------|-------------------------------|
| 1. AIMS across FFI | AIMS is a compile-time pass (`ori_arc`); no stdlib equivalent |
| 2. `#header(...)` attribute | Requires build-time header parsing via libclang; compiler-only |
| 3. Struct layout verification | Compares Ori layout against C layout at build time |
| 4. `#borrow_from(p)` | Type checker + AIMS locality tracking |
| 5. Callback capability propagation | Type checker verifies capability subsets |
| 6. FFI-aware diagnostics | `ori_diagnostic` extension; compiler-internal |
| 7. Handler-mocking formalization | §7a + §7b already exist; §7c record/replay is stdlib (`std.ffi.trace`) — partial purity |

**Recommendation:** Proceed as compiler features. `std.ffi.trace` is the one stdlib component; everything else is compiler. The dependency on libclang is already common in the Rust/Swift/C++ ecosystems; it is an acceptable build-time dependency for Ori toolchain.

---

## Spec & Grammar Impact

### Grammar Additions

```ebnf
(* Extended extern block attributes *)
extern_block_attr = "#header" "(" string_literal ")"
                  | "#header_path" "(" string_literal ")"
                  | (* existing: #error, #free *) .

(* Extended extern item attributes *)
extern_item_attr  = "#borrow_from" "(" identifier { "," identifier } ")"
                  | "#verify_layout" "(" string_literal ")"
                  | (* existing: #error, #free *) .

(* Callback type with capabilities *)
callback_type     = "(" param_list ")" "->" type [ "uses" cap_list ] .

(* Handler attributes *)
handler_attr      = "#strict" .
```

### Spec Clause Updates

- `spec/24-ffi.md` — new subsections for §2 (header-assisted blocks), §4 (`#borrow_from`), §5 (callback capabilities), §3 (layout verification).
- `spec/24-ffi.md` — `#error`/`#free` sections remain; §1 of this proposal adds a new subsection on AIMS integration that cross-references `aims-rules.md §5`.
- `aims-rules.md §5` — `ParamContract`, `ReturnContract` definitions extended to note FFI-origin contracts.

### Errata on Approved Proposal

Per `proposals.md §"Errata on Approved Proposals"`, add to `deep-ffi-proposal.md`:

```markdown
## Errata (added 2026-04-20)

> **Superseded by ffi-boundary-safety-proposal.md**: The design decision in
> "Why not auto-import C headers (like Zig's @cImport)?" rejected header
> auto-import on boundary-visibility grounds. A third design — header-assisted
> extern blocks with explicit function lists (`#header(...)`) — preserves
> boundary visibility AND eliminates signature boilerplate. The original
> rejection of the Zig-style namespace import stands; the header-assisted
> approach is distinct and is approved separately.
```

### New Diagnostic Codes

Reserve E4015–E4025 for FFI boundary safety diagnostics:

| Code | Condition |
|------|-----------|
| E4015 | Struct layout mismatch with C header |
| E4016 | Signature mismatch between Ori declaration and C header |
| E4017 | `#borrow_from` references non-parameter |
| E4018 | Borrowed value outlives source parameter |
| E4019 | Callback capability subset violation |
| E4020 | `#header(...)` path not resolvable |
| E4021 | Header parse error (libclang diagnostic bubbled up) |
| E4022 | `#strict` handler: call to unmocked function |
| E4023 | Replay divergence from recorded trace |
| E4024 | Invalid combination of `owned` / `borrowed` / `#borrow_from` |
| E4025 | `#verify_layout` target struct not found in header |

---

## Roadmap Impact

This proposal introduces five new compiler milestones; suggested sequencing matches the §Design numbering:

1. **AIMS FFI integration** — extends existing AIMS contract extraction in `ori_arc/src/aims/interprocedural/extract.rs` to consume ownership annotations. Estimated: 4–6 weeks.
2. **Header-assisted blocks** — adds libclang dependency, build-time header parsing, signature generation. Estimated: 6–8 weeks.
3. **Layout verification** — depends on (2); reuses the libclang integration. Estimated: 3–4 weeks.
4. **`#borrow_from`** — extends AIMS locality tracking + type checker. Estimated: 3–4 weeks.
5. **Callback capability propagation** — type checker + capability subset verification. Estimated: 3–4 weeks.
6. **Diagnostic quality** — `ori_diagnostic` extension; incremental. Estimated: 2–3 weeks.
7. **Handler formalization + record/replay** — mostly stdlib; `#strict` is type-checker. Estimated: 3–4 weeks.

**Total**: 24–33 weeks (6–8 months) for full implementation, assuming prior dependencies (Deep FFI Phase 1–4, stateful mock testing, platform FFI) are complete.

## Migration / Breaking Changes

None. All additions are opt-in annotations on existing declarations. Existing FFI code continues to work unchanged; AIMS extensions improve emitted code quality without changing semantics.

---

## Prior Art

| Language/Tool | What They Do | What Ori Learns |
|---------------|--------------|-----------------|
| **Rust `bindgen`** | External tool generates Rust FFI bindings from C headers | Demonstrates the drift problem — generated files go stale. Ori integrates compile-time instead. |
| **Rust `cxx`** | Type-safe Rust ↔ C++ interop; shared type definitions verified at both sides | Template for build-time verification against foreign types. Our struct layout verification is analogous. |
| **Zig `@cImport`** | Whole-header namespace import | Rejected: hides boundary at call site. Our `#header(...)` requires explicit function list to preserve visibility. |
| **Swift Clang Importer** | Auto-imports C/Obj-C headers; propagates nullability annotations; bridges ARC | Swift demonstrates extensive automatic header consumption is viable. Ori's scope is narrower (function list required) but the mechanism is similar. |
| **Go cgo** | Captures errno automatically; manual type marshaling otherwise | Errno auto-capture matches approved Deep FFI `#error(errno)`. Broader auto-marshaling is Ori's contribution. |
| **Python cffi** | `ffi.gc()` attaches destructor to pointer; declarative C declarations | Approved Deep FFI's `#free(fn)` is the Ori equivalent. |
| **Koka FFI** | Effect-based FFI with mask/unmask for effect scoping | Koka's mask-to-exclude-effects inspires the callback capability propagation design — capabilities as effects that must be declared at registration. |
| **Java Project Panama** | Auto-generates Java bindings from C headers via `jextract` | Panama validates the industry direction: header-assisted binding generation is the mature approach. |

### Intelligence-Graph Findings

Manually verified against intel-query results (2026-04-20):

- [rust#132699] Spurious FFI-safety warnings on non_exhaustive struct — evidence Rust's FFI diagnostics are still an active issue (2 reactions, 2 comments, open).
- [rust#130757] Spurious "infinitely recursive" not ffi-safe warning — same theme.
- [go#42469] `proposal: cmd/cgo: unsafe FFI calls` (12 reactions, 30 comments, closed completed) — Go community attempted to mark which cgo calls are unsafe; formal proposal process.
- [lean4#751] doc: FFI — evidence Lean4's FFI documentation is a known gap.
- Cross-language search "bindgen automatic C header": 13,428 Go hits, 9,083 Swift hits, 4,808 Rust hits — industry-wide unsolved problem.

### What No Language Has Combined

- Ownership annotations that extend memory-analysis lattices across the boundary (our §1) — Swift's ARC bridges exist, but Swift has no lattice analysis comparable to AIMS.
- Capability propagation through callback registration (our §5) — Koka has effect propagation, but callbacks through C are an open problem in Koka.
- Record-and-replay FFI traces via the language's effect system (our §7c) — implementations exist as testing libraries in various ecosystems (e.g., VCR for Ruby HTTP) but none operate at the language-FFI level.

---

## Future Work (Deferred to Sibling Proposals)

Not covered here, to be addressed separately:

1. **`extern "objc"` — Objective-C interop** — required for macOS platform APIs (CoreText, AppKit). Obj-C message-passing ABI is distinct from plain C.
2. **`extern "c++"` — C++ interop** — name mangling, vtables, exceptions, destructors. Approved proposal lists this as future work; remains so.
3. **`extern "rust"` — Rust static library interop** — some high-value libraries (rustls, wgpu) have Rust-only APIs.
4. **C enum → Ori sum type auto-conversion** — declarative mapping for C enums declared in `#header(...)`.
5. **Linking and distribution** — static vs dynamic linking policy, pkg-config integration, how Ori FFI binding packages are published.
6. **WebAssembly component model interop** — calling Wasm components from Ori; forward-looking.
7. **Arena-scoped FFI** — from approved proposal's future work; orthogonal to boundary safety.
