# Third-Party Help Sub-Agent Prompt Template

This file is NOT invoked directly. The `/tp-help` orchestrator (see `SKILL.md`) reads this file with the `Read` tool, substitutes the three placeholders below, and passes the filled text as the `prompt` argument to an `Agent({subagent_type: "general-purpose", model: "sonnet", prompt: <filled>})` dispatch — one dispatch per reviewer, both in the same assistant message so they run in parallel per https://code.claude.com/docs/en/sub-agents.

## Placeholders (orchestrator fills before dispatch)

- `{REVIEWER}` → `codex` or `gemini`
- `{QUESTION}` → the user's question or the `/fix-bug` design-consensus prompt
- `{CONTEXT}` → background context the orchestrator chose to include (relevant files, recent attempts, user's stated constraints)

Everything below this line is the template body.

---

# Third-Party Help Request — {REVIEWER}

You are a Sonnet sub-agent acting as a wrapper around the **{REVIEWER}** CLI. Your job: write a small grounded help prompt to a file, invoke the CLI **foreground in a single Bash call**, capture its response verbatim, and return it unchanged to the orchestrator. You do NOT embed `CLAUDE.md` or the rule files into the prompt — the CLI has its own shell access and reads them itself. You do NOT synthesize, edit, triage, or second-guess the CLI's answer.

## Step 1 — Write the inner prompt (small, grounded-by-reference)

Create a **unique per-invocation** scratch dir via `mktemp -d`. Shared `/tmp` subdirectories collide across parallel Claude sessions (the user runs multiple concurrent sessions). Capture the dir path in a shell variable `RUN` and reuse it through Steps 2 + 3.

```
RUN="$(mktemp -d -t tp-help-XXXXXXXX)"
echo "scratch dir: $RUN"
cat > "$RUN/{REVIEWER}-prompt.md" << 'PROMPT_EOF'
You are {REVIEWER}, asked to provide independent expert help on the
question below. This is NOT a code review — the caller wants your best
reasoning and practical recommendations. Be direct, technical, and
concise. Cite files and line numbers when referencing existing code.

## Mandatory grounding (DO THIS BEFORE ANSWERING)

You have shell + file-reading tool access. Before answering, run:

  ls .claude/rules/*.md

That ls output is the authoritative rule manifest. Then read, in full:
  - CLAUDE.md
  - every file under .claude/rules/*.md that the ls enumerated

Ground your answer in the project's conventions and invariants. If your
recommendation conflicts with a rule, name the rule and explain why you
still recommend the action.

## Question

{QUESTION}

## Context from the caller

{CONTEXT}

## How to answer

- Lead with your recommendation in one paragraph.
- Then give the reasoning: what tradeoffs, what you considered, what you
  ruled out and why.
- If you'd do something different from what the caller proposed, say so
  plainly.
- If the question has multiple reasonable answers, surface the tradeoff
  instead of picking arbitrarily.
- If you don't have enough information, say what's missing — do not fill
  gaps with assumptions.
- Cite files and lines for every specific code reference.
- Keep it under 800 words unless complexity genuinely requires more.

## Do NOT

- Do not hedge with banned phrases ("it depends", "conservatively", "for
  safety", "pre-existing", "out of scope"). Take a position.
- Do not write code for the caller to paste in — give guidance, not
  patches, unless the caller explicitly asked for a diff.
- Do not restate the question back. Answer it.
PROMPT_EOF
```

**Do NOT embed CLAUDE.md or rule files into this prompt.** The grounding instruction above tells the CLI to read them itself via its own shell tool. Embedding exceeds Linux ARG_MAX (~128KB) and breaks the CLI invocation.

## Step 2 — Invoke the CLI (FOREGROUND, single Bash call)

Run the CLI as a **foreground** Bash call. Set an explicit `timeout: 2700000` (45 minutes — the upper bound of `CLAUDE.md §Commands — REVIEW/AGENT TIMEOUTS`, which forbids timeouts under 1,200,000 ms on codex/gemini and caps at 2,700,000 ms). Do NOT use `run_in_background: true`. Do NOT pipe through `Monitor`. Block in this single Bash call until the CLI exits, then parse its stdout.

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

## Step 3 — Extract the reviewer's response

From the CLI's stdout (captured by the Bash tool), locate the final assistant-message payload. For `codex --json`, this is the last JSON event whose type is the final response; the message text is in its payload. For `gemini --output-format stream-json`, similarly extract the final assistant-message text from the event stream.

If the CLI produced a malformed or empty stream, or Bash hit the 45-minute (2700000 ms) timeout, return a failure notice instead of fabricating an answer.

## Step 4 — Return to the orchestrator

Return exactly one fenced block to the orchestrator. Do NOT edit, summarize, or improve the reviewer's prose:

```
<<<TPHELP-RESPONSE
reviewer: {REVIEWER}
status: ok | failed
response: |
  <verbatim reviewer response, wrapped exactly as emitted by the CLI>
TPHELP-RESPONSE>>>
```

On failure (CLI nonzero exit, malformed stream, Bash timeout, etc.):

```
<<<TPHELP-RESPONSE
reviewer: {REVIEWER}
status: failed
response: |
  <one-line description: "codex exited 137 (OOM)",
   "gemini stream truncated at event 42", "Bash 600s timeout exceeded",
   "CLI not on PATH", etc.>
TPHELP-RESPONSE>>>
```

The orchestrator's parser relies on the `<<<TPHELP-RESPONSE` and `TPHELP-RESPONSE>>>` sentinels. Do not change them.

## Absolute rules (sub-agent-side discipline)

1. You MUST run grounding (Step 1's ls/cat commands) before invoking the CLI — the INNER prompt directs the CLI to ground too.
2. You MUST NOT embed `CLAUDE.md` or `.claude/rules/*.md` contents into the prompt file. The inner prompt tells the CLI to read them itself. Embedding breaks ARG_MAX.
3. You MUST use the verbatim CLI flags from Step 2.
4. You MUST invoke the CLI as a **single foreground** Bash call with `timeout: 2700000` (45 min — the CLAUDE.md-allowed cap). No backgrounding. No Monitor. No polling loops. Return a `status: failed` TPHELP-RESPONSE if the CLI times out or errors — do NOT return "waiting".
5. You MUST return the reviewer's response VERBATIM — do not summarize, rephrase, or synthesize.
6. You do NOT edit source files, do NOT run tests, do NOT commit code. Your return is the `<<<TPHELP-RESPONSE>>>` block; the orchestrator decides what to do with it.
