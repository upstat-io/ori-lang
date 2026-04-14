# Rules Brief Composition — Sonnet Subagent Prompt

This document is the prompt template for the Sonnet subagent that composes
tailored rules briefs for reviewer prompts. It is invoked by `/tpr-review`,
`/tp-help`, and `/review-work` before writing reviewer prompts.

## When to Use

Before writing reviewer prompts (codex.prompt.md / gemini.prompt.md), the
orchestrating skill (Claude Opus) should:

1. Run `scripts/rules-for-review.py` to classify changed files and get the
   list of relevant rule files
2. Spawn a Sonnet subagent with this prompt template, passing:
   - The classifier output (JSON with subsystems + rule file list)
   - A summary of what changed (diff stat or file list)
   - The review objective (code review, plan review, or custom)
3. Inject the subagent's output into the grounding block of both reviewer
   prompts

## Subagent Prompt Template

The following is the prompt to send to the Sonnet subagent via the Agent tool.
Replace `{CLASSIFIER_JSON}`, `{DIFF_SUMMARY}`, and `{REVIEW_OBJECTIVE}` with
actual values.

---

```
You are composing a **rules brief** for external code reviewers (Codex and
Gemini) who are about to review changes to the Ori compiler. Your job is to
read the relevant rule files and produce a focused, actionable summary that
the reviewers will use as their grounding context.

## What you're working with

**Classifier output** (which subsystems are touched and which rule files matter):
{CLASSIFIER_JSON}

**What changed** (diff summary):
{DIFF_SUMMARY}

**Review objective**: {REVIEW_OBJECTIVE}

## Your task

1. Read EVERY rule file listed under "critical" in the classifier output.
   Also read `CLAUDE.md` (project root) for the overarching project rules.

2. For each rule file, identify the specific rules, invariants, and
   constraints that are **relevant to the changed code**. A rule is relevant
   if:
   - The changed files could violate it
   - The changed code path is governed by it
   - A reviewer checking this diff should know about it to catch regressions

3. Compose a **Rules Brief** (markdown) that contains:

   a. A "Finding Vocabulary" section — the category definitions from
      impl-hygiene.md (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE)
      with their one-line meanings. This is always included.

   b. For each relevant rule file, a section containing:
      - The rule file name as a header
      - The specific rules/invariants that apply, with their anchors
        (e.g., TR-2, NR-1, UN-1, RL-2)
      - For each rule: the rule text verbatim if short (<10 lines), or a
        precise summary if long, preserving the rule anchor ID
      - Why this rule matters for this specific diff (1 sentence)

   c. A "Reference Files" section listing any "reference" priority files
      from the classifier output, with a one-line description of what
      they cover.

## Output constraints

- **Target: 200–400 lines.** This is the reviewer's primary context. Too
  short = they miss rules. Too long = they skim and miss rules. Aim for
  the sweet spot.
- **Preserve rule anchor IDs** (TR-2, NR-1, etc.) — reviewers cite these
  in findings.
- **Verbatim for short rules** — if a rule is under 10 lines, include it
  verbatim. Summarizing short rules loses precision.
- **Summarize long sections** — for sections over 20 lines (like AIMS
  lattice details or detailed ABI tables), write a precise summary that
  captures what a reviewer needs to check, not the full theory.
- **No editorializing** — don't add your own opinions about the code or
  the rules. Just present the rules that apply.
- **Use markdown** — headers, bullet points, code blocks as appropriate.
- **Include the "always" rules** (impl-hygiene finding categories, key
  test rules, phase purity) even if they seem generic — they scope the
  reviewer's vocabulary.

## What NOT to include

- Rules from files NOT in the classifier output — don't read or summarize
  rule files that aren't relevant to this diff
- Background theory or history — just the actionable rules
- The full text of very long sections (>20 lines) — summarize precisely
- Rules that have no bearing on the changed code paths

## Output format

Output ONLY the Rules Brief markdown. No preamble, no explanation of what
you did, no sign-off. Start directly with the `## Rules Brief` header.
```

---

## Integration Notes

The orchestrating skill should:

1. Capture the Sonnet subagent's output (the Rules Brief markdown)
2. Inject it into both reviewer prompts in place of the static
   "Grounding — read these files FIRST" block
3. After the inline brief, STILL include a line like:
   "For full rule details, read: {list of critical files}"
   This gives reviewers the option to deep-dive, but the inline brief
   ensures they have the key rules even if they don't.

The brief is generated fresh every time — it automatically adapts when
rule files are updated, new rules are added, or sections are restructured.
No manifest, no tags, no maintenance.
