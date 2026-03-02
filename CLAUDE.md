- **Under construction** — Rust tooling trusted; Ori tooling (lexer, parser, typeck, eval, test runner) is NOT. Bugs are in the compiler, not user code.
- **One system** — compiler, typeck, eval, codegen, tests, spec, stdlib are one machine. No "unrelated", "pre-existing", or "out of scope." Fix every issue encountered. Add discovered issues to todo list.
- **Proper fixes only** — no workarounds, hacks, shortcuts, or temporary fixes. Correct architecture over quick hacks.
- **When unsure, STOP and ASK** — don't guess or assume
- **Fact-check** against spec. Consult `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4).
- **If you can't do it right, say so** — communicate blockers, don't ship bad code

**TDD for bugs** — NEVER fix without tests first:
1. **STOP** — resist urge to immediately change code
2. **Consult spec** (`docs/ori_lang/v2026/spec/`) for intended behavior
3. **Write MULTIPLE tests**: exact failing case, edge cases, related variations, regression guards
4. **Verify tests fail** — if they pass, you misunderstand the bug
5. **Fix the code**
6. **Tests pass unchanged** — needing to change tests = wrong tests or wrong fix

---

## Ori Language

- **Ori**: statically-typed expression-based, HM inference, ARC memory, capability effects, mandatory tests. Targets LLVM/WASM. Compiler in Rust (Salsa-based).
- **NO `return`**: last expression = block value. Exit via `?`/`break`/`panic`. Similar to Rust, Gleam, Roc.
- **Syntax ref**: `.claude/rules/ori-syntax.md` (auto-loaded for `.ori` files) | `/ori-syntax` skill
- **Spec authoritative**: `docs/ori_lang/v2026/spec/` (`grammar.ebnf`, `operator-rules.md`)

### Design Pillars
1. **Expression-based**: everything is expression; last expr = block value; no `return`
2. **Mandatory verification**: functions need tests; contracts (`pre()`/`post()`)
3. **Dependency-aware**: tests in dep graph; changes propagate
4. **Explicit effects**: capabilities (`uses Http`); mocking (`with Http = Mock in`)
5. **ARC-safe**: no GC/borrow checker; capture by value; no shared mutable refs

---

## Compiler Coding Guidelines

- **Architecture**: `oric` → `ori_types/eval` → `ori_parse` → `ori_lexer` → `ori_ir/diagnostic` (no upward); IO only in `oric`; no phase bleeding
- **Memory**: Arena + ID (`ExprArena`+`ExprId`); intern identifiers (`Name`); newtypes for IDs; no `Arc` in hot paths; `#[cold]` on error factories
- **Salsa**: derive `Clone, Eq, PartialEq, Hash, Debug`; no `Arc<Mutex<T>>`, fn pointers, `dyn Trait`; deterministic; accumulate errors
- **API**: >3-4 params → config struct; no boolean flags; RAII guards; return iterators not `Vec`
- **Dispatch**: enum for fixed sets; `dyn Trait` only for user-extensible; cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`
- **Diagnostics**: all errors have spans; imperative suggestions; no `panic!` on user errors; accumulate
- **Testing**: verify behavior not implementation; spec-based; multiple angles (happy, edge, error). **Test files**: sibling `tests.rs` (not inline); `#[cfg(test)] mod tests;` declaration only. `foo.rs` → `foo/tests.rs`; `mod.rs` → `bar/tests.rs`; `lib.rs`/`main.rs` → `tests.rs` in same dir
- **Performance**: O(n²) → O(n); hash lookups not linear scans; no alloc in hot loops; iterators over indexing
- **ARM portability**: C string pointers in `ori_rt` use `std::ffi::c_char`, never `i8` — `c_char` is `i8` on x86_64 but `u8` on aarch64
- **Style**: no `#[allow(clippy)]` without justification; functions < 100 lines (target < 50); no dead/commented code; `//!`/`///` docs
- **File size**: 500 line limit (excl. tests). Stop and split before exceeding. Extract to submodules. `scripts/extract_tests.py` for test extraction.
- **Tracing — USE FIRST**: `ORI_LOG` before `println!`. Levels: `error`/`warn`/`debug`/`trace`. Targets: `ori_types`/`ori_eval`/`ori_llvm`/`oric`. `#[tracing::instrument]` on pub APIs. Never `println!`/`eprintln!`. Setup: `compiler/oric/src/tracing_setup.rs`.
- **Match extraction**: no 20+ arm match in single file; group related arms; 3+ similar → extract helper
- **Continuous improvement**: fix ALL issues in code you touch — dead code, unclear names, duplicated logic. No boundary between "your code" and "other code." If broken, fix; if messy, clean; if drifted, sync.

---

## Commands

**Primary**: `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh`, `./build-all.sh` (includes LLVM)
**Tests**: `cargo t` (Rust, excl. LLVM), `cargo st` (Ori), `cargo st tests/spec/path/` (specific), `./llvm-test.sh`
**Build**: `cargo c`/`cl`/`b`/`fmt` (excl. LLVM), `./llvm-build.sh`, `./llvm-clippy.sh`
**LLVM/AOT**: `cargo bl` (debug), `cargo blr` (release) — builds oric + ori_rt; `cargo test -p ori_llvm` (LLVM tests); `cargo cll` (LLVM clippy)
**Release LTO**: `cargo build --profile release-lto` — fat LTO, ~20% faster binary, ~3.5x longer build. Output: `target/release-lto/ori`. Regular `--release` unaffected.
**Tracing** (USE FIRST): `ORI_LOG=debug ori check file.ori` | `=ori_types=trace ORI_LOG_TREE=1 ori check f.ori` | `=ori_eval=debug ori run file.ori` | `=oric=debug` (Salsa) | Falls back to `RUST_LOG`
**Phase dumps**: `ORI_DUMP_AFTER_PARSE=1` (AST) | `ORI_DUMP_AFTER_TYPECK=1` (typed IR) | `ORI_DUMP_AFTER_ARC=1` (ARC IR) | `ORI_DUMP_AFTER_LLVM=1` (LLVM IR, superset of `ORI_DEBUG_LLVM`) | `ORI_EMIT_ARC_DOT=1` (GraphViz DOT) — stderr, zero release overhead
**Runtime debug**: `ORI_TRACE_RC=1` (RC log) | `ORI_RT_DEBUG=1` (assertions) | `ORI_CHECK_LEAKS=1` (leak report)
**Codegen audit**: `ORI_AUDIT_CODEGEN=1` — RC balance, COW sequencing, ABI args, aggregate loads, safety checks. Zero cost off. `ORI_AUDIT_STRICT=1` (pessimistic) | `ORI_AUDIT_FUNCTION=name` (filter)
**Always run `./test-all.sh` after compiler changes.**
**Perf baseline**: `./scripts/perf-baseline.sh [--release]` | **Consistency**: `diagnostics/check-debug-flags.sh`
**Diagnostic scripts** (`diagnostics/`) — all support `--help`, `--no-color`/`--color`:
- `ir-dump.sh` — LLVM IR (`--raw`) | `ir-diff.sh` — compare two programs | `disasm-ori.sh` — native disassembly
- `rc-stats.sh` — RC balance per function | `codegen-audit.sh` — static RC/COW/ABI analysis (`--strict`, `--function`)
- `diagnose-aot.sh` — all-in-one: build + run + leak check + RC stats + IR (`--valgrind`, `--verbose`)
- `dual-exec-debug.sh` — interpreter vs AOT comparison; auto-dumps on mismatch (`--verbose`)
- `valgrind-aot.sh [file.ori ...]` — Valgrind memory errors (defaults to `tests/valgrind/`, not in test-all.sh)
- `dual-exec-verify.sh [test-path]` — batch interpreter vs LLVM (`--test-only`, `--main-only`, `--json`)

## Versioning

CalVer — see `docs/ori_lang/versioning.md` | `docs/development/versioning.md` (full details)
**Build**: `v<Y>.<M>.<D>.<N>-<Stage>` (e.g. `v2026.03.01.1-Alpha`) | **Source of truth**: `BUILD_NUMBER` file
**Spec edition**: year-scoped directory `docs/ori_lang/v2026/` — covers all `v2026.*` builds; displayed version injected from `BUILD_NUMBER`
**Scripts**: `./scripts/bump-build.sh` (derive build number) | `./scripts/sync-version.sh` (sync all manifests)

## Key Paths

`compiler/oric/` — compiler | `docs/ori_lang/v2026/spec/` — **spec (authoritative)** | `spec/grammar.ebnf` — syntax | `spec/operator-rules.md` — operator semantics | `docs/ori_lang/proposals/` — proposals | `docs/ori_lang/versioning.md` — versioning scheme | `library/std/` — stdlib | `tests/spec/` — conformance | `compiler/oric/tests/phases/` — phase tests | `compiler/ori_llvm/tests/aot/` — AOT tests | `tests/valgrind/` — Valgrind tests | `tests/benchmarks/` — benchmarks | `diagnostics/` — diagnostic scripts | `plans/roadmap/` — roadmap

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

`.ori` source | Tests in `_test/`: `foo.ori` → `_test/foo.test.ori` | Attached: `@test tests @target () -> void` | Floating: `tests _` | Private: `::` prefix | Every function (except `@main`) requires tests

## Entry Points

`@main () -> void` | `() -> int` | `(args: [str]) -> void` | `(args: [str]) -> int` — `args` excludes program name
`@panic (info: PanicInfo) -> void` — optional handler; `print()` → stderr; first panic wins; re-panic = immediate termination
