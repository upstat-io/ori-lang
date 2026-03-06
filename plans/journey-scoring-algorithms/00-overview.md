---
plan: "journey-scoring-algorithms"
title: "Code Journey Scoring Algorithms: Deterministic Metric Extraction"
status: complete
references:
  - ".claude/skills/code-journey/SKILL.md"
  - ".claude/skills/code-journey/SCHEMA.md"
  - ".claude/skills/code-journey/score.py"
---

# Code Journey Scoring Algorithms: Deterministic Metric Extraction

## Mission

Eliminate AI judgment from the code journey scoring pipeline by building deterministic algorithms that extract every metric directly from compiler artifacts (LLVM IR, binary, trace output). The AI's job becomes *narrative analysis and finding discovery* — the numbers come from scripts.

## Problem Statement

The current scoring system has two layers:

1. **`score.py`** — maps metrics to scores via strict threshold tables. Deterministic, works correctly.
2. **Metric extraction** — done by the AI reading the LLVM IR. Subjective, variable, unreliable.

The vulnerability is in layer 2. On 2026-03-05, the AI scored Journey 1 at 9.7/10 (defining "ideal" as the best IR *including* overflow checking, ratio 1.00x). On 2026-03-06, a re-run scored the same compiler output at 8.9/10 (defining "ideal" as *no* overflow checking, ratio 3.50x). The compiler actually *improved* between runs (added `noreturn` to `ori_panic_cstr`), but the score went down because the AI changed its interpretation.

**Root cause**: the instruction ratio metric (`actual/ideal`) depends on a subjective definition of "ideal instruction count." No amount of documentation prevents drift — the AI reinterprets on every run.

## Architecture

```
                        CURRENT (broken)
                        ================
  LLVM IR ──> [AI reads IR, counts things] ──> metrics ──> score.py ──> scores
                  ↑ subjective, variable

                        TARGET (deterministic)
                        ======================
  LLVM IR ──> [extract-metrics.py parses IR] ──> metrics.json ──> score.py ──> scores
  Binary  ──>          ↑ deterministic              ↑                 ↑
  Traces  ──>          |                    AI reads metrics    AI writes
                       |                    for narrative       narrative
                 No AI judgment
                 in the numbers
```

## Design Principles

1. **Sonar model: algorithms over judgment.** SonarQube doesn't ask "is this complex?" — it computes cyclomatic complexity with a formula. Every metric must have a deterministic algorithm that produces the same output given the same input, regardless of who runs it.

2. **Justified overhead is part of ideal.** Ori mandates overflow checking on all integer arithmetic. The "ideal" IR for `a + b` is the 7-instruction overflow-checked pattern, not the 2-instruction unchecked `add + ret`. The algorithm must classify overflow checking, panic paths, and ABI compliance as part of the ideal — not as overhead above it.

3. **AI narrates, scripts score.** The AI retains full control over the qualitative analysis: prose descriptions, finding discovery, cross-journey observations, educational content. But every number in the `score_metrics` frontmatter and the `## Codegen Quality Score` table must come from a script.

## Section Dependency Graph

```
  01-ir-parser ──────────────────────────┐
       │                                 │
       ├── ir_utils.py (shared)          │
       │     ├── 02-instruction-metrics  │
       │     └── 05-control-flow-metrics │
       │                                 │
       ├── 03-arc-metrics                │
       │                                 │
       └── 04-attribute-metrics          │
                                         │
  06-binary-metrics (independent) ───────┤
                                         │
  07-integration ────────────────────────┘
       │
  08-verification
```

- Section 01 (IR parser) is the foundation — sections 02-05 depend on it.
- Sections 02-05 are independent of each other and can be developed in parallel after 01.
- **Shared code:** Sections 02 and 05 share `ir_utils.py` for redundant branch detection and trivial phi detection. This module depends on 01's data model but contains no circular dependencies.
- Section 06 (binary metrics) is independent of all other sections and can be developed in parallel with 01.
- Section 07 (integration) wires everything together into the pipeline.
- Section 08 (verification) validates the complete system.

## Implementation Sequence

```
Phase 0 - Foundation
  └─ 01: LLVM IR parser (function extraction, instruction counting)

Phase 1a - Shared Utilities
  └─ ir_utils.py: shared detection functions (redundant branch, trivial phi)

Phase 1b - Metric Extractors (parallelizable after 1a)
  ├─ 02: Instruction efficiency metrics (ideal computation, ratio)
  ├─ 03: ARC metrics (RC op counting, violation detection)
  ├─ 04: Attribute metrics (presence checking, compliance)
  ├─ 05: Control flow metrics (block analysis, defect detection)
  └─ 06: Binary metrics (section sizes, disassembly parsing)
  Gate: each extractor produces correct output on Journey 1's known IR

Phase 2 - Integration
  └─ 07: Unified extract-metrics.py + score.py pipeline
  Gate: `extract-metrics.py journey_ir.txt | score.py` reproduces Journey 1's correct score

Phase 3 - Verification
  └─ 08: Test suite, re-run all journeys, SKILL.md update
  Gate: all existing journeys re-scored deterministically, SKILL.md updated
```

**Why this order:**
- Phase 0 must come first because all extractors parse the same IR format.
- Phase 1 extractors are independent — each reads the parsed IR and computes one dimension.
- Phase 2 wires them into a single pipeline invocation.
- Phase 3 proves the system works end-to-end and updates the skill definition.

### Circular Dependency Check

**No circular dependencies exist in this design.** The dependency graph is a strict DAG:

```
ir_parser.py          ← no dependencies (foundation)
ir_utils.py           ← depends on ir_parser (data model only)
instruction_metrics.py ← depends on ir_parser + ir_utils
arc_metrics.py        ← depends on ir_parser only
attribute_metrics.py  ← depends on ir_parser only
control_flow_metrics.py ← depends on ir_parser + ir_utils
binary_metrics.py     ← no dependencies (independent)
extract-metrics.py    ← depends on all of the above (integration)
```

**Potential concern:** `instruction_metrics.py` (section 02) and `control_flow_metrics.py` (section 05) share `ir_utils.py` and also share the concept of "unjustified instructions" (section 02) vs "control flow defects" (section 05). This is NOT a circular dependency -- the overlap is intentional (same IR pattern, different scoring dimensions). The shared detection code lives in `ir_utils.py` (owned by neither), and each consumer applies its own scoring logic independently.

### The 7th Dimension: "Other Findings"

`score.py` has 7 scoring dimensions, but only 6 are algorithmically extractable. The 7th — "Other Findings" (15% weight) — captures journey-specific discoveries from categories 8+ that don't map to instruction counts, ARC violations, attributes, control flow, IR quality, or binary defects. This dimension remains AI-determined by design. `extract-metrics.py` outputs `null` for `other_critical/high/low` to signal that the background agent must supply these values. If the agent does not override them, `score.py` defaults to 0 (no penalty).

### Python Version Requirement

`extract-metrics.py` and all metric scripts require **Python 3.10+** for union type syntax (`str | None`), `match` statements, and `dataclass` features. Document this in the script's `#!/usr/bin/env python3` header and in SKILL.md.

### Test File Conventions

These are Python scripts, not Rust crates, so the sibling `tests.rs` convention does not apply. Instead:

- **Test directory:** `.claude/skills/code-journey/tests/` (a `tests/` subdirectory within the skill folder)
- **One test file per module:** `tests/test_ir_parser.py`, `tests/test_instruction_metrics.py`, etc.
- **Golden fixtures:** `tests/golden/journey1_ir.txt`, `tests/golden/journey1_metrics.json`
- **Run tests:** `cd .claude/skills/code-journey && python3 -m pytest tests/`
- **No `__init__.py` needed** if using `pytest` (auto-discovers test files)
- Test files do NOT count toward the 500-line file size limit (matching the Rust convention)

### Location Rationale

All scripts live in `.claude/skills/code-journey/` because:
1. They are tooling FOR the code-journey skill, used by the background agent
2. They sit alongside `score.py`, `SKILL.md`, and `SCHEMA.md` (the existing skill assets)
3. They are not compiler code and do not belong in `compiler/`, `diagnostics/`, or `scripts/`
4. The `.claude/skills/` directory is the standard location for skill-specific tooling

## Estimated Effort

| Section | Est. Lines (code) | Est. Lines (tests) | Complexity | Depends On |
|---------|-------------------|--------------------| -----------|------------|
| 01 IR Parser | ~250-300 | ~200 | **High** | — |
| 02 Instruction Metrics | ~150-200 | ~150 | Medium-High | 01, ir_utils |
| 03 ARC Metrics | ~100-120 | ~100 | Medium | 01 |
| 04 Attribute Metrics | ~150-180 | ~120 | Medium | 01 |
| 05 Control Flow Metrics | ~120-150 | ~100 | Medium | 01, ir_utils |
| 06 Binary Metrics | ~80-100 | ~80 | Low | — |
| 07 Integration (`extract-metrics.py`) | ~150-200 | ~100 | Medium | 01-06 |
| 08 Verification | ~120-150 | ~150 | Medium | 07 |
| Shared utilities (`ir_utils.py`) | ~50-80 | (tested via 02/05) | Low | 01 |
| **Total new** | **~1170-1380** | **~1000** | | |

> **WARNING (file size limit):** The 500-line limit from `impl-hygiene.md` applies even to Python scripts in this project. Section 01 (IR Parser) is the highest risk -- if the regex-based parser needs extensive edge-case handling (multi-line instructions, metadata nodes, debug info, comdat groups), it may exceed 500 lines. **Mitigation:** Split `ir_parser.py` proactively at ~400 lines into `ir_parser.py` (data model + entry point) and `ir_parser_core.py` (regex parsing logic).

**Why estimates increased from ~990:**
- Original estimates omitted test files entirely (each module needs a dedicated test file)
- IR parser complexity was underestimated -- LLVM IR has many syntactic forms the plan does not account for (metadata `!dbg`, comdat, alignment annotations, addrspace, vector types)
- Shared `ir_utils.py` was missing (sections 02 and 05 share redundant branch detection)
- Error handling in IR parser (+50 lines for empty/malformed input handling)
- Attribute rules expanded (RC memory functions, noalias) and attribute group auditing
- Migration strategy in integration (rescore script, IR extraction from markdown)
- Verification expanded (rescore-all.sh, discrepancy documentation)

## Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| **LLVM IR regex parsing breaks on edge cases** (metadata, vector types, multi-line phi) | High | Start with Journey 1's simple IR. Add patterns incrementally per-journey. Each new edge case becomes a golden test. |
| **Section 01 exceeds 500 lines** (many LLVM IR forms to handle) | Medium | Proactively split at 400 lines into `ir_parser.py` (data model) and `ir_parser_core.py` (parsing logic). |
| **Per-function RC balance is too coarse** (false positives on ownership transfers) | Medium | Document known false positive patterns. Add module-level balance check as complementary metric. |
| **`is_unnecessary_alloca_chain()` requires use-def analysis** | Medium | Start with conservative same-block-only heuristic. Full cross-block analysis can be added later. |
| **Compiler codegen changes invalidate golden files** | Low | Golden files track compiler output, not algorithm correctness. Update golden files when compiler improves. Tests verify script logic is unchanged. |
| **Python 3.10+ not available on some systems** | Low | Document requirement in script headers and SKILL.md. CI/testing environment already has Python 3.11+. |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | LLVM IR Parser | `section-01-ir-parser.md` | Complete |
| 02 | Instruction Efficiency Metrics | `section-02-instruction-metrics.md` | Complete |
| 03 | ARC Metrics | `section-03-arc-metrics.md` | Complete |
| 04 | Attribute Metrics | `section-04-attribute-metrics.md` | Complete |
| 05 | Control Flow Metrics | `section-05-control-flow-metrics.md` | Complete |
| 06 | Binary Metrics | `section-06-binary-metrics.md` | Complete |
| 07 | Integration | `section-07-integration.md` | Complete |
| 08 | Verification | `section-08-verification.md` | Complete |
