---
name: tpr-review
description: "Run a third-party review via Codex CLI — TRIGGER proactively after completing ANY non-trivial work: bug fixes, new features, refactors, multi-file changes, compiler changes, codegen changes, test additions, plan implementations, or anything touching correctness-sensitive code. When in doubt, run it. The cost of an unnecessary review is near zero; the cost of a missed bug is high."
allowed-tools: Bash, Read, Edit, Write, Grep, Glob
---

# TPR Review via Codex

Run the Codex CLI non-interactively to perform an independent review-work pass. Codex has its own context, rules, and skills — it will figure out scope on its own.

## When to Trigger — Bias Toward Running

**Run this skill after completing ANY of the following:**
- Bug fixes (any severity)
- New features or feature extensions
- Refactors or code reorganization
- Multi-file changes (2+ files)
- Any change to compiler crates (`ori_types`, `ori_eval`, `ori_llvm`, `ori_arc`, `ori_parse`, `ori_lexer`, `ori_rt`)
- Any change to codegen, type checking, or evaluation
- Any change to the ARC/AIMS pipeline
- Test matrix additions or test infrastructure changes
- Plan section implementations
- Stdlib or registry changes
- Changes to error handling or diagnostics

**Also run when:**
- You're unsure whether the change warrants review (default: run it)
- The work involved multiple steps or non-obvious decisions
- The change touches code paths shared across subsystems
- You fixed something that was interfering with other code

**The only time NOT to run:** purely cosmetic single-line changes (typo fixes, comment edits, formatting-only).

## Steps

### 1. Run Codex

```bash
codex exec "run the /review-work skill" --full-auto --json
```

### 2. Parse Output

Read the JSONL output. Extract `agent_message` items (type: `item.completed`, item.type: `agent_message`) — the last few messages contain the findings.

### 3. Present Summary

Summarize findings to the user with severity, file references, and reviewer consensus.

### 4. File Findings

For each validated finding from the Codex review:

1. **Check if an owning plan section exists** — is there an active plan (roadmap or reroute) with a section covering the affected code?
2. **If yes** — record as a TPR finding in that section's `Third Party Review Findings` block using standard TPR format:
   ```md
   - [ ] `[TPR-{section}-{ordinal}][{severity}]` `file:line` — Finding summary.
     Evidence: {from Codex output}
     Impact: {from Codex output}
   ```
   Update plan metadata (`third_party_review.status: findings`, `updated: {today}`).

3. **If no owning plan exists** — file as a bug in `plans/bug-tracker/` under the appropriate subsystem section:
   ```md
   - [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by tpr-review.
     Repro: {from Codex output}
     Subsystem: {crate/file path}
     Found: {YYYY-MM-DD} | Source: tpr-review
   ```

   Subsystem mapping:
   - `ori_parse`/`ori_lexer` → section-01
   - `ori_types` → section-02
   - `ori_eval`/`ori_patterns` → section-03
   - `ori_llvm`/`ori_arc` → section-04
   - `ori_rt` → section-05
   - `library/std`/`ori_registry` → section-06
   - `oric`/`ori_fmt`/`ori_diagnostic` → section-07
   - `docs/`/`.claude/`/`plans/` → section-08

### 5. Report

Tell the user:
- How many findings were surfaced
- Where each was filed (plan TPR section or bug-tracker)
- Any that couldn't be classified (present for manual decision)
