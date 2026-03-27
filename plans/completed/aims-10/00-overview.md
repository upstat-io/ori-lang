---
plan: "aims-10"
title: "AIMS-10: All Code Journeys to 10/10"
status: complete
supersedes:
  - "Deferred items from plans/aims-codegen-quality/ sections 02-05"
  - "Roadmap 21.16.3-21.16.6"
references:
  - "plans/aims-codegen-quality/"
  - "plans/code-journeys/overview.md"
---

# AIMS-10: All Code Journeys to 10/10

## Mission

Make the Ori LLVM codegen emit optimal IR — correct attributes on every function, zero dead blocks, zero unjustified instructions. The measurement: all 13 code journeys score 10.0/10. This is not journey-specific tuning — it's systematic codegen infrastructure that benefits every Ori program.

## Architecture

```
Source → Lexer → Parser → Typeck → Canon → ARC Pipeline → LLVM Emission → LLVM IR
                                                              ↓
                                                     THIS PLAN'S SCOPE
                                                              ↓
                                              ┌─ Attribute pipeline (Section 01) [function_compiler/, ir_builder/]
                                              ├─ CFG simplification (Section 02) [ir_builder/cfg_simplify.rs, arc_emitter/dead_unwind.rs]
                                              └─ IR emission patterns (Section 03) [arc_emitter/, abi/]
```

Almost all changes are in `compiler/ori_llvm/src/codegen/`. The range materialization fix (Section 03.2) may alternatively require a change in `compiler/ori_arc/src/lower/collections/mod.rs` if the lowering strategy is changed rather than adding a peephole in LLVM emission. Scoring tools in `.claude/skills/code-journey/` (`attribute_metrics.py`, `control_flow_metrics.py`, `instruction_metrics.py`) also need updates.

## Design Principle

**Emit optimal IR at O0.** LLVM's optimization passes clean up sloppy IR at O1+, but debug builds (O0) run no passes. The codegen must emit clean IR on its own — no empty blocks, no redundant branches, no missing attributes. This also produces faster compile times (less work for LLVM passes) and better debug-build performance.

## Section Dependency Graph

```
01 Attributes ──────────┐
                        ├──→ 04 Verification
02 CFG Cleanup ──┬──────┤
                 │      │
03 IR Quality ───┘──────┘
  (03.1 audit depends on 02)
```

Sections 01 and 02 are fully independent — work in any order or parallel.
Section 03.1 (audit) MUST run after Section 02 completes. Sections 03.2-03.4 can be developed in parallel with 02 but their necessity is determined by 03.1.
Section 04 runs after all three are complete.

## Implementation Sequence

```
Phase 1a — Independent work (Sections 01, 02 in any order)
  ├─ 01: Attribute pipeline completion
  └─ 02: dead_unwind extraction (02.1) + post-emission CFG simplification (02.2-02.3)
  Gate: timeout 150 ./test-all.sh green after each section

Phase 1b — Conditional work (Section 03, after 02)
  ├─ 03.1: Audit — re-run journeys to see what's left after 02
  └─ 03.2-03.4: Fix remaining IR quality issues (if any)
  Gate: timeout 150 ./test-all.sh green

Phase 2 — Verification (Section 04)
  └─ Re-run all 13 journeys, confirm 10.0/10, full test suite
  Gate: All 13 journeys 10.0/10, zero leaks, all tests green
```

## Current Baseline (2026-03-16)

| Journey | Score | IE | ARC | Attr | CF | IR | Bin | Other | Blocking Categories |
|---------|-------|----|-----|------|----|-----|-----|-------|-------------------|
| J1 | 9.8 | 10 | 10 | 8 | 10 | 10 | 10 | 10 | Attr |
| J2 | 9.2 | 9 | 10 | 9 | 7 | 9 | 10 | 10 | CF, IE, IR |
| J3 | 9.2 | 9 | 10 | 8 | 7 | 9 | 10 | 10 | CF, Attr, IE, IR |
| J4 | 9.7 | 10 | 10 | 7 | 10 | 10 | 10 | 10 | Attr |
| J5 | 9.2 | 9 | 10 | 6 | 9 | 9 | 10 | 10 | Attr, IE, CF, IR |
| J6 | 9.8 | 10 | 10 | 8 | 10 | 10 | 10 | 10 | Attr |
| J7 | 9.2 | 9 | 10 | 8 | 7 | 9 | 10 | 10 | CF, IE, IR, Attr |
| J8 | 9.9 | 10 | 10 | 9 | 10 | 10 | 10 | 10 | Attr |
| J9 | 8.8 | 9 | 10 | 7 | 7 | 8 | 10 | 10 | CF, IR, Attr, IE |
| J10 | 8.8 | 9 | 10 | 7 | 7 | 8 | 10 | 10 | CF, IR, Attr, IE |
| J11 | 9.8 | 10 | 10 | 8 | 10 | 10 | 10 | 10 | Attr |
| J12 | 9.3 | 9 | 10 | 9 | 8 | 9 | 10 | 10 | CF, IE, IR |
| J13 | 9.4 | 10 | 10 | 4 | 10 | 10 | 10 | 10 | Attr |
| **Avg** | **9.4** | **9.5** | **10.0** | **7.5** | **8.6** | **9.3** | **10.0** | **10.0** | |

**What must change to reach 10.0:**
- **Attr 7.5 → 10.0**: Missing `noundef`, `nounwind`, `readonly`, `memory(read)`, `nonnull`, `dereferenceable`
- **CF 8.6 → 10.0**: Empty blocks (single `br` terminator, no phi) in 7 journeys, redundant entry blocks in 3
- **IR 9.3 → 10.0**: Range materialization (J7), SSO gating (J9), parameter extract/repack (J10)
- **IE 9.5 → 10.0**: Auto-fixes when CF and IR are fixed (IE deductions come from unjustified instructions)

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Files |
|---------|-------------------|------------|-------|
| 01 Attributes | ~200 | Medium | 3-4 in function_compiler/ + ir_builder/attributes.rs + abi/mod.rs + `.claude/skills/code-journey/attribute_metrics.py` |
| 02 CFG Cleanup | ~250 | **High** (unsafe FFI for `LLVMSetSuccessor`, predecessor tracking, fixed-point iteration) | ir_builder/cfg_simplify.rs (new) + arc_emitter/dead_unwind.rs (new extraction) + `.claude/skills/code-journey/control_flow_metrics.py` |
| 03 IR Quality | ~50-100 | Low | 1-2 in define_phase.rs + possibly `.claude/skills/code-journey/instruction_metrics.py` (may be N/A after Section 02) |
| 04 Verification | ~0 (scripted) | Low | Journey results + overview.md |
| **Total** | **~500-550** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Attribute Completion | `section-01-attributes.md` | Not Started |
| 02 | CFG Cleanup | `section-02-cfg-cleanup.md` | Not Started |
| 03 | IR Quality | `section-03-ir-quality.md` | Not Started |
| 04 | Verification | `section-04-verification.md` | Not Started |
