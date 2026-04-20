---
id: "02"
title: "Header-Assisted Extern Blocks — #header(...)"
status: not-started
reviewed: false
goal: "Read C headers via libclang at build time; auto-derive extern signatures for functions listed in the block body; preserve boundary visibility (explicit function list + explicit annotations) while eliminating signature-transcription boilerplate."
success_criteria:
  - "`#header(\"sqlite3.h\")` on an `extern \"c\" from \"sqlite3\" { ... }` block produces Ori-visible signatures for every listed function, with C→Ori type translation applied automatically."
  - "Ownership (`owned`/`borrowed`), error protocols (`#error(...)`), and free declarations (`#free(...)`) remain EXPLICIT in Ori source; none are auto-inferred from the header."
  - "Explicit Ori signatures take precedence — the compiler validates the header MATCHES rather than overriding."
  - "Header path resolution follows deterministic order: (1) `#header_path(...)` explicit, (2) pkg-config `--cflags <lib>`, (3) system default include paths, (4) E4020 error."
  - "Signature mismatch between an explicit Ori declaration and the C header produces E4016 with secondary span to the header."
  - "All Ori builds that DO NOT use `#header(...)` continue to work unchanged (zero regression to approved Deep FFI codebase)."
inspired_by:
  - "Swift Clang Importer: `swift/lib/ClangImporter/` — block-scoped header import with explicit visibility rules."
  - "Java Project Panama / jextract: header-driven binding generation with explicit symbol list."
  - "Rust bindgen (rejected approach): external tool with drift risk — §02 solves drift by reading on every build."
  - "Zig @cImport (rejected approach): namespace import, hides boundary — §02 requires explicit function list."
depends_on:
  - "01"
third_party_review:
  status: none
  updated: null
---

# §02 — Header-Assisted Extern Blocks — `#header(...)`

## Context

Per approved proposal §2. Supersedes `docs/ori_lang/proposals/superseded/c-header-import-proposal.md` — the namespace-import `@cImport(...)` approach is rejected; this section implements the block-scoped `#header(...)` approach.

## §02.1 — Grammar + parser

- [ ] Extend `compiler/ori_parse/src/grammar/item/extern_def.rs` to accept block-level attributes `#header("file.h")` and `#header_path("search/path")` at the `extern "c" from "lib"` position.
- [ ] Grammar production (see `grammar.ebnf` additions tracked in §08):
  ```ebnf
  extern_block_attr = "#header" "(" string_literal ")"
                    | "#header_path" "(" string_literal ")"
                    | (existing: #error, #free) .
  ```
- [ ] Update AST: add `header: Option<PathBuf>`, `header_path: Option<PathBuf>` fields to the extern block AST node.
- [ ] Parser-level validation: `#header` and `#header_path` require a string literal path. E1xxx error code (reserve at parser layer) for malformed attribute syntax; NOT in the E40xx FFI range.

## §02.2 — libclang integration

- [ ] Create new crate `compiler/ori_cheader/` (candidate name; final name decided during implementation) that wraps libclang. Crate is a pure leaf (no Ori-compiler dependencies — produces a `CHeaderFacts` struct consumed by `ori_types`). Why a separate crate: libclang is a heavy native dependency; isolating it keeps the `ori_types` rebuild graph clean when libclang's transitive C++ deps change.
- [ ] Dependencies: use the `clang-sys` or `clang` Rust crate (evaluate in design phase). libclang itself is already present on the host (LLVM 21 toolchain per `c-header-import-proposal.md` observation).
- [ ] Public API: `fn parse_header(path: &Path, search_paths: &[PathBuf]) -> Result<CHeaderFacts, CHeaderError>` returning declared functions, structs, typedefs, enum values (not used by §02 but needed by §03).
- [ ] Cache parsed headers per-build using Salsa (the `CHeaderFacts` for a given `(path, search_paths)` tuple is pure with respect to the header's content hash).

## §02.3 — Signature auto-derivation + validation

- [ ] In `ori_types` during extern-block type-checking, when `#header(...)` is present:
  - [ ] For each listed function in the block body WITHOUT an explicit signature: derive signature from header facts. Map C types → Ori types per the table below.
  - [ ] For each listed function WITH an explicit signature: validate the signature MATCHES the header's declaration. Mismatch → E4016.
  - [ ] If a function is listed but NOT present in the header → E4020 or E4021 (depending on whether the header parsed at all).
- [ ] C → Ori type mapping table (normative):

  | C Type | Ori Type |
  |---|---|
  | `int`, `int32_t` | `c_int` |
  | `unsigned int`, `uint32_t` | `c_uint` |
  | `long`, `int64_t` | `c_long` |
  | `unsigned long`, `uint64_t` | `c_ulong` |
  | `short`, `int16_t` | `c_short` |
  | `unsigned short`, `uint16_t` | `c_ushort` |
  | `char`, `int8_t` | `c_char` |
  | `unsigned char`, `uint8_t` | `byte` |
  | `size_t` | `c_size` |
  | `ssize_t` | `c_long` |
  | `float` | `c_float` |
  | `double` | `c_double` |
  | `void` | `void` |
  | `void*` | `CPtr` |
  | `const char*` | `str` (borrowed by default) |
  | `T*` (named struct) | `CPtr` (opaque, unless Ori has matching `#repr("c")` struct with `#verify_layout`) |
  | `T[]` byte arrays | `[byte]` |

## §02.4 — Header path resolution (deterministic)

Per approved proposal §2 rule 5:

- [ ] Implement resolution order:
  1. Explicit `#header_path("...")` on the block → use verbatim.
  2. `pkg-config --cflags <lib>` when `from "<lib>"` is a pkg-config-registered library → extract `-I` paths.
  3. System default include paths (`/usr/include`, `/usr/local/include`, compiler-default on host) → host C toolchain defaults.
  4. No match → E4020 "header path not resolvable."
- [ ] First match wins. `#header_path` ALWAYS trumps pkg-config; hermetic build systems (Bazel, Nix) use this to vendor headers.
- [ ] pkg-config invocation wrapped in a cached helper (cache per `(lib, working_dir)` via Salsa).
- [ ] When pkg-config is ABSENT on the host (Windows without MSYS2, sandboxed builds), resolution falls through to step 3 silently — no error unless step 4 fires.

## §02.5 — Matrix tests

- [ ] **Failing tests FIRST:**
  - AOT test `tests/spec/ffi/header_sqlite.ori` — simulates a small SQLite binding via `#header("sqlite3.h")` — requires libsqlite3-dev in CI. If CI does not provide sqlite3-dev, use a checked-in fixture header `tests/fixtures/mock_sqlite.h`.
  - FileCheck test: derived signatures match the header's actual declarations.
- [ ] **Matrix dimensions:**
  - **Declaration style × Header availability:** `{no #header, explicit sig}` × `{#header, explicit sig}` × `{#header, no explicit sig}` × `{#header, sig conflicts with header}` = 4 cells.
  - **Type mapping coverage:** at least one test per row in §02.3's mapping table (15 cells).
  - **Path resolution:** `{explicit #header_path}` × `{pkg-config resolves}` × `{system include resolves}` × `{none resolve → E4020}` = 4 cells.
- [ ] **Semantic pins:**
  - A program that compiles ONLY with §02 (uses `#header(...)` without explicit signatures) — FileCheck that the signatures are present in the LLVM IR.
  - A negative pin: a program whose explicit Ori signature diverges from the header MUST emit E4016 at compile time.
- [ ] **Regression guard:** 20+ approved Deep FFI test programs (no `#header`) continue to compile unchanged — run as a separate `cargo test -p oric -- existing_ffi_regression` suite.

## §02.6 — Documentation

- [ ] Add §02-specific section to `docs/ori_lang/v2026/spec/26-ffi.md` (co-commit with §08 sync run — see §08 for the aggregate).
- [ ] Add attribute entries to `.claude/rules/ori-syntax.md` Deep FFI section (`#header`, `#header_path`).
- [ ] Document the C→Ori type mapping table in the spec; reference it from `ori-syntax.md` without duplicating.

## Completion Checklist

- [ ] §02.1 grammar + parser ships with tests.
- [ ] §02.2 libclang wrapper crate ships; Salsa-cached.
- [ ] §02.3 signature auto-derivation + validation works; type mapping complete.
- [ ] §02.4 path resolution deterministic; pkg-config absence handled.
- [ ] §02.5 matrix tests green, semantic pin green, negative pin green, regression guard green.
- [ ] §02.6 spec clause + `ori-syntax.md` entries committed.
- [ ] `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` all green (debug + release).
- [ ] `/tpr-review` clean. `/impl-hygiene-review` clean.
- [ ] `/improve-tooling` section-close sweep. `/sync-claude` section-close.
- [ ] `sections[id=02].status: complete`, `reviewed: true`.

## §02.R — Third Party Review Findings

- None.
