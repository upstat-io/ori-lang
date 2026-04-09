---
section: "08"
title: "Spec & Docs"
status: in-progress
goal: "Track and resolve all known spec/documentation bugs"
sections: []
---

# Section 08: Spec & Docs

**Subsystem:** `docs/ori_lang/`, `.claude/rules/`, `.claude/commands/`, `plans/`

Bugs in the language specification, EBNF grammar, design docs, CLAUDE.md, rule files, command/skill definitions, and plan structure.

---

## Open Bugs

- [ ] `[BUG-08-011][medium]` **lint-command-skill-pairs.sh: add a tool that classifies and validates .claude/commands/ ↔ .claude/skills/ pairs by pattern** — found by dual-tpr-gemini §07.1 retrospective.
  Repro: currently, 2 command-skill overlap pairs exist: `tp-help` uses thin-pointer pattern (skill canonical, command = thin pointer; consolidated in dual-tpr-gemini §07.1 on 2026-04-08) and `review-work` uses parallel-workflow pattern (command = Claude self-reviews directly, skill = dual-source codex wrapper; both canonical for different use cases per 00-overview line 32 of plans/dual-tpr-gemini/). No tool validates these pairs for drift. If someone re-duplicates operational content in `tp-help.md` (breaking the thin-pointer contract) or swaps `review-work.md` semantics (breaking the parallel-workflow contract), there is no automated check to catch it.
  Impact: low-to-medium. No immediate breakage — both existing pairs are stable. But future SSOT drift between command-skill pairs would be invisible until a manual review catches it. This is the class of bug R10 from the dual-tpr-gemini plan represents; §07.1 resolved the specific `tp-help` instance but did NOT install a permanent regression guard.
  Suggested fix: create `.claude/skills/dual-tpr/scripts/lint-command-skill-pairs.sh` (location TBD) that:
  1. Enumerates all overlap pairs (for each `.claude/commands/X.md`, check if `.claude/skills/X/SKILL.md` exists).
  2. Classifies each pair as `thin-pointer` (command body < 30 lines, references skill path, NO operational markers like `codex exec`/`python3`/`Step N:`/`run_in_background`) or `parallel-workflow` (both substantive, command header explicitly declares parallel relationship) or `unknown` (drift finding).
  3. Validates each classified pair against its pattern's rules.
  4. Exits non-zero if any pair fails classification or validation.
  Alternative: drive classification from explicit frontmatter metadata (`overlap_pattern: thin-pointer` or `overlap_pattern: parallel-workflow`), trivial to classify but requires updating both existing pairs' frontmatter.
  Subsystem: .claude/ tooling (location TBD — likely .claude/skills/dual-tpr/scripts/ or .claude/hooks/)
  Found: 2026-04-08 | Source: continue-roadmap (pre-filed by dual-tpr-gemini §07.1 retrospective)
  Note: Related tool `lint-transport-contract.sh` (implemented in §07.0 retrospective) uses the same dead-code-detection pattern but scoped to transport script args, not command-skill pairs. Active work in `plans/dual-tpr-gemini/` §07.1 touches this area.

- [ ] `[BUG-08-010][high]` **create-plan: add --root/ORI_PLAN_ROOT override for test harnesses** — found by dual-tpr-gemini §07.PRE preflight.
  Repro: `/create-plan` currently writes plan files unconditionally under `plans/<slug>/`. No env var or flag exists to redirect output for test harnesses. Any test that runs `/create-plan` non-destructively either (a) writes persistent artifacts into the repo that must be cleaned by exact path, or (b) collides with an existing plan if the slug is not unique.
  Impact: blocks the preferred "Mode A" execution path of `dual-tpr-gemini` §07.3 Scenario 4, which verifies that `/create-plan`'s 5 internal `/tp-help` call sites still work correctly when `/tp-help` is rewritten for dual-source concatenation output in §07.2. Without the override, Scenario 4 falls back to Mode B (deterministic slug under `plans/` with collision pre-check + exact-path cleanup) which is safe but fragile and leaves no cleanup safety net beyond the pre-check. More broadly: any future test harness that wants to exercise `/create-plan` non-destructively needs this flag — the current single-path design makes `/create-plan` untestable in isolation.
  Suggested fix: add `ORI_PLAN_ROOT` env var (or `--root` flag) to `.claude/skills/create-plan/SKILL.md`. When set, shadow the `plans/` prefix used in Step 10's directory creation with `$ORI_PLAN_ROOT/`. Default behavior unchanged. Touch points: Step 10 "Create Directory Structure" (around line 618) — replace hardcoded `plans/$slug` with `${ORI_PLAN_ROOT:-plans}/$slug`; search the rest of the SKILL.md for other `plans/$slug` references; add a one-line override note in the skill preamble; add a regression test that runs `/create-plan` with `ORI_PLAN_ROOT=/tmp/test-plan-$$` and verifies output is written to the tmpdir, not `plans/`.
  Dependency relationship: `dual-tpr-gemini` §07.3 Scenario 4 reads `plans/dual-tpr-gemini/section-07-scenario4-blocker.txt` at test time to get this BUG-ID and picks Mode A (closed) or Mode B (open). Scenario 4 does NOT stall on this bug — it has a fallback — but Mode A is architecturally preferred because it eliminates the need for exact-path cleanup.
  Subsystem: .claude/skills/create-plan/SKILL.md
  Found: 2026-04-08 | Source: continue-roadmap (pre-filed by dual-tpr-gemini §07.PRE preflight)
  Note: Active work in `plans/dual-tpr-gemini/` §07.PRE and §07.3 touches this area. §07.PRE pre-files this bug as a §07.3 Scenario 4 Mode A prerequisite; §07.3 Scenario 4 reads the assigned BUG-ID from `plans/dual-tpr-gemini/section-07-scenario4-blocker.txt` and picks Mode A or Mode B accordingly.

- [ ] `[BUG-08-008][low]` **classify-review-command.py: flags_with_values lists are incomplete for several wrappers** — found by dual-tpr-gemini Section 04.3 iteration 6 (gemini).
  Repro: review of commit `f027620f`. Per-wrapper `flags_with_values` sets in WRAPPER_SPECS cover the most common flag-value pairs but are not exhaustive. New wrapper flags get added on each iteration as bypasses are discovered. Without comprehensive coverage, FUTURE wrapper flag-value pairs that consume the next token may be misinterpreted as positional args, producing either bypasses or false positives.
  Examples gemini cited (not exhaustive): `sudo --remove-timestamp`, `xargs --process-slot-var`, additional ssh -[Q,W] options, `gdb -p PID -batch`. None are exploitable bypasses today (the existing test suite would catch them) but each is a latent edge case for future shell environments.
  Architectural fix: this is a test-coverage and registry-completeness concern, not a structural defect. Either (a) generate the WRAPPER_SPECS lists from the man page output of each wrapper at build time, or (b) accept the manual list and add a periodic audit task.
  Subsystem: .claude/hooks/classify-review-command.py
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 iteration 6 verification

- [ ] `[BUG-08-009][low]` **verify-hook.sh: missing regression coverage for shell-string fall-through and clustered-flag interactions** — found by dual-tpr-gemini Section 04.3 iteration 6 (codex).
  Repro: review of commit `f027620f`. The 102-test verify-hook.sh suite covers the verified bypass forms from iterations 1-6, but does NOT cover all interaction shapes between the iter 5 `_check_wrapper_shell_string` recursion and the iter 6 fall-through bug fix. Specifically missing: `eval "bash -c 'codex'"` (nested wrappers via shell strings), `bash -c "sudo codex"` (shell string contains another wrapper), `ssh host "eval 'codex'"` (recursive ssh→eval→codex), and additional `su` flag combinations interacting with the username position.
  Impact: a future regression in `_check_wrapper_shell_string` could go undetected by the test suite. The fix (recursive shell-string classification) appears correct based on manual verification, but the test matrix doesn't exhaustively pin all interaction shapes.
  Architectural fix: extend verify-hook.sh with a "nested wrappers via shell strings" test cluster (~10-15 cases) covering each level of recursion.
  Subsystem: .claude/hooks/verify-hook.sh
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 iteration 6 verification

- [ ] `[BUG-08-003][high]` **findings-schema.json uses if/then constructs that OpenAI Structured Outputs API rejects** — found by dual-tpr-gemini Section 04.3 Scenario 1+2 first real-reviewer attempt.
  Repro: ran the dual-source `/tpr-review` against a 5-commit scope (`81ff576b..816cb891`) via `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh`. Codex failed deterministically on all 3 retry attempts with an OpenAI API 400: `Invalid schema for response_format 'codex_output_schema': In context=(), 'if' is not permitted.` (See `/tmp/ori-tpr-KU4EiXAY/codex.jsonl` for the full error envelope.)
  Root cause: `.claude/skills/dual-tpr/scripts/dual-invoke.sh:50` passes `--output-schema "$SCHEMA"` to `codex exec`. Codex forwards the schema to OpenAI's Structured Outputs API as `response_format.json_schema.schema`, which only accepts a strict subset of JSON Schema. The current `findings-schema.json` uses `if`/`then` constructs at two locations:
    - Lines 42-47: `scope_actually_reviewed` requires `expansion_reason` if `expanded_beyond_packet: true`
    - Lines 107-114: top-level rule requiring `findings` to be empty when `no_findings: true`
  OpenAI's Structured Outputs subset rejects: `if`/`then`/`else`, `not`, `oneOf`, `allOf`, `dependentSchemas`, `dependentRequired`, `format`, `pattern`, `maxLength`, `minLength`, `minimum`, `maximum`, `multipleOf`, `minItems`, `maxItems`, `uniqueItems`, `patternProperties`, `propertyNames`, `contains`. The current schema uses `pattern` (line 59), `maxLength` (line 64), `minimum` (line 55), and `format: "uri"` (line 85) — these likely fail too once `if` is removed.
  Architectural issue: the schema is the canonical home for envelope structure AND for additional invariants (the if/then business rule). Moving the rules to a separate file would create a SSOT/LEAK violation. The correct fix is: schema = canonical home for OpenAI-compatible structure; `validate-envelope.py` (and its callers in `parse-codex.py` / `parse-gemini.py`) = canonical home for code-level invariants. The schema becomes leaner; the validator gains explicit checks.
  Severity: high — blocks the entire dual-source `/tpr-review` workflow on the codex side. Gemini is unaffected (it doesn't use Structured Outputs). The dual-source contract requires BOTH reviewers to succeed.
  Subsystem: .claude/skills/dual-tpr/findings-schema.json
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 validation gate (canary release pattern doing its job)

- [ ] `[BUG-08-004][medium]` **dual-invoke.sh subshell `set -e` aborts before recording exit codes when codex/gemini fail fast** — found alongside BUG-08-003.
  Repro: when codex exits non-zero quickly (e.g. OpenAI API rejection in <10s), `dual-invoke.sh`'s subshell aborts immediately at the failed `codex exec` line and never reaches `echo "$?" > "$RUN/codex.exit"`. Postmortem: scratch dir `/tmp/ori-tpr-KU4EiXAY/` is missing `codex.exit`, `codex.walltime`, `gemini.exit`, `gemini.walltime`, and the round.log "codex finished" / "gemini finished" entries that the subshells should have written.
  Root cause: `.claude/skills/dual-tpr/scripts/dual-invoke.sh:25` sets `set -euo pipefail` at the script level; subshells inherit it. Inside each subshell (lines 48-54 for codex, lines 58-64 for gemini), the layout is `command; echo "$?" > exit_file; echo walltime; echo log_entry`. With `set -e`, a non-zero exit from `command` immediately aborts the subshell — the three `echo` lines never run.
  Impact: post-failure analysis is degraded. Cannot determine the real exit code, walltime, or completion ordering. The retry script reads non-existent `$RUN/codex.exit` (returns empty string), making error categorization unreliable.
  Architectural fix: each subshell should disable `set -e` locally OR use a `trap "echo $? > exit_file" EXIT` to record the exit code regardless of how the subshell terminates. The trap pattern is more robust because it also captures unexpected aborts.
  Subsystem: .claude/skills/dual-tpr/scripts/dual-invoke.sh
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 validation gate

- [ ] `[BUG-08-005][medium]` **dual-invoke.sh leaks orphaned reviewer subprocess when the other fails fast** — found alongside BUG-08-003.
  Repro: round.log line 12 of `/tmp/ori-tpr-KU4EiXAY/round.log` shows `[1775624450] gemini finished` — 133 seconds AFTER the retry loop already exited at second 317. Gemini was running orphaned in the background, completing its 70KB JSONL stream long after dual-invoke.sh had been killed.
  Root cause: `.claude/skills/dual-tpr/scripts/dual-invoke.sh:67-69` sequence is `wait $CODEX_PID; wait $GEMINI_PID`. With `set -e`, when `wait $CODEX_PID` returns non-zero (because the codex subshell aborted on the failed command — see BUG-08-004), the script aborts immediately and `wait $GEMINI_PID` never executes. Gemini becomes orphaned and continues running in the background, writing to `$RUN/gemini.jsonl` even after the parent script has exited. Subsequent retry attempts open the same gemini.jsonl path, racing with the orphan.
  Impact: (a) gemini quota is wasted on doomed attempts, (b) gemini.jsonl can be corrupted by interleaved writes from orphans + new attempts, (c) retry attempts that should validate gemini's output may instead see a mid-stream snapshot from a previous attempt's orphan, (d) postmortem state is unreliable because gemini.jsonl may not reflect any single attempt cleanly.
  Architectural fix: in dual-invoke.sh, replace `wait $CODEX_PID; wait $GEMINI_PID` with explicit per-PID wait that captures exit codes individually and ALWAYS waits for both, e.g.:
  ```
  set +e
  wait "$CODEX_PID";  CODEX_WAIT_EXIT=$?
  wait "$GEMINI_PID"; GEMINI_WAIT_EXIT=$?
  set -e
  ```
  Plus a `trap` on EXIT/INT/TERM in dual-invoke.sh that kills any still-running child PIDs to prevent orphan accumulation across retries.
  Subsystem: .claude/skills/dual-tpr/scripts/dual-invoke.sh
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 validation gate

- [ ] `[BUG-08-006][medium]` **dual-invoke-with-retry.sh wastes 3 retry attempts on deterministic schema rejections** — found alongside BUG-08-003.
  Repro: round.log shows three full retry attempts (304, 311, 315) for the same deterministic OpenAI schema rejection. Each attempt burns gemini quota (gemini ran successfully each time, producing 70KB+ of JSONL output) and wall time (3s of backoff between attempts plus the per-attempt wall time). For a deterministic failure, retrying is pure waste.
  Root cause: `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh:43-83` treats all failure categories EXCEPT `dirty_worktree` as retry-eligible (the BUG-08-002 fix). But schema rejections, malformed JSONL, missing CLI binaries, auth errors, and other deterministic failures will always fail the same way on retry. Only true infra-transient failures (network blips, rate limits) benefit from retry.
  Architectural fix: introduce a failure classifier that maps the parser error suffix to `terminal | retryable`:
    - `terminal`: `dirty_worktree`, `codex_invalid_json_schema`, `codex_invalid_request_error`, `codex_authentication_error`, `gemini_no_begin`, `gemini_authentication_error`, parser categories that indicate skill misconfiguration
    - `retryable`: `launch_or_exit_fail` (could be launch race), `codex_parse_error` (could be mid-stream truncation), `gemini_parse_error`, `gemini_no_end` (could be cancelled mid-stream), network categories
  After classification, the retry loop breaks immediately on `terminal` (like BUG-08-002's dirty_worktree fix) and only retries on `retryable`.
  Cross-reference: this is the symmetric form of BUG-08-002. Both bugs are about the retry loop incorrectly retrying deterministic failures. BUG-08-002 fixed the dirty_worktree case via a special-case `break`; BUG-08-006 generalizes that pattern to a classifier.
  Subsystem: .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 validation gate

- [ ] `[BUG-08-007][low]` **tpr-review SKILL.md example masks transport exit code with trailing echo** — found alongside BUG-08-003.
  Repro: the example bash snippet at `.claude/skills/tpr-review/SKILL.md:207-216` (Step 3 — Invoke the dual-source transport in the background) ends with `echo "transport_exit=$?"`. When run via the Bash tool with `run_in_background: true`, the background-task notification reports the exit code of the LAST command in the script — which is the trailing echo, always 0. The transport's actual non-zero exit code is captured into `$?` for the echo's argument but never propagates to the script's overall exit. Symptom: a complete transport failure (exit 1, e.g. `infra_retries_exhausted: launch_or_exit_fail`) is reported as `exit code 0` in the notification, misleading the orchestrator.
  Root cause: bash script exit code = exit code of the last executed command. With the trailing `echo`, the last command is the `echo`, not the dual-invoke wrapper. The exit code is interpolated into the echo's stdout text but not into the script's exit semantics.
  Architectural fix: remove the trailing `echo "transport_exit=$?"` from the SKILL.md example. The Bash tool's notification reports the script's exit code authoritatively — that IS the source of truth. If the orchestrator wants to also see the exit code in stdout, use `; ec=$?; echo "transport_exit=$ec"; exit "$ec"` instead. Add a note to the skill that the notification's reported exit code is authoritative, not the stdout contents.
  Severity: low — this is a documentation/example bug, not a runtime correctness bug. But it caused real misdiagnosis during BUG-08-003's investigation: the notification reported "exit 0" when the transport had actually failed with `infra_retries_exhausted`. Fixing it removes a footgun for every future consumer of the SKILL.md.
  Subsystem: .claude/skills/tpr-review/SKILL.md
  Found: 2026-04-08 | Source: dual-tpr-gemini section-04.3 validation gate

---

## Resolved Bugs

- [x] `[BUG-08-002][high]` **dual-invoke-with-retry.sh launders dirty_worktree failures via fresh snapshots** — found by validate-dual-tpr.sh during Section 04.3 (dual-tpr-gemini) Scenario 3.
  Repro: ran `bash .claude/skills/dual-tpr/scripts/validate-dual-tpr.sh` against the stub-reviewer-dirty mode. The wrapper detected `dirty_worktree` on attempt 1, retried, then exited 0 on attempt 2 — 2 of 4 Scenario 3 assertions failed.
  Root cause: `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh:43-74` snapshotted the worktree at the START of every retry attempt (line 48). After attempt 1 dirties tracked file F (e.g. `A  F` → `AM F` in `git status --porcelain`), attempt 2's snapshot captured `AM F` as the new "before" baseline. The dirty stub appended again — `git status --porcelain` still reported `AM F` (same status code; content invisible to status) — so `worktree-guard.sh compare` reported CLEAN, the `else` branch fired (line 63), and the round exited 0. The `2> $RUN/worktree-error` redirect (line 60) also truncated the diff file on the successful attempt, erasing the evidence from attempt 1.
  Architectural issue: the retry loop treated `dirty_worktree` as a transient failure category (worth retrying), but it is a deterministic signal of reviewer misbehavior — a misbehaving reviewer will misbehave on retry too. Retry CANNOT fix it.
  Resolved 2026-04-08: added `break` after the dirty_worktree branch records its failure (single-line surgical fix). `dirty_worktree` is now a terminal failure category — recorded in round.log, worktree-error preserved, retry loop exits immediately. Other failure categories (`launch_or_exit_fail`, `codex_*`, `gemini_*`) remain retry-eligible because they CAN be transient. Verified end-to-end: validate-dual-tpr.sh now reports 8/8 passing; the original Section 02 transport-tests.sh regression suite still reports 18/18 passing (no regressions in clean-state tests). The corrected behavior matches the design intent stated in tpr-review/SKILL.md §"What NOT to do on transport failure" line 379: "DO NOT silently retry the semantic loop on infra failure" — the same principle applies inside the infra retry loop itself for deterministic categories.
  Subsystem: .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh
  Found: 2026-04-08 | Source: validate-dual-tpr.sh / dual-tpr-gemini section-04.3 validation gate
  Note: This bug is the canary-release validation gate of dual-tpr-gemini doing exactly what it was designed to do — catching a Section 02 transport bug before it propagates to Sections 05/06/07. The plan's Section 04.3 explicitly says: "Transport bugs found here are the REASON this section exists as a validation gate — they're expected, they're valuable, and fixing them before propagation is the whole point of the canary release pattern." TPR/hygiene reviews skipped by user policy on shell-only fixes (same precedent as BUG-08-001).

- [x] `[BUG-08-001][medium]` **block-banned-commands.sh false-matches codex/gemini substrings in non-review commands** — found by continue-roadmap.
  Repro: invoke Bash tool with `git commit -m "feat: dual codex + gemini transport"` and `timeout: 150000` — the hook blocks with "Blocked: timeout (X ms) on codex/gemini command is too short. Reviews need 20-35 minutes" even though the command is a plain git commit, not a reviewer invocation. Same false-match hits `grep codex .claude/`, `ls .gemini/skills/`, `cat .codex/skills/review-work/SKILL.md`, and any Bash command whose arguments/paths/messages contain the literal substrings `codex` or `gemini`.
  Root cause: `.claude/hooks/block-banned-commands.sh:70` uses `[[ "$COMMAND" == *"codex"* || "$COMMAND" == *"gemini"* ]]` — a naive substring match over the entire command line, with no word boundary or command-position check. Every occurrence of the literal in an argument, file path, commit message body, or grep pattern is treated as a reviewer invocation.
  Resolved 2026-04-08 by commit `81ff576b` (`fix(hooks): narrow codex/gemini match to shell command position`). Replaced the substring check with a shell-command-position regex that only matches when codex/gemini appears as an invoked command — at start-of-command, after a compound operator (`|`/`;`/`&`/`(`), optionally behind env-var prefixes, always followed by trailing whitespace. The general banned-pattern substring list (`--no-verify`, `git stash`, etc.) is untouched because those patterns have no legitimate non-invocation use. Extended `verify-hook.sh` from 9 to 27 cases: 10 false-positive suppression tests, 8 bypass-closure tests (env-var prefix / pipeline / && / sequence / subshell / flag-form gemini), plus the original 9 preserved unchanged. Also fixed a latent JSON-encoding bug in `run_test` that would have corrupted commands containing `"` or `\`. Verified end-to-end: the scanner-fix commit `463eb082` — whose commit message references `plans/dual-tpr-gemini` three times — landed cleanly through the fixed hook. Fix section: `plans/bug-tracker/fix-BUG-08-001.md`. TPR/hygiene reviews skipped by user decision (shell hook with deterministic 27-test matrix; lefthook full-check gate ran green on commit).
  Subsystem: .claude/hooks/block-banned-commands.sh
  Found: 2026-04-07 | Source: continue-roadmap
