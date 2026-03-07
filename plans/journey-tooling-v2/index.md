---
reroute: true
name: "Journey Tooling"
full_name: "Journey Tooling V2"
status: queued
order: 3
---

# Journey Tooling V2 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Builds on:** `plans/journey-scoring-algorithms/` (complete — deterministic metric extraction)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Runtime Function Effect Summaries
**File:** `section-01-effect-summaries.md` | **Status:** Not Started

```
runtime effects, function summary, ori_str_from_raw, ori_list_alloc_data,
ori_rc_alloc, ori_map_literal_alloc, allocation point, opaque construction,
false positive, rc_balance.rs, arc_metrics.py, RetainCountChecker, Clang,
Swift, summary table, RC effect, +1 retained, ownership transfer,
ori_set_literal_alloc, ori_set_buffer_rc_dec, ori_set_buffer_drop_unique,
ori_str_from_int, ori_str_from_float, ori_str_from_bool, ori_format_int,
ori_iter_from_list, ori_iter_from_range, ori_iter_drop, ori_iter_map,
ori_list_empty, ori_set_empty, ori_catch_recover
```

---

### Section 02: CFG-Aware RC Balance Checking
**File:** `section-02-cfg-rc-balance.md` | **Status:** Not Started

```
control flow graph, CFG, dataflow, per-SSA-value, conditional RC,
SSO gate, null guard, branch, phi node, basic block, join point,
TopDownRefCountState, BottomUpRefCountState, KnownSafe, Swift ARC,
RefCountState, bidirectional, lattice, rc_balance, false positive
```

---

### Section 03: Cross-Function Ownership Tracking
**File:** `section-03-cross-function-ownership.md` | **Status:** Not Started

```
ownership transfer, cross-function, closure, environment, env_ptr,
caller, callee, Owned, Borrowed, AnnotatedSig, RCIdentityAnalysis,
borrow inference, parameter ownership, consume, return +1, J5 closures
```

---

### Section 04: IR Parser Hardening
**File:** `section-04-ir-parser-hardening.md` | **Status:** Not Started

```
ir_parser.py, ir_parser_internal.py, file split, quoted function names,
monomorphization, generics,
@"_ori_first$24m$24int_int", _FUNC_NAME_RE, regex, LLVM IR,
invoke, landing pad, phi multi-line, metadata, debug info,
invoke targets, unwind, to label, ir_utils.py, extract_branch_targets,
_RC_INVOKE_RE, indirect call
```

---

### Section 05: ARC IR-Level Verification
**File:** `section-05-arc-ir-verification.md` | **Status:** Not Started

```
ARC IR, ori_arc, Construct, RcInc, RcDec, ArcFunction, ArcBlock, ArcInstr,
ArcVarId, ArcBlockId, ValueRepr, RcStrategy, ArcBlock.body,
Checker.lean, Lean 4, structural verification, pre-codegen,
variable scope, type consistency, balance by construction,
run_arc_pipeline, pipeline.rs,
ArcTerminator, Invoke, Resume, Unreachable, used_vars, defined_var
```

---

### Section 06: Attribute Compliance Improvements
**File:** `section-06-attribute-compliance.md` | **Status:** Not Started

```
attribute_metrics.py, noundef, nounwind, uwtable, fastcc, cold,
noreturn, memory, readonly, readnone, closure, indirect call,
attribute group, J5 compliance 60%, not_applicable_reason,
closure detection, leaf function, indirect call detection
```

---

### Section 07: Integration and Re-scoring
**File:** `section-07-integration.md` | **Status:** Not Started

```
extract-metrics.py, score.py, rescore, false positive elimination,
regression test, golden file, journey re-run, deterministic,
Python 3.10, pytest, test suite, end-to-end,
extract_ir_from_results.py, rescore-v2.sh, arc_module_balanced,
arc_ownership_transfers, arc_conditional_ops, pipeline test
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Runtime Function Effect Summaries | `section-01-effect-summaries.md` |
| 02 | CFG-Aware RC Balance Checking | `section-02-cfg-rc-balance.md` |
| 03 | Cross-Function Ownership Tracking | `section-03-cross-function-ownership.md` |
| 04 | IR Parser Hardening | `section-04-ir-parser-hardening.md` |
| 05 | ARC IR-Level Verification | `section-05-arc-ir-verification.md` |
| 06 | Attribute Compliance Improvements | `section-06-attribute-compliance.md` |
| 07 | Integration and Re-scoring | `section-07-integration.md` |
