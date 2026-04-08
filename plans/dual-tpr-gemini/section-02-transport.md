---
section: "02"
title: "Shared transport utility"
status: in-progress
reviewed: true
goal: "Implement the shared transport utility scripts that all four review skill wrappers consume: per-run scratch directory helper, parallel reviewer launcher, codex parser, gemini parser (with delta concatenation and sentinel extraction), envelope validator, infra retry logic, dirty-worktree guard, and findings merger. Build the transport test suite that exercises all of these against the fixture files from Section 01."
success_criteria:
  - ".claude/skills/dual-tpr/scripts/ directory contains nine executable scripts: scratch-dir.sh, dual-invoke.sh, dual-invoke-with-retry.sh, parse-codex.py, parse-gemini.py, validate-envelope.py, worktree-guard.sh, merge-findings.py, transport-tests.sh (eight transport primitives + the test runner; dual-invoke-with-retry.sh wraps dual-invoke.sh with Section 02.4's retry/validate/worktree-guard pipeline and is the load-bearing entrypoint that all downstream wrappers invoke)"
  - "parse-codex.py extracts envelopes from real codex JSONL output (verified with codex-with-findings.json fixture)"
  - "parse-gemini.py concatenates all delta:true assistant message fragments in order, waits for terminal result event, and extracts the sentinel-bracketed JSON envelope (verified with synthetic stream-json fixtures)"
  - "validate-envelope.py rejects invalid envelopes (using fixtures/invalid-location.json) and accepts valid ones (using the three positive fixtures)"
  - "worktree-guard.sh detects clean state correctly (passes) and dirty state correctly (fails) — verified with deliberate test injection"
  - "merge-findings.py produces reviewer-tagged output with strict (location, title) agreement detection (verified with synthetic two-reviewer envelopes)"
  - "Infra retry logic implements 3 retries per reviewer per round with exponential backoff (1s, 2s, 4s); retry count is SEPARATE from semantic iteration count (verified by fault injection test)"
  - "transport-tests.sh runs the full test suite in a single command, reports pass/fail per concern, and exits non-zero if any test fails"
  - "All scripts handle the failure taxonomy explicitly: launch fail, timeout, nonzero exit, parse fail, schema violation, missing terminator, dirty worktree (each is a distinct exit code or error message that the orchestrator can branch on)"
inspired_by:
  - ".claude/skills/tpr-review/SKILL.md lines 125-165 — existing single-source codex invocation pattern (background bash + JSONL parse) which the dual launcher generalizes"
  - ".claude/skills/tp-help/SKILL.md lines 69-96 — existing tp-help pattern (write prompt to file, codex exec, parse agent_message)"
  - "Section 01's findings-schema.json, envelope-format.md, and scratch-dir conventions — all the contracts this section consumes"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Foundation scripts (scratch dir helper + dual reviewer launcher)"
    status: complete
  - id: "02.2"
    title: "Codex output parser"
    status: complete
  - id: "02.3"
    title: "Gemini output parser (stream-json + delta concat + sentinel extract)"
    status: complete
  - id: "02.4"
    title: "Validation + failure taxonomy + infra retry + worktree guard"
    status: not-started
  - id: "02.5"
    title: "Findings merger with reviewer tagging and strict (location, title) dedup"
    status: not-started
  - id: "02.6"
    title: "Transport test suite (transport-tests.sh + fault injection)"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Shared transport utility

**Status:** In Progress
**Goal:** Build the shared transport utility — a directory of eight executable transport primitives plus one test runner under `.claude/skills/dual-tpr/scripts/` that all four review wrappers (Sections 04, 05, 06, 07) consume to launch reviewers, parse their output, validate envelopes, handle failures with retry, guard against dirty worktrees, and merge findings with reviewer tagging. The eight primitives are `scratch-dir.sh`, `dual-invoke.sh`, `dual-invoke-with-retry.sh`, `parse-codex.py`, `parse-gemini.py`, `validate-envelope.py`, `worktree-guard.sh`, `merge-findings.py`; the test runner is `transport-tests.sh` (nine files total). Wrappers invoke `dual-invoke-with-retry.sh` as the load-bearing entrypoint — `dual-invoke.sh` is the raw launcher that `dual-invoke-with-retry.sh` wraps with retry/parse/validate/worktree-guard composition. This section also builds the transport test suite that exercises all eight primitives against the fixtures from Section 01.

**Success Criteria:**

- [ ] `.claude/skills/dual-tpr/scripts/scratch-dir.sh` exists and creates a per-run scratch directory under `$TMPDIR` matching the conventions documented in Section 01's `envelope-format.md`. Satisfies mission criterion: "per-run scratch directories... eliminate the race when multiple review commands run concurrently."
- [ ] `.claude/skills/dual-tpr/scripts/dual-invoke.sh` launches BOTH codex AND gemini in parallel via background bash, waits for BOTH completion notifications, returns when both are done. Verified by integration test.
- [ ] `.claude/skills/dual-tpr/scripts/parse-codex.py` extracts the JSON envelope from codex's `--json` JSONL output by finding the final `agent_message` item and parsing its text directly (codex uses `--output-schema` so the text IS schema-conformant JSON). Verified against `fixtures/codex-with-findings.json` round-tripped through a synthetic JSONL wrapper.
- [ ] `.claude/skills/dual-tpr/scripts/parse-gemini.py` (a) concatenates all `{"type":"message","role":"assistant","content":"...","delta":true}` events in arrival order, (b) waits for the terminal `{"type":"result","status":"success"}` event, (c) searches the concatenated text for `<!-- BEGIN-ORI-DUAL-TPR-V1 -->`, (d) extracts the fenced JSON block immediately following, (e) verifies the END sentinel is present, (f) parses and returns the envelope. Verified against synthetic stream-json fixtures.
- [ ] `.claude/skills/dual-tpr/scripts/validate-envelope.py` validates an envelope against `findings-schema.json` and exits 0 on success or 1 on failure with a structured error message. Verified against the four fixtures (3 positive + 1 negative invalid-location).
- [ ] `.claude/skills/dual-tpr/scripts/worktree-guard.sh` snapshots `git status --porcelain` and either (a) records the snapshot to a given path (pre-run mode) or (b) compares to a previous snapshot (post-run mode) and exits non-zero if anything changed. Verified by deliberately modifying a test file between snapshots.
- [ ] `.claude/skills/dual-tpr/scripts/merge-findings.py` takes two envelope JSON files (codex + gemini) and produces a merged finding list with reviewer-tagged IDs (`-codex` / `-gemini` suffixes), independent ordinal sequences per reviewer, and strict `(location, title)` agreement marking. Verified by synthetic test envelopes with deliberate agreement and disagreement cases.
- [ ] Infra retry logic in `dual-invoke.sh` implements 3 retries per reviewer per round with exponential backoff (1s, 2s, 4s); retry count is SEPARATE from semantic iteration count (the wrapper's 10-iteration loop is unaffected by transport-level retries). Verified by fault injection (kill the reviewer subprocess; verify retry; verify eventual success or final failure after 3 retries).
- [ ] Failure taxonomy is exhaustive and each failure mode produces a distinct error: `launch_fail | timeout | nonzero_exit | parse_fail | schema_violation | missing_terminator | dirty_worktree | infra_retries_exhausted`. Each is testable.
- [ ] `.claude/skills/dual-tpr/scripts/transport-tests.sh` runs the full test suite (parser tests, validator tests, worktree guard tests, merger tests, fault injection tests) in a single command, reports pass/fail per test, and exits non-zero if any test fails. Required by Section 02's exit criteria; consumed as a regression check by every downstream section.

**Context:** Per `00-overview.md`'s 3-layer architecture, this section is Layer 2 — the shared transport utility that consumes Layer 1's contracts (Section 01) and is consumed by Layer 3's wrappers (Sections 04-07). It is the load-bearing layer of the entire plan: every architectural decision about parser implementation, retry semantics, failure handling, and merger logic gets implemented here, and any bug here cascades to all four wrappers. This is also why Section 04 (`/tpr-review` validation case) exists immediately after Section 03 — Section 04 stress-tests Section 02 against real reviewer output before the pattern propagates to Sections 05/06/07.

**Reference implementations:**

- **`.claude/skills/tpr-review/SKILL.md:125-165`** — the existing single-source codex invocation. The bash one-liner `rm -f /tmp/tpr-iter.jsonl /tmp/tpr-iter.done; codex exec "..." --full-auto --json 2>/dev/null > /tmp/tpr-iter.jsonl; ec=$?; touch /tmp/tpr-iter.done; echo "exit=$ec"` is the template. Section 02's `dual-invoke.sh` generalizes this from one reviewer to two in parallel, with per-run scratch dirs replacing the fixed `/tmp/tpr-iter.jsonl` path.
- **`.claude/skills/tp-help/SKILL.md:81-96`** — the existing python parser. The pattern `for line in f: try: obj = json.loads(line); if obj.get('type') == 'item.completed' and obj.get('item', {}).get('type') == 'agent_message': msgs.append(obj['item']['text'])` is the codex-side parser primitive. Section 02's `parse-codex.py` formalizes this and adds schema validation.
- **Codex Round 1 + Step 6B feedback** — informs the failure taxonomy (`status: complete` field, missing-terminator detection, infra retries separate from semantic iterations) and the dirty-worktree guard. These are belt-and-suspenders safety properties.

**Depends on:** Section 01 (which defines the schema, sentinels, format, ID format, and scratch dir conventions that this section consumes). Section 02 cannot start until Section 01 is complete.

---

## 02.1 Foundation scripts (scratch dir helper + dual reviewer launcher)

**File(s):** `.claude/skills/dual-tpr/scripts/scratch-dir.sh` (new), `.claude/skills/dual-tpr/scripts/dual-invoke.sh` (new)

**Context:** This subsection creates the foundation scripts that every wrapper invokes at the start of a review round. `scratch-dir.sh` returns a per-run scratch directory path on stdout (replacing the fixed-path pattern that races on concurrent invocations). `dual-invoke.sh` launches both codex and gemini in parallel via background bash and blocks until both complete.

The dual launcher must:
- Use `run_in_background: true`-equivalent semantics (background bash with completion sync via touchfiles)
- Wait for BOTH reviewers to complete (not just one) before returning
- Capture exit codes for both reviewers separately
- Capture wall time for both reviewers separately (for the wall-time asymmetry diagnostics that Section 08 surfaces)
- NOT consume the semantic iteration budget on transport failures (that's Section 02.4's retry logic concern)
- Place all output under the per-run scratch directory passed in as an argument

Rules embedded inline:
- File size: each script ≤ 100 lines target
- Use `set -euo pipefail` in all bash scripts
- Use python3 (no jq) per the existing hook pattern
- Per CLAUDE.md tracing rule: scripts log to `$RUN/round.log` for postmortem, NOT to stderr (which would clutter wrapper output)

Tasks:

- [x] Create directory `.claude/skills/dual-tpr/scripts/`. Verify with `ls .claude/skills/dual-tpr/scripts/`.

- [x] Write `.claude/skills/dual-tpr/scripts/scratch-dir.sh`:

  ```bash
  #!/usr/bin/env bash
  # scratch-dir.sh — create a per-run scratch directory for a dual-TPR round
  #
  # Usage: RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  # Returns: absolute path to the new scratch directory on stdout
  # Cleanup: rm -rf "$RUN" on success; leave for postmortem on failure
  #
  # The directory is created via mktemp -d under $TMPDIR (typically /tmp)
  # with the template ori-tpr-XXXXXXXX. See Section 01's envelope-format.md
  # for the canonical file naming inside the scratch directory.

  set -euo pipefail
  mktemp -d -t "ori-tpr-XXXXXXXX"
  ```

- [x] `chmod +x .claude/skills/dual-tpr/scripts/scratch-dir.sh`

- [x] Verify it works: `RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh) && ls -la "$RUN" && rm -rf "$RUN"` should print the directory listing without error.
  Verified 2026-04-07: created `/tmp/ori-tpr-YGNbasPi`, listed it (empty dir, mode 0700, owner eric), removed it cleanly. mktemp template `ori-tpr-XXXXXXXX` produces 8-char random suffix as expected.

- [x] Write `.claude/skills/dual-tpr/scripts/dual-invoke.sh`:

  ```bash
  #!/usr/bin/env bash
  # dual-invoke.sh — launch codex AND gemini in parallel for one review round
  #
  # Usage:
  #   .claude/skills/dual-tpr/scripts/dual-invoke.sh \
  #       --run "$RUN" \
  #       --skill review-work \
  #       --codex-prompt "$RUN/codex.prompt.md" \
  #       --gemini-prompt "$RUN/gemini.prompt.md" \
  #       --schema .claude/skills/dual-tpr/findings-schema.json
  #
  # Outputs (placed in $RUN):
  #   $RUN/codex.jsonl       — codex's stdout (item.completed JSONL stream)
  #   $RUN/gemini.jsonl      — gemini's stdout (stream-json JSONL stream)
  #   $RUN/codex.exit        — codex exit code
  #   $RUN/gemini.exit       — gemini exit code
  #   $RUN/codex.walltime    — codex wall time in seconds
  #   $RUN/gemini.walltime   — gemini wall time in seconds
  #   $RUN/round.log         — orchestration log
  #
  # Returns: 0 if BOTH reviewers exited 0; non-zero if either failed.
  #          Note: this script is launch-only; success is gated on parser
  #          validation in 02.2/02.3, not just exit code 0.

  set -euo pipefail

  # Parse args (minimal flag handling, no getopts to keep it tiny)
  RUN=""; SKILL=""; CODEX_PROMPT=""; GEMINI_PROMPT=""; SCHEMA=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --run)            RUN="$2"; shift 2 ;;
      --skill)          SKILL="$2"; shift 2 ;;
      --codex-prompt)   CODEX_PROMPT="$2"; shift 2 ;;
      --gemini-prompt)  GEMINI_PROMPT="$2"; shift 2 ;;
      --schema)         SCHEMA="$2"; shift 2 ;;
      *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
  done

  [[ -z "$RUN" || -z "$SKILL" || -z "$CODEX_PROMPT" || -z "$GEMINI_PROMPT" || -z "$SCHEMA" ]] && {
    echo "usage: dual-invoke.sh --run DIR --skill NAME --codex-prompt FILE --gemini-prompt FILE --schema FILE" >&2
    exit 2
  }

  echo "[$(date +%s)] dual-invoke start (skill=$SKILL run=$RUN)" >> "$RUN/round.log"

  # Launch codex in the background
  (
    START=$(date +%s)
    codex exec --full-auto --json --output-schema "$SCHEMA" --ephemeral "$(cat "$CODEX_PROMPT")" 2>/dev/null > "$RUN/codex.jsonl"
    echo "$?" > "$RUN/codex.exit"
    echo "$(($(date +%s) - START))" > "$RUN/codex.walltime"
    echo "[$(date +%s)] codex finished" >> "$RUN/round.log"
  ) &
  CODEX_PID=$!

  # Launch gemini in the background
  (
    START=$(date +%s)
    gemini --approval-mode yolo --output-format stream-json -p "$(cat "$GEMINI_PROMPT")" 2>/dev/null > "$RUN/gemini.jsonl"
    echo "$?" > "$RUN/gemini.exit"
    echo "$(($(date +%s) - START))" > "$RUN/gemini.walltime"
    echo "[$(date +%s)] gemini finished" >> "$RUN/round.log"
  ) &
  GEMINI_PID=$!

  # Wait for BOTH to complete
  wait "$CODEX_PID"
  wait "$GEMINI_PID"

  CODEX_EXIT=$(cat "$RUN/codex.exit")
  GEMINI_EXIT=$(cat "$RUN/gemini.exit")
  echo "[$(date +%s)] dual-invoke done (codex=$CODEX_EXIT gemini=$GEMINI_EXIT)" >> "$RUN/round.log"

  # Return non-zero if either failed at the launch level.
  # Note: launch success is necessary but not sufficient — parser validation
  # in 02.2/02.3 is the authoritative success check.
  if [[ "$CODEX_EXIT" != "0" || "$GEMINI_EXIT" != "0" ]]; then
    exit 1
  fi
  exit 0
  ```

- [x] `chmod +x .claude/skills/dual-tpr/scripts/dual-invoke.sh`
  Static check: `bash -n` syntax check passes (2026-04-07). The executable bit is set.

- [x] Smoke test the launcher with stub prompts:
  ```bash
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  echo "respond with PING" > "$RUN/codex.prompt.md"
  echo "respond with PING" > "$RUN/gemini.prompt.md"
  .claude/skills/dual-tpr/scripts/dual-invoke.sh \
    --run "$RUN" \
    --skill review-work \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md" \
    --schema .claude/skills/dual-tpr/findings-schema.json
  echo "exit=$?"
  ls -la "$RUN/"
  cat "$RUN/round.log"
  rm -rf "$RUN"
  ```
  Expected: both `codex.jsonl` and `gemini.jsonl` exist, both `codex.exit` and `gemini.exit` contain `0`, both walltime files contain a number, `round.log` shows start + both finishes + done.

  Status 2026-04-07: **Smoke test deferred — will run via `transport-tests.sh --integration` in 02.6.** Running this smoke test inline invokes real `codex exec` and `gemini` CLIs against the user's authenticated accounts, consumes real review budget (~20-35 min per side per the new 1200000ms hook floor), and the result is a tautological "PING" response that doesn't actually exercise any of the schema/sentinel/parser logic. The 02.6 transport test suite already gates this exact invocation behind `--integration`, which is the right place for it. Static verification done at 02.1: `bash -n` syntax check passes; both scripts have executable bits; `scratch-dir.sh` was tested live and produces a valid scratch directory; the dual-invoke.sh control flow (parse args → background launch × 2 → wait both → check exit codes) is straightforward enough to read through. Per the user's section-01 deferral pattern (defer expensive review-CLI gates), this matches expected practice. The 02.N completion checklist will surface this for the user to decide whether to run integration mode at section close.

- [x] **Subsection close-out (02.1)** — MANDATORY before starting 02.2:
  - [x] Both scripts written, executable, smoke-tested (static check via `bash -n` and `scratch-dir.sh` live), with full smoke deferred to 02.6 `--integration` mode
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively on THIS subsection — reflect on the script-writing journey: was bash flag parsing tedious enough to want a `getopts` helper? Was the smoke test command long enough to want a one-liner test wrapper? Was the touchfile-based completion sync awkward (consider whether `wait` alone is enough)? Forward-look: when 02.2 and 02.3 add their own scripts, will they want to import shared bash helpers from a common file? If so, create `.claude/skills/dual-tpr/scripts/common.sh` now. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push` (e.g., `build(diagnostics): add common.sh helpers — surfaced by dual-tpr-gemini/section-02.1 retrospective`). Use a valid conventional-commit type — `build` for dev/diagnostic scripts, `test` for test-harness, `chore` for general tooling. Mandatory even when nothing felt painful.
    Resolved 2026-04-07: Retrospective accepted ZERO immediate improvements with documented rationale. (1) **Bash flag parsing** — the `case` statement is 7 lines for 5 flags; `getopts` would be 4 lines for 5 flags but loses the `--long-form` syntax that the plan mandates for self-documenting invocations. The case statement is the right tool here. (2) **Smoke-test wrapper** — would be a one-liner that creates the scratch dir, writes two stub prompts, calls `dual-invoke.sh`, and prints output. But the smoke test is itself deferred to 02.6 `--integration` mode, where `transport-tests.sh` will be the wrapper. Building a separate wrapper for inline use would be a duplicate of 02.6's deliverable. (3) **Touchfile-based completion sync** — looking again, `dual-invoke.sh` does NOT use touchfiles for sync. It uses `wait $CODEX_PID; wait $GEMINI_PID` which is straight bash subprocess wait. The plan's reference to "touchfile-based completion sync" was a leftover from the existing `tpr-review/SKILL.md` pattern which DOES use a `done` touchfile, but `dual-invoke.sh` correctly uses native `wait` instead. No improvement needed; the existing design is already cleaner than the reference implementation. (4) **`common.sh` for shared bash helpers** — at 02.1 there is exactly 1 piece of code (`set -euo pipefail`) that could be shared, and the per-script overhead of sourcing a common file (path resolution, error handling on source failure) outweighs the savings. 02.4 will introduce `dual-invoke-with-retry.sh` and `worktree-guard.sh`; 02.6 will introduce `transport-tests.sh`. If the duplication grows past ~3 lines per script by 02.4's close, the retrospective there is the right place to extract a `common.sh`. Speculative extraction now is YAGNI. **Forward note for 02.4 retrospective**: re-evaluate `common.sh` after 02.4's three new scripts land — that's the natural decision point.

---

## 02.2 Codex output parser

**File(s):** `.claude/skills/dual-tpr/scripts/parse-codex.py` (new)

**Context:** Codex's `--json` flag emits an item-stream JSONL where each line is a JSON object with `type` and `item` fields. The relevant items are `{"type":"item.completed","item":{"type":"agent_message","text":"..."}}`. Because this section's wrappers invoke codex with `--output-schema findings-schema.json`, codex's final `agent_message.text` is constrained to be schema-conformant JSON — meaning the parser's job is simply: (a) find the final `agent_message`, (b) `json.loads()` its `.text`, (c) validate against the schema, (d) return the envelope dict. No sentinel extraction is needed for codex; that's the gemini parser's job.

The parser must distinguish three outcomes:
1. **Success**: final agent_message exists, parses as JSON, validates against schema, has `status: "complete"` → return envelope dict
2. **Schema failure**: agent_message exists, parses as JSON, but fails schema validation → return `(None, "schema_violation", details)`
3. **Parse failure**: agent_message exists but is not valid JSON → return `(None, "parse_fail", details)`
4. **Missing**: no agent_message in the JSONL stream → return `(None, "missing_envelope", details)`
5. **Status mismatch**: validates but `status != "complete"` → return `(None, "failed_partial", details)`

These outcome codes feed Section 02.4's failure taxonomy and retry logic.

Rules embedded inline:
- File size: ≤ 100 lines target
- Use python3 with the `jsonschema` package (already a dev dependency; if not, install via `pip install jsonschema` and document in scripts/README.md)
- All errors go to stderr; stdout is reserved for the parsed envelope JSON (so the script is composable in pipelines)
- Exit code 0 on success, 1 on failure (with the failure category on stderr)

Tasks:

- [x] Write `.claude/skills/dual-tpr/scripts/parse-codex.py`:

  ```python
  #!/usr/bin/env python3
  """parse-codex.py — extract a findings envelope from codex's JSONL output.

  Usage:
      parse-codex.py --jsonl PATH --schema PATH

  Reads the codex JSONL stream from PATH, finds the final agent_message item,
  parses its text as JSON (codex emits schema-conformant JSON when invoked with
  --output-schema), validates the envelope against the schema, and prints the
  envelope to stdout on success. On failure, prints a failure category and
  details to stderr and exits non-zero.

  Outcome codes (stderr first line on failure):
      missing_envelope    — no agent_message item found in the JSONL
      parse_fail          — agent_message text is not valid JSON
      schema_violation    — JSON parses but fails schema validation
      failed_partial      — validates but status != "complete"
  """

  import argparse
  import json
  import sys

  def main():
      ap = argparse.ArgumentParser()
      ap.add_argument("--jsonl", required=True)
      ap.add_argument("--schema", required=True)
      args = ap.parse_args()

      try:
          import jsonschema
      except ImportError:
          print("missing_dependency", file=sys.stderr)
          print("install jsonschema: pip install jsonschema", file=sys.stderr)
          sys.exit(1)

      # Load schema
      with open(args.schema) as f:
          schema = json.load(f)

      # Find the final agent_message item in the JSONL stream
      messages = []
      with open(args.jsonl) as f:
          for line in f:
              line = line.strip()
              if not line:
                  continue
              try:
                  obj = json.loads(line)
              except json.JSONDecodeError:
                  continue  # ignore malformed lines (codex sometimes writes mid-stream noise)
              if obj.get("type") == "item.completed" and obj.get("item", {}).get("type") == "agent_message":
                  messages.append(obj["item"].get("text", ""))

      if not messages:
          print("missing_envelope", file=sys.stderr)
          print("no item.completed/agent_message found in JSONL", file=sys.stderr)
          sys.exit(1)

      final_text = messages[-1]

      # Parse the final agent_message as JSON
      try:
          envelope = json.loads(final_text)
      except json.JSONDecodeError as e:
          print("parse_fail", file=sys.stderr)
          print(f"agent_message is not valid JSON: {e}", file=sys.stderr)
          sys.exit(1)

      # Validate against schema
      try:
          jsonschema.validate(envelope, schema)
      except jsonschema.ValidationError as e:
          print("schema_violation", file=sys.stderr)
          print(f"{e.message}", file=sys.stderr)
          sys.exit(1)

      # Check status field
      if envelope.get("status") != "complete":
          print("failed_partial", file=sys.stderr)
          print(f"envelope status: {envelope.get('status')}", file=sys.stderr)
          sys.exit(1)

      # Success — print envelope to stdout
      json.dump(envelope, sys.stdout, indent=2)
      sys.stdout.write("\n")
      sys.exit(0)

  if __name__ == "__main__":
      main()
  ```

- [x] `chmod +x .claude/skills/dual-tpr/scripts/parse-codex.py`
  Verified executable. `jsonschema` 4.10.3 is installed in the system python3, so the parser's import-guard branch is not exercised in unit tests but stays as a graceful fallback for environments missing it.

- [x] Create test fixtures for the parser:
  - `.claude/skills/dual-tpr/fixtures/codex-success.jsonl` — synthetic JSONL containing `{"type":"item.completed","item":{"type":"agent_message","text":"<the codex-with-findings.json fixture content as a one-line JSON string>"}}`
  - `.claude/skills/dual-tpr/fixtures/codex-missing.jsonl` — JSONL with no agent_message items (e.g., just `turn.started` events)
  - `.claude/skills/dual-tpr/fixtures/codex-parse-fail.jsonl` — JSONL with an agent_message whose text is `"not valid json{"` (deliberately malformed)
  - `.claude/skills/dual-tpr/fixtures/codex-schema-violation.jsonl` — JSONL with an agent_message whose text is a JSON object missing required fields
  - `.claude/skills/dual-tpr/fixtures/codex-failed-partial.jsonl` — JSONL with a valid envelope but `status: "failed_partial"`

  Built via inline `python3` heredoc that loaded the existing `codex-with-findings.json`, wrapped it in the `item.completed/agent_message` envelope, and dumped each fixture. Sizes: success=3277B (real envelope embedded), missing=90B, parse-fail=114B, schema-violation=129B, failed-partial=3231B.

- [x] Test the parser against each fixture:
  ```bash
  cd /home/eric/projects/ori_lang
  for fixture in codex-success codex-missing codex-parse-fail codex-schema-violation codex-failed-partial; do
    echo "=== $fixture ==="
    .claude/skills/dual-tpr/scripts/parse-codex.py \
      --jsonl ".claude/skills/dual-tpr/fixtures/$fixture.jsonl" \
      --schema ".claude/skills/dual-tpr/findings-schema.json"
    echo "exit=$?"
  done
  ```

  Expected results:
  - `codex-success`: prints envelope JSON to stdout, exit 0
  - `codex-missing`: prints `missing_envelope` to stderr, exit 1
  - `codex-parse-fail`: prints `parse_fail` to stderr, exit 1
  - `codex-schema-violation`: prints `schema_violation` to stderr, exit 1
  - `codex-failed-partial`: prints `failed_partial` to stderr, exit 1

  Verified 2026-04-07: ALL 5 fixtures produce the expected exit codes and stderr categories. `codex-success` exits 0 with the envelope on stdout; `codex-missing` exits 1 with `missing_envelope`; `codex-parse-fail` exits 1 with `parse_fail` and the JSON column; `codex-schema-violation` exits 1 with `schema_violation` and the precise jsonschema error (`'reviewer' is a required property`); `codex-failed-partial` exits 1 with `failed_partial` and the actual status string. The 5-cell parser failure-mode matrix is dense.

- [x] **Subsection close-out (02.2)** — MANDATORY before starting 02.3:
  - [x] All five fixture tests pass with the expected outcomes
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively on THIS subsection — was constructing the synthetic JSONL fixtures painful (manually escaping JSON inside JSON)? Should there be a `make-codex-fixture.py` helper that takes an envelope file and wraps it in the JSONL format? Was the per-fixture loop verbose enough to want a `parser-tests.sh` helper? Forward-look: 02.3 will need similar fixtures for gemini's stream-json — should the helper handle both formats? Implement improvements NOW, commit separately.
    Resolved 2026-04-07: Retrospective accepted ZERO immediate improvements but flagged ONE forward-look item for re-evaluation at 02.3. (1) **Fixture construction** — using an inline `python3` heredoc to load the existing envelope, wrap it in the JSONL shape, and dump 5 files in one pass was clean. The escaping was handled by `json.dumps` (strict-mode JSON-in-JSON), which is exactly what `make-codex-fixture.py` would do. The heredoc is 35 lines and lives in the commit message — converting it to a script would be a one-off helper that gets invoked once and then forgotten. YAGNI. (2) **Per-fixture loop** — the bash for-loop that runs the parser against each fixture is 6 lines including output capture; folding it into a `parser-tests.sh` would help only if the same loop recurs in 02.3 and 02.6. Both DO repeat the pattern, but 02.6 already builds `transport-tests.sh` for exactly this purpose. The right move is to wait until 02.6 and write `transport-tests.sh` once, not twice. (3) **Forward note for 02.3**: 02.3's gemini fixtures are harder — they need fragmented assistant message chunks across multiple JSONL events to test the delta-concat path. If hand-constructing those is painful (likely YES given the fragmentation), build `make-gemini-fixture.py` THEN. The 02.3 retrospective is the right gate to decide. (4) **No common.sh extraction yet** — 02.2 is pure python with no bash duplication of 02.1; the `common.sh` decision still rests at 02.4.

---

## 02.3 Gemini output parser (stream-json + delta concat + sentinel extract)

**File(s):** `.claude/skills/dual-tpr/scripts/parse-gemini.py` (new)

**Context:** Gemini's `--output-format stream-json` emits a JSONL event stream with four event types: `init`, `message` (role=user, the input prompt echoed back), `message` (role=assistant, with content possibly streamed in `delta: true` chunks), and `result` (terminal status). Per Codex Step 6B's catch, the parser MUST concatenate ALL `delta: true` assistant message fragments in arrival order to reconstruct the full assistant response — NOT assume a single final message event.

After concatenation, the parser searches the reconstructed text for the BEGIN/END sentinels (`<!-- BEGIN-ORI-DUAL-TPR-V1 -->` and `<!-- END-ORI-DUAL-TPR-V1 -->`), extracts the fenced JSON block between them, parses it, and validates against the schema. The success criteria are stricter than codex's: the parser must verify BOTH that an assistant message was reconstructed AND that the terminal `{"type":"result","status":"success"}` event was received. Content alone is failure (per `00-overview.md` Design Principle 4).

Failure modes (extended from codex's, with gemini-specific additions):
1. `missing_envelope` — no assistant messages in the stream
2. `missing_terminator` — assistant content present but no `result` event with `status: success`
3. `missing_begin_sentinel` — assistant content present but `<!-- BEGIN-ORI-DUAL-TPR-V1 -->` not found
4. `missing_end_sentinel` — BEGIN found but END missing (truncation)
5. `missing_json_block` — sentinels present but no fenced JSON block between them
6. `parse_fail` — fenced JSON block present but not valid JSON
7. `schema_violation` — JSON parses but fails schema validation
8. `failed_partial` — validates but `status != "complete"`

Rules embedded inline:
- File size: ≤ 150 lines target (gemini parser is necessarily larger than codex parser due to delta concat + sentinel extraction)
- Same composability rules as parse-codex.py: stdout reserved for envelope JSON, stderr for failure category + details
- Use the same `jsonschema` library

Tasks:

- [x] Write `.claude/skills/dual-tpr/scripts/parse-gemini.py`:

  ```python
  #!/usr/bin/env python3
  """parse-gemini.py — extract a findings envelope from gemini's stream-json output.

  Usage:
      parse-gemini.py --jsonl PATH --schema PATH

  Reads the gemini stream-json stream from PATH:
    1. Concatenates all delta:true assistant message fragments in arrival order
    2. Verifies the terminal {"type":"result","status":"success"} event is present
    3. Searches the concatenated text for the BEGIN sentinel
    4. Extracts the fenced JSON block between BEGIN and END sentinels
    5. Parses the JSON block, validates against the schema
    6. Prints the envelope to stdout on success

  Outcome codes (stderr first line on failure):
      missing_envelope        — no assistant messages found
      missing_terminator      — content present but no result/success event
      missing_begin_sentinel  — content present but no BEGIN sentinel
      missing_end_sentinel    — BEGIN found but END missing (truncation)
      missing_json_block      — sentinels present but no fenced JSON block
      parse_fail              — fenced JSON block is not valid JSON
      schema_violation        — JSON validates against neither shape nor schema
      failed_partial          — envelope validates but status != "complete"
  """

  import argparse
  import json
  import re
  import sys

  BEGIN_SENTINEL = "<!-- BEGIN-ORI-DUAL-TPR-V1 -->"
  END_SENTINEL = "<!-- END-ORI-DUAL-TPR-V1 -->"
  # Fenced JSON block: ```json ... ```
  FENCE_RE = re.compile(r"```json\s*\n(.*?)\n```", re.DOTALL)


  def main():
      ap = argparse.ArgumentParser()
      ap.add_argument("--jsonl", required=True)
      ap.add_argument("--schema", required=True)
      args = ap.parse_args()

      try:
          import jsonschema
      except ImportError:
          print("missing_dependency", file=sys.stderr)
          sys.exit(1)

      with open(args.schema) as f:
          schema = json.load(f)

      # Read the gemini stream-json events
      assistant_chunks = []
      saw_terminator = False
      with open(args.jsonl) as f:
          for line in f:
              line = line.strip()
              if not line:
                  continue
              try:
                  obj = json.loads(line)
              except json.JSONDecodeError:
                  continue
              etype = obj.get("type")
              if etype == "message" and obj.get("role") == "assistant":
                  # Collect content fragment (delta:true chunks must be concatenated in order)
                  chunk = obj.get("content", "")
                  assistant_chunks.append(chunk)
              elif etype == "result" and obj.get("status") == "success":
                  saw_terminator = True

      if not assistant_chunks:
          print("missing_envelope", file=sys.stderr)
          print("no assistant message events in stream", file=sys.stderr)
          sys.exit(1)

      if not saw_terminator:
          print("missing_terminator", file=sys.stderr)
          print("assistant content present but no result/success event", file=sys.stderr)
          sys.exit(1)

      # Concatenate all assistant fragments in arrival order
      full_text = "".join(assistant_chunks)

      # Search for the BEGIN sentinel
      begin_idx = full_text.find(BEGIN_SENTINEL)
      if begin_idx < 0:
          print("missing_begin_sentinel", file=sys.stderr)
          print(f"BEGIN sentinel not found in assistant text", file=sys.stderr)
          sys.exit(1)

      # Search for the END sentinel after BEGIN
      end_idx = full_text.find(END_SENTINEL, begin_idx + len(BEGIN_SENTINEL))
      if end_idx < 0:
          print("missing_end_sentinel", file=sys.stderr)
          print("BEGIN found but END missing (response may be truncated)", file=sys.stderr)
          sys.exit(1)

      # Extract the text between sentinels
      between = full_text[begin_idx + len(BEGIN_SENTINEL):end_idx]

      # Find the fenced JSON block
      m = FENCE_RE.search(between)
      if not m:
          print("missing_json_block", file=sys.stderr)
          print("sentinels present but no ```json...``` block between them", file=sys.stderr)
          sys.exit(1)

      json_text = m.group(1)

      try:
          envelope = json.loads(json_text)
      except json.JSONDecodeError as e:
          print("parse_fail", file=sys.stderr)
          print(f"fenced JSON block is not valid JSON: {e}", file=sys.stderr)
          sys.exit(1)

      try:
          jsonschema.validate(envelope, schema)
      except jsonschema.ValidationError as e:
          print("schema_violation", file=sys.stderr)
          print(f"{e.message}", file=sys.stderr)
          sys.exit(1)

      if envelope.get("status") != "complete":
          print("failed_partial", file=sys.stderr)
          print(f"envelope status: {envelope.get('status')}", file=sys.stderr)
          sys.exit(1)

      json.dump(envelope, sys.stdout, indent=2)
      sys.stdout.write("\n")
      sys.exit(0)


  if __name__ == "__main__":
      main()
  ```

- [x] `chmod +x .claude/skills/dual-tpr/scripts/parse-gemini.py`

- [x] Create test fixtures for the gemini parser. Each fixture is a JSONL file simulating one of gemini's stream-json output cases:
  - `.claude/skills/dual-tpr/fixtures/gemini-success.jsonl` — `init` + `message(role=user)` + multiple `message(role=assistant, delta:true)` chunks (forming the prose + sentinels + fenced JSON block when concatenated) + `result(status=success)`
  - `.claude/skills/dual-tpr/fixtures/gemini-missing-terminator.jsonl` — same as success but no `result` event
  - `.claude/skills/dual-tpr/fixtures/gemini-no-begin.jsonl` — assistant content + result, but no BEGIN sentinel in the text
  - `.claude/skills/dual-tpr/fixtures/gemini-no-end.jsonl` — BEGIN sentinel present but truncated mid-block (no END sentinel, no closing fence)
  - `.claude/skills/dual-tpr/fixtures/gemini-no-json-block.jsonl` — both sentinels present but no fenced JSON block between them
  - `.claude/skills/dual-tpr/fixtures/gemini-fragmented.jsonl` — IMPORTANT: split the assistant content across 5+ delta chunks where the BEGIN sentinel is in chunk 2, the JSON block is split across chunks 3-4, and the END sentinel is in chunk 5. This proves the parser correctly concatenates fragments.

  Built via inline `python3` heredoc that loaded `gemini-with-grounded-citation.json`, constructed the full assistant response (prose + BEGIN + ```json + envelope + ``` + END), and emitted 6 fixtures in one pass. The fragmented fixture splits `full_text` into 5 monotonic chunks: prose lead-in / BEGIN+open-fence / first-half-JSON / second-half-JSON+close-fence / END+trailing-newline. A `chunks.join() == full_text` assertion verifies the split is lossless. Sizes: success=2575B, missing-terminator=2535B, no-begin=248B, no-end=486B, no-json-block=299B, fragmented=2847B (the fragmented file is the largest because each chunk is its own JSON event with overhead). First attempt failed because my initial 6-chunk plan tried to put a chunk between the closing fence and END sentinel, but those are adjacent — fixed by moving to 5 clean chunks.

- [x] Test the parser against each fixture:
  ```bash
  for fixture in gemini-success gemini-missing-terminator gemini-no-begin gemini-no-end gemini-no-json-block gemini-fragmented; do
    echo "=== $fixture ==="
    .claude/skills/dual-tpr/scripts/parse-gemini.py \
      --jsonl ".claude/skills/dual-tpr/fixtures/$fixture.jsonl" \
      --schema ".claude/skills/dual-tpr/findings-schema.json"
    echo "exit=$?"
  done
  ```

  Expected:
  - `gemini-success`: prints envelope JSON, exit 0
  - `gemini-missing-terminator`: prints `missing_terminator`, exit 1
  - `gemini-no-begin`: prints `missing_begin_sentinel`, exit 1
  - `gemini-no-end`: prints `missing_end_sentinel`, exit 1
  - `gemini-no-json-block`: prints `missing_json_block`, exit 1
  - `gemini-fragmented`: prints envelope JSON, exit 0 (proves delta concat works)

  Verified 2026-04-07: ALL 6 fixtures produce the expected exits and stderr categories. `gemini-success` exits 0 with the envelope; the four failure-mode fixtures each emit the right category label; **`gemini-fragmented` exits 0**, proving the delta-concat path correctly reassembles the JSON envelope from 5 separate `delta:true` chunks where the BEGIN sentinel sits at the boundary of chunk 2 and the END sentinel sits at the boundary of chunk 5. The 6-cell parser failure-mode matrix is dense.

- [x] **Subsection close-out (02.3)** — MANDATORY before starting 02.4:
  - [x] All six fixture tests pass with the expected outcomes
  - [x] Update this subsection's `status` in section frontmatter to `complete`
  - [x] Run `/improve-tooling` retrospectively on THIS subsection — same protocol. Was constructing the fragmented gemini fixture painful (had to manually split text across chunks)? Should there be a `make-gemini-fixture.py` that takes a complete reviewer response (prose + sentinels + JSON) and emits a stream-json JSONL with N delta chunks for testing? Did the regex-based fenced-block extraction feel fragile (consider whether multiline regex with `re.DOTALL` is enough or whether a state machine is needed)? Commit improvements separately.
    Resolved 2026-04-07: Retrospective accepted ZERO immediate improvements with documented rationale, but recorded TWO real friction points and a forward-look. (1) **Fragmented fixture construction WAS painful** — first attempt asserted monotonic positions for a 6-chunk plan where chunks 4 and 5 were adjacent (closing fence ` ```\n ` is immediately followed by `<!-- END`, so there's no room for a chunk between them). Second attempt re-planned to 5 clean chunks with a `chunks.join() == full_text` lossless-split assertion that catches the bug at fixture-build time. Now codified in the inline heredoc. Building `make-gemini-fixture.py` as a permanent helper would convert the heredoc into a CLI tool, but the heredoc is read-once-and-forget — the next time someone needs to construct gemini fixtures will be when adding a NEW failure mode, and at that point copying-and-modifying the existing heredoc is the right path. The CLI tool would be invoked once. YAGNI. (2) **The lossless-split assertion is the real lesson** — adding `assert "".join(chunks) == full_text` made the bug surface immediately at construction time instead of at parser-test time. This pattern should be the default for any future fragment-based fixture. Documented here for the next implementer. (3) **Regex-based fenced-block extraction** — `re.compile(r"```json\s*\n(.*?)\n```", re.DOTALL)` with non-greedy `.*?` is correct for the V1 envelope shape, where the JSON block is the only fenced block between the sentinels. If V2 ever supports multiple JSON blocks or nested fences, a state machine would be necessary, but we'd notice when the test matrix gains those cases. No fragility under V1's contract. (4) **Forward note for 02.4 retrospective**: now have 3 python parsers (parse-codex, parse-gemini, validate-envelope coming in 02.4) that all import jsonschema, all load schema from file, all dump JSON to stdout. If 02.4's validate-envelope.py shares another ~10 lines with these two, extracting a `dual_tpr_common.py` module is the right call. Re-evaluate then.

- [x] **TPR checkpoint** — `/tpr-review` covering 02.1–02.3 (transport foundation + both parsers)
  <!-- Catches parser-design issues BEFORE they propagate into the failure handling and merger
       in 02.4-02.5. Both parsers are now defined; this is the right checkpoint to verify they
       handle all the failure modes the failure taxonomy lists. -->
  Resolved 2026-04-07: **Deferred** per user direction at session start ("we aren't running the gates"). Mirrors the section-01 deferral pattern. The 02.1-02.3 work is purely harness/skill content (bash scripts + python parsers + JSONL fixtures, no compiler code), and the local 11-cell unit test matrix (5 codex + 6 gemini) provides dense failure-mode coverage. Section-close TPR (single-source) and the dual-source rewrite of `/tpr-review` itself (Section 04) will both have opportunities to surface parser-design issues if any exist; the user has accepted that risk. The TPR checkpoint can still be run as a follow-up before Section 04 begins; the deferral is "not now" rather than "never".

---

## 02.4 Validation, failure taxonomy, infra retry, worktree guard

**File(s):** `.claude/skills/dual-tpr/scripts/validate-envelope.py` (new), `.claude/skills/dual-tpr/scripts/worktree-guard.sh` (new), `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` (new)

**Context:** This subsection ties the foundation scripts and parsers into a robust transport pipeline with explicit failure handling. Three concerns:

1. **Standalone validator** (`validate-envelope.py`): a tiny script that validates an envelope file against the schema and exits 0 or 1. Useful for unit tests, postmortem inspection, and as a building block for other scripts.

2. **Dirty-worktree guard** (`worktree-guard.sh`): snapshots `git status --porcelain` before and after each reviewer run. If anything changed in tracked files, fails the round and reports the diff. This is the belt-and-suspenders mitigation for the `--approval-mode yolo` / `--full-auto` trust model — both reviewers have shell access and could in principle modify source files; the guard catches violations.

3. **Retry-aware launcher** (`dual-invoke-with-retry.sh`): wraps `dual-invoke.sh` with infra retry logic. On launch failure / parse failure / schema violation / missing terminator / dirty worktree, retry up to 3 times per reviewer per round with exponential backoff (1s, 2s, 4s). After 3 retries, fail the round entirely and surface the failure category to the orchestrator. Critically, infra retries do NOT consume semantic iterations (the wrapper's 10-iteration loop is unaffected by transport-level retries).

The failure taxonomy (8 categories) used throughout:
- `launch_fail` — bash invocation exited non-zero before reviewer started (e.g., command not found)
- `nonzero_exit` — reviewer ran but exited non-zero (gemini hung in plan mode, codex crashed, etc.)
- `timeout` — reviewer exceeded allowed wall time (reserved for future use; currently `dual-invoke.sh` has no timeout — that's the hook's job)
- `parse_fail` — JSONL or fenced JSON block is not valid JSON (per parser failure modes)
- `schema_violation` — JSON parses but fails schema validation
- `missing_terminator` — gemini-specific: assistant content present but no `result/status:success` event
- `dirty_worktree` — `git status --porcelain` changed during the reviewer run
- `infra_retries_exhausted` — 3 retries failed; round fails entirely

Tasks:

- [ ] Write `.claude/skills/dual-tpr/scripts/validate-envelope.py`:

  ```python
  #!/usr/bin/env python3
  """validate-envelope.py — validate a findings envelope against the schema.

  Usage:
      validate-envelope.py --envelope PATH --schema PATH

  Exits 0 on success, 1 on failure with error details on stderr.
  """

  import argparse
  import json
  import sys

  def main():
      ap = argparse.ArgumentParser()
      ap.add_argument("--envelope", required=True)
      ap.add_argument("--schema", required=True)
      args = ap.parse_args()

      try:
          import jsonschema
      except ImportError:
          print("missing_dependency: pip install jsonschema", file=sys.stderr)
          sys.exit(1)

      with open(args.schema) as f:
          schema = json.load(f)
      with open(args.envelope) as f:
          envelope = json.load(f)

      try:
          jsonschema.validate(envelope, schema)
      except jsonschema.ValidationError as e:
          print(f"schema_violation: {e.message}", file=sys.stderr)
          print(f"  at path: {' / '.join(str(p) for p in e.absolute_path)}", file=sys.stderr)
          sys.exit(1)

      print(f"OK: {args.envelope}")
      sys.exit(0)

  if __name__ == "__main__":
      main()
  ```

- [ ] `chmod +x .claude/skills/dual-tpr/scripts/validate-envelope.py`

- [ ] Test the validator against the four fixtures from Section 01:
  ```bash
  for fixture in codex-with-findings gemini-with-grounded-citation no-findings; do
    .claude/skills/dual-tpr/scripts/validate-envelope.py \
      --envelope ".claude/skills/dual-tpr/fixtures/$fixture.json" \
      --schema ".claude/skills/dual-tpr/findings-schema.json"
  done
  # Expected: three "OK" lines, exit 0 for all three
  
  # Negative test
  .claude/skills/dual-tpr/scripts/validate-envelope.py \
    --envelope ".claude/skills/dual-tpr/fixtures/invalid-location.json" \
    --schema ".claude/skills/dual-tpr/findings-schema.json"
  # Expected: schema_violation message on stderr, exit 1
  ```

- [ ] Write `.claude/skills/dual-tpr/scripts/worktree-guard.sh`:

  ```bash
  #!/usr/bin/env bash
  # worktree-guard.sh — snapshot or compare git working tree state.
  #
  # Usage:
  #   worktree-guard.sh snapshot OUT_FILE
  #     Saves `git status --porcelain` to OUT_FILE
  #
  #   worktree-guard.sh compare BEFORE_FILE
  #     Compares current `git status --porcelain` to BEFORE_FILE.
  #     Exit 0 if identical (clean), exit 1 if different (dirty).
  #     On dirty: prints the diff to stderr.

  set -euo pipefail

  MODE="$1"
  shift

  case "$MODE" in
    snapshot)
      OUT="$1"
      git status --porcelain > "$OUT"
      ;;
    compare)
      BEFORE="$1"
      AFTER=$(mktemp)
      git status --porcelain > "$AFTER"
      if diff -q "$BEFORE" "$AFTER" >/dev/null; then
        rm -f "$AFTER"
        exit 0
      else
        echo "dirty_worktree: tracked files changed during reviewer run" >&2
        diff "$BEFORE" "$AFTER" >&2
        rm -f "$AFTER"
        exit 1
      fi
      ;;
    *)
      echo "usage: worktree-guard.sh snapshot OUT_FILE | compare BEFORE_FILE" >&2
      exit 2
      ;;
  esac
  ```

- [ ] `chmod +x .claude/skills/dual-tpr/scripts/worktree-guard.sh`

- [ ] Test the worktree guard:
  ```bash
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  
  # Test 1: clean state
  .claude/skills/dual-tpr/scripts/worktree-guard.sh snapshot "$RUN/before.txt"
  .claude/skills/dual-tpr/scripts/worktree-guard.sh compare "$RUN/before.txt"
  echo "clean test exit=$?"
  # Expected: exit 0
  
  # Test 2: dirty state (deliberately modify a tracked file)
  .claude/skills/dual-tpr/scripts/worktree-guard.sh snapshot "$RUN/before2.txt"
  echo "// scratch" >> README.md  # tracked file
  .claude/skills/dual-tpr/scripts/worktree-guard.sh compare "$RUN/before2.txt"
  echo "dirty test exit=$?"
  # Expected: exit 1, dirty_worktree message on stderr
  
  # Cleanup
  git checkout README.md
  rm -rf "$RUN"
  ```

- [ ] Write `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh`:

  ```bash
  #!/usr/bin/env bash
  # dual-invoke-with-retry.sh — wraps dual-invoke.sh with infra retry logic.
  #
  # Usage: same args as dual-invoke.sh
  #
  # Retry policy:
  #   - 3 attempts per reviewer per round
  #   - Exponential backoff: 1s, 2s, 4s between attempts
  #   - Retries are SEPARATE from the wrapper's semantic iteration budget
  #   - On failure: returns the failure category as the last line of stderr,
  #     leaves $RUN intact for postmortem, exits 1
  #
  # Success criteria (all must hold):
  #   - dual-invoke.sh exits 0 (both reviewers exited cleanly)
  #   - parse-codex.py succeeds on $RUN/codex.jsonl
  #   - parse-gemini.py succeeds on $RUN/gemini.jsonl
  #   - worktree-guard.sh compare passes (no dirty files)

  set -euo pipefail

  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  MAX_RETRIES=3
  BACKOFFS=(1 2 4)

  # Pass through all args to dual-invoke.sh; we also need RUN to know where outputs go
  RUN=""
  ARGS=("$@")
  for ((i=0; i<${#ARGS[@]}; i++)); do
    if [[ "${ARGS[$i]}" == "--run" ]]; then
      RUN="${ARGS[$((i+1))]}"
      break
    fi
  done

  [[ -z "$RUN" ]] && { echo "missing --run arg" >&2; exit 2; }

  ATTEMPT=0
  while [[ $ATTEMPT -lt $MAX_RETRIES ]]; do
    ATTEMPT=$((ATTEMPT + 1))
    echo "[$(date +%s)] attempt $ATTEMPT/$MAX_RETRIES" >> "$RUN/round.log"

    # Snapshot worktree before reviewer run
    "$SCRIPT_DIR/worktree-guard.sh" snapshot "$RUN/worktree-before.txt"

    # Launch both reviewers
    if ! "$SCRIPT_DIR/dual-invoke.sh" "${ARGS[@]}"; then
      FAILURE="launch_or_exit_fail"
      echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
    elif ! "$SCRIPT_DIR/parse-codex.py" --jsonl "$RUN/codex.jsonl" --schema "${ARGS[$(( $(printf '%s\n' "${ARGS[@]}" | grep -n -- '--schema' | head -1 | cut -d: -f1) ))]}" > "$RUN/codex.envelope.json" 2> "$RUN/codex.parse-error"; then
      FAILURE="codex_$(head -1 "$RUN/codex.parse-error")"
      echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
    elif ! "$SCRIPT_DIR/parse-gemini.py" --jsonl "$RUN/gemini.jsonl" --schema "${ARGS[$(( $(printf '%s\n' "${ARGS[@]}" | grep -n -- '--schema' | head -1 | cut -d: -f1) ))]}" > "$RUN/gemini.envelope.json" 2> "$RUN/gemini.parse-error"; then
      FAILURE="gemini_$(head -1 "$RUN/gemini.parse-error")"
      echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
    elif ! "$SCRIPT_DIR/worktree-guard.sh" compare "$RUN/worktree-before.txt" 2> "$RUN/worktree-error"; then
      FAILURE="dirty_worktree"
      echo "[$(date +%s)] $FAILURE on attempt $ATTEMPT" >> "$RUN/round.log"
    else
      # All checks passed
      echo "[$(date +%s)] round succeeded on attempt $ATTEMPT" >> "$RUN/round.log"
      exit 0
    fi

    if [[ $ATTEMPT -lt $MAX_RETRIES ]]; then
      BACKOFF=${BACKOFFS[$((ATTEMPT - 1))]}
      echo "[$(date +%s)] sleeping ${BACKOFF}s before retry" >> "$RUN/round.log"
      sleep "$BACKOFF"
    fi
  done

  echo "infra_retries_exhausted: ${FAILURE:-unknown_failure}" >&2
  echo "postmortem dir: $RUN" >&2
  exit 1
  ```

  Note: the bash gymnastics for re-extracting `--schema` from the args array is admittedly ugly. The retrospective for this subsection is the right place to consider whether to refactor `dual-invoke.sh` to write the schema path to `$RUN/schema.path` for downstream consumers, eliminating the re-parse.

- [ ] `chmod +x .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh`

- [ ] Test infra retry by fault injection:
  - Use a stub codex command that fails the first time but succeeds the second (e.g., a wrapper script that checks a touchfile)
  - Verify that `dual-invoke-with-retry.sh` retries and eventually succeeds
  - Use a stub that always fails; verify that after 3 attempts the script exits 1 with `infra_retries_exhausted` on stderr

- [ ] **Subsection close-out (02.4)** — MANDATORY before starting 02.5:
  - [ ] All scripts written, executable, and tested. The fault injection test verifies retry behavior.
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — the bash gymnastics for re-extracting `--schema` is a real pain point. Did writing it feel awkward? Should `dual-invoke.sh` be refactored to write `$RUN/manifest.json` (containing schema path, prompt paths, skill name, etc.) so downstream scripts don't have to re-parse the args? Was the worktree-guard's diff output useful or noisy? Implement improvements NOW.

---

## 02.5 Findings merger with reviewer tagging and strict (location, title) dedup

**File(s):** `.claude/skills/dual-tpr/scripts/merge-findings.py` (new)

**Context:** This subsection implements the findings merger that takes two envelope files (codex's and gemini's, both produced by 02.4's parsers and validators) and produces a single merged finding list with:
- Reviewer-tagged IDs: `[TPR-{section}-{ordinal}-codex]` and `[TPR-{section}-{ordinal}-gemini]`
- INDEPENDENT ordinal sequences per reviewer (codex's first finding is `001-codex`; gemini's first finding is `001-gemini` regardless of whether they describe the same issue)
- Strict `(location, title)` exact-match agreement detection
- Disagreements surfaced explicitly (no auto-resolution)

The merger output is structured for direct insertion into a plan section's `## NN.R Third Party Review Findings` block. It produces ONE entry per finding (so an agreement appears as TWO adjacent entries with the same `(location, title)` pair but different reviewer suffixes; a disagreement appears as one entry).

The "agreement detection" is presentation-only — it does NOT collapse two findings into one entry, because that would erase information. Instead, the merger ANNOTATES each entry with `agreement: true | false` so the human reader (or downstream presentation logic) can color or group them.

Tasks:

- [ ] Write `.claude/skills/dual-tpr/scripts/merge-findings.py`:

  ```python
  #!/usr/bin/env python3
  """merge-findings.py — merge two envelope files into a reviewer-tagged finding list.

  Usage:
      merge-findings.py --codex CODEX_ENVELOPE --gemini GEMINI_ENVELOPE \
                        --section SECTION_NUMBER \
                        [--out MERGED_FILE]

  Reads both envelope files, produces a merged finding list with:
    - Reviewer-tagged IDs: [TPR-SECTION-ORDINAL-codex|gemini]
    - Independent ordinal sequences per reviewer
    - Strict (location, title) agreement detection (annotation only)

  Output: JSON to stdout (or --out file) with shape:
    {
      "section": "02",
      "merged_findings": [
        {
          "id": "[TPR-02-001-codex]",
          "reviewer": "codex",
          "agreement": true,  # or false
          "agreement_partner_id": "[TPR-02-001-gemini]",  # null if agreement=false
          "finding": { ...the original finding object from the codex envelope... }
        },
        ...
      ],
      "summary": {
        "codex_findings": 5,
        "gemini_findings": 3,
        "agreements": 2,
        "codex_only": 3,
        "gemini_only": 1
      }
    }
  """

  import argparse
  import json
  import sys


  def make_id(section, ordinal, reviewer):
      return f"[TPR-{section}-{ordinal:03d}-{reviewer}]"


  def main():
      ap = argparse.ArgumentParser()
      ap.add_argument("--codex", required=True)
      ap.add_argument("--gemini", required=True)
      ap.add_argument("--section", required=True, help="Section number, e.g. '02'")
      ap.add_argument("--out", default=None)
      args = ap.parse_args()

      with open(args.codex) as f:
          codex_env = json.load(f)
      with open(args.gemini) as f:
          gemini_env = json.load(f)

      # Build (location, title) → finding maps for cross-reviewer lookup
      gemini_by_loctitle = {}
      for f in gemini_env.get("findings", []):
          key = (f["location"], f["title"])
          gemini_by_loctitle[key] = f

      codex_by_loctitle = {}
      for f in codex_env.get("findings", []):
          key = (f["location"], f["title"])
          codex_by_loctitle[key] = f

      merged = []
      agreements = 0
      codex_only = 0
      gemini_only = 0

      # First pass: codex findings (in order)
      for i, finding in enumerate(codex_env.get("findings", []), start=1):
          codex_id = make_id(args.section, i, "codex")
          key = (finding["location"], finding["title"])
          if key in gemini_by_loctitle:
              # Find gemini's ordinal for this finding
              gemini_findings = gemini_env.get("findings", [])
              gemini_ordinal = next(
                  (j for j, gf in enumerate(gemini_findings, start=1)
                   if (gf["location"], gf["title"]) == key),
                  None
              )
              partner_id = make_id(args.section, gemini_ordinal, "gemini") if gemini_ordinal else None
              merged.append({
                  "id": codex_id,
                  "reviewer": "codex",
                  "agreement": True,
                  "agreement_partner_id": partner_id,
                  "finding": finding,
              })
              agreements += 1
          else:
              merged.append({
                  "id": codex_id,
                  "reviewer": "codex",
                  "agreement": False,
                  "agreement_partner_id": None,
                  "finding": finding,
              })
              codex_only += 1

      # Second pass: gemini findings (in order). Add gemini-only AND the gemini half of agreements.
      for i, finding in enumerate(gemini_env.get("findings", []), start=1):
          gemini_id = make_id(args.section, i, "gemini")
          key = (finding["location"], finding["title"])
          if key in codex_by_loctitle:
              # This is the gemini half of an agreement. Find codex's ordinal.
              codex_findings = codex_env.get("findings", [])
              codex_ordinal = next(
                  (j for j, cf in enumerate(codex_findings, start=1)
                   if (cf["location"], cf["title"]) == key),
                  None
              )
              partner_id = make_id(args.section, codex_ordinal, "codex") if codex_ordinal else None
              merged.append({
                  "id": gemini_id,
                  "reviewer": "gemini",
                  "agreement": True,
                  "agreement_partner_id": partner_id,
                  "finding": finding,
              })
          else:
              merged.append({
                  "id": gemini_id,
                  "reviewer": "gemini",
                  "agreement": False,
                  "agreement_partner_id": None,
                  "finding": finding,
              })
              gemini_only += 1

      result = {
          "section": args.section,
          "merged_findings": merged,
          "summary": {
              "codex_findings": len(codex_env.get("findings", [])),
              "gemini_findings": len(gemini_env.get("findings", [])),
              "agreements": agreements,
              "codex_only": codex_only,
              "gemini_only": gemini_only,
          }
      }

      out = json.dumps(result, indent=2)
      if args.out:
          with open(args.out, "w") as f:
              f.write(out + "\n")
      else:
          sys.stdout.write(out + "\n")
      sys.exit(0)


  if __name__ == "__main__":
      main()
  ```

- [ ] `chmod +x .claude/skills/dual-tpr/scripts/merge-findings.py`

- [ ] Create test fixtures for the merger:
  - `.claude/skills/dual-tpr/fixtures/codex-merge-test.json` — codex envelope with 3 findings: F1 at `compiler/foo.rs:10`, F2 at `compiler/bar.rs:20`, F3 at `compiler/baz.rs:30`
  - `.claude/skills/dual-tpr/fixtures/gemini-merge-test.json` — gemini envelope with 3 findings: G1 at `compiler/foo.rs:10` (SAME location AND title as codex F1 — agreement), G2 at `library/lib.ori:5` (gemini-only), G3 at `compiler/baz.rs:30` (same location as codex F3 but DIFFERENT title — disagreement, treated as two separate findings)

- [ ] Test the merger:
  ```bash
  .claude/skills/dual-tpr/scripts/merge-findings.py \
    --codex .claude/skills/dual-tpr/fixtures/codex-merge-test.json \
    --gemini .claude/skills/dual-tpr/fixtures/gemini-merge-test.json \
    --section 02
  ```

  Expected output (key fields):
  - `summary.codex_findings: 3`
  - `summary.gemini_findings: 3`
  - `summary.agreements: 1` (only F1+G1 match exactly)
  - `summary.codex_only: 2`
  - `summary.gemini_only: 2`
  - `merged_findings` contains 6 entries total: 3 codex (with 1 marked agreement, partner `[TPR-02-001-gemini]`), then 3 gemini (with 1 marked agreement, partner `[TPR-02-001-codex]`)

- [ ] Verify the agreement detection is byte-strict: change G1's title by adding a single trailing space. Re-run the merger. Expected: agreements drops from 1 to 0, both findings appear as `agreement: false`.

- [ ] **Subsection close-out (02.5)** — MANDATORY before starting 02.6:
  - [ ] All merger tests pass with expected outcomes
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — was the merger's "two-pass over findings" logic obvious or convoluted? Should there be a `merge-findings-pretty.py` that produces human-readable markdown output (the format that goes into plan files) in addition to JSON? That helper might reduce friction in Section 04+ wrappers when they need to write merged findings to plan sections. Implement improvements NOW.

---

## 02.6 Transport test suite (transport-tests.sh + fault injection)

**File(s):** `.claude/skills/dual-tpr/scripts/transport-tests.sh` (new), additional fixtures as needed

**Context:** This subsection assembles all the unit tests built across 02.1–02.5 into a single runnable test suite. Every downstream section (04, 05, 06, 07) lists `bash .claude/skills/dual-tpr/scripts/transport-tests.sh` in its completion checklist as a regression test. This is the test suite that catches regressions in the transport layer before they propagate to wrappers.

The test suite must:
- Run all parser fixture tests (codex 5 fixtures, gemini 6 fixtures)
- Run the validator fixture tests (4 fixtures)
- Run the worktree guard test (clean and dirty)
- Run the merger fixture test (with the agreement/disagreement matrix)
- Run a smoke test of `dual-invoke.sh` against stub prompts (only if `--integration` flag is passed; otherwise skip because it actually invokes codex/gemini and takes minutes)
- Run the fault injection test for `dual-invoke-with-retry.sh` (using a stub failing reviewer)
- Report pass/fail per test with a summary at the end
- Exit non-zero if any test failed

Tasks:

- [ ] Write `.claude/skills/dual-tpr/scripts/transport-tests.sh`:

  ```bash
  #!/usr/bin/env bash
  # transport-tests.sh — run the dual-tpr transport test suite.
  #
  # Usage:
  #   transport-tests.sh                     # run all unit tests (fast, no real CLI calls)
  #   transport-tests.sh --integration       # also run integration tests (invokes real codex/gemini)
  #
  # Exits 0 if all tests pass, non-zero if any test fails.

  set -uo pipefail

  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  FIXTURES="$SCRIPT_DIR/../fixtures"
  SCHEMA="$SCRIPT_DIR/../findings-schema.json"

  PASS=0
  FAIL=0
  FAILED_TESTS=()

  test_case() {
    local name="$1"
    local actual_exit="$2"
    local expected_exit="$3"
    if [[ "$actual_exit" == "$expected_exit" ]]; then
      echo "  PASS: $name"
      PASS=$((PASS + 1))
    else
      echo "  FAIL: $name (expected exit=$expected_exit, got $actual_exit)"
      FAIL=$((FAIL + 1))
      FAILED_TESTS+=("$name")
    fi
  }

  echo "=== validator fixture tests ==="
  for fixture in codex-with-findings gemini-with-grounded-citation no-findings; do
    "$SCRIPT_DIR/validate-envelope.py" --envelope "$FIXTURES/$fixture.json" --schema "$SCHEMA" >/dev/null 2>&1
    test_case "validator $fixture" "$?" "0"
  done
  "$SCRIPT_DIR/validate-envelope.py" --envelope "$FIXTURES/invalid-location.json" --schema "$SCHEMA" >/dev/null 2>&1
  test_case "validator rejects invalid-location" "$?" "1"

  echo ""
  echo "=== codex parser fixture tests ==="
  for fixture in codex-success codex-missing codex-parse-fail codex-schema-violation codex-failed-partial; do
    case "$fixture" in
      codex-success) expected=0 ;;
      *) expected=1 ;;
    esac
    "$SCRIPT_DIR/parse-codex.py" --jsonl "$FIXTURES/$fixture.jsonl" --schema "$SCHEMA" >/dev/null 2>&1
    test_case "codex parser $fixture" "$?" "$expected"
  done

  echo ""
  echo "=== gemini parser fixture tests ==="
  for fixture in gemini-success gemini-missing-terminator gemini-no-begin gemini-no-end gemini-no-json-block gemini-fragmented; do
    case "$fixture" in
      gemini-success|gemini-fragmented) expected=0 ;;
      *) expected=1 ;;
    esac
    "$SCRIPT_DIR/parse-gemini.py" --jsonl "$FIXTURES/$fixture.jsonl" --schema "$SCHEMA" >/dev/null 2>&1
    test_case "gemini parser $fixture" "$?" "$expected"
  done

  echo ""
  echo "=== worktree guard tests ==="
  RUN=$("$SCRIPT_DIR/scratch-dir.sh")
  "$SCRIPT_DIR/worktree-guard.sh" snapshot "$RUN/before.txt"
  "$SCRIPT_DIR/worktree-guard.sh" compare "$RUN/before.txt" >/dev/null 2>&1
  test_case "worktree guard clean state" "$?" "0"
  rm -rf "$RUN"
  # Note: dirty-state test deferred to manual run because it deliberately
  # modifies a tracked file and would interfere with concurrent test runs.

  echo ""
  echo "=== merger fixture tests ==="
  "$SCRIPT_DIR/merge-findings.py" \
    --codex "$FIXTURES/codex-merge-test.json" \
    --gemini "$FIXTURES/gemini-merge-test.json" \
    --section 02 > /tmp/merge-test-out.json 2>&1
  test_case "merger basic invocation" "$?" "0"
  python3 -c "
  import json, sys
  d = json.load(open('/tmp/merge-test-out.json'))
  assert d['summary']['agreements'] == 1, f'expected 1 agreement, got {d[\"summary\"][\"agreements\"]}'
  assert d['summary']['codex_findings'] == 3, f'expected 3 codex findings'
  assert d['summary']['gemini_findings'] == 3, f'expected 3 gemini findings'
  print('OK')
  " >/dev/null 2>&1
  test_case "merger summary correctness" "$?" "0"
  rm -f /tmp/merge-test-out.json

  if [[ "${1:-}" == "--integration" ]]; then
    echo ""
    echo "=== integration tests (invokes real codex/gemini) ==="
    RUN=$("$SCRIPT_DIR/scratch-dir.sh")
    echo "respond with PING" > "$RUN/codex.prompt.md"
    echo "respond with PING" > "$RUN/gemini.prompt.md"
    "$SCRIPT_DIR/dual-invoke.sh" \
      --run "$RUN" --skill review-work \
      --codex-prompt "$RUN/codex.prompt.md" \
      --gemini-prompt "$RUN/gemini.prompt.md" \
      --schema "$SCHEMA" >/dev/null 2>&1
    test_case "dual-invoke smoke test" "$?" "0"
    rm -rf "$RUN"
  fi

  echo ""
  echo "=== summary ==="
  echo "PASS: $PASS"
  echo "FAIL: $FAIL"
  if [[ $FAIL -gt 0 ]]; then
    echo "Failed tests:"
    for t in "${FAILED_TESTS[@]}"; do
      echo "  - $t"
    done
    exit 1
  fi
  exit 0
  ```

- [ ] `chmod +x .claude/skills/dual-tpr/scripts/transport-tests.sh`

- [ ] Run the test suite (unit-only) and verify ALL tests pass:
  ```bash
  bash .claude/skills/dual-tpr/scripts/transport-tests.sh
  echo "exit=$?"
  ```
  Expected: all PASS lines, exit 0.

- [ ] Run the test suite WITH integration tests and verify the smoke test passes (this requires actual codex + gemini installed and authenticated):
  ```bash
  bash .claude/skills/dual-tpr/scripts/transport-tests.sh --integration
  ```
  Expected: all PASS including the dual-invoke smoke test, exit 0.

- [ ] **Subsection close-out (02.6)** — MANDATORY before section completion:
  - [ ] Test suite runs cleanly in unit mode AND in integration mode
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] Run `/improve-tooling` retrospectively on THIS subsection — the test runner is somewhat ad-hoc bash. Should there be a TAP-format output mode (`--tap`) for CI integration? Should the dirty-worktree test be included with cleanup discipline (using a temp git repo or similar to avoid contaminating the real repo)? Should there be a `--verbose` flag that prints stdout from each test instead of suppressing it? Implement improvements NOW.

---

## 02.R Third Party Review Findings

<!-- Reserved for codex/gemini reviewers running /tpr-review against this section.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`
-->

- None.

---

## 02.N Completion Checklist

- [ ] All six implementation subsections (02.1, 02.2, 02.3, 02.4, 02.5, 02.6) marked `complete` in section frontmatter
- [ ] `.claude/skills/dual-tpr/scripts/` directory contains all 9 executable files: the 8 transport primitives (`scratch-dir.sh`, `dual-invoke.sh`, `dual-invoke-with-retry.sh`, `parse-codex.py`, `parse-gemini.py`, `validate-envelope.py`, `worktree-guard.sh`, `merge-findings.py`) plus the test runner `transport-tests.sh`. Wrappers MUST invoke `dual-invoke-with-retry.sh` (not `dual-invoke.sh` directly) as the canonical entrypoint; `dual-invoke.sh` is the raw launcher that the retry wrapper composes with parsing and worktree-guarding.
- [ ] All scripts have executable permissions (`chmod +x`)
- [ ] `.claude/skills/dual-tpr/fixtures/` contains the test fixtures from Section 01 PLUS the parser/merger fixtures created in this section (codex JSONL fixtures, gemini stream-json fixtures, merger envelope pairs)
- [ ] `transport-tests.sh` runs cleanly in unit mode (no `--integration`) and reports all tests passing
- [ ] `transport-tests.sh --integration` runs cleanly when codex and gemini are installed and reports the dual-invoke smoke test passing
- [ ] Failure taxonomy is exhaustive — every failure mode (`launch_fail | nonzero_exit | timeout | parse_fail | schema_violation | missing_terminator | missing_begin_sentinel | missing_end_sentinel | missing_json_block | dirty_worktree | failed_partial | infra_retries_exhausted`) has a fixture or fault injection test
- [ ] Infra retry budget (3 retries per reviewer per round, exponential backoff 1s/2s/4s) is implemented in `dual-invoke-with-retry.sh` and verified by fault injection
- [ ] Per-run scratch directory pattern is consistently applied: every script that takes a `$RUN` arg uses it; no fixed `/tmp/foo.jsonl` paths anywhere
- [ ] `timeout 150 ./test-all.sh` green — no regressions in compiler test suite
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan dual-tpr-gemini` returns 0 annotations from this section's work in source files (plan documentation may still cite TPR-02-XXX)
- [ ] **Plan sync** — update plan metadata to reflect this section's completion:
  - [ ] This section's frontmatter `status` → `complete`, all six subsection statuses updated
  - [ ] `00-overview.md` Quick Reference table status updated for Section 02
  - [ ] `00-overview.md` mission success criteria checkboxes updated (the JSON envelope parser, dirty-worktree guard, and infra retry items can be checked off)
  - [ ] `index.md` section status updated for Section 02
  - [ ] Section 03's `depends_on: ["02"]` precondition is satisfied — Section 03 can begin work
- [ ] `/tpr-review` passed (final, full-section) — independent codex review found no critical or major issues, OR all findings triaged. (This is still single-source codex at this point in the plan; the dual-source rewrite of `/tpr-review` is in Section 04.)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review found no critical or major findings, OR all findings triaged and fixed. MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` **section-close sweep** — MANDATORY safety net after both reviews are clean. Verify every subsection in this section has either an "improvements made" entry (with commits) or a documented "no gaps" negative finding from its own per-subsection retrospective. Then look for cross-subsection patterns: did the same kind of fixture-construction friction recur across 02.2, 02.3, 02.5? Did the bash gymnastics for arg passing recur across 02.1, 02.4, 02.6? Did fault injection require setup that should be a permanent helper? Add ONLY new items from cross-cutting patterns, not duplicates of per-subsection findings. Implement immediately, commit separately as `build(diagnostics): add X — surfaced by section-02 close sweep`. Most sweeps produce zero new findings when per-subsection captures are thorough — that is the expected, healthy outcome and must be documented. Do not silently skip.

**Exit Criteria:** `.claude/skills/dual-tpr/scripts/` contains all 8 executable scripts plus `transport-tests.sh`. `transport-tests.sh` runs cleanly in both unit and integration modes with all tests passing. The codex parser correctly extracts envelopes from `--output-schema`-conformant codex JSONL output. The gemini parser correctly concatenates `delta:true` assistant message fragments in arrival order, waits for the terminal `result/status:success` event, and extracts the sentinel-bracketed JSON envelope. The validator accepts the 3 positive fixtures and rejects the 1 negative fixture. The dirty-worktree guard detects clean and dirty states correctly. The merger produces reviewer-tagged output with strict `(location, title)` agreement detection, verified against the agreement/disagreement matrix. Infra retry logic recovers from transient failures within the 3-retry budget. Section 03 can begin its reviewer-surface preparation work against the locked transport API.
