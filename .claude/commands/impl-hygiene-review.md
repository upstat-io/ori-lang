---
name: impl-hygiene-review
description: Review implementation hygiene at phase boundaries — plumbing quality and file organization.
allowed-tools: Read, Grep, Glob, Agent, Bash, Skill
---

# Implementation Hygiene Review

Review implementation hygiene against `.claude/rules/impl-hygiene.md` and generate a plan to fix violations.

**Implementation hygiene is NOT architecture** (design decisions are made). It covers the full plumbing layer — phase boundaries, data flow, error propagation, abstraction discipline, file organization, naming, comments, visibility, and lint discipline.

## Target

`$ARGUMENTS` specifies the boundary or scope to review. **If empty or blank, default to last commit mode** (equivalent to `/impl-hygiene-review last commit`). Otherwise, there are two modes:

### Path Mode (explicit crate/directory targets)
- `/impl-hygiene-review compiler/ori_lexer compiler/ori_parse` — review lexer→parser boundary
- `/impl-hygiene-review compiler/ori_parse compiler/ori_types` — review parser→type-checker boundary
- `/impl-hygiene-review compiler/ori_types` — review internal phase boundaries within a crate
- `/impl-hygiene-review compiler/ori_arc` — review ARC pass composition

### Commit Mode (use a commit as a scope selector)
- `/impl-hygiene-review last commit` — review files touched by the most recent commit
- `/impl-hygiene-review last 3 commits` — review files touched by the last N commits
- `/impl-hygiene-review <commit-hash>` — review files touched by a specific commit

**CRITICAL: Commits are scope selectors, NOT content filters.** The commit determines WHICH files and areas to review. Once the files are identified, review them completely — report ALL hygiene findings in those files, regardless of whether the finding is "related to" or "caused by" the commit. The commit is a lens to focus on a region of the codebase, nothing more. Do NOT annotate findings with whether they relate to the commit. Do NOT deprioritize or exclude findings because they predate the commit.

**Commit scoping procedure:**
1. Use `git diff --name-only HEAD~N..HEAD` (or appropriate range) to get the list of changed `.rs` files
2. Expand to include the full crate(s) those files belong to (e.g., if `compiler/ori_llvm/src/derive.rs` was touched, include all of `compiler/ori_llvm/`)
3. Proceed with the standard review process using those crates as the target

## Execution

### Step 1: Load Rules

The full rule set is embedded below (source of truth files — do not maintain separate copies):

**Hygiene Rules** (`.claude/rules/impl-hygiene.md`):
@.claude/rules/impl-hygiene.md

**Compiler Guidelines** (`.claude/rules/compiler.md`):
@.claude/rules/compiler.md

### Step 2: Load Plan Context

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

### Step 3: Identify Review Targets

Determine the distinct crates or phase boundaries to review based on the target scope from Step 1:

1. List the crates (directories) in scope
2. Identify which phase boundaries exist between them (e.g., lexer→parser, parser→types)
3. Group crates into **review units** — each review unit is either:
   - A single crate (for internal review)
   - A pair of crates sharing a boundary (for boundary review)
   - Closely related crates that should be reviewed together

Each review unit will be reviewed by a **separate Agent** in the next step.

### Step 4: Review Each Target (Separate Agent Per Review Unit)

For **each review unit** identified in Step 3, spawn a **separate Agent** (using the Agent tool). Each agent receives:

1. **The full rule set** — both hygiene rules and compiler guidelines (from Step 1)
2. **Plan context summary** — which plans are active and relevant (from Step 2)
3. **The specific crate(s)/boundary** to review
4. **The audit checklist** (below)

Each agent performs the following work within its review unit:

#### 4a. Map the Boundary

1. What types cross the boundary? (tokens, AST nodes, IR types)
2. What functions form the interface? (entry points, constructors, conversion functions)
3. What data flows across? (source text, spans, errors, metadata)

Read `lib.rs` and key interface files to understand the public API surface.

#### 4b. Trace Data Flow

1. **Read the producer's output types** — What does the upstream phase emit?
2. **Read the consumer's input handling** — How does the downstream phase receive and process it?
3. **Check the boundary types** — Are they minimal? Do they carry unnecessary baggage?
4. **Check ownership** — Is data moved, borrowed, or cloned? Are clones necessary?

#### 4c. Audit Each Rule Category

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
- [ ] New variants added in one location are present in all parallel locations? (e.g., new error code in enum → `from_str()` → `DOCS` → `explain`)
- [ ] When centralization isn't feasible, is there a test enforcing completeness?
- [ ] Operator→trait mappings, keyword→token mappings, error code→doc mappings — are these centralized or at risk of drift?

**Gap Detection:**
- [ ] Features supported in downstream phases (type checker, evaluator, codegen) also supported in upstream phases (parser, lexer)?
- [ ] No silent workarounds for missing capabilities? (e.g., destructuring instead of `.0` because parser blocks it)
- [ ] Full pipeline works end-to-end for each feature? (lexer → parser → type checker → evaluator → codegen)

**File Organization:**
- [ ] All production source files under 500 lines? (test files exempt)
- [ ] Each file has a single clear responsibility? (not mixing closures, operators, construction, dispatch)
- [ ] Logical groups of 200+ lines within a file extracted to submodules?
- [ ] File names describe what the file does? (not just `mod.rs` holding everything)
- [ ] Directory structure mirrors the logical phase/pass structure?
- [ ] Files touched by these commits that were already over 500 lines — were they split?

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

#### 4d. Return Findings

Each agent must return its findings as a structured list using the categories from `.claude/rules/impl-hygiene.md` (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE) with their default severity levels. Every finding must include `file:line`, the boundary it violates, and a concrete fix.

**Parallelization:** Review agents for independent crates/boundaries should be spawned in parallel. Only serialize agents that share a boundary (e.g., if reviewing lexer→parser, don't also spawn a separate lexer-only and parser-only agent).

### Step 5: Compile Findings

Collect the findings returned by all review agents. Deduplicate any findings that overlap at shared boundaries. Organize findings by boundary/interface and present them to the user.

### Step 6: Generate Plan (Separate Agent)

Spawn a **separate Agent** to generate the fix plan. This agent should use `/create-plan` (via the **Skill** tool). Pass it:

1. **All compiled findings** from Step 5
2. **The plan name**: `hygiene-{target-short-name}` (e.g., `hygiene-ori-types`, `hygiene-lexer-parser`, `hygiene-last-commit`)

The agent should create a plan that:

1. Lists every LEAK, DRIFT, GAP, WASTE, EXPOSURE, and BLOAT finding with `file:line` references
2. Groups by boundary (e.g., "lexer→parser", "parser→types")
3. Estimates scope: "N boundaries, ~M findings"
4. Orders: leaks first (phase bleeding), then drift (sync), then gaps (feature coverage), then bloat (file organization), then waste (perf), then exposure (type safety)

The **final section** of the plan must be a cleanup step:

```markdown
## Cleanup

- [ ] Run `./test-all.sh` to verify no behavior changes
- [ ] Run `./clippy-all.sh` to verify no regressions
- [ ] Delete this plan directory: `rm -rf plans/hygiene-{name}/`
```

Hygiene fix plans are disposable — they exist to track the fixes, then get deleted when complete.

### Plan Section Format

Each section groups findings by boundary:

```
## {Boundary: Phase A → Phase B}

**Interface types:** {list types crossing this boundary}
**Entry points:** {list key functions}

### Active Plan Context

{List each plan file read and its relevance. If a plan has a reroute/suspension, note it here.}
- `plans/trait_arch/` — Active reroute: all roadmap work suspended until trait architecture refactor completes
- (none) — if no plan files were found

### Findings

1. **[LEAK]** `file:line` — {description}
2. **[DRIFT]** `file:line` — {description}
   → covered by plans/{plan}/ ({section name})
3. **[DRIFT] [PLANNED]** `file:line` — {description}
   → fix described in plans/{plan}/{section}.md
4. **[GAP]** `file:line` — {description}
5. **[WASTE]** `file:line` — {description}
6. **[EXPOSURE]** `file:line` — {description}
```

## Important Rules

1. **No architecture changes** — Don't propose new phases, new IRs, or restructured crate graphs
2. **Full scope** — Phase boundaries, data flow, naming, comments, visibility, file organization, lint discipline, unsafe hygiene, and code fixes are all in scope. Only new phases, IRs, or crate graph restructures are out of scope (that's architecture).
3. **Trace, don't grep** — Follow actual data flow through the code, don't just search for patterns
4. **Read both sides** — Always read both the producer and consumer of a boundary
5. **Understand before flagging** — Some apparent violations are intentional (e.g., lexer tracking nesting depth for nested comments is acceptable phase-local state, not phase bleeding)
6. **Be specific** — Every finding must have `file:line`, the boundary it violates, and a concrete fix
7. **Compare to reference compilers** — When in doubt, check how Rust/Zig/Go/Gleam handle the same boundary at `~/projects/reference_repos/lang_repos/`
8. **Finding targets** — Scale with scope. Single boundary or single crate: **20**. Multi-crate or last N commits spanning multiple crates: **30**. Full project: **40**. Dig deep, read broadly, trace more paths. Do NOT fabricate, exaggerate, or inflate findings to hit the target — every finding must be real and verifiable. If the target area genuinely has fewer issues, report what you find honestly and note the shortfall.
