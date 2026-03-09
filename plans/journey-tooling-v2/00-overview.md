---
plan: "journey-tooling-v2"
title: "Journey Tooling V2: Eliminating False Positives and Adopting Compiler-Grade Verification"
status: resolved
references:
  - "plans/journey-scoring-algorithms/"
  - "plans/code-journeys/overview.md"
  - ".claude/skills/code-journey/"
  - "compiler/ori_llvm/src/verify/"
  - "docs/compiler/design/10-llvm-backend/codegen-verification.md"
---

# Journey Tooling V2: Eliminating False Positives and Adopting Compiler-Grade Verification

## Mission

Upgrade the code journey scoring pipeline from naive counting to compiler-grade verification by adopting proven patterns from Swift's ARC optimizer, Lean 4's IR Checker, Clang's RetainCountChecker, and Rust's modular pass design. The journey approach is the right approach — the tooling needs to catch up. All tools remain in Python for now (eventual Ori rewrite planned).

## Problem Statement

The `journey-scoring-algorithms` plan (complete) successfully eliminated AI judgment from metric extraction. But the algorithms themselves produce false positives in three systematic categories:

| Category | Affected Journeys | Root Cause | Severity |
|----------|-------------------|------------|----------|
| **Opaque construction** | J9 (strings), J10 (lists) | `ori_str_from_raw`/`ori_list_alloc_data` allocate RC-managed buffers internally, invisible to IR scanner | 9-15 false violations |
| **Cross-function ownership** | J5 (closures) | Env allocated in one function, consumed in another — per-function balance can't track this | 14 false violations |
| **Conditional RC paths** | J9 (SSO gate) | `rc_dec` behind SSO/null guard — raw counts overcount | 5 false CF defects |

**Impact**: J5 dropped from 9.3 to 6.7, J10 from 7.2 to 5.2. These scores are _worse_ than the AI-judged scores they replaced, and for the wrong reasons.

**Reference implementations**: Production compilers don't do naive counting:
- **Swift** (`lib/SILOptimizer/ARC/`): `RefCountState` with `KnownSafe` flag, bidirectional CFG dataflow, `RCIdentityAnalysis` (in `include/swift/SILOptimizer/Analysis/`) for tracing RC identity through projections
- **Lean 4** (`Compiler/IR/Checker.lean`): Structural IR verification — validates variable scope, type consistency, jump validity before codegen
- **Clang** (`RetainCountChecker`): Function effect summaries mapping runtime functions to their RC effects (+1 retained, -1 consumed, 0 borrowed)
- **Rust** (`rustc_mir_transform/src/check_*.rs`): Modular per-issue verification passes

## Architecture

```
                    CURRENT (false positives)
                    =========================
  LLVM IR ──> [regex counting per function] ──> metrics.json ──> score.py
                 ↑ no runtime knowledge
                 ↑ no CFG awareness
                 ↑ no cross-function tracking

                    TARGET (compiler-grade)
                    ========================
  LLVM IR ──> [effect-aware parser] ──> [CFG-aware balance] ──> metrics.json
    │              ↑                        ↑
    │         effect_summaries.py      per-SSA-value state
    │         (runtime fn → RC effect)  through branches
    │
    ├── [ownership tracker] ──> cross-function balance
    │        ↑
    │   caller/callee ownership pairs
    │
    └── [attribute checker] ──> attribute compliance
             ↑
        closure-aware rules

  ARC IR ──> [structural verifier] ──> pre-codegen validation (Rust, in ori_arc)
                 ↑
            Lean 4 Checker pattern
```

## Design Principles

1. **Effect summaries over naive counting.** Clang's `RetainCountChecker` solved this exact problem: runtime functions are opaque to the scanner, so you annotate them with their RC effects. A table mapping `ori_str_from_raw → returns +1` eliminates 90% of false positives with ~50 lines of code. This is Tier 1 priority.

2. **Verify at the highest useful level.** Lean 4 verifies at their LCNF IR level (before C/LLVM lowering) because that's where all information is still present. Our ARC IR has `Construct`/`RcInc`/`RcDec` instructions with full type info — verifying there catches bugs that are invisible at the LLVM IR level. The Python tools should remain for LLVM IR quality scoring; the ARC IR verifier is a separate Rust pass.

3. **Modular per-issue passes.** Rust's `rustc_mir_transform` has `check_alignment.rs`, `check_pointers.rs`, etc. — each handles one concern. Our verification should follow the same pattern: effect summaries, CFG balance, ownership tracking, and attribute compliance are independent modules with no coupling.

## Section Dependency Graph

```
  01-effect-summaries ──────────────────────┐
       │ (required by 02, 03)               │
       ├── 02-cfg-rc-balance ←──────┐       │
       │                            │       │
       └── 03-cross-function-ownership      │
                                    │       │
  04-ir-parser-hardening ───────────┘ ──────┤
       (invoke handling needed by 02)       │
                                            │
  05-arc-ir-verification (independent) ─────┤  ← Rust, not Python
                                            │
  06-attribute-compliance (independent) ────┤
                                            │
  07-integration ───────────────────────────┘
```

- Section 01 (effect summaries) is the foundation -- sections 02 and 03 build on it.
- Sections 04, 05, 06 are independent of each other. Section 04 has no upstream dependencies, but **Section 02 depends on Section 04** because CFG construction requires `invoke` target extraction (a Section 04 fix). If implementing in parallel, extract the `invoke` regex fix into shared code first.
- Section 05 is independent of all Python tooling sections (it is a Rust pass in `ori_arc`).
- Section 07 wires everything together and re-scores all 12 journeys.

## Implementation Sequence

```

Phase 0 - Quick Wins (immediate false positive reduction)
  └─ 04.0: Split ir_parser.py (503 lines → two files ≤400 lines each)
  └─ 01: Runtime function effect summary table
  └─ 04: IR parser quoted name fix + invoke handling (unblocks J8 generics)
  └─ Pre-req: Fix _RC_DEC_RE to include ori_map_buffer_rc_dec,
     ori_set_buffer_rc_dec, ori_set_buffer_drop_unique (trivial regex fix)
  Gate: J9 arc_violations drops from 9 to ≤2, J8 parses all functions

Phase 1 - Structural Improvements (parallelizable)
  ├─ 02: CFG-aware RC balance (per-SSA-value through branches)
  ├─ 03: Cross-function ownership (caller/callee pairs)
  └─ 06: Attribute compliance (closure-aware rules)
  Gate: J5 arc_violations drops from 14 to ≤3, J5 attr compliance >80%

Phase 2 - Deep Verification (Rust, independent track)
  └─ 05: ARC IR structural verifier (Lean 4 Checker pattern)
  Gate: `cargo test -p ori_arc` includes structural verification tests

Phase 3 - Validation
  └─ 07: Re-score all 12 journeys, update golden files, regression tests
  Gate: All journeys ≥7.5 with ≤2 documented false positives total
```

**Why this order:**
- Phase 0 has the highest impact-per-effort ratio. The effect summary table (~50 lines) fixes 90% of J9/J10 false positives.
- Phase 1 addresses the remaining false positives with deeper analysis.
- Phase 2 is the "right" long-term solution (verify before lowering) but is independent of Python tooling.
- Phase 3 proves everything works together.

## Tool Inventory (Current State)

### Python Tools (`.claude/skills/code-journey/`)

| Tool | Lines | Purpose | Issues |
|------|-------|---------|--------|
| `ir_parser.py` | 503 (**at limit -- split in 04.0**) | LLVM IR text -> structured Module | Can't parse quoted names (`@"..."`); no `invoke` handling |
| `arc_metrics.py` | 172 | RC op counting, violation detection | No runtime function awareness, per-function only; `_RC_DEC_RE` missing `ori_map_buffer_rc_dec`, `ori_set_buffer_rc_dec`, `ori_set_buffer_drop_unique`; `_RC_INVOKE_RE` defined but never used |
| `attribute_metrics.py` | 190 | Attribute compliance scoring | No closure-aware rules; no indirect call detection |
| `control_flow_metrics.py` | 133 | CF defect detection | No SSO/null-guard awareness; no `invoke` target extraction |
| `instruction_metrics.py` | 168 | Instruction efficiency scoring | Works well |
| `binary_metrics.py` | 128 | Binary section analysis | Works well |
| `extract-metrics.py` | 261 | Pipeline orchestrator | Works well; no new V2 fields yet |
| `score.py` | 630 (**over guideline -- split in follow-up**) | Metric -> score mapping | Works well, deterministic |
| `extract_ir_from_results.py` | 105 | Extract IR from results.md | Works well |
| `ir_utils.py` | 93 | Shared IR analysis utilities | `extract_branch_targets()` missing `invoke` |

### Rust Verifier (`compiler/ori_llvm/src/verify/`)

| Module | Lines | Purpose | Issues |
|--------|-------|---------|--------|
| `rc_balance.rs` | 274 | In-pipeline RC lifecycle tracking (inkwell, not text) | Only recognizes `ori_rc_alloc` as allocation; linear walk, not CFG-aware |
| `cow_rules.rs` | 170 | COW sequencing validation | Works |
| `abi_check.rs` | 197 | ABI conformance | Works |
| `safety_checks.rs` | 216 | Panic/assert density | Works |
| `report.rs` | 137 | Finding types and report structure | Works |

### Shell Scripts (`diagnostics/`)

All working. Not targets of this plan.

## Estimated Effort

| Section | Est. Lines (code) | Est. Lines (tests) | Complexity | Depends On |
|---------|-------------------|--------------------| -----------|------------|
| 01 Effect Summaries | ~150-200 | ~100 | **Low-Medium** | -- |
| 02 CFG RC Balance | ~250-350 | ~200 | **High** (risk: see section warning) | 01, 04 |
| 03 Cross-Function Ownership | ~180-250 | ~120 | **Medium** | 01 |
| 04 IR Parser Hardening | ~60-100 | ~80 | **Medium** | -- |
| 05 ARC IR Verification (Rust) | ~200-300 | ~200 | **High** | -- |
| 06 Attribute Compliance | ~100-150 | ~80 | **Medium** | -- |
| 07 Integration | ~80-120 | ~120 | **Medium** | 01-04, 06 |
| **Total new** | **~1020-1470** | **~900** | | |

## Known False Positives (Pre-existing)

| Journey | False Positive | Root Cause | Fix Section | Violations |
|---------|---------------|------------|-------------|------------|
| J5 | 14 ARC violations from closures | Cross-function env allocation/consumption | 01 + 03 | 14 |
| J9 | 9 ARC violations from strings | `ori_str_from_raw` opaque to scanner | 01 | 9 |
| J10 | 15 ARC violations from lists | `ori_list_alloc_data` opaque to scanner | 01 | 15 |
| J9 | 5 CF defects from SSO gate | `rc_dec` behind SSO conditional | 02 | 5 |
| J8 | Missing functions in parse | Quoted LLVM names not parsed | 04 | N/A |
| Any w/ maps | Undercounted map RC decs | `_RC_DEC_RE` missing `ori_map_buffer_rc_dec` | 01 | ? |
| Any w/ sets | Undercounted set RC decs | `_RC_DEC_RE` missing `ori_set_buffer_rc_dec`, `ori_set_buffer_drop_unique` | 01 | ? |
| Any w/ invoke | Missed RC ops in invoke instructions | `_RC_INVOKE_RE` defined but never used in balance counting | 04 | ? |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Runtime Function Effect Summaries | `section-01-effect-summaries.md` | Complete |
| 02 | CFG-Aware RC Balance Checking | `section-02-cfg-rc-balance.md` | Complete |
| 03 | Cross-Function Ownership Tracking | `section-03-cross-function-ownership.md` | Complete |
| 04 | IR Parser Hardening | `section-04-ir-parser-hardening.md` | Complete |
| 05 | ARC IR-Level Verification | `section-05-arc-ir-verification.md` | Complete |
| 06 | Attribute Compliance Improvements | `section-06-attribute-compliance.md` | Complete |
| 07 | Integration and Re-scoring | `section-07-integration.md` | Complete |
