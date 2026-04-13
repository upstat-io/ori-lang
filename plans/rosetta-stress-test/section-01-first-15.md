---
section: "01"
title: "Infrastructure + First 15 Programs"
status: in-progress
reviewed: true
goal: "Methodically implement/evaluate 15 Rosetta programs — each running every diagnostic tool, verification flag, and benchmark in the full pipeline."
success_criteria:
  - "rosetta-manifest.json describes all 15 programs with status, findings, perf data"
  - "Each task folder has task.md, source.ori with @main, _test/ with assertions"
  - "All 15 programs pass every step of the per-program pipeline (Phases A-J)"
  - "Every bug discovered filed or fixed"
  - "Every language/syntax gap documented with roadmap cross-reference"
  - "Performance baselines captured for all 15 programs"
  - "/tpr-review passed for every subsection"
inspired_by:
  - "Rosetta Code (deterministic, well-defined problems)"
  - "Rust compiletest (per-test expected outcomes)"
  - "Swift SIL tests (positive + negative ARC pairing)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.PRE"
    title: "Infrastructure"
    status: complete
  - id: "01.1"
    title: "001_100_doors"
    status: not-started
  - id: "01.2"
    title: "002_100_prisoners"
    status: not-started
  - id: "01.3"
    title: "003_15_puzzle_game"
    status: not-started
  - id: "01.4"
    title: "004_15_puzzle_solver"
    status: not-started
  - id: "01.5"
    title: "005_2048"
    status: not-started
  - id: "01.6"
    title: "006_21_game"
    status: not-started
  - id: "01.7"
    title: "007_24_game"
    status: not-started
  - id: "01.8"
    title: "008_24_game_Solve"
    status: not-started
  - id: "01.9"
    title: "009_4_rings_or_4_squares_puzzle"
    status: not-started
  - id: "01.10"
    title: "010_9_billion_names_of_God_the_integer"
    status: not-started
  - id: "01.11"
    title: "011_99_bottles_of_beer"
    status: not-started
  - id: "01.12"
    title: "012_A_B"
    status: not-started
  - id: "01.13"
    title: "013_Abbreviations_automatic"
    status: not-started
  - id: "01.14"
    title: "014_Abbreviations_easy"
    status: not-started
  - id: "01.15"
    title: "015_Abbreviations_simple"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Infrastructure + First 15 Programs

**Status:** Not Started
**Goal:** Work through 15 Rosetta programs — each running every single diagnostic tool, verification flag, and benchmark.

**Program selection (ordered by `_tasks/` index):**

| # | Index | Program | Task File | Current State |
|---|-------|---------|-----------|--------------|
| 1 | #001 | 001_100_doors | `_tasks/001_100_doors.md` | Has `@main`, has `_test/`, has `task.md` |
| 2 | #002 | 002_100_prisoners | `_tasks/002_100_prisoners.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 3 | #003 | 003_15_puzzle_game | `_tasks/003_15_puzzle_game.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 4 | #004 | 004_15_puzzle_solver | `_tasks/004_15_puzzle_solver.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 5 | #005 | 005_2048 | `_tasks/005_2048.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 6 | #006 | 006_21_game | `_tasks/006_21_game.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 7 | #007 | 007_24_game | `_tasks/007_24_game.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 8 | #008 | 008_24_game_Solve | `_tasks/008_24_game_Solve.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 9 | #009 | 009_4_rings_or_4_squares_puzzle | `_tasks/009_4_rings_or_4_squares_puzzle.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 10 | #010 | 010_9_billion_names_of_God_the_integer | `_tasks/010_9_billion_names_of_God_the_integer.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 11 | #011 | 011_99_bottles_of_beer | `_tasks/011_99_bottles_of_beer.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 12 | #012 | 012_A_B | `_tasks/012_A_B.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 13 | #013 | 013_Abbreviations_automatic | `_tasks/013_Abbreviations_automatic.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 14 | #014 | 014_Abbreviations_easy | `_tasks/014_Abbreviations_easy.md` | Folder exists but no `.ori` source yet, has `task.md` |
| 15 | #015 | 015_Abbreviations_simple | `_tasks/015_Abbreviations_simple.md` | Folder exists but no `.ori` source yet, has `task.md` |

---

## 01.PRE Infrastructure

- [x] Create `tests/run-pass/rosetta/rosetta-manifest.json` with per-program schema (status, features, has_main, has_tests, aot_eligible, bugs_filed, language_findings, perf)
- [x] Update `tests/run-pass/rosetta/README.md` documenting the per-program pipeline, manifest, and folder structure

- [x] **Subsection close-out (01.PRE)**:
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] **Run `/improve-tooling` retrospectively on THIS subsection** — Retrospective 01.PRE: no tooling gaps (subsection was pure infrastructure — JSON manifest + README, no compiler/diagnostic usage)
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` — clean

---
## 01.1 001_100_doors

**#001 — 100 doors** | **Task file:** `_tasks/001_100_doors.md` | **Current state:** Has `@main`, has `_test/`, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/001_100_doors/` if it does not exist: `mkdir -p tests/run-pass/rosetta/001_100_doors/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/001_100_doors.md tests/run-pass/rosetta/001_100_doors/task.md`
- [ ] Read `tests/run-pass/rosetta/001_100_doors/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/001_100_doors/_test/001_100_doors.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/001_100_doors/_test/001_100_doors.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /tmp/rosetta_001_100_doors_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /tmp/rosetta_001_100_doors_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_001_100_doors_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_001_100_doors_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_001_100_doors_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_001_100_doors_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_001_100_doors_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_001_100_doors_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_001_100_doors_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_001_100_doors_debug /tmp/rosetta_001_100_doors_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_001_100_doors_stripped /tmp/rosetta_001_100_doors_release && ls -la /tmp/rosetta_001_100_doors_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/001_100_doors/001_100_doors.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_001_100_doors_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_001_100_doors_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /tmp/rosetta_001_100_doors_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/001_100_doors/001_100_doors.ori -o /tmp/rosetta_001_100_doors_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "100 doors <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 001_100_doors ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.1 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.1)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.1 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.2 002_100_prisoners

**#002 — 100 prisoners** | **Task file:** `_tasks/002_100_prisoners.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/002_100_prisoners/` if it does not exist: `mkdir -p tests/run-pass/rosetta/002_100_prisoners/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/002_100_prisoners.md tests/run-pass/rosetta/002_100_prisoners/task.md`
- [ ] Read `tests/run-pass/rosetta/002_100_prisoners/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/002_100_prisoners/_test/002_100_prisoners.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/002_100_prisoners/_test/002_100_prisoners.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /tmp/rosetta_002_100_prisoners_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /tmp/rosetta_002_100_prisoners_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_002_100_prisoners_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_002_100_prisoners_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_002_100_prisoners_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_002_100_prisoners_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_002_100_prisoners_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_002_100_prisoners_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_002_100_prisoners_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_002_100_prisoners_debug /tmp/rosetta_002_100_prisoners_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_002_100_prisoners_stripped /tmp/rosetta_002_100_prisoners_release && ls -la /tmp/rosetta_002_100_prisoners_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_002_100_prisoners_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_002_100_prisoners_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /tmp/rosetta_002_100_prisoners_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/002_100_prisoners/002_100_prisoners.ori -o /tmp/rosetta_002_100_prisoners_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "100 prisoners <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 002_100_prisoners ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.2 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.2)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.2 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.3 003_15_puzzle_game

**#003 — 15 puzzle game** | **Task file:** `_tasks/003_15_puzzle_game.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/003_15_puzzle_game/` if it does not exist: `mkdir -p tests/run-pass/rosetta/003_15_puzzle_game/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/003_15_puzzle_game.md tests/run-pass/rosetta/003_15_puzzle_game/task.md`
- [ ] Read `tests/run-pass/rosetta/003_15_puzzle_game/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/003_15_puzzle_game/_test/003_15_puzzle_game.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/003_15_puzzle_game/_test/003_15_puzzle_game.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /tmp/rosetta_003_15_puzzle_game_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /tmp/rosetta_003_15_puzzle_game_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_003_15_puzzle_game_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_003_15_puzzle_game_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_003_15_puzzle_game_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_003_15_puzzle_game_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_003_15_puzzle_game_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_003_15_puzzle_game_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_003_15_puzzle_game_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_003_15_puzzle_game_debug /tmp/rosetta_003_15_puzzle_game_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_003_15_puzzle_game_stripped /tmp/rosetta_003_15_puzzle_game_release && ls -la /tmp/rosetta_003_15_puzzle_game_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_003_15_puzzle_game_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_003_15_puzzle_game_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /tmp/rosetta_003_15_puzzle_game_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/003_15_puzzle_game/003_15_puzzle_game.ori -o /tmp/rosetta_003_15_puzzle_game_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "15 puzzle game <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 003_15_puzzle_game ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.3 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.3)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.3 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.4 004_15_puzzle_solver

**#004 — 15 puzzle solver** | **Task file:** `_tasks/004_15_puzzle_solver.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/004_15_puzzle_solver/` if it does not exist: `mkdir -p tests/run-pass/rosetta/004_15_puzzle_solver/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/004_15_puzzle_solver.md tests/run-pass/rosetta/004_15_puzzle_solver/task.md`
- [ ] Read `tests/run-pass/rosetta/004_15_puzzle_solver/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/004_15_puzzle_solver/_test/004_15_puzzle_solver.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/004_15_puzzle_solver/_test/004_15_puzzle_solver.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /tmp/rosetta_004_15_puzzle_solver_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /tmp/rosetta_004_15_puzzle_solver_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_004_15_puzzle_solver_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_004_15_puzzle_solver_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_004_15_puzzle_solver_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_004_15_puzzle_solver_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_004_15_puzzle_solver_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_004_15_puzzle_solver_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_004_15_puzzle_solver_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_004_15_puzzle_solver_debug /tmp/rosetta_004_15_puzzle_solver_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_004_15_puzzle_solver_stripped /tmp/rosetta_004_15_puzzle_solver_release && ls -la /tmp/rosetta_004_15_puzzle_solver_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_004_15_puzzle_solver_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_004_15_puzzle_solver_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /tmp/rosetta_004_15_puzzle_solver_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/004_15_puzzle_solver/004_15_puzzle_solver.ori -o /tmp/rosetta_004_15_puzzle_solver_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "15 puzzle solver <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 004_15_puzzle_solver ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.4 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.4)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.4 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.5 005_2048

**#005 — 2048** | **Task file:** `_tasks/005_2048.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/005_2048/` if it does not exist: `mkdir -p tests/run-pass/rosetta/005_2048/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/005_2048.md tests/run-pass/rosetta/005_2048/task.md`
- [ ] Read `tests/run-pass/rosetta/005_2048/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/005_2048/005_2048.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/005_2048/_test/005_2048.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/005_2048/005_2048.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/005_2048/005_2048.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/005_2048/005_2048.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/005_2048/_test/005_2048.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /tmp/rosetta_005_2048_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/005_2048/005_2048.ori -o /tmp/rosetta_005_2048_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_005_2048_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_005_2048_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_005_2048_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_005_2048_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_005_2048_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/005_2048/005_2048.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/005_2048/005_2048.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/005_2048/005_2048.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_005_2048_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_005_2048_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_005_2048_debug /tmp/rosetta_005_2048_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_005_2048_stripped /tmp/rosetta_005_2048_release && ls -la /tmp/rosetta_005_2048_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/005_2048/005_2048.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_005_2048_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_005_2048_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/005_2048/005_2048.ori -o /tmp/rosetta_005_2048_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/005_2048/005_2048.ori -o /tmp/rosetta_005_2048_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "2048 <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 005_2048 ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.5 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.5)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.5 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.6 006_21_game

**#006 — 21 game** | **Task file:** `_tasks/006_21_game.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/006_21_game/` if it does not exist: `mkdir -p tests/run-pass/rosetta/006_21_game/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/006_21_game.md tests/run-pass/rosetta/006_21_game/task.md`
- [ ] Read `tests/run-pass/rosetta/006_21_game/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/006_21_game/006_21_game.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/006_21_game/_test/006_21_game.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/006_21_game/_test/006_21_game.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /tmp/rosetta_006_21_game_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /tmp/rosetta_006_21_game_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_006_21_game_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_006_21_game_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_006_21_game_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_006_21_game_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_006_21_game_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/006_21_game/006_21_game.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_006_21_game_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_006_21_game_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_006_21_game_debug /tmp/rosetta_006_21_game_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_006_21_game_stripped /tmp/rosetta_006_21_game_release && ls -la /tmp/rosetta_006_21_game_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/006_21_game/006_21_game.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_006_21_game_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_006_21_game_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /tmp/rosetta_006_21_game_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/006_21_game/006_21_game.ori -o /tmp/rosetta_006_21_game_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "21 game <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 006_21_game ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.6 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.6)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.6 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.7 007_24_game

**#007 — 24 game** | **Task file:** `_tasks/007_24_game.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/007_24_game/` if it does not exist: `mkdir -p tests/run-pass/rosetta/007_24_game/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/007_24_game.md tests/run-pass/rosetta/007_24_game/task.md`
- [ ] Read `tests/run-pass/rosetta/007_24_game/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/007_24_game/007_24_game.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/007_24_game/_test/007_24_game.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/007_24_game/_test/007_24_game.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /tmp/rosetta_007_24_game_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /tmp/rosetta_007_24_game_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_007_24_game_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_007_24_game_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_007_24_game_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_007_24_game_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_007_24_game_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/007_24_game/007_24_game.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_007_24_game_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_007_24_game_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_007_24_game_debug /tmp/rosetta_007_24_game_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_007_24_game_stripped /tmp/rosetta_007_24_game_release && ls -la /tmp/rosetta_007_24_game_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/007_24_game/007_24_game.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_007_24_game_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_007_24_game_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /tmp/rosetta_007_24_game_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/007_24_game/007_24_game.ori -o /tmp/rosetta_007_24_game_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "24 game <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 007_24_game ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.7 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.7)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.7 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.8 008_24_game_Solve

**#008 — 24 game Solve** | **Task file:** `_tasks/008_24_game_Solve.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/008_24_game_Solve/` if it does not exist: `mkdir -p tests/run-pass/rosetta/008_24_game_Solve/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/008_24_game_Solve.md tests/run-pass/rosetta/008_24_game_Solve/task.md`
- [ ] Read `tests/run-pass/rosetta/008_24_game_Solve/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/008_24_game_Solve/_test/008_24_game_Solve.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/008_24_game_Solve/_test/008_24_game_Solve.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /tmp/rosetta_008_24_game_Solve_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /tmp/rosetta_008_24_game_Solve_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_008_24_game_Solve_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_008_24_game_Solve_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_008_24_game_Solve_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_008_24_game_Solve_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_008_24_game_Solve_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_008_24_game_Solve_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_008_24_game_Solve_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_008_24_game_Solve_debug /tmp/rosetta_008_24_game_Solve_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_008_24_game_Solve_stripped /tmp/rosetta_008_24_game_Solve_release && ls -la /tmp/rosetta_008_24_game_Solve_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_008_24_game_Solve_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_008_24_game_Solve_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /tmp/rosetta_008_24_game_Solve_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/008_24_game_Solve/008_24_game_Solve.ori -o /tmp/rosetta_008_24_game_Solve_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "24 game Solve <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 008_24_game_Solve ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.8 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.8)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.8 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.9 009_4_rings_or_4_squares_puzzle

**#009 — 4 rings or 4 squares puzzle** | **Task file:** `_tasks/009_4_rings_or_4_squares_puzzle.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/` if it does not exist: `mkdir -p tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/009_4_rings_or_4_squares_puzzle.md tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/task.md`
- [ ] Read `tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/_test/009_4_rings_or_4_squares_puzzle.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/_test/009_4_rings_or_4_squares_puzzle.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /tmp/rosetta_009_4_rings_or_4_squares_puzzle_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_009_4_rings_or_4_squares_puzzle_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug /tmp/rosetta_009_4_rings_or_4_squares_puzzle_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_009_4_rings_or_4_squares_puzzle_stripped /tmp/rosetta_009_4_rings_or_4_squares_puzzle_release && ls -la /tmp/rosetta_009_4_rings_or_4_squares_puzzle_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_009_4_rings_or_4_squares_puzzle_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /tmp/rosetta_009_4_rings_or_4_squares_puzzle_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/009_4_rings_or_4_squares_puzzle/009_4_rings_or_4_squares_puzzle.ori -o /tmp/rosetta_009_4_rings_or_4_squares_puzzle_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "4 rings or 4 squares puzzle <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 009_4_rings_or_4_squares_puzzle ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.9 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.9)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.9 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.10 010_9_billion_names_of_God_the_integer

**#010 — 9 billion names of God the integer** | **Task file:** `_tasks/010_9_billion_names_of_God_the_integer.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/` if it does not exist: `mkdir -p tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/010_9_billion_names_of_God_the_integer.md tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/task.md`
- [ ] Read `tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/_test/010_9_billion_names_of_God_the_integer.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/_test/010_9_billion_names_of_God_the_integer.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /tmp/rosetta_010_9_billion_names_of_God_the_integer_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_010_9_billion_names_of_God_the_integer_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_010_9_billion_names_of_God_the_integer_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug /tmp/rosetta_010_9_billion_names_of_God_the_integer_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_010_9_billion_names_of_God_the_integer_stripped /tmp/rosetta_010_9_billion_names_of_God_the_integer_release && ls -la /tmp/rosetta_010_9_billion_names_of_God_the_integer_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_010_9_billion_names_of_God_the_integer_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /tmp/rosetta_010_9_billion_names_of_God_the_integer_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/010_9_billion_names_of_God_the_integer/010_9_billion_names_of_God_the_integer.ori -o /tmp/rosetta_010_9_billion_names_of_God_the_integer_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "9 billion names of God the integer <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 010_9_billion_names_of_God_the_integer ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.10 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.10)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.10 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.11 011_99_bottles_of_beer

**#011 — 99 bottles of beer** | **Task file:** `_tasks/011_99_bottles_of_beer.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/011_99_bottles_of_beer/` if it does not exist: `mkdir -p tests/run-pass/rosetta/011_99_bottles_of_beer/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/011_99_bottles_of_beer.md tests/run-pass/rosetta/011_99_bottles_of_beer/task.md`
- [ ] Read `tests/run-pass/rosetta/011_99_bottles_of_beer/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/011_99_bottles_of_beer/_test/011_99_bottles_of_beer.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/011_99_bottles_of_beer/_test/011_99_bottles_of_beer.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /tmp/rosetta_011_99_bottles_of_beer_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /tmp/rosetta_011_99_bottles_of_beer_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_011_99_bottles_of_beer_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_011_99_bottles_of_beer_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_011_99_bottles_of_beer_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_011_99_bottles_of_beer_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_011_99_bottles_of_beer_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_011_99_bottles_of_beer_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_011_99_bottles_of_beer_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_011_99_bottles_of_beer_debug /tmp/rosetta_011_99_bottles_of_beer_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_011_99_bottles_of_beer_stripped /tmp/rosetta_011_99_bottles_of_beer_release && ls -la /tmp/rosetta_011_99_bottles_of_beer_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_011_99_bottles_of_beer_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_011_99_bottles_of_beer_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /tmp/rosetta_011_99_bottles_of_beer_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/011_99_bottles_of_beer/011_99_bottles_of_beer.ori -o /tmp/rosetta_011_99_bottles_of_beer_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "99 bottles of beer <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 011_99_bottles_of_beer ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.11 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.11)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.11 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.12 012_A_B

**#012 — A B** | **Task file:** `_tasks/012_A_B.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/012_A_B/` if it does not exist: `mkdir -p tests/run-pass/rosetta/012_A_B/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/012_A_B.md tests/run-pass/rosetta/012_A_B/task.md`
- [ ] Read `tests/run-pass/rosetta/012_A_B/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/012_A_B/012_A_B.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/012_A_B/_test/012_A_B.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/012_A_B/_test/012_A_B.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /tmp/rosetta_012_A_B_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /tmp/rosetta_012_A_B_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_012_A_B_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_012_A_B_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_012_A_B_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_012_A_B_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_012_A_B_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/012_A_B/012_A_B.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_012_A_B_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_012_A_B_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_012_A_B_debug /tmp/rosetta_012_A_B_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_012_A_B_stripped /tmp/rosetta_012_A_B_release && ls -la /tmp/rosetta_012_A_B_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/012_A_B/012_A_B.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_012_A_B_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_012_A_B_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /tmp/rosetta_012_A_B_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/012_A_B/012_A_B.ori -o /tmp/rosetta_012_A_B_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "A B <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 012_A_B ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.12 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.12)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.12 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.13 013_Abbreviations_automatic

**#013 — Abbreviations automatic** | **Task file:** `_tasks/013_Abbreviations_automatic.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/013_Abbreviations_automatic/` if it does not exist: `mkdir -p tests/run-pass/rosetta/013_Abbreviations_automatic/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/013_Abbreviations_automatic.md tests/run-pass/rosetta/013_Abbreviations_automatic/task.md`
- [ ] Read `tests/run-pass/rosetta/013_Abbreviations_automatic/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/013_Abbreviations_automatic/_test/013_Abbreviations_automatic.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/013_Abbreviations_automatic/_test/013_Abbreviations_automatic.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /tmp/rosetta_013_Abbreviations_automatic_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /tmp/rosetta_013_Abbreviations_automatic_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_013_Abbreviations_automatic_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_013_Abbreviations_automatic_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_013_Abbreviations_automatic_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_013_Abbreviations_automatic_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_013_Abbreviations_automatic_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_013_Abbreviations_automatic_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_013_Abbreviations_automatic_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_013_Abbreviations_automatic_debug /tmp/rosetta_013_Abbreviations_automatic_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_013_Abbreviations_automatic_stripped /tmp/rosetta_013_Abbreviations_automatic_release && ls -la /tmp/rosetta_013_Abbreviations_automatic_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_013_Abbreviations_automatic_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_013_Abbreviations_automatic_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /tmp/rosetta_013_Abbreviations_automatic_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/013_Abbreviations_automatic/013_Abbreviations_automatic.ori -o /tmp/rosetta_013_Abbreviations_automatic_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "Abbreviations automatic <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 013_Abbreviations_automatic ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.13 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.13)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.13 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.14 014_Abbreviations_easy

**#014 — Abbreviations easy** | **Task file:** `_tasks/014_Abbreviations_easy.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/014_Abbreviations_easy/` if it does not exist: `mkdir -p tests/run-pass/rosetta/014_Abbreviations_easy/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/014_Abbreviations_easy.md tests/run-pass/rosetta/014_Abbreviations_easy/task.md`
- [ ] Read `tests/run-pass/rosetta/014_Abbreviations_easy/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/014_Abbreviations_easy/_test/014_Abbreviations_easy.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/014_Abbreviations_easy/_test/014_Abbreviations_easy.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /tmp/rosetta_014_Abbreviations_easy_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /tmp/rosetta_014_Abbreviations_easy_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_014_Abbreviations_easy_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_014_Abbreviations_easy_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_014_Abbreviations_easy_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_014_Abbreviations_easy_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_014_Abbreviations_easy_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_014_Abbreviations_easy_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_014_Abbreviations_easy_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_014_Abbreviations_easy_debug /tmp/rosetta_014_Abbreviations_easy_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_014_Abbreviations_easy_stripped /tmp/rosetta_014_Abbreviations_easy_release && ls -la /tmp/rosetta_014_Abbreviations_easy_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_014_Abbreviations_easy_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_014_Abbreviations_easy_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /tmp/rosetta_014_Abbreviations_easy_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/014_Abbreviations_easy/014_Abbreviations_easy.ori -o /tmp/rosetta_014_Abbreviations_easy_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "Abbreviations easy <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 014_Abbreviations_easy ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.14 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.14)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.14 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.15 015_Abbreviations_simple

**#015 — Abbreviations simple** | **Task file:** `_tasks/015_Abbreviations_simple.md` | **Current state:** Folder exists but no `.ori` source yet, has `task.md`

### Setup
- [ ] Create folder `tests/run-pass/rosetta/015_Abbreviations_simple/` if it does not exist: `mkdir -p tests/run-pass/rosetta/015_Abbreviations_simple/_test`
- [ ] Copy task definition: `cp tests/run-pass/rosetta/_tasks/015_Abbreviations_simple.md tests/run-pass/rosetta/015_Abbreviations_simple/task.md`
- [ ] Read `tests/run-pass/rosetta/015_Abbreviations_simple/task.md` — understand the problem requirements, success criteria, and expected outputs

### Spec & Grammar Gate (MANDATORY — before writing ANY Ori code)

- [ ] Read `docs/ori_lang/v2026/spec/grammar.ebnf` — the **authoritative grammar** for ALL Ori syntax
- [ ] Read `.claude/rules/ori-syntax.md` — the quick reference for Ori syntax, types, prelude, and formatting rules
- [ ] Read the relevant spec clauses for the features this program will use. Key clauses:
  - `docs/ori_lang/v2026/spec/08-types.md` — type system (primitives, collections, sum types, generics)
  - `docs/ori_lang/v2026/spec/10-declarations.md` — functions, types, traits, impls, constants
  - `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` — blocks, semicolons, scoping rules
  - `docs/ori_lang/v2026/spec/14-expressions.md` — expressions, operators, literals, lambdas
  - `docs/ori_lang/v2026/spec/15-patterns.md` — pattern matching, destructuring
  - `docs/ori_lang/v2026/spec/16-control-flow.md` — for/while/loop, yield, break, ranges, labels
  - `docs/ori_lang/v2026/spec/18-modules.md` — imports, use declarations, visibility
  - `docs/ori_lang/v2026/spec/19-testing.md` — test syntax, test attributes, test runner

> **ABSOLUTE RULE: NEVER modify `.ori` source to work around a compiler error.**
>
> When the compiler rejects or mishandles syntax that is valid per the spec/grammar:
>
> 1. **STOP** — do NOT rewrite the code to avoid the error
> 2. **Validate** the syntax against `grammar.ebnf` and the spec — confirm it SHOULD work
> 3. **If valid per spec:** invoke `/add-bug` immediately with: the exact error message, the code that triggered it, and the spec/grammar clause that says it should work
> 4. **Keep the original code** — do NOT "fix" it by avoiding the feature. Add `#skip("BUG-XX-NNN: <description>")` if the test cannot run
> 5. **Record as language finding** in the subsection results and `rosetta-manifest.json`
>
> Rewriting code to avoid a compiler limitation is **deferral** — it hides the bug from the roadmap,
> the bug tracker, and future implementers. **The bugs found ARE the primary deliverable of this plan.**
> A working program that silently avoids broken features is worth LESS than a blocked program that
> exposes and records compiler issues.

### A. Language Design
- [ ] Design the most elegant, idiomatic Ori solution — push the full feature set (generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions, everything available). **Reference the spec/grammar read above** — use features because the spec says they exist, not because you've seen them work before.
- [ ] Write `tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` with implementation functions + `@main () -> void` that demonstrates the program with `print()` calls
- [ ] Write `tests/run-pass/rosetta/015_Abbreviations_simple/_test/015_Abbreviations_simple.test.ori` with `use std.testing { assert_eq }` and comprehensive assertions (happy path + edge cases + boundary conditions)
- [ ] Record language findings: where Ori shines, where it forces workarounds, missing features → blocker with roadmap/bug-tracker xref

### B. Compiler Correctness
- [ ] `timeout 30 cargo run -- check tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** clean type-check, 0 errors
- [ ] `ORI_DUMP_AFTER_PARSE=1 timeout 30 cargo run -- check tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **inspect:** AST has correct structure
- [ ] `ORI_DUMP_AFTER_TYPECK=1 timeout 30 cargo run -- check tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **inspect:** types resolved correctly
- [ ] `ORI_LOG=ori_types=debug timeout 30 cargo run -- check tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **inspect:** type inference trace, no warnings
- [ ] `timeout 30 cargo run -- test tests/run-pass/rosetta/015_Abbreviations_simple/_test/015_Abbreviations_simple.test.ori` — **expected:** all tests pass, 0 failures, 0 skips
- [ ] `timeout 30 cargo run -- run tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** correct output from `@main`

### C. LLVM Codegen & AOT
- [ ] `timeout 60 cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /tmp/rosetta_015_Abbreviations_simple_debug` — **expected:** successful compilation
- [ ] `timeout 60 cargo run -- build --release tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /tmp/rosetta_015_Abbreviations_simple_release` — **expected:** successful compilation
- [ ] `ORI_DUMP_AFTER_LLVM=1 timeout 60 cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /dev/null` — **inspect:** LLVM IR quality, correct function lowering
- [ ] `ORI_DUMP_AFTER_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /dev/null` — **inspect:** ARC IR, RC strategy decisions
- [ ] `/tmp/rosetta_015_Abbreviations_simple_debug` — **expected:** correct output, exit code 0
- [ ] `/tmp/rosetta_015_Abbreviations_simple_release` — **expected:** correct output identical to debug, exit code 0
- [ ] `diagnostics/dual-exec-debug.sh tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** interpreter output == AOT output, no mismatch
- [ ] `diagnostics/debug-release-compare.sh tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** debug output == release output, no divergence

### D. Memory & ARC Verification
- [ ] `ORI_CHECK_LEAKS=1 /tmp/rosetta_015_Abbreviations_simple_debug` — **expected:** zero leaks reported
- [ ] `ORI_TRACE_RC=1 /tmp/rosetta_015_Abbreviations_simple_debug 2>&1 | tail -20` — **inspect:** RC events balanced (alloc/inc/dec/free)
- [ ] `ORI_RT_DEBUG=1 /tmp/rosetta_015_Abbreviations_simple_debug` — **expected:** no runtime assertion failures
- [ ] `ORI_VERIFY_ARC=1 timeout 60 cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /dev/null` — **expected:** ARC IR verification clean
- [ ] `ORI_VERIFY_EACH=1 timeout 60 cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /dev/null` — **expected:** LLVM IR verification after every pass clean
- [ ] `ORI_LLVM_LINT=1 timeout 60 cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /dev/null` — **expected:** no UB patterns detected
- [ ] `diagnostics/rc-stats.sh tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** all functions show balance = 0
- [ ] `diagnostics/rc-stats.sh --block-level tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **inspect:** per-block RC breakdown, verify no imbalanced blocks
- [ ] `diagnostics/codegen-audit.sh tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** clean (exit 0), no RC/COW/ABI findings
- [ ] `diagnostics/codegen-audit.sh --strict tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **expected:** clean even in pessimistic mode
- [ ] If any RC imbalance found: `diagnostics/bisect-passes.sh tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` — **inspect:** which AIMS pipeline phase caused it

### E. Debug Symbols & Binary Quality
- [ ] `readelf --debug-dump=info /tmp/rosetta_015_Abbreviations_simple_debug 2>/dev/null | grep DW_TAG_subprogram` — **expected:** at least 1 subprogram entry
- [ ] `readelf --debug-dump=line /tmp/rosetta_015_Abbreviations_simple_debug 2>/dev/null | head -20` — **expected:** line number table references `.ori` source
- [ ] Record binary sizes: `ls -la /tmp/rosetta_015_Abbreviations_simple_debug /tmp/rosetta_015_Abbreviations_simple_release` — **record:** debug KB, release KB
- [ ] `strip -o /tmp/rosetta_015_Abbreviations_simple_stripped /tmp/rosetta_015_Abbreviations_simple_release && ls -la /tmp/rosetta_015_Abbreviations_simple_stripped` — **record:** stripped KB

### F. Performance Benchmarking
- [ ] Interpreter: `time cargo run -- run tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori` (3 runs) — **record:** median wall-clock ms
- [ ] AOT debug: `time /tmp/rosetta_015_Abbreviations_simple_debug` (3 runs) — **record:** median wall-clock ms
- [ ] AOT release: `time /tmp/rosetta_015_Abbreviations_simple_release` (3 runs) — **record:** median wall-clock ms
- [ ] Compile time debug: `time cargo run -- build tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /tmp/rosetta_015_Abbreviations_simple_debug` — **record:** ms
- [ ] Compile time release: `time cargo run -- build --release tests/run-pass/rosetta/015_Abbreviations_simple/015_Abbreviations_simple.ori -o /tmp/rosetta_015_Abbreviations_simple_release` — **record:** ms
- [ ] Calculate: AOT-release / interpreter speedup ratio — **record**
- [ ] Calculate: release / debug speedup ratio — **record**

### G. Bug Filing & Findings
- [ ] If ANY step above failed unexpectedly → `/add-bug` immediately with the exact failing command as repro
- [ ] If ANY step revealed a bad/misleading error message → `/add-bug`
- [ ] If ANY performance anomaly (debug faster than release, unreasonable slowness) → investigate, `/add-bug` if codegen issue
- [ ] If ANY missing language feature blocked the most elegant implementation → record as blocker with roadmap/bug-tracker xref
- [ ] Update `rosetta-manifest.json` entry: status, has_main, has_tests, aot_eligible, perf data, bugs_filed, language_findings

### H. Cross-Language Intelligence Query
- [ ] Run `/query-intel` (via `scripts/intel-query.sh`) for this program's key features — search for similar bugs, design patterns, and prior art across reference compilers (Rust, Go, Swift, Zig, Gleam, Elm, Roc, Koka, Lean 4):
  - [ ] `scripts/intel-query.sh search "Abbreviations simple <primary feature>"` — find related issues/patterns in reference compilers
  - [ ] `scripts/intel-query.sh compare "<feature area>"` — how do other compilers handle the same construct?
  - [ ] If the program hit a codegen or ARC issue: `scripts/intel-query.sh fixed "<issue description>" --repo rust,swift,koka` — have reference compilers fixed similar bugs?
  - [ ] Record cross-language insights: does Ori's approach match best-of-breed? Any design improvements suggested by prior art?

### I. `/tpr-review` — Independent Review of This Program's Work
- [ ] **`/tpr-review`** — dual-source (Codex + Gemini) review scoped to this program. The reviewers must evaluate:
  1. **Implementation elegance** — is this the most idiomatic Ori possible? Are there language features that could simplify the code but weren't used? Would a different approach (multi-clause, pattern matching, `for...yield`, pipe `|>`, etc.) be cleaner?
  2. **Test quality** — do the tests cover edge cases, boundary conditions, and negative cases? Are assertions meaningful (not trivial)? Any missing test dimensions?
  3. **Codegen findings** — review the LLVM IR dump and ARC IR dump outputs. Is the generated code reasonable? Any unnecessary RC operations? Any missed optimizations? Any suspicious patterns in `codegen-audit.sh --strict` output?
  4. **Memory correctness** — review `rc-stats.sh` output. Are all functions balanced? Any concerns from `ORI_TRACE_RC` output? Any patterns that might leak under different inputs?
  5. **Language gap analysis** — are the recorded language findings accurate and complete? Were any gaps missed? Are the roadmap/bug-tracker cross-references correct?
  6. **Performance assessment** — are the benchmark numbers reasonable? Any anomalies (debug faster than release, interpreter faster than AOT)?
  7. **Bug completeness** — were all discovered issues filed? Any issues glossed over or rationalized away?
  8. **Cross-language intelligence** — review the `/query-intel` findings. Were relevant prior art patterns incorporated? Any cross-language insights missed?

### J. Results Report
Present a formatted results summary to the user using the insight format. This is the deliverable for each program — the user sees the analysis, not just checkboxes.

- [ ] **Present results to user** using the insight block format:
  ```
  `★ Rosetta: 015_Abbreviations_simple ─────────────────────────────`
  **Status:** PASS / PARTIAL / BLOCKED
  **Ori Elegance:** [assessment — where the language shined, what was beautiful]
  **Language Gaps:** [missing features, awkward workarounds, roadmap xrefs]
  **Compiler Issues:** [bugs found, error message problems, type inference gaps]
  **Codegen Quality:** [LLVM IR assessment, RC operation count, unnecessary ops]
  **Memory:** [leak status, RC balance, ARC verification result]
  **Performance:** interp=Xms | debug=Xms | release=Xms | speedup=Xx
  **Binary:** debug=XKB | release=XKB | stripped=XKB | DWARF=OK/MISSING
  **Cross-Language:** [insights from reference compilers]
  **Suggestions:** [specific improvements, if any]
  **Bugs Filed:** [BUG-XX-NNN list, or "none"]
  `─────────────────────────────────────────────────`
  ```

- [ ] **Record results** in a `### 01.15 Results` block below this subsection (append after the close-out). This becomes the permanent record of this program's evaluation. Include:
  - All performance numbers (interpreter, debug, release, compile times, speedup ratios, binary sizes)
  - All diagnostic tool results (pass/fail for each: leak check, RC stats, codegen audit, DWARF, dual-exec, debug-release)
  - All bugs filed with BUG IDs
  - All language findings with roadmap xrefs
  - Cross-language intelligence insights
  - `/tpr-review` verdict and any changes made from reviewer feedback

- [ ] **Subsection close-out (01.15)** — MANDATORY before starting next subsection:
  - [ ] ALL pipeline steps above are `[x]` with results recorded
  - [ ] `/tpr-review` findings resolved
  - [ ] Results report presented to user and recorded in results block
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — which diagnostics were hard to interpret? Which commands did you repeat? What tool would save 10 min next time?
  - [ ] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check`

### 01.15 Results

<!-- Filled in during execution. This is the permanent record of this program's evaluation. -->

| Metric | Value |
|--------|-------|
| Status | |
| Interpreter time (ms) | |
| AOT debug time (ms) | |
| AOT release time (ms) | |
| Compile debug (ms) | |
| Compile release (ms) | |
| AOT/interp speedup | |
| Release/debug speedup | |
| Binary debug (KB) | |
| Binary release (KB) | |
| Binary stripped (KB) | |
| Leak check | |
| RC stats balanced | |
| Codegen audit | |
| Codegen audit --strict | |
| ORI_VERIFY_ARC | |
| ORI_VERIFY_EACH | |
| ORI_LLVM_LINT | |
| Dual-exec parity | |
| Debug-release parity | |
| DWARF symbols | |
| Bugs filed | |
| Language findings | |
| Cross-language insights | |
| TPR verdict | |

---
## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `rosetta-manifest.json` has accurate entries for all 15 programs (status, bugs, findings, perf)
- [ ] All 15 programs have: `task.md`, `<name>.ori` with `@main`, `_test/` with tests
- [ ] All 15 programs ran EVERY step of Phases A-J (no shortcuts, no abbreviations)
- [ ] `/tpr-review` passed for every subsection
- [ ] Passing programs: zero dual-exec mismatches, zero leaks, clean codegen audit, clean `--strict`, clean `ORI_VERIFY_ARC`, clean `ORI_VERIFY_EACH`, clean `ORI_LLVM_LINT`
- [ ] DWARF symbols verified on all AOT debug binaries
- [ ] Performance baselines recorded for all 15 programs (interpreter, debug, release, compile times, speedup ratios)
- [ ] Every bug filed (`/add-bug`) or fixed (`/fix-bug`)
- [ ] Every language/syntax gap documented in manifest with roadmap/bug-tracker cross-reference
- [ ] Blocked programs have explicit cross-references
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] Plan annotation cleanup
- [ ] Plan sync — update plan metadata
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` section-close sweep
- [ ] **Run `/create-plan` to add next section** — task selection informed by this section's findings

**Exit Criteria:** All 15 programs fully evaluated through every step of the pipeline. Manifest complete with status, bugs, language findings, and performance data. Every blocked program has a concrete cross-reference. Primary deliverable = findings, fixes, and language insights.

