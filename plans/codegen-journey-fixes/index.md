# Code Journey Findings Index

> **Maintenance Notice:** Update this index when adding/modifying sections.
> **Source:** `plans/code-journeys/summary.md` (findings #2–#10)

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Nounwind Soundness
**File:** `section-01-nounwind-soundness.md` | **Status:** Not Started

```
nounwind, unwind, invoke, call, landing pad, UB, undefined behavior
indirect call, closure, function pointer, fn_ptr, Apply, trampoline
monomorphization, monomorphized, generic, specialization, compilation order
is_arc_function_nounwind, nounwind_functions, FxHashSet
function_compiler/mod.rs, arc_emitter/mod.rs, ir_builder/calls.rs
finding #2, finding #3, journey 3, journey 4
```

---

### Section 02: IR Emission Cleanup
**File:** `section-02-ir-emission-cleanup.md` | **Status:** Not Started

```
runtime declarations, declare, extern, eager, lazy, on-demand
dead blocks, unreachable, landing pad, unwind target, dead_unwind
match arm, switch, phi, branch, redundant, SimplifyCFG
runtime_decl/mod.rs, declare_runtime, declare_extern_function
arc_emitter/mod.rs, dead_unwind, all_invoke_unwind, unwind_blocks
finding #4, finding #5, finding #7, journey 1, journey 2, journey 6
```

---

### Section 03: Closure Pipeline
**File:** `section-03-closure-pipeline.md` | **Status:** Not Started

```
closure, lambda, trampoline, non-capturing, fat pointer, env_ptr
_ori_partial, _ori_apply, trampolines.rs, TrampolineKind
function pointer, bare function, direct call, indirection
nounwind attribute, trampoline nounwind, closure nounwind
builtins/trampolines.rs, function_compiler/mod.rs, compile_lambda_arc
finding #6, finding #9, journey 4
```

---

### Section 04: IR Readability
**File:** `section-04-ir-readability.md` | **Status:** Not Started

```
struct name, type name, opaque, ori.3, %Point, named struct
type_info/mod.rs, type_name, named_structs, struct_name, enum_name
Pool, Idx, type_named_struct, set_struct_body
finding #8, journey 5
```

---

### Section 05: Developer Tooling
**File:** `section-05-developer-tooling.md` | **Status:** Not Started

```
cargo run, LLVM feature, feature flag, binary overwrite, symlink
target/debug/ori, ~/.local/bin/ori, cargo bl, cargo blr
developer experience, DX, silent failure, feature stripping
finding #10, journey 1
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Not Started

```
verification, test matrix, code journey, dual-exec, regression
test-all.sh, llvm-test.sh, dual-exec-verify.sh, valgrind-aot.sh
behavioral equivalence, eval vs AOT, JIT vs LLVM
summary.md, journey results, coverage map
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Nounwind Soundness | `section-01-nounwind-soundness.md` |
| 02 | IR Emission Cleanup | `section-02-ir-emission-cleanup.md` |
| 03 | Closure Pipeline | `section-03-closure-pipeline.md` |
| 04 | IR Readability | `section-04-ir-readability.md` |
| 05 | Developer Tooling | `section-05-developer-tooling.md` |
| 06 | Verification | `section-06-verification.md` |
