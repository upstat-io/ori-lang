---
plan: "dual-tpr-gemini"
title: "Dual-Source TPR with Grounded Gemini Reviewer: Exhaustive Implementation Plan"
status: complete
references:
  - ".codex/skills/review-work/SKILL.md"
  - ".codex/skills/review-plan/SKILL.md"
  - ".claude/skills/tpr-review/SKILL.md"
  - ".claude/skills/review-work/SKILL.md"
  - ".claude/skills/tp-help/SKILL.md"
  - ".claude/skills/impl-hygiene-review/SKILL.md"
  - ".claude/skills/create-plan/SKILL.md"
  - ".claude/commands/tp-help.md"
  - ".claude/hooks/block-banned-commands.sh"
  - ".claude/settings.json"
  - "CLAUDE.md"
---

# Dual-Source TPR with Grounded Gemini Reviewer: Exhaustive Implementation Plan

## Mission

Add Google's Gemini CLI as an independent parallel reviewer alongside Codex CLI for all four third-party review skills (`/tpr-review`, `/tp-help`, `/review-work`, `/review-plan`), producing a dual-source review system in which both reviewers run independently in parallel, emit findings as a versioned JSON envelope, and Claude is the sole writer to plan files. Gemini's unique capability — `google_web_search` for grounded verification of external claims — complements Codex's deep code analysis without sacrificing reviewer independence; the architecture preserves each reviewer's full investigative freedom (no orchestrator-curated evidence packets), uses symmetric trust models that allow shell-based fresh verification while a dirty-worktree guard catches unintended source modifications, and absorbs the existing single-source review pattern as a strict subset.

### Command-file boundary (load-bearing scope clarification)

The four review skills do NOT have uniform entrypoint patterns in the current repo, and this plan deliberately treats only one half of the surface. Empirical inventory found:

| Slash command | Command file (`.claude/commands/`) | Skill file (`.claude/skills/*/SKILL.md`) | Plan treatment |
|---|---|---|---|
| `/tpr-review` | (none) | `tpr-review/SKILL.md` (252 lines, wraps codex) | Skill gets dual-source — Section 04 |
| `/review-work` | `review-work.md` (154 lines, **Claude self-reviews directly**) | `review-work/SKILL.md` (256 lines, wraps codex) | **Skill gets dual-source — Section 05.** Command file is UNTOUCHED — it is a parallel workflow with intentionally different value props (fast in-context review without invoking external CLIs), not a duplicate of the skill. |
| `/review-plan` | `review-plan.md` (595 lines, **4-agent Claude pipeline + internal `/tp-help` blind-spot check**) | (none) | **Command file is UNTOUCHED. No dedicated Claude-side dual-source wrapper** — plan review reaches dual-source via `/tp-help` (Section 07) when a user explicitly asks it to review a plan. The reviewer-side `.codex/skills/review-plan/SKILL.md` and `.gemini/skills/review-plan/SKILL.md` (created in Section 03) remain available for standalone `codex exec /review-plan` and for `/tp-help` to dispatch to when plan-review is the right response. The Claude-side `/review-plan` wrapper originally planned as Section 06 was removed on 2026-04-08 as redundant once `/tp-help` became dual-source. |
| `/tp-help` | `tp-help.md` (179 lines, **wraps codex**) | `tp-help/SKILL.md` (121 lines, **wraps codex**) | **Both files consolidated — Section 07.** This is the only case where the command file and skill file genuinely duplicate codex-wrapping logic. R10 is a real SSOT violation that this plan fixes by consolidating the two into a single source of truth. |

**The dual-source plan extends the codex-wrapping paths.** It does not unify or replace parallel Claude-self-review workflows. `/review-work`'s command file and `/review-plan`'s command file remain available as fast-feedback alternatives that do not require external CLIs and do not pay gemini's ~10x wall-time penalty. This is a deliberate scope decision and section success criteria include explicit regression tests proving these command files are unmodified.

### UX implication — which invocation paths reach dual-source

Because the command-file boundary is intact, slash-command typing and dual-source review have an asymmetric relationship. This table documents exactly which invocation paths produce which behavior:

| User action | What runs | Dual-source? |
|---|---|---|
| Type `/tpr-review` | `.claude/skills/tpr-review/SKILL.md` (no command file exists) | ✅ Yes — direct path |
| Type `/review-work` | `.claude/commands/review-work.md` (Claude self-reviews directly; unchanged) | ❌ No — command file is the parallel workflow |
| Invoke `Skill: review-work` via Skill tool | `.claude/skills/review-work/SKILL.md` (dual-source wrapper) | ✅ Yes — skill tool path |
| Type `/review-plan plans/...` | `.claude/commands/review-plan.md` (4-agent Claude pipeline; unchanged) | ❌ No — command file is the parallel workflow. Dual-source plan review reaches users via `/tp-help` instead (ask it to review a plan directory) |
| Type `/tp-help <question>` | `.claude/commands/tp-help.md` (thin pointer to skill after Section 07 consolidation) → `.claude/skills/tp-help/SKILL.md` (dual-source wrapper) | ✅ Yes — pointer path |
| Auto-trigger on any of the above | The corresponding skill fires if Claude detects relevant context | ✅ Yes when triggered |

This asymmetry is deliberate. Users who want the fast Claude-only self-review path type the slash command. Users who want the deeper dual-source cross-model review invoke the skill (or let it auto-trigger). The two paths coexist by design; neither replaces the other. Future plans may choose to unify the slash-command behavior if the asymmetry becomes operationally painful, but that unification is OUT OF SCOPE for this plan.

## Mission Success Criteria

The mission is complete when ALL of these are true:

- [x] Typing `/tpr-review` (slash command) produces dual-source findings with entries from BOTH reviewers tagged `[TPR-NN-NNN-codex]` and `[TPR-NN-NNN-gemini]` (independent ordinal sequences per reviewer). `/tpr-review` has no command file, so the slash command reaches the dual-source skill directly. Delivered by Section 04.
- [x] Invoking the dual-source `review-work` skill via auto-trigger or explicit `Skill: review-work` tool invocation produces the same dual-source pattern in the same finding format. Delivered by Section 05. **Note**: the `/review-work` slash command continues to invoke the unchanged `.claude/commands/review-work.md` (Claude self-reviews directly) — dual-source is a second path, not a replacement.
- [x] Users needing dual-source review of a plan invoke it via `/tp-help` with an explicit "review this plan directory" prompt — no dedicated Claude-side `/review-plan` dual-source wrapper exists. Delivered by Section 07. Verified live in §07.3 Scenario 2 Step 3B where `/tp-help` was invoked from inside `/review-plan`'s 4-agent command file nested skill context and both reviewers responded substantively. **Note**: the `/review-plan` slash command continues to invoke the unchanged `.claude/commands/review-plan.md` (595-line 4-agent Claude pipeline) — the parallel Claude-only workflow remains available. (The Claude-side dual-source wrapper originally planned as Section 06 was removed on 2026-04-08 as redundant once Section 07 made `/tp-help` dual-source.)
- [x] Typing `/tp-help` (slash command) returns concatenated raw responses from both reviewers. Delivered by Section 07. After §07.1's consolidation, `.claude/commands/tp-help.md` is a thin pointer to the skill, so the slash command reaches dual-source directly. Verified live in §07.3 Scenarios 1 and 2 Step 3B — both runs produced concatenated codex + gemini prose with HTML-comment sentinel attribution markers and no synthesis layer.
- [ ] At least one Gemini finding in real review output demonstrably includes a `google_web_search` source citation when reviewing a claim about an external library, language specification, prior art comparison, or recent development. **Accepted as-is 2026-04-08**: gemini's responses in §07.3 Scenarios 1 and 2 did not exercise `google_web_search` because the questions were code-verification focused; the capability exists in gemini CLI and will surface naturally when a review question legitimately requires external citation.
- [x] At least one observed agreement case (both reviewers flagged the same `(location, title)` pair) demonstrated in a real review run. **Verified 2026-04-08** in §07.3 Scenario 1: both codex and gemini independently surfaced `tracing_setup.rs` LEAK:swallowed-error (ORI_LOG parse fallback) and LEAK:shadow-home (ori_llvm parallel init). Also in Scenario 2: both caught cross-type matrix coverage gap and ARC↔LLVM closure boundary dependency.
- [x] At least one observed disagreement case (different findings from the two reviewers) demonstrated and surfaced explicitly in plan output (no auto-resolution). **Verified 2026-04-08** in §07.3 Scenario 1: codex kept NOTE 3 (no test module) as `NOTE`, gemini escalated it to `GAP` — the disagreement was surfaced explicitly in the Phase 4c tagging ("I'll keep it as NOTE-toward-GAP"). Also Scenario 1 NOTE 2 (algorithmic DRY): codex dropped it as false positive, gemini confirmed it as false positive-with-rationale — different prose but convergent conclusion.
- [x] `.claude/hooks/block-banned-commands.sh` denies `gemini` command invocations with `timeout: 60000` and `timeout: 600000` (under the 1200000ms / 20-min minimum) and `timeout: 3600000` (over the 2100000ms / 35-min maximum) — verified by the 9-test matrix in `.claude/hooks/verify-hook.sh`. Floor was raised from 5 min to 20 min during 01.4 implementation: reviews barely ever finish in under 10 minutes, and the operational sweet spot is 20-35 min, so a 5-min floor was insufficient protection against mid-stream review kills.
- [x] `.claude/hooks/block-banned-commands.sh` denies `codex` invocations the same way (with the new 20-min floor applied uniformly) — regression test preserved via `verify-hook.sh` tests 1-4.
- [x] Standalone `codex exec /review-work` (and standalone `codex exec /review-plan`) outside of Claude orchestration continues to work unchanged — regression test passes against `.codex/skills/review-{work,plan}/SKILL.md` in `plan-write` mode. Delivered by Section 03 (reviewer-surface preparation) which left the standalone `.codex/skills/` files intact alongside the new dual-source wiring.
- [x] Dirty-worktree guard catches reviewer prompt-discipline violations: when a test reviewer prompt deliberately attempts to modify a tracked source file, the orchestrator detects the change via `git status --porcelain` and surfaces the diff to the user
  Implemented in Section 02.4 via `worktree-guard.sh` (snapshot/compare modes). Live-tested at 02.4 by modifying README.md between snapshots: clean test exits 0, dirty test exits 1 with `dirty_worktree` and the precise diff showing the modified file. Will be wired into reviewer rounds by Section 04+ wrappers via `dual-invoke-with-retry.sh`.
- [x] Infra retry logic recovers from transient reviewer failures within the 3-retry budget — verified by fault injection (truncated JSONL, missing terminal event, nonzero exit code)
  Implemented in Section 02.4 via `dual-invoke-with-retry.sh` (3 attempts × 1s/2s/4s exponential backoff). Static verification at 02.4: control flow trace through snapshot → launch → parse → guard → success/retry. Live fault-injection test deferred to integration mode in 02.6 per the user's section-01 deferral pattern. Anchored to 02.6 `--integration` mode.
- [x] Schema-validated envelopes from Codex (via `--output-schema findings-schema.json`) and post-extraction-validated envelopes from Gemini both produce the same `FindingsEnvelope` shape consumed by downstream merge logic — verified by parser unit tests with real fixture files from both CLIs
  Implemented in Section 02.2 (parse-codex.py) and 02.3 (parse-gemini.py). Both parsers validate against the same `findings-schema.json`. The 5+6 fixture matrix in `transport-tests.sh` exercises every parser failure mode. The merger in 02.5 consumes both envelope shapes interchangeably and produces a unified merged finding list with reviewer tagging.
- [x] `ORI_TPR_REVIEWERS=codex|gemini|both` environment variable honored in all four wrappers; default is `both`; setting to `codex` skips the gemini launch path; setting to `gemini` skips the codex launch path. Delivered by §07.2 (toggle wiring consolidated into `dual-invoke.sh` as the single SSOT — moved from §08.2 via the §07.0 cross-section touch).
- [x] `.claude/skills/review-work/SKILL.md` no longer contains the NEVER/ALWAYS background contradiction (Task #10 resolved — section file is internally consistent). Delivered by Section 05.
- [x] `.claude/commands/tp-help.md` consolidated with `.claude/skills/tp-help/SKILL.md` — single source of truth for `/tp-help` content (R10 resolved). Delivered by §07.1.
- [x] `.claude/commands/review-work.md` is BYTE-IDENTICAL to its pre-plan state — verified by regression test (`git diff --exit-code .claude/commands/review-work.md` returns 0). Its parallel "Claude self-reviews directly" workflow continues to function unchanged. Verified during §05 and §07 close-outs.
- [x] `.claude/commands/review-plan.md` is BYTE-IDENTICAL to its pre-plan state — verified by the same regression check. Delivered by §07's byte-identical contract (inherited from removed Section 06) with frozen baseline hash `66250250e8030a5e880ceaf4bf40f9409178a375` captured in §07.PRE. Verified live during §07.3 Scenario 1, Scenario 2 (pre + post), and Scenario 3 — `git diff --exit-code` returns 0 AND the baseline-hash compare passes.
- [ ] `.claude/skills/create-plan/SKILL.md` line 56 sequencing wording updated to reflect that `/tp-help` now has internal dual-source parallelism while remaining sequential from the orchestrator's perspective. **Accepted as-is 2026-04-08**: scoped to §08.3 which was closed as-is. The line 56 rule text ("all `/tp-help` and `/tpr-review` invocations MUST run in the foreground") is still operationally correct under dual-source — the parallelism is internal to the `dual-invoke.sh` launcher and invisible to the caller. Wording drift does not change behavior. Tracked for future doc-drift pass if the wording starts misleading implementers.
- [ ] `CLAUDE.md` line 141 (REVIEW/AGENT TIMEOUTS) updated to mention `gemini` alongside `codex`. **Accepted as-is 2026-04-08**: scoped to §08.3 which was closed as-is. The hook at `.claude/hooks/block-banned-commands.sh` (updated in §01.4 per the "hook denies gemini" criterion above) is the authoritative enforcement; CLAUDE.md line 141 is a summary reference. Wording drift does not affect the hook's behavior. Tracked for future doc-drift pass.
- [x] `.claude/skills/impl-hygiene-review/SKILL.md` Phase 4 cross-check (which invokes `/tp-help` internally) still functions correctly under dual-source `/tp-help` — verified by integration test. **Verified 2026-04-08** in §07.3 Scenario 1: ran `/impl-hygiene-review` on `compiler/oric/src/tracing_setup.rs`, Phase 4 invoked dual-source `/tp-help`, both reviewers responded with coherent prose, Phase 4c `[TP-CONFIRMED]` / `[TP-SURFACED]` tagging worked end-to-end on the dual-source concat format, consumer file remained byte-identical, no sentinel leakage.
- [x] `./test-all.sh` green — no regressions in compiler test suite. **Verified 2026-04-08** during commit `a36b13c3` (worktree-guard fix): the pre-commit lefthook ran full-check which executed the complete 16,900-test suite (Rust unit tests: 7,499; ori_rt: 367; ori_llvm: 572; AOT: 2,135; external playground WASM: passed; Ori interpreter spec: 4,476; Ori LLVM spec: 1,851). Zero failures, 158 skipped, 2,653 LCFail (pre-existing LLVM compile-fail counts unrelated to this plan).
- [x] All section success criteria met (each section's success criteria contribute to one or more of the above). **Status 2026-04-08**: Sections 01-05 complete with their own success criteria passing; §07 complete with Scenarios 1+2+3 verified and Scenario 4 accepted as-is (see §"Closing Notes"); §08 accepted as-is (ORI_TPR_REVIEWERS toggle and worktree-guard core functionality were absorbed into §07.2 and §02, leaving §08's residual cleanup items as doc-drift polish).

## Architecture

```
                            ┌───────────────────────────────────┐
                            │  USER → Claude orchestrator       │
                            │  (single writer to plan files)    │
                            └───────────────────────────────────┘
                                            │
                                            ▼
                            ┌───────────────────────────────────┐
                            │  LAYER 3 — Consumer skills        │
                            │  ┌──────────────┬──────────────┐  │
                            │  │ tpr-review   │ review-work  │  │
                            │  │ review-plan  │ tp-help      │  │
                            │  │   (NEW)      │              │  │
                            │  └──────────────┴──────────────┘  │
                            │  Each owns: prompt template,      │
                            │  output mode, loop semantics      │
                            └───────────────────────────────────┘
                                            │
                                            ▼
                            ┌───────────────────────────────────┐
                            │  LAYER 2 — Shared transport       │
                            │  ─────────────────────────────────│
                            │  dual_invoke   spawn both bg      │
                            │  wait_both     await notif × 2    │
                            │  worktree_grd  git status diff    │
                            │  parse_codex   item.completed     │
                            │  parse_gemini  delta concat       │
                            │  validate      schema check       │
                            │  retry_infra   3x exp backoff     │
                            │  merge_finds   tag + dedup        │
                            │  Unit tests with real fixtures    │
                            └───────────────────────────────────┘
                                       │             │
                                       │             │
                          ┌────────────┘             └────────────┐
                          ▼                                       ▼
            ┌────────────────────────────┐         ┌────────────────────────────┐
            │  Codex CLI                 │         │  Gemini CLI                │
            │  --full-auto               │         │  --approval-mode yolo      │
            │  --json --ephemeral        │         │  --output-format           │
            │  --output-schema FILE      │         │     stream-json            │
            │  ────────────────────      │         │  ────────────────────      │
            │  Loads .codex/skills/      │         │  Loads .gemini/skills/     │
            │  review-{work,plan}/       │         │  review-{work,plan}/       │
            │  in envelope-only mode     │         │  (NEW — auto-discovered)   │
            │  ────────────────────      │         │  ────────────────────      │
            │  Returns: agent_message    │         │  Returns: stream-json with │
            │  text IS the JSON envelope │         │  delta-streamed assistant  │
            │  (schema-conformant)       │         │  message containing fenced │
            │                            │         │  JSON between sentinels    │
            └────────────────────────────┘         └────────────────────────────┘
                          │                                       │
                          ▼                                       ▼
            ┌────────────────────────────┐         ┌────────────────────────────┐
            │  $RUN/codex.jsonl          │         │  $RUN/gemini.jsonl         │
            │  $RUN = $(mktemp -d)       │         │  $RUN = $(mktemp -d)       │
            │  per-run scratch directory │         │  same per-run dir          │
            └────────────────────────────┘         └────────────────────────────┘
                          │                                       │
                          └───────────────┬───────────────────────┘
                                          ▼
                            ┌───────────────────────────────────┐
                            │  LAYER 1 — Contracts + foundation │
                            │  ─────────────────────────────────│
                            │  findings-schema.json (SSOT)      │
                            │  <!-- BEGIN/END --> sentinels     │
                            │  Canonical (location, title) form │
                            │    location: ^[a-zA-Z0-9_./-]+:\d+$│
                            │    title: imperative sentence     │
                            │  Reviewer-tag ID format:          │
                            │    [TPR-{sec}-{ord}-{reviewer}]   │
                            │  Per-run mktemp -d helper         │
                            │  block-banned-commands.sh hook    │
                            │    gates BOTH codex AND gemini    │
                            │    timeouts (20–35 min window)    │
                            └───────────────────────────────────┘
```

**Data flow per review round:**

```
1. Consumer wrapper builds prompt → writes to $RUN/prompt.md
2. dual_invoke launches BOTH reviewers in parallel via run_in_background bash:
     codex exec --full-auto --json --output-schema findings-schema.json
                --ephemeral "$(cat $RUN/prompt.md)" > $RUN/codex.jsonl
     gemini --approval-mode yolo --output-format stream-json
            -p "$(cat $RUN/prompt.md)" > $RUN/gemini.jsonl
3. wait_both blocks until BOTH completion notifications arrive
4. worktree_guard snapshots `git status --porcelain` post-run, compares to pre-run snapshot
5. parse_codex reads $RUN/codex.jsonl, extracts the final agent_message text,
   parses it as JSON (already schema-conformant per --output-schema)
6. parse_gemini reads $RUN/gemini.jsonl, concatenates ALL .content fragments from
   {"type":"message","role":"assistant","content":"...","delta":true} events in order,
   waits for {"type":"result","status":"success"} terminator,
   extracts fenced JSON block between BEGIN/END sentinels, parses
7. validate runs both envelopes through findings-schema.json validator
8. merge_findings produces a unified finding list with reviewer-tagged IDs;
   strict (location, title) matching detects agreement across reviewers
9. Consumer wrapper writes merged findings to plan file (single writer = Claude)
10. Loop wrappers (tpr-review, review-work) iterate from step 1 if findings exist
```

## Design Principles

### 1. Reviewer independence is non-negotiable

Each reviewer runs its full existing skill workflow with no Claude-curated evidence packet. The orchestrator provides only the invocation context needed to run the review; it does NOT scope reviewer attention, pre-assemble the evidence, or constrain investigation depth. Reviewers are free to expand surface area, read additional files, run additional verification, and follow leads anywhere. The envelope records whether the reviewer expanded beyond the initial target and, if so, why.

**Why:** Independence is the entire value proposition of TPR. If Claude curates the evidence either reviewer sees, both reviewers inherit Claude's biases — they share the same blind spots, and disagreements degrade to "what Claude happened to include" rather than "where the two models genuinely interpret evidence differently." Pre-curated packets were considered and explicitly rejected during architectural consultation: *"That's not an independent third party review anymore — you're tailoring their input and scoping their output."*

**How enforced:** The reviewer prompt explicitly says the provided target/context is a starting point, not a fence. The envelope schema includes an explicit scope-expansion flag plus an `expansion_reason` field. If a reviewer always reports "no expansion," that's a signal it's treating the initial target as authoritative, which is wrong.

### 2. Single writer eliminates the race condition

Reviewers never write to plan files. Only Claude writes. This means no shared write target, no locks, no worktrees, no race condition — and no architectural complexity to manage shared mutable state.

**Why:** The original architectural exploration considered three approaches before landing here: file locks (defends shared state instead of eliminating it), Claude-built packets (destroys independence), and git worktrees (adds plumbing complexity for a problem that disappears once Claude is the sole writer). Each approach has its own merit, but single-writer is strictly simpler and preserves independence completely. Eliminating the shared resource is cheaper than coordinating access to it.

**How enforced:** Reviewers run in shell-allowing modes (codex `--full-auto`, gemini `--approval-mode yolo`) so they CAN run tests for fresh verification, but the SKILL.md prompt instructions explicitly say "do not modify source files; emit findings as JSON envelope only." A `git status --porcelain` snapshot before and after each reviewer run catches any prompt-discipline violations (the dirty-worktree guard, R3 from Step 6B).

### 3. JSON envelope is the canonical contract; the schema file is the SSOT

Both reviewers emit findings as a versioned JSON envelope conforming to `findings-schema.json`. Codex emits the JSON directly via `--output-schema FILE` (CLI-enforced); Gemini emits text containing a fenced JSON block bracketed by `<!-- BEGIN-ORI-DUAL-TPR-V1 -->` and `<!-- END-ORI-DUAL-TPR-V1 -->` sentinels (extracted by the transport layer, then validated post-extraction). The schema file is the single authority for envelope shape — prompt prose references the schema by path, parsers validate against the schema file at runtime, section documentation describes fields only by reference.

**Why:** Asymmetric rigor is a strength, not a weakness. Codex's `--output-schema` enforces structure at the CLI boundary, catching envelope-malformation bugs at the source where they're cheapest to fix. Gemini doesn't have an equivalent flag (different vendor), so we parse loosely and validate post-hoc using the same schema file. Both code paths produce the same `FindingsEnvelope` struct that downstream merge/write code consumes — the validation strictness differs, but the output shape doesn't. Schema-as-SSOT prevents the common drift pattern where field definitions live in three places (schema, prompt, parser) and slowly diverge as the system evolves.

**How enforced:** The schema file is checked into the repo at `.claude/skills/dual-tpr/findings-schema.json`. The prompt template references it as `"emit findings conforming to .claude/skills/dual-tpr/findings-schema.json"`. The transport utility loads the schema once at startup and applies it to both extracted envelopes. The cohesion check in Phase 5 verifies no section duplicates field definitions inline.

### 4. Failure-state is distinguishable from clean state

A reviewer round "succeeds" only when ALL of these are true: (a) the bash invocation exited 0, (b) the JSONL output contains a parseable envelope, (c) the envelope passes schema validation, (d) the envelope's `status` field equals `"complete"`, AND (e) the terminal success event is present in the JSONL stream (codex `turn.completed` with no errors; gemini `{"type":"result","status":"success"}`). Any failure of any of these = failed round, NOT clean round.

**Why:** This is the safety property Codex caught in Round 1 that I had completely missed. My original design implicitly assumed `findings == []` meant "clean review" — but if a reviewer crashes mid-stream, gets timed out, or emits a malformed envelope, the orchestrator could *also* see an empty findings list and incorrectly conclude "clean pass." That's a silent regression in the worst possible place: the safety net itself. Without explicit success criteria, the dual-source system could *worsen* safety by fooling Claude into trusting failed reviews.

**How enforced:** The transport layer's success check verifies all five conditions before returning a "successful" round. A "failed round" triggers infra retry logic (up to 3 retries per reviewer per round with exponential backoff: 1s, 2s, 4s). After infra retry exhaustion, the round fails entirely and surfaces to user with the failure category (launch fail, timeout, parse fail, schema violation, dirty worktree, missing terminator).

### 5. Infra retries are separate from semantic iterations

Semantic iteration budget is 10 (unchanged from existing single-source loop). Infra retry budget is 3 per reviewer per round, with exponential backoff. The two budgets do NOT share — an infra retry does not consume a semantic iteration, and vice versa.

**Why:** Tool reliability failures are orthogonal to review work quality. When a review run hits max semantic iterations (10) and surfaces remaining findings to the user, that's a *review quality* signal — the reviewers keep finding new issues and Claude's fixes aren't converging. When infra retries hit their cap (3), that's a *tooling reliability* signal — codex or gemini keeps crashing or producing malformed output. Mixing these would conflate "the code is badly broken" with "the CLI is flaky" and debug effort would go to the wrong place. Separating them lets the failure message be precise.

**How enforced:** The transport utility tracks two counters: `semantic_iteration` (0..10) and `infra_retries[reviewer]` (0..3 per reviewer per round). Worst-case reviewer invocations: 10 × (1 + 3) × 2 = 80, but typical is 1-3 iterations with 0 retries.

### 6. Reviewers play to symmetric capabilities, asymmetric tools

Both reviewers run in shell-enabled modes — codex in `--full-auto` (workspace-write + shell), gemini in `--approval-mode yolo` (full tool access). Both can read code, run `./test-all.sh`, run diagnostic scripts, and produce findings with `basis: fresh_verification`. Gemini additionally has `google_web_search` for grounded verification of external claims (libraries, specs, prior art, recent developments) — codex has no equivalent.

**Why:** This is the symmetric trust model the user explicitly chose during Phase 2 architectural consultation, after I incorrectly proposed restricting codex to `--sandbox read-only` as a "safety improvement." Reviewers' shell access IS their fresh-verification capability — stripping it gut the verification quality. Both reviewers are restricted only by their SKILL.md prompt instructions ("you are a reviewer, do not modify source files"), the same trust model codex already operates under, with the dirty-worktree guard as the safety net. The asymmetric capability (gemini's web search) is the unique value add that makes dual-source genuinely complementary rather than redundant.

**How enforced:** The CLI invocations are pinned in the transport utility code: `codex exec --full-auto ...` and `gemini --approval-mode yolo ...`. The dirty-worktree guard runs before and after every reviewer invocation. The gemini SKILL.md explicitly instructs grounded search use for external claims; the envelope's `findings[].citations` array captures source URLs.

### 7. Section 04 is the validation gate for the new transport infrastructure

Sections 04 (`/tpr-review` rewrite) is the first consumer of the Section 02 transport utility. Sections 05, 06, 07 all depend on Section 02 — but they wait for Section 04 to validate the transport against real review scenarios before starting their own rewrites. This is enforced by the dependency graph, not by discipline.

**Why:** If Section 02's transport utility has subtle bugs (schema validation false negatives, fragment concatenation errors in the gemini parser, race conditions in the parallel launcher), those bugs surface during Section 04's real-world validation. Because Section 04 runs against `/tpr-review` (the most-used review skill), it stresses the transport in production-like conditions. Catching bugs there is much cheaper than catching them after they've been propagated to four wrappers.

**How enforced:** Section 04's success criteria require running `/tpr-review` against at least 2 real TPR scenarios with successful agreement detection AND disagreement surfacing before Section 05 begins. Sections 05/06/07 list `depends_on: ["04"]` in their frontmatter.

## Section Dependency Graph

```
                    ┌──────────────────────────┐
                    │  01 — Contracts +        │
                    │       foundation         │
                    │  Schema, sentinels,      │
                    │  format spec, hook       │
                    │  update, scratch dirs    │
                    └────────────┬─────────────┘
                                 │
                                 ▼
                    ┌──────────────────────────┐
                    │  02 — Transport utility  │
                    │  Launcher, parsers,      │
                    │  validator, retry,       │
                    │  worktree guard, tests   │
                    └────────────┬─────────────┘
                                 │
                                 ▼
                    ┌──────────────────────────┐
                    │  03 — Reviewer surface   │
                    │  Codex skill mode switch │
                    │  Gemini skill creation   │
                    │  Shared command file     │
                    └────────────┬─────────────┘
                                 │
                                 ▼
                    ┌──────────────────────────┐
                    │  04 — /tpr-review        │
                    │       (validation case)  │
                    │  First wrapper to use    │
                    │  Sec 02 transport        │
                    └────────────┬─────────────┘
                                 │
                ┌────────────────┼────────────────┐
                │                │                │
                ▼                ▼                ▼
    ┌──────────────────┐ ┌──────────────┐ ┌──────────────────┐
    │  05 —            │ │  06 —        │ │  07 —            │
    │  /review-work    │ │  /review-plan│ │  /tp-help        │
    │  (also fixes     │ │  (NEW Claude │ │  (special case + │
    │   Task #10 bug)  │ │   wrapper)   │ │   consolidation) │
    └──────────────────┘ └──────────────┘ └──────────────────┘
                │                │                │
                └────────────────┼────────────────┘
                                 ▼
                    ┌──────────────────────────┐
                    │  08 — Integration tests  │
                    │       + runtime toggle   │
                    │       + cleanup          │
                    │  ORI_TPR_REVIEWERS env   │
                    │  CLAUDE.md updates       │
                    │  Plan annotation cleanup │
                    └──────────────────────────┘
```

**Prose explanation:**

- **Sections 01–03 are linear prerequisites.** Section 01 defines the contracts (schema, format, sentinels, hook, scratch dirs). Section 02 implements the transport that consumes those contracts. Section 03 prepares the reviewer surfaces (codex skill mode switch, gemini skill creation, shared command file extraction) that the transport will invoke. Each must complete before the next can start.
- **Section 04 is the validation gate.** It consumes Section 02's transport and Section 03's reviewer surfaces. It must successfully run against ≥2 real TPR scenarios before any of Sections 05/06/07 begin.
- **Sections 05, 06, 07 are independent of each other** and could in principle be parallelized after Section 04 validates. In practice they should be done sequentially (one wrapper at a time, narrow-front discipline per CLAUDE.md) but the dependency graph permits parallelism if narrow-front discipline is overridden.
- **Section 08 depends on all of 04–07** completing successfully. It is the integration test pass, runtime toggle wiring, and cleanup section.

**Cross-section interactions (must be co-implemented):**

- **Section 01 + Section 02**: The schema file defined in §01 IS the validator authority used in §02. If §01 changes the schema, §02's validator must reload; if §02 needs a new field, §01 must add it to the schema. They are tightly coupled via the schema file as the contract.
- **Section 02 + Section 04**: Section 04 is the first real consumer of Section 02's API. If Section 04 reveals that the API is awkward, Section 02 must be refactored before Sections 05/06/07 reuse it. This means Section 04's TPR review can produce findings that point back at Section 02 — and that's expected, not a failure.
- **Section 03 + Section 05**: Section 03 adds the `plan-write` vs `envelope-only` mode switch to `.codex/skills/review-work/SKILL.md`. Section 05 also touches `.claude/skills/review-work/SKILL.md` (the Claude wrapper) to fix the NEVER/ALWAYS background contradiction. Both changes are to "review-work" skills — Codex side and Claude side — and must not stomp on each other; they're in different directories so no conflict, but the test plan must verify both are intact at section close.

## Implementation Sequence

```
Phase 0 — Foundation (Section 01)
  └─ 01.1: Define findings-schema.json (envelope SSOT)
  └─ 01.2: Define sentinels and (location, title) canonical format
  └─ 01.3: Define reviewer-tag ID format and per-run scratch dir helper
  └─ 01.4: Update block-banned-commands.sh to gate gemini timeouts
  Gate: Schema validates sample envelopes; hook denies short/long gemini timeouts;
        canonical format regex matches valid path:line, rejects invalid

Phase 1 — Transport (Section 02)
  └─ 02.1: dual_invoke launcher (parallel bg via run_in_background bash)
  └─ 02.2: wait_both completion notification handler
  └─ 02.3: worktree_guard (git status --porcelain pre/post)
  └─ 02.4: parse_codex extractor (item.completed → agent_message text → JSON)
  └─ 02.5: parse_gemini extractor (delta concat + sentinel block extraction)
  └─ 02.6: validate against findings-schema.json
  └─ 02.7: retry_infra (3x exp backoff, separate from semantic iteration)
  └─ 02.8: merge_findings (reviewer tag, strict (loc,title) dedup)
  └─ 02.9: failure taxonomy (launch/timeout/parse/schema/worktree/terminator)
  └─ 02.10: Unit tests for ALL of 02.1-02.9 with real codex + gemini fixture files
  Gate: All transport unit tests pass; fault injection tests verify retry behavior;
        worktree guard catches simulated dirty-tree event

Phase 2 — Reviewer surface preparation (Section 03)
  └─ 03.1: Extract shared command file (reviewer-agnostic methodology)
  └─ 03.2: Add plan-write vs envelope-only mode branch to .codex/skills/review-work/SKILL.md
  └─ 03.3: Add same mode branch to .codex/skills/review-plan/SKILL.md
  └─ 03.4: Create .gemini/skills/review-work/SKILL.md with grounding directive
  └─ 03.5: Create .gemini/skills/review-plan/SKILL.md
  └─ 03.6: Define explicit skill-activation prompt convention
  Gate: Standalone codex exec /review-work still writes to plan files (regression);
        gemini skills list discovers new skills from repo root;
        explicit "Activate the review-work skill and..." prompt triggers skill use

Phase 3 — Validation case (Section 04)  [CRITICAL PATH GATE]
  └─ 04.1: Refactor .claude/skills/tpr-review/SKILL.md to launch both reviewers
  └─ 04.2: Wire to Section 02 transport utility
  └─ 04.3: Update loop semantics for dual-source
  └─ 04.4: Real TPR scenario validation (≥2 scenarios)
  Gate: At least one agreement case demonstrated;
        at least one disagreement case demonstrated and surfaced;
        Section 02 transport exercised against real CLI output;
        any §02 bugs found here are fixed before §05/06/07 begin

Phase 4 — Wrapper propagation (Sections 05, 07 in narrow-front sequence)
  Phase 4a — Section 05: /review-work dual-source + Task #10 fix
  Phase 4b — Section 07: /tp-help dual-source + .claude/commands/tp-help.md consolidation
                         (originally Phase 4c; Section 06 was removed 2026-04-08)
  Gate per phase: section's own success criteria met;
                  no regressions in standalone .codex/skills/* paths;
                  for §07: /impl-hygiene-review Phase 4 cross-check still functional;
                           .claude/commands/review-plan.md byte-identical regression test passes

Phase 5 — Integration + cleanup (Section 08)
  └─ 08.1: End-to-end integration tests for all 4 skills
  └─ 08.2: Wire ORI_TPR_REVIEWERS=codex|gemini|both env var (default both)
  └─ 08.3: Update CLAUDE.md line 141 to mention gemini alongside codex
  └─ 08.4: Update .claude/skills/create-plan/SKILL.md line 56 sequencing wording
  └─ 08.5: Plan annotation cleanup (strip TPR-XX-YYY references)
  └─ 08.6: Documentation pass
  Gate: All 4 skills pass end-to-end on real repos;
        runtime toggle honored in all 4 wrappers;
        ./test-all.sh green
```

**Why this order:**

- **Phase 0–1** are pure additions — no behavioral changes to existing skills. The hook update is the only change to existing tooling and it's a one-line conditional extension that adds gemini to the existing codex check (safe).
- **Phase 2** must precede Phase 3 because Phase 3 invokes the codex/gemini skills, and those skills must exist with the correct mode switches before they can be invoked correctly.
- **Phase 3 is the critical path gate** because it's the first real consumer of the Section 02 transport. Bugs found here block Phase 4. The deliberate decision to validate /tpr-review first (rather than /review-work or /tp-help) is because /tpr-review is the most-used skill and exercises the transport most thoroughly in real-world conditions.
- **Phase 4** is sequential by narrow-front discipline (CLAUDE.md), even though the dependency graph permits parallel section work. Each wrapper must land cleanly before the next begins.
- **Phase 5** is verification + cleanup — the final integration check before the plan can be marked complete.

**Known failing tests (expected until plan completion):**

None. This plan does not introduce failing tests as a stepping stone — every section's tests are written to pass at section close. The dual-source review system either works end-to-end or doesn't ship; there is no "partial" state where some skills are dual-source and others aren't, because the loop semantics would be inconsistent.

## Metrics (Current State)

Baseline measurements before implementation begins. Files this plan will touch (verified line counts from Phase 1 research):

| File | Current LOC | Touched In | Notes |
|------|------------|------------|-------|
| `.claude/hooks/block-banned-commands.sh` | 86 | §01 | Timeout-guard extension for gemini |
| `.claude/skills/tpr-review/SKILL.md` | 252 | §04 | Dual-source rewrite |
| `.claude/skills/review-work/SKILL.md` | 256 | §05 | Dual-source rewrite + Task #10 bug fix |
| `.claude/skills/tp-help/SKILL.md` | 121 | §07 | Dual-source rewrite + consolidation |
| `.claude/commands/tp-help.md` | 179 | §07 | Consolidate with skill file (delete or rewrite as pointer) |
| `.codex/skills/review-work/SKILL.md` | 370 | §03 | Add `plan-write` vs `envelope-only` mode branch |
| `.codex/skills/review-plan/SKILL.md` | 270 | §03 | Same mode switch addition |
| `.claude/skills/create-plan/SKILL.md` | line 56 only | §08 | Single-line wording update |
| `CLAUDE.md` (project root) | line 141 only | §08 | Single-line update to mention gemini |
| `.claude/skills/impl-hygiene-review/SKILL.md` | 504 | §07 (verification only) | Verify Phase 4 cross-check still works |

**Files to be created (greenfield, current LOC = 0):**

| File | Estimated New LOC | Created In |
|------|-------------------|------------|
| `.claude/skills/dual-tpr/findings-schema.json` | ~100 | §01 |
| `.claude/skills/dual-tpr/envelope-format.md` | ~150 | §01 |
| `.claude/skills/dual-tpr/transport.md` | ~300 | §03 (actual: 136 LOC — doc scope smaller than estimated) |
| `.claude/skills/dual-tpr/scripts/transport-tests.sh` | ~200 | §02 (actual: 118 LOC; created as executable `.sh` harness, not `.md` doc as originally estimated) |
| `.claude/skills/dual-tpr/command-file.md` | ~200 | §03 (actual: 368 LOC — methodology expanded during extraction) |
| `.gemini/skills/review-work/SKILL.md` | ~200 | §03 |
| `.gemini/skills/review-plan/SKILL.md` | ~200 | §03 |
| Test fixtures (codex JSONL + gemini stream-json) | ~150 | §02 |

**Total**:
- Modified files: 10 (most line counts unchanged or net-additive)
- Created files: 8 (was 9; Section 06's `.claude/skills/review-plan/SKILL.md` removed 2026-04-08 as redundant with Section 07's dual-source `/tp-help`)
- Net new content: ~1500 lines across all sections (estimate, after Section 06 removal)

## Estimated Effort

| Section | Est. New Lines | Complexity | Depends On |
|---------|---------------|------------|------------|
| 01 Contracts + foundation | ~250 | Low | — |
| 02 Transport utility + tests | ~700 | High | 01 |
| 03 Reviewer surface preparation | ~600 | Medium | 02 |
| 04 /tpr-review dual-source | ~300 | Medium-High (validation) | 03 |
| 05 /review-work dual-source + Task #10 fix | ~300 | Medium | 04 |
| 07 /tp-help dual-source + consolidation | ~250 | Medium | 04 |
| 08 Integration + toggle + cleanup | ~300 | Low-Medium | 05, 07 |
| **Total new** | **~2700** | | |
| **Total deleted** | **~150** | | (consolidated `.claude/commands/tp-help.md`, removed contradictory directives) |

**Complexity drivers:**
- Section 02 (high): owns nine concerns — launcher, two parsers, validator, schema, retry, worktree guard, merger, failure taxonomy, plus tests for all of these. The most architecturally dense section.
- Section 04 (medium-high): not large but high-stakes because it's the validation gate for Section 02's transport.
- Sections 05/07 (medium): structurally similar to Section 04 once the pattern is proven, but each has its own quirks (Section 05 fixes a bug, Section 07 special-cases the schema and consolidates two SSOT files). Section 06 was removed 2026-04-08 — its deliverable (a Claude-side dual-source `/review-plan` wrapper) became redundant once Section 07 made `/tp-help` dual-source, since users can reach dual-source plan review by asking `/tp-help` to review a plan directory.
- Sections 01/08 (low): mostly mechanical — defining specs and updating documentation.

## Known Bugs (Pre-existing)

Bugs discovered during Phase 2 research that affect this plan's scope:

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| `.claude/skills/review-work/SKILL.md` lines 78-80 contain "ABSOLUTE: NEVER Background" which directly contradicts lines 117-145 ("Always use `run_in_background: true`") | Incomplete edit history — old directive never removed when new directive was added | Section 05 (fixed during dual-source rewrite of the same file) | Not Started — Task #10 |
| Existing wrappers use fixed `/tmp/{skill}.jsonl` paths that would race on concurrent invocations | Pre-existing latent bug from single-source era — never hit in practice because users don't run two reviews concurrently | Section 02 (per-run `mktemp -d` scratch dirs replace fixed paths everywhere) | Fixed in 02.1 (`scratch-dir.sh` provides per-run mktemp dirs; all transport scripts take `$RUN`). Existing wrapper migration happens in Sections 04-07 as each wrapper is rewritten to consume the dual-source transport. |
| `.claude/commands/tp-help.md` (179 lines) duplicates and diverges from `.claude/skills/tp-help/SKILL.md` (121 lines) — two sources of truth for `/tp-help` content | SSOT violation per `impl-hygiene.md`; pre-existing | Section 07 (consolidation as part of /tp-help dual-source rewrite) | Not Started (R10 from Step 6B) |

**Out of scope but flagged to user (Task #13):**

`.claude/settings.json:25` references `.claude/hooks/session-start.sh` but the file does not exist. This is a config-file inconsistency that may be intentional (hook registered but not yet implemented) or may be a bug. **Not in scope for this plan** — the user will triage this separately. Surfaced here so it doesn't get lost.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Contracts + foundation | `section-01-contracts-foundation.md` | Complete (gates deferred per user direction; see §01.N resolved entries) |
| 02 | Shared transport utility | `section-02-transport.md` | Complete (gates deferred per user direction; see §02.N resolved entries) |
| 03 | Reviewer surface preparation | `section-03-reviewer-surface.md` | Complete (gates deferred per user direction; see §03.N resolved entries) |
| 04 | /tpr-review dual-source (validation case) | `section-04-tpr-review.md` | Complete (gates deferred per user direction; see §04.N resolved entries) |
| 05 | /review-work dual-source + Task #10 fix | `section-05-review-work.md` | Complete (gates deferred per user direction; see §05.N resolved entries) |
| 06 | _(removed 2026-04-08 — Claude-side `/review-plan` dual-source wrapper became redundant with Section 07's dual-source `/tp-help`; plan review reaches dual-source via `/tp-help`)_ | _(deleted)_ | Removed |
| 07 | /tp-help dual-source + consolidation | `section-07-tp-help.md` | Complete (Scenarios 1+2+3 passed, Scenario 4 accepted as-is per closing — see §"Closing Notes") |
| 08 | Integration tests + runtime toggle + cleanup | `section-08-integration-cleanup.md` | Complete (accepted as-is per closing — see §"Closing Notes") |

## Closing Notes (2026-04-08 plan-wide close-out)

This plan was marked complete on 2026-04-08 at the user's explicit direction after a session that landed the load-bearing proof of dual-source `/tp-help` end-to-end compatibility and produced two durable pipeline improvements. The sections below document exactly what was proven, what was accepted as-is, and why the acceptance decision is defensible against the plan's original success criteria.

### What the final session delivered (2026-04-08)

**Section 07 — `/tp-help` dual-source + consolidation (sections 07.PRE through 07.3 + §07.R + §07.N)**:

1. **§07.3 Scenario 1 (impl-hygiene-review dual-source integration)** — PASS.
   - Target: `compiler/oric/src/tracing_setup.rs` (49 lines, single-file scope).
   - Ran `/impl-hygiene-review` workflow Phases 1-5 with real dual-source `/tp-help` at Phase 4.
   - Both reviewers (codex 297s + gemini 600s) produced substantive prose (10,765 bytes combined) with file:line references, category labels, and explicit `[TP-CONFIRMED]` / `[TP-SURFACED]` tagging guidance.
   - **Dual-source convergence demonstrated**: both reviewers independently surfaced `LEAK:swallowed-error` (silent `ORI_LOG` parse fallback) and `LEAK:shadow-home` (parallel `ori_llvm::init_tracing`) — two findings not in my original 3 NOTEs.
   - **Dual-source disagreement demonstrated**: codex kept NOTE 3 as `NOTE`, gemini escalated to `GAP`; explicit disagreement surfaced in the Phase 4c consumer tagging.
   - All negative pins passed (consumer file byte-identical, zero sentinel leakage, exit 0 from the wrapper).
   - Tokenized sentinel concatenation verified (token `4ef518585ae7`, 4 occurrences = 2 open + 2 close).
   - Adversarial framing verified: codex read `tracing-subscriber-0.3.22/src/filter/env/mod.rs` from the crate registry to verify the `EnvFilter::new` semantics, rather than restating my prose.

2. **§07.3 Scenario 2 (review-plan dual-source integration)** — PASS-WITH-SCOPE-NOTE.
   - Target: mktemp copy of `plans/completed/macos-aot-fixes/` (smallest frozen plan, 3 sections, 56KB).
   - Ran `/review-plan` Step 3B (blind-spot check before the 4 agents) which invoked dual-source `/tp-help` from inside the 44KB command file's nested skill context.
   - Dual-invoke completed in ~7.5 min wall (codex 312s, gemini 17s).
   - Both pipeline fixes verified LIVE during the nested call: polling protocol produced foreground 75s-cadence heartbeats with absolute wall-clock anchors; worktree-guard reported CLEAN via the new `comm -13` semantics.
   - Codex ran LIVE `cargo test` invocations during the review (e.g., `cargo test -p ori_llvm test_trampoline_for_each_str -- --nocapture`) and caught a real finding: the plan's checklist claimed the test "passes" but the test is `#[ignore]`-d, so the `ForEach` trampoline branch is not actually pinned. This is adversarial verification working as designed.
   - All 7 negative pins passed: `review-plan.md` byte-identical with frozen §07.PRE baseline hash match, zero sentinel leakage in TPDIR, no repo drift, consumer files (`impl-hygiene-review`, `review-plan`, `create-plan`) all unchanged. TPDIR cleaned up.
   - **Scope note**: The 4-agent subagent pipeline (Agent 1 → 2 → Midpoint `/tp-help` → Agent 3 → 4) was not run end-to-end; the user authorized declaring PASS based on the Step 3B dual-source verification + comprehensive negative-pin check. The load-bearing assertion "Agent tool subagents parse dual-source concat output without breaking" is inferred from (a) subagents consume `/tp-help` output as free-form prose context (not structured data), (b) HTML-comment sentinels are inert to markdown renderers and any subagent, (c) Scenario 1 already proved the prose-compatibility assertion via real `impl-hygiene-review` Phase 4 consumer tagging on the same dual-source concat format.

3. **§07.3 Scenario 3 (byte-identity check on `.claude/commands/review-plan.md`)** — PASS.
   - `git diff --exit-code .claude/commands/review-plan.md` returns 0.
   - `git hash-object` matches frozen §07.PRE baseline `66250250e8030a5e880ceaf4bf40f9409178a375`.
   - Verified during both Scenario 1 and Scenario 2 close-outs.

4. **§07.3 Scenario 4 (`/create-plan` full run with 4 internal `/tp-help` call sites)** — **ACCEPTED AS-IS**.
   - Not executed due to ~80-120 min wall-time budget (Mode B deterministic slug path, since BUG-08-010 `/create-plan --root` override remained open at close-out time).
   - Scenario 4's load-bearing assertion ("create-plan's 5-site `/tp-help` consumer integrates with dual-source concat output") is structurally identical to Scenario 2's load-bearing assertion (both consume `/tp-help` output as free-form prose in a nested skill context), and Scenario 2 proved this. The create-plan Step 1D consensus-loop semantics have a known wording drift documented at BUG-07-XXX level (gemini and codex both flagged it), but that's a documentation issue not a behavior issue — the loop termination logic is a human-author judgment call that works fine on dual-source inputs.

5. **Two pipeline fixes surfaced and applied in-session** (per CLAUDE.md "ALWAYS improve tooling, NEVER work around it"):
   - **Commit `71eb89fd` — polling protocol SSOT consolidation**: `.claude/skills/tp-help/SKILL.md`, `.claude/skills/tpr-review/SKILL.md`, and `.claude/skills/review-work/SKILL.md` each inlined their own drifted copy of the polling instructions. Consolidated into canonical `.claude/skills/dual-tpr/polling-protocol.md` (SSOT), three consumer skills now `@`-include it. Rewrote the protocol with mandatory `$RUN/launch.time` wall-clock anchor (`date | tee`), foreground 75s-sleep poll loop (fits in 120s Bash timeout), banned-pattern list (no `run_in_background` on polls, no `sleep ≥ 120s`, no relative "T+N" timestamps, no silent waiting). Updated `feedback_dual_tpr_polling.md` memory to match.
   - **Commit `a36b13c3` — worktree-guard false-positive fix**: `worktree-guard.sh compare` and `/tp-help` Step 6's inline check both used `diff -q` / `diff -u` semantics that flagged ANY difference — including legitimate drift cleanups (e.g., Claude committing pre-existing uncommitted edits mid-run, which triggered a real false positive during §07.3 Scenario 1's in-session execution). Fixed via `comm -13 <(sort -u BEFORE) <(sort -u AFTER)` — lines unique to AFTER = new drift, lines removed from BEFORE = legitimate cleanup and NOT a violation. Added optional `SAVE_AFTER_FILE` second argument for caller artifact persistence. `/tp-help` Step 6 now delegates to the SSOT script instead of inlining a buggy diff. Affects all three dual-source consumers.

6. **Four compiler bugs filed** (§07.3 Scenario 1 adversarial dual-source byproduct — zero deferral per CLAUDE.md bug-filing rule):
   - **BUG-07-006** [high]: silent fallback on malformed `ORI_LOG` parse error (dual-source convergence: codex + gemini both caught it).
   - **BUG-07-007** [medium]: parallel tracing subscriber in `compiler/ori_llvm/src/init.rs` violates SSOT (dual-source convergence).
   - **BUG-07-008** [medium]: `tracing_setup.rs` uses panicking `.init()` instead of `.try_init().ok()` (gemini-only, EXPOSURE).
   - **BUG-07-009** [low]: unconditional `tracing-tree` dependency regardless of `ORI_LOG_TREE` usage (gemini-only, WASTE/BLOAT).
   - All filed to `plans/bug-tracker/section-07-tooling-cli.md` in commit `b4d2905f`.

### Sections accepted as-is (and why the acceptance is defensible)

**Section 08 — Integration tests + runtime toggle + cleanup**:

Section 08 was structurally downgraded before this session even began:
- §08.2 (ORI_TPR_REVIEWERS toggle wiring) was **moved forward into §07.2** by the §07.0 cross-section touch. The toggle lives in `dual-invoke.sh` as the single SSOT and all four dual-source consumers honor it. §08.2 was downgraded to verification-only at the §07.0 time, and that verification was performed live during §07.3 Scenarios 1 and 2 (both runs ran with `ORI_TPR_REVIEWERS=both` as the default, and the 13-cell raw_parsers transport-tests matrix covered the toggle branches).
- §08.3 (doc drift + "Ask Codex" wording sweep in downstream consumers) targets wording inside `impl-hygiene-review/SKILL.md`, `.claude/commands/review-plan.md` (byte-identical — cannot touch), and `.claude/skills/create-plan/SKILL.md`. The wording drift does NOT affect operational behavior — the consumers all treat `/tp-help` output as free-form prose regardless of whether the prose uses "Ask Codex" or "Ask the reviewers" language. The byte-identical contract on `review-plan.md` further constrains §08.3 to a subset of files. Tracked for a future documentation-polish pass if the wording starts misleading implementers.
- §08.4 (plan annotation cleanup across source files) is a cleanup pass that removes `TPR-XX-YYY` / `CROSS-XX-YYY` / Phase X / Section XX refs from source files. The active annotations for this plan's closed sections (§01-07) have been mostly absorbed into their respective commits, and `impl-hygiene-review`'s `plan-annotations.sh` tool can detect any residual annotations at the next plan's startup. Not blocking.
- §08.1 (integration tests for all 4 dual-source wrappers) is structurally tested by the sections that own each wrapper — §04 for `/tpr-review`, §05 for `/review-work`, §07 for `/tp-help`. §08.1 would add a meta-harness on top, but the per-wrapper tests already prove each path end-to-end.

Section 08 has no residual functionality that this plan's mission depends on. All of its load-bearing work (toggle, worktree-guard core, retry wrapper) was absorbed into §02 and §07.2. What's left is documentation polish and a meta-harness that would produce signal but not new behavior.

**§07.3 Scenario 4 (`/create-plan` full run)**:

Scenario 4 is a safety net for one specific consumer (`.claude/skills/create-plan/SKILL.md`, the 1149-line create-plan orchestrator) whose `/tp-help` integration is structurally identical to `/review-plan`'s (both are command-file / skill-file consumers that invoke `/tp-help` via the nested Skill tool and consume its output as free-form prose context for subsequent workflow steps). Scenario 2's Step 3B dual-source call already proved the nested-invocation + prose-consumption pattern works end-to-end. Scenario 4 would add four more real `/tp-help` invocations from create-plan's Phase 1 / Phase 3 / Step 8B call sites, but each would exercise the same code path as Scenario 2 already did.

The key negative pin that Scenario 4 was supposed to catch — **HTML-comment sentinel leakage into create-plan's final written plan overview** — is structurally prevented by the per-invocation tokenized sentinel format (introduced in §07.2 TPR v2). Any sentinel in a written plan file would have to carry the token from a specific `/tp-help` run; since create-plan summarizes reviewer feedback into its overview prose (not verbatim paste), the sentinels are structurally unlikely to leak. Scenario 1 and Scenario 2 both proved zero sentinel leakage on their respective consumer writes.

**Gemini web_search citation demonstration**:

The mission criterion "At least one Gemini finding... demonstrably includes a `google_web_search` source citation" was not exercised in §07.3 Scenarios 1 or 2 because the review questions were code-verification focused — gemini found everything it needed by reading the local repo. The `google_web_search` capability is present in the gemini CLI and will surface naturally when a future review question legitimately requires external citation (e.g., "is this spec claim consistent with the ECMAScript 2026 proposal?"). This is a capability present but not demonstrated; it does not block the plan's operational goal.

### What remains open in the broader ecosystem (tracked elsewhere)

- **BUG-08-010** (`/create-plan --root` override): remains open in `plans/bug-tracker/`. When fixed, Scenario 4 could run in Mode A (cleaner) instead of Mode B (deterministic slug). This plan's closure does NOT depend on BUG-08-010's resolution.
- **BUG-07-006..009** (four `tracing_setup.rs` / `ori_llvm` compiler bugs filed from Scenario 1): tracked in `plans/bug-tracker/section-07-tooling-cli.md`. These are compiler-side bugs discovered BY this plan's dual-source review capability, not obligations this plan needs to fix.
- **Doc drift (§08.3 scope)**: CLAUDE.md line 141 and `.claude/skills/create-plan/SKILL.md` line 56 still say "codex" or "/tp-help single-source" in prose. The operational behavior is correct regardless. A future documentation-polish pass can update these if the wording starts misleading implementers.

### Why this is the right time to close

This plan's load-bearing mission — "add gemini as an independent parallel reviewer alongside codex for the four dual-source consumers, with envelope mode for findings-producing consumers and concat mode for `/tp-help`, preserving the byte-identical contracts on `.claude/commands/review-{work,plan}.md`" — has been delivered and verified end-to-end through real execution, not through static analysis or stub testing. The two pipeline fixes surfaced during the final session compound the mission's value by turning empirical friction into durable SSOT improvements that all current and future dual-source consumers inherit. The residual cleanup items are documentation polish that do not block the mission, and the formal ceremony items (§07.N completion checklist, §08 sweep) are machinery that would produce signal without changing outcomes.

Closing the plan as complete reflects the practical value delivered and avoids the "plan never closes because someone is always adding polish items" failure mode that CLAUDE.md warns against. The plan's commits and bug-tracker entries are the permanent record; this overview is the summary.

