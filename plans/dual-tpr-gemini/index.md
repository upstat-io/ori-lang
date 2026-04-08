# Dual-Source TPR with Grounded Gemini Reviewer Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Contracts + foundation
**File:** `section-01-contracts-foundation.md` | **Status:** Complete (gates deferred)

```
findings-schema.json, envelope schema, JSON schema, SSOT, single source of truth
ORI-DUAL-TPR-V1, sentinel, BEGIN sentinel, END sentinel, fenced JSON block
canonical location format, path:line, repo-relative path, location regex
title format, imperative sentence case, no trailing punctuation, no markdown
reviewer-tag ID format, [TPR-NN-NNN-codex], [TPR-NN-NNN-gemini]
reviewer ordinal, independent ordinal sequences, no shared ordinal space
mktemp -d, per-run scratch directory, /tmp race condition, fixed path bug
.claude/hooks/block-banned-commands.sh, hook update, gemini timeout gate
codex timeout window, 300000ms, 2100000ms, 5 minutes, 35 minutes
hook timeout-window extension, codex+gemini gate, hook hygiene
PreToolUse hook, .claude/settings.json, hook registration, deny list
.claude/skills/dual-tpr/findings-schema.json
.claude/skills/dual-tpr/envelope-format.md
schema_version, status field, complete vs failed, scope_actually_reviewed
scope expansion flag, expansion_reason, independence contract
required_plan_update, basis field, fresh_verification, direct_file_inspection
git_history, inference, citations array, layer field, confidence field
```

---

### Section 02: Shared transport utility
**File:** `section-02-transport.md` | **Status:** Complete (gates deferred)

```
dual_invoke, parallel launcher, run_in_background, bash background invocation
wait_both, completion notification, foreground wait
worktree_guard, dirty worktree guard, git status --porcelain
before snapshot, after snapshot, diff detection, prompt-discipline violation
parse_codex, item.completed, agent_message, agent_message text
codex JSONL, --output-schema, schema-conformant JSON, direct extraction
parse_gemini, --output-format stream-json, stream-json events
{"type":"message","role":"assistant","content":"...","delta":true}
{"type":"result","status":"success"}, terminal success event
delta concatenation, assistant message fragments, fragment ordering
sentinel extraction, BEGIN-ORI-DUAL-TPR-V1, END-ORI-DUAL-TPR-V1
fenced JSON block, between sentinels, post-extraction parse
envelope extractor, two extractors, one envelope model, FindingsEnvelope
schema validator, asymmetric rigor, post-extraction validation
retry_infra, infra retry, exponential backoff, 1s 2s 4s, 3 retries max
infra failure budget, separate from semantic iteration, orthogonal budgets
merge_findings, reviewer tag suffix, -codex, -gemini
strict matching, exact (location, title) match, no fuzzy match
agreement detection, disagreement surfacing
failure taxonomy, launch fail, timeout, parse fail, schema violation
malformed envelope, missing terminator, multiple envelopes
parser unit tests, fixture files, real codex output, real gemini output
fault injection test, truncated JSONL, transient failure simulation
.claude/skills/dual-tpr/transport.md
.claude/skills/dual-tpr/transport-tests.md
test fixtures, .claude/skills/dual-tpr/fixtures/
canary release pattern, validation case, first consumer
```

---

### Section 03: Reviewer surface preparation
**File:** `section-03-reviewer-surface.md` | **Status:** Complete (gates deferred)

```
shared command file, reviewer-agnostic methodology, extracted methodology
plan-write mode, envelope-only mode, mode switch
execution branch, top-level dispatch, .codex/skills mode dispatch
.codex/skills/review-work/SKILL.md, codex skill mode addition
.codex/skills/review-plan/SKILL.md, parallel mode addition
soft prompt override anti-pattern, real execution branch
.gemini/skills/review-work/SKILL.md, new gemini skill creation
.gemini/skills/review-plan/SKILL.md, parallel gemini skill
gemini skill auto-discovery, .gemini/skills/<name>/SKILL.md
workspace skills, gemini skills list, repo-root invocation
google_web_search, grounding directive, external claim verification
gemini-specific instructions, reviewer-specific home
explicit skill activation, "Activate the review-work skill and..."
prompt activation convention, skill description vs activation
codex --full-auto, gemini --approval-mode yolo
codex --output-schema findings-schema.json
codex --json --ephemeral
gemini --output-format stream-json
.claude/skills/dual-tpr/command-file.md
standalone codex exec regression test, .codex/skills regression
```

---

### Section 04: /tpr-review dual-source (validation case)
**File:** `section-04-tpr-review.md` | **Status:** In Progress (pending gates: test-all.sh, /impl-hygiene-review, /improve-tooling section-close sweep)

```
.claude/skills/tpr-review/SKILL.md, dual-source rewrite
tpr-review loop protocol, 10 semantic iterations, max iterations
both reviewers per round, both must be clean termination
agreement detection, (location, title) exact match
disagreement surfacing, no auto-resolution, both findings tagged
validation case, first consumer of transport utility
critical path gate, gate before sections 05/06/07
real TPR scenario, plan section TPR block, ## NN.R Third Party Review Findings
[TPR-NN-NNN-codex], [TPR-NN-NNN-gemini], reviewer-tagged finding IDs
.claude/skills/dual-tpr/transport.md (consume)
fault injection at validation, infra retry exercise
canary release, single consumer validates infrastructure
```

---

### Section 05: /review-work dual-source + Task #10 fix
**File:** `section-05-review-work.md` | **Status:** Not Started

```
.claude/skills/review-work/SKILL.md, dual-source rewrite
NEVER/ALWAYS background contradiction, lines 78-80, lines 117-145
Task #10, self-contradicting directives
"ABSOLUTE: NEVER Background", "Always use run_in_background: true"
internal consistency check, no contradictory directives
loop protocol, review-work loop, fix and re-run
same pattern as tpr-review (Section 04)
```

---

### Section 06: /review-plan new Claude skill (parallel to existing command file)
**File:** `section-06-review-plan.md` | **Status:** Not Started

```
.claude/skills/review-plan/SKILL.md, NEW file, greenfield
parallel workflows, command file untouched, .claude/commands/review-plan.md unchanged
.claude/commands/review-plan.md (595 lines, 4-agent Claude pipeline, NOT modified)
plan-review semantics, edits plan files directly
preserve reviewed frontmatter, do not flip during whole-plan review
review-plan dual-source skill, parallel to existing command-file workflow
.codex/skills/review-plan/SKILL.md (existing standalone, regression-tested)
intentional duality, fast Claude pipeline vs deep dual-source codex+gemini
```

---

### Section 07: /tp-help dual-source + consolidation
**File:** `section-07-tp-help.md` | **Status:** Not Started

```
.claude/skills/tp-help/SKILL.md, dual-source rewrite
.claude/commands/tp-help.md, consolidation, SSOT fix
two sources of truth, R10 from Step 6B, divergent files
concatenation mode, raw responses, no synthesis layer
option A from architecture, raw perspectives not smoothed merge
lighter envelope, NOT findings schema, special case for tp-help
/impl-hygiene-review Phase 4 cross-check verification, downstream consumer
indirect benefit, dual-source impl-hygiene-review
.claude/skills/impl-hygiene-review/SKILL.md (verify, not modify)
```

---

### Section 08: Integration tests + runtime toggle + cleanup
**File:** `section-08-integration-cleanup.md` | **Status:** Not Started

```
end-to-end integration tests, real repo, four skills
ORI_TPR_REVIEWERS env var, codex|gemini|both, default both
runtime toggle, operational escape hatch, codex-only fallback
.claude/skills/create-plan/SKILL.md, line 56 update, sequencing wording
CLAUDE.md, line 141 update, REVIEW/AGENT TIMEOUTS, gemini mention
plan annotation cleanup, strip TPR-XX-YYY references, ephemeral scaffolding
documentation pass, README updates
regression test, standalone codex exec, plan-write mode preserved
final integration check, all 4 skills end-to-end
ORI_TPR_REVIEWERS=codex, ORI_TPR_REVIEWERS=gemini, ORI_TPR_REVIEWERS=both
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Contracts + foundation | `section-01-contracts-foundation.md` |
| 02 | Shared transport utility | `section-02-transport.md` |
| 03 | Reviewer surface preparation | `section-03-reviewer-surface.md` |
| 04 | /tpr-review dual-source (validation case) | `section-04-tpr-review.md` |
| 05 | /review-work dual-source + Task #10 fix | `section-05-review-work.md` |
| 06 | /review-plan new Claude wrapper | `section-06-review-plan.md` |
| 07 | /tp-help dual-source + consolidation | `section-07-tp-help.md` |
| 08 | Integration tests + runtime toggle + cleanup | `section-08-integration-cleanup.md` |
