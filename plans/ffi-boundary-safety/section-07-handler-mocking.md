---
id: "07"
title: "Handler-Mocking Formalization + Record/Replay"
status: not-started
reviewed: false
goal: "Formalize three practical details of handler-based FFI mocking: (7a) #strict partial-mock mode rejecting unmocked calls at compile time; (7b) stateful-mock + FFI state integration (documentation only — mechanism already approved); (7c) record/replay via `std.ffi.trace` with deterministic-by-construction replay."
success_criteria:
  - "`handler #strict { ... }` rejects unmocked function calls at compile time with E4022."
  - "Default handler mode (no #strict) falls through unmocked calls to the real C library (as approved Deep FFI specifies)."
  - "Record mode produces a deterministic transcript file; replay mode reads the transcript and returns recorded values verbatim for matching call sequences."
  - "Replay divergence (different call sequence) produces E4023 / ReplayDivergenceError with a structured diff."
  - "Traces are text-serialized by default; binary format available via `format: TraceFormat.Binary`."
  - "Replay is deterministic-by-construction: recorded values for non-deterministic C calls (time, rand) are returned verbatim — no separate 'non-determinism detected' signal."
  - "`std.ffi.trace` module exists in `library/std/ffi/trace.ori` using pure Ori (existing language features only; no new syntax)."
inspired_by:
  - "VCR (Ruby HTTP recording): record/replay pattern at the library level."
  - "Python vcrpy: canonical non-Ori record/replay implementation."
  - "Koka: handler rows as formal type surface — §7a #strict is the equivalent of row-polymorphism narrowing."
depends_on: []
third_party_review:
  status: none
  updated: null
---

# §07 — Handler-Mocking Formalization + Record/Replay

## Context

Per approved proposal §7 (split into 7a, 7b, 7c). §07 is INDEPENDENT of §01–§06 — can execute in parallel with §02 if scheduling permits. §07 delivers the missing bits left underspecified in approved Deep FFI and stateful-mock-testing proposals.

## §07.1 — Stdlib module `library/std/ffi/`

- [ ] Create `library/std/ffi/` directory (new).
- [ ] `library/std/ffi/mod.ori` — module root re-exports.
- [ ] `library/std/ffi/trace.ori` — `Trace`, `record(to:, format:)`, `replay(from:)`, `ReplayDivergenceError`, `TraceFormat` (enum: `Text | Binary`).
- [ ] `library/std/ffi/borrow.ori` — `BorrowedView<T, ParamName>` opaque type + `.to_owned()` (shared with §04).
- [ ] All pure Ori; no new language syntax. Uses `uses FFI("lib")` capability propagation.
- [ ] Add `library/std/ffi/tests.ori` covering trace round-trip, format selection, divergence detection.

## §07.2 — `#strict` handler attribute

- [ ] Grammar production (ships with §08):
  ```ebnf
  handler_attr = "#strict" .
  ```
- [ ] Parser: `handler #strict { ... }` accepts the attribute. AST: `HandlerExpr` gains a `strict: bool` field.
- [ ] Type-check pass: when a with-in block has a `#strict` handler AND the body calls an FFI function from the mocked library that is NOT listed in the handler's function table, emit E4022 at the call site citing the missing function and the strict handler's span.
- [ ] Default (no `#strict`) preserves approved Deep FFI fall-through semantics — unmocked calls dispatch to the real library.

## §07.3 — Record/replay trace format

- [ ] Text format schema (human-reviewable):
  ```
  # Ori FFI trace v1
  # library: sqlite3
  [call] sqlite3_open args=["mydb.sqlite"] result=Ok(CPtr(0x7f1234))
  [call] sqlite3_exec args=[CPtr(0x7f1234), "CREATE TABLE..."] result=Ok(())
  [call] sqlite3_close args=[CPtr(0x7f1234)] result=Ok(())
  ```
- [ ] Binary format: efficient serialization of the same data (bincode or similar).
- [ ] `ReplayDivergenceError` payload: structured diff showing expected vs actual call sequence (function name, argument hashes, position in trace).

## §07.4 — Record mode implementation

- [ ] `record(to: str, format: TraceFormat = TraceFormat.Text) -> FFIHandler` — returns a handler that wraps the real FFI library.
- [ ] On each FFI call: (1) invoke the real library, (2) serialize args + result to the trace file, (3) return the result to the caller.
- [ ] Thread-safety: trace writes are serialized via a mutex (or similar); trace file is appended atomically per call.
- [ ] Flush on handler drop.

## §07.5 — Replay mode implementation

- [ ] `replay(from: str) -> FFIHandler` — returns a handler that reads the trace file.
- [ ] On each FFI call: (1) read next trace entry, (2) verify function name + args hash match, (3) return the recorded result; (4) on mismatch emit `ReplayDivergenceError` with the diff.
- [ ] Deterministic-by-construction: recorded values for time/rand/getpid/socket-reads are returned verbatim on replay — no separate signal. Tests needing real non-determinism drop out of replay mode.
- [ ] Replay consumes trace entries linearly; test can run in isolation without the real library.

## §07.6 — Matrix tests

- [ ] **Failing tests FIRST:** a test that records a 3-call sequence, then replays it — results match exactly. A test with a 4th call that wasn't recorded fires `ReplayDivergenceError`.
- [ ] **Matrix:**
  - **Mode × Handler kind:** `{record, default}` / `{record, #strict}` / `{replay, default}` / `{replay, #strict}` / `{stateful handler with record}` / `{stateful handler with replay}` = 6 cells.
  - **Format:** Text / Binary = 2 cells.
  - **Divergence pattern:** missing call (replay expects more than recorded) / extra call (replay runs out of entries before test ends) / wrong arg / wrong function name = 4 cells.
  - **`#strict` coverage:** 3 functions in lib, handler mocks 2, test calls all 3 → E4022 on the 3rd = 1 cell.
- [ ] **Semantic pin:** record-then-replay round-trip for a mock SQLite session file produces identical observable Ori behavior; removing the `replay` wrapper causes the test to fail without the real library.
- [ ] **Negative pin:** `#strict` handler + unmocked call = E4022 at compile time.

## §07.7 — Documentation

- [ ] Add §7a / §7b / §7c surface to spec §26-ffi.md (co-commit with §08).
- [ ] Update `.claude/rules/ori-syntax.md` Deep FFI section with `#strict` + `record` / `replay` entries.
- [ ] Add `library/std/ffi/trace.ori` to stdlib module index (`docs/ori_lang/v2026/modules/` if that surface exists for stdlib — verify in implementation).

## Completion Checklist

- [ ] §07.1 `library/std/ffi/` module ships.
- [ ] §07.2 `#strict` attribute + E4022 emission lands.
- [ ] §07.3 trace format schemas defined (text + binary).
- [ ] §07.4 record mode works end-to-end.
- [ ] §07.5 replay mode works end-to-end; E4023 / ReplayDivergenceError on divergence.
- [ ] §07.6 matrix tests green; semantic pin green; negative pin green.
- [ ] §07.7 documentation synced.
- [ ] `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` green.
- [ ] `/tpr-review` clean; `/impl-hygiene-review` clean.
- [ ] `/improve-tooling` + `/sync-claude` sweeps.
- [ ] `sections[id=07].status: complete`, `reviewed: true`.

## §07.R — Third Party Review Findings

- None.
