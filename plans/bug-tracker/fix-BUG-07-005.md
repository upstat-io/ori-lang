---
bug: BUG-07-005
severity: low
title: "Orphan env vars ORI_NO_REPR_OPT and ORI_VERIFY_ARC not registered in debug_flags.rs"
status: complete
---

# Fix: BUG-07-005 — Register orphan env vars in debug_flags.rs

## § 1. Investigation

**Root cause**: Both flags are actively used but not registered in the centralized `debug_flags.rs` registry. `check-debug-flags.sh` reports them as ORPHAN.

**Files affected**:
- `compiler/oric/src/debug_flags.rs` — add 2 flag constants + compile-time sync assert
- `compiler/ori_repr/src/plan/query.rs` — export constant for env var name
- `compiler/oric/src/commands/codegen_pipeline.rs` — use constant instead of string literal
- `compiler/oric/src/arc_dump/mod.rs` — use constant instead of string literal
- `compiler/oric/src/arc_dot/mod.rs` — use constant instead of string literal
- `CLAUDE.md` — document both flags

## § 1.5 Fix Consensus

**Round 1** (`/tmp/ori-tpr-fn8qIiQB`): Both Codex and Gemini converge. Key additions beyond minimal registration: (1) use `debug_flags::ORI_VERIFY_ARC` constant at oric call sites, (2) add compile-time sync for ORI_NO_REPR_OPT, (3) update CLAUDE.md, (4) do NOT use dbg_set!.

## § 2. Completion Checklist

- [ ] Add both flags to `flags!` macro
- [ ] Export constant from ori_repr, add compile-time sync assert
- [ ] Update 3 ORI_VERIFY_ARC call sites to use constant
- [ ] Update CLAUDE.md
- [ ] check-debug-flags.sh passes clean
- [ ] test-all.sh green
