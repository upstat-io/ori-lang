# /sync-docs Pre-Sweep Artifact — 2026-04-18

Snapshot of graph-derived ground-truth captured by `/sync-docs` Phase 0.6 before
running the 13-batch TPR loop. Reviewer sub-agents read the corresponding
`/tmp/sync-docs-intel/*.txt` artifacts before opening source files; this pointer
is what the Phase 3 final report references.

## Availability

- `scripts/intel-query.sh status` — `status: ok`
- Graph totals: 191,151 Symbol, 505,905 CALLS, 298,111 Issue, 11 Repos
- Ori symbol count: 32,285

## Per-crate symbol inventories

File: `/tmp/sync-docs-intel/crate-<crate>-symbols.txt`

| Crate | Indexed rows |
|---|---|
| ori_lexer_core | 35 |
| ori_lexer | 36 |
| ori_parse | 35 |
| ori_types | 37 |
| ori_canon | 36 |
| ori_patterns | 40 |
| ori_arc | 36 |
| ori_llvm | 37 |
| ori_repr | 38 |
| ori_rt | 36 |
| ori_eval | 36 |
| ori_ir | 34 |
| ori_registry | 39 |
| ori_diagnostic | 35 |
| ori_compiler | 35 |
| ori_fmt | 35 |
| ori_stack | 15 |
| ori_test_harness | 38 |
| ori_lsp | 1 |
| oric | 35 |

**Reclassification flag:** `ori_lsp` has 1 row. Either the crate is a stub or the
graph indexer skipped it. `.claude/rules/missions.md` does NOT carry a per-crate
mission for `ori_lsp` — Batch 2 objective covers this as a mission-inventory gap.

## Subsystem preset sweeps

File: `/tmp/sync-docs-intel/preset-<preset>.txt` — `--limit 10` each.

| Preset | Rows |
|---|---|
| ori-arc | 30 |
| ori-inference | 32 |
| ori-codegen | 31 |
| ori-patterns | 30 |
| ori-diagnostics | 32 |

## Plan-status snapshots

File: `/tmp/sync-docs-intel/plan-<plan>.txt`

| Plan | Rows (0 = graph has no plan-corpus data yet) |
|---|---|
| plan-bug-dag-ingestion | 4 |
| bug-tracker | 0 |
| roadmap | 0 |
| perf-engineering | 0 |
| query-intel-adoption | 0 |
| empty-container-typeck-phase-contract | 0 |
| sync-docs-redesign | 0 |
| rosetta-stress-test | 0 |

Plan-corpus ingestion (`plans/plan-bug-dag-ingestion/section-04-query-subcommands.md`)
covers only the plan actively being ingested. Absence of rows is expected and
represents graceful-degradation — batches fall back to file-based plan reading.

## Cross-repo prior-art snapshots

File: `/tmp/sync-docs-intel/similar-<subsystem>.txt`

| Subsystem | Anchor symbol | Rows |
|---|---|---|
| AIMS | `analyze_function` | 22 |
| patterns | `check_exhaustiveness` | 22 |
| codegen | `emit_rc_inc` | 14 |
| inference | `infer` | 22 |
| runtime | `ori_rc_inc` | 1 |

Reviewers drawing on these snapshots should NEVER cite a row as authority
without a `file:line` spot-check (see `compose-intel-summary.md §Step D`).

## Batch manifest

Total in-scope markdown: **1013 files**. Batches sum to exactly 1013 with zero
overlap across batches.

| Batch | File count | Manifest |
|---|---|---|
| 1 | 1 | `/tmp/sync-docs-intel/batch-01.txt` |
| 2 | 1 | `/tmp/sync-docs-intel/batch-02.txt` |
| 3 | 5 | `/tmp/sync-docs-intel/batch-03.txt` |
| 4 | 7 | `/tmp/sync-docs-intel/batch-04.txt` |
| 5 | 8 | `/tmp/sync-docs-intel/batch-05.txt` |
| 6 | 6 | `/tmp/sync-docs-intel/batch-06.txt` |
| 7 | 1 | `/tmp/sync-docs-intel/batch-07.txt` |
| 8 | 1 | `/tmp/sync-docs-intel/batch-08.txt` |
| 9 | 29 | `/tmp/sync-docs-intel/batch-09.txt` |
| 10 | 79 | `/tmp/sync-docs-intel/batch-10.txt` |
| 11 | 57 | `/tmp/sync-docs-intel/batch-11.txt` |
| 12 | 61 | `/tmp/sync-docs-intel/batch-12.txt` |
| 13 | 757 | `/tmp/sync-docs-intel/batch-13.txt` |

Batch 13 composition:
- 615 `tests/run-pass/rosetta/**/task.md` — rosetta stress-test problem specs
- 74 `compiler/ori_diagnostic/src/errors/EXXXX.md` — user-facing error-code docs
- 33 `.claude/skills/**/*.md` — skill-internal helpers (non-SKILL.md)
- 3 `.codex/skills/`, 2 `.gemini/skills/` — reviewer-agent skill docs
- 3 `blog/*.md` — published dev-blog posts
- 1 `.claude/rules/eval.md` — reclassification candidate (should be Batch 4 next run)
- 1 `CONTRIBUTING.md`, 1 `CHANGELOG.md`, 1 `docs/internal/*.md`, 1 `diagnostics/fixtures/*.md`, 2 `docs/ori_lang/...`

## Batch 9 & 12 dedup note

`docs/ori_lang/README.md` and `docs/ori_lang/v2026/modules/README.md` matched
both the Batch 9 (README*.md) glob and the Batch 12 (docs/ori_lang/**) glob.
Batch 9 wins because README-verification is more specific; Batch 12 manifest
removed them.
