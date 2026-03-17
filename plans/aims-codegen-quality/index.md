---
reroute: true
name: "AIMS CQ"
full_name: "AIMS Codegen Quality: All Journeys >= 9.8"
status: done
order: 1
---

# AIMS Codegen Quality Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

### Section 01: AIMS Regressions
**File:** `section-01-aims-regressions.md` | **Status:** Complete

```
aims, regression, closure, rc_dec, memory_leak, drop_unique, invoke, landingpad
exception_handling, ori_buffer_drop_unique, ori_buffer_rc_dec, make_adder
J5, J10, closure_env, null_env, EH, unwind, cleanup, nounwind
decide_drop_hint, DropHints, collect_borrowed_call_args, realize_annotations
compiler/ori_arc/src/aims/realize/, compiler/ori_arc/src/aims/realize/decide.rs
compiler/ori_arc/src/aims/emit_rc/drop_hints.rs, compiler/ori_arc/src/uniqueness/drop_hints/mod.rs
compiler/ori_llvm/src/codegen/arc_emitter/, closures.rs, emit_function.rs
compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs
```

---

### Section 02: Attribute Compliance
**File:** `section-02-attribute-compliance.md` | **Status:** Complete

```
noundef, uwtable, nounwind, memory, fastcc, noalias, readonly
attribute, calling_convention, function_attributes, LLVM_attributes
struct_params, enum_params, main_wrapper, pure_functions, is_llvm_scalar
Direct, Indirect, ParamPassing, impl_methods, two_pass_nounwind
compiler/ori_llvm/src/codegen/function_compiler/mod.rs, nounwind.rs, entry_point.rs, impls.rs
compiler/ori_llvm/src/codegen/ir_builder/attributes.rs, calls.rs
compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs, compiler/ori_llvm/src/codegen/type_info/info.rs
J1, J2, J3, J4, J5, J6, J7, J8, J9, J10, J11, J12, J13
```

---

### Section 03: Control Flow Cleanup
**File:** `section-03-control-flow.md` | **Status:** Complete

```
empty_blocks, trampoline, redundant_branch, entry_block, block_merging
passthrough, unconditional_br, phi_node, trivial_phi, dead_block
SimplifyCFG, O0, post_emission, BasicBlock, eraseFromParent
J2, J3, J5, J7, J9, J10, J12
compiler/ori_llvm/src/codegen/arc_emitter/mod.rs, emit_function.rs
compiler/ori_llvm/src/codegen/ir_builder/control_flow.rs, checked_ops.rs
compiler/ori_llvm/src/aot/passes/config.rs
```

---

### Section 04: IR Quality Polish
**File:** `section-04-ir-quality.md` | **Status:** Complete

```
unjustified_instructions, sso_gating, range_materialization
instruction_reduction, redundant_construct, extractvalue, insertvalue
parameter_materialization, overflow_check, FunctionAbi, ParamPassing
J3, J5, J7, J9, J10
compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs, operators/mod.rs
compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs
compiler/ori_llvm/src/codegen/arc_emitter/apply_helpers.rs
```

---

### Section 05: Verification
**File:** `section-05-verification.md` | **Status:** Complete

```
code_journey, re-run, score, verification, merge_gate, rollback
test-all, clippy-all, dual-exec-verify, valgrind-aot, ORI_CHECK_LEAKS
rescore-all, extract-metrics, release_build, FastISel
plans/code-journeys/, .claude/skills/code-journey/score.py, .claude/skills/code-journey/extract-metrics.py
.claude/skills/code-journey/rescore-all.sh, diagnostics/valgrind-aot.sh
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | AIMS Regressions | `section-01-aims-regressions.md` |
| 02 | Attribute Compliance | `section-02-attribute-compliance.md` |
| 03 | Control Flow Cleanup | `section-03-control-flow.md` |
| 04 | IR Quality Polish | `section-04-ir-quality.md` |
| 05 | Verification | `section-05-verification.md` |
