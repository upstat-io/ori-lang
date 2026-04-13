---
name: improve-tooling
description: "AUTO-TRIGGER: Improve testing, diagnostic, debugging, or developer tooling. TRIGGER when: (1) a script in diagnostics/ or scripts/ produces confusing output, missing information, or wrong results, (2) test-all.sh, clippy-all.sh, or any test harness has gaps, missing coverage, or unclear failure output, (3) dual-exec-verify.sh, diagnose-aot.sh, or any diagnostic script doesn't cover a case you need, (4) you work around a tool limitation instead of fixing the tool, (5) you notice a script is missing --help, error handling, or useful flags, (6) you manually do something a script should automate, (7) RETROSPECTIVE — PER SUBSECTION (primary): invoked immediately after marking a plan subsection complete (e.g., {NN}.1, {NN}.2) to look back at THAT subsection's debugging journey while pain points are still fresh, (8) RETROSPECTIVE — BUG-FIX CLOSE: invoked at /fix-bug Phase 5 completion checklist step 8 to capture root-cause analysis tooling gaps, (9) RETROSPECTIVE — SECTION CLOSE (sweep): invoked at the end of a roadmap/plan section as an integration safety net that verifies per-subsection and bug-fix retrospectives ran and adds only NEW items from cross-cutting patterns, (10) DOC AWARENESS — a tool exists but isn't documented in its canonical doc surface (see §Documentation Surfaces), (11) DOC AWARENESS — tooling was skipped during the session because it wasn't discoverable in loaded docs, (12) DOC SYNC — a tool was created or modified during the session but its canonical doc surface wasn't updated. DO NOT TRIGGER for: normal tool usage that works correctly, or one-off ad-hoc commands."
---

# Improve Tooling

**SCOPE: This skill improves tooling — scripts, harnesses, diagnostics, and skills. It does NOT save memories, update memory files, or modify MEMORY.md. When invoked with user feedback about workflow behavior, improve the relevant skill files directly.**

**ABSOLUTE RULE: Never work around deficient tooling. Fix the tool.**

When you encounter friction, gaps, or deficiencies in any developer tooling — testing scripts, diagnostic scripts, build scripts, or any automation — you MUST improve the tool rather than working around it. The tool improvement IS the work.

**Tooling grows organically.** You cannot predict every use case ahead of time. The way the diagnostic suite gets sharp is by ratcheting it up by one improvement after every subsection, every bug fix, every debugging session — guided by what was *actually* painful, not what was imagined to be painful. This skill has three pillars: **reactive** (mid-task friction, the original auto-trigger), **reflective** (post-subsection, post-bug-fix, and post-section retrospective — see Retrospective Mode below), and **documentation awareness** (ensuring tools are discoverable and that new tools get documented — see Documentation Awareness Verification below).

**Pain memory decays fast.** This is why retrospectives must fire at the smallest natural unit of work, not at section close. By the time you've finished six subsections plus TPR plus hygiene review, the friction from subsection `.1` is days old and three reviews ago — you have already smoothed over it. Retrospective Mode therefore has THREE granularities: per-subsection (the primary capture mechanism, run while the journey is fresh), bug-fix close (captures root-cause analysis friction — mandated by `/fix-bug` Phase 5), and section-close (an integration sweep that catches cross-cutting patterns invisible at finer scope).

## Trigger Conditions

This skill auto-triggers when ANY of these are true:

1. **Confusing output** — a script produces output that requires manual interpretation, is ambiguous, or buries the important information
2. **Missing coverage** — a test harness, diagnostic script, or verification tool doesn't cover a case you need
3. **Manual workaround** — you find yourself manually doing something (piping output, grepping logs, running multiple commands in sequence) that a script should automate
4. **Wrong/stale results** — a tool produces incorrect, outdated, or misleading information
5. **Missing error handling** — a script silently fails, produces no output on error, or gives cryptic error messages
6. **Missing flags/options** — you need a capability the tool doesn't expose (e.g., `--verbose`, `--filter`, `--json`, `--help`)
7. **Friction during debugging** — you spend more than 30 seconds interpreting tool output or running follow-up commands to get the information you actually need
8. **Incomplete automation** — a multi-step manual process that should be a single command

Additionally, this skill is **mandatorily invoked** as a retrospective at three boundaries — these are not optional auto-triggers but required workflow steps:

9. **Per-subsection close** — immediately after marking a plan subsection complete (see Retrospective Mode §Per-Subsection)
10. **Bug-fix close** — at `/fix-bug` Phase 5 completion checklist step 8 (see Retrospective Mode §Bug-Fix Close)
11. **Section-close sweep** — at the end of a full section, after TPR and hygiene are clean (see Retrospective Mode §Section-Close Sweep)

Finally, this skill triggers for **documentation awareness gaps** — the invisible failure mode where tools exist but sessions don't know about them:

12. **Undocumented tool** — a script exists in `diagnostics/`, `scripts/`, or project root but isn't listed in its canonical doc surface (see §Documentation Surfaces)
13. **Tooling unawareness** — during the session, you manually did something that an existing tool already automates, but you didn't know the tool existed because it wasn't documented where you looked
14. **New tool without docs** — a tool was created or significantly modified during this session but its documentation wasn't updated across all canonical surfaces

## Tooling Scope

These are the tools you own and must improve:

| Category | Location | Canonical reference |
|----------|----------|---------------------|
| **Test harnesses** | `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh`, `./build-all.sh` | `CLAUDE.md` §Commands |
| **Diagnostic scripts** | `diagnostics/` | `diagnostics/README.md` — single canonical home (see §Documentation Surfaces) |
| **Scripts** (build, release, test utilities) | `scripts/` | `CLAUDE.md` §Commands |
| **Diagnostic common** | `diagnostics/_common.sh` | `.claude/rules/diagnostic.md` |
| **LLVM test harness** | `./llvm-test.sh` | `CLAUDE.md` §Commands |

## Documentation Surfaces

Every tool must be documented in its **canonical doc surface** (the single source of truth for that tool's documentation). Some tools also appear in **secondary surfaces** for discoverability. The mapping:

| Tool location | Canonical doc surface | Secondary surfaces |
|---|---|---|
| `diagnostics/*.sh` | `diagnostics/README.md` (full reference: usage, flags, workflows) | `.claude/rules/diagnostic.md` §Diagnostic Scripts (quick-reference table — MAY lag canonical), `CLAUDE.md` §Commands (key scripts only) |
| `scripts/` (commonly-needed: `perf-baseline.sh`, `cow-benchmark.sh`, `cache-doctor.sh`, `bump-build.sh`, `sync-version.sh`) | `CLAUDE.md` §Commands | Script's own `--help` |
| `scripts/` (maintenance/migration: `extract_tests.py`, `pgo-build.sh`, `release.sh`, etc.) | Script's own `--help` | Not required in CLAUDE.md — these are infrequently needed and would bloat the curated Commands section |
| Root harnesses (`test-all.sh`, `clippy-all.sh`, `fmt-all.sh`, `build-all.sh`) | `CLAUDE.md` §Commands | — |
| Root build/setup (`llvm-test.sh`, `llvm-build.sh`, `install.sh`, `full-check.sh`, etc.) | `CLAUDE.md` §Commands (if commonly needed) or script's own `--help` (if one-off) | — |

**SSOT invariant**: Each tool has exactly ONE canonical surface. `diagnostics/README.md` is the single source of truth for diagnostic scripts — `.claude/rules/diagnostic.md` is a quick-reference table that may lag; it is NOT a second mandatory sync target. Do NOT mandate parallel updates to multiple doc surfaces — that guarantees drift.

**Canonical means one, not two**: when the skill says "update the canonical surface," that means update ONE file. Secondary surfaces are best-effort — update them if convenient, but a missing secondary entry is not a gap finding. A missing canonical entry IS a gap finding.

**Why CLAUDE.md matters**: CLAUDE.md is loaded into every conversation's context. Tools documented *only* in `diagnostics/README.md` or hidden in `scripts/` are effectively invisible to most sessions. But CLAUDE.md is curated for interactive use — it should list commonly-needed scripts with brief descriptions, not serve as an exhaustive script index. Maintenance/migration scripts that are rarely needed interactively should have `--help` but don't need CLAUDE.md entries.

## Workflow

### Step 1: Identify the Deficiency

When you notice tooling friction, STOP and articulate:
- **What tool** is deficient (file path)
- **What the gap is** (missing feature, wrong output, no error handling, etc.)
- **What you were about to do instead** (the workaround you were about to use)

### Step 2: Read the Tool

Read the existing tool code. Understand:
- Its current capabilities and flags
- Its conventions (does it follow `_common.sh` patterns? Does it support `--help`?)
- Where the gap is in the code

Before creating a new tool, if the intelligence graph is available (`scripts/intel-query.sh status` returns `"ok"`), check if similar tools already exist: `scripts/intel-query.sh --human symbols "<keyword>" --repo ori --kind function --limit 10`

### Step 3: Fix the Tool

Make the improvement. Follow existing conventions:
- **Shell scripts**: follow `_common.sh` patterns — `--help`, `--no-color`/`--color`, error handling, exit codes
- **Python scripts**: argparse, clear error messages, `if __name__ == "__main__"`
- **Test harnesses**: clear pass/fail output, exit code reflects success/failure, no silent swallowing of errors

### Step 4: Use the Improved Tool

Now use the improved tool for your original task. The improvement must actually solve the friction that triggered it.

### Step 5: Update Documentation (MANDATORY — Zero Exceptions)

**Every tool change requires a doc sync pass.** This is not optional, even for "minor" flag additions.

1. **Canonical surface MUST be updated** — see §Documentation Surfaces for the mapping. If the tool is in `diagnostics/`, update `diagnostics/README.md`. If commonly-needed in `scripts/` or root, update `CLAUDE.md` §Commands. If maintenance/migration, ensure `--help` is complete.
2. **Secondary surfaces — best-effort** — if the tool is already listed in a secondary surface (e.g., `.claude/rules/diagnostic.md` quick table), update it there too. But secondary surfaces are not mandatory sync targets.
3. **`--help` MUST reflect the change** — the script's own help output is part of the doc surface.
4. **New commonly-needed tools MUST appear in their canonical doc index** — commonly-needed scripts that only have `--help` but aren't in CLAUDE.md or `diagnostics/README.md` are invisible to future sessions. Maintenance/migration scripts whose canonical surface IS their `--help` do not need doc index entries.

**Verification**: After updating docs, re-read the doc surface and confirm the new tool/flag appears correctly. A doc update that's syntactically wrong (broken table row, missing column) is worse than no update.

## Anti-Patterns (BANNED)

The canonical list of banned tooling workarounds lives in `CLAUDE.md` §"ALWAYS improve tooling, NEVER work around it". All of those anti-patterns trigger this skill. In summary: any action that works *around* a tool's limitation instead of *fixing* the tool is banned — piping/grepping output, running multiple commands for one answer, manually interpreting output, ignoring wrong output, writing one-off scripts, or saying "the tool doesn't support X" and moving on.

## Quality Standards for Tool Improvements

Every tool improvement must meet these standards:

1. **`--help` works** and documents all flags
2. **Error messages are clear** — say what went wrong and what to do about it
3. **Exit codes are correct** — 0 for success, non-zero for failure
4. **Output is structured** — important info first, details available via `--verbose`
5. **Idempotent** — safe to run multiple times
6. **Tested** — if adding a flag, verify it works before moving on
7. **Consistent** — follows the same conventions as sibling scripts

## Documentation Awareness Verification

This is the third pillar of the skill, alongside reactive improvements (§Workflow) and reflective retrospectives (§Retrospective Mode). It catches the failure mode where tools exist but nobody uses them because the docs don't mention them.

### Three Checks

**Check 1: Undocumented Tools (doc-exists gap)**
Verify that every tool in `diagnostics/`, `scripts/`, and root has an entry in its canonical doc surface. This is a structural audit — it doesn't require any session context.

How to run:
1. List all **tracked** (git-tracked) tool scripts in `diagnostics/`, `scripts/`, and root — this includes `*.sh` AND `*.py` utilities (excluding `_common.sh`, test fixtures, `__pycache__`, and untracked scratch files)
2. For each, check whether it appears in its canonical doc surface (see §Documentation Surfaces). Remember: maintenance/migration scripts only need `--help`, not a CLAUDE.md entry
3. Any commonly-needed tool missing from its canonical doc index is a gap — add it NOW

When to run: at every section-close sweep (§Retrospective Mode §Section-Close Sweep step 1.5 — added below), and whenever you notice a tool that isn't documented.

**Check 2: Awareness Gap (session-skipped-tool)**
During retrospectives, ask: "Did I manually do something during this session that an existing tool already automates, but I didn't know the tool existed?" This catches the invisible cost of incomplete docs — you can't use what you don't know about.

Indicators:
- You wrote a one-off shell command that a `diagnostics/` script already handles
- You piped/grepped output that a script's `--flag` already filters
- You ran `ORI_*` env vars manually when a diagnostic script wraps them
- You read a script's source code to discover flags that should have been in `CLAUDE.md`

When discovered: (1) Use the existing tool going forward, (2) Add it to `CLAUDE.md` §Commands or the appropriate doc surface so future sessions find it, (3) Record the gap in the retrospective output.

**Check 3: New Tool Without Docs (creation-sync gap)**
After creating or significantly modifying a tool during this session, verify:
1. The tool has `--help` that documents all flags
2. The tool appears in its canonical doc surface (see §Documentation Surfaces — ONE surface, not multiple)
3. If the tool is commonly needed during interactive sessions, it appears in `CLAUDE.md` §Commands
4. If the tool is a diagnostic script, `diagnostics/README.md` is the single canonical home — update `.claude/rules/diagnostic.md` only if the tool is already listed there (best-effort, not mandatory)

When to run: at every commit that adds or modifies a tool script. Step 5 of the Workflow (§Update Documentation) enforces this for reactive improvements; this check also applies during retrospectives for tools added earlier in the session.

### Anti-Patterns (Documentation-Specific)

- **"The tool has `--help`, that's enough documentation."** — For commonly-needed tools, `--help` alone is insufficient — if the tool isn't listed in its canonical doc surface, nobody will discover it. However, for maintenance/migration scripts (see §Documentation Surfaces), `--help` IS the canonical surface and no doc index entry is required.
- **"I'll document it later when the tool stabilizes."** — BANNED. An undocumented tool is invisible. Document it now with what it does today; update the docs when it changes.
- **"It's only used by one script / one workflow."** — Irrelevant. If it lives in `diagnostics/` or `scripts/`, it's a project tool and needs documentation. Internal helpers that are genuinely private should be functions inside the calling script, not standalone scripts.
- **"CLAUDE.md is already long, I don't want to add more."** — CLAUDE.md §Commands should list key scripts with brief descriptions, not full reference docs. One line per tool. The canonical surface (`diagnostics/README.md` or the script's own `--help`) carries the full reference.

## Retrospective Mode

Retrospective mode is **reflective, not reactive**. It runs even when nothing felt blocked. The premise: small frictions normalize and disappear from memory within hours, so you must capture them while the debugging journey is fresh.

It has **three granularities**, fired at different boundaries:

| Granularity | Trigger | Scope | Purpose |
|---|---|---|---|
| **Per-subsection** (PRIMARY) | Immediately after a subsection's tasks are all `[x]` and the subsection is marked `complete` — BEFORE moving to the next subsection | Just THIS subsection's debugging journey | Fresh-pain capture. The main mechanism by which tooling grows. |
| **Bug-fix close** | After `/fix-bug` completion checklist step 8 — AFTER TPR and hygiene are clean | The fix section's root-cause analysis and debugging journey | Bug fixes are the richest source of tooling gaps — you've just fought the diagnostic surface during root-cause analysis. Mandated by `/fix-bug` Phase 5. |
| **Section-close** (SWEEP) | At the end of a full section, after `/tpr-review` and `/impl-hygiene-review` are clean | The section as an integrated whole | Verify per-subsection and bug-fix retrospectives ran. Add only NEW items from cross-cutting patterns invisible at finer scope. Safety net, not main capture. |

**Why three granularities:** the per-subsection retrospective is where almost all real value lives — it fires while you can still remember which `dbg!` you added to chase what symptom in which file. The bug-fix close retrospective captures tooling gaps from root-cause analysis — a different debugging shape than subsection work (more diagnostic scripts, more tracing, more ad-hoc instrumentation). The section-close sweep exists because some friction is only visible *after integration*: e.g., "I noticed I ran the same 3 commands every time I switched between subsections .2 and .4" or "the test failure messages from .1 only became confusing once they collided with the new variants from .3." Without the sweep, those cross-cutting patterns get lost. Without the per-subsection and bug-fix captures, *everything* gets lost.

### Per-Subsection Workflow (PRIMARY — fires after every subsection)

When invoked immediately after marking a subsection complete:

1. **Reconstruct THIS subsection's debugging journey.** Look at exactly what you did inside this subsection's task block. Ask:
   - Which `diagnostics/` scripts did I run for this subsection? How many times? Did I have to pipe/grep/manually parse output?
   - Which command sequences did I repeat across this subsection's tasks? (e.g., "build, run with `ORI_DUMP_AFTER_ARC=1`, grep for the function, eyeball the IR")
   - Where did I add `dbg!` / `eprintln!` / `tracing::debug!` while implementing this subsection? What was each one looking for?
   - Where did I stare at output for >30 seconds trying to understand it?
   - Which test failures gave unhelpful messages — "expected X, got Y" without context about *why*?
   - Did I write any one-off shell incantations a script should own permanently?
   - **[DOC AWARENESS]** Did I manually do something that an existing tool already handles — but I didn't know the tool existed? (Check: `diagnostics/README.md` for the full inventory)
   - **[DOC AWARENESS]** Did I discover a tool mid-session by reading source code or stumbling on it, rather than finding it in `CLAUDE.md`? If so, it needs a doc surface entry.
   - **[DOC SYNC]** Did I create or modify any scripts during this subsection? Are they documented in all applicable surfaces (see §Documentation Surfaces)?

2. **Forward-look as well as back-look.** Ask: "If someone hits a regression in this exact code path next month, what tool/log/diagnostic would shorten their debugging session by 10 minutes?"

3. **List concrete improvement candidates** (see "Candidate Format" below).

4. **Filter brutally** (see "Filter Criteria" below).

5. **Implement accepted improvements NOW** — zero deferral. The improvement IS subsection close-out work. Do not start the next subsection until improvements are committed.

6. **Commit improvements separately** via `/commit-push` with a message like `build(diagnostics): add --per-block flag to codegen-audit.sh — surfaced by {plan}/section-NN.M retrospective`. Tool improvements have their own provenance and reviewability — never bundled into the subsection's implementation commit. Use a valid conventional-commit type per `/commit-push` (e.g., `build` for dev scripts, `test` for test-harness, `chore` for general tooling, `ci` for CI, `docs` for tool docs).

7. **Verify the improvement actually solves the friction** by re-running the original workflow against the improved tool. If it doesn't noticeably help, iterate until it does.

8. **Update documentation** if the tool gained new flags — `CLAUDE.md` (if listed), the script's `--help`, `diagnostics/README.md`.

**Output of a per-subsection retrospective is one of two states:**
- **Improvements made** — list each tool changed + the friction it removes, with commit hashes
- **No gaps** — document the negative finding briefly: "Retrospective: no tooling gaps — subsection {NN}.M relied entirely on existing scripts X, Y which were sufficient." The negative finding is itself the deliverable — it proves you actually looked, not that you skipped.

**Where to persist the outcome:** Record in the owning plan's subsection completion notes (e.g., append to the `[x]` line or add a sub-bullet: `Tooling retrospective: improvements in commits {hashes}` or `Tooling retrospective: no gaps`). This is the durable record that the section-close sweep checks in step 1. If no owning plan exists (ad-hoc work), record in the commit message body of the last improvement commit.

### Bug-Fix Close Workflow (fires after every `/fix-bug` completion)

When invoked at `/fix-bug` Phase 5 completion checklist step 8, AFTER TPR and hygiene are clean:

1. **Reconstruct THIS bug's debugging journey.** The scope is the fix section (`fix-BUG-XX-NNN.md`), not a plan subsection. Ask:
   - Which `diagnostics/` scripts did I run during root-cause analysis? Did any produce confusing or incomplete output?
   - Where did I add `dbg!` / `tracing::debug!` to chase the root cause? What was each one looking for? Could a script flag have surfaced the same information?
   - Where did the original failure message or test output fail to explain *why* something was wrong?
   - Did the TDD matrix writing reveal missing test helpers or assertion utilities?
   - Did I manually run `ORI_CHECK_LEAKS=1`, `ORI_TRACE_RC=1`, or other environment flags that a script should orchestrate?
   - **[DOC AWARENESS]** Did I miss an existing tool that would have shortened root-cause analysis? (Check `diagnostics/README.md` for the full inventory — was there a script I didn't know about?)
   - **[DOC SYNC]** Did I create any new diagnostic scripts or test utilities during this fix? Are they documented?

2. **Forward-look.** Ask: "If this same bug class recurs in a different code path, what tool/flag/diagnostic would make root-cause analysis 10 minutes faster?"

3. **List, filter, implement, commit, verify, document** — same rules as per-subsection (zero deferral, separate commits per `/commit-push`, re-run verification, update docs).

**Output:** Record in the fix section's completion checklist: either "Tooling retrospective: improvements in commits {hashes}" or "Tooling retrospective: no gaps — root-cause analysis used scripts X, Y which were sufficient." This is the durable record that the section-close sweep verifies.

### Section-Close Sweep Workflow (SAFETY NET — fires once per section)

When invoked at the end of a section, after `/tpr-review` and `/impl-hygiene-review` are clean:

1. **Verify per-subsection and bug-fix retrospectives actually ran.** For each subsection in this section, confirm there is either an "Improvements made" entry (with commits) or a documented "no gaps" negative finding. For each `/fix-bug` completed during this section, confirm the fix section's completion checklist has a "Tooling retrospective" entry. If any retrospective was skipped, **STOP** — go back and run it now. The sweep cannot substitute for the missing captures; it can only catch what they missed.

   - **Documentation Awareness audit (§Documentation Awareness Verification Check 1).** List all tracked tool scripts in `diagnostics/`, `scripts/`, and root (both `*.sh` and `*.py`). For each, verify it appears in its canonical doc surface per §Documentation Surfaces. Fix any gaps NOW. Also verify that any tools *created during this section* have complete doc coverage (Check 3). This is the structural audit that catches drift between the actual tool inventory and the documentation.

2. **Look for cross-item patterns invisible at finer granularity:**
   - Did I run the same command sequence transitioning between different subsections or between subsection work and bug-fix work? (e.g., "every time I moved from a typeck change to a codegen change, I had to manually clear the salsa cache and re-run two diagnostic scripts")
   - Did test failures from *interactions between* items (subsections, bug fixes) give worse messages than failures *within* a single item?
   - Did integration steps require mentally cross-referencing files that no tool combined?
   - Did any forward-looking instrumentation become obvious only after seeing all subsections and bug fixes together?

3. **List concrete improvement candidates** for items the per-subsection and bug-fix captures could not have surfaced (see "Candidate Format" below).

4. **Filter brutally** — and bias toward NOT duplicating per-subsection or bug-fix work. If a candidate could have been captured at finer granularity but wasn't, that's a process failure (go fix the missed retrospective), not a sweep finding.

5. **Implement, commit, verify, document** — same rules as per-subsection (zero deferral, separate commits, re-run verification).

**Output of a section-close sweep is one of two states:**
- **Cross-cutting improvements made** — list each tool changed + the integration pattern it addresses
- **No new gaps beyond finer-grained captures** — "Section-close sweep: per-subsection and bug-fix retrospectives covered everything; no cross-cutting patterns required new tooling." This is a perfectly valid (and common) outcome when finer-grained captures were thorough.

**Where to persist the outcome:** Record at the bottom of the section's plan file (e.g., as a `## Tooling Sweep` block or appended to the section's completion notes). The sweep outcome is the final verification that all finer-grained captures ran.

### Candidate Format (all granularities)

For each candidate, articulate:
- **Tool**: which script/harness needs the change (e.g., `diagnostics/codegen-audit.sh`)
- **Gap**: what's missing or painful (e.g., "doesn't show RC balance per basic block, only per function")
- **Improvement**: the specific change (e.g., "add `--per-block` flag")
- **Payoff**: how it would have shortened *this* item's work (subsection, bug fix, or section), or how it sharpens future debugging
- **Source**: which subsection (`{NN}.M`), bug fix (`BUG-XX-NNN`), or cross-pattern surfaced it — used in commit messages

### Filter Criteria (all granularities)

Not every small annoyance becomes a tool change. Apply this filter:

- **DO improve** if the friction would recur: same workflow on similar bugs or subsections, same script run across items, same output format misread by future implementers
- **DO improve** if the manual workaround is non-obvious — meaning it relies on tribal knowledge nobody documented
- **DO improve** if a 10-line script change saves 5+ minutes per future debugging session
- **DO NOT improve** if the friction was a one-off due to unique content (subsection or bug fix) with no recurring pattern
- **DO NOT improve** if the "fix" would add complexity to a stable, simple tool for a marginal gain

### Anti-Patterns Specific to Retrospective Mode

- **"Nothing was painful, skipping retrospective."** — The retrospective is mandatory at every subsection close, bug-fix close, and section-close sweep. The fact that nothing *felt* painful is exactly why the look-back is needed: small frictions become invisible. Force yourself to enumerate the actual commands run; gaps will surface. If genuinely none, the negative-finding documentation IS the deliverable.
- **"I'll batch all my retrospectives at section close instead."** — BANNED. This is exactly the failure mode that motivated splitting into per-subsection granularity. By section close you have already forgotten the pain points from the early subsections. The section-close sweep can ONLY catch cross-cutting patterns; it cannot reconstruct per-item friction.
- **"I'll add a TODO comment for the tool change."** — Banned. Either implement the improvement now or don't claim it's needed. Comments are not tracking.
- **"The improvement would touch 3 scripts, that's too much."** — CLAUDE.md correctness rule applies: scope, effort, and complexity are irrelevant. If the right improvement crosses scripts, that IS the improvement.
- **"This is a one-off, no future debugging session will need it."** — Be honest. If you genuinely can't articulate a recurring use case, skip it. But "one-off" is often a rationalization — most debugging patterns recur.
- **Combining tooling improvements into the item's main commit** (subsection or bug fix). — Separate commits keep provenance clean and let `/improve-tooling` retrospectives be reviewed independently of feature/fix work.
- **Section-close sweep being used as the primary capture.** — If your section-close sweep produces 8 improvements while the per-subsection and bug-fix retrospectives produced 0, the finer-grained captures were skipped. Sweep findings should be small in number (often zero) and explicitly cross-cutting.

### Why Retrospective Mode Exists

The reactive auto-trigger catches friction *as it happens*, but it has blind spots:
- Workflows that are tedious but not blocking ("I always have to run these 3 commands in a row") never trigger reactive mode because no single moment is painful enough
- Output that's *interpretable but slow* never triggers — you read it, you continue, the friction normalizes
- Forward-looking instrumentation ("logging that doesn't exist yet but would help future debugging") cannot be reactive by definition

Retrospective mode covers all three. The per-subsection cadence ensures the capture happens while memory is hot; the bug-fix close captures root-cause analysis friction (the richest source of tooling gaps); the section-close sweep ensures cross-cutting patterns aren't lost. Together, they're the difference between a tooling suite that grows by accident and one that grows on purpose.

## Examples

**Bad**: "dual-exec-verify.sh doesn't check for RC leaks, so I'll manually run `ORI_CHECK_LEAKS=1` after it"
**Good**: Add `--leak-check` flag to `dual-exec-verify.sh` that sets `ORI_CHECK_LEAKS=1` and reports results

**Bad**: "test-all.sh output is too long to scan, let me grep for FAIL"
**Good**: Add a summary section to `test-all.sh` that lists all failures at the end

**Bad**: "I need to compare IR before and after my change, let me manually diff two ir-dump.sh runs"
**Good**: `ir-diff.sh` already exists — use it. If it's missing a feature you need, improve it.

**Bad**: "This script doesn't handle the case where the file doesn't exist"
**Good**: Add existence checks with clear error messages: `echo "Error: $file not found" >&2; exit 1`

**Bad**: "I manually ran `ORI_DUMP_AFTER_ARC=1 cargo run -- build file.ori | grep my_func` to see the ARC IR"
**Good**: `arc-dump.sh` already does this — use `arc-dump.sh file.ori --function my_func`. If you didn't know it existed, add it to `CLAUDE.md` §Commands so the next session finds it.

**Bad**: "I wrote a new `scripts/check-foo.sh` and it works great" (but it's not documented anywhere)
**Good**: After writing the script, add `--help` and update `CLAUDE.md` §Commands if it's commonly needed during interactive sessions. If it's a maintenance/migration script, `--help` alone is sufficient.

**Bad**: "I wrote a new diagnostic script but only put it in `scripts/`"
**Good**: Diagnostic scripts MUST live in `diagnostics/`, not `scripts/`. Add it to `diagnostics/`, add `--help`, and update `diagnostics/README.md` (the single canonical home for diagnostic docs).

**Bad**: "Retrospective: no gaps" (but you manually ran 5 env-var incantations during the subsection)
**Good**: Check `diagnostics/README.md` for the full tool inventory. If any of those 5 manual incantations match an existing script's flags, that's an awareness gap — document the tool in `CLAUDE.md` so future sessions find it.
