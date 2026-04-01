---
section: "04"
title: "Codegen & LLVM"
status: open
goal: "Track and resolve all known codegen/LLVM bugs"
sections: []
---

# Section 04: Codegen & LLVM

**Subsystem:** `compiler/ori_llvm/`, `compiler/ori_arc/`

Bugs in LLVM IR generation, JIT/AOT compilation, monomorphization, ARC pipeline lowering, type lowering, and optimization.

---

## Open Bugs

- [ ] `[BUG-04-001][high]` **Cross-compilation to Windows fails: host linker used instead of cross-linker** — found by manual.
  Repro: `ori build hello.ori --target=x86_64-pc-windows-msvc` on Linux host
  Error: `R_AMD64_IMAGEBASE with __ImageBase undefined` — GNU ld receives Windows COFF object
  Root cause: `LinkerFlavor::for_target()` correctly selects `Msvc`, but `LinkerDetection::is_available()` fails (no `link.exe`/`lld-link` on Linux), fallback cascades to host `cc`. Additionally, Linux-compiled `libori_rt.a` is linked with Linux system libraries (`-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`). Three issues: (1) no validation that cross-linker exists before attempting cross-compile, (2) no cross-compiled runtime for target, (3) system library selection ignores target OS.
  Subsystem: `compiler/ori_llvm/src/aot/linker/driver.rs`, `mod.rs` (fallback logic), `gcc.rs` (system libs)
  Found: 2026-03-28 | Source: manual
  Note: Also applies to `--target=x86_64-pc-windows-gnu` (needs `x86_64-w64-mingw32-gcc`).

- [ ] `[BUG-04-003][high]` **Trait impl methods that access `self` struct fields produce LLVM verification errors in AOT** — found by continue-roadmap.
  Repro: `type Box = { w: int, h: int }` with `impl Printable for Box { @to_str (self) -> str = \`{self.w}x{self.h}\`; }` — LLVM verification: "Call parameter type does not match function signature!" Codegen extracts field 0 and passes it as the `self` parameter instead of passing the whole struct. Inherent impl methods with field access work fine; only trait impl methods are affected.
  Subsystem: `compiler/ori_llvm/src/codegen/` — `compile_impls()` trait method calling convention
  Found: 2026-03-28 | Source: continue-roadmap
  Note: Active work in roadmap section 03 (traits) and 21A (LLVM backend) touches this area.

- [x] `[BUG-04-004][high]` **AOT test `test_arc_loop_allocation` fails with exit code 1** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Same stale release binary pattern as BUG-04-002 — a fresh `cargo build` during §06 work rebuilt the release binary, and all 4 AOT tests now pass (14,584 total, 0 failures).

- [x] `[BUG-04-005][critical]` **AOT test `test_aot_derive_eq_mixed_types` segfaults (exit code -139)** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Stale release binary — same root cause as BUG-04-004.

- [x] `[BUG-04-006][high]` **Derived comparison codegen uses `icmp` on narrowed float fields** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Stale release binary — same root cause as BUG-04-004.

- [x] `[BUG-04-007][high]` **AOT test `test_float_narrowed_mixed_exact_non_exact` fails with exit code 1** — found by continue-roadmap.
  Resolved: OBE on 2026-03-29. Stale release binary — same root cause as BUG-04-004.

- [x] `[BUG-04-008][high]` **Zero-sized enum payload mismatch: `A(()) | B` triggers build_struct error and inconsistent sizing** — found by tpr-review.
  Resolved: Fixed on 2026-03-30. Five changes across 4 files: (1) `resolve_enum()` skips Unit/Never fields in payload size computation, (2) `construction.rs` returns const_zero for unit tuple construction and filters void args from enum variant construction (user-defined enums only), (3) `instr_dispatch.rs` short-circuits void field projection to zero constant, (4) `drop_enum.rs` skips void fields in offset computation, (5) `type_layout.rs` uses payload size (not field presence) for enum alignment. Tests: 8 Ori spec tests + 5 AOT tests + semantic pin (IR layout verification). 14,707 tests passing.

- [x] `[BUG-04-009][high]` **Result coalesce (`??`) always takes Err path in AOT/LLVM codegen** — found by continue-roadmap.
  Resolved: Fixed on 2026-03-30. Root cause: `lower_binary()` in ori_arc eagerly evaluated both operands of `??`, causing `panic()` on RHS to fire unconditionally. Fix: intercept `Coalesce` in `lower_binary()` and route to `lower_coalesce()` which generates conditional control flow (branch on tag → lazy RHS evaluation → merge). The LLVM `emit_coalesce()` (which uses `select`) is now dead code for `??` since the ARC IR already has the branch structure.

- [x] `[BUG-04-010][medium]` **`Option.iter()` has no AOT/LLVM support** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-01. Added `ori_iter_from_option` runtime function + `emit_option_iter()` codegen + dispatch entry. Verified working in both interpreter and LLVM with count, fold, for-loop. No leaks with RC'd elements.

- [ ] `[BUG-04-011][high]` **Option/Result spec tests all fail LLVM compilation: assert_eq unresolved, type variables unresolved at codegen** — found by continue-roadmap.
  Repro: `timeout 150 cargo run -p oric --bin ori -- test --backend=llvm tests/spec/types/option tests/spec/types/result` → `0 passed, 0 failed, 0 skipped, 11 llvm compile fail` (all 7 files fail). Interpreter passes all tests (4351 passed).
  Errors include: `unresolved function 'assert_eq' in apply — missing mono instance?`, `unresolved type variable at codegen — type inference bug`, `ArcIrEmitter: variable not yet defined`, `binary op on type with Unsupported strategy`, `icmp on non-int operands`.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/` (multiple: monomorphization, type resolution, variable definition ordering)
  Found: 2026-04-01 | Source: continue-roadmap (hygiene-full Section 03 TPR triage)
  Note: Individual Option/Result methods work in standalone `@main` AOT tests (verified for `ok_or`, `map`, `expect`). The failures appear to be test-runner/spec-framework issues with `assert_eq` monomorphization and generic type variable resolution, not specific to Option/Result method codegen. Active work in repr-opt and hygiene-full touches this area.

- [x] `[BUG-04-012][critical]` **Borrowed `Option`/`Result` AOT projections duplicate RC payload bits without retaining, causing double-free** — found by review-work.
  Resolved: Fixed on 2026-04-01. Added conditional `inc_value_rc` in `emit_option_iter()`, `Result.ok()`, and `Result.err()` codegen paths — guarded by tag check (only when Some/Ok/Err respectively). Remaining extraction methods (unwrap_or, expect, first, etc.) tracked as BUG-04-013.
  Repro:
  - `timeout 150 diagnostics/diagnose-aot.sh --valgrind /tmp/option_iter_heap_str.ori`
  - `timeout 150 diagnostics/diagnose-aot.sh --valgrind /tmp/result_err_projection_heap_str.ori`
  - `timeout 150 diagnostics/diagnose-aot.sh --valgrind /tmp/result_ok_projection_heap_str.ori`
  All three abort with `ori_rc_dec called on already-freed allocation` when the payload is a heap string (>SSO).
  Root cause: new LLVM paths for `Option.iter()` and `Result.ok()/err()` memcpy payload bytes out of borrowed wrappers into a fresh iterator/result wrapper without cloning or `RcInc` on nested RC fields. The interpreter clones these values instead, so AOT violates the established ownership contract.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_type_impls/option.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/result_monadic.rs`, `compiler/ori_rt/src/iterator/sources.rs`
  Found: 2026-04-01 | Source: review-work
  Note: The immediate crashes were reproduced for heap `str`; audit any other borrowed wrapper methods added in the same section that forward payloads into new wrappers or closure calls.

- [ ] `[BUG-04-013][critical]` **AOT wrapper extraction methods copy payload bytes without RC retain** — found by tpr-review.
  Partially fixed on 2026-04-01: `Option.unwrap_or`, `Option.expect`, `Result.unwrap_or`, `Result.expect`, `Result.expect_err` now emit conditional `inc_value_rc` on the extracted payload. Verified clean with standalone AOT repros using heap strings.
  Remaining: `unwrap`/`unwrap_err` (no tag check, deeper issue — need tag guard + panic first), `first`/`last` (runtime functions `ori_list_first`/`ori_list_last` copy elements without RC retain), niche-encoded Option/Result paths in `option_result_helpers.rs`.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`, `option_result_helpers.rs`, `compiler/ori_rt/src/list/query.rs`
  Found: 2026-04-01 | Source: tpr-review (hygiene-full §04)
  Note: Active work in repr-opt touches codegen area.

- [ ] `[BUG-04-014][high]` **AOT Option/Result debug output wrong for compound payloads** — found by tpr-review.
  Repro: `@main () -> void = { let x = Some([1, 2, 3]); print(msg: x.debug()) }` — interpreter prints `Some([1, 2, 3])`, AOT prints empty string. `emit_element_to_str()` only handles primitives and `str`, returns `None` for lists/tuples/nested wrappers.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs` (`emit_option_debug_branch`, `emit_result_debug`)
  Found: 2026-04-01 | Source: tpr-review (hygiene-full §03)

- [ ] `[BUG-04-015][medium]` **AOT Option/Result debug uses Printable semantics for str payloads instead of Debug** — found by tpr-review.
  Repro: `@main () -> void = { let x = Some("hi"); print(msg: x.debug()) }` — interpreter prints `Some("hi")`, AOT prints `Some(hi)` (missing quotes). The debug path calls `to_str` on inner values instead of `debug`.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs` (`emit_option_debug_branch`)
  Found: 2026-04-01 | Source: tpr-review (hygiene-full §03)

---

## Resolved Bugs

- [x] `[BUG-04-002][critical]` **Inherent impl method returns wrong value when type also has trait impl** — found by manual.
  Resolved: OBE on 2026-03-28. False positive — caused by stale release binary from prior session. After `cargo b --release` (force rebuild), `test_aot_multiple_impl_blocks` passes. The AOT test framework falls back to the release binary when debug lacks LLVM; the stale release binary had code from before range analysis field narrowing was fixed.

- None.
