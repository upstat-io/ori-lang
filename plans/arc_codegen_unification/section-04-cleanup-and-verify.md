# Section 04 — Cleanup and Verify

## Clean SYNC comments

Remove all `ExprLowerer` and `Tier 1` references from `arc_emitter/mod.rs`.

## Update rules files

- `.claude/rules/llvm.md` — remove Tier 1/2 architecture, update key files
- `.claude/rules/arc.md` — delete "Enabling ARC Codegen" and "Tier 1/2 Sync" sections

## Update doc comments

- `evaluator.rs` — remove ExprLowerer references
- `codegen/mod.rs` — update module documentation
- `lib.rs` — remove ExprLowerer references

## Grep verification

Confirm zero remaining references to:
- `ExprLowerer`, `use_arc_codegen`, `set_arc_codegen`
- `define_function_body_tier1`, `Tier 1` / `Tier 2` in LLVM/ARC context
- Any deleted file names in non-deleted files

## Final verification

1. `./test-all.sh` — all spec tests pass
2. `./llvm-test.sh` — all LLVM unit tests pass
3. `cargo blr && ./test-all.sh` — release mode
4. `./clippy-all.sh` + `./llvm-clippy.sh` — clean
5. `./fmt-all.sh` — formatted
6. Check `ori_rc_live_count()` returns 0 after all tests
7. Check `#[ignore]` tests in `arc.rs` — un-ignore if now passing
