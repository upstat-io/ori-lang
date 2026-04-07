---
name: improve-tooling
description: "AUTO-TRIGGER: Improve testing, diagnostic, debugging, or developer tooling. TRIGGER when: (1) a script in diagnostics/ or scripts/ produces confusing output, missing information, or wrong results, (2) test-all.sh, clippy-all.sh, or any test harness has gaps, missing coverage, or unclear failure output, (3) dual-exec-verify.sh, diagnose-aot.sh, or any diagnostic script doesn't cover a case you need, (4) you work around a tool limitation instead of fixing the tool, (5) you notice a script is missing --help, error handling, or useful flags, (6) you manually do something a script should automate, (7) RETROSPECTIVE — PER SUBSECTION (primary): invoked immediately after marking a plan subsection complete (e.g., {NN}.1, {NN}.2) to look back at THAT subsection's debugging journey while pain points are still fresh, (8) RETROSPECTIVE — SECTION CLOSE (sweep): invoked at the end of a roadmap/plan section as an integration safety net that verifies per-subsection retrospectives ran and adds only NEW items from cross-subsection patterns. DO NOT TRIGGER for: normal tool usage that works correctly, or one-off ad-hoc commands."
---

# Improve Tooling

**ABSOLUTE RULE: Never work around deficient tooling. Fix the tool.**

When you encounter friction, gaps, or deficiencies in any developer tooling — testing scripts, diagnostic scripts, build scripts, or any automation — you MUST improve the tool rather than working around it. The tool improvement IS the work.

**Tooling grows organically.** You cannot predict every use case ahead of time. The way the diagnostic suite gets sharp is by ratcheting it up by one improvement after every subsection, every bug fix, every debugging session — guided by what was *actually* painful, not what was imagined to be painful. This skill has two trigger modes: **reactive** (mid-task friction, the original auto-trigger) and **reflective** (post-subsection and post-section retrospective — see Retrospective Mode below).

**Pain memory decays fast.** This is why retrospectives must fire at the smallest natural unit of work, not at section close. By the time you've finished six subsections plus TPR plus hygiene review, the friction from subsection `.1` is days old and three reviews ago — you have already smoothed over it. Retrospective Mode therefore has TWO granularities: per-subsection (the primary capture mechanism, run while the journey is fresh) and section-close (an integration sweep that catches cross-subsection patterns invisible at per-item scope).

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

## Tooling Scope

These are the tools you own and must improve:

| Category | Location | Examples |
|----------|----------|----------|
| **Test harnesses** | `./test-all.sh`, `./clippy-all.sh`, `./fmt-all.sh`, `./build-all.sh` | Test coverage gaps, unclear failure output, missing test categories |
| **Diagnostic scripts** | `diagnostics/` | `diagnose-aot.sh`, `dual-exec-verify.sh`, `ir-dump.sh`, `rc-stats.sh`, `codegen-audit.sh`, `valgrind-aot.sh` |
| **Build/release scripts** | `scripts/` | `bump-build.sh`, `sync-version.sh`, `release.sh`, `perf-baseline.sh`, `cow-benchmark.sh` |
| **Test utilities** | `scripts/regen_expected.py`, `scripts/extract_tests.py` | Missing features, poor error messages |
| **Diagnostic common** | `diagnostics/_common.sh` | Shared helpers, color output, `--help` generation |
| **LLVM test harness** | `./llvm-test.sh` | Missing test patterns, unclear failure reporting |

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

### Step 3: Fix the Tool

Make the improvement. Follow existing conventions:
- **Shell scripts**: follow `_common.sh` patterns — `--help`, `--no-color`/`--color`, error handling, exit codes
- **Python scripts**: argparse, clear error messages, `if __name__ == "__main__"`
- **Test harnesses**: clear pass/fail output, exit code reflects success/failure, no silent swallowing of errors

### Step 4: Use the Improved Tool

Now use the improved tool for your original task. The improvement must actually solve the friction that triggered it.

### Step 5: Update Documentation

If the tool gained new flags or capabilities:
- Update `CLAUDE.md` if the tool is listed there
- Update the tool's `--help` output
- Update `diagnostics/README.md` if it's a diagnostic script

## Anti-Patterns (BANNED)

These are all forms of "working around the tool" — they trigger this skill:

- **Piping and grepping** script output to find what you need → fix the script's output format
- **Running 3 commands** to get one answer → make a script that does all three
- **Manually interpreting** IR/RC/codegen output → add a `--summary` or `--check` flag
- **Copy-pasting** output between tools → add piping support or combine the tools
- **Ignoring** a tool's wrong output and doing the check mentally → fix the tool
- **Writing a one-off script** for something a permanent tool should do → extend the permanent tool
- **Saying "the tool doesn't support X"** and moving on → add support for X

## Quality Standards for Tool Improvements

Every tool improvement must meet these standards:

1. **`--help` works** and documents all flags
2. **Error messages are clear** — say what went wrong and what to do about it
3. **Exit codes are correct** — 0 for success, non-zero for failure
4. **Output is structured** — important info first, details available via `--verbose`
5. **Idempotent** — safe to run multiple times
6. **Tested** — if adding a flag, verify it works before moving on
7. **Consistent** — follows the same conventions as sibling scripts

## Retrospective Mode

Retrospective mode is **reflective, not reactive**. It runs even when nothing felt blocked. The premise: small frictions normalize and disappear from memory within hours, so you must capture them while the debugging journey is fresh.

It has **two granularities**, fired at different boundaries:

| Granularity | Trigger | Scope | Purpose |
|---|---|---|---|
| **Per-subsection** (PRIMARY) | Immediately after a subsection's tasks are all `[x]` and the subsection is marked `complete` — BEFORE moving to the next subsection | Just THIS subsection's debugging journey | Fresh-pain capture. The main mechanism by which tooling grows. |
| **Section-close** (SWEEP) | At the end of a full section, after `/tpr-review` and `/impl-hygiene-review` are clean | The section as an integrated whole | Verify per-subsection retrospectives ran. Add only NEW items from cross-subsection patterns invisible at per-item scope. Safety net, not main capture. |

**Why two granularities:** the per-subsection retrospective is where almost all real value lives — it fires while you can still remember which `dbg!` you added to chase what symptom in which file. The section-close sweep exists because some friction is only visible *after integration*: e.g., "I noticed I ran the same 3 commands every time I switched between subsections .2 and .4" or "the test failure messages from .1 only became confusing once they collided with the new variants from .3." Without the sweep, those cross-cutting patterns get lost. Without the per-subsection capture, *everything* gets lost.

### Per-Subsection Workflow (PRIMARY — fires after every subsection)

When invoked immediately after marking a subsection complete:

1. **Reconstruct THIS subsection's debugging journey.** Look at exactly what you did inside this subsection's task block. Ask:
   - Which `diagnostics/` scripts did I run for this subsection? How many times? Did I have to pipe/grep/manually parse output?
   - Which command sequences did I repeat across this subsection's tasks? (e.g., "build, run with `ORI_DUMP_AFTER_ARC=1`, grep for the function, eyeball the IR")
   - Where did I add `dbg!` / `eprintln!` / `tracing::debug!` while implementing this subsection? What was each one looking for?
   - Where did I stare at output for >30 seconds trying to understand it?
   - Which test failures gave unhelpful messages — "expected X, got Y" without context about *why*?
   - Did I write any one-off shell incantations a script should own permanently?

2. **Forward-look as well as back-look.** Ask: "If someone hits a regression in this exact code path next month, what tool/log/diagnostic would shorten their debugging session by 10 minutes?"

3. **List concrete improvement candidates** (see "Candidate Format" below).

4. **Filter brutally** (see "Filter Criteria" below).

5. **Implement accepted improvements NOW** — zero deferral. The improvement IS subsection close-out work. Do not start the next subsection until improvements are committed.

6. **Commit improvements separately** via `/commit-push` with a message like `build(diagnostics): add --per-block flag to codegen-audit.sh — surfaced by {plan}/section-NN.M retrospective`. Tool improvements have their own provenance and reviewability — never bundled into the subsection's implementation commit. **Use a valid conventional-commit type** — `build` for dev scripts / build infra, `test` for test-harness changes, `chore` for general tooling, `ci` for CI config, `docs` for tool docs. Do NOT use `tools(...)` as a type — the pre-commit hook (`lefthook commit-msg`) enforces the standard set (`feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert`) and will reject any other type outright. Pick the type that reflects the change's actual nature, not a made-up category.

7. **Verify the improvement actually solves the friction** by re-running the original workflow against the improved tool. If it doesn't noticeably help, iterate until it does.

8. **Update documentation** if the tool gained new flags — `CLAUDE.md` (if listed), the script's `--help`, `diagnostics/README.md`.

**Output of a per-subsection retrospective is one of two states:**
- **Improvements made** — list each tool changed + the friction it removes, with commit hashes
- **No gaps** — document the negative finding briefly: "Retrospective: no tooling gaps — subsection {NN}.M relied entirely on existing scripts X, Y which were sufficient." The negative finding is itself the deliverable — it proves you actually looked, not that you skipped.

### Section-Close Sweep Workflow (SAFETY NET — fires once per section)

When invoked at the end of a section, after `/tpr-review` and `/impl-hygiene-review` are clean:

1. **Verify per-subsection retrospectives actually ran.** For each subsection in this section, confirm there is either an "Improvements made" entry (with commits) or a documented "no gaps" negative finding. If any subsection skipped its retrospective, **STOP** — go back and run it now. The sweep cannot substitute for the missing per-subsection captures; it can only catch what they missed.

2. **Look for cross-subsection patterns invisible at per-item scope:**
   - Did I run the same command sequence transitioning between *different* subsections? (e.g., "every time I moved from a typeck change to a codegen change, I had to manually clear the salsa cache and re-run two diagnostic scripts")
   - Did test failures from *interactions between* subsections give worse messages than failures *within* a subsection?
   - Did integration steps require mentally cross-referencing files that no tool combined?
   - Did any forward-looking instrumentation become obvious only after seeing all subsections together?

3. **List concrete improvement candidates** for items the per-subsection captures could not have surfaced (see "Candidate Format" below).

4. **Filter brutally** — and bias toward NOT duplicating per-subsection work. If a candidate could have been captured per-subsection but wasn't, that's a process failure (go fix the missed retrospective), not a sweep finding.

5. **Implement, commit, verify, document** — same rules as per-subsection (zero deferral, separate commits, re-run verification).

**Output of a section-close sweep is one of two states:**
- **Cross-cutting improvements made** — list each tool changed + the integration pattern it addresses
- **No new gaps beyond per-subsection captures** — "Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling." This is a perfectly valid (and common) outcome when per-subsection captures were thorough.

### Candidate Format (both granularities)

For each candidate, articulate:
- **Tool**: which script/harness needs the change (e.g., `diagnostics/codegen-audit.sh`)
- **Gap**: what's missing or painful (e.g., "doesn't show RC balance per basic block, only per function")
- **Improvement**: the specific change (e.g., "add `--per-block` flag")
- **Payoff**: how it would have shortened *this* subsection/section's work, or how it sharpens future debugging
- **Source**: which subsection (`{NN}.M`) or which cross-pattern surfaced it — used in commit messages

### Filter Criteria (both granularities)

Not every small annoyance becomes a tool change. Apply this filter:

- **DO improve** if the friction would recur: same workflow on similar bugs, same script run by other subsections, same output format misread by future implementers
- **DO improve** if the manual workaround is non-obvious — meaning it relies on tribal knowledge nobody documented
- **DO improve** if a 10-line script change saves 5+ minutes per future debugging session
- **DO NOT improve** if the friction was a one-off due to unique subsection content with no recurring pattern
- **DO NOT improve** if the "fix" would add complexity to a stable, simple tool for a marginal gain

### Anti-Patterns Specific to Retrospective Mode

- **"Nothing was painful, skipping retrospective."** — The retrospective is mandatory at every subsection close (and the section-close sweep). The fact that nothing *felt* painful is exactly why the look-back is needed: small frictions become invisible. Force yourself to enumerate the actual commands run; gaps will surface. If genuinely none, the negative-finding documentation IS the deliverable.
- **"I'll batch all my retrospectives at section close instead."** — BANNED. This is exactly the failure mode that motivated splitting into per-subsection granularity. By section close you have already forgotten the pain points from the early subsections. The section-close sweep can ONLY catch cross-cutting patterns; it cannot reconstruct per-item friction.
- **"I'll add a TODO comment for the tool change."** — Banned. Either implement the improvement now or don't claim it's needed. Comments are not tracking.
- **"The improvement would touch 3 scripts, that's too much."** — CLAUDE.md correctness rule applies: scope, effort, and complexity are irrelevant. If the right improvement crosses scripts, that IS the improvement.
- **"This is a one-off, no future subsection will need it."** — Be honest. If you genuinely can't articulate a recurring use case, skip it. But "one-off" is often a rationalization — most debugging patterns recur.
- **Combining tooling improvements into the subsection's main commit.** — Separate commits keep provenance clean and let `/improve-tooling` retrospectives be reviewed independently of feature work.
- **Section-close sweep being used as the primary capture.** — If your section-close sweep produces 8 improvements while the per-subsection retrospectives produced 0, the per-subsection captures were skipped. Sweep findings should be small in number (often zero) and explicitly cross-cutting.

### Why Retrospective Mode Exists

The reactive auto-trigger catches friction *as it happens*, but it has blind spots:
- Workflows that are tedious but not blocking ("I always have to run these 3 commands in a row") never trigger reactive mode because no single moment is painful enough
- Output that's *interpretable but slow* never triggers — you read it, you continue, the friction normalizes
- Forward-looking instrumentation ("logging that doesn't exist yet but would help future debugging") cannot be reactive by definition

Retrospective mode covers all three. The per-subsection cadence ensures the capture happens while memory is hot; the section-close sweep ensures cross-cutting patterns aren't lost. Together, they're the difference between a tooling suite that grows by accident and one that grows on purpose.

## Examples

**Bad**: "dual-exec-verify.sh doesn't check for RC leaks, so I'll manually run `ORI_CHECK_LEAKS=1` after it"
**Good**: Add `--leak-check` flag to `dual-exec-verify.sh` that sets `ORI_CHECK_LEAKS=1` and reports results

**Bad**: "test-all.sh output is too long to scan, let me grep for FAIL"
**Good**: Add a summary section to `test-all.sh` that lists all failures at the end

**Bad**: "I need to compare IR before and after my change, let me manually diff two ir-dump.sh runs"
**Good**: `ir-diff.sh` already exists — use it. If it's missing a feature you need, improve it.

**Bad**: "This script doesn't handle the case where the file doesn't exist"
**Good**: Add existence checks with clear error messages: `echo "Error: $file not found" >&2; exit 1`
