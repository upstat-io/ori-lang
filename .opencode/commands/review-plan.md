---
description: Collaboratively review a plan between Claude and GPT-5.4 until full consensus
---

# Review Plan Command

Claude and @review (GPT-5.4) collaboratively review a plan through 4 review passes, debating and reaching consensus at each stage before moving on. Claude makes the edits; @review challenges them. Neither proceeds until both agree.

## Usage

```
/review-plan <plan-path>
```

- `plan-path`: **Required.** Path to the plan directory or a specific plan file (e.g., `plans/aims/`, `plans/roadmap/section-05.md`).
  - If a directory: reviews all files in the directory
  - If a single file: reviews that file (and reads siblings for context)

**Arguments:** `$ARGUMENTS`

## Workflow

### Step 0: Load Context

Read ALL project rules before starting:
- CLAUDE.md (project root)
- Every file in .claude/rules/ (aot.md, arc.md, cargo.md, compiler.md, diagnostic.md, eval.md, impl-hygiene.md, ir.md, llvm.md, ori-lang.md, ori-syntax.md, parse.md, patterns.md, registry.md, roadmap.md, runtime.md, spec.md, tests.md, typeck.md, types.md)

These rules constrain every decision. Do not skip.

### Step 1: Read the Plan

Read the plan file(s) specified in `$ARGUMENTS`. If the path doesn't exist, report the error and stop.

- If a directory, read all `.md` files: `index.md`, `00-overview.md`, and all `section-*.md` files
- If a single file, read it plus any sibling plan files for context

### Step 2: Initial Assessment + @review

Do a quick read-through and form your initial impressions:
- Plan name and scope
- Number of sections/files
- First impressions: obvious strengths and concerns

**Consult @review**: Send the full plan text to @review via Task tool:
```
Here is a plan for the Ori compiler at {plan_dir}/. I'm about to run a 4-pass collaborative review (accuracy, completeness, hygiene, clarity). Before we start, read the plan and give me your initial impressions:
1. What are the plan's biggest strengths?
2. What are the most concerning areas?
3. What should we focus on most in our review?

[paste full plan text]
```

Compare your impressions with @review's. Note areas where you agree and disagree. These disagreements become focus areas.

### Step 3: Pass 1 — Technical Accuracy

**You (Claude)** do the first accuracy pass:
1. Cross-reference every technical claim against the actual codebase:
   - Do referenced files, types, functions, modules exist?
   - Are crate dependency assumptions correct? (ori_lexer -> ori_parse -> ori_ir -> ori_types -> ori_eval -> ori_llvm -> oric)
   - Are described code patterns accurate?
2. Check claims against the spec in docs/ori_lang/v2026/spec/
3. **Run tests** to verify assumptions: `cargo t` (Rust tests), `cargo st` (Ori spec tests), or targeted tests for areas the plan touches. Don't just read — verify.
4. Draft a list of inaccuracies with proposed fixes

**Consult @review**: Send your findings to @review via Task tool:
```
Pass 1: Technical Accuracy. I found these inaccuracies in the plan at {plan_dir}/:

[list each finding with: what's wrong, evidence from codebase/spec, proposed fix]

Questions:
1. Did I miss any inaccuracies? Read the plan and codebase to check. Run tests yourself to verify — don't take my word for it.
2. Do you agree with each proposed fix? Challenge any you disagree with.
3. Are any of my "inaccuracies" actually correct and I misread the code?
```

**Iterate**: If @review disagrees with any finding or proposes additional ones, discuss. Re-consult @review until you both agree on the exact set of accuracy fixes.

**Verify @review's claims**: When @review proposes new findings or challenges yours, DO NOT blindly accept. Independently verify every claim @review makes — read the actual code, run the actual tests. @review can hallucinate file paths, function signatures, and behavior. Trust but verify.

**Apply**: Once consensus is reached AND you have verified all findings from both sides, edit the plan files to apply all agreed accuracy fixes.

### Step 4: Pass 2 — Completeness & Gaps

**You (Claude)** do the completeness pass:
1. Review each section for missing steps, edge cases, error handling
2. Check for missing sync points — enum variants, types, registration entries
3. Check test strategies — run existing tests (`cargo st tests/spec/path/`) to see what's already covered and what's missing
4. Draft a list of gaps with proposed additions

**Consult @review**: Send your findings to @review via Task tool:
```
Pass 2: Completeness & Gaps. After accuracy fixes, here are the gaps I found in {plan_dir}/:

[list each gap with: what's missing, why it matters, proposed addition]

Questions:
1. What gaps did I miss? Review the plan for anything I overlooked.
2. Do you agree each gap is real? Some might be intentionally out of scope.
3. Are my proposed additions the right fix, or is there a better approach?
```

**Iterate**: Debate any disagreements. @review may identify gaps you missed, or argue some of yours aren't real gaps. Re-consult until consensus.

**Verify @review's claims**: If @review says a gap exists or doesn't exist, check it yourself. Read the code, run the tests. Don't accept "this sync point is missing" without verifying the actual enum/registry/file.

**Apply**: Once verified, edit the plan files to add all agreed completeness fixes.

### Step 5: Pass 3 — Hygiene & Feasibility

**You (Claude)** do the hygiene pass:

**Part A — Plan-level hygiene:**
1. Check against .claude/rules/impl-hygiene.md and .claude/rules/compiler.md
2. File size limits, phase boundary discipline, test conventions, step ordering
3. Flag complex or risky steps

**Part B — Codebase scan:**
4. Extract every file path, crate, module the plan will touch
5. Read those files (up to 30, prioritize core files)
6. **Build and test** the affected areas: `cargo c` to check compilation, `cargo t -p <crate>` for targeted crate tests, `./clippy-all.sh` for lint issues. Record any existing failures — the plan should account for them.
7. Audit against hygiene rules: BLOAT, WASTE, DRIFT, EXPOSURE, LEAK, STYLE
8. Draft cleanup items to weave into the plan

**Consult @review**: Send your findings to @review via Task tool:
```
Pass 3: Hygiene & Feasibility. Here's what I found in {plan_dir}/:

Plan-level issues:
[list reordering, warnings, hygiene violations]

Codebase findings (from scanning files the plan touches):
[list each finding: category (BLOAT/WASTE/DRIFT/etc), file:line, issue, which plan section should fix it]

Files scanned: N | Files with findings: M

Questions:
1. Are my codebase findings real? Don't let me fabricate issues — verify the ones that seem questionable.
2. Did I miss any hygiene violations in the plan itself?
3. Is the step ordering I'm proposing correct? Check crate dependencies.
4. Are any cleanup items too aggressive (would destabilize rather than improve)?
```

**Iterate**: @review verifies findings against the actual code, pushes back on fabrications, adds missed items. Re-consult until consensus.

**Verify @review's claims**: If @review says a file is clean or flags a new violation, verify it. Read the file, count the lines, check the imports. @review cannot write or edit — it may misread code it only skimmed.

**Apply**: Once verified, edit the plan files to apply hygiene fixes and weave in cleanup items.

### Step 6: Pass 4 — Clarity & Consistency

**You (Claude)** do the clarity pass:
1. Check section descriptions for ambiguity
2. Sharpen vague checklist items into specific, verifiable tasks
3. Fix inconsistent terminology
4. Verify overview matches section contents after all prior edits
5. Check for contradictions between sections

**Consult @review**: Send your proposed clarity edits to @review via Task tool:
```
Pass 4: Clarity & Consistency. Final pass on {plan_dir}/ — here are my proposed clarity improvements:

[list each change: what's unclear/inconsistent, proposed rewrite]

Questions:
1. Do my rewrites actually improve clarity, or did I make them worse?
2. Did I miss any vague or ambiguous items?
3. Is the terminology consistent now across all sections?
4. Does the overview still accurately reflect the plan after all our edits?
```

**Iterate**: @review checks that "clarity improvements" didn't introduce new ambiguity or change meaning. Re-consult until consensus.

**Verify @review's claims**: If @review says a rewrite changed meaning or introduced ambiguity, re-read the original and your edit side by side. Confirm before reverting.

**Apply**: Once verified, edit the plan files with all agreed clarity fixes.

### Step 7: Final Consensus Check

Before presenting the verdict, do one final round with @review:

**Consult @review**: Send the final state to @review via Task tool:
```
We've completed all 4 review passes on {plan_dir}/. Here's a summary of everything we changed:

Accuracy fixes: [count and list]
Completeness additions: [count and list]
Hygiene fixes: [count and list]
Clarity improvements: [count and list]

Read the plan one more time in its current state. Final questions:
1. Is there anything we missed across all 4 passes?
2. Are there any remaining concerns that need human judgement?
3. What verdict do you recommend: CLEAN, MINOR FIXES APPLIED, SIGNIFICANT REWORK APPLIED, or NEEDS MANUAL ATTENTION?
4. Do you fully endorse this plan as ready for implementation?
```

**Resolve any final disagreements.** If you and @review disagree on the verdict, present both positions to the user.

### Step 8: Present Verdict

Present the collaborative verdict:

```
## Plan Review: {plan name}
### Collaboration: Claude (Opus 4.6) + GPT-5.4

### Review Summary

| Pass | Findings | Consensus Rounds |
|------|----------|-----------------|
| 1. Technical Accuracy | N fixes | M rounds |
| 2. Completeness & Gaps | N additions | M rounds |
| 3. Hygiene & Feasibility | N fixes (P codebase findings) | M rounds |
| 4. Clarity & Consistency | N improvements | M rounds |

### Changes Made
[consolidated list of all edits, grouped by pass]

### Remaining Concerns (Consensus)
[issues both agents agree need human attention]

### Disagreements (if any)
[points where Claude and GPT-5.4 could not reach consensus — present both positions]

---

## Verdict: **{VERDICT}**
### Claude: {VERDICT} | GPT-5.4: {VERDICT}

{2-3 sentence assessment agreed by both agents. If verdicts differ, explain why.}
```

**Verdict definitions:**
- **CLEAN**: No issues found. Plan is ready for implementation. Both agents endorse.
- **MINOR FIXES APPLIED**: Small corrections made. Plan is ready. Both agents endorse.
- **SIGNIFICANT REWORK APPLIED**: Substantial edits. Review the diff before proceeding.
- **NEEDS MANUAL ATTENTION**: Issues requiring human judgement. Cannot be auto-fixed.

## Important Rules

1. **Every pass consults @review** — No pass is complete without @review's sign-off.
2. **Iterate until consensus** — Don't move to the next pass with unresolved disagreements. Hash it out.
3. **Claude edits, @review verifies** — Claude has write access; @review reads and challenges.
4. **Be specific** — Every finding needs evidence: a spec clause, a file:line, or concrete reasoning.
5. **Cross-reference, don't guess** — Both agents must actually read spec files and source code.
6. **No fabrications** — @review's job is partly to catch Claude inventing findings. And vice versa.
7. **Track consensus rounds** — Note how many back-and-forths each pass took. More rounds = more contentious = flag for user.
8. **Flag what can't be resolved** — If consensus fails on a point, present both positions. Don't suppress disagreement.
9. **Test, don't assume** — Both agents should run tests (`cargo t`, `cargo st`, `cargo c`, `./clippy-all.sh`, `./test-all.sh`) to verify claims about the codebase. Reading code is not enough — compile it, test it, prove it. If a plan claims something works or doesn't work, run the test. If a plan touches a crate, build that crate. Ask @review to independently verify test results when findings are contentious.
