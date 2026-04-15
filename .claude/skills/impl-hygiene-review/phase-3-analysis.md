# Phase 3 — Deep Analysis (Multi-Pass, Multi-Lens)

Read by an **Opus** sub-agent dispatched from `/impl-hygiene-review`. Not a registered skill. This is the one phase that genuinely requires Opus judgment — finding detection through multi-lens hygiene analysis (phase-boundary bleeding, SSOT violations, algorithmic DRY, data-flow traces).

Consumes `/tmp/impl-hygiene-{run}/phase-{0,1,2}.json`. Writes `/tmp/impl-hygiene-{run}/phase-3.json` with: findings list (each with severity, category, file:line, evidence, proposed fix), plus a depth-of-analysis note so Phase 4 cross-check knows what to probe.

---

### Phase 3: Deep Analysis (Multi-Pass, Multi-Lens)

This is the "go deep" phase. Each review unit gets **multiple analysis passes**, each with a different lens. This catches issues that a single-pass review misses because different violation types require different reading strategies.

For **each review unit** identified in Phase 2, spawn agents for the following passes. Passes within the same review unit run **sequentially** (each builds on the prior). Passes for **different review units** run in parallel.

#### Pass 1: LEAK & SSOT Scan (Structural Pass)

**Goal**: Find all side logic, scattered knowledge, duplicated dispatch, and SSOT violations.

This pass reads the code structurally — it's looking for *where* logic lives relative to where it *should* live.

**Checklist:**
- [ ] **No duplicated dispatch**: match/if-chain on TypeTag, MethodKind, or operator kind exists ONLY at the canonical dispatch point? Any parallel match elsewhere is a LEAK — even if it produces correct results today.
- [ ] **No scattered knowledge**: type behavior (methods, operators, memory strategy) read from the registry, never hardcoded? Any `if type == X { special_behavior }` outside the canonical dispatcher is a LEAK.
- [ ] **No re-derived facts**: information computed by a prior phase is queried, not recomputed? Recomputing what's already stored creates a shadow source of truth.
- [ ] **No inline policy**: defaults, thresholds, format strings, validation rules defined at their canonical home, not at consumption sites? If changing a default requires grep-and-replace across files, it's a LEAK.
- [ ] **No validation at consumption**: invariants enforced at construction/entry, not checked at every use site? (parse-don't-validate)
- [ ] **No format logic outside formatters**: Display/Debug/diagnostic strings built in their formatting impls, not inline at error sites?
- [ ] **"Where would I look?" passes**: for every behavioral decision in this code, can you point to exactly ONE canonical location that defines it?
- [ ] **"What if it changes?" passes**: if the behavior changed, would exactly ONE file need updating (plus tests/docs)? If N > 1, it's a LEAK.
- [ ] **Canonical home exists**: for every behavioral decision, type relationship, or dispatch rule in this code — is there exactly ONE file that defines it? If the knowledge has no home (scattered everywhere), that's a structural SSOT violation.
- [ ] **No parallel authority**: are there two locations both claiming to define the same knowledge? (e.g., two match tables that both define "what methods does type X have?") Designate one as canonical, derive the other.
- [ ] **Consumers query, don't cache**: do consumers of shared knowledge call a function/query on the canonical owner, or do they maintain a local lookup table? Local tables are shadow homes.
- [ ] **Enforcement exists**: for every canonical source, is there a compile-time (exhaustive match) or test-time (exhaustiveness test) mechanism that catches consumers falling out of sync?
- [ ] **Architectural centers respected**: does this code correctly query from: registry (builtin behavior), type pool (type structure), AIMS (memory facts), repr-opt (representation)? Or does it re-derive what these centers already know?

#### Pass 2: Algorithmic DRY Scan (Pattern Pass)

**Goal**: Find duplicated algorithms — functions with identical control-flow skeletons that differ only in types, operations, or field names.

This pass reads the code *comparatively* — it's looking for structural similarity between function bodies, match arms, and dispatch tables. This is the hardest pass because it requires comparing code across files.

**Checklist:**
- [ ] **"Diff the bodies" test**: Read pairs of functions that handle similar cases (e.g., `dispatch_list_method` vs `dispatch_map_method`, `eval_iter_fold` vs `eval_iter_count`). Do their bodies differ only in type names, field names, or closure bodies while sharing the same control-flow skeleton?
- [ ] **"Count the steps" test**: Are there 3+ call sites that perform the same sequence of 2+ operations (even with different arguments)? Example: validate args → extract typed value → perform operation → wrap result.
- [ ] **"Cross-backend mirror" test**: Do eval and codegen maintain parallel dispatch tables, match arms, or routing logic with the same structure? Trace the method name through both backends — does each maintain its own routing independently?
- [ ] **"Match arm count" test**: Is the same enum/tag matched in N files with similar arm structure? If N > 2, N-1 of those are candidates for consolidation.
- [ ] **Threshold check**: 2 instances with >5 shared skeleton lines = extract. 3+ instances any size = extract. Cross-crate = always extract. Cross-backend = always extract to shared registry.
- [ ] **Remediation check**: For each algorithmic duplication found, identify the correct extraction: generic fn, higher-order fn, trait + blanket impl, data-driven dispatch, or (last resort) macro?

**How to execute this pass:**
1. For each crate, identify the major dispatch/routing functions (method dispatch, operator dispatch, type resolution, IR traversal)
2. Group structurally similar functions — same parameters shape, same loop/match/if structure
3. Read them side by side. Count the lines that are structurally identical vs. lines that differ.
4. If >60% structural overlap across 5+ lines: flag as `LEAK:algorithmic-duplication`
5. For cross-backend patterns, read the eval handler AND the LLVM emitter for the same feature — trace both paths

#### Pass 3: Boundary & Flow Scan (Plumbing Pass)

**Goal**: Find boundary violations, data flow issues, error handling gaps, and type discipline problems.

This pass reads the code *across boundaries* — it's looking at how data crosses phase lines.

**Checklist:**

**Phase Boundary Discipline:**
- [ ] Data flows one way? (no callbacks to earlier phase, no reaching back)
- [ ] No circular imports between phase crates?
- [ ] Boundary types are minimal? (only what's needed crosses)
- [ ] Clean ownership transfer? (move at boundaries, borrow within)
- [ ] No phase bleeding? (each phase does only its job)

**Data Flow:**
- [ ] Zero-copy where possible? (spans, not string copies)
- [ ] No allocation in hot paths? (no `String::from()` per token)
- [ ] Interned values via opaque IDs? (not raw integers)
- [ ] Source text borrowed, not copied?
- [ ] Arena/temporary data freed with phase?

**Error Handling at Boundaries:**
- [ ] Errors accumulated, not bailed on first?
- [ ] Phase-scoped error types? (lexer errors ≠ parse errors)
- [ ] Upstream errors propagated? (not swallowed or silently dropped)
- [ ] All errors carry spans?
- [ ] Recovery behavior explicit? (enum, not boolean flag)

**Type Discipline:**
- [ ] Separate raw vs cooked types at each boundary?
- [ ] Newtypes for all IDs crossing boundaries?
- [ ] No phase state leaked in output types? (no parser cursor in AST)
- [ ] Metadata separated from semantic data?

**Pass Composition (for optimization passes):**
- [ ] Each pass is IR → IR? (no hidden inputs)
- [ ] Pass ordering explicit and documented?
- [ ] No shared mutable state between passes?
- [ ] Boundary invariants asserted?

**Registration Sync Points:**
- [ ] Any enum/variant that must appear in multiple locations has a single source of truth?
- [ ] Parallel lists (match arms, arrays, maps) that must cover the same variants are derived from a shared source rather than manually mirrored?
- [ ] New variants added in one location are present in all parallel locations?
- [ ] When centralization isn't feasible, is there a test enforcing completeness?
- [ ] Operator→trait mappings, keyword→token mappings, error code→doc mappings — are these centralized or at risk of drift?

**Gap Detection:**
- [ ] Features supported in downstream phases also supported in upstream phases?
- [ ] No silent workarounds for missing capabilities?
- [ ] Full pipeline works end-to-end for each feature?

**Compiler-Specific Invariants:**
- [ ] **IR variant exhaustiveness**: New ExprKind/CanExpr/StmtKind variants handled in ALL consuming phases? No `_ => unreachable!()` catch-all arms hiding unhandled variants?
- [ ] **Cross-phase invariant contracts**: Does each phase boundary have explicit validation? ARC→Codegen: RC ops balanced? TypeCheck→Codegen: no unresolved type variables? Canon→All: no sugar variants, no TypeId::INFER?
- [ ] **Lowering completeness**: Every language construct lowered in BOTH eval AND LLVM codegen? No construct that works in one backend but crashes/panics in the other?
- [ ] **Span provenance**: Spans survive every lowering step (AST → CanExpr → ARC IR → LLVM IR)? No IR nodes with DUMMY spans outside of compiler-generated code?
- [ ] **Error recovery monotonicity**: TyError propagates silently without generating cascading diagnostics? Error nodes skipped (not re-diagnosed) by later phases?
- [ ] **Debug/release parity**: No `#[cfg(debug_assertions)]` blocks that change semantics (only verification)? Both debug and release builds produce identical observable output?
- [ ] **Interning discipline**: All identifier comparisons use `Name` (not `String`)? All type comparisons use `Idx` (not structure)? No string-based identity checks in non-test code?
- [ ] **Layout computation**: Type layout computed once and cached, not recomputed per-consumer? Codegen queries layout facts, never re-derives from field types?
- [ ] **Strategy dispatch coverage**: Strategy tables (e.g., DeriveStrategy) cover all IR variants? Test iterating ALL variants asserts each has a strategy entry?

#### Pass 4: Surface Hygiene Scan (Polish Pass)

**Goal**: Find file organization violations, naming issues, comment problems, visibility leaks, and style violations.

This pass reads the code *locally* — each file on its own terms.

**Checklist:**

**File Organization:**
- [ ] All production source files under 500 lines? (test files exempt)
- [ ] Each file has a single clear responsibility?
- [ ] Logical groups of 200+ lines within a file extracted to submodules?
- [ ] File names describe what the file does?
- [ ] Directory structure mirrors the logical phase/pass structure?

**Plan Annotation Hygiene:**
- [ ] Run `plan-annotations.sh --scope <review-paths>` (in this skill's folder) to scan the review scope. The tool classifies each annotation as **stale-resolved** (ID is `[x]` in an active plan — REMOVE NOW), **stale-completed-plan** (ID is in an archived plan under `plans/completed/` — REMOVE NOW), **orphan** (ID references a plan that no longer exists — INVESTIGATE), **active-scaffolding** (ID is `[ ]` in an active plan — OK for now), or **permanent** (spec citations, architecture-internal). Use `--cleanup-only` to see just the removal candidates; `--active-only` to confirm what's being tracked as in-progress; `--orphans-only` to find broken references; `--all --count` for a full per-classification summary. The tool reads every plan's markdown content to build the ID→status map, so classifications are per-finding accurate.
- [ ] For a quick hygiene-review scope check: `plan-annotations.sh --scope <paths> --cleanup-only` lists stale annotations grouped by finding ID, each showing the plan file and line where the finding was resolved. Every group is directly actionable.
- [ ] **Ephemeral names in function/fixture names** are also scanned automatically: `plan-annotations.sh --cleanup-only` now includes an `EPHEMERAL NAMES` section that catches underscore-form IDs baked into `fn` names (e.g., `fn tpr_07_017_two_unrelated_...`) and `include_str!` fixture paths. These are classified with the same stale/active logic as comment annotations but require *renaming* (not comment stripping) — the output includes `[fn]`/`[fixture]` tags and the full name for each hit.
- [ ] Active plan annotations (classification `active-scaffolding`) are acceptable only while the specific finding checkbox is `[ ]`; flip to stale the instant the checkbox becomes `[x]`
- [ ] Spec references (`Spec: Clause N.M`), `AIMS Section N`, and `eval_v2 Section N` are permanent and always acceptable (classified as `permanent` / `arch-internal` by the tool)

**Unsafe & FFI (for ori_llvm, ori_rt, oric):**
- [ ] Every unsafe block has a `// SAFETY:` comment?
- [ ] Unsafe scope minimized?
- [ ] FFI exports use `ori_` prefix, `#[no_mangle]`, `extern "C"`?
- [ ] C types use `std::ffi` (c_char, c_int), never raw primitives?

**Naming, Comments, Visibility, Style:**
- [ ] Phase-specific verb prefixes used? (cook_, parse_, check_, eval_, emit_)
- [ ] Spec citations on non-obvious language semantics implementations?
- [ ] No decorative banners, no commented-out code, no bare TODOs?
- [ ] Functions < 100 lines? Nesting depth ≤ 4?
- [ ] pub(crate)/pub(super) used appropriately? No dead pub items?

**Test Function Naming** (NAMING category — violations MUST be renamed in this review, never deferred):
- [ ] Every test name follows `<subject>_<scenario>_<expected>` shape — self-explanatory without looking at the body or any external artifact?
- [ ] No ephemeral identifiers in test names? Scan test function names (`#[test] fn ...` in Rust; `@test ... tests @target` in Ori) for:
  - Plan names (`locality_repr`, `repr_opt`, `capability_unification`, etc.)
  - Section / subsection numbers (`section_04_3`, `4_3_2`, `§4_3`, `04_2_phase_b`)
  - Plan annotations (`TPR_04_005`, `CROSS_04_014`, `roadmap_04`)
  - Bug / issue IDs (`BUG_04_045`, `issue_42`, `bug042`, `fix_2031`)
  - Dates (`2026_03_15`, `march_fix`, `q1_regression`)
  - Author initials (`eric_fix`, `es_repro`)
  - Commit hashes / ticket refs
- [ ] No banned weak descriptors? `_works`, `_works_correctly`, `_basic`, `_simple`, `_default`, `_correct`, `_valid`, `_ok`, `_sanity`, `_handles_X`, `_check_X`, `_verify_X`, or bare unit names (`test_iterator`, `test_parse_function`, `test_eval`).
- [ ] Provenance lives in `///` (Rust) or `//` (Ori) doc comments above the test, never in the function name?
- [ ] **Action on any violation found**: rename the test in the same pass. Extract the behavioral scenario from the test body, build a new name from it, move any useful plan/bug/issue provenance into a `///` doc comment above the test. The rename is local to the test file (test names have no callers) — scope, complexity, and "it's just a test" are not valid reasons to defer.

**Automated detection** (handled by Phase 0 tools — verify output, don't re-scan manually):
- `hygiene-lint.py --check test-ephemeral,test-weak` detects both ephemeral IDs and weak descriptors in test names
- `plan-annotations.sh --cleanup-only` detects ephemeral IDs classified against the plan index (stale vs active)
- `fn-rename.py` can batch-rename violations found by the above tools

Results from tools are **candidate violations** — read each one before renaming (a test named `test_cow_default_value_clone` legitimately contains `default` as a domain word, not a weak descriptor). The NAMING category requires *behavioral* judgment, not just grep matches.

Full rules: `.claude/rules/impl-hygiene.md` §Test Function Naming.


---

## Finding Targets

Finding targets scale with scope. These are **minimums** — dig deep, read broadly, trace more paths. Do NOT fabricate, exaggerate, or inflate findings to hit the target — every finding must be real and verifiable. If the target area genuinely has fewer issues, report what you find honestly and note the shortfall.

| Mode | Minimum Findings | Expected Range |
|------|-----------------|----------------|
| Single boundary or single crate | 20 | 20-35 |
| Multi-crate or last N commits spanning multiple crates | 40 | 40-60 |
| Full project | 80 | 80-120 |
| Full project with --focus | 60 (focused category) + 30 (other categories) | 60-100 focused + 30-50 other |

**Algorithmic DRY findings count as high-value** — a single algorithmic duplication finding that spans 5 files is worth more than 5 individual surface hygiene findings. Quality over quantity, but quantity matters too because thoroughness is the point.
