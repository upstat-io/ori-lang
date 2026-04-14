---
name: rosetta-test
description: Execute rosetta-stress-test plan programs, auto-fixing bugs encountered at each phase. Files bugs via /add-bug, fixes via /fix-bug, retries until clean or blocked by missing features. TRIGGER when working on rosetta programs or stress-testing the compiler.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, AskUserQuestion, Skill
argument-hint: "[subsection-id] — optional: start at specific subsection (e.g., 01.3)"
---

# Rosetta Test

Execute the `plans/rosetta-stress-test` plan with a **bug-fix-retry loop**: run each program through every pipeline phase, and when a phase fails due to a compiler bug, automatically file it via `/add-bug`, fix it via `/fix-bug`, and retry the phase — repeating until the subsection passes clean or is blocked by a missing language feature that requires user decision.

This is `/continue-roadmap plans/rosetta-stress-test` with the following behavioral overlay:

1. **Bug encounters trigger immediate fix-and-retry** — not deferral, not workarounds, not `#skip`
2. **Missing features escalate to the user** — the skill cannot implement new language features
3. **Each subsection loops until clean** — partial passes are not accepted
4. **The plan's phase structure is authoritative** — this skill executes it, not replaces it

## Usage

```
/rosetta-test              # pick up first incomplete subsection
/rosetta-test 01.3         # start at specific subsection
```

## Step -1 — MANDATORY: Re-read CLAUDE.md

Before doing ANYTHING else, re-read the entire project CLAUDE.md with the Read tool. Context compression may have dropped critical rules. Do this every single time this skill runs.

```
Read: CLAUDE.md
```

## Step 0 — Load the Plan

Read the plan section file to determine current state:

```
Read: plans/rosetta-stress-test/section-01-first-15.md
```

Parse the YAML frontmatter `sections:` array. Each subsection has an `id`, `title`, and `status` (`not-started`, `in-progress`, `complete`).

**Focus selection:**
- If `ARGS` specifies a subsection (e.g., `01.3`), use that — verify it is not already `complete`
- Otherwise, find the **first subsection** (after `01.PRE`) with `status: not-started` or `status: in-progress`
- Skip `01.R` (TPR findings) and `01.N` (completion checklist) — those run after all programs are done

If all program subsections are `complete`, proceed to `01.R` and `01.N` close-out (see Step 8).

## Step 0.5 — Load the Program Context

For the selected subsection (e.g., `01.1` = `001_100_doors`):

1. Read the subsection's full content from the section file — it contains the complete phase checklist (A through M)
2. Read the task definition: `tests/run-pass/rosetta/{NAME}/task.md`
3. Check if implementation already exists: `tests/run-pass/rosetta/{NAME}/{NAME}.ori`
4. Check if tests already exist: `tests/run-pass/rosetta/{NAME}/_test/{NAME}.test.ori`

Record what phases are already `[x]` completed vs `[ ]` pending. Resume from the first pending phase.

## Step 1 — Execute Pipeline Phases with Bug-Fix-Retry

The plan defines phases A through M for each program. Execute them **sequentially** — each phase depends on the previous. The phases are defined in the plan file and are authoritative; this skill does NOT redefine them.

**Phase reference (from the plan — read the plan for full details):**
- **A. Setup** — create folder, copy task.md, read requirements
- **B. Spec & Grammar Gate** — read grammar.ebnf, ori-syntax.md, relevant spec clauses
- **C. Language Design** — design idiomatic solution, write .ori + tests, record findings
- **D. Compiler Correctness** — `ori check`, type inference trace, AST dump, typeck dump, `ori test`, `ori run`
- **E. LLVM Codegen & AOT** — debug/release build, LLVM IR, ARC IR, binary execution, dual-exec parity, debug-release parity
- **F. Memory & ARC Verification** — leak check, RC trace, runtime debug, ARC/LLVM verify, RC stats, codegen audit, bisect
- **G. Debug Symbols & Binary Quality** — readelf, line tables, binary sizes
- **H. Performance Benchmarking** — interpreter/AOT/release timing, compile times, speedup ratios
- **I. Cross-Language Intelligence** — follow the canonical intel-summary protocol: @.claude/skills/dual-tpr/compose-intel-summary.md. Per SSOT Step F — /rosetta-test extension: `symbols "<feature keyword>" --repo ori --limit 15`, `file-symbols "<suspect module path>" --repo ori`, `callers`/`callees "<failing symbol>" --repo ori`, `similar "<failing symbol>" --repo rust,swift,go --limit 5`, `search "<failure mode>" --limit 5`.
- **J. Bug Filing & Findings** — file any crashes, wrong output, leaks, missing features
- **K. `/tpr-review`** — independent dual-source review
- **L. Results Report** — formatted results to user, record in plan
- **M. Subsection Close-out** — verify all `[x]`, commit, repo hygiene

### The Bug-Fix-Retry Loop (Core of This Skill)

When executing ANY phase (D through F are the most common failure points), apply this protocol:

```
PHASE_RETRY_COUNT = 0
MAX_PHASE_RETRIES = 5

while PHASE_RETRY_COUNT < MAX_PHASE_RETRIES:
    Execute the current phase step (e.g., D1: ori check)

    if step PASSES:
        Mark [x], move to next step
        continue

    if step FAILS:
        Classify the failure:

        CASE 1 — COMPILER BUG (crash, wrong output, type error on valid code,
                 codegen failure, RC imbalance, memory leak, ICE, etc.):
            1. STOP — do not work around the bug
            2. Validate against spec (docs/ori_lang/v2026/spec/) — confirm
               the code is valid and the compiler is wrong
            3. File via /add-bug:
               Skill(add-bug, args: "<short description>")
               Include: repro (the .ori file + specific command), subsystem,
               severity, source: rosetta-test
            4. Mark the bug as blocking this subsection in the plan:
               Add <!-- blocked-by:BUG-XX-NNN --> to the failing phase step
            5. Fix via /fix-bug:
               Skill(fix-bug, args: "BUG-XX-NNN")
               This runs the full /fix-bug workflow: investigation, TDD matrix,
               implementation, TPR review, hygiene review, commit
            6. After /fix-bug completes, RETRY the failing step from the top
               PHASE_RETRY_COUNT += 1

        CASE 2 — MISSING LANGUAGE FEATURE (syntax not yet implemented,
                 feature referenced in spec but not in compiler, etc.):
            1. Record the missing feature with roadmap cross-reference
            2. Check if the feature exists in plans/roadmap/ — find the
               section that would implement it
            3. Mark as blocker in the plan:
               Add <!-- blocked-by:roadmap-section-XX --> to the step
            4. Use AskUserQuestion to escalate:
               "Subsection {id} ({name}) is blocked at Phase {phase} by a
               missing language feature: {description}.
               Roadmap reference: {section or 'not yet planned'}.
               Options:
               1. Skip this program and move to the next subsection
               2. Implement a simplified version that avoids this feature
               3. Stop the rosetta-test session here
               4. Other guidance"
            5. Follow user's decision

        CASE 3 — TEST/ASSERTION FAILURE (test is wrong, not compiler):
            1. Verify by consulting the spec and the task definition
            2. If the test IS wrong, fix the test (this is rare — assume
               compiler bug first per CLAUDE.md)
            3. If ambiguous, investigate the compiler path before changing tests

        CASE 4 — MULTIPLE BUGS IN SAME PHASE:
            Fix them ONE AT A TIME. Each bug gets its own /add-bug + /fix-bug
            cycle. After fixing bug #1, retry the phase — this may reveal
            bug #2 (which was masked by bug #1). Repeat until the phase passes
            or MAX_PHASE_RETRIES is hit.

    if PHASE_RETRY_COUNT >= MAX_PHASE_RETRIES:
        Escalate via AskUserQuestion:
        "Subsection {id} ({name}) Phase {phase} has failed {MAX_PHASE_RETRIES}
        times after filing and fixing {N} bugs. The phase still does not pass.
        Last failure: {description}.
        Bugs filed: {list of BUG-XX-NNN}.
        Options:
        1. Continue retrying (raise the cap)
        2. Skip this program and move to the next
        3. Stop the session"
```

**CRITICAL RULES for the retry loop:**

- **NEVER work around a compiler bug.** Do not rewrite the .ori code to avoid a bug. The elegant, idiomatic implementation is the SPEC — the compiler must handle it. File and fix.
- **NEVER use `#skip` to hide a bug.** `#skip` is only valid for genuinely unimplemented features that are in the roadmap, not for bugs in implemented features.
- **ALWAYS validate against the spec first.** Before filing a bug, confirm the code is valid per `docs/ori_lang/v2026/spec/`. If the spec says the code is invalid, adjust the code — that's not a bug.
- **Each /fix-bug gets full rigor.** No ad-hoc fixes. The Skill tool invocation is mandatory — never inline the workflow.
- **Test timeouts apply.** All test commands use `timeout 150`. Hanging = bug you introduced.
- **Commit after each fix.** `/fix-bug` handles its own commit via `/commit-push`. After returning, verify clean `git status` before retrying.

## Step 2 — Phase-Specific Execution Notes

### Phase C (Language Design) — Writing the Code

When designing the implementation:

1. Read `/ori-syntax` or `.claude/rules/ori-syntax.md` for current syntax
2. Push the FULL feature set — generics, pattern matching, closures, traits, iterators, sum types, `as`/`as?`, pipe `|>`, `for...yield`, multi-clause functions
3. Write the most elegant, idiomatic Ori solution — NOT the simplest one that compiles
4. If the elegant solution hits a compiler bug, that's the primary deliverable — file it
5. Write comprehensive tests in `_test/{NAME}.test.ori` with `use std.testing { assert_eq }`

### Phases D-F (Compiler, LLVM, Memory) — Primary Bug Discovery Zone

These phases generate the most bugs. For each command:

```bash
# Phase D examples — all with mandatory timeout
timeout 150 cargo run -- check tests/run-pass/rosetta/{NAME}/{NAME}.ori
timeout 150 cargo run -- test tests/run-pass/rosetta/{NAME}/_test/{NAME}.test.ori
timeout 150 cargo run -- run tests/run-pass/rosetta/{NAME}/{NAME}.ori

# Phase E examples
timeout 150 cargo run -- build -o /tmp/{NAME}_debug tests/run-pass/rosetta/{NAME}/{NAME}.ori
timeout 150 cargo run -- build --release -o /tmp/{NAME}_release tests/run-pass/rosetta/{NAME}/{NAME}.ori
timeout 150 /tmp/{NAME}_debug
timeout 150 /tmp/{NAME}_release

# Phase F examples
timeout 150 bash -c "ORI_CHECK_LEAKS=1 /tmp/{NAME}_debug"
timeout 150 bash -c "ORI_VERIFY_ARC=1 cargo run -- build tests/run-pass/rosetta/{NAME}/{NAME}.ori"
```

**Interpret failures per the retry loop in Step 1.** Crash = bug. Wrong output = bug. Leak = bug. RC imbalance = bug. File and fix each one.

### Phase H (Performance) — No Retry Needed

Performance benchmarking does not produce pass/fail outcomes that trigger the retry loop. Record the numbers and move on. Performance anomalies (interpreter faster than AOT, debug faster than release) should be filed via `/add-bug` as investigation items but do NOT block the subsection.

### Phase K (TPR Review) — Use /tpr-review

Invoke via the Skill tool:
```
Skill(tpr-review)
```

The `/tpr-review` skill handles its own fix-and-re-run loop. Findings from TPR review that reveal bugs should be filed via `/add-bug` and fixed via `/fix-bug` — the TPR skill handles this internally. After TPR passes clean, mark Phase K complete.

## Step 3 — Subsection Completion

When ALL phases (A through M) are `[x]` for the current program:

1. Update the subsection status in the section file frontmatter to `complete`
2. Present the results report (Phase L format from the plan)
3. Run `/commit-push` to commit all changes
4. Immediately proceed to the next incomplete subsection (Step 0 loop)

**Do NOT ask permission between subsections.** This skill runs autonomously through programs until blocked or all are complete. The user chose `/rosetta-test` knowing it would run continuously.

## Step 4 — Cross-Subsection State

Track across the session:

```
programs_completed = []      # subsections that passed all phases
programs_blocked = []        # subsections blocked by missing features
bugs_filed = []              # all BUG-XX-NNN entries created
bugs_fixed = []              # all BUG-XX-NNN entries fixed via /fix-bug
language_findings = []       # syntax/feature gaps recorded
performance_data = {}        # per-program benchmark results
```

After each subsection completes or blocks, update these accumulators. They feed the session report in Step 7.

## Step 5 — Handling Feature Blockers Across Programs

When a missing feature blocks program A, it may also block programs B and C. Before starting a new subsection, check whether any known feature blockers would also affect it:

1. Read the task.md for the next program
2. Assess whether the same missing feature would block it
3. If yes, skip to the next program that would NOT be blocked, and note the skip

This avoids wasting time on programs that will hit the same wall. The skipped programs remain `not-started` and can be revisited when the feature is implemented.

## Step 6 — Re-read CLAUDE.md Between Programs

**After every 3 programs completed**, re-read CLAUDE.md. Context compression during long sessions can drop critical rules. This is a safeguard against drift.

```
Read: CLAUDE.md
```

## Step 7 — Session Report

When the session ends (all programs done, user stops, or blocked), present:

```
## Rosetta Test — Session Report

Programs completed: {N}/{total}
Programs blocked: {N} (missing features)
Programs remaining: {N}

### Completed Programs
{For each:}
  - {name}: PASS | all phases clean | {N} bugs found and fixed
    Performance: interp={X}ms | AOT-debug={X}ms | AOT-release={X}ms
    Bugs: {BUG-XX-NNN list or "none"}

### Blocked Programs
{For each:}
  - {name}: BLOCKED at Phase {X} — {missing feature description}
    Roadmap ref: {section or "unplanned"}
    Partial progress: Phases A-{last complete} done

### Bugs Filed and Fixed This Session
  Total filed: {N}
  Total fixed: {N}
  {For each:}
  - [BUG-XX-NNN][severity] {title} — {fixed | escalated | blocked}

### Language Findings
  {For each:}
  - {feature gap or syntax limitation} — roadmap xref: {section}

### Feature Blockers (Require User Decision)
  {For each:}
  - {missing feature} — blocks: {list of programs}
    Roadmap: {section or "needs /create-draft-proposal"}
```

## Step 8 — Section Close-Out (After All Programs)

When all 15 program subsections are either `complete` or `blocked`:

1. **01.R Third Party Review** — run `/tpr-review` on the entire section's work
2. **01.N Completion Checklist** — verify all items per the plan
3. Update section-level frontmatter `status` based on outcome:
   - All programs complete + 01.R + 01.N done → `status: complete`
   - Some programs blocked → `status: in-progress` with blockers noted
4. `/commit-push` final state

If all 15 programs passed AND 01.R + 01.N are done, the section is complete. Use `AskUserQuestion` to ask:
```
Section 01 is complete. All 15 programs passed the full pipeline.
{N} bugs were filed and fixed along the way.
{N} language findings recorded.

Next step: Run /create-plan to add Section 02 with the next batch of programs?
```

## Key Rules Summary

- **Bugs are the primary deliverable** — working programs are a means to finding bugs
- **NEVER work around compiler bugs** — file and fix them
- **NEVER use #skip to hide bugs** — only for genuinely unimplemented features
- **Full /fix-bug rigor for every bug** — no ad-hoc fixes, no inlining the workflow
- **Missing features escalate to user** — this skill cannot implement new language features
- **Retry until clean** — partial passes are not accepted
- **Test timeouts are mandatory** — `timeout 150` on all test commands
- **The plan's phase structure is authoritative** — execute it, don't modify it
- **Spec is law** — validate code against spec before filing bugs
- **Autonomous between subsections** — no per-program permission needed
- **Re-read CLAUDE.md every 3 programs** — guard against context drift
