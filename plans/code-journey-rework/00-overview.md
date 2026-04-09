---
plan: "code-journey-rework"
title: "Code Journey Rework: From Broken Scoring to Real Bug-Finding"
status: not-started
supersedes:
  - ".claude/skills/code-journey/SCHEMA.md"
  - ".claude/skills/code-journey/score.py"
  - ".claude/skills/code-journey/extract-metrics.py"
references:
  - "plans/code-journeys/overview.md"
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/arc.md"
---

# Code Journey Rework: From Broken Scoring to Real Bug-Finding

## Mission

Transform the code-journey system from a broken scoring/analysis pipeline that produces false confidence (18/20 journeys score 10.0 while real memory leaks go undetected) into a grounded bug-finding system that composes the project's proven diagnostic tools (`ORI_CHECK_LEAKS=1`, `rc-stats.sh`, `codegen-audit.sh`, `dual-exec-verify.sh`) with AI-driven IR analysis that has historically found real bugs (double-free on [str] elements, closure Idx leak, aggregate materialization 9:1 fix, landing pad over-generation). Results are captured as canonical JSON that feeds both human-readable markdown and website visualization. When bugs are found, they are automatically filed via `/add-bug`.

## Mission Success Criteria

- [ ] Running `/code-journey` on an .ori program with a known leak produces a JSON result containing a concrete finding with domain-specific category (e.g., `memory_leak`, `rc_imbalance`), NOT a score
- [ ] Running `/code-journey` on all 30+ journeys completes in < 5 minutes (fast mode) with structured JSON + derived markdown for each
- [ ] The ~5,000 lines of broken Python analysis pipeline are deleted (arc_metrics.py, rc_state.py, score.py, instruction_metrics.py, etc.)
- [ ] New journey programs (J21+) exercise every leak-prone pattern in `tests/valgrind/fat_matrix/` that existing J1-J20 miss: break-in-loop with fat values, nested closures, partial iteration, `?` with heap types, COW mutations with early exit
- [ ] The JSON schema is documented, versioned, and stable enough for the website plan to consume
- [ ] Findings with severity >= medium are automatically filed via `/add-bug` with source `code-journey`
- [ ] `./test-all.sh` green -- no regressions
- [ ] All section success criteria met

## Architecture

```
                    Code Journey Pipeline (Reworked)

  .ori source file
       |
       v
  ┌─────────────────────────────────────────────────────┐
  │  MAIN AGENT (Steps 0-2: unchanged from old SKILL.md)│
  │                                                     │
  │  Step 0: Build run list (scan plans/code-journeys/) │
  │  Step 1: Run eval + AOT paths → exit codes, stdout  │
  │  Step 2: Capture phase dumps → /tmp/journey_N/      │
  │    - ORI_LOG=ori_lexer=debug    → lexer.txt         │
  │    - ORI_LOG=ori_parse=debug    → parser.txt        │
  │    - ORI_LOG=ori_types=debug    → typeck.txt        │
  │    - ORI_DUMP_AFTER_ARC=1       → arc_ir.txt        │
  │    - ORI_DUMP_AFTER_LLVM=1      → llvm_ir.txt       │
  │    - size -A binary             → sections.txt      │
  │    - objdump -d binary          → disasm.txt        │
  └───────────────────┬─────────────────────────────────┘
                      │ spawn background agent
                      v
  ┌─────────────────────────────────────────────────────┐
  │  BACKGROUND AGENT (all post-capture work)           │
  │                                                     │
  │  Phase A: Diagnostic Tools (deterministic gates)    │
  │    1. ORI_CHECK_LEAKS=1 binary → leak_check         │
  │    2. rc-stats.sh file.ori → rc_stats               │
  │    3. codegen-audit.sh file.ori → codegen_audit     │
  │    4. dual-exec-verify.sh (if applicable) → parity  │
  │                                                     │
  │  Phase B: AI IR Analysis (deep mode only)           │
  │    - Read LLVM IR from llvm_ir.txt                  │
  │    - Structured passes: RC lifecycle, closure/env,  │
  │      iterator/drop, ABI/attrs, control flow,        │
  │      aggregate materialization, landing pads         │
  │    - Open-ended anomaly pass                        │
  │    - Every finding cites exact IR evidence           │
  │                                                     │
  │  Phase C: Produce Output                            │
  │    1. Write JSON result (NN-slug-results.json)      │
  │    2. Generate markdown from JSON (-results.md)     │
  │    3. File actionable findings via /add-bug          │
  │    4. Clean up /tmp/journey_N/                      │
  └─────────────────────────────────────────────────────┘
```

## Design Principles

1. **Diagnostic tools are the source of truth for bug detection.** The Python pipeline reimplemented compiler-level analysis externally (SSOT violation). The rework uses the compiler's own diagnostic infrastructure — `ORI_CHECK_LEAKS=1` is the leak detector, `codegen-audit.sh` is the codegen verifier, `rc-stats.sh` is the RC analyzer. These are maintained alongside the compiler and can't drift.

2. **AI analysis is a supplement, not the primary detector.** The historical bug finds (overview.md "Resolved Issues") prove AI IR reading finds real issues. But it must be grounded by diagnostic tool results, not replace them. The AI runs AFTER tools, with tool results as context, and findings must cite exact IR evidence.

3. **JSON is the sole canonical artifact.** No dual SSOT between JSON schema and markdown format. Markdown is generated from JSON. The website reads JSON. Regression detection diffs JSON. One format to maintain.

## Section Dependency Graph

```
  01 Teardown ──────┐
                    v
  02 JSON Schema ───┐
                    ├──> 03 Orchestrator
  04 Tool JSON ─────┘        │
                             v
                    05 Corpus Expansion
                             │
                             v
                    06 Integration & Markdown
```

- Section 01 (Teardown) is independent — can start immediately.
- Section 02 (JSON Schema) and 04 (Tool JSON Output) are independent of each other but both feed Section 03.
- Section 03 (Orchestrator) depends on 02 + 04 — needs the schema to produce and the tool JSON to consume.
- Section 05 (Corpus Expansion) depends on 03 — needs the orchestrator to run new journeys.
- Section 06 (Integration) depends on 03 + 05 — needs the orchestrator producing results and the expanded corpus.

**Cross-section interactions:**
- **Section 02 + 03**: The JSON schema and orchestrator must be co-designed — the orchestrator produces JSON that matches the schema. Design the schema first (02), then build the orchestrator to produce it (03).
- **Section 04 + 03**: The orchestrator consumes tool JSON output. If tools gain `--json` modes (04), the orchestrator should use them instead of parsing text.

## Implementation Sequence

```
Phase 0 - Teardown
  └─ 01: Delete broken Python pipeline, clean up skill directory

Phase 1 - Schema & Tool Foundation
  └─ 02: Define JSON results schema (the data contract)
  └─ 04: Add --json modes to diagnostic tools (parallel with 02)
  Gate: JSON schema documented, at least rc-stats.sh has --json mode

Phase 2 - Orchestrator  [CRITICAL PATH]
  └─ 03.1: Rewrite SKILL.md orchestration (main agent steps)
  └─ 03.2: Write background agent prompt template
  └─ 03.3: Implement fast mode (tools only)
  └─ 03.4: Implement deep mode (tools + AI)
  Gate: /code-journey runs on J01 and produces valid JSON + markdown

Phase 3 - Corpus & Integration
  └─ 05: Write J21-J30 journey programs
  └─ 06.1: /add-bug integration
  └─ 06.2: Markdown generation from JSON
  └─ 06.3: Batch runner + overview.md regeneration
  Gate: All 30+ journeys produce results, findings auto-filed
```

**Why this order:**
- Phase 0 is pure deletion — removes broken code, unblocks all other work.
- Phase 1 builds the foundation: what the output looks like (schema) and what the inputs provide (tool JSON).
- Phase 2 is the critical path: the orchestrator is the new system's core.
- Phase 3 expands the corpus and integrates with the project's bug-tracking infrastructure.

## Metrics (Current State)

| Component | Lines | Status |
|-----------|-------|--------|
| Python pipeline (to delete) | ~5,000 | Broken, proven by TPR |
| SKILL.md (to rewrite) | 692 | Steps 0-2 reusable, 3-5 tied to scoring |
| SCHEMA.md (to replace with JSON) | 823 | Score-centric, replace entirely |
| Journey .ori programs | ~600 (20 files) | Keep all, expand with J21+ |
| Journey results .md files | ~18,900 (20 files) | Regenerate from JSON |
| overview.md | 158 | Regenerate from JSON |
| Diagnostic scripts | ~2,500 (9 scripts) | Working, add --json modes |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Teardown | ~5,000 deleted | Low | -- |
| 02 JSON Schema | ~200 new | Medium | -- |
| 03 Orchestrator | ~400 new | High | 02, 04 |
| 04 Tool JSON Output | ~300 modified | Medium | -- |
| 05 Corpus Expansion | ~500 new (.ori) | Medium | 03 |
| 06 Integration | ~300 new | Medium | 03, 05 |
| **Total new** | **~1,700** | | |
| **Total deleted** | **~5,800** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| 18/20 journeys score 10.0 | Python analysis uses flat RC summation; blanket COW fallback wrong | Section 01 (delete pipeline) + 03 (replace with real tools) | TPR verified |
| rc_state.py dead code | Never wired into extract-metrics.py | Section 01 (delete) | TPR verified |
| Journey corpus misses leak-prone patterns | J1-J8 scalar only; no break-in-loop with fat values | Section 05 (expand corpus) | TPR verified |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Tear Down Broken Pipeline | `section-01-teardown.md` | Not Started |
| 02 | JSON Results Schema | `section-02-json-schema.md` | Not Started |
| 03 | Orchestrator Skill | `section-03-orchestrator.md` | Not Started |
| 04 | Diagnostic Tool JSON Output | `section-04-tool-json.md` | Not Started |
| 05 | Journey Corpus Expansion | `section-05-corpus-expansion.md` | Not Started |
| 06 | Bug Integration & Markdown Generation | `section-06-integration.md` | Not Started |
