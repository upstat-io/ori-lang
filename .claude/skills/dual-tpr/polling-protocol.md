# Dual-Source Review Polling Protocol — Canonical SSOT

**Single source of truth** for polling cadence, wall-clock anchoring, and countdown visibility across ALL dual-source review skills: `/tpr-review`, `/review-work`, `/tp-help`, and any future consumer that launches `dual-invoke.sh` (or the retry wrapper) in the background. Every such skill MUST reference this file from its polling section instead of inlining the protocol text. Updates to polling behavior land HERE, then propagate to consumers via `@`-include (not copy).

This file replaces the pre-2026-04-08 pattern where each skill inlined its own copy of the polling instructions. Three near-identical copies had already started to drift (tpr-review and review-work used identical text; tp-help had slight wording drift) — a textbook `impl-hygiene.md` §SSOT violation. Consolidation here is the SSOT fix.

## Why this exists

After launching a background dual-source transport (codex + gemini in parallel), the operator needs real-time visibility into reviewer state for the entire wait — which can span anywhere from ~20 seconds (trivial smoke-test prompts) up to ~45 minutes (deep code-review prompts where gemini reads many files). Two constraints are load-bearing and non-negotiable:

1. **Visible heartbeats** — the operator MUST see regular output during the wait, not a silent period followed by a completion notification. The prior pattern used `sleep 300 && status-check.sh` with `run_in_background: true`, which produced **zero visible output for the entire 5-minute sleep**. Even worse, foreground Bash calls with `sleep 300` get auto-backgrounded at the 2-minute default timeout, with the same no-visibility result. The fix is SHORT FOREGROUND polls whose stdout lands in the conversation within the Bash foreground timeout window.
2. **Absolute wall-clock anchors** — every status update MUST include an absolute wall-clock timestamp (`HH:MM:SS TZ`), not relative "T+N min" style. Relative timestamps are unusable because the operator has no anchor for T=0 unless Claude recorded it and echoed it back. Absolute wall-clock is always interpretable.

Both constraints were surfaced empirically during `plans/dual-tpr-gemini` §07.3 Scenario 1 execution on 2026-04-08: the prior protocol produced 5-minute silent periods with relative-only timestamps, and the operator reported zero real-time visibility. This file is the fix.

## Mandatory Protocol

### Step A — At launch (immediately after starting `dual-invoke.sh` in the background)

Capture the launch wall-clock time BOTH on stdout (operator-visible heartbeat) AND in `$RUN/launch.time` (artifact for later analysis):

```
Bash (foreground, default timeout):
  date "+LAUNCH %Y-%m-%d %H:%M:%S %Z" | tee "$RUN/launch.time"
```

This is the "T=0" anchor. Every subsequent status update references this time. The `tee` is load-bearing — printing to stdout alone doesn't create the artifact; writing to the file alone doesn't give the operator a heartbeat. Do BOTH.

### Step B — Polling loop (repeat until the background-task completion notification arrives)

Each poll MUST be a SINGLE FOREGROUND Bash call that sleeps briefly, then runs `status-check.sh`. The foreground-with-sleep pattern is load-bearing: background polls don't stream output, so the operator sees nothing until the poll ends, defeating the visibility goal. Foreground polls return their stdout to Claude as soon as the call completes, and Claude immediately surfaces it to the operator with a brief commentary.

```
Bash (foreground, default 120s Bash timeout):
  date "+WALLCLOCK %H:%M:%S %Z (sleeping 75s)" \
    && sleep 75 \
    && date "+WALLCLOCK %H:%M:%S %Z (polling)" \
    && .claude/skills/dual-tpr/scripts/status-check.sh "$RUN" --events 5
```

**Cadence target**: **~75-second intervals**. This produces ~10-20 status updates over a typical 15-25 min dual-source run — enough for real-time visibility without overwhelming the transcript.

**Allowed range**: 30s to 90s sleeps.
- **< 30s**: noise; the reviewers don't produce meaningful new events that fast.
- **> 90s**: cuts too close to the 120s default Bash foreground timeout and risks auto-backgrounding, which reintroduces the silent-period problem.

**Per-poll commentary requirement**: after each poll returns, Claude MUST surface the output to the operator WITH a brief explanation of what changed since the last poll (new events, changed walltime, reviewer completion status). A poll that returns without commentary is a missed visibility opportunity.

### Step C — Stopping condition

Stop polling when the background-task completion notification arrives. The notification's reported exit code is **authoritative** — do NOT infer success from `status-check.sh` output alone. `status-check.sh` reads append-only streams (`round.log`, `codex.jsonl`, `gemini.jsonl`); it does NOT read the final atomic-write artifacts (`$RUN/{reviewer}.exit`, `$RUN/{reviewer}.envelope.json`, `$RUN/{reviewer}.walltime`) because those can be empty or partially written mid-stream. Only the task-completion notification guarantees those atomic-write files are fully written.

## Normal reviewer timing — what is NOT a stall

Polls must distinguish "reviewer is loading / thinking" from "reviewer is actually stuck." Misreading the first as the second wastes time and produces false-alarm escalations (terminating a live run, relaunching into a new scratch dir, asking the operator for guidance on a non-problem). The two reviewers have markedly different startup and thinking patterns — judge each by its own baseline.

**Codex baseline**:
- Begins streaming `item.started command_execution` events within 5-15 seconds of launch.
- Reaches 50-100 events within the first 2-3 minutes on a substantive prompt.
- May sit idle for 1-3 minutes between the last tool event and the final `agent_message` / `turn.completed` while composing the answer. This is the **thinking phase** — normal, not a stall.
- Total wall-time: 3-10 minutes on focused empirical questions, 10-15 minutes on deep multi-question prompts.

**Gemini baseline**:
- May have a **long cold-start** where `status-check.sh` shows ONLY the initial two events (`[init]` and `[msg.user]`) with no `tool_use` / `tool_result` activity for **up to 8-10 minutes**. This is NOT a failure, NOT a stall, and NOT a prompt-discipline violation — gemini is loading context and planning before it begins reading files. Gemini is substantially slower than codex in most cases; its cold-start alone can exceed codex's total wall-time. Empirically observed during the TPR-07-022 Q5 follow-up round on 2026-04-08: gemini stayed at 2 events / 5877 bytes for 4.5 minutes, then accelerated to 14 events in the next ~90 seconds and produced a full answer shortly after. Subsequent observations show cold-starts routinely reaching 6-8 minutes on complex prompts.
- Once it starts producing `tool_use` events, it usually finishes within 3-8 additional minutes.
- Total wall-time: ~3-8 minutes on focused prompts, 15-30 minutes on deep prompts. The 179-second fast-finish in TPR-07-022 round 1 is the lower bound, not the typical case.

**How to distinguish a real stall from normal cold-start or thinking**:
- **Codex stalled**: byte count AND event count BOTH unchanged for >5 minutes with no `turn.completed`.
- **Gemini stalled**: stuck at the initial `[init]` + `[msg.user]` pair for >14 minutes with zero `tool_use` events. Below 14 minutes, assume gemini is still warming up — keep polling. Gemini is substantially slower than codex; its cold-start can legitimately take 8-10 minutes on complex prompts.
- **Either reviewer actually failed**: `rc=1` in `round.log` with `walltime < 30s` = early API / transport failure, relaunch needed. `rc=1` with longer walltime usually means prompt discipline violation (gemini tried to write) or a codex sandbox error — inspect the events tail.

When in doubt: poll once more. 75-second cadence is cheap; premature relaunch is expensive (re-does all the file reading the reviewer already invested in).

## Banned Patterns

The following patterns are explicitly banned because they reintroduce the problems this protocol exists to prevent:

- **`run_in_background: true` on polling calls** — the operator won't see output until the poll completes, defeating the visibility goal. ONLY the primary `dual-invoke.sh` launch uses `run_in_background: true`; polls are ALWAYS foreground.
- **`sleep 300` or any sleep ≥ 120 seconds** — exceeds the default 120s Bash foreground timeout, causes auto-backgrounding. Use 30-90s sleeps per Step B.
- **Relative "T+N min" timestamps without an absolute wall-clock anchor** — the operator has no reference. Every status update includes absolute `HH:MM:SS TZ`.
- **Skipping the `$RUN/launch.time` artifact** — without it, post-run analysis has no anchor for when the transport started. Step A is mandatory.
- **Reading atomic-write files during a poll** — `$RUN/{reviewer}.exit`, `.walltime`, `.envelope.json`, `.parse-error` can be empty or partially written mid-stream. `status-check.sh` handles these safely via existence checks. Consumers outside `status-check.sh` MUST NOT read these files until the completion notification arrives.
- **Silent waiting between polls** — if Claude is "waiting" without producing any output, the operator has no visibility. The polling loop IS the visibility mechanism; silence between polls defeats it.

## Safe to Read During a Poll

These files are append-only or immutable-after-initial-write, so reading them during a poll is safe:

- `$RUN/round.log` — append-only orchestration log
- `$RUN/codex.jsonl` — append-only codex event stream
- `$RUN/gemini.jsonl` — append-only gemini event stream
- `$RUN/worktree.before` — immutable after initial snapshot (written before `dual-invoke.sh` starts)
- `$RUN/launch.time` — immutable after initial capture (written in Step A)

## Unsafe to Read Mid-Poll (Atomic-Write Race)

`status-check.sh` handles these safely via **existence checks only** (never reads contents mid-stream). Consumers outside `status-check.sh` MUST follow the same discipline:

- `$RUN/{reviewer}.exit` — written atomically at subshell exit
- `$RUN/{reviewer}.walltime` — written atomically at subshell exit
- `$RUN/{reviewer}.envelope.json` — written atomically at parser exit (envelope-mode consumers only)
- `$RUN/{reviewer}.parse-error` — written atomically at parser exit
- `$RUN/{reviewer}.skipped` — written atomically at `dual-invoke.sh` filter time

## Enforcement

Consumers reference this file via `@.claude/skills/dual-tpr/polling-protocol.md` at their polling section. The `@`-include pattern causes the harness to splice this file's content into the skill prompt at expansion time — no manual Read calls required, and the content is automatically in sync across all consumers.

If a consumer's polling section inlines the protocol text instead of `@`-including this file, that is an **SSOT violation** (`impl-hygiene.md` §Algorithmic DRY → `LEAK:algorithmic-duplication`) and must be fixed by replacing the inlined text with the `@`-include reference.

## Related

- Memory: `feedback_dual_tpr_polling.md` — operator-facing rule on why polling matters and what "going silent" looks like
- Script: `.claude/skills/dual-tpr/scripts/status-check.sh` — the read-only status helper invoked by every poll
- Launcher: `.claude/skills/dual-tpr/scripts/dual-invoke.sh` — the background transport this protocol polls against
- Surfacing: `plans/dual-tpr-gemini` §07.3 Scenario 1 (2026-04-08) — empirical surfacing of the pre-fix protocol's visibility deficiencies
