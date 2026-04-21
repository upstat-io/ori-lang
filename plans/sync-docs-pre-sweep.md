# /sync-docs Pre-Sweep — 2026-04-21

- Worktree base: 9d9a6261 (2026-04-17 — 4 days of dev drift)
- Graph availability: `ok` (32407 ori symbols, 505K CALLS edges, 298K issues, 11 repos)
- Snapshot gaps (see `/tmp/sync-docs-intel/snapshot-gaps.txt`):
  - `.claude/rules/missions.md`: ABSENT → skip Batch 2; Tier-A fallback elsewhere
  - `scripts/prose-lint.py`: ABSENT → fallback grep
  - `.claude/skills/tpr-review/compose-round-prompt.md`: ABSENT
  - `.claude/skills/tpr-review/gemini-depth-appendix.md`: ABSENT

## Phase 0.7 focus

- Day-of-year: 111 → index 3 → codegen / LLVM
- Tier-A source (degraded — missions.md absent): `.claude/rules/codegen-rules.md`, `llvm.md`, `aot.md`, `repr.md`
- Primary code home: `compiler/ori_llvm/`, `compiler/ori_repr/`

## Artifact inventory under `/tmp/sync-docs-intel/`

- `ori_<crate>-symbols.txt` — 18 per-crate `file-symbols` dumps
- `preset-ori-<subsystem>.txt` — 5 subsystem presets (arc, inference, codegen, patterns, diagnostics)
- `similar-aims.txt`, `similar-patterns.txt`, `similar-codegen.txt` — cross-repo prior-art snapshots
- `snapshot-gaps.txt` — Phase 0.5 gap triage record

## In-scope files

- 1246 total (filtered by banned-paths in Phase 0.5)
- Top sources: `tests/` (638, mostly Rosetta task.md + plan-audit fixtures — Batch 14 + reclassification candidates), `plans/` (258), `docs/` (167), `.claude/` (90), `compiler/` (78)

## Contract

- Batches commit via `/commit-push` (TRIVIAL/crossref tag for mechanical pre-sweep commit)
- /tpr-review always invoked with `--autonomous`
- NEVER fetch/rebase/merge; dev drift is expected
