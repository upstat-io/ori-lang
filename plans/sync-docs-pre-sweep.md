---
sync_run: 2026-04-19
status: phase-0.6-complete
---

# /sync-docs Pre-Sweep Artifact — 2026-04-19

Graph ground-truth map produced by Phase 0.6. Every batch reads this before opening source.

## Availability

- `scripts/intel-query.sh status`: `ok`
- Neo4j 5.26.24
- Ori symbols: 32,348 | CALLS edges: 506,482
- Reference repos: 10 (elm, gleam, go, koka, lean4, roc, rust, swift, typescript, zig)

## Scratch root

`/tmp/sync-docs-intel/`

## Per-crate symbol inventories (`<crate>-symbols.txt`)

| Crate | Lines | Scope |
|---|---|---|
| ori_arc | 30 | limit 30 sample |
| ori_canon | 166 | limit 150 extended |
| ori_compiler | 35 | limit 30 sample |
| ori_diagnostic | 162 | limit 150 extended |
| ori_eval | 163 | limit 150 extended |
| ori_fmt | 35 | limit 30 sample |
| ori_ir | 163 | limit 150 extended |
| ori_lexer | 163 | limit 150 extended |
| ori_llvm | 167 | limit 150 extended |
| ori_parse | 164 | limit 150 extended |
| ori_patterns | 171 | limit 150 extended |
| ori_registry | 169 | limit 150 extended |
| ori_repr | 38 | limit 30 sample |
| ori_rt | 36 | limit 30 sample |
| ori_stack | 15 | full (small crate) |
| ori_test_harness | 38 | limit 30 sample |
| ori_types | 162 | limit 150 extended |
| oric | 165 | limit 150 extended |

High-priority crates (Batches 3-5) extended to 150; low-churn crates (runtime, repr, fmt, test harness, stack, compiler facade) kept at 30.

## Subsystem presets (`preset-<name>.txt`)

| Preset | Lines |
|---|---|
| ori-arc | 30 |
| ori-inference | 32 |
| ori-codegen | 31 |
| ori-patterns | 30 |
| ori-diagnostics | 32 |

## Cross-repo similarity (`similar-<subsystem>.txt`)

| Subsystem | Seed symbol | Lines | Notes |
|---|---|---|---|
| aims | `analyze_function` | 22 | 3 repos hit (rust, swift, koka) |
| patterns | `check_exhaustiveness` | 22 | 2 repos hit (rust, swift) |
| codegen | `emit_rc_inc` | 14 | 2 repos hit (rust, swift) |
| eval | `eval_expr` | 18 | 2 repos hit (rust, swift) |
| inference | `resolve_primitive_name` | 1 | no embedding — skipped |
| runtime | (prior artifact — see scratch) | 1 | empty result |

Inference and runtime have no cross-repo similar hits on the seed symbols we probed. Batch 3 and Batch 4 reviewers should run `similar` on different seed symbols as needed.

## Plan-status snapshots (`plan-<slug>.txt`)

| Plan | Sections | Open | Blockers | Bugs |
|---|---|---|---|---|
| Query-Intel Adoption | 9 | 4 | 0 | 0 |
| Empty-Container Typeck | (see file) | | | |
| Iter Ownership | (see file) | | | |
| Sync Docs Redesign | (see file) | | | |
| Rosetta Stress | (see file) | | | |

## Phase 0.7 focus

- `day-of-year 109 mod 12 = 1` → **typeck / inference**
- Mission: `.claude/rules/missions.md §ori_types`
- Rules: `typeck.md`, `types.md`
- Code home: `compiler/ori_types/`

## Usage contract

Every batch's `/tpr-review --autonomous` objective passes this artifact path as context. Reviewers read `/tmp/sync-docs-intel/<relevant>.txt` BEFORE opening source, and cite graph results with representative `file:line` spot-checks per the shared GRAPH-FIRST PROTOCOL.
