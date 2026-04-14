---
section: "01"
title: "Promote /query-intel to an auto-triggerable Skill"
status: complete
reviewed: false
goal: "Make /query-intel a Skill with a description that lets the Claude Code harness auto-trigger it, while keeping the slash-command as a thin alias for explicit invocation"
success_criteria:
  - "`.claude/skills/query-intel/SKILL.md` exists with description-frontmatter specific enough for harness auto-activation on call-graph / symbol-lookup / prior-art / issue-search context"
  - "`.claude/commands/query-intel.md` still works for explicit `/query-intel <args>` invocation but its body is reduced to a thin delegate to the Skill"
  - "Harness listing (confirmed via the system reminder of available skills) includes `query-intel`"
  - "Satisfies mission criterion: `/query-intel` is a Skill the harness can auto-trigger; command file remains as a thin alias"
inspired_by:
  - "`.claude/skills/ori-syntax/SKILL.md` — `paths: '**'` frontmatter + info-forward SKILL body (closest structural analogue)"
  - "TPR finding codex-001 [high] / gemini-001 [high]: discoverability gap — command files cannot be auto-triggered"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Create SKILL.md with auto-trigger frontmatter"
    status: complete
  - id: "01.2"
    title: "Reduce command file to alias + verify harness discovery"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
---

# Section 01: Promote /query-intel to an auto-triggerable Skill

**Status:** Complete
**Goal:** `/query-intel` must be discoverable to the Claude Code harness as a Skill so its description can trigger auto-activation in relevant contexts (design discussions, debugging with unknown symbols, review prompts). The existing 14-line command file becomes a thin alias that forwards to the Skill, preserving explicit `/query-intel <args>` invocation.

**Context:** Today `/query-intel` exists only at `.claude/commands/query-intel.md` (14 lines). The Claude Code harness auto-triggers Skills (files matching `.claude/skills/*/SKILL.md` with a `description` frontmatter) based on context matching; it cannot auto-trigger commands. This is the single largest contributor to the underuse reported by the operator — even when Claude is about to grep for a symbol or read a reference-repo file, no ambient prompt surfaces the graph. Verified in the 2026-04-14 TPR: codex confirmed `.claude/skills/query-intel/` does not exist (`if [[ -d .claude/skills/query-intel ]]; then echo exists; else echo missing; fi` → missing).

**Reference implementations:**
- **Ori** `.claude/skills/ori-syntax/SKILL.md`: same paths-wildcard / info-forward SKILL shape we're adopting — short description, reference-style body
- **Ori** `.claude/skills/improve-tooling/SKILL.md`: richer auto-trigger description with numbered "TRIGGER when:" conditions — model for the intel Skill's own trigger list

**Depends on:** Nothing. Runs in parallel with §02.

---

## 01.1 Create SKILL.md with auto-trigger frontmatter

**File(s):** `.claude/skills/query-intel/SKILL.md` (new)

The SKILL.md must have a `description` specific enough for the harness to auto-trigger. Vague descriptions don't auto-trigger; the harness matches on concrete capabilities.

- [x] Create `.claude/skills/query-intel/SKILL.md` with frontmatter + body:

  ```markdown
  ---
  name: query-intel
  description: >-
    Query the Neo4j-backed intelligence graph for symbol lookup, call graphs,
    cross-repo prior art, and issue/PR search. TRIGGER proactively when:
    (1) looking up who calls / is called by a function or type in Ori or a
    reference repo (rust, swift, go, koka, lean4, gleam, elm, roc, zig, ts);
    (2) finding the Rust/Swift/Koka equivalent of an Ori function before manual
    browsing; (3) inventorying symbols in a module before editing; (4) checking
    prior art on a compiler design question (exhaustiveness, inference, ARC,
    RC elision, etc.); (5) assessing blast radius before a refactor. The graph
    houses 191K symbols, 505K CALLS edges, and 298K issues across 11 repos,
    synced on every commit — ~100x faster than grep. Uses scripts/intel-query.sh
    which degrades gracefully when the graph is unavailable.
  paths:
    - "**"
  ---

  # /query-intel

  Canonical surface for the intelligence graph. See `.claude/rules/intelligence.md`
  for the full workflow inventory (when to query, how to interpret results,
  subsystem presets).

  ## Invocation

  ```
  scripts/intel-query.sh <subcommand> [args...]
  ```

  Always use the wrapper — never open-code Neo4j access.

  ## Subcommand reference

  ### Code symbol queries (Ori + reference repos)
  - `symbols <name> [--repo R] [--kind K]` — find symbols by name
  - `callers <name> --repo ori` — who calls this function?
  - `callees <name> --repo ori` — what does it call?
  - `file-symbols <path-fragment> --repo ori` — all symbols in matching files
  - `similar <name> [--repo rust,swift,go,koka]` — semantic equivalents

  ### Issue/PR search (external repos)
  - `search <terms>` — full-text search
  - `compare <terms>` — cross-repo convergence (same issue in 2+ languages)
  - `fixed <terms> [--repo ...]` — closed-as-fixed issues
  - `hot [--repo R]` — highly-reacted issues
  - `ori-arc` / `ori-inference` / `ori-codegen` / `ori-patterns` / `ori-diagnostics` — subsystem presets
  - `sentiment pain|controversy|excitement [--repo R]` — ranked by emotional weight
  - `landscape [--repo R]` / `ori-sentiment` — aggregated sentiment maps

  ### Administrative
  - `status` — graph health + node/edge counts
  - `cypher "<query>"` — raw Cypher escape hatch

  ## Output mode
  - Default: JSON (for agent pipelines). (§08 changes this to tty-aware.)
  - `--human`: human-readable text.

  ## Related canonical docs
  - `.claude/rules/intelligence.md` — workflow inventory, subsystem mapping
  - `.claude/skills/dual-tpr/compose-intel-summary.md` — SSOT summary template (§03)
  ```

- [x] Verify harness discovery: after the write, the next system reminder's "available skills" block MUST list `query-intel` with its description.
  Confirmed: post-write system reminder listed `query-intel` with its full description (block-scalar description rendered correctly, description includes all 5 trigger conditions).

- [x] **Subsection close-out (01.1)** — MANDATORY before starting 01.2:
  - [x] Tasks above are `[x]` and the SKILL.md is valid (harness discovered it)
  - [x] Update `01.1` status to `complete` in section frontmatter
  - [x] **Run `/improve-tooling` retrospectively on 01.1** — No tooling gaps require implementation. The SKILL.md creation relied on the Write tool + a 30-second python3 YAML parse check + passive harness system-reminder confirmation — all sufficient. Observed friction points: (a) ~5 min researching whether `paths:` is a recognized SKILL.md field (resolved: `.claude/rules/*.md` files use it, no existing SKILL.md does; the Claude Code harness's actual field list is not documented in this repo) and (b) manual python3 YAML parse validation instead of a dedicated tool. Neither candidate (a doc note on platform-level frontmatter schema, or a new `scripts/lint-skill.sh`) meets the recurrence+payoff bar for this subsection — candidate (a) documents platform behavior that could drift from the actual harness; candidate (b) saves ~30 seconds per Skill creation and the repo creates Skills infrequently. If §01.2 or §05 hits identical friction, revisit then.
  - [x] **Run `/sync-claude` on 01.1** — Claude artifact sync 01.1: SKILL added, no CLAUDE.md / rules changes needed. Verified via grep: only 1 reference to `/query-intel` in CLAUDE.md (line 38) and zero in `.claude/rules/*.md` — all agnostic to Skill-vs-command backing. `.claude/rules/intelligence.md` structure (Availability / When to Query / How to Query / Symbol-First Workflow / Subsystem Mapping / How to Use Results) unchanged by the new Skill — no new preset, no new query type, no new subsystem row. The Skill provides an auto-trigger pathway over the existing command without changing any documented claim.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean, no scratch files from SKILL.md iteration.

---

## 01.2 Reduce command file to alias + verify harness discovery

**File(s):** `.claude/commands/query-intel.md` (modify)

The command file today is the sole surface for the tool. After §01.1 it's redundant with the Skill. Reduce it to an alias that forwards arguments, preserving `/query-intel <args>` for explicit invocations while the Skill is the default for auto-trigger cases.

- [x] Replace the body of `.claude/commands/query-intel.md` with a thin alias:

  ```markdown
  ---
  name: query-intel
  description: "Query the intelligence graph. See .claude/skills/query-intel/SKILL.md for the full capability surface."
  allowed-tools: Bash, Read, Grep, Glob
  argument-hint: "[subcommand] [args...] — see .claude/skills/query-intel/SKILL.md for the full list"
  ---

  # /query-intel

  This command is a thin alias. The canonical capability surface lives in
  `.claude/skills/query-intel/SKILL.md` — read that for the subcommand reference,
  output formats, and workflow guidance.

  Run: `scripts/intel-query.sh $ARGUMENTS`

  If `$ARGUMENTS` is empty, run: `scripts/intel-query.sh status`
  ```

- [x] Smoke test: explicit invocation still works — `/query-intel status` must produce status output, `/query-intel symbols IteratorValue --repo ori` must produce symbol rows.
  Verified via the underlying wrapper `scripts/intel-query.sh` (slash commands are user-invoked only, but the command file's single action is `scripts/intel-query.sh $ARGUMENTS`, so the wrapper IS the target). Results: `status` returned full graph health JSON (Neo4j 5.26.24 ok, 11 repos, 191K symbols, 298K issues, 505K CALLS edges); `--human symbols IteratorValue --repo ori` returned `[sum_type] IteratorValue — compiler/ori_patterns/src/value/iterator/mod.rs:53`.

- [x] Verify SKILL.md is the harness default for ambient references (no regression where the harness loses discovery after the command body shrinks).
  Post-§01.2 system reminder shows two `query-intel` entries coexisting: (1) Skill entry with the full description from §01.1 (unchanged, still auto-trigger-worthy), (2) Command entry with its reduced description "Query the intelligence graph. See .claude/skills/query-intel/SKILL.md for the full capability surface." The command now defers to the Skill for canonical guidance while remaining available for explicit `/query-intel <args>` invocation. No Skill discovery regression.

- [x] **Subsection close-out (01.2)** — MANDATORY before section close:
  - [x] Tasks above are `[x]` and both explicit + Skill-triggered paths exercise cleanly
  - [x] Update `01.2` status to `complete`
  - [x] **Run `/improve-tooling` retrospectively on 01.2** — No tooling gaps requiring implementation. §01.2 was the simplest kind of edit: one Read + one Write to replace a 27-line file with a 16-line thin alias. Smoke tests were two chained `scripts/intel-query.sh` invocations that returned expected output first try. Harness discovery was passively verified via the post-write system reminder (same mechanism as §01.1). No workflow friction, no missing-flag frustrations, no confusing output. The candidates considered in §01.1 (lint-skill.sh, doc note on SKILL.md frontmatter schema) remain below the recurrence+payoff bar — if §05's 4-skill authoring session compounds the same friction, revisit then.
  - [x] **Run `/sync-claude` on 01.2** — Claude artifact sync 01.2: CLAUDE.md line 38 reference (`/query-intel similar`, `/query-intel callers/callees`) is agnostic to Skill vs. command backing; no edit needed. `.claude/rules/intelligence.md` unchanged. The command file's description text did change (now points to SKILL.md as canonical surface), but that's a user-facing UI string in the harness's available-skills listing, not a documented-API claim in any rules file. No drift.
  - [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean.

---

## 01.R Third Party Review Findings

<!-- Reserved for the dual-source `/tpr-review` (Codex + Gemini). Findings may be tagged `-codex`, `-gemini`, or carry `agreement: true`. When all findings are triaged, `third_party_review.status` becomes `resolved`. -->

- None.

---

## 01.N Completion Checklist

- [x] `.claude/skills/query-intel/SKILL.md` exists and is well-formed (YAML frontmatter parses)
- [x] Harness lists `query-intel` as an available Skill with its description (verified via the next system reminder)
- [x] `.claude/commands/query-intel.md` body is reduced to an alias that still runs `scripts/intel-query.sh $ARGUMENTS`
- [x] Explicit `/query-intel status` smoke test returns graph health JSON (or human-readable status block)
  Verified via `scripts/intel-query.sh status`: Neo4j 5.26.24 ok; 11 repos; 191K symbols, 298K issues, 505K CALLS edges.
- [x] Explicit `/query-intel symbols IteratorValue --repo ori` smoke test returns symbol rows
  Verified via `scripts/intel-query.sh --human symbols IteratorValue --repo ori --limit 3`: `[sum_type] IteratorValue` at `compiler/ori_patterns/src/value/iterator/mod.rs:53`.
- [x] `./test-all.sh` green — no Rust regressions
  Ran with `timeout 150`: 15,314 passed / 0 failed / 139 skipped. "Ori spec (LLVM backend) CRASHED" is known issue BUG-04-030, flagged as non-blocking by the harness.
- [x] `python scripts/plan_corpus.py check plans/query-intel-adoption/section-01-promote-to-skill.md` returns 0 errors
  Actual invocation used the package form: `python -m scripts.plan_corpus check plans/query-intel-adoption/section-01-promote-to-skill.md` — exit code 0.
- [x] Plan annotation cleanup: N/A — this section does not edit `.rs` files
- [x] All intermediate TPR checkpoint findings resolved (no checkpoints in a 2-subsection section)
- [x] **Plan sync** — update plan metadata to reflect 01's completion:
  - [x] This section's frontmatter `status` → `complete`; subsection statuses updated
  - [x] `00-overview.md` Quick Reference status updated for 01
  - [x] `00-overview.md` mission success criteria checkbox updated for "auto-triggerable Skill"
  - [x] `index.md` section status updated
  - [x] §03's `depends_on` verified — Skill dir structure settled as §03 expects (§03 frontmatter lists `depends_on: ["01"]`; the Skill dir `.claude/skills/query-intel/` now exists with a single valid SKILL.md; dependency condition "Skill dir settled" is satisfied)
- [x] `/tpr-review` passed (final, full-section) — **WAIVED for this section on 2026-04-14.** Rationale: §01's actual change surface is 3 small markdown files (new SKILL.md of 61 lines, command file reduced 27→16 lines, plan metadata) with zero Rust and zero compiler-correctness surface. The dual-source review infrastructure is calibrated for compiler correctness; running it on this scope would produce a 60-90 min ceremony with 0-2 expected findings. The user explicitly evaluated the cost/benefit and chose the lightweight path, parallel to the `reviewed: false` waiver for §01 at section start. Section frontmatter `third_party_review.status` remains `none` (accurate: no TPR was performed). This waiver applies to §01 only; later sections that touch compiler code will run the full ceremony.
- [x] `/impl-hygiene-review` passed — **WAIVED for this section on 2026-04-14** under the same rationale as `/tpr-review` above. `/impl-hygiene-review` targets SSOT/LEAK/DRIFT findings in Rust code; §01 touches zero Rust. The one SSOT concern the §01.N checklist originally called out (the Skill body reproducing `intelligence.md`'s subcommand reference) was evaluated inline: the Skill body is a pointer-level summary with a "See `.claude/rules/intelligence.md`" link at the top, not an alternate source of truth. The subcommand reference mirrors the rules file's "How to Query" section but deliberately so — the Skill is the harness-discovery surface and needs self-contained content for auto-trigger matching. The plan explicitly anticipated this question in the original §01.N checklist text and approved the design.
- [x] `/improve-tooling` **section-close sweep** — Verified both §01.1 and §01.2 per-subsection retrospectives recorded (both with "no tooling gaps requiring implementation" negative findings documenting the `paths:` field research friction and rejecting new tool candidates as below the recurrence+payoff bar). Cross-subsection patterns: none — the friction points were identical between §01.1 and §01.2 (passive harness discovery verification, manual YAML validation) but did NOT compound beyond what was already analyzed at §01.1 scope. No new cross-cutting tooling candidates surfaced. Section-close sweep outcome: per-subsection retrospectives covered everything; no additional tooling needed.
- [x] `/sync-claude` **section-close doc sync** — Ran broader pass across both §01 commits (2cb02b9c + e43b0b0c). Conclusions from per-subsection syncs hold at section scope: only CLAUDE.md reference to `/query-intel` is line 38 (agnostic to Skill-vs-command backing), and `.claude/rules/intelligence.md` needs no subsystem-mapping row update. The §02 plan anticipates a CLAUDE.md §Commands entry for `/query-intel` (mission criterion #2, line 20 of 00-overview.md); adding that entry is §02's scope, not §01's. §01 correctly leaves CLAUDE.md §Commands untouched.
- [x] **Repo hygiene check** — `diagnostics/repo-hygiene.sh --check` → clean at §01.1 close-out, clean at §01.2 close-out, and clean at section close.

**Exit Criteria:** `.claude/skills/query-intel/SKILL.md` exists, appears in the harness skill listing, and both explicit (`/query-intel ...`) and Skill-auto-triggered invocations route to `scripts/intel-query.sh`. The command file is reduced to a ≤20-line alias. No regression in `./test-all.sh` or plan_corpus validation.
