---
paths:
  - "compiler/**/*.rs"
  - "tests/compiler/**"
  - "docs/compiler/**"
---

# Compiler

## Architecture

- **Deps**: `oric` → `ori_llvm` → `ori_arc/ori_repr` → `ori_canon` (depends on `ori_arc` for decision tree primitives) → `ori_types`/`ori_eval`/`ori_patterns` → `ori_parse` → `ori_lexer` → `ori_ir`/`ori_diagnostic`. Note: `ori_eval` depends on `ori_ir`, `ori_registry`, `ori_patterns`, `ori_stack` (not `ori_types`). Support: `ori_compiler` (pure facade — depends on `ori_ir`, `ori_diagnostic`, `ori_lexer`, `ori_parse`, `ori_types`, `ori_canon`, `ori_eval`, `ori_patterns`, `ori_fmt`), `ori_registry`, `ori_stack`, `ori_fmt`, `ori_test_harness`, `ori_rt`
- **IO**: only in `oric`; core crates pure
- **No phase bleeding**: parser != type-check, lexer != parse

### Phase-Specific Purity

- **Lexer**: scanning with minimal local state (nesting depth, mode stack); produces `(tag, len)`. No semantic state (names, types, scopes).
- **Parser**: syntax only; builds AST from tokens; no name resolution or semantic validation
- **Type Checker**: consumes AST, produces typed IR; no re-parsing, no codegen. Salsa queries must be pure.
- **Canonicalizer**: consumes typed IR, produces `CanExpr`; no re-type-checking, no codegen
- **Evaluator**: interprets `CanExpr`; no re-type-checking, no codegen
- **ARC Pass**: lowers `CanExpr` to ARC IR, analyzes ownership; no codegen, no interpretation
- **LLVM Codegen**: emits LLVM IR from realized ARC IR; no interpretation, no re-type-checking
- **Diagnostics**: formats and renders errors; no phase logic, no semantic analysis
- **Optimization Passes**: reads IR, produces transformed IR; analysis is pass-local

## Memory

- Arena + ID (`ExprArena`+`ExprId`), not `Box<Expr>`
- Intern identifiers (`Name`), not `String`
- Newtypes for IDs | no `Arc` cloning in hot paths
- `&'a T` for borrowing, `Arc<T>` only for shared ownership

## Dispatch

- Enum for fixed sets (exhaustiveness, static dispatch)
- `dyn Trait` only for user-extensible
- Cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`

## API

- >3 params → config struct | no boolean flags
- Return iterators, not `Vec` | RAII guards for context

## Salsa

- Query types: `Clone, Eq, PartialEq, Hash, Debug`
- No `Arc<Mutex<T>>`, fn pointers, or `dyn Trait` | deterministic (no random/time/IO)

## Diagnostics

- All errors have spans | accumulate, don't bail
- Imperative: "try using X" | no `panic!` on user errors

## Tracing — ALWAYS USE FOR DEBUGGING

**`ORI_LOG` is your first debugging tool.** Before `println!`, before reading code line-by-line, turn on tracing.

### Environment Variables

- `ORI_LOG`: filter string (`RUST_LOG` syntax), falls back to `RUST_LOG`, default `warn`
- `ORI_LOG_TREE=1`: hierarchical tree output with indentation (`tracing-tree`)
- Setup: `compiler/oric/src/tracing_setup.rs`, initialized in `main.rs`

### Quick Reference

- `ORI_LOG=debug ori check file.ori` — all phases at debug
- `ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check f.ori` — type inference call tree
- `ORI_LOG=ori_eval=debug ori run file.ori` — evaluator method dispatch
- `ORI_LOG=oric=debug ori check file.ori` — Salsa query execution
- `ORI_LOG=ori_types=debug,ori_eval=debug ori run f.ori` — multiple targets

### Phase Dumps

- `ORI_DUMP_AFTER_PARSE=1 ori check file.ori` — AST after parse
- `ORI_DUMP_AFTER_TYPECK=1 ori check file.ori` — typed IR after typeck
- `ORI_DUMP_AFTER_ARC=1 ori build file.ori` — ARC IR with RC strategies
- `ORI_DUMP_AFTER_LLVM=1 ori build file.ori` — annotated LLVM IR

### Runtime Instrumentation (AOT binaries)

- `ORI_TRACE_RC=1 ./binary` — RC event trace (alloc/inc/dec/free)
- `ORI_RT_DEBUG=1 ./binary` — runtime assertions (header validation)
- `ORI_CHECK_LEAKS=1 ./binary` — leak check with attribution

### Tracing Targets (by crate)

| Target | What it shows |
|--------|--------------|
| `oric` | Salsa queries (lexing, parsing, type checking, evaluating), cache hits/misses |
| `ori_types` | Type checking phases, inference, unification, type errors |
| `ori_eval` | Expression evaluation, method dispatch, function calls |
| `ori_llvm` | LLVM codegen, pattern matching, control flow |
| `ori_parse` | Parser (dependency declared, limited instrumentation) |
| `ori_patterns` | Pattern system (dependency declared, limited instrumentation) |

### Levels

- `error`: should never happen — internal invariant violations
- `warn`: recoverable issues
- `debug`: phase boundaries, query execution, function-level events
- `trace`: per-expression, hot paths — very verbose

### Coding Guidelines

- Use `tracing` crate, never `println!`/`eprintln!` for debug output
- `#[tracing::instrument]` on public API entry points | `skip_all` or `skip(arena, engine)` for large/non-Debug args
- Salsa `#[tracked]` functions: manual `tracing::debug!()` events (not `#[instrument]`)

## Verification Test Suites — FROM LLVM VERIFICATION TOOLING PLAN

These test suites were built by `plans/llvm-verification-tooling/` and verify deep compiler properties. Use them when touching ARC, LLVM codegen, or optimization passes.

| Suite | Command | What it verifies |
|-------|---------|-----------------|
| **AIMS Snapshots** | `cargo test -p oric --test aims_snapshots` | Per-pass ARC IR snapshots (22 tests, 6 pass categories). Bless: `ORI_BLESS=1` |
| **FileCheck IR** | `cargo test -p ori_llvm --test codegen_checks` | LLVM IR pattern assertions (44+ tests: RC, COW, ABI, iterators, closures) |
| **Lattice Properties** | `cargo test -p ori_arc -- lattice::prop_tests` | Join laws, partial-order axioms, fixpoint convergence (36 tests) |
| **Contract Oracle** | `cargo test -p ori_arc -- oracle` | Re-derives MemoryContract from realized IR, detects analysis/realization mismatches (8 tests) |
| **Protocol Builtins** | `cargo test -p ori_arc -- builtins::tests` | Protocol builtin ownership matrix (11 consumer tests) |
| **Sanitizer Smoke** | `scripts/sanitizer-smoke.sh` | ASan/UBSan on 17 programs (O0+O2 matrix). Requires Clang. |
| **Alive2 Curated** | `diagnostics/alive2-verify.sh --corpus` | Formal translation validation: 8 pure functions verified via Z3 SMT solver (proves optimization correctness for ALL inputs). Requires `alive-tv` (`scripts/build-alive2.sh`). |
| **Alive2 Full Sweep** | `diagnostics/alive2-verify.sh --all-codegen` | Weekly: all codegen tests through alive-tv with false positive suppression. |

### Shared Test Harness (`ori_test_harness` crate)

The `compiler/ori_test_harness/` crate provides shared infrastructure for snapshot/baseline tests:
- **Directives**: `// CHECK:`, `// CHECK-NOT:`, `// CHECK-LABEL:`, `// CHECK-NEXT:`, `// @revisions:`
- **Bless mode**: `ORI_BLESS=1` writes actual output as new baseline (only `"1"` accepted)
- **Runner**: `run_test_directory(path, strategy, bless) → TestSummary`
- **Test corpus locations**: `compiler/oric/tests/aims-snapshots/` (AIMS), `compiler/ori_llvm/tests/codegen/` (FileCheck), `tests/alive2/` (Alive2 corpus)

## Diagnostic Scripts — USE THESE

**Before reading code line-by-line, run the diagnostic scripts.** See @diagnostic.md §Diagnostic Scripts for the full table with all flags.

## Bug Debugging Workflow

Follow TDD discipline from CLAUDE.md §TDD for Bugs. Then run diagnostics by symptom:
- Wrong output → `dual-exec-debug.sh` | Crash/segfault → `diagnose-aot.sh --valgrind`
- Memory leak → `ORI_CHECK_LEAKS=1` then `rc-stats.sh` (`--block-level` to localize) | RC corruption → `ORI_TRACE_RC=1` then `codegen-audit.sh --strict`
- Type error → `ORI_LOG=ori_types=debug ori check` | Wrong IR → `ir-dump.sh` + `ir-diff.sh`

## Cascading Fixes = Architectural Smell

- Fix at one callsite moves failure to next layer → STOP, diagnose the shared wrong assumption
- Same logical fix at 3+ callsites → missing abstraction or violated boundary contract — fix at boundary, not consumers
- Present 2-3 options to user (boundary fix, abstraction, workaround)

## Narrow the Front — One Fix at a Time

See CLAUDE.md §Stabilization Discipline for the full narrow-the-front principle.

## Style

- Functions < 100 lines (target < 50) | no dead code | no `#[allow(clippy)]` without reason
- `//!`/`///` docs

## Testing

- TDD for bugs: tests first → verify fail → fix → tests pass unchanged
- Tests in sibling `tests.rs` files (not inline): `#[cfg(test)] mod tests;` declaration in source
  - `foo.rs` → `foo/tests.rs` | `mod.rs` in `bar/` → `bar/tests.rs` | `lib.rs`/`main.rs` → `tests.rs` in same dir
- `cargo t` (all) | `cargo st` (spec) | `./test-all.sh` (full)

## Key Patterns

- **TypeChecker (V2)**: InferEngine, Pool, Registries, ModuleChecker
- **Method Dispatch**: builtin-first via `resolve_builtin_method()` → impl lookup via `TraitRegistry::lookup_method()`. `MethodRegistry` is a future thin wrapper for trait lookup only; builtin dispatch currently bypasses it.

## Crates (19 workspace members)

- `oric`: CLI, Salsa orchestration | `ori_compiler`: compiler orchestration facade (pure, no Salsa, no IO — for WASM)
- `ori_ir`: AST, spans, TypeId, DerivedTrait | `ori_lexer_core`: core lexer types/interfaces | `ori_lexer`: tokenization
- `ori_parse`: parser | `ori_types`: type checking (V2 — Pool, InferEngine, registries)
- `ori_eval`: interpreter | `ori_patterns`: pattern system
- `ori_canon`: canonicalization (AST → CanExpr) | `ori_arc`: ARC/AIMS pipeline
- `ori_repr`: representation optimization | `ori_stack`: stack overflow protection
- `ori_llvm`: LLVM backend | `ori_rt`: AOT runtime (C-ABI static library)
- `ori_registry`: builtin type behavior (pure data) | `ori_diagnostic`: error reporting
- `ori_fmt`: Ori source formatter | `ori_test_harness`: test runner orchestration

## Change Locations

- Expression: `ori_parse/grammar/expr/` | `ori_types/infer/expr/` | `ori_eval/interpreter/`
- Type: `ori_ir/type_id.rs` | `ori_types/pool/` | `ori_types/check/`
- Method: `ori_types/registry/methods/` | `ori_eval/interpreter/method_dispatch/`
- Derive: see `ir.md` §DerivedTrait for canonical sync point list

## Graph-first, manual second

Before opening any path in `~/projects/reference_repos/lang_repos/` by hand,
query the intelligence graph:

- `scripts/intel-query.sh --human similar "<symbol>" --repo rust,swift,zig,lean4 --limit 5`
  — semantic equivalents across large-compiler architectures (crate boundaries,
  Salsa-style incremental, phase ordering)
- `scripts/intel-query.sh --human callers "<symbol>" --repo ori` — blast radius
  across all 19 workspace crates (see §Crates above)
- `scripts/intel-query.sh --human file-symbols "<crate-name>" --repo ori` — the
  symbol surface of a single crate before refactoring across its boundary
- `scripts/intel-query.sh --human callees "<entry-point>" --repo ori` — the
  downstream dependency tree for a function (useful when tracing `oric` →
  `ori_types/eval` → `ori_parse` → `ori_lexer` → `ori_ir/diagnostic` flow)

Compiler-architecture work is cross-crate by nature — no single subsystem
preset covers it; use the bare `callers`/`file-symbols` form scoped to the
relevant crate(s). The graph covers Ori plus 10 reference compilers, synced on every commit. Manual
reference-repo reading stays authoritative — but only AFTER the graph
narrows the search. Never cite a graph result without verifying against the
actual source. See `.claude/rules/intelligence.md` for the canonical when-to-query workflow and subcommand reference and `.claude/skills/query-intel/compose-intel-summary.md` for the
canonical query protocol used by review-family skills.

## Source of Truth

1. `docs/ori_lang/v2026/spec/` — authoritative
2. `~/projects/reference_repos/lang_repos/` — Rust, Go, TS, Zig, Gleam, Elm, Roc, Swift, Koka, Lean 4
