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

- [ ] `[BUG-04-011][high]` **LLVM test runner cannot compile imported generic functions (e.g., assert_eq from std.testing)** — found by continue-roadmap.
  Repro: `timeout 30 cargo run -- test --backend=llvm /tmp/test.ori` where test.ori uses `use std.testing { assert_eq }` and `assert_eq(actual: 42, expected: 42)` in a test. Interpreter passes, LLVM reports `unresolved function 'assert_eq' in apply — missing mono instance?`.
  Root cause (investigated 2026-04-01): Three-layer issue:
  1. **Sig lookup** — `collect_mono_functions` in both `arc_lowering.rs` and `compile.rs` only receives `function_sigs` (module-local). Imported generic function sigs (e.g., `assert_eq<T: Eq>`) are filtered at `llvm_backend.rs:199` (`if sig.is_generic() { continue; }`), so the mono collection silently skips them.
  2. **Canon body lookup** — `lower_to_arc` for mono functions uses the test file's canon, but imported functions' bodies are in the imported module's canon. Using the wrong canon causes stack overflow.
  3. **ARC codegen correctness** — Even with layers 1-2 fixed, the monomorphized bodies (e.g., `assert_eq<str>` calling `str()`, `!=`, `+`, `panic()`) produce ARC IR with "variable not yet defined" errors, leading to double-frees that crash the test runner.
  Fix for layers 1-2 was implemented and verified but reverted because layer 3 causes a regression (crashes the LLVM test runner, losing 281 previously-passing tests).
  Layer 3 root cause (investigated further 2026-04-01): Type variable index mismatch. The imported canon is re-interned into the merged pool, giving type variable `T` index `Y`. But `body_type_map` from MonoInstance maps the TYPE CHECKER's index `Z` for `T` → `int`. Since `Y != Z`, `resolve_body_type()` doesn't find the substitution → type stays as unresolved variable → broken ARC IR. Local generics work because both canon and body_type_map use the same pool. Fix direction: build a combined type_subst map using `per_module_caches` (source→merged mapping) to translate body_type_map keys to match re-interned canon indices.
  Subsystem: `compiler/oric/src/test/runner/llvm_backend.rs` (sig filter), `compiler/oric/src/test/runner/arc_lowering.rs` (mono collection + canon), `compiler/ori_llvm/src/codegen/arc_emitter/` (ARC variable definition ordering)
  Found: 2026-04-01 | Source: continue-roadmap (hygiene-full Section 03)

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

- [ ] `[BUG-04-016][critical]` **AOT `Result.debug()` formats the inactive payload variant and can segfault on mixed-layout payloads** — found by review-work.
  Repro: `ORI_BIN=./target/release/ori timeout 150 diagnostics/dual-exec-debug.sh --no-color /tmp/review_result_inactive_payload_debug.ori` with `@main () -> void = { let r: Result<[int], str> = Err("oops"); print(msg: r.debug()) }` — interpreter prints `Err("oops")`, AOT exits 139 (segfault).
  Root cause: `emit_result_debug()` in `debug_helpers.rs` extracts and formats both payload arms before branching on `tag`, so the inactive variant's bytes are reinterpreted as the wrong layout and passed into recursive debug formatting.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`
  Found: 2026-04-01 | Source: review-work (hygiene-full §03)

- [ ] `[BUG-04-017][high]` **AOT wrapper debug still prints `<?>` for Debug-capable payload types such as maps** — found by review-work.
  Repro: `ORI_BIN=./target/release/ori timeout 150 diagnostics/dual-exec-debug.sh --no-color /tmp/review_option_map_debug.ori` with `@main () -> void = { let o: Option<{str: int}> = Some({"x": 1}); print(msg: o.debug()) }` — interpreter prints `Some({x: 1})`, AOT prints `Some(<?>)`.
  Root cause: `emit_element_debug()` only special-cases primitives, wrappers, lists, and tuples, then falls back to a placeholder literal instead of dispatching to the payload's real Debug implementation.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`
  Found: 2026-04-01 | Source: review-work (hygiene-full §03)

- [ ] `[BUG-04-018][medium]` **AOT wrapper debug formats `byte` payloads as hex, diverging from current interpreter/spec behavior** — found by review-work.
  Repro: `ORI_BIN=./target/release/ori timeout 150 diagnostics/dual-exec-debug.sh --no-color /tmp/review_byte_wrapper_debug.ori` with `@main () -> void = { let b: byte = 42; let o: Option<byte> = Some(b); print(msg: o.debug()) }` — interpreter prints `Some(42)`, AOT prints `Some(0x2a)`.
  Root cause: `emit_element_debug()` routes `TypeInfo::Byte` through `ori_byte_debug_format()`, but the current spec/tests still pin byte debug to decimal output until byte storage semantics change.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`, `compiler/ori_rt/src/string/convert.rs`, `tests/spec/traits/debug/primitives.ori`
  Found: 2026-04-01 | Source: review-work (hygiene-full §03)

---

## Resolved Bugs

- [x] `[BUG-04-002][critical]` **Inherent impl method returns wrong value when type also has trait impl** — found by manual.
  Resolved: OBE on 2026-03-28. False positive — caused by stale release binary from prior session. After `cargo b --release` (force rebuild), `test_aot_multiple_impl_blocks` passes. The AOT test framework falls back to the release binary when debug lacks LLVM; the stale release binary had code from before range analysis field narrowing was fixed.

- None.
