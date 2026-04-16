---
name: roadmap-work
description: Execute a roadmap subsection on Opus. Invoked by /continue-roadmap (Sonnet) at its Step 6 handoff after the user confirms which subsection to work on. Reads code on Opus, writes code on Opus, invokes /fix-bug / /tpr-review / /impl-hygiene-review as nested skills. Not invoked directly by the user — always chained from /continue-roadmap.
argument-hint: "<plan-path>/<section-file> <subsection-id>"
model: opus
---

# Roadmap Work — Execute a Subsection on Opus

**Invoked by `/continue-roadmap` at its Step 6 handoff.** The parent Sonnet skill has already:

1. Re-read CLAUDE.md.
2. Scanned the roadmap, run all gates (schema, stale-frontmatter, unreviewed-plan, TPR triage, bug tracker, clean working tree).
3. Resolved blockers and impediments.
4. Presented the focus section summary to the user.
5. Got the user's pacing choice (full-section / subsection-by-subsection).
6. Identified the specific subsection to execute.

Your job is the code-execution body that used to live in `/continue-roadmap` Step 6 — but running on Opus so the code reads and code writes benefit from Opus's judgment on Ori compiler invariants (ARC soundness, phase purity, type-system rules, spec conformance).

## Rule of model usage

**Opus for:**
- Reading affected source files before editing
- Writing code (Rust compiler code, Ori stdlib, Ori tests, Rust tests)
- Triage decisions surfaced by `/tpr-review` (nested — inherits its own Opus triage phase)
- Root-cause analysis in `/fix-bug` (nested)
- `/impl-hygiene-review` findings interpretation (nested)

**Sonnet for (delegate via `Agent(model: "sonnet")` subagents):**
- Updating plan checkbox flips (`- [ ]` → `- [x]`) at subsection close-out
- Frontmatter metadata updates (`updated:`, `status:`, `reviewed:`)
- Progress summaries / retrospective report text
- Running mechanical scripts (`./test-all.sh`, `diagnostics/*.sh`, `roadmap-scan.sh`)

**Shell, nested skill invocations, and `/commit-push`** inherit their own model policies — no Opus vs Sonnet choice needed here.

## Protocol

### Step 0: Re-read CLAUDE.md (MANDATORY even though /continue-roadmap just did it)

Context compression between skill invocations can drop rules. Read CLAUDE.md in full before executing.

### Step 1: Load subsection detail

Read the target section file at `<plan-path>/<section-file>` in full. Identify the specific `- [ ]` items under `<subsection-id>` (e.g., `§04.2 Phase B`, subsection `1.1A`, etc.).

### Step 2: Intelligence recon (CONDITIONAL — per `.claude/rules/intelligence.md`)

Follow the canonical intel-summary injection protocol:

@.claude/skills/query-intel/compose-intel-summary.md

Per SSOT Step F / `/continue-roadmap` extension — use `file-symbols`, `callers`/`callees`, `similar` on section-body symbols to map blast radius before editing.

### Step 3: Read affected source code (Opus-mandatory)

Read the code paths the subsection will touch **before** modifying anything. This read feeds directly into your edit decisions — per the user's empirical experience, Opus produces materially better Ori compiler code, and pre-edit reads are inseparable from the edit quality. Do not delegate this read to a Sonnet subagent; same-model-read-and-write is the correctness invariant.

### Step 4: Execute the subsection's checkboxes

Follow the **Implementation Guidelines** in `.claude/skills/continue-roadmap/SKILL.md` — specifically the sections after Step 6:

- ZERO DEFERRAL — Implement, Don't Document For Later
- ALL Deferrals Must Have Implementation Anchors
- Plan Boundary Integrity
- Scope Rule: ALL Checkboxes in the Section Are In Scope
- Verification Rule: Empty Checkboxes Must Be Verified
- Matrix Testing Rule (delegate to `.claude/rules/tests.md`)
- TDD for Bugs (delegate to CLAUDE.md §TDD for Bugs)

These guidelines are authored in `/continue-roadmap` and read here by reference rather than duplicated — the content is long and drift between two copies is an SSOT violation.

### Step 4.5: Invariant-Anchoring (MANDATORY before test-chasing)

Before writing or modifying ANY code in Step 4, answer in plain text at the top of your scratchpad:

1. **What system invariant does this subsection enforce?** (e.g., `typeck.md §PC-2`, `aims-rules.md` lattice dimensions, ARC RC balance, phase-purity contract, spec clause).
2. **Which downstream system consumes that invariant?** (e.g., AIMS analysis reads PC-2-clean typed IR; codegen requires all `Tag::Var` resolved; monomorphization relies on `scheme_var_ids`.)
3. **If tests fail, is the correct response to (a) fix the code under test so it satisfies the invariant, or (b) weaken the invariant so tests pass?** The answer is ALWAYS (a). If you catch yourself choosing (b), stop — that's the exact failure mode Step 4.5 exists to block.

This is the anchor against "narrow-scope goal drift": the tendency to forget the system invariant a subsection serves and optimize the local tests instead. CLAUDE.md §The One Rule is explicit — "When you see two possible fixes — one simpler and one more correct — the simpler one does not exist." A gate/flag/workaround that makes tests pass by neutering the deliverable is the simpler one; it does not exist.

For AIMS-adjacent work specifically: CLAUDE.md §AIMS invariants 1–5 apply. Any subsection that touches typeck → AIMS or AIMS → codegen handoff MUST preserve the through-line (every memory decision derives from proof; every RC op points at a specific proof failure). Subsections that surface PC-2 / lattice / contract violations by enforcing them are doing exactly what they should — do not rationalize those violations away.

### Step 5: Run tests

```
timeout 150 ./test-all.sh
```

### Step 5.5: Blocker Bug Protocol (MANDATORY when tests fail)

When Step 5 surfaces test failures, classify each failure before taking action:

**Classification A — Plan-anticipated Known Failing Test**: the failure matches a pattern explicitly listed in the section's "Known Failing Tests" block (or equivalent), and the plan already points to a concrete follow-up section/plan/item that will resolve it. Acceptable; record in the Known Failing Tests table per plan.

**Classification B — Blocker bug surfaced by THIS subsection's deliverable**: the failure is caused by a real PC-2 / lattice / contract / spec violation that the subsection's code correctly surfaces. The code IS doing its job; the failure IS the discovery. Response:

1. File the bug via `/add-bug` (if it's non-blocking to the current subsection) OR via `/fix-bug` (if blocking the subsection's success criteria — default assumption if the validator / check / enforcement the subsection adds is rendered inert on the failing path).
2. If `/fix-bug`: pause subsection work. Complete the bug fix (full plan-section rigor: root cause, TDD matrix, TPR, hygiene). Then resume the subsection.
3. Do NOT mark the subsection complete until the blocker bug is fixed AND the subsection's deliverable is actually active on the previously-failing path.

**Classification C — Regression caused by THIS subsection's code**: something the subsection should NOT have broken is now broken. Shelve the subsection, fix the regression, re-apply. (CLAUDE.md §Stabilization Discipline — Fix interference = reorder, don't skip.)

**BANNED responses to test failures** (explicitly — these are the "make tests pass by neutering deliverable" anti-patterns):

- BANNED: adding a feature-flag / gate / early-return that skips the subsection's validator, check, or enforcement on the failing path.
- BANNED: widening the exemption set of a validator/check to make failing tests pass (unless the exemption is architecturally correct per spec — and correctness is gated by a fresh reading of the spec, not by the tests passing).
- BANNED: moving the failing case to "Known Failing Tests" without a concrete anchor section/plan that will resolve it.
- BANNED: adding a test for the neutered behavior ("when the gate is active, validator correctly skips generic bodies") — this tests the workaround, not the invariant.
- BANNED: marking the subsection complete when the subsection's core deliverable is gated off on one or more code paths.

**Required response to a blocker bug**: choose the correct architectural fix (CLAUDE.md §The One Rule) regardless of scope/effort/cost/risk. If the fix crosses crate boundaries, that IS the work. If the fix requires inference-engine changes to eagerly resolve Vars, that IS the work. If the fix requires adding a `fresh_instance_var_ids` field to `DeferredMonoCall` to expose mono-deferred vars to downstream enforcement, that IS the work.

### Step 5.7: AskUserQuestion contract when stopping for triage

When Step 5 test failures require user triage (legitimately ambiguous choice — e.g., "fix inference vs extend IR metadata"), the AskUserQuestion options you present MUST obey these rules:

**Every option must be architecturally correct.** None of the options may neuter the subsection's deliverable or defer a blocker without a concrete fix anchor. If you find yourself about to write "Accept current state, file bugs, proceed" — STOP. That option is banned per Step 5.5 unless the bugs are Classification A (Known Failing Tests) AND the anchors are concrete.

**Valid option categories when stopping for triage**:

- VALID: "Fix blocker via /fix-bug NOW, resume subsection after" (always valid; usually recommended).
- VALID: "Fix blocker via a targeted approach {A}" vs "Fix blocker via a more general approach {B}" (valid when the architecturally-correct fix admits multiple valid shapes).
- VALID: "Revert subsection wiring and rewrite plan" (valid when the plan's assumptions are proven wrong).
- VALID: "Stop /roadmap-work and escalate plan-level" (valid when the subsection's scope exceeds what can be resolved inline).

**Invalid option categories** (never present these):

- BANNED: "Accept current test failures and proceed" (unless failures are Classification A).
- BANNED: "Gate/flag/skip the failing path and proceed" (this is Inverted TDD — the deliverable tests-green only because you disabled it).
- BANNED: "Mark as future improvement and proceed" (CLAUDE.md §'Future improvement' MUST be concretely tracked — banned without an anchor).
- BANNED: "File bugs and proceed" without a decision point for each filed bug ("which bugs are blockers → /fix-bug now" vs "which are non-blocking → /add-bug").

Prefix the "Recommended" label ONLY on options that choose the most correct fix per CLAUDE.md §The One Rule, not on the lowest-effort option.

### Step 6: Nested skill invocations (as needed)

- If a bug surfaces during Step 4, invoke `Skill: fix-bug BUG-XX-NNN` (inherits its own Opus judgment phases). Classification per Step 5.5 determines whether `/fix-bug` is invoked NOW (Classification B blocker) or later (Classification A / non-blocking). Do NOT invoke `/add-bug` on a Classification B blocker and proceed — that is banned deferral.
- After code changes that touch more than the subsection's narrow slice, invoke `Skill: tpr-review` (inherits its own dispatch/triage split — triage is Opus).
- At subsection close-out (per `/continue-roadmap` §Step 5.5), invoke `Skill: impl-hygiene-review` **after** TPR is clean.

### Step 7: Subsection close-out (per `/continue-roadmap` §Step 5.5 close-out sequence)

1. Verify all subsection tasks are `[x]` and behavior is verified.
2. Update subsection `status` in section frontmatter to `complete` — **delegate the frontmatter edit to an `Agent(model: "sonnet")` subagent** (mechanical-writing, Sonnet-safe).
3. Invoke `Skill: improve-tooling` retrospectively on THIS subsection.
4. Run `diagnostics/repo-hygiene.sh --check` (and `--clean` if needed).
5. Invoke `Skill: commit-push` for the subsection's implementation work.

### Step 8: Return control to `/continue-roadmap`

Your skill exits. The Sonnet parent resumes for the next subsection's pacing decision (full-section mode → next subsection; subsection-by-subsection mode → AskUserQuestion).

## What this skill does NOT do

- **Does not run the gates** (schema, stale-frontmatter, unreviewed-plan, TPR triage, bug tracker, clean working tree) — those are `/continue-roadmap` Steps 1–2 and already ran on Sonnet before this skill was invoked.
- **Does not decide which subsection to execute** — the parent skill picked it; this skill receives the ID as args.
- **Does not loop to the next subsection** — control returns to `/continue-roadmap` after close-out, which then decides whether to invoke `/roadmap-work` again (full-section mode) or prompt the user (subsection-by-subsection mode).

## Invocation contract

Called as:
```
Skill: roadmap-work <plan-path>/<section-file> <subsection-id>
```

- `<plan-path>/<section-file>`: e.g., `plans/roadmap/section-04-aims.md`
- `<subsection-id>`: e.g., `4.2`, `4.2B`, `Phase-A`

Optional third arg: freeform note from `/continue-roadmap` about what the user specifically asked for (e.g., "focus on LLVM Rust tests only", "resolve impediment first"). When present, honor the note before starting general subsection execution.

## Related

- `.claude/skills/continue-roadmap/SKILL.md` — parent skill, contains the gate logic and Implementation Guidelines this skill references.
- `.claude/skills/fix-bug/SKILL.md` — nested for bug fixes.
- `.claude/skills/tpr-review/SKILL.md` — nested for dual-source review.
- `.claude/skills/impl-hygiene-review/SKILL.md` — nested for hygiene sweep.
- `.claude/skills/commit-push/SKILL.md` — nested for close-out commits.
- Memory `project_skill_model_policy.md` — cross-skill Model Policy index.
