- **Under construction** — Rust tooling trusted; Ori tooling (lexer, parser, typeck, eval, test runner) is NOT. Bugs are in the compiler, not user code.

## The One Rule: Correctness Above All

**Every decision you make must optimize for correctness.** The most correct, clean, and proper fix is the ONLY acceptable fix. There are no countervailing concerns:

- **Effort** is irrelevant. If the correct fix touches 10 files across 3 crates, that IS the fix.
- **Time** is irrelevant. A proper fix that takes longer is the only option.
- **Cost** is irrelevant. Complexity in service of correctness is not over-engineering.
- **Scope** is irrelevant. There is no "out of scope" — there is only correct and incorrect.
- **Risk** is irrelevant. "Safer to do the simple thing" is choosing incorrectness.
- **Responsibility** is irrelevant. There is no "not my code" or "different subsystem."
- **Current task** is irrelevant. "I was working on something else" is not a reason to do less.
- **Ownership** is irrelevant. There is no ownership — every file, every crate, every subsystem is yours.
- **Relatedness** is irrelevant. "That's a separate issue" does not exist.

When you see two possible fixes — one simpler and one more correct — the simpler one does not exist. You do not weigh tradeoffs between correctness and convenience. There is no tradeoff. Correctness wins. Always.

**Proper fixes only** — no workarounds, hacks, shortcuts, or temporary fixes. If the correct fix requires architectural change, that IS the work, not a reason to defer. If it requires refactoring a subsystem, that IS the fix, not a future improvement.

## Ownership & Deferral

- **One system, one owner: YOU** — compiler, typeck, eval, codegen, tests, spec, stdlib are one machine. Every bug, every failure, every interference is YOUR problem. You don't get to say "that's a different bug" or "that was already broken" or "my fix didn't cause that." If you found it, touched it, or your fix surfaced it — you own it. Fix every issue encountered. Add discovered issues to todo list.
- **ZERO DEFERRAL on bugs** — when you discover a bug, issue, leak, performance problem, or anything broken: every bug MUST get a concrete tracked artifact immediately. No mental notes, no "I'll remember," no comments-only. The two valid responses are:
  - **Blocking or critical/high:** fix it NOW using `/fix-bug` (creates a fix section file with plan-section rigor: root cause analysis, TDD matrix, completion checklist including TPR + hygiene review). The discovery IS the assignment.
  - **Non-blocking medium/low or unrelated to current task:** file it NOW using `/add-bug` (creates a tracked `- [ ]` entry in the bug tracker with repro, subsystem, severity). Filing via `/add-bug` is NOT deferral — it creates a concrete artifact that `/review-bugs` will triage. Deferral is when a bug has no artifact at all.
  No "tracked for later" (without an artifact), no "known issue" (without filing), no "pre-existing" (as justification for skipping). **Pre-existing bugs MUST be tracked immediately** — "pre-existing" is diagnosis only, never justification for ignoring.
- **Comments are NOT documentation** — a code comment (`//`, `#[ignore = "..."]`, TODO) is non-visible and non-actionable. It does NOT count as documenting an issue. Discovered bugs that cannot be fixed immediately MUST be added to the active plan or roadmap as `- [ ]` checkbox items. A comment alone is NEVER sufficient — comments are invisible to the planning system.
- **Tests that expose bugs = bugs found** — when writing tests (especially matrix tests), a failing test IS a bug discovery. Do NOT "fix the test to work around the bug" or say "this is a separate bug" and continue writing more tests. The moment a test reveals a bug: (1) STOP writing more tests, (2) file the bug via `/add-bug` (creates a tracked entry in the bug tracker) or add a `- [ ]` item to the active plan if the bug is in-scope, (3) THEN decide: if the bug blocks the current task or is critical/high severity, fix it now via `/fix-bug`; otherwise continue testing with the bug tracked. Rewriting a test to avoid a compiler bug without recording the bug is deferral — the test's purpose is to find bugs, and the bugs are the deliverable. "Completing the test matrix" is NOT more important than recording what the matrix found.
- **Proactive bug filing with `/add-bug`** — when you encounter ANY bug not related to your current task, invoke `/add-bug` immediately. Do NOT gloss over it as "not related", note it mentally and move on, or say "separate issue" without filing. If in doubt, file it — verification happens at `/review-bugs` time. A false positive costs nothing; a missed bug costs everything. Triggers: unrelated test failures, suspicious behavior, spec/impl mismatches, wrong error messages, fixable `#skip` reasons, TODO/FIXME describing unfixed bugs.
- **Bug fix rigor with `/fix-bug`** — when fixing ANY bug (whether from the bug tracker, discovered during plan work, or surfaced by TPR), use `/fix-bug BUG-XX-NNN`. This creates a fix section file (`plans/bug-tracker/fix-BUG-XX-NNN.md`) with plan-section rigor: investigation, root cause analysis, TDD matrix (semantic + negative pins), implementation, completion checklist (test-all, TPR, hygiene review). No ad-hoc bug fixes — every bug gets a fix section, even "obvious" ones. The fix section is the permanent record of investigation and verification.
- **NEVER reason out of TPR findings** — when `/tpr-review` or `/review-work` surfaces a finding, the ONLY valid responses are: (1) fix it NOW, or (2) create a concrete implementation plan and execute it. You are NEVER permitted to dismiss findings as "pre-existing", "architectural limitation", "out of scope", "conservative/safe", "not a regression", or "future improvement". Marking a finding as resolved with a scope note or rationalization is DEFERRAL. The size of the fix is irrelevant — if the correct fix requires cross-crate refactoring, that IS the work. If genuinely blocked (need user decision, missing domain knowledge), use `AskUserQuestion` immediately.
- **"Future improvement" MUST be concretely tracked** — NEVER say "tracked as future improvement" without creating a concrete artifact in the same response: a bug-tracker entry (`/add-bug`), plan section `- [ ]` item, or roadmap checkbox. Ask: "When would this get done? Who would find it?" If the answer is nobody/never, fix it now or `AskUserQuestion`. Empty promises are deferral.
- **ALL deferrals MUST have implementation anchors** — a deferred item MUST point to a concrete `- [ ]` checkbox in the section/plan where it WILL be implemented. The only valid deferral reasons are: (1) dependency — blocked by another section's incomplete work, with `<!-- blocked-by:X -->` pointing to a specific item; (2) better location — the work fits naturally in a future section, with a specific `- [ ]` item there. Everything else is not deferral — it is skipping. "Nice-to-have", "existing tests already cover it", "test gap but not blocking", "coverage gap" are ALL banned rationalizations. If the user approved the plan item, it is mandatory. Scope, effort, complexity, difficulty, number of call sites, architectural sprawl are irrelevant — the difficulty IS the assignment.
- **Flaky tests ARE bugs** — if a test passes sometimes and fails sometimes, that is a bug — not noise. Do NOT retry and move on. Research the root cause (race condition, timing dependency, temp file collision, state leakage, non-deterministic ordering, filesystem caching) and fix it so the test is deterministic. File via `/add-bug` if discovered during a different fix.
- **NEVER investigate "pre-existing?"** — do NOT use `git checkout`, `git stash`, `git bisect`, `git log --diff-filter`, or any git archaeology to determine whether a bug or test failure existed before your changes. **It does not matter.** The question "was this pre-existing?" is banned. The only valid question is: "is it fixed?" Spending time checking out old commits to see if something "was already broken" produces zero value. It's broken now → fix it now. The timeline is irrelevant. The fix is everything.
- **When unsure, STOP and ASK** — don't guess or assume
- **Fact-check** against spec. Consult `~/projects/reference_repos/lang_repos/` (Rust, Go, Zig, TS, Gleam, Elm, Roc, Swift, Koka, Lean 4).
- **If you can't do it right, say so** — communicate blockers, don't ship bad code
- **Continuous improvement everywhere** — if you see something wrong or suboptimal — stale docs, missing CLAUDE.md instructions, incomplete memory, unclear scripts, weak tests, imprecise error messages — fix it at the source. Never work around a problem when you can eliminate it. Every interaction should leave the project better than you found it.
- **ALWAYS improve tooling, NEVER work around it** — when debugging, testing, or using diagnostic scripts and you notice ANY deficiency (confusing output, missing coverage, missing flags, manual multi-step workaround, silent failures, wrong results), STOP and fix the tool. The tool improvement IS the work. Banned: piping/grepping script output to find what you need (fix the output format), running 3 commands for one answer (make one script), manually interpreting output (add `--summary`/`--check`), ignoring wrong output (fix the tool), writing one-off scripts (extend permanent tools), saying "the tool doesn't support X" and moving on (add support for X). Scope: `test-all.sh`, `clippy-all.sh`, `diagnostics/*`, `scripts/*`, `llvm-test.sh`, and any project automation. See `/improve-tooling` skill.

## TDD for Bugs

**Use `/fix-bug` for all bug fixes** — it enforces the full workflow below and creates a permanent fix section file. The TDD discipline below is built into `/fix-bug`'s Phase 3 (TDD) and Phase 4 (Implementation).

NEVER fix without tests first:
1. **STOP** — resist urge to immediately change code
2. **Consult spec** (`docs/ori_lang/v2026/spec/`) for intended behavior
3. **Write MATRIX tests** — not just "multiple." Every fix requires:
   - **Exact failing case**: the specific input that triggered the bug
   - **Edge cases**: empty, single-element, boundary conditions
   - **Cross-type coverage**: if the fix is type-dependent, test ALL relevant types through the same code path (e.g., str, [int], Option<str>, closures, structs, maps, sets)
   - **Cross-pattern coverage**: if the fix is pattern-dependent, test ALL relevant control-flow patterns (e.g., full iteration, break, yield, guard, nested, two-call)
   - **Cross-feature coverage**: test interactions with other features that flow through the same code path (e.g., closures, generics, `?`, pattern matching, traits — see `.claude/rules/tests.md` §Interaction Testing)
   - **Semantic pin**: at least one test that ONLY passes with the new semantics — this is the permanent regression guard
   - **Negative pin**: at least one test that REJECTS the old/broken behavior — proves the compiler actively prevents regression
4. **Verify tests fail** — if they pass, you misunderstand the bug
5. **Fix the code** — choose the most correct fix, not the simplest one
6. **Tests pass unchanged** — needing to change tests = wrong tests or wrong fix
7. **Verify matrix completeness** — missing cells in the type × pattern × feature matrix are future regressions
8. **Matrix squeeze principle** — each matrix test narrows the gap between "works" and "crashes," triangulating the bug from multiple angles. When the matrix is dense, the correct fix surface becomes surgically obvious because all surrounding cases are pinned. Regressions from fixes are caught immediately by the existing matrix, forcing the fix to be precise rather than broad. A fix that breaks existing matrix cells was too broad; the matrix reveals exactly where the boundary is.

## Fix Completeness

A fix is NOT done until ALL of these are true:
- Matrix tests cover every type × pattern × feature interaction that flows through the changed code path
- At least one semantic pin test exists that would fail if the fix is reverted
- At least one negative pin rejects the broken behavior
- Positive + negative pairing: every "should work" has a corresponding "should fail"
- Debug AND release builds pass (FastISel behavior differs)
- Interpreter and LLVM produce identical results for all new tests (dual-execution parity)
- `ORI_CHECK_LEAKS=1` reports zero leaks on all test programs (for memory-touching fixes)
- Plan/roadmap updated if the fix crosses section boundaries
- The fix is architecturally correct — not merely functional. A workaround that passes tests is not a fix.
- `/tpr-review` passed — independent third-party review clean
- `/impl-hygiene-review` passed — AFTER TPR is clean (Auto Mode autoscopes across the active work arc; never use `last commit` — it is too narrow for multi-commit work)
- Fix section file (`plans/bug-tracker/fix-BUG-XX-NNN.md`) status updated to `complete`

## Stabilization Discipline

- **Every fix becomes a permanent test** — no fix lands without a test that catches its regression
- **Narrow the front** — complete one fix/section fully before starting another. RC + control-flow + lowering interactions multiply failure surfaces; concurrent changes across these domains compound risk
- **Fix interference = reorder, don't skip** — when fixing Bug A causes Bug B to surface (new failures that weren't in the original test run), this is INTERFERENCE, not a "pre-existing issue to ignore." The correct response is: (1) revert or shelve Bug A's fix, (2) fix Bug B first using `/fix-bug` (it's now a dependency — full plan-section rigor applies), (3) re-apply Bug A's fix on top of Bug B's fix. Do NOT declare Bug A "fixed" when Bug B is interfering — that's shipping a regression. Do NOT waste time on git archaeology (`git checkout`, `git bisect`, `git stash`) to determine if Bug B was "pre-existing" — it does not matter. It's broken now → fix it now.
- **Plan boundaries = implementation boundaries** — if a fix in Section X touches code owned by Section Y, update Section Y's plan before proceeding. No partial fixes absorbed silently across sections.
- **Invariants are explicit** — if correctness depends on a property (RC balanced, scope restored, phantom inserted), it MUST be either a `debug_assert!` or a test. Implicit invariants become invisible regressions.
- **Multi-commit sequences must be ordered by dependency, not by chronology** — when planning N separate commits whose files cross-reference each other (e.g., a tooling improvement + a fix that uses the tooling, or a refactor + a documentation update referencing the new APIs), commit them in **dependency order** so each commit is self-contained AND its working tree is in a consistent post-commit state for the next commit. The lefthook pre-commit hook hides "partially staged files" (status `MM` or `RM` — content edits not in the index) by force-checking-out the index version. If a partially-staged file `A` references an unstaged sibling `B`, lefthook checks out the OLD `A` while leaving the NEW `B` alone, producing a frankenstate where the API `B` calls is missing from the OLD `A`. Symptom: `cargo clippy` fails inside the lefthook hook with `no method named 'foo' found`, while `./full-check.sh` run manually outside the hook passes against the same working tree. Fix: stage all dependent files together (so neither is partial) OR commit in dependency order so the dependent files land first. **Never `--no-verify` to bypass — it just hides the inconsistency.**

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

### AIMS — ARC Intelligent Memory System
AIMS is a **unified semantic framework** — RC placement, reuse, COW, FIP, contracts, and TRMC are facets of one model, not separate features. Every memory decision flows through the unified lattice, contracts, and realization. **Non-negotiable invariants:**
1. Contracts and realization must agree (FipContract::Certified ↔ zero unmatched alloc/dealloc)
2. Active rewrites must be sound (identical observable behavior, behavioral verification required)
3. No pass may rely on stale summaries (pipeline ordering is load-bearing)
4. Every active subsystem must be end-to-end verified (implementation + invariant enforcement + tests)

When fixing any AIMS-related bug: ask "does this preserve system coherence?" A fix in one subsystem that leaves another inconsistent is a new bug, not a fix. See `.claude/rules/arc.md` for full details.

---

## Compiler Coding Guidelines

- **Architecture**: `oric` → `ori_types/eval` → `ori_parse` → `ori_lexer` → `ori_ir/diagnostic` (no upward); IO only in `oric`; no phase bleeding
- **Memory**: Arena + ID (`ExprArena`+`ExprId`); intern identifiers (`Name`); newtypes for IDs; no `Arc` in hot paths; `#[cold]` on error factories
- **Salsa**: derive `Clone, Eq, PartialEq, Hash, Debug`; no `Arc<Mutex<T>>`, fn pointers, `dyn Trait`; deterministic; accumulate errors
- **API**: >3-4 params → config struct; no boolean flags; RAII guards; return iterators not `Vec`
- **Dispatch**: enum for fixed sets; `dyn Trait` only for user-extensible; cost: `&dyn` < `Box<dyn>` < `Arc<dyn>`
- **Diagnostics**: all errors have spans; imperative suggestions; no `panic!` on user errors; accumulate
- **Testing**: verify behavior not implementation; spec-based; multiple angles (happy, edge, error). **Test files**: sibling `tests.rs` (not inline); `#[cfg(test)] mod tests;` declaration only. `foo.rs` → `foo/tests.rs`; `mod.rs` → `bar/tests.rs`; `lib.rs`/`main.rs` → `tests.rs` in same dir
- **Test function naming — behavior, not provenance**: test names follow the `<subject>_<scenario>_<expected>` shape (Arrange-Act-Assert / Given-When-Then / Roy Osherove three-part naming) and are self-explanatory — the name alone must answer *what is tested*, *under what conditions*, *with what outcome*. Examples: `test_iterator_drop_when_break_inside_for_releases_remaining`, `test_cow_slice_with_two_borrows_does_not_double_free`, `test_parse_empty_input_returns_empty_ast`. **NEVER encode ephemeral identifiers in test names** — no plan names, no section/subsection numbers (`section_04_3`, `4_3_2`, `§4.3`), no plan annotations (`TPR_04_005`, `CROSS_04_014`), no bug/issue IDs (`BUG_04_045`, `issue_42`), no dates, no author initials, no commit hashes. Tests are permanent; those artifacts are ephemeral, and encoding them creates names that rot the moment the artifact is renamed, renumbered, merged, or closed. Provenance belongs in `///` (Rust) or `//` (Ori) doc comments above the test (`/// Regression: ...`), never in the function name. **Banned weak descriptors**: `_works`, `_basic`, `_simple`, `_correct`, `_valid`, `_handles_X`, `_check_X`, bare unit names like `test_iterator`. **`/impl-hygiene-review` enforcement**: any test name containing an ephemeral identifier or weak descriptor is a NAMING violation and MUST be renamed in the same review pass — never deferred. The rename is mechanical and local to the test file; there is no scope or risk argument that excuses leaving an ephemeral name in place. Full rules: `.claude/rules/impl-hygiene.md` §Test Function Naming.
- **Performance**: O(n²) → O(n); hash lookups not linear scans; no alloc in hot loops; iterators over indexing
- **ARM portability**: C string pointers in `ori_rt` use `std::ffi::c_char`, never `i8` — `c_char` is `i8` on x86_64 but `u8` on aarch64
- **Style**: no `#[allow(clippy)]` without justification; functions < 100 lines (target < 50); no dead/commented code; `//!`/`///` docs
- **Plan annotations are temporary scaffolding**: Code annotations referencing plans (`TPR-04-005`, `CROSS-04-014`, `§04.3 Phase A`, `Section 04.2`) are allowed during active development — they aid navigation. But they are **ephemeral** and MUST be removed when the plan completes. Every plan MUST include a final cleanup section to strip all its code annotations. Stale annotations from completed plans are hygiene violations. Only **spec references** (`Spec: Clause N.M`) are permanent.
- **File size**: 500 line limit (excl. tests). Stop and split before exceeding. Extract to submodules. `scripts/extract_tests.py` for test extraction.
- **Tracing — USE FIRST**: `ORI_LOG` before `println!`. Levels: `error`/`warn`/`debug`/`trace`. Targets: `ori_types`/`ori_eval`/`ori_llvm`/`oric`. `#[tracing::instrument]` on pub APIs. Never `println!`/`eprintln!`. Setup: `compiler/oric/src/tracing_setup.rs`.
- **Match extraction**: no 20+ arm match in single file; group related arms; 3+ similar → extract helper
- **Continuous improvement**: fix ALL issues in code you touch — dead code, unclear names, duplicated logic. No boundary between "your code" and "other code." If broken, fix; if messy, clean; if drifted, sync.

---

## Commands

**Primary**: `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh`, `./build-all.sh` (includes LLVM)
**Tests**: `cargo t` (Rust, incl. LLVM), `cargo st` (Ori), `cargo st tests/spec/path/` (specific), `./llvm-test.sh`
**MANDATORY TIMEOUT**: NEVER run tests without a timeout. Max 150 seconds (2m30s). Use `timeout 150` prefix for shell commands, `--timeout 150000` for Bash tool calls. If a test hangs past the timeout, you introduced a hanging test — kill it, find the cause, fix it.
**REVIEW/AGENT TIMEOUTS**: Review/analysis tasks (`/tpr-review`, `/tp-help`, `codex exec`, `/review-work`, `/independent-review`, Agent tool tasks) legitimately take 5–35 minutes, far longer than the Bash tool's 2-minute default. NEVER use short timeouts (shell `timeout 60 codex` or Bash `timeout: 60000`) on these — they kill reviews mid-stream. The `.claude/hooks/block-banned-commands.sh` hook enforces this: it blocks any `timeout` under 300000 ms (5 min) or over 2100000 ms (35 min) on codex commands. To run a full-length review, prefer `run_in_background: true` on the Bash tool (no timeout cap) and wait for the completion notification; if you must run foreground, use a Bash `timeout:` in the allowed 5–35 min window. Test commands are a separate rule (see "MANDATORY TIMEOUT" above — 150s max for tests).
**Build**: `cargo c`/`cl`/`b`/`fmt` (all crates incl. LLVM)
**LLVM/AOT**: `cargo b` (debug), `cargo b --release` (release) — LLVM is a default feature; `cargo test -p ori_llvm` (LLVM tests)
**Release LTO**: `cargo build --profile release-lto` — fat LTO, ~20% faster binary, ~3.5x longer build. Output: `target/release-lto/ori`. Regular `--release` unaffected.
**Tracing** (USE FIRST): `ORI_LOG=debug ori check file.ori` | `=ori_types=trace ORI_LOG_TREE=1 ori check f.ori` | `=ori_eval=debug ori run file.ori` | `=oric=debug` (Salsa) | `=ori_arc::aims::realize=trace` (per-phase post-walk RC snapshots — bisect which AIMS pass touched a block) | Falls back to `RUST_LOG`
**Phase dumps**: `ORI_DUMP_AFTER_PARSE=1` (AST) | `ORI_DUMP_AFTER_TYPECK=1` (typed IR) | `ORI_DUMP_AFTER_ARC=1` (ARC IR) | `ORI_DUMP_AFTER_LLVM=1` (LLVM IR, superset of `ORI_DEBUG_LLVM`) | `ORI_EMIT_ARC_DOT=1` (GraphViz DOT) — stderr, zero release overhead
**Runtime debug**: `ORI_TRACE_RC=1` (RC log) | `ORI_RT_DEBUG=1` (assertions) | `ORI_CHECK_LEAKS=1` (leak report)
**Codegen audit**: `ORI_AUDIT_CODEGEN=1` — RC balance, COW sequencing, ABI args, aggregate loads, safety checks. Zero cost off. `ORI_AUDIT_STRICT=1` (pessimistic) | `ORI_AUDIT_FUNCTION=name` (filter)
**AIMS**: The ARC pipeline uses the AIMS unified lattice — no feature flags needed. `diagnostics/aims-compare.sh` for behavioral + RC comparison.
**Always run `./test-all.sh` after compiler changes.**
**Perf baseline**: `./scripts/perf-baseline.sh [--release] [--include-cow]` | **COW benchmarks**: `./scripts/cow-benchmark.sh [--release] [--include-macro] [--compare baseline.json]` | **Consistency**: `diagnostics/check-debug-flags.sh` | **Cargo cache**: `./scripts/cache-doctor.sh [--print-cleanup|--clean]` — detects root-owned files in `target/` that cargo cannot update (accidental `sudo cargo build`); refuses destructive actions by default
**Diagnostic scripts** (`diagnostics/`) — all support `--help`, `--no-color`/`--color`:
- `ir-dump.sh` — LLVM IR (`--raw`, `--optimized`, `--function`) | `arc-dump.sh` — ARC IR post-lowering (`--raw`, `--function`) — debug AIMS pipeline (alias chains, take-projects, lineage) | `ir-diff.sh` — compare two programs | `disasm-ori.sh` — native disassembly
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
