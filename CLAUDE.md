**Ori is under construction.** Rust tooling (cargo, rustc) is trusted and stable. Ori tooling (lexer, parser, type checker, evaluator, test runner) is NOT—all are being built from scratch. When something fails, investigate the Ori infrastructure first. Do not assume user code or tests are wrong; the bug is often in the compiler/tooling itself.

**This system is ONE thing.** The compiler, type checker, evaluator, codegen, tests, spec, stdlib — they are not separate projects. They are one machine. A broken piece anywhere means the whole machine is broken. There is no "my change" vs "someone else's code." There is no "unrelated." There is no "pre-existing." There is no "outside scope." If you encounter something broken, it is your problem right now. Do not label it and move on. Investigate why it's broken. Understand the cause. Fix it. Every single piece must fit together to form a complete, working whole — a puzzle with even one missing piece is an incomplete puzzle. Add discovered issues to your todo list and fix them before completing your task.

**Do it properly, not just simply. Correct architecture over quick hacks; no shortcuts or "good enough" solutions.**

**NO WORKAROUNDS. NO HACKS. NO SHORTCUTS.**
- **Proper fixes only** — If a fix feels hacky, it IS hacky. Find the right solution.
- **When unsure, STOP and ASK** — Do not guess. Do not assume. Pause and ask the user for guidance.
- **Fact-check everything** — Verify behavior against the spec. Test your assumptions. Read the code you're modifying.
- **Consult reference repos** — Check `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TypeScript, Gleam, Elm, Roc, Swift, Koka, Lean 4) for established patterns and idioms.
- **No "temporary" fixes** — There is no such thing. Today's temporary fix is tomorrow's permanent tech debt.
- **If you can't do it right, say so** — Communicate blockers rather than shipping bad code.

**TDD for bugs — NEVER fix without tests first**:
1. **STOP** — Do not jump to fixing. Resist the urge to immediately change code.
2. **Understand** — Consult the spec (`docs/ori_lang/0.1-alpha/spec/`), grammar, and design docs. Know the *intended* behavior.
3. **Reproduce with tests** — Write MULTIPLE tests (not just one):
   - The exact failing case that exposed the bug
   - Edge cases: boundaries, empty inputs, single elements, maximum values
   - Related variations: similar patterns that might also be affected
   - Regression guards: cases that currently work and must continue to work
4. **Verify tests fail** — All bug-reproducing tests MUST fail. If they pass, you misunderstand the bug or the spec.
5. **Fix the code** — Now implement the fix.
6. **Tests pass unchanged** — All tests must pass WITHOUT modifying them. If you need to change tests to pass, you either wrote wrong tests or made a wrong fix. Go back to step 2.

---

## Ori Language

**Ori**: Statically-typed **expression-based** language. HM inference, ARC memory, capability effects, mandatory tests. Targets LLVM/WASM. Compiler in Rust (Salsa-based).

**EXPRESSION-BASED — NO `return` KEYWORD**: Every block's value is its last expression. There is no `return` statement. Early exit uses `?` (propagate errors), `break` (exit loops), or `panic` (terminate). The `return` keyword is intentionally not part of Ori. Similar to: Rust (closest), Gleam, Roc, Ruby, Elixir, OCaml.

### Ori Syntax Reference

For Ori syntax, types, patterns, and prelude:
- **Auto-loaded** when editing `.ori` files (via `.claude/rules/ori-syntax.md`)
- **On-demand**: Use `/ori-syntax` skill
- **Manual**: Read `.claude/rules/ori-syntax.md`

**Spec is authoritative**: `docs/ori_lang/0.1-alpha/spec/` (`grammar.ebnf` for syntax, `operator-rules.md` for semantics)

### Design Pillars

1. **Expression-Based**: Everything is an expression; last expression is the block's value; no `return` keyword
2. **Mandatory Verification**: Functions need tests; contracts (`pre()`/`post()` on function declarations)
3. **Dependency-Aware Integrity**: Tests in dep graph; changes propagate
4. **Explicit Effects**: Capabilities (`uses Http`); trivial mocking (`with Http = Mock in`)
5. **ARC-Safe**: No GC/borrow checker; capture by value; no shared mutable refs

---

## Compiler Coding Guidelines

**Architecture**: Crate deps: `oric` → `ori_types/eval` → `ori_parse` → `ori_lexer` → `ori_ir/diagnostic` (no upward); IO only in CLI (`oric`); no phase bleeding

**Memory**: Arena + ID (`ExprArena`+`ExprId`, not `Box<Expr>`); intern identifiers (`Name`, not `String`); newtypes for IDs; no `Arc` cloning in hot paths; `#[cold]` on error factories

**Salsa**: Query types derive `Clone, Eq, PartialEq, Hash, Debug`; no `Arc<Mutex<T>>`, fn pointers, or `dyn Trait`; deterministic (no random/time/IO); accumulate errors

**API Design**: >3-4 params → config struct; no boolean flags; RAII guards for context; return iterators not `Vec`; document public items

**Dispatch**: Enum for fixed sets (exhaustiveness, static dispatch); `dyn Trait` only for user-extensible; cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`

**Diagnostics**: All errors have spans; imperative suggestions ("try using X"); verb phrase fixes ("Replace X with Y"); no `panic!` on user errors; accumulate

**Testing**: Verify behavior not implementation; tests based on spec, not current code; multiple test angles per feature (happy path, edge cases, error cases); TDD is mandatory for bugs (see above). **Test file locations**: tests live in sibling `tests.rs` files, not inline. Source files use `#[cfg(test)] mod tests;` (declaration only). Resolution: `foo.rs` → `foo/tests.rs`; `mod.rs` in `bar/` → `bar/tests.rs`; `lib.rs`/`main.rs` → `tests.rs` in same dir

**Performance**: O(n²) → O(n) or O(n log n); hash lookups not linear scans; no allocation in hot loops; iterators over indexing

**Style**: No `#[allow(clippy)]` without justification; functions < 100 lines (strongly prefer shorter — target < 50); no dead/commented code or banners; `//!`/`///` docs

**File Size — 500 line recommended limit**: Source files (excluding tests) should stay under 500 lines. When writing code that would push a file past this limit, **stop and split first** — do not add the code and plan to split later. Extract logical groups (e.g., a set of related methods, a submodule's types, a match arm group) into a new sibling file/submodule. When touching a file already over 500 lines, take the opportunity to split it. Use `scripts/extract_tests.py` for test extraction; for production code, create submodules with `mod foo;` and move the extracted code there.

**Tracing — ALWAYS USE FOR DEBUGGING**: `ORI_LOG` is your **first** debugging tool. Before `println!`, before reading code line-by-line, turn on tracing. Use `tracing` macros, not `println!`/`eprintln!`. Levels: `error` (never happen), `warn` (recoverable), `debug` (phases/queries), `trace` (per-expression). Targets by crate: `ori_types` (type checker), `ori_eval` (evaluator), `ori_llvm` (codegen), `oric` (Salsa queries). Use `#[tracing::instrument]` on public API functions. Salsa queries use manual `tracing::debug!()`. Setup: `compiler/oric/src/tracing_setup.rs`.

**Match Extraction**: No 20+ arm match in single file; group related arms; 3+ similar → extract helper

**Continuous Improvement**: Fix ALL issues in code you touch — dead code, unclear names, duplicated logic, style violations. There is no boundary between "your code" and "other code." Every file you open, every function you read, every test you run is part of the same system you are building. If it's broken, fix it. If it's messy, clean it. If it drifted, sync it. Do not narrow your vision to a single function or file — zoom out, see the whole system, and make it coherent. Refactor when patterns emerge.

---

## Commands

**Primary**: `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh`, `./build-all.sh` (includes LLVM)
**Tests**: `cargo t` (Rust), `cargo st` (Ori), `cargo st tests/spec/path/` (specific), `./llvm-test.sh`
**Build**: `cargo c`/`cl`/`b`/`fmt`, `./llvm-build.sh`, `./llvm-clippy.sh`
**LLVM/AOT**: `cargo bl` (debug), `cargo blr` (release) — builds oric + ori_rt (LLVM is a default feature; `cargo bl` additionally builds `ori_rt` for AOT linking)
**Optimized release**: `cargo build --profile release-lto` — fat LTO + single codegen unit (~20% faster binary, ~3.5x longer build). Output: `target/release-lto/ori`. Use for published binaries and benchmarks. Regular `--release` (used by `test-all.sh`, `cargo blr`) is unaffected.
**Tracing/Debugging** (USE FIRST — before println, before reading code line-by-line):
`ORI_LOG=debug ori check file.ori` | `ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check file.ori` | `ORI_LOG=ori_eval=debug ori run file.ori` | `ORI_LOG=oric=debug ori check file.ori` (Salsa queries) | Falls back to `RUST_LOG`
**Phase dumps** (show compiler IR at each stage): `ORI_DUMP_AFTER_PARSE=1` (AST) | `ORI_DUMP_AFTER_TYPECK=1` (typed IR) | `ORI_DUMP_AFTER_ARC=1` (ARC IR with RC strategies) | `ORI_DUMP_AFTER_LLVM=1` (annotated LLVM IR, superset of `ORI_DEBUG_LLVM`) — all output to stderr, zero overhead in release builds.
**Runtime debug** (checked in `ori_rt`): `ORI_TRACE_RC=1` (RC operation log) | `ORI_RT_DEBUG=1` (bounds/underflow assertions) | `ORI_CHECK_LEAKS=1` (report live RC objects on exit)
**Codegen audit**: `ORI_AUDIT_CODEGEN=1` — in-pipeline RC verification: checks alloc/dec balance, COW sequencing, ABI arg counts, large aggregate loads. Zero cost when disabled. Options: `ORI_AUDIT_STRICT=1` (pessimistic: COW treated as freeing, params tracked as RC-managed) | `ORI_AUDIT_FUNCTION=name` (filter to functions matching substring).
**Consistency**: `diagnostics/check-debug-flags.sh` — validates all `ORI_*` flags are defined, used, and documented.
**Always run `./test-all.sh` after compiler changes.**
**Performance baseline**: `./scripts/perf-baseline.sh [--release]` — measures compile time, AOT vs JIT runtime, and binary sizes for benchmark programs in `tests/benchmarks/`.
**Diagnostic scripts** (`diagnostics/`): Quick AOT/codegen debugging — USE THESE when investigating LLVM, ARC, or backend mismatch bugs:
- `diagnostics/ir-dump.sh <file.ori>` — annotated LLVM IR dump (use `--raw` for undecorated)
- `diagnostics/ir-diff.sh <a.ori> <b.ori>` — side-by-side IR comparison between two programs
- `diagnostics/disasm-ori.sh <file.ori>` — native disassembly with Ori symbol demangling
- `diagnostics/rc-stats.sh <file.ori>` — count RC operations (alloc/inc/dec/free) per function in LLVM IR, flag imbalances
- `diagnostics/diagnose-aot.sh <file.ori>` — all-in-one: compile + execute + leak check + RC stats + IR dump + optional Valgrind (`--valgrind`) and disasm (`--verbose`)
- `diagnostics/dual-exec-debug.sh <file.ori>` — run through interpreter AND AOT, compare exit codes + stdout; on mismatch auto-dumps IR and RC stats (`--verbose` adds `ORI_LOG=debug` traces)
- `diagnostics/codegen-audit.sh <file.ori>` — static analysis of LLVM IR: RC balance, COW correctness, ABI conformance (`--strict` for pessimistic, `--function <name>` to filter)
- `diagnostics/valgrind-aot.sh [file.ori ...]` — compile Ori programs to native binaries and run under Valgrind for memory error detection (use-after-free, double-free, leaks). Defaults to `tests/valgrind/`. Not part of `test-all.sh` (Valgrind is 20-50x slower).
- `diagnostics/dual-exec-verify.sh [test-path]` — batch comparison of interpreter vs LLVM backend across spec tests. Flags behavioral mismatches. Supports `--test-only`, `--main-only`, `--verbose`, `--json[=PATH]`.
All scripts support `--help`, `--no-color`/`--color`, and handle missing args and compilation failures gracefully.

> **Note**: AOT compilation (`ori build`) requires `libori_rt.a`. Use `cargo bl`/`blr` to build both the compiler and runtime library together.

## Key Paths

`compiler/oric/` — compiler | `docs/ori_lang/0.1-alpha/spec/` — **spec (authoritative)** | `spec/grammar.ebnf` — syntax | `spec/operator-rules.md` — operator semantics | `docs/ori_lang/proposals/` — proposals | `library/std/` — stdlib | `tests/spec/` — conformance | `compiler/oric/tests/phases/` — phase tests | `compiler/ori_llvm/tests/aot/` — AOT integration tests | `tests/valgrind/` — Valgrind memory safety tests | `tests/benchmarks/` — AOT performance benchmarks | `diagnostics/` — AOT/codegen diagnostic scripts (ir-dump, ir-diff, disasm, rc-stats, diagnose-aot, dual-exec-debug, valgrind-aot, dual-exec-verify) | `plans/roadmap/` — roadmap

## Reference Repos (`~/projects/reference_repos/lang_repos/`)

- **rust** — `rustc_errors/src/{lib,diagnostic,json}.rs`, `rustc_lint_defs/src/lib.rs`
- **golang** — `cmd/compile/internal/base/print.go`, `go/types/errors.go`, `internal/types/errors/codes.go`
- **typescript** — `compiler/{types.ts,diagnosticMessages.json}`, `services/{codeFixProvider,textChanges}.ts`
- **zig** — `src/{Compilation,Sema,Type,Value,InternPool,Zcu,main}.zig`
- **gleam** — `compiler-core/src/{error,diagnostic,warning,analyse,exhaustiveness}.rs`
- **elm** — `compiler/src/Reporting/{Error,Suggest,Doc}.hs`, `Error/{Type,Syntax}.hs`
- **roc** — `crates/reporting/src/{report,error/{type,canonicalize,parse}}.rs`
- **swift** — `lib/SILOptimizer/ARC/`, `lib/SIL/`, `lib/Sema/`, `include/swift/AST/Ownership.h`
- **koka** — `src/Type/{Infer,Operations,Unify}.hs`, `src/Core/{Borrowed,CheckFBIP}.hs`, `src/Compile/`
- **lean4** — `src/Lean/Compiler/IR/{RC,Borrow,ExpandResetReuse}.lean`, `src/Lean/Compiler/LCNF/`

## CLI

`ori run file.ori` | `ori check file.ori` | `ori test` | `ori test --only-attached` | `ori fmt src/`

## Files & Tests

`.ori` source | Tests in `_test/` subdirectory: `foo.ori` → `_test/foo.test.ori` | Attached: `@test tests @target () -> void` (runs on target/caller changes) | Floating: `tests _` (runs via `ori test`) | Private: `::` prefix | Every function (except `@main`) requires tests

## Entry Points

`@main () -> void` | `() -> int` | `(args: [str]) -> void` | `(args: [str]) -> int` — `args` excludes program name
`@panic (info: PanicInfo) -> void` — optional handler; `print()` → stderr; first panic wins; re-panic = immediate termination
