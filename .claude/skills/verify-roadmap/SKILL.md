---
name: verify-roadmap-tooling
description: Programmatic cross-plan coherence checker. Two implemented modes — --quick (fast pre-check, BLOCKED + DEAD_REFERENCE only) and --full (stub; returns 'not yet implemented'). Distinct from the agent-based /verify-roadmap slash command. Item-level verification lives in the slash command today; the extracted item-verifier.md is a design note for the future 5-phase pipeline, not a currently-wired phase.
---

# Verify-Roadmap Tooling Skill

`scripts/verify_roadmap/` is the programmatic, machine-driven verification
surface for cross-plan coherence checks. It complements (it does NOT
replace) the agent-driven `/verify-roadmap` slash command.

## When this skill is invoked

- **Automatically** by `/continue-roadmap`'s `roadmap-scan.sh --verify-quick`
  flag as a pre-check before next-section selection
- **Manually** when you want a fast structural lint over the plan corpus
  without spinning up review agents

## Modes

### `--quick` (fast pre-check)

Runs ONLY the read-only DAG classifiers that don't require git activity
signals or expensive shared-subsystem analysis:

  - `BLOCKED` (a section depends on a not-yet-complete section in another plan)
  - `DEAD_REFERENCE` (a `depends_on:` entry points at a missing file/plan)

Explicitly **not** in `--quick`:
  - `CONFLICT` — requires O(N²) shared-subsystem analysis
  - `STATUS_CONTRADICTION` — requires body scanning + `WriteBackContext`
  - `SUPERSEDED` — requires reroute resolution + git activity signals
  - `MISSING_DEPENDENCY` — requires full prose scan

`--quick` mode passes `context=None` to `classify_safety` (per §03.1),
which returns `ExposureReview` for **every** finding. This is intentional:
`--quick` is a discovery surface, never a write-back trigger. It writes
no source files; it just produces the report.

Performance target: **< 5 seconds** on the full corpus (no `git log`
subprocess calls, no shared-subsystem cross-product).

### `--full` (complete sweep + auto-fix; not yet implemented)

Will run all classifiers from §§01–02, populate `WriteBackContext` from
git activity signals, run `classify_safety` with full context, and apply
auto-fixes for `SafeFix` findings via the §03.4 patcher.

For now, `--full` mode exits with code 2 and a clear "not implemented"
message. Use `--quick` for the available pre-check surface.

### Planned / aspirational modes — NOT currently supported by the CLI

The `verify-roadmap-redesign` plan envisioned three additional invocation
shapes (`--deep-all`, `--section <path>`, `--plan <name>`) that would wire
item-level verification into the programmatic pipeline. None of these
modes are supported by `scripts/verify_roadmap/__main__.py` today —
`argparse` only accepts `--quick | --full` plus the shared flags
(`--no-auto-fix`, `--dry-run`, `--quiet`, `--no-color`, `--output-dir`).

Until those phases are actually implemented, item-level verification
lives in `.claude/commands/verify-roadmap.md` (the agent-driven slash
command, ~914 lines, review + update agent protocol). Invoke that
command directly; do NOT invoke it through this skill. See
`.claude/skills/verify-roadmap/item-verifier.md` for the design note
describing how Phase 4 *would* delegate to the command's prompt
templates if/when the 5-phase pipeline ships.

## Usage

```bash
# Fast pre-check
python -m scripts.verify_roadmap --quick

# As a /continue-roadmap pre-check
.claude/skills/continue-roadmap/roadmap-scan.sh --verify-quick

# Output suppression / formatting
python -m scripts.verify_roadmap --quick --quiet         # JSON + MD only
python -m scripts.verify_roadmap --quick --no-color      # plain console output

# Custom output dir (default: build/verify-roadmap/)
python -m scripts.verify_roadmap --quick --output-dir /tmp/audit
```

## Outputs

Every run writes:

- `build/verify-roadmap/findings.json` — machine-parseable; each entry
  carries the `Finding.to_json()` dict. In `--quick` mode, entries omit
  `safety_class` and `rationale` (no classification was performed).
- `build/verify-roadmap/findings.md` — human-readable; grouped by severity
  (critical → high → medium → low). In `--full` mode, sub-grouped by
  safety class within each severity (`ExposureReview` before `SafeFix`).
- `build/verify-roadmap/fixes-applied.json` — only in `--full` mode;
  audit trail of every applied fix (finding id, file, operations,
  before/after hashes, timestamp, backup path).

These directories are not committed (build artifacts).

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Clean — no findings, no unapplied fixes |
| 1 | Findings present (low/medium/high) OR unapplied fixes |
| 2 | At least one critical finding |

## Interpretation guide

- **BLOCKED**: a roadmap or plan section can't actually be picked up
  because its `depends_on:` chain leads back to incomplete work in
  another plan. Resolve by completing the dependency, or by
  re-evaluating whether the dependency is still required.
- **DEAD_REFERENCE**: a `depends_on:` entry, prose reference, or
  HTML-comment annotation points at a path or plan name that no longer
  exists. Resolve by removing the dead entry (`depends_on` removals are
  `SafeFix` and would auto-fix in `--full` mode) or by replacing it
  with the correct target (prose / HTML-comment refs are
  `ExposureReview` — human authorship needed).

## When to invoke `--quick` vs `--full`

| Scenario | Mode |
|----------|------|
| `/continue-roadmap` pre-check | `--quick` (auto via `--verify-quick`) |
| Major milestone / pre-merge audit | `--full` (when implemented) |
| Manual ad-hoc check, "is anything obviously broken?" | `--quick` |
| Debugging a specific BLOCKED report | `--full` (when implemented) — provides full classifier output |

## Architecture

| Module | Role |
|--------|------|
| `scripts/verify_roadmap/safety.py` | Safety taxonomy — `SafetyClass`, `ClassifiedFinding`, `WriteBackContext`, `PreimageRecord`, `FmOperation`, `PatchResult`, `classify_safety` |
| `scripts/verify_roadmap/report.py` | Report renderers — JSON, markdown, console; `Report`, `ReportMode`, exit codes |
| `scripts/verify_roadmap/auto_fix.py` | Auto-fix dispatcher + applier — `build_fix_plan`, `apply_fixes`, defense-in-depth invariants |
| `scripts/verify_roadmap/patcher.py` | Frontmatter text patcher — atomic writes, concurrent-session SHA256 guard, comment-preserving regex ops |
| `scripts/verify_roadmap/quick.py` | `--quick` mode runner — DAG-only classifiers, no git, all-ExposureReview |
| `scripts/verify_roadmap/__main__.py` | CLI entry point |

`plan_corpus` produces factual `Finding` records; `verify_roadmap`
classifies and acts on them. The dependency direction is one-way:
`plan_corpus` MUST NEVER import from `verify_roadmap`.

## Related

- `plans/verify-roadmap-redesign/` — the design plan for this tooling
- `.claude/commands/verify-roadmap.md` — agent-driven slash command
  (different concern; verifies *content* of roadmap sections)
- `scripts/plan_corpus/` — corpus SSOT (parser, schemas, DAG builder)
- `.claude/skills/continue-roadmap/roadmap-scan.sh` — pre-check
  integration via `--verify-quick`
