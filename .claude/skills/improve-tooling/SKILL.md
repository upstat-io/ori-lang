---
name: improve-tooling
description: "AUTO-TRIGGER: Improve testing, diagnostic, debugging, or developer tooling. TRIGGER when: (1) a script in diagnostics/ or scripts/ produces confusing output, missing information, or wrong results, (2) test-all.sh, clippy-all.sh, or any test harness has gaps, missing coverage, or unclear failure output, (3) dual-exec-verify.sh, diagnose-aot.sh, or any diagnostic script doesn't cover a case you need, (4) you work around a tool limitation instead of fixing the tool, (5) you notice a script is missing --help, error handling, or useful flags, (6) you manually do something a script should automate. DO NOT TRIGGER for: normal tool usage that works correctly, or one-off ad-hoc commands."
---

# Improve Tooling

**ABSOLUTE RULE: Never work around deficient tooling. Fix the tool.**

When you encounter friction, gaps, or deficiencies in any developer tooling — testing scripts, diagnostic scripts, build scripts, or any automation — you MUST improve the tool rather than working around it. The tool improvement IS the work.

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

## Examples

**Bad**: "dual-exec-verify.sh doesn't check for RC leaks, so I'll manually run `ORI_CHECK_LEAKS=1` after it"
**Good**: Add `--leak-check` flag to `dual-exec-verify.sh` that sets `ORI_CHECK_LEAKS=1` and reports results

**Bad**: "test-all.sh output is too long to scan, let me grep for FAIL"
**Good**: Add a summary section to `test-all.sh` that lists all failures at the end

**Bad**: "I need to compare IR before and after my change, let me manually diff two ir-dump.sh runs"
**Good**: `ir-diff.sh` already exists — use it. If it's missing a feature you need, improve it.

**Bad**: "This script doesn't handle the case where the file doesn't exist"
**Good**: Add existence checks with clear error messages: `echo "Error: $file not found" >&2; exit 1`
