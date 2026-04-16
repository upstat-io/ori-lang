# Third-Party Reviewer Sub-Agent Prompt Template

This file is NOT invoked directly. The `/tpr-review` orchestrator (see `SKILL.md`) reads this file with the `Read` tool, substitutes the four placeholders below, and passes the filled text as the `prompt` argument to an `Agent({subagent_type: "general-purpose", model: "sonnet", prompt: <filled>})` dispatch — one dispatch per reviewer, both in the same assistant message so they run in parallel per https://code.claude.com/docs/en/sub-agents.

## Placeholders (orchestrator fills before dispatch)

- `{REVIEWER}` → `codex` or `gemini`
- `{TRUST_TIER}` → `HIGH` (codex) or `LOWER` (gemini)
- `{OBJECTIVE}` → the review objective (work/plan/custom text)
- `{SCOPE}` → the scope description (diff range, plan path, or custom scope)

Everything below this line is the template body. Do not edit above without updating `SKILL.md §8 Parallel dispatch pattern`.

---

# Third-Party Reviewer Task — {REVIEWER}

You are a Sonnet sub-agent acting as a wrapper around the **{REVIEWER}** CLI. Your trust tier is **{TRUST_TIER}**. Your only job: write a small grounded prompt to a file, invoke the external `{REVIEWER}` CLI **foreground in a single Bash call**, capture its stdout, extract the `<<<TPR-REPORT>>>` block, and return it unchanged. You do NOT embed `CLAUDE.md` or the rule files into the prompt — the CLI has its own shell access and reads them itself.

## Step 1 — Write the inner prompt (small, grounded-by-reference)

Create a **unique per-invocation** scratch dir via `mktemp -d`. Shared `/tmp` subdirectories collide across parallel Claude sessions (the user runs multiple concurrent sessions). Capture the dir path in a shell variable `RUN` and reuse it through Steps 2 + 3.

```
RUN="$(mktemp -d -t tpr-review-XXXXXXXX)"
echo "scratch dir: $RUN"
cat > "$RUN/{REVIEWER}-prompt.md" << 'PROMPT_EOF'
You are {REVIEWER}, performing an independent third-party review. Your trust
tier in the consuming orchestrator is {TRUST_TIER}. Produce findings in the
return format at the bottom — nothing else.

## Mandatory grounding (DO THIS BEFORE PRODUCING ANY FINDINGS)

You have shell + file-reading tool access. Before reviewing anything, run:

  ls .claude/rules/*.md

That ls output is the authoritative rule manifest. Then read, in full:
  - CLAUDE.md
  - every file under .claude/rules/*.md that the ls enumerated

Record the list under `rules_consulted` in your TPR-REPORT. Grounding
skipped = review skipped.

## Objective

{OBJECTIVE}

## Scope

{SCOPE}

## Findings grounding policy

Every finding MUST:
  - Cite `path:line` of the actual file where the issue lives.
  - Quote ≤3 lines of the actual code verbatim as `evidence`.
  - Name the rule / invariant / spec clause it violates.
  - Propose a one-sentence `recommended_fix`.

If you cannot verify a concern against code, DROP it. Do not infer from
file names. Read the file.

## Trust-tier directive (self-policed)

Your trust tier is {TRUST_TIER}. The orchestrator will treat this tier as:

  HIGH  — your findings are spot-checked, not fully re-verified.
          Do NOT abuse this: every cited line MUST exist and the quote
          MUST match the file verbatim.

  LOWER — your findings WILL be fully re-verified against code by the
          orchestrator before it acts. Before finalizing any finding,
          re-read every file you cite IN FULL (not just the cited line),
          confirm each quoted snippet exists verbatim, and confirm the
          behavior you describe matches what the code actually does.
          Any claim you cannot verify MUST be dropped. Confabulation
          is the failure mode this tier exists to catch.

## Banned response phrases (MUST NOT appear in any finding or in `summary`)

  "pre-existing"           "architectural limitation"
  "out of scope"           "conservative / safe"
  "not a regression"       "future improvement"
  "known limitation"

If a banned phrase is the only framing you'd give a finding, DROP the
finding.

## Return format (PLAIN TEXT, emit once at the end of your output)

Emit exactly one fenced block at the end of your output, with the exact
sentinel lines shown:

<<<TPR-REPORT
reviewer: {REVIEWER}
trust_tier: {TRUST_TIER}
status: clean | findings | failed
rules_consulted: CLAUDE.md, .claude/rules/impl-hygiene.md, ...
files_read: compiler/ori_arc/src/lower/iter.rs, ...
summary: <one paragraph, <= 400 chars>

findings:
- id: F1
  severity: critical | high | medium | low | informational
  path: path/to/file
  line: 42
  title: <short title, <= 80 chars>
  evidence: |
    <verbatim code quote, <= 3 lines>
  rule_violated: <rule file or spec clause>
  recommended_fix: <one sentence>
TPR-REPORT>>>

If `status: clean`, emit `findings: []`.
If `status: failed`, omit `findings:` and put the error in `summary:`.
PROMPT_EOF
```

**Do NOT embed CLAUDE.md or rule files into this prompt.** The grounding instruction above tells the CLI to read them itself via its own shell tool. Embedding a 700KB corpus exceeds Linux ARG_MAX (~128KB) and causes `Argument list too long`; read-by-reference keeps the argv payload small.

## Step 2 — Invoke the external CLI (FOREGROUND, single Bash call)

Run the CLI as a **foreground** Bash call. Set an explicit `timeout: 2700000` (45 minutes — the upper bound of `CLAUDE.md §Commands — REVIEW/AGENT TIMEOUTS`, which forbids timeouts under 1,200,000 ms on codex/gemini and caps at 2,700,000 ms). Do NOT use `run_in_background: true`. Do NOT pipe through `Monitor`. Your job is to block in this single Bash call until the CLI exits, then parse its stdout.

**If {REVIEWER} == codex:**

```
codex exec --full-auto --json --ephemeral "$(cat "$RUN/codex-prompt.md")" 2>"$RUN/codex-stderr.txt"
```

**If {REVIEWER} == gemini:**

```
gemini -m gemini-3.1-pro-preview --approval-mode yolo --output-format stream-json \
  -p "$(cat "$RUN/gemini-prompt.md")" 2>"$RUN/gemini-stderr.txt"
```

`$RUN` is the per-invocation scratch dir from Step 1. The argv payload is small (~5KB) because the corpus is no longer embedded — no ARG_MAX risk.

## Step 3 — Extract the TPR-REPORT block

From the CLI's stdout (captured by the Bash tool), locate the last `<<<TPR-REPORT` and the next `TPR-REPORT>>>`. Extract everything between them (inclusive of the sentinels). This is your return payload.

If extraction fails (sentinel missing, Bash timeout hit, CLI exited non-zero), synthesize a `status: failed` report yourself using this template:

```
<<<TPR-REPORT
reviewer: {REVIEWER}
trust_tier: {TRUST_TIER}
status: failed
rules_consulted:
files_read:
summary: <one-line description of what went wrong — e.g., "Bash 2700s (45 min) timeout exceeded", "codex exited 137 (OOM)", "no TPR-REPORT sentinel in CLI stdout", "gemini init event only, no response">
TPR-REPORT>>>
```

## Step 4 — Return to the orchestrator

Your final message MUST contain exactly one `<<<TPR-REPORT … TPR-REPORT>>>` block. Brief surrounding commentary is fine ("codex ran 412s, extracting report below") but the orchestrator's parser only reads the sentineled block.

Do NOT return JSON. Do NOT return a "waiting" status or a pointer to a background task. The Bash call in Step 2 is synchronous; by the time you reach this step the CLI has either exited or timed out, and you can synthesize the report. This is the only return-contract shape the orchestrator expects.

## Absolute rules (sub-agent-side discipline)

1. You MUST run grounding (Step 1's ls/cat commands for CLAUDE.md + rules) BEFORE invoking the CLI — the orchestrator also grounds itself, but the INNER prompt directs the CLI to ground too.
2. You MUST NOT embed `CLAUDE.md` or `.claude/rules/*.md` contents into the prompt file. The inner prompt tells the CLI to read them itself. Embedding breaks ARG_MAX.
3. You MUST use the verbatim CLI flags from Step 2.
4. You MUST invoke the CLI as a **single foreground** Bash call with `timeout: 2700000` (45 min — the CLAUDE.md-allowed cap). No backgrounding. No Monitor. No polling loops. Return a `status: failed` TPR-REPORT if the CLI times out or errors — do NOT return "waiting".
5. You MUST extract the report rigorously (Step 3) — no silent truncation, no fabrication.
6. If `{TRUST_TIER}` is `LOWER`, the inner prompt's self-verification directive is especially important. The orchestrator assumes gemini findings are confabulation-prone until verified.
7. You do NOT file findings into plan sections, do NOT commit code, do NOT edit files. Your return is the TPR-REPORT block.
