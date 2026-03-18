- **Under construction** — Rust tooling trusted; Ori tooling (lexer, parser, typeck, eval, test runner) is NOT. Bugs are in the compiler, not user code.
- **One system** — compiler, typeck, eval, codegen, tests, spec, stdlib are one machine. No "unrelated", "pre-existing", or "out of scope." Fix every issue encountered. Add discovered issues to todo list.
- **ZERO DEFERRAL on bugs** — when you discover a bug, issue, leak, performance problem, or anything broken: (1) IMMEDIATELY add it to your todo list, (2) fix it NOW in this session. No "tracked for later", no "known issue", no "pre-existing", no "separate fix needed". The discovery IS the assignment. There are zero valid reasons to defer — not scope, not complexity, not "it was already broken". If you found it, you fix it. **Pre-existing bugs MUST be fixed immediately** — "pre-existing" is diagnosis only, never justification for skipping.
- **Comments are NOT documentation** — a code comment (`//`, `#[ignore = "..."]`, TODO) is non-visible and non-actionable. It does NOT count as documenting an issue. Discovered bugs that cannot be fixed immediately MUST be added to the active plan or roadmap as `- [ ]` checkbox items. A comment alone is NEVER sufficient — comments are invisible to the planning system.
- **Proper fixes only** — no workarounds, hacks, shortcuts, or temporary fixes. Correct architecture over quick hacks.
- **When unsure, STOP and ASK** — don't guess or assume
- **Fact-check** against spec. Consult `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4).
- **If you can't do it right, say so** — communicate blockers, don't ship bad code
- **Continuous improvement everywhere** — if you see something wrong or suboptimal — stale docs, missing CLAUDE.md instructions, incomplete memory, unclear scripts, weak tests, imprecise error messages — fix it at the source. Never work around a problem when you can eliminate it. Every interaction should leave the project better than you found it.

**TDD for bugs** — NEVER fix without tests first:
1. **STOP** — resist urge to immediately change code
2. **Consult spec** (`docs/ori_lang/v2026/spec/`) for intended behavior
3. **Write MATRIX tests** — not just "multiple." Every fix requires:
   - **Exact failing case**: the specific input that triggered the bug
   - **Edge cases**: empty, single-element, boundary conditions
   - **Cross-type coverage**: if the fix is type-dependent, test ALL relevant types through the same code path (e.g., str, [int], Option<str>, closures, structs, maps, sets)
   - **Cross-pattern coverage**: if the fix is pattern-dependent, test ALL relevant control-flow patterns (e.g., full iteration, break, yield, guard, nested, two-call)
   - **Semantic pin**: at least one test that ONLY passes with the new semantics — this is the permanent regression guard
4. **Verify tests fail** — if they pass, you misunderstand the bug
5. **Fix the code**
6. **Tests pass unchanged** — needing to change tests = wrong tests or wrong fix
7. **Verify matrix completeness** — missing cells in the type x pattern matrix are future regressions

**Fix completeness** — a fix is NOT done until ALL of these are true:
- Matrix tests cover every type and pattern that flows through the changed code path
- At least one semantic pin test exists that would fail if the fix is reverted
- Debug AND release builds pass (FastISel behavior differs)
- Plan/roadmap updated if the fix crosses section boundaries

**Stabilization discipline:**
- **Every fix becomes a permanent test** — no fix lands without a test that catches its regression
- **Narrow the front** — complete one fix/section fully before starting another. RC + control-flow + lowering interactions multiply failure surfaces; concurrent changes across these domains compound risk
- **Plan boundaries = implementation boundaries** — if a fix in Section X touches code owned by Section Y, update Section Y's plan before proceeding. No partial fixes absorbed silently across sections.
- **Invariants are explicit** — if correctness depends on a property (RC balanced, scope restored, phantom inserted), it MUST be either a `debug_assert!` or a test. Implicit invariants become invisible regressions.

---

## Ori Language

- **Ori**: statically-typed expression-based, HM inference, ARC memory, capability effects, smart testing. Targets LLVM/WASM. Compiler in Rust (Salsa-based).
- **NO `return`**: last expression = block value. Exit via `?`/`break`/`panic`. Similar to Rust, Gleam, Roc.
- **Syntax ref**: `.claude/rules/ori-syntax.md` (auto-loaded for `.ori` files) | `/ori-syntax` skill
- **Spec authoritative**: `docs/ori_lang/v2026/spec/` (`grammar.ebnf`, `operator-rules.md`)

### Design Pillars
1. **Expression-based**: everything is expression; last expr = block value; no `return`
2. **Smart verification**: configurable test enforcement (`--test-enforcement=off|warn|error`, default `off`); contracts (`pre()`/`post()`)
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
**Tests**: `cargo t` (Rust, incl. LLVM), `cargo st` (Ori), `cargo st tests/spec/path/` (specific), `./llvm-test.sh`
**MANDATORY TIMEOUT**: NEVER run tests without a timeout. Max 150 seconds (2m30s). Use `timeout 150` prefix for shell commands, `--timeout 150000` for Bash tool calls. If a test hangs past the timeout, you introduced a hanging test — kill it, find the cause, fix it.
**Build**: `cargo c`/`cl`/`b`/`fmt` (all crates incl. LLVM)
**LLVM/AOT**: `cargo b` (debug), `cargo b --release` (release) — LLVM is a default feature; `cargo test -p ori_llvm` (LLVM tests)
**Release LTO**: `cargo build --profile release-lto` — fat LTO, ~20% faster binary, ~3.5x longer build. Output: `target/release-lto/ori`. Regular `--release` unaffected.
**Tracing** (USE FIRST): `ORI_LOG=debug ori check file.ori` | `=ori_types=trace ORI_LOG_TREE=1 ori check f.ori` | `=ori_eval=debug ori run file.ori` | `=oric=debug` (Salsa) | Falls back to `RUST_LOG`
**Phase dumps**: `ORI_DUMP_AFTER_PARSE=1` (AST) | `ORI_DUMP_AFTER_TYPECK=1` (typed IR) | `ORI_DUMP_AFTER_ARC=1` (ARC IR) | `ORI_DUMP_AFTER_LLVM=1` (LLVM IR, superset of `ORI_DEBUG_LLVM`) | `ORI_EMIT_ARC_DOT=1` (GraphViz DOT) — stderr, zero release overhead
**Runtime debug**: `ORI_TRACE_RC=1` (RC log) | `ORI_RT_DEBUG=1` (assertions) | `ORI_CHECK_LEAKS=1` (leak report)
**Codegen audit**: `ORI_AUDIT_CODEGEN=1` — RC balance, COW sequencing, ABI args, aggregate loads, safety checks. Zero cost off. `ORI_AUDIT_STRICT=1` (pessimistic) | `ORI_AUDIT_FUNCTION=name` (filter)
**AIMS**: The ARC pipeline uses the AIMS unified lattice — no feature flags needed. `diagnostics/aims-compare.sh` for behavioral + RC comparison.
**Always run `./test-all.sh` after compiler changes.**
**Perf baseline**: `./scripts/perf-baseline.sh [--release] [--include-cow]` | **COW benchmarks**: `./scripts/cow-benchmark.sh [--release] [--include-macro] [--compare baseline.json]` | **Consistency**: `diagnostics/check-debug-flags.sh`
**Diagnostic scripts** (`diagnostics/`) — all support `--help`, `--no-color`/`--color`:
- `ir-dump.sh` — LLVM IR (`--raw`) | `ir-diff.sh` — compare two programs | `disasm-ori.sh` — native disassembly
- `rc-stats.sh` — RC balance per function | `codegen-audit.sh` — static RC/COW/ABI analysis (`--strict`, `--function`)
- `diagnose-aot.sh` — all-in-one: build + run + leak check + RC stats + IR (`--valgrind`, `--verbose`)
- `dual-exec-debug.sh` — interpreter vs AOT comparison; auto-dumps on mismatch (`--verbose`)
- `valgrind-aot.sh [file.ori ...]` — Valgrind memory errors (defaults to `tests/valgrind/`, not in test-all.sh)
- `dual-exec-verify.sh [test-path]` — batch interpreter vs LLVM (`--test-only`, `--main-only`, `--json`)

## Feature Flags

| Flag | Crate | Effect |
|------|-------|--------|
| `cache` | `ori_arc` | Enables serde/bincode serialization for incremental compilation cache. |

## Versioning

CalVer — see `docs/ori_lang/versioning.md` | `docs/development/versioning.md` (full details)
**Build**: `v<Y>.<M>.<D>.<N>-<Stage>` (e.g. `v2026.03.01.1-Alpha`) | **Source of truth**: `BUILD_NUMBER` file
**Spec edition**: year-scoped directory `docs/ori_lang/v2026/` — covers all `v2026.*` builds; displayed version injected from `BUILD_NUMBER`
**Scripts**: `./scripts/bump-build.sh` (derive build number) | `./scripts/sync-version.sh` (sync all manifests)

## Key Paths

`compiler/oric/` — compiler | `docs/ori_lang/v2026/spec/` — **spec (authoritative)** | `spec/grammar.ebnf` — syntax | `spec/operator-rules.md` — operator semantics | `docs/ori_lang/proposals/` — proposals | `docs/ori_lang/versioning.md` — versioning scheme | `library/std/` — stdlib | `tests/spec/` — conformance | `tests/spec/collections/cow/` — COW spec tests | `compiler/oric/tests/phases/` — phase tests | `compiler/ori_llvm/tests/aot/` — AOT tests | `tests/valgrind/` — Valgrind tests | `tests/valgrind/cow/` — COW Valgrind tests | `tests/benchmarks/` — benchmarks | `tests/benchmarks/cow/` — COW benchmarks (+ `baseline.json`) | `diagnostics/` — diagnostic scripts | `plans/roadmap/` — roadmap

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

`.ori` source | Tests in `_test/`: `foo.ori` → `_test/foo.test.ori` | Attached: `@test tests @target () -> void` | Floating: `tests _` | Private: `::` prefix | Test enforcement configurable via `--test-enforcement=off|warn|error` (default: `off`)

## Entry Points

`@main () -> void` | `() -> int` | `(args: [str]) -> void` | `(args: [str]) -> int` — `args` excludes program name
`@panic (info: PanicInfo) -> void` — optional handler; `print()` → stderr; first panic wins; re-panic = immediate termination
