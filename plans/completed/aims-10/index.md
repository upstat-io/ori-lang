---
reroute: true
name: "AIMS-10"
full_name: "AIMS-10: All Code Journeys to 10/10"
status: resolved
order: 1
---

# AIMS-10 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Supersedes:** Deferred items from `plans/aims-codegen-quality/` sections 02-05 and roadmap 21.16.3-21.16.6

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

### Section 01: Attribute Completion
**File:** `section-01-attributes.md` | **Status:** Complete

```
nounwind, noundef, readonly, memory(none), memory(read), nonnull, dereferenceable
attributes.rs, function_compiler/, nounwind.rs, define_phase.rs, ParamAbi, Ownership
attribute compliance, LLVM attributes, function attributes, purity analysis
declare_function_llvm, add_memory_read_attribute, abi_size, entry_point.rs
.claude/skills/code-journey/attribute_metrics.py
J13 57.1%, J5 78.6%, main wrapper noundef, closure wrappers _ori_partial_N
```

---

### Section 02: CFG Cleanup
**File:** `section-02-cfg-cleanup.md` | **Status:** Complete

```
empty blocks, redundant branches, entry block merging, loop preheader
SimplifyCFG, block merging, dead blocks, dead_unwind extraction
arc_emitter/emit_function.rs, arc_emitter/dead_unwind.rs (new extraction)
ir_builder/cfg_simplify.rs (new), ir_builder/control_flow.rs, ir_builder/checked_ops.rs
.claude/skills/code-journey/control_flow_metrics.py (scoring tool update)
J2 J3 J5 J7 J9 J10 J12 CF defects
post-emission CFG simplification, inkwell BasicBlock, LLVMSetSuccessor
```

---

### Section 03: IR Quality
**File:** `section-03-ir-quality.md` | **Status:** Complete

```
unjustified instructions, instruction ratio, range materialization
SSO gating, parameter materialization, FastISel safety pattern
arc_emitter/value_emission.rs, builtins/collections/string_builtins.rs
arc_emitter/apply_helpers.rs, arc_emitter/rc_ops.rs, abi/mod.rs
.claude/skills/code-journey/instruction_metrics.py (scoring tool update)
J7 range construct-then-destructure, J9 SSO rc_dec diamond, J10 count_items
post-Section-02 audit determines which items are needed
```

---

### Section 04: Verification
**File:** `section-04-verification.md` | **Status:** Complete

```
code journey re-run, score validation, 10.0/10, merge gate
test-all.sh, clippy-all.sh, fmt-all.sh, ORI_CHECK_LEAKS, valgrind
dual-exec-verify.sh, .claude/skills/code-journey/extract-metrics.py
.claude/skills/code-journey/score.py, rescore-all.sh
plans/code-journeys/overview.md, *-results.md files
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Attribute Completion | `section-01-attributes.md` | Complete |
| 02 | CFG Cleanup | `section-02-cfg-cleanup.md` | Complete |
| 03 | IR Quality | `section-03-ir-quality.md` | Complete |
| 04 | Verification | `section-04-verification.md` | Complete |
