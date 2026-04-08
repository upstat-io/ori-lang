---
name: review-work
description: Review actual implementation work (bug fixes, features, refactors, multi-file changes) and emit a JSON findings envelope. Use this when the user asks for a third-party review of work done across committed history, staged changes, unstaged changes, or a plan section. Does NOT modify any files — emits envelope only.
---

# Review Work (gemini side)

This skill implements the review-work workflow for Gemini as part of
the dual-source TPR system. It always runs in envelope-only mode —
it does NOT write findings to plan files or source files, only emits
a JSON envelope conforming to `.claude/skills/dual-tpr/findings-schema.json`.

## Step 0: Execution Mode (MANDATORY — read first)

This skill has ONE execution mode: **envelope-only**. Unlike the
codex-side review-work skill (which has a `plan-write` vs `envelope-only`
Step 0 dispatch), this gemini skill has no plan-write branch because
there is no standalone "write to plan files" use case for the gemini
side — the gemini skill exists solely for the dual-source wrapper.

This is a REAL execution branch, not a soft override:
1. You MUST emit a JSON envelope conforming to the schema at the end
   of your response
2. You MUST NOT modify any files (source, plan, bug-tracker, anything)
3. You MUST NOT write to any location on disk other than your own
   output stream
4. Every code path that would modify a file is suppressed by this Step 0

If a later instruction in this file appears to contradict Step 0,
Step 0 wins. Envelope-only is non-negotiable.

## Methodology

Follow the shared reviewer-agnostic methodology documented in
`.claude/skills/dual-tpr/command-file.md` for:
- Scope resolution
- Evidence gathering
- Deep investigation standard
- Mandatory standards checks
- Finding format
- Verification basis categories
- Review boundaries

The shared command file is the single source of truth for HOW to
review. This file adds gemini-specific instructions on top of that
methodology.

## Grounding directive (gemini-specific)

You have access to `google_web_search`. USE IT proactively for any
finding that makes a claim about:
- External libraries (Rust crates, Python packages, Node modules, etc.)
- Language specifications (Rust reference, Python PEPs, TC39, etc.)
- Compiler internals of other projects (rustc, swift, lean4, koka)
- Prior art comparisons ("how does X handle this?")
- Recent developments (changes since your training cutoff)
- Security best practices
- Performance claims that require citation

When you use `google_web_search`, cite the source URL in the finding's
`citations` array. Each citation is an object:
```json
{
  "url": "https://doc.rust-lang.org/std/sync/atomic/",
  "description": "Rust atomic ordering reference"
}
```

Grounded findings are strictly more valuable than ungrounded ones —
they can be independently verified by the reader. Prefer grounded
analysis over confident assertion for external claims.

## Envelope output requirement

Your response MUST end with a JSON envelope bracketed by sentinels.
The format is:

    (free-form prose about what you investigated and why)

    <!-- BEGIN-ORI-DUAL-TPR-V1 -->
    ```json
    { ...complete envelope per findings-schema.json... }
    ```
    <!-- END-ORI-DUAL-TPR-V1 -->

Critical envelope contract points:
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
- The `skill` field MUST be `"review-work"`
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
  `^[a-zA-Z0-9_./-]+:[0-9]+$` (repo-relative path:line)
- Each finding's `title` MUST be imperative voice, sentence case, no
  markdown, no trailing punctuation, ≤200 chars

See `.claude/skills/dual-tpr/envelope-format.md` for the full contract
including positive and negative examples.

## What you must NOT do

- DO NOT modify any files (source, plan, bug-tracker, anything)
- DO NOT attempt to edit plan sections directly
- DO NOT emit multiple envelopes — only ONE at the end of your response
- DO NOT skip the sentinels even if you think the JSON block is
  unambiguous without them
- DO NOT use fresh_verification basis for findings you did not actually
  verify by running tests or scripts — use direct_file_inspection or
  inference instead
