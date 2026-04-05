---
reroute: true
name: "Closure Ownership"
full_name: "ApplyIndirect Closure Ownership Model"
status: active
order: 1
---

# ApplyIndirect Closure Ownership Model Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters

### Section 01: ARC IR Shape
```
ApplyIndirect, InvokeIndirect, arg_ownership, ArgOwnership, ArcInstr,
ArcTerminator, is_owned_position, ir/instr.rs, ir/mod.rs,
closure call, indirect call, ownership field, used_vars,
emit_terminator_rc, forward_walk.rs, is_var_defined_in_block, emit_unified.rs
```

### Section 02: Ownership Propagation
```
annotate_arg_ownership, compute_arg_ownership, emit_arg_ownership,
resolve_indirect_arg_ownership, MemoryContract, ParamContract, AnnotatedSig,
PartialApply, closure contract, rc_insert/annotate.rs, aims/emit_rc/arg_ownership.rs,
borrow/update.rs, lambda_capture_ownership, non_capturing_lambdas, realize_rc_reuse,
def_map, ResolvedDef, build_closure_def_map, ConsumingCtx, apply_consuming_overrides
```

### Section 03: LLVM Cleanup & Verification
```
build_closure_env, generate_env_drop_fn, closure_wrappers, emit_partial_apply,
non-capturing fast path, phantom env, drop_hints, collect_borrowed_call_args,
InvokeIndirect terminator, define_phase.rs, context.rs, stale doc comment,
BUG-04-035, ORI_CHECK_LEAKS, curried closure, nested closure, RC leak,
unwind_cleanup, TPR-01-006
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | ARC IR Shape | `section-01-arc-ir-shape.md` | Complete |
| 02 | Ownership Propagation | `section-02-ownership-propagation.md` | In Progress |
| 03 | LLVM Cleanup & Verification | `section-03-llvm-cleanup.md` | Not Started |
