---
name: impl-hygiene-review
description: Deep, wide implementation hygiene review — multi-pass analysis across the full compiler with third-party cross-checking.
allowed-tools: Read, Grep, Glob, Agent, Bash, Skill
---

# Implementation Hygiene Review

Deep, wide-angle review of implementation hygiene against `.claude/rules/impl-hygiene.md`. Multi-pass, multi-lens analysis that traces data flow end-to-end, detects algorithmic duplication, and cross-checks findings via third-party review.

**Implementation hygiene is NOT architecture** (design decisions are made). It covers the full plumbing layer — phase boundaries, data flow, error propagation, abstraction discipline, algorithmic DRY, file organization, naming, comments, visibility, and lint discipline.

## Target

`$ARGUMENTS` specifies the boundary or scope to review. **If empty or blank, default to last commit mode** (equivalent to `/impl-hygiene-review last commit`). Otherwise, there are three modes:

### Path Mode (explicit crate/directory targets)
- `/impl-hygiene-review compiler/ori_lexer compiler/ori_parse` — review lexer→parser boundary
- `/impl-hygiene-review compiler/ori_parse compiler/ori_types` — review parser→type-checker boundary
- `/impl-hygiene-review compiler/ori_types` — review internal phase boundaries within a crate
- `/impl-hygiene-review compiler/ori_arc` — review ARC pass composition

### Commit Mode (use a commit as a scope selector)
- `/impl-hygiene-review last commit` — review files touched by the most recent commit
- `/impl-hygiene-review last 3 commits` — review files touched by the last N commits
- `/impl-hygiene-review <commit-hash>` — review files touched by a specific commit

### Full Project Mode (landscape survey)
- `/impl-hygiene-review full` — review the entire compiler across all crates and boundaries
- `/impl-hygiene-review full --focus=dry` — full review with emphasis on algorithmic duplication
- `/impl-hygiene-review full --focus=leaks` — full review with emphasis on side logic and SSOT

Full project mode is the widest sweep. It reviews every compiler crate, every phase boundary, all cross-crate interactions, and end-to-end pipeline flow. Use this when you want the complete landscape picture.

**CRITICAL: Commits are scope selectors, NOT content filters.** The commit determines WHICH files and areas to review. Once the files are identified, review them completely — report ALL hygiene findings in those files, regardless of whether the finding is "related to" or "caused by" the commit. The commit is a lens to focus on a region of the codebase, nothing more. Do NOT annotate findings with whether they relate to the commit. Do NOT deprioritize or exclude findings because they predate the commit.

**Commit scoping procedure:**
1. Use `git diff --name-only HEAD~N..HEAD` (or appropriate range) to get the list of changed `.rs` files
2. Expand to include the full crate(s) those files belong to (e.g., if `compiler/ori_llvm/src/derive.rs` was touched, include all of `compiler/ori_llvm/`)
3. **Dependency expansion**: Also include crates that *consume* the changed crate's public types or functions. If `ori_types` was changed, also expand to `ori_eval`, `ori_llvm`, `ori_arc` (its consumers). This catches boundary violations that the changed crate creates for its downstream consumers.
4. Proceed with the standard review process using those crates as the target

**Dependency map for expansion:**
```
ori_lexer      → consumed by: ori_parse
ori_parse      → consumed by: ori_types
ori_ir         → consumed by: ori_types, ori_eval, ori_llvm, ori_arc
ori_diagnostic → consumed by: all compiler crates
ori_registry   → consumed by: ori_types, ori_eval, ori_llvm
ori_types      → consumed by: ori_eval, ori_llvm, ori_arc
ori_patterns   → consumed by: ori_eval
ori_eval       → consumed by: oric
ori_arc        → consumed by: ori_llvm
ori_llvm       → consumed by: oric
ori_rt         → consumed by: ori_llvm (FFI contract)
```

## Execution

### Phase 1: Load Rules & Context

#### 1a. Load Rules

The full rule set is embedded below (source of truth files — do not maintain separate copies):

**Hygiene Rules** (`.claude/rules/impl-hygiene.md`):
@.claude/rules/impl-hygiene.md

**Compiler Guidelines** (`.claude/rules/compiler.md`):
@.claude/rules/compiler.md

#### 1b. Load Plan Context

Gather context from active and recently-modified plan files so the review doesn't flag work that is already planned, in-progress, or intentionally deferred.

**Procedure:**
1. Run `git diff --name-only HEAD` and `git diff --name-only --cached` to find uncommitted modified files in `plans/`
2. Run `git diff --name-only HEAD~3..HEAD -- plans/` to find plan files changed in recent commits
3. Combine both lists (deduplicate) to get all recently-touched plan files
4. Read each discovered plan file (skip files > 1000 lines — read the `00-overview.md` or `index.md` instead)

**How to use plan context:**

Plan context does NOT suppress or deprioritize findings. Instead, it **annotates** them:

- If a finding falls within scope of an active plan, append `→ covered by plans/{plan}/` to the finding
- If a plan has an active reroute or suspension notice (e.g., "all work suspended until X"), note this in the review preamble so the user knows which areas are in flux
- If a plan explicitly describes a refactor that would resolve a finding, mark it as `[PLANNED]` instead of proposing a separate fix — but still list it so nothing falls through cracks
- Findings NOT covered by any plan are reported normally — these are the high-value discoveries

**Example annotation:**
```
3. **[DRIFT]** `compiler/ori_types/src/check/registration/mod.rs:142` — Missing sync for new `Serialize` variant
   → covered by plans/trait_arch/ (Section 3: Registration Overhaul)
```

This ensures the review adds value by distinguishing "known debt being addressed" from "unknown debt needing attention."

### Phase 2: Map the Full Landscape

Before diving into findings, build a high-level map of the review scope. This is the "go wide" phase — understand the shape before probing the details.

#### 2a. Identify Review Targets

Determine the distinct crates or phase boundaries to review based on the target scope:

1. List the crates (directories) in scope
2. Identify which phase boundaries exist between them (e.g., lexer→parser, parser→types)
3. Map the dependency graph between in-scope crates
4. Group crates into **review units** — each review unit is either:
   - A single crate (for internal review)
   - A pair of crates sharing a boundary (for boundary review)
   - Closely related crates that should be reviewed together

#### 2b. Map Cross-Crate Data Flow (Full Project & Multi-Crate Mode)

For full project mode or when 3+ crates are in scope, spawn an agent to trace the key data flows end-to-end through the pipeline:

1. **Type flow**: How do types get defined (registry) → checked (ori_types) → evaluated (ori_eval) → compiled (ori_llvm)?
2. **Method dispatch flow**: Where is the canonical dispatch table? How does a method call route from parse → typecheck → eval/codegen?
3. **Error flow**: How do errors propagate across phase boundaries? Where do they get accumulated, deduplicated, formatted?
4. **Memory/RC flow**: How do ownership decisions flow from AIMS analysis → ARC pass → codegen emission → runtime?

This agent produces a **flow map** — a brief summary of how each major data category crosses the phase boundaries. This map is passed to all subsequent review agents as context.

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
- [ ] No stale plan annotations from completed plans? Run `plan-annotations.sh` (in this skill's folder) to scan
- [ ] Active plan annotations acceptable only while the plan is in progress
- [ ] Spec references (`Spec: Clause N.M`) are permanent and always acceptable

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

### Phase 4: Third-Party Cross-Check

**MANDATORY for full project mode. Recommended for all other modes.**

After Phase 3 agents return their findings, use `/tp-help` to cross-check the work. This creates a two-brain review: you found the patterns, now Codex validates them and looks for what you missed.

#### 4a. Validate Findings

Invoke `/tp-help` with a focused question. Pass a summary of 5-10 of the most significant findings (not all — pick the ones that are most ambiguous or architecturally significant) and ask Codex to validate:

```
/tp-help I'm running a hygiene review of [scope]. Here are my top findings — validate whether these are real violations or false positives, and tell me if I'm missing anything obvious in these areas:

[List of 5-10 findings with file:line and brief description]

Key files involved: [list the main files]
```

**What to do with Codex's response:**
- If Codex confirms a finding: increase confidence, keep it
- If Codex challenges a finding: re-read the code, check if you misunderstood the pattern. Update or drop the finding if Codex is right.
- If Codex surfaces NEW findings you missed: add them to the findings list. These are high-value discoveries — a fresh pair of eyes saw what you didn't.

#### 4b. Probe Blind Spots

After validating findings, use `/tp-help` again to probe areas you might have under-examined. Ask Codex to look at a specific area you didn't go deep on:

```
/tp-help I reviewed [scope] and found [N] findings, but I'm worried I may have missed algorithmic duplication in [specific area]. Can you compare [file A] and [file B] structurally and tell me if their control-flow skeletons are duplicated?
```

Or for cross-backend duplication:

```
/tp-help Compare the eval path for [feature] in [eval file] with the LLVM codegen path in [llvm file]. Are these maintaining parallel dispatch tables that should be unified?
```

**When to probe:**
- Any crate that yielded zero findings (suspiciously clean — likely under-examined)
- Cross-backend code (eval ↔ LLVM) — hardest to catch because it requires reading two codebases in parallel
- Large match/dispatch functions — easy to skim past structural duplication when arms look "different enough"
- Code paths you traced superficially (read the entry point but not the helpers)

#### 4c. Integrate Cross-Check Results

Merge Codex's validated and new findings back into the main findings list. Tag findings that Codex confirmed with `[TP-CONFIRMED]` and findings Codex surfaced with `[TP-SURFACED]` — this helps the plan prioritize high-confidence issues.

### Phase 5: Compile & Present Findings

Collect the findings from all passes across all review units. This is synthesis, not just concatenation.

#### 5a. Deduplicate

Same violation caught by multiple passes → keep the deepest analysis, drop the others.

#### 5b. Cross-Reference

Look for patterns across findings:
- **Cluster analysis**: 5+ findings in one module = design problem (escalate to architectural review)
- **3+ LEAKs in one module** = systemic side logic; the module lacks a canonical dispatch/query point
- **Same algorithm duplicated across N files** = missing abstraction (report as a single finding, not N findings)
- **Cross-backend findings** = highest priority; these drift silently

#### 5c. Severity Calibration

Apply default severities from the finding categories, then adjust:
- **LEAK:algorithmic-duplication** across 3+ sites → Critical (not just because it's a LEAK, but because the blast radius of protocol change is proportional to copy count)
- **Cross-backend LEAKs** → always Critical (eval ↔ LLVM dispatch drift is a correctness risk, not just a maintainability concern)
- Findings tagged `[TP-CONFIRMED]` → keep severity. Findings tagged `[TP-SURFACED]` → bump severity one level (Codex caught what you missed, which means it's less obvious and more likely to be missed again)

#### 5d. Present to User

Present findings organized by category and severity, with a summary preamble:

```
## Hygiene Review: [scope]

**Scope**: [crates/boundaries reviewed]
**Passes**: LEAK/SSOT, Algorithmic DRY, Boundary/Flow, Surface Hygiene
**Third-party cross-check**: [Yes/No] — [N confirmed, M surfaced]
**Finding counts**: [N LEAK, N DRIFT, N GAP, N WASTE, N EXPOSURE, N BLOAT]

### Active Plan Context
[Plans read and their relevance]

### Critical Findings (LEAKs)
...

### Major Findings (DRIFT, GAP)
...

### Minor Findings (WASTE, EXPOSURE, BLOAT)
...
```

### Phase 6: Generate Plan (Separate Agent)

Spawn a **separate Agent** to generate the fix plan. This agent should use `/create-plan` (via the **Skill** tool). Pass it:

1. **All compiled findings** from Phase 5
2. **The plan name**: `hygiene-{target-short-name}` (e.g., `hygiene-ori-types`, `hygiene-lexer-parser`, `hygiene-last-commit`, `hygiene-full`)

The agent should create a plan that:

1. Lists every LEAK, DRIFT, GAP, WASTE, EXPOSURE, and BLOAT finding with `file:line` references
2. Groups by boundary (e.g., "lexer→parser", "parser→types") or by violation type for full-project mode
3. Estimates scope: "N boundaries, ~M findings"
4. Orders: **LEAKs first and separately** (side logic is the root of all evil — every LEAK is a ticking architectural bomb), then drift (sync), then gaps (feature coverage), then bloat (file organization), then waste (perf), then exposure (type safety). LEAKs must NEVER be deferred — they cascade.
5. **Algorithmic duplication findings get their own section** — these often require coordinated multi-file refactoring (extracting a shared helper, adding a generic function, creating a data-driven dispatch table). Group by the algorithm being duplicated, not by where the copies live.

The **final section** of the plan must be a cleanup step:

```markdown
## Cleanup

- [ ] Run `./test-all.sh` to verify no behavior changes
- [ ] Run `./clippy-all.sh` to verify no regressions
- [ ] Delete this plan directory: `rm -rf plans/hygiene-{name}/`
```

Hygiene fix plans are disposable — they exist to track the fixes, then get deleted when complete.

### Plan Section Format

Each section groups findings by boundary or violation cluster:

```
## {Boundary: Phase A → Phase B}

**Interface types:** {list types crossing this boundary}
**Entry points:** {list key functions}

### Active Plan Context

{List each plan file read and its relevance. If a plan has a reroute/suspension, note it here.}
- `plans/trait_arch/` — Active reroute: all roadmap work suspended until trait architecture refactor completes
- (none) — if no plan files were found

### Findings

1. **[LEAK:duplicated-dispatch]** `file:line` — {description} — **canonical home**: `{canonical_file:line}`
2. **[LEAK:algorithmic-duplication]** `file_a:line` ↔ `file_b:line` — {description of shared skeleton} — **extraction**: {generic fn / HOF / trait / data-driven / macro}
3. **[LEAK:scattered-knowledge]** `file:line` — {description} — **canonical home**: `{canonical_file:line}`
4. **[DRIFT]** `file:line` — {description}
   → covered by plans/{plan}/ ({section name})
5. **[DRIFT] [PLANNED]** `file:line` — {description}
   → fix described in plans/{plan}/{section}.md
6. **[GAP]** `file:line` — {description}
7. **[WASTE]** `file:line` — {description}
8. **[EXPOSURE]** `file:line` — {description}
```

## Important Rules

1. **No architecture changes** — Don't propose new phases, new IRs, or restructured crate graphs
2. **Full scope** — Phase boundaries, data flow, naming, comments, visibility, file organization, lint discipline, unsafe hygiene, algorithmic DRY, and code fixes are all in scope. Only new phases, IRs, or crate graph restructures are out of scope (that's architecture).
3. **Trace, don't grep** — Follow actual data flow through the code, don't just search for patterns
4. **Read both sides** — Always read both the producer and consumer of a boundary
5. **Compare function bodies** — For algorithmic DRY, you must read pairs/groups of structurally similar functions side by side. Grepping for names is not enough — you need to compare control-flow skeletons.
6. **Understand before flagging** — Some apparent violations are intentional (e.g., lexer tracking nesting depth for nested comments is acceptable phase-local state, not phase bleeding)
7. **Be specific** — Every finding must have `file:line`, the boundary it violates, and a concrete fix
8. **Compare to reference compilers** — When in doubt, check how Rust/Zig/Go/Gleam handle the same boundary at `~/projects/reference_repos/lang_repos/`
9. **Cross-check with /tp-help** — For full project mode: MANDATORY. For other modes: RECOMMENDED. Always validate ambiguous findings and probe blind spots. A hygiene review that doesn't question its own completeness is incomplete.
10. **Follow the algorithm, not the name** — Two functions named differently but with identical control-flow skeletons are duplicated. Two functions named similarly but with genuinely different logic are not. Read bodies, not signatures.

## Finding Targets

Finding targets scale with scope. These are **minimums** — dig deep, read broadly, trace more paths. Do NOT fabricate, exaggerate, or inflate findings to hit the target — every finding must be real and verifiable. If the target area genuinely has fewer issues, report what you find honestly and note the shortfall.

| Mode | Minimum Findings | Expected Range |
|------|-----------------|----------------|
| Single boundary or single crate | 20 | 20-35 |
| Multi-crate or last N commits spanning multiple crates | 40 | 40-60 |
| Full project | 80 | 80-120 |
| Full project with --focus | 60 (focused category) + 30 (other categories) | 60-100 focused + 30-50 other |

**Algorithmic DRY findings count as high-value** — a single algorithmic duplication finding that spans 5 files is worth more than 5 individual surface hygiene findings. Quality over quantity, but quantity matters too because thoroughness is the point.
