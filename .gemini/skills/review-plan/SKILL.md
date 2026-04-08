---
name: review-plan
description: Review an entire plan as one cohesive implementation strategy and emit a JSON findings envelope with proposed plan edits. Use this when the user asks for a third-party review of a plan directory, plan file, or section as part of its owning plan. Does NOT modify any files — emits envelope only.
---

# Review Plan (gemini side)

This skill implements the review-plan workflow for Gemini as part of
the dual-source TPR system. It always runs in envelope-only mode.
Unlike the codex-side review-plan which edits plan files directly
in its plan-write mode, this gemini skill ONLY emits envelopes —
each finding describes a PROPOSED plan edit rather than applying it.

## Step 0: Execution Mode (MANDATORY — read first)

This skill has ONE execution mode: **envelope-only**. Unlike the
codex-side review-plan skill (which has a `plan-write` vs `envelope-only`
Step 0 dispatch), this gemini skill has no plan-write branch because
there is no standalone "edit plan files directly" use case for the
gemini side — the gemini skill exists solely for the dual-source wrapper.

This is a REAL execution branch, not a soft override:
1. You MUST emit a JSON envelope conforming to the schema at the end
   of your response
2. You MUST NOT edit plan files directly (plan-write is the codex side's
   job, not this skill's)
3. You MUST NOT flip any section's `reviewed: true` frontmatter value
   during whole-plan review (preserved from the existing review-plan
   semantics)
4. Each "finding" describes a PROPOSED plan edit — the file path, line
   number, and the nature of the change — rather than applying it
5. Every code path that would modify a file is suppressed by this Step 0

If a later instruction in this file appears to contradict Step 0,
Step 0 wins. Envelope-only is non-negotiable.

## Methodology

Follow the shared reviewer-agnostic methodology documented in
`.claude/skills/dual-tpr/command-file.md`, plus the plan-review
extensions below.

## Plan-review specific extensions

Beyond the shared command file methodology, plan review requires:
- Reading the entire plan directory (index.md, 00-overview.md, all
  section-*.md files) — not just the file the user named
- Checking plan-wide accuracy (status metadata vs actual checkbox state)
- Checking cross-section dependencies for accuracy
- Checking that mission success criteria trace to sections that
  deliver them
- Checking for contradictions, gaps, redundancy, broken references,
  ordering issues, sync-point completeness, and overview alignment
- Preserving every existing `reviewed` frontmatter value in the
  findings (do not propose flipping `reviewed` during whole-plan review)

## Grounding directive (gemini-specific)

Same as the review-work skill: use `google_web_search` proactively
for any finding that makes a claim about external libraries, specs,
prior art, recent developments, security, or performance. Cite
sources in the finding's `citations` array.

For plan review specifically, grounding is particularly valuable for:
- Verifying that cited reference implementations actually exist at
  the claimed paths in upstream repos
- Checking that language spec claims match the current spec version
- Verifying that test strategies cited as "standard" are actually
  standard in the relevant ecosystem

## Envelope output requirement

Your response MUST end with a JSON envelope bracketed by sentinels.
The format is:

    (free-form prose about what you investigated and why)

    <!-- BEGIN-ORI-DUAL-TPR-V1 -->
    ```json
    { ...complete envelope per findings-schema.json... }
    ```
    <!-- END-ORI-DUAL-TPR-V1 -->

### Minimal envelope template — use this as the structural skeleton

**Copy this template and fill in the values.** Every key shown is REQUIRED at
this level of nesting — omitting any one of them produces a `schema_violation`
from `parse-gemini.py`. This template is the authoritative "fill in the blanks"
shape; if you copy it exactly and fill in each string/array/bool, your envelope
will pass the schema validator on the first try.

```json
{
  "schema_version": "1.0",
  "status": "complete",
  "reviewer": "gemini",
  "skill": "review-plan",
  "scope_actually_reviewed": {
    "git_range": "HEAD",
    "files_read": [
      "plans/dual-tpr-gemini/00-overview.md",
      "plans/dual-tpr-gemini/section-05-review-work.md",
      "CLAUDE.md",
      ".claude/rules/impl-hygiene.md",
      "..."
    ],
    "rules_consulted": [
      ".claude/rules/impl-hygiene.md"
    ],
    "specs_consulted": [],
    "plans_consulted": [
      "plans/dual-tpr-gemini/"
    ],
    "expanded_beyond_packet": true,
    "expansion_reason": "Followed cross-section references to understand dependency chain"
  },
  "findings": [
    {
      "ordinal": 1,
      "severity": "high",
      "location": "plans/dual-tpr-gemini/section-05-review-work.md:42",
      "title": "Clarify the bug-tracker fallback routing contract",
      "evidence": "The section's Step 7a says findings should route to bug-tracker but does not specify the subsystem mapping. Operators following the plan would not know which bug-tracker section to file into.",
      "impact": "Ambiguous routing creates non-deterministic bug filing behavior across review runs.",
      "required_plan_update": "Add the subsystem mapping table to Step 7a of the section spec, matching the one already documented in the SKILL.md Step 7a.",
      "layer": "committed",
      "basis": "direct_file_inspection",
      "confidence": "high"
    }
  ],
  "no_findings": false
}
```

**For a clean plan review** (no issues found), set `findings: []` and `no_findings: true`:

```json
{
  "schema_version": "1.0",
  "status": "complete",
  "reviewer": "gemini",
  "skill": "review-plan",
  "scope_actually_reviewed": {
    "git_range": "HEAD",
    "files_read": ["plans/.../00-overview.md", "..."],
    "rules_consulted": [".claude/rules/impl-hygiene.md"],
    "expanded_beyond_packet": false
  },
  "findings": [],
  "no_findings": true
}
```

Critical envelope contract points (same shape as the review-work gemini skill):
- The envelope MUST conform to `.claude/skills/dual-tpr/findings-schema.json`
- The `schema_version` field is REQUIRED and MUST be exactly the
  string `"1.0"`. **Forgetting this field is the single most common
  envelope bug** — emit it as the first key of the object so you cannot
  skip it. The parser rejects the envelope with `schema_violation:
  'schema_version' is a required property` if it is missing.
- The `status` field MUST be `"complete"` if you finished the review
  successfully; use `"failed_partial"` only if you were unable to
  complete the investigation for a stated reason
- The `reviewer` field MUST be `"gemini"`
- The `skill` field MUST be `"review-plan"`
- The `scope_actually_reviewed` field is REQUIRED — it is an object
  (not a string) containing the scope you actually reviewed
- The `scope_actually_reviewed.expanded_beyond_packet` field is
  REQUIRED — set it to `true` if you investigated beyond the starting
  packet the wrapper gave you, with a one-sentence `expansion_reason`
- The `findings` field is REQUIRED and MUST be an array (possibly
  empty `[]` for a clean review). **`findings` is NEVER a boolean or
  a string — always an array.**
- The `no_findings` field is REQUIRED and MUST be a boolean: `true`
  if the `findings` array is empty, `false` if it contains any findings.
  This is a redundant signal so the parser can distinguish
  "envelope intentionally clean" from "envelope accidentally empty".
- Each finding's `basis` field MUST be one of `fresh_verification |
  direct_file_inspection | git_history | inference`
- Each finding's `location` MUST match the canonical regex
  `^[a-zA-Z0-9_./-]+:[0-9]+$` — but for review-plan the path is a
  plan file (e.g., `plans/dual-tpr-gemini/section-02-transport.md:45`)
  not a source file
- Each finding's `title` MUST be imperative voice, sentence case, no
  markdown, no trailing punctuation, ≤200 chars

**Finding semantics for plan-review (additional to the shared contract):**
- Each finding in envelope-only mode describes a PROPOSED edit to
  the plan
- The `title` describes the proposed edit in imperative form
  (e.g., "Add worktree guard description to Section 02 success criteria")
- The `evidence` cites the current plan content that is inaccurate
  or missing
- The `impact` explains why the plan is incomplete or wrong without
  the edit
- The `required_plan_update` contains the proposed replacement text
  or addition

**Note on apply-ability:** the `required_plan_update` field is
free-text prose describing the proposed change, not a structured
patch. The consumer that invokes this skill (`/tp-help` in the
dual-source flow, or a human reviewer running `codex exec
/review-plan` standalone) interprets and applies each edit after
user approval — Claude is the single writer to plan files, not the
reviewers. If finer-grained apply semantics are needed, a future
revision can extend the schema with a structured `patch` field; the
current envelope treats edit application as consumer-mediated, not
reviewer-deterministic.

See `.claude/skills/dual-tpr/envelope-format.md` for the full contract.

## What you must NOT do

- DO NOT edit plan files directly (that's the codex-side plan-write
  mode, not this skill)
- DO NOT change the `reviewed` frontmatter values of any section
  during whole-plan review
- DO NOT emit multiple envelopes
- DO NOT skip sentinels
