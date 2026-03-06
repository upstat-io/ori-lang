# Journey Scoring Algorithms Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: LLVM IR Parser
**File:** `section-01-ir-parser.md` | **Status:** Complete

```
llvm ir, parser, function extraction, instruction count
define, declare, attributes, basic block, label
extract-metrics, ir-parser, parse_module, parse_function, parse_block
empty ir, malformed ir, parse_errors, error handling
attribute_groups, param_types, attribute_group_refs
Module, Function, BasicBlock, Instruction, data model
user_functions, runtime_declarations, llvm_intrinsics, is_entry_called
```

---

### Section 02: Instruction Efficiency Metrics
**File:** `section-02-instruction-metrics.md` | **Status:** Complete

```
instruction ratio, ideal, actual, overflow checking
sadd.with.overflow, smul.with.overflow, ssub.with.overflow
justified overhead, panic path, extractvalue, br i1
optimal ir, instruction purity, BLOATED, OPTIMAL
ir_utils, shared utilities, redundant branch, trivial phi
ir_unjustified, ir_quality, unjustified count
```

---

### Section 03: ARC Metrics
**File:** `section-03-arc-metrics.md` | **Status:** Complete

```
arc, reference counting, rc_inc, rc_dec, ori_rc_inc, ori_rc_dec
ori_buffer_rc_dec, ori_list_rc_inc, ori_buffer_drop_unique, ori_rc_free
balanced, unbalanced, scalar rc, borrow elision, move semantics
violation, leak, double-free, wasted pair
```

---

### Section 04: Attribute Metrics
**File:** `section-04-attribute-metrics.md` | **Status:** Complete

```
attributes, fastcc, nounwind, noreturn, cold, uwtable, noundef
calling convention, compliance, attribute checklist
function declaration, attribute group, noalias, memory
wrong attribute, MUST_NOT_BE_NOUNWIND, is_entry_called
```

---

### Section 05: Control Flow Metrics
**File:** `section-05-control-flow-metrics.md` | **Status:** Complete

```
control flow, basic block, empty block, redundant branch
phi node, trivial phi, unreachable, block layout
cfg, defect, br label, ir_utils, switch
is_incorrect, cf_incorrect, ir_incorrect, predecessor map
```

---

### Section 06: Binary Metrics
**File:** `section-06-binary-metrics.md` | **Status:** Complete

```
binary, elf, section size, .text, .rodata, .debug
disassembly, objdump, nm, size, user code bytes
exit code, eval, aot, mismatch, hard_fail
compilation failure, crash, sections-file, disasm-file
```

---

### Section 07: Integration
**File:** `section-07-integration.md` | **Status:** Complete

```
extract-metrics.py, pipeline, metrics.json, score.py
end-to-end, journey runner, SKILL.md, background agent
other findings, 7th dimension, AI-determined, migration
empty ir, error handling, compilation failure
rescore, backward compatibility, transition
```

---

### Section 08: Verification
**File:** `section-08-verification.md` | **Status:** Complete

```
verification, re-score, deterministic, reproducible
test suite, golden files, regression, journey re-run
rescore-all, discrepancy report, ir extraction, migration
python version, error path verification
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | LLVM IR Parser | `section-01-ir-parser.md` |
| 02 | Instruction Efficiency Metrics | `section-02-instruction-metrics.md` |
| 03 | ARC Metrics | `section-03-arc-metrics.md` |
| 04 | Attribute Metrics | `section-04-attribute-metrics.md` |
| 05 | Control Flow Metrics | `section-05-control-flow-metrics.md` |
| 06 | Binary Metrics | `section-06-binary-metrics.md` |
| 07 | Integration | `section-07-integration.md` |
| 08 | Verification | `section-08-verification.md` |
