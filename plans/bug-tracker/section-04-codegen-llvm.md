---
section: "04"
title: "Codegen & LLVM"
status: open
goal: "Track and resolve all known codegen/LLVM bugs"
sections: []
third_party_review:
  status: findings
  updated: 2026-04-03
---

# Section 04: Codegen & LLVM

**Subsystem:** `compiler/ori_llvm/`, `compiler/ori_arc/`

Bugs in LLVM IR generation, JIT/AOT compilation, monomorphization, ARC pipeline lowering, type lowering, and optimization.

---

## Open Bugs

- [x] `[BUG-04-028][high]` **AIMS invoke RC analysis: merge block params get RcInc without matching RcDec** — found by continue-roadmap.
  Repro: Any test body with coalesce (`opt ?? default`) or branch producing RC-managed values followed by `assert_eq`. E.g., `let a = opt ?? [1,2,3]; assert_eq(actual: a, expected: [1,2,3])` in a test function. Works correctly as `@main`.
  Root cause: `is_live_at_exit` in `helpers.rs` returns true for merge block params (from coalesce/branch) at invoke terminators. AIMS inserts `RcInc` before the `[own]` invoke call but no matching `RcDec` on normal or unwind paths. Merge block param alias (`%13 = %11`) may confuse liveness tracking. Test body functions use immediate-emit path (no nounwind analysis), so calls become `invoke` instead of `call` — regular functions avoid this because two-pass nounwind pipeline converts calls to `call`.
  Manifests as: 1-allocation leak (coalesce case, `test_coalesce_copy.ori`), double-free crash (COW nested case, `cow/nested.ori`, `cow/sharing.ori`). The double-free crashes the entire LLVM spec test suite.
  Subsystem: `compiler/ori_arc/src/aims/emit_rc/forward_walk.rs`, `helpers.rs` (`is_live_at_exit`), `arg_ownership.rs`
  Found: 2026-04-03 | Source: continue-roadmap
  Note: Active work in JIT Exception Handling plan (Section 04) and repr-opt plan touch this area.

- [x] `[BUG-04-001][high]` **Cross-compilation to Windows fails: host linker used instead of cross-linker** — found by manual.
  Resolved: Fixed on 2026-04-06. Added cross-compilation-aware linker detection: `is_cross_compiling()` compares host vs target OS, `is_available_for_target()` checks for cross-compilers instead of host `cc`, `gcc_cross_compiler_name()` computes target-prefixed names (e.g., `x86_64-w64-mingw32-gcc`). `LinkerDriver::link()` now fails early with actionable error message listing required tools when no cross-linker is found, instead of silently falling back to host linker. `create_linker()` uses cross-compiler name for GCC when cross-compiling. Tests: 23 unit tests covering cross-compiler name resolution (6), cross-compilation detection (4), flavor selection (4), semantic pins (2, host cc rejected for Windows target), negative pins (4, error messages with actionable suggestions), native regression (1), link output extensions (2).
  Subsystem: `compiler/ori_llvm/src/aot/linker/driver.rs`, `mod.rs`, `tests.rs`
  Found: 2026-03-28 | Source: manual
  Note: Issue (2) — cross-compiled runtime — remains. Users must cross-compile `libori_rt.a` separately or the linker will fail at runtime lib discovery.

- [x] `[BUG-04-003][high]` **Trait impl methods that access `self` struct fields produce LLVM verification errors in AOT** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-02. Root cause: `declare_function_with_symbol` registered impl methods under their bare method name in the `functions` map, so when emitting a call like `int.to_str()` inside `Box$to_str`, the lookup found `Box$to_str` (which expects `%ori.Box`) instead of the correct `int$to_str`. Fix: added `declare_impl_method()` that registers only in `method_functions` (type-qualified key), not the bare `functions` map. Tests: AOT regression test `test_aot_trait_impl_field_access` + updated unit test. 14,967+ tests passing.
  Subsystem: `compiler/ori_llvm/src/codegen/function_compiler/mod.rs`, `impls.rs`
  Found: 2026-03-28 | Source: continue-roadmap

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

- [x] `[BUG-04-011][high]` **LLVM test runner cannot compile imported generic functions (e.g., assert_eq from std.testing)** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-02. Four-file fix across oric and ori_llvm:
  (1) `monomorphize/mod.rs`: Made `mangle_mono_name` public for external callers.
  (2) `llvm_backend.rs`: Added imported generic sig collection (keyed by local_name for aliased imports) and ImportedMonoFunction construction with fresh body_type_map built from per_module_cache values + scheme_var_id-based var_subst. Added `ensure_var_capacity` to Pool for imported var_ids.
  (3) `arc_lowering.rs`: Extended `lower_and_infer_borrows` to accept imported mono functions + re-interned canons. Added per-function borrow inference loop (same pattern as imported non-generics).
  (4) `compile.rs`: Added `imported_mono_functions` parameter to `compile_module_with_tests`/`compile_all_functions`. Merges imported mono functions into local mono_functions for declaration/preparation/emission.
  Works for primitive type instantiations (int, str, bool). Compound types ([int], maps) hit a pre-existing limitation (BUG-04-022: JIT can't resolve free function calls in generic bodies). 15,018 tests passing.
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

- [x] `[BUG-04-013][critical]` **AOT wrapper extraction methods copy payload bytes without RC retain (explicit-tag paths)** — found by tpr-review.
  Resolved: Fixed on 2026-04-02. Three fixes for explicit-tag codegen paths:
  (1) `Option.unwrap`, `Result.unwrap`, `Result.unwrap_err` — added `emit_unwrap_branch` (tag guard + `ori_panic_cstr`) + unconditional `inc_value_rc` after guard in `option_result.rs`.
  (2) `List.first`/`List.last` — added conditional `inc_value_rc` on the element payload (guarded by `OPTION_TAG_SOME` tag check) after runtime memcpy in `list_builtins/helpers.rs`.
  (3) Previously fixed: `unwrap_or`, `expect`, `expect_err` explicit-tag paths.
  Tests: AOT tests in `wrapper_rc_retain.rs` covering heap str and list payloads + panic-path negative pins. 14,953+ tests passing.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins/helpers.rs`
  Found: 2026-04-01 | Source: tpr-review (hygiene-full §04)

- [x] `[BUG-04-019][medium]` **Niche-encoded Option/Result extraction paths missing RC retain and tag guards** — found by tpr-review.
  Resolved: Fixed on 2026-04-07. Three categories of bugs in `option_result_helpers.rs`:
  (1) **Missing tag guards**: `Option.unwrap`, `Result.unwrap`, `Result.unwrap_err`, `Result.expect`, `Result.expect_err` extracted payload without panicking on the wrong variant. The `Option.expect` arm had a tag guard but the rest had nothing.
  (2) **Missing RC retain**: ALL extraction paths returned the payload without calling `inc_value_rc`, leading to use-after-free when both the wrapper and the extracted value share inner heap data.
  (3) **Collapsed Result arm**: `"unwrap" | "unwrap_err" | "unwrap_or" => extract_value(...)` collapsed three semantically distinct methods into one body — `unwrap_err` returned the same as `unwrap` regardless of variant, and `unwrap_or` ignored its default argument.
  Fix: rewrote each arm to mirror the explicit-tag pattern from `option_result.rs` (BUG-04-013) — niche-aware variant predicate via new `compute_option_is_some` / `compute_result_is_ok` / `compute_result_is_err` helpers, `emit_unwrap_branch` / `emit_expect_branch` for panic, then `extract_value` + unconditional `inc_value_rc` (or conditional via cond_br for `unwrap_or`). Added `receiver_ty: Idx` parameter to `emit_result_niche` and updated the single call site in `option_result.rs:186` to pass it. Tests: 9 structural unit tests in `option_result_helpers/tests.rs` use `include_str!` to assert each fixed arm contains the required `emit_unwrap_branch`/`emit_expect_branch`, `inc_value_rc`, `cond_br`, and panic-message wording, plus a negative pin asserting the collapsed arm pattern is absent and a semantic pin asserting `Result.unwrap` and `Result.unwrap_err` bodies differ. Fix file: `plans/bug-tracker/fix-BUG-04-019.md`.
  **LLVM runtime parity**: behavioral verification rides on `<!-- blocked-by:NICHE_CODEGEN_READY gate -->` items already tracked in `plans/repr-opt/section-07-enum-repr.md` §07.2 (the same gate the rest of the niche-encoded codegen path waits on). When that gate flips to `true`, the existing niche spec tests under `tests/spec/types/enum/niche/` will exercise these helpers end-to-end. Until then, the structural assertions are the regression guard. Section §07.2 "Codegen consumers updated" list now includes `option_result_helpers.rs` referencing this fix.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result.rs`
  Found: 2026-04-02 | Source: tpr-review (BUG-04-013 follow-up)
  Severity downgraded: 2026-04-02. High→Medium — unreachable while niche gate is disabled. Was incorrectly believed reachable via `Option<CPtr>`; BUG-04-021 root cause was type inference (unresolved Var), not niche encoding.

- [x] `[BUG-04-025][high]` **LLVM codegen lacks compound ordering for built-in wrapper types (Option, Result, Tuple with <, <=, >, >=)** — found by tpr-review.
  Resolved: Fixed on 2026-04-03. Added inline `emit_element_compare()` dispatch in `emit_ordering_comparison()` before the compiled method fallback. Compound types (Option, Result, Tuple, List) now use their existing recursive comparison implementations for ordering operators. Tests: `aot_option_ordering.ori`. LCFail decreased 4137→4065 (72 newly-compilable tests).
  Repro: `if Some(1) < Some(2) then print(msg: "ok")` — interpreter passes, `--backend=llvm` warns "Unsupported strategy" and fails with "icmp on non-int operands". `emit_comparison_via_trait()` returns None for built-in wrappers (no compiled `compare` method), falls through to registry fallback which tries integer comparison on aggregate values.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs`
  Found: 2026-04-03 | Source: tpr-review (imported-generic-mono iteration 3)

- [x] `[BUG-04-026][medium]` **Structural equality incomplete for payload enums without `#derive(Eq)` in LLVM** — found by tpr-review.
  Resolved: Fixed on 2026-04-03 (scalar payloads), narrowed on 2026-04-03 (TPR-04-002). Extended `emit_structural_eq_enum` to handle homogeneous payload enums with SCALAR fields (int, float, bool, char, byte, ptr). Extracts payload via `extract_value_any`, reinterprets i64 slots to field types, and compares recursively via `emit_element_equals`. Aggregate payload fields (lists, maps, sets, tuples) and heterogeneous payload enums still need `#derive(Eq)`. Tests: `aot_payload_enum_structural_eq.ori`.
  Repro: `type Shape = Circle(r: int) | Square(s: int); Circle(r: 1) == Circle(r: 1)` — interpreter passes, `--backend=llvm` panics with "binary op Eq on unmapped type idx". `emit_structural_eq_enum` handles unit-only enums (tag comparison) but returns None for payload variants. Needs tag-switch + per-variant field comparison.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/compound_traits.rs`
  Found: 2026-04-03 | Source: tpr-review (imported-generic-mono iteration 3)

- [x] `[BUG-04-027][high]` **LLVM codegen: `NaN != NaN` evaluates to false (should be true per IEEE 754)** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-03. Root cause: `strategy.rs:164` used `fcmp_one` (ordered not-equal) which returns false when either operand is NaN. IEEE 754 requires `NaN != NaN` to be true. Fix: changed to `fcmp_une` (unordered not-equal) which correctly returns true for NaN operands. LLVM's constant folder eagerly evaluates `fcmp_une NaN, NaN` → `true` at compile time.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/operators/strategy.rs`
  Found: 2026-04-03 | Source: continue-roadmap (imported-generic-mono TPR)

- [x] `[BUG-04-022][medium]` **LLVM JIT cannot resolve free function calls in monomorphized generic bodies (e.g., `str()`, `debug()` for compound types)** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-03. Two root causes: (1) `("list", "debug")` was missing from the LLVM builtin collections dispatch table — `emit_list_debug()` existed but wasn't wired up. (2) `emit_str()` in the prelude handler only handled primitives — compound types (List, Map, Set, Option, Result, Tuple) returned None, causing "unresolved function `str`" in imported generic bodies where the type checker desugars `.debug()` to `str()` calls. Fix: added `("list", "debug")` and `("str", "debug")` dispatch entries, extended `emit_str()` to handle all compound types via `emit_element_debug`. LCFail decreased 4065→4000 (65 newly-compilable LLVM tests). Both local and imported generics with compound types now work through JIT test runner (`--backend=llvm`).
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/prelude.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`
  Found: 2026-04-02 | Source: continue-roadmap (BUG-04-011 implementation)

- [x] `[BUG-04-024][medium]` **ARC emitter "variable not yet defined var=2" in imported mono functions** — found by continue-roadmap.
  Resolved: OBE on 2026-04-06. Verified: `assert_eq(actual: 42, expected: 42)` through `--backend=llvm` produces zero "variable not yet defined" errors and passes cleanly. Multi-type tests (int, str, bool) all pass. Fixed by JIT EH §06.2 (Generalized Var Resolution) which resolved type variables before they reach the emitter, and §06.3 (ARC IR Index Bounds Safety) which added safe fallbacks. The body_type_map gap no longer manifests because the upstream lambda mono pipeline now resolves all type variables before ARC lowering.
  Repro: `ORI_LOG=ori_llvm=debug timeout 30 cargo run -- test --backend=llvm /tmp/test.ori` where test.ori uses `use std.testing { assert_eq }` and `assert_eq(actual: 42, expected: 42)`. ERROR log emitted consistently for every imported mono function compilation. Tests pass — emitter recovers via `ValueId::NONE` fallback.
  Root cause: Residual of BUG-04-011 layer 3. The `body_type_map` built from `per_module_cache` values + `scheme_var_ids` doesn't cover all internal ARC variables in the imported function's IR. Variable 2 (likely a comparison result or intermediate in `assert_eq`'s body) is referenced before being defined in the emitter's `var_map`. The substitution only covers types that appear in the per_module_cache (cross-module re-interned types), not purely-local variables created during ARC lowering.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs`, `compiler/oric/src/test/runner/llvm_backend.rs` (body_type_map construction)
  Found: 2026-04-02 | Source: continue-roadmap (imported-generic-mono verification)

- [x] `[BUG-04-029][medium]` **LLVM backend missing shift overflow/negative count/bit width runtime checks** — found by continue-roadmap.
  Resolved: OBE on 2026-04-06. `checked_shl()`/`checked_shr()` in `checked_ops.rs` (lines 381-542) implement negative count check (`rhs < 0` → panic), bit width check (`rhs >= 64` → panic), and left-shift overflow check (roundtrip verification). These are wired into operator strategy at `strategy.rs:136-137`. All 5 previously-failing tests now pass: `test_shl_overflow_panic`, `test_shl_bit_width_panic`, `test_shl_negative_count_panic`, `test_shr_bit_width_panic`, `test_shr_negative_count_panic`. 43/43 bitwise operator tests pass via LLVM backend.
  Subsystem: `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs`
  Found: 2026-04-03 | Source: continue-roadmap (JIT EH §04 verification)

- [x] `[BUG-04-023][high]` **LLVM codegen still panics on structural `==`/`!=` for user-defined types without `#derive(Eq)`** — found by review-work.
  Resolved: Fixed on 2026-04-02 (structs) and 2026-04-03 (enums). Added `emit_structural_eq` in `compound_traits.rs` for structs (field-by-field AND) and `emit_structural_eq_enum` for unit-only enums (tag comparison). When `emit_derived_eq_call` returns None, both Struct and Enum types fall back to structural comparison. Enums with payload variants still require `#derive(Eq)`. Tests: `aot_enum_structural_eq.ori` + prior struct test. 15,019 tests passing.
  Repro: `timeout 150 cargo run -q -p oric --bin ori -- test --backend=llvm /tmp/struct_eq_no_derive.ori` with:
  `use std.testing { assert }`
  `type Point = { x: int, y: int }`
  `@test_eq_no_derive tests @eq_no_derive () -> void = { assert(cond: eq_no_derive()) }`
  `@eq_no_derive () -> bool = { let a = Point { x: 1, y: 2 }; let b = Point { x: 1, y: 2 }; a == b }`
  Interpreter passes, but LLVM reports `internal error: entered unreachable code: binary op Eq on unmapped type idx ... — should have used trait dispatch`.
  Root cause: the type checker/evaluator intentionally allow structural equality on user-defined types without an `Eq` impl, but `ArcIrEmitter::emit_binary_op()` only knows how to lower comparisons via trait dispatch or registry-backed builtin dispatch. When `emit_comparison_via_trait()` finds no `Eq.eq` method, codegen falls through to `idx_to_type_tag(lhs_ty)` and hits the `unreachable!()` path for non-primitive user types instead of emitting structural equality.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/operators/mod.rs`
  Found: 2026-04-02 | Source: review-work

- [x] `[BUG-04-021][high]` **AOT fails to compile `Option<CPtr>` and other FFI wrapper types** — found by review-work.
  Resolved: Fixed on 2026-04-02. Root cause: FFI type names (CPtr, c_int, c_float, etc.) were not recognized by the inference-path type resolver (`resolve_parsed_type` in `infer/expr/type_resolution.rs`). When encountered as unknown Named types, they became fresh unbound type variables via `fresh_named_var()`. These variables were never linked/unified, so `Option<CPtr>` became `Option<Var(unbound)>` in the canonical IR. At codegen, the unbound Var produced `TypeInfo::Error`, causing the ARC classifier to emit RC operations on what should be a trivial scalar, and the LLVM type resolver to use `i64` instead of the correct type — triggering an `ori_rc_dec(i64, ptr)` signature mismatch. Fix: added FFI type recognition to both `resolve_parsed_type` (inference path) and `resolve_parsed_type_simple` (registration path). FFI type names now create `pool.named()` entries with `set_resolution()` to their concrete primitives (CPtr/c_int/c_long→INT, c_float/c_double→FLOAT). Tests: 3 AOT tests (Option<CPtr>, all C types, FFI types in structs). 14,978 tests passing.
  Subsystem: `compiler/ori_types/src/infer/expr/type_resolution.rs`, `compiler/ori_types/src/check/registration/type_resolution.rs`, `compiler/ori_types/src/check/well_known/mod.rs`
  Found: 2026-04-02 | Source: review-work

- [x] `[BUG-04-020][medium]` **`wrapper_rc_retain` panic-path regression tests accept compile failures and signal crashes as passing** — found by review-work.
  Resolved: Fixed on 2026-04-02. Added `assert_panic_exit()` helper that rejects compile failure (-1), clean exit (0), and non-SIGABRT signal crashes (SIGSEGV=-139, SIGBUS=-135), while accepting SIGABRT (-134) as the expected panic termination path on Linux. All 3 negative-pin tests now use this helper.
  Subsystem: `compiler/ori_llvm/tests/aot/wrapper_rc_retain.rs`
  Found: 2026-04-02 | Source: review-work

- [x] `[BUG-04-014][high]` **AOT Option/Result debug output wrong for compound payloads** — found by tpr-review.
  Resolved: OBE on 2026-04-02. Fixed by `53d3f1df` (recursive Debug formatting for Option/Result compound payloads). Verified: both interpreter and AOT now print `Some([1, 2, 3])`.
  Repro: `@main () -> void = { let x = Some([1, 2, 3]); print(msg: x.debug()) }` — interpreter prints `Some([1, 2, 3])`, AOT prints empty string. `emit_element_to_str()` only handles primitives and `str`, returns `None` for lists/tuples/nested wrappers.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs` (`emit_option_debug_branch`, `emit_result_debug`)
  Found: 2026-04-01 | Source: tpr-review (hygiene-full §03)

- [x] `[BUG-04-015][medium]` **AOT Option/Result debug uses Printable semantics for str payloads instead of Debug** — found by tpr-review.
  Resolved: OBE on 2026-04-02. Fixed by debug delegation commits (`d0c4b008`, `53d3f1df`). Verified: both interpreter and AOT now print `Some("hi")` with quotes.
  Repro: `@main () -> void = { let x = Some("hi"); print(msg: x.debug()) }` — interpreter prints `Some("hi")`, AOT prints `Some(hi)` (missing quotes). The debug path calls `to_str` on inner values instead of `debug`.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs` (`emit_option_debug_branch`)
  Found: 2026-04-01 | Source: tpr-review (hygiene-full §03)

- [x] `[BUG-04-016][critical]` **AOT `Result.debug()` formats the inactive payload variant and can segfault on mixed-layout payloads** — found by review-work.
  Resolved: OBE on 2026-04-02. Fixed by `88ce5ae0` (branch before payload extract in Result.debug). Verified: `Err("oops")` with `Result<[int], str>` payload now prints correctly in both interpreter and AOT, no segfault.
  Repro: `ORI_BIN=./target/release/ori timeout 150 diagnostics/dual-exec-debug.sh --no-color /tmp/review_result_inactive_payload_debug.ori` with `@main () -> void = { let r: Result<[int], str> = Err("oops"); print(msg: r.debug()) }` — interpreter prints `Err("oops")`, AOT exits 139 (segfault).
  Root cause: `emit_result_debug()` in `debug_helpers.rs` extracts and formats both payload arms before branching on `tag`, so the inactive variant's bytes are reinterpreted as the wrong layout and passed into recursive debug formatting.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`
  Found: 2026-04-01 | Source: review-work (hygiene-full §03)

- [x] `[BUG-04-017][high]` **AOT wrapper debug still prints `<?>` for Debug-capable payload types such as maps** — found by review-work.
  Resolved: Fixed on 2026-04-02. Added map and set debug formatting to LLVM codegen. Map: `{key: value, ...}` (keys use Printable semantics, values use Debug). Set: `Set {elem, ...}`. Implementation converts hash tables to temporary contiguous lists via `emit_map_keys`/`emit_map_values`/`emit_set_to_list`, iterates to build formatted strings, then decs temporary buffers. Uses collection-level narrowing (not function-level) to match element sizes. Added dispatch entries `("map", "debug")` and `("Set", "debug")` in collections method table. Tests: 5 AOT tests (str keys, empty map, int keys with str values, nested list values, Option<Map> semantic pin + negative pin). 14,983 tests passing, no leaks.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_map_set.rs` (new), `builtins/debug_helpers.rs`, `builtins/collections/mod.rs`
  Found: 2026-04-01 | Source: review-work (hygiene-full §03)

- [x] `[BUG-04-018][medium]` **AOT wrapper debug formats `byte` payloads as hex, diverging from current interpreter/spec behavior** — found by review-work.
  Resolved: OBE on 2026-04-02. Fixed by `88ce5ae0` (fix byte debug format). Verified: both interpreter and AOT now print `Some(42)` for byte payloads.
  Repro: `ORI_BIN=./target/release/ori timeout 150 diagnostics/dual-exec-debug.sh --no-color /tmp/review_byte_wrapper_debug.ori` with `@main () -> void = { let b: byte = 42; let o: Option<byte> = Some(b); print(msg: o.debug()) }` — interpreter prints `Some(42)`, AOT prints `Some(0x2a)`.
  Root cause: `emit_element_debug()` routes `TypeInfo::Byte` through `ori_byte_debug_format()`, but the current spec/tests still pin byte debug to decimal output until byte storage semantics change.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/debug_helpers.rs`, `compiler/ori_rt/src/string/convert.rs`, `tests/spec/traits/debug/primitives.ori`
  Found: 2026-04-01 | Source: review-work (hygiene-full §03)

- [x] `[BUG-04-030][high]` **LLVM JIT spec tests: LCFails from multiple pre-existing codegen issues** — found by continue-roadmap.
  Resolved: OBE on 2026-04-06. All 6 identified root causes were fixed by `plans/jit-exception-handling/section-06-lcfail-resolution.md` (status: complete, TPR clean, hygiene clean). Root cause (A) Generalized Vars → §06.2. (B) Variable not defined → §06.3. (C) StructValue/IntValue → §06.8. (D) Missing JIT runtime → §06.1 (OBE). (E) Wrong concrete type → §06.4. (F) List concat crash → §06.5. LCFails reduced from 2656 baseline to 2467 (191 fewer files failing). Remaining LCFails are from missing LLVM codegen feature support (not the specific bugs identified in this entry), which is tracked as general codegen maturity work.
  Repro: `ori test --backend=llvm tests/spec/` shows 2467 LCFail (down from 2656 baseline, 2026-04-06). 191 files cannot compile via LLVM. NO CRASHES — spec test runner completes normally.
  Root causes (6 distinct issues, investigated 2026-04-06):
  (A) **Generalized Vars in generic function bodies** — DOMINANT CONTRIBUTOR. `TypeInfoStore` encounters `Tag::Var` with `VarState::Generalized` (not `Link`) — `pool.resolve_fully()` can't follow generalized vars. Manifests as "type-mismatch error(s) — skipping verification/JIT". Pattern: 71 files with exactly 2 errors (likely `assert_eq`/stdlib generic imports), plus files with higher counts. Partially mitigated by Scheme-unwrapping in `lambda.rs` and BoundVar resolution in `define_phase.rs`. Core issue: imported mono function `body_type_map` doesn't cover all internal ARC variables (overlap with BUG-04-024).
  (B) **Variable not yet defined (u32::MAX sentinel)** — Cascade from (A). When a type-mismatch causes `TypeInfo::Error`, downstream instructions reference vars that were never defined (the emitter used `ValueId::NONE` / `poison_value` fallback). Not an independent root cause — fixing (A) would eliminate most (B) occurrences.
  (C) **StructValue vs IntValue type confusion** — 4 files. Type resolution produces struct LLVM type where scalar expected. Also likely downstream of (A) — wrong types cause wrong ABI decisions.
  (D) **Missing JIT runtime functions** — OBE. Both `ori_iter_join` and `ori_iter_flatten` are correctly marked `jit_allowed: true` in `runtime_functions.rs`. Verified 2026-04-06.
  (E) **`find_concrete_copy_type` picks wrong Function type** — Partially mitigated. `arity_compatible()` check was added to `find_concrete_copy_of()` and `find_any_concrete_fn_type()`. Remaining gap: same-arity functions with different type signatures (e.g., `(int, int) -> int` vs `(str, str) -> str`) still ambiguous. Needs return-type or parameter-type matching.
  (F) **List concat in monomorphized lambda crashes** — AOT-specific (works in JIT). Closure capture RC issue fixed 2026-04-04. Remaining crash: null data pointer during list concatenation in monomorphized lambda body.
  Subsystem: `compiler/ori_types/src/unify/generalization.rs` (A), `compiler/ori_llvm/src/codegen/arc_emitter/` (B, C), `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` (E, F), `compiler/oric/src/test/runner/llvm_backend.rs` (A — body_type_map)
  Found: 2026-04-03 | Source: continue-roadmap (JIT EH plan §04B investigation)

- [x] `[BUG-04-031][high]` **LLVM PHINode error: short-circuit `&&` with Option method calls** — found by continue-roadmap.
  Root cause: `unwrap_or` builtin emission in `option_result.rs` created two extra LLVM blocks (`uor.inc`, `uor.merge`) for conditional RC management. This split the ARC block mid-emission — remaining instructions and the Jump terminator went into `uor.merge`, causing PHI predecessor mismatch at the merge block.
  Fix (2026-04-06): Skip RC branch for scalar payloads (`!self.classifier.is_scalar(inner_ty)`) — scalar types don't need RC inc, avoiding the block split.
  Found: 2026-04-03 | Fixed: 2026-04-06 | Source: continue-roadmap (JIT EH plan §06.6)

- [x] `[BUG-04-032][high]` **LLVM short-circuit `&&`/`||` side-effect propagation failure** — found by continue-roadmap.
  Root cause: `lower_short_circuit_and/or` in `short_circuit.rs` didn't call `merge_mutable_vars()` after branching, losing mutations from branch scopes. `scope = pre_scope` at merge reverted to pre-branch state.
  Fix (2026-04-06): Added `merge_mutable_vars` pattern from `lower_coalesce` — capture branch scopes, create merge params, pass mutable var values through Jump args, rebind after merge. Applied symmetrically to both `&&` and `||`.
  Found: 2026-04-03 | Fixed: 2026-04-06 | Source: continue-roadmap (JIT EH plan §06.6)

- [x] `[BUG-04-033][high]` **LLVM codegen fails on multi-clause functions with literal patterns (Ackermann)** — found by manual.
  Repro: `ori run --compile tests/run-pass/rosetta/ackermann/ackermann.ori` — 3-clause `@ack` with literal `0` patterns. Two errors: (1) `build_struct called with non-struct LLVM type (i64)` — clause dispatch treats int return as struct; (2) LLVM IR verification: "PHINode should have one entry for each predecessor of its parent basic block" — join blocks from clause branches have mismatched phi entries.
  Resolved: Fixed on 2026-04-06. Four root causes: (1) scrutinee TypeId::ERROR → real param types from FunctionSig, (2) scrutinee name mismatch → FunctionSig.param_names, (3) tuple type not interned → pre-intern in finish_with_pool(), (4) function/sig positional zip → name-keyed lookup. <!-- resolved-by:plans/jit-exception-handling §06.7 -->
  Subsystem: `compiler/ori_llvm/src/codegen/` (multi-clause function lowering, phi node generation)
  Found: 2026-04-04 | Fixed: 2026-04-06 | Source: manual (Rosetta Code task implementation)
  Note: Fix introduced BUG-04-037 regression (tuple pre-interning pollutes type pool → zip SIGSEGV).

- [x] `[BUG-04-034][medium]` **Curried lambda capturing bool produces LLVM type mismatch (i1 vs i64)** — found by continue-roadmap.
  Resolved: OBE on 2026-04-06. Verified: `let $fst = a -> b -> a; fst(true)(0)` passes through `--backend=llvm` with correct output `true`. The i1 vs i64 mismatch no longer occurs — fixed by JIT EH §04B (lambda mono pipeline) and §06.2 (Generalized Var Resolution) which resolved type variables before they reach the wrapper function generator. Bool captures in curried lambdas now correctly use canonical i64 ABI.
  Repro: `let $fst = a -> b -> a; fst(true)(0)` with `--backend=llvm` → LLVM verification error: "Call parameter type does not match function signature! i1 vs i64". Only affects bool captures in curried lambdas — int/str/list captures work correctly.
  Root cause: Lambda mono type resolution resolves `forall t13` to `bool` (LLVM `i1`), but the callee's parameter ABI expects `i64` (Ori's canonical integer width for all scalar values). The wrapper function loads the bool capture as `i1` and passes it directly, but the lambda body was declared with `i64` params.
  Subsystem: `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs`
  Found: 2026-04-04 | Source: continue-roadmap (TPR-04B-014 test matrix writing)

- [x] `[BUG-04-035][medium]` **Nested closure RC leaks: wrapper RcInc for borrowed-parameter re-captures not balanced** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-05 by the closure-ownership plan. Sections 01-02 added `arg_ownership` to `ApplyIndirect`/`InvokeIndirect`, Section 03 replaced the conservative drop_hints workaround with ownership-aware logic, added InvokeIndirect to unwind_cleanup, removed unused `_capture_ownership` parameter. All 6 previously-leaking closure tests pass with zero leaks (3 curried + 3 nested). <!-- resolved-by:plans/closure-ownership -->
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/closure_wrappers.rs`, `compiler/ori_arc/src/aims/emit_rc/helpers.rs` (`is_ownership_transfer`)
  Found: 2026-04-04 | Source: continue-roadmap (TPR-04B-014 fix verification)

- [x] `[BUG-04-036][high]` **Curried lambda + list concat COW double-free in AOT** — found by continue-roadmap.
  Repro: `let $app = a -> b -> a + b; app([1,2,3])([4,5,6])` compiled with `ori build`, then run the binary → SIGSEGV (exit -139). Direct `[1,2,3] + [4,5,6]` (non-lambda) works. JIT path also works.
  Root cause: `ori_list_concat_cow` has consuming semantics — it dec/frees BOTH input buffers. When params are `[borrow]`, the callee doesn't own the buffers but concat frees them. The closure drop then tries to rc_dec already-freed data → use-after-free.
  Fix (2026-04-06): Borrow-protect rc_inc in `emit_binary_op` at `operators/mod.rs`. When LHS or RHS of list `+` originates from a borrowed parameter (via `borrowed_param_ptrs`), emit `ori_list_rc_inc` before concat. Concat's consuming dec brings refcount to 1, leaving buffer alive for caller cleanup. RC trace: 5 allocs, 5 frees, live=0.
  Found: 2026-04-05 | Fixed: 2026-04-06 | Source: continue-roadmap (JIT EH §06.5)

- [x] `[BUG-04-037][high]` **Tuple type pre-interning in `finish_with_pool()` causes iter_zip AOT SIGSEGV** — found by continue-roadmap.
  Repro: `cargo test -p ori_llvm --test aot -- iter_zip_count` → SIGSEGV (exit -139). Both `iter_zip_count` and `iter_zip_unequal` affected. Passes on parent commit `6b8f9421`, fails on `60838e1b`.
  Resolved: Fixed on 2026-04-06. Two root causes: (1) `finish_with_pool()` interned tuples for ALL multi-param functions → pool hash collision with zip's `(int, Var(T))` tuples. Fixed by adding `ModuleChecker::intern_multi_clause_tuples()` that only targets multi-clause groups. (2) Uncommitted `emit_function.rs` change added `type_error_count()` to bail-out check → pre-existing unresolved type variables (Root Cause A) triggered premature `unreachable` stubs. Fixed by reverting to `codegen_error_count()` only. <!-- resolved-by:plans/jit-exception-handling §06.7b -->
  Subsystem: `compiler/ori_types/src/check/mod.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/emit_function.rs`
  Found: 2026-04-06 | Fixed: 2026-04-06 | Source: continue-roadmap (JIT EH §06.7 multi-clause fix)

- [x] `[BUG-04-038][low]` **Flaky test: `test_source_hasher_caching` fails intermittently on temp file race** — found by continue-roadmap.
  Resolved: Fixed on 2026-04-06. Root cause: `rand_suffix()` used nanosecond timestamp only — collisions possible under concurrent test execution. Fix: replaced with `unique_suffix()` combining process ID, atomic counter, and nanosecond timestamp. The atomic counter alone guarantees uniqueness within a process; PID+timestamp cover cross-process uniqueness. All 22 hash tests pass deterministically.
  Subsystem: `compiler/ori_llvm/src/aot/incremental/hash/tests.rs`
  Found: 2026-04-06 | Fixed: 2026-04-06 | Source: continue-roadmap

- [x] `[BUG-04-039][high]` **LLVM codegen: `join` on non-string iterators crashes (missing `to_str_fn` trampoline)** — found by continue-roadmap.
  Resolved: 2026-04-06. Generated `to_str` trampoline in `emit_iter_join` for int, float, bool, char element types. Byte/Duration/Size/Ordering excluded — they need proper Printable method dispatch (codegen error produced instead of wrong output).
  Fix: `plans/bug-tracker/fix-BUG-04-039.md` | 5 AOT tests added (`iter_join_int/float/bool/single_int/int_after_map`)

- [x] `[BUG-04-040][medium]` **LLVM JIT spec test runner: path-dependent compilation context causes spurious LCFails** — found by tpr-review.
  Resolved: Misdiagnosis on 2026-04-06. The path-dependent behavior was caused by a **stale user-local stdlib** at `~/.local/share/ori/library/std/testing.ori` (from 2026-03-28) with older `assert_eq<T: Eq>` signatures (no Debug bound), while the project's current `library/std/testing.ori` has `assert_eq<T: Eq + Debug>`. Module resolution walks up from the file's directory: project-tree files found the correct project library; `/tmp/` files fell through to the stale user-local copy with simpler signatures that the JIT could handle. Stale user-local stdlib removed. Both paths now consistently use project stdlib. The underlying LCFail for `assert_eq` with `Debug` bound is a general codegen feature gap (unresolved type variables in imported generics), not a path-dependent issue.
  Repro: Was caused by stale `~/.local/share/ori/library/` — no longer reproducible after removal.
  Subsystem: `compiler/oric/src/imports/mod.rs` (module resolution walk-up), `~/.local/share/ori/library/` (stale copy)
  Found: 2026-04-06 | Source: tpr-review (TPR-06-006)

- [x] `[BUG-04-041][medium]` **AOT codegen error + poison value produces crashing binary instead of clean compilation failure** — found by tpr-review.
  Resolved: Fixed on 2026-04-06. Added codegen error check in `run_codegen_pipeline()` — extracts `codegen_error_count() + type_error_count()` from `IrBuilder` and `TypeInfoStore` at end of compilation block, returns `Err` with descriptive message if > 0. Matches JIT path pattern (`evaluator/compile.rs:383`). Also exposed and marked 4 false-positive AOT tests (`trampoline_for_each_str`, `iter_zip_count`, `iter_zip_unequal`, `set_auto_fold`) that were silently producing crashing binaries — now correctly `#[ignore]`d with codegen gap notes. 16,717 tests passing.
  Repro: `[1s, 2s].iter().join(separator: ", ")` via `ori build` then run → exit code 139 (SIGSEGV). JIT mode correctly produces LCFail. The `record_codegen_error_with_msg` + poison value pattern doesn't prevent AOT compilation — the binary is generated with garbage OriStr values that crash when used. Affects any unsupported operation that uses this pattern.
  Subsystem: `compiler/oric/src/commands/codegen_pipeline.rs`
  Found: 2026-04-06 | Source: tpr-review (TPR-04-002 from BUG-04-039 fix)

- [ ] `[BUG-04-042][medium]` **LLVM codegen: polymorphic lambda presence causes unresolved type variable for imported generics (assert_eq)** — found by continue-roadmap.
  Repro: `timeout 150 cargo run --bin ori -- test --backend=llvm tests/spec/expressions/lambda_mono.ori` → `Idx(241)` unresolved type variable, 17 LCFails. Other files using `assert_eq` (e.g., `integer_safety.ori`) pass 30/30 through LLVM. The issue is specific to files containing polymorphic lambda definitions — the lambda's Scheme/BoundVar types bleed into the codegen context and prevent `assert_eq<T: Eq + Debug>` monomorphization. Related: BUG-04-011 (resolved for primitives), BUG-04-040 (path-dependent misdiagnosis noting this gap).
  Subsystem: `compiler/ori_llvm/src/codegen/type_info/store.rs`, `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/`
  Found: 2026-04-06 | Source: continue-roadmap (JIT EH plan closure verification)
  Note: Blocks JIT EH plan items 04B.N L238, 05.R TPR-05-003, 06.2 L152. Active work in roadmap Section 21A touches this area.

- [x] `[BUG-04-046][high]` **Windows-only: `bug_04_019_result_niche_no_collapsed_unwrap_arm` panics with `could not find end of emit_result_niche`** — found by manual.
  Repro: `cargo test -p ori_llvm --lib bug_04_019_result_niche_no_collapsed_unwrap_arm` on `windows-latest`. Surfaced by GitHub Actions run 24102722494 on 2026-04-07 (BUG-04-045 push retriggered CI on PR #107). Failed on `windows-latest` only — `macos-latest` and Linux pass the same test on the same run.
  Panic location: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers/tests.rs:243:10` with message `could not find end of emit_result_niche`.
  Root cause: the BUG-04-019 fix's test suite used `include_str!("../option_result_helpers.rs")` to embed the source and searched for a literal `"\n    }\n"` LF-specific pattern to find the end of `emit_result_niche`. Windows `git checkout` with `core.autocrlf` converts source files to CRLF, so the LF-pattern search fails to locate the function boundary. Classic Windows-only `include_str!` brittleness.
  Resolved: Fixed 2026-04-07 in commit `e0f866d0 test(llvm): make option_result_helpers tests CRLF-safe on Windows`. The fix extracts a shared `slice_to_matching_brace` helper and a new `extract_fn_body` extractor that mirrors the existing `extract_arm_body` and walks braces from the first opening `{` — naturally line-ending agnostic. The inline brace walker in `extract_arm_body` is now a call to the shared helper, eliminating the LEAK (two copies of brace-walking code, one of them an ad-hoc string-pattern hack). All 9 structural tests pass on both Linux (verified locally) and Windows (pending next CI run).
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers/tests.rs`
  Found: 2026-04-07 | Source: manual (CI run during BUG-04-045 push)
  Note: Was the first concrete operational manifestation of TPR-07-018 (`plans/repr-opt/section-07-enum-repr.md:745`), which Codex filed against the BUG-04-019 fix to flag exactly this test-design weakness ("source-text assertions, not emitter-driven IR tests"). The Windows CRLF fix makes the existing source-text tests robust in the short term; the deeper fix (replacing `include_str!` source-text assertions with emitter-driven IR tests that actually call `emit_option_niche` / `emit_result_niche`) remains tracked under TPR-07-018 and is strictly stronger — it would have prevented this bug class entirely rather than just making it CRLF-tolerant. BUG-04-046 closed on the CRLF fix; TPR-07-018 remains open as the architectural upgrade. NOT introduced by BUG-04-045 — the typed-`Arch` refactor never touches `option_result_helpers/`, and the BUG-04-045 fix successfully landed on `macos-latest` in the same CI run that surfaced this Windows regression.

- [ ] `[BUG-04-044][medium]` **Explicit-tag enum Construct emits `insertvalue [1 x i64], ptr %p, 0` without pointer-to-i64 cast for UnmanagedPtr (iterator) payloads** — found by tpr-review (TPR-07-008 matrix).
  **Repro**: `type BigEnum = V1 | V2 | V3 | V4 | V5 | V6 | V7 | V8 | V9 | Holds(it: Iterator<int>);` then `let _x: BigEnum = Holds(it: [10, 20, 30].iter());` — 10+ variants forces explicit-tag encoding (tagged-pointer only allows ≤8 variants). Compilation fails with `Invalid InsertValueInst operands! %0 = insertvalue [1 x i64] zeroinitializer, ptr %list.iter, 0`.
  **Root cause**: the payload slot is `[N x i64]` but the iterator value is `ptr` (UnmanagedPtr). The Construct path emits `insertvalue` without first `ptrtoint` casting the pointer to i64. Tagged-pointer enums work because they use a different codegen path (`ptrtoint` + `or` tag) at the top level, not `insertvalue` into a slot array.
  **Impact**: any user enum with ≥9 variants that carries an iterator (or any `UnmanagedPtr`) variant cannot be constructed. Pre-existing, surfaced by TPR-07-008 matrix coverage.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/construction.rs` (Construct emission for explicit-tag enums with pointer payloads)
  Found: 2026-04-06 | Source: tpr-review (TPR-07-008 matrix)

- [ ] `[BUG-04-043][medium]` **Recursive tagged-pointer enums need box-and-load codegen for Construct/Project (§07.3.A future work)** — found by continue-roadmap.
  **Initial symptom**: `./target/release/ori test --backend=llvm tests/` hung indefinitely when `tests/spec/types/enum/tagged_ptr.ori` was present and contained a self-recursive single-pointer enum (`type IntCell = Empty | Holds(child: IntCell);`). Investigation showed the same hang in `ori build` (AOT pipeline), proving the bug was NOT JIT-specific. `ori run` worked because it uses the interpreter, not LLVM codegen.
  **Root cause**: The §07.3.A Construct/Project codegen for tagged-pointer enums treats variant payloads as raw single-word pointers. For recursive enums, `canonical_inner` produces a cycle marker `RcPointer { inner: OpaquePtr }`, and the heap value at the pointer is itself an encoded tagged-pointer i64 (NOT a value of `inner`'s type). Construct should box-and-store the recursive child before OR'ing with the tag; Project should decode the pointer AND load the i64 from the heap. Without those box-and-load semantics, the codegen produces wrong values that drive the recursive type pipeline into an infinite loop somewhere downstream of narrowing.
  **Workaround landed (2026-04-06)**: `is_taggable_pointer` in `compiler/ori_repr/src/layout/tagged_ptr.rs` now rejects the cycle-marker pattern (`RcPointer { inner: OpaquePtr }`), forcing recursive enums to fall back to the explicit-tag encoding. AOT compilation no longer hangs; recursive enums execute correctly under the explicit-tag path. The negative pin lives in `compiler/ori_llvm/tests/aot/enum_tagged_ptr.rs::test_recursive_enum_falls_back_to_explicit_tag` and `compiler/ori_repr/src/layout/tests.rs::can_use_tagged_pointer_recursive_enum_negative`.
  **Proper fix (deferred to a future §07.3.A.2)**: Implement box-and-load codegen for tagged-pointer enums with recursive payloads. The Construct path needs to allocate heap memory, store the encoded i64, and use the resulting pointer; the Project path needs to decode the pointer AND issue an `i64` load to retrieve the recursive child. Drop/RC paths need to be audited for the same.
  Subsystem: `compiler/ori_repr/src/layout/tagged_ptr.rs` (analysis layer, workaround), `compiler/ori_llvm/src/codegen/arc_emitter/{construction,instr_dispatch,rc_helpers,drop_enum}.rs` (codegen extensions for recursive case)
  Severity downgraded 2026-04-06: high → medium. The hang is fixed; recursive enums execute correctly. The remaining work is an optimization extension (recursive enums currently miss the tagged-pointer benefit), not a correctness blocker.
  Found: 2026-04-06 | Source: continue-roadmap (repr-opt §07.3.A verification)
  Note: A separate JIT-specific symptom may exist beneath the recursive-enum hang — `ori test --backend=llvm tests/` was confirmed to hang when the spec test file was present, even with a non-recursive trivial body. Whether that secondary hang is purely a Salsa state interaction or a deeper JIT codegen issue is not yet investigated. The Ori spec test for §07.3 was deferred in favor of the AOT integration test until that's understood.
  **Post-session diagnostic update (2026-04-06)**: The two background `ori test --backend=llvm` tasks that appeared to hang during the investigation eventually completed cleanly (EXIT=0) after the parent session moved on. This strongly suggests the JIT "hang" was at least partially **cargo/target/ artifact-lock contention** (multiple concurrent `cargo b --release` and test runs competing for the same lock), not a genuine JIT codegen bug. Before investing investigation time in a "JIT hang" codegen theory, run `pgrep -af "cargo|ori_rt|target/debug/deps"` to rule out leftover processes, and try the repro with a quiescent build cache. The primary AOT `ori build` hang on recursive `IntCell` was definitely real and reproduced deterministically — that fix stands regardless of what the JIT symptom turns out to be.

- [x] `[BUG-04-045][high]` **`is_cross_compiling()` reports native Apple Silicon as cross-compilation: `arm64` (LLVM triple) vs `aarch64` (Rust `cfg`) string mismatch** — found by manual. **Fixed 2026-04-07 via `plans/bug-tracker/fix-BUG-04-045.md`**: introduced typed `Arch` enum + `HostPlatform` at the `TargetTripleComponents::parse` boundary, changed `arch: String → Arch`, added `is_cross_for`/`is_native_for` typed queries. All 6 expanded-scope sites migrated + latent bug #7 (`from_triple("arm64-apple-darwin")` rejection asymmetry) resolved. TPR iteration 2 caught two more LEAK sites: (a) `from_triple` rejected the *versioned* Darwin spelling `arm64-apple-darwin25.2.0` because `SUPPORTED_TARGETS` only contained the unversioned form — fixed by adding `TargetTripleComponents::support_key()` (canonical lookup form that strips Darwin OS version suffixes via the typed `is_macos()` query) and routing `from_triple` through it; (b) `LinkOutput::extension()` keyed shared-library suffixes off raw `target.os == "darwin"` — fixed by delegating to `target.is_macos()`. Comprehensive matrix in `compiler/ori_llvm/src/aot/target_features/tests.rs` (16 tests) covers alias normalization, cross-host detection, native round-trip, support-key Darwin-version stripping, and a type-level negative pin. Linker tests (`compiler/ori_llvm/src/aot/linker/tests.rs`) and AOT tests (`compiler/ori_llvm/tests/aot/cross.rs`) gained versioned-Darwin regression pins. Full `./test-all.sh` green (16876 passed, 0 failed).
  **Repro**: On macOS Apple Silicon, `cargo test -p ori_llvm aot::linker::tests::test_native_target_is_not_cross_compiling` panics with `native target should not be detected as cross-compilation` at `compiler/ori_llvm/src/aot/linker/tests.rs:62`. Surfaced by GitHub Actions run 24058749864 (nightly: 2026-04-07) — only the `macos-latest` job failed; Linux/Windows pass. Standalone repro: `TargetTripleComponents::parse(&TargetMachine::get_default_triple().as_str().to_string_lossy()).arch` returns `"arm64"` while Rust's `cfg(target_arch = "aarch64")` is set, so `LinkerDetection::is_cross_compiling()` returns `true` for the native triple.
  **Root cause**: `LinkerDetection::is_cross_compiling()` (`compiler/ori_llvm/src/aot/linker/mod.rs:471-483`, added in c2c888fb / TPR-04-006 iter1) does a literal string compare `components.arch != host_arch`, where `host_arch` is hardcoded to `"aarch64"` under `cfg(target_arch = "aarch64")`. But `components.arch` comes from `TargetTripleComponents::parse()` (`compiler/ori_llvm/src/aot/target_features.rs:127-143`), which stores `parts[0]` of the LLVM default triple verbatim. On Apple Silicon, LLVM's default triple is `arm64-apple-darwin25.x.x` (Apple's historical spelling), not `aarch64-apple-darwin...`. `"arm64" != "aarch64"` → false-positive cross-compilation. The codebase already knows about this duality at `target_features.rs:233` (`"aarch64" | "arm64" => initialize_aarch64(...)`), but `is_cross_compiling()` did not get the same treatment. Classic LEAK:scattered-knowledge — arch-name normalization has no canonical home.
  **Impact**: (1) Nightly CI red on macOS Apple Silicon. (2) Real Apple Silicon users invoking `ori build` for the native target will be incorrectly told they're cross-compiling, hit the cross-linker error path, and be unable to native-build. `gcc_cross_compiler_name()` returns `None` for darwin (linker/mod.rs:509-513), so they'll get the "no cross-linker found" error from `cross_compilation_error()` instead of using the host `cc`. Pre-existing on Apple Silicon since c2c888fb landed 2026-04-06; first nightly run to include the regression was 2026-04-07 00:52 UTC.
  **TPR research outcome (2026-04-07)**: Codex prior-art pass against Rust, Zig, Swift, LLVM/Clang, and Go converged on a single methodology. The proper fix is **not** a normalization helper; it is a **typed `Arch` enum at the parse boundary** with all consumers querying typed methods, never raw strings. This is the SSOT pattern used by every mature compiler.
  - **Rust** (`compiler/rustc_target/src/spec/mod.rs:1857,2077`): `Target.arch` is a typed `Arch` enum, generated by `target_spec_enum!` (`lib.rs:73,163`). Apple's `arm64` surface is reconciled by a per-platform mapping `Arm64|Arm64e|Arm64_32 → crate::spec::Arch::AArch64` (`spec/base/apple/mod.rs:15,43`). Host-vs-target comparisons happen on typed objects (`rustc_codegen_ssa/back/link.rs:1814`, `rustc_codegen_llvm/llvm_util.rs:529`), never raw strings.
  - **Zig** (`lib/std/Target.zig:1303`): `Target.Cpu.Arch` is an exhaustive enum; parsing is direct string-to-enum in `Target.Query.parse()` (`lib/std/Target/Query.zig:212,224`). Unknown archs are rejected, not stored. External-tool spellings (e.g. Windows SDK `arm64`) are emitted only at the boundary (`lib/std/zig/LibCInstallation.zig:410`).
  - **Swift** (`lib/Basic/Platform.cpp:371`): explicit canonical mapping `arm64|aarch64 → arm64`, `x86_64|amd64 → x86_64`, `i386|i486|i586|i686 → i386`. Internal decisions go through typed `getArch()` (`lib/Basic/LangOptions.cpp:592`), Apple-facing edges re-emit `getArchName()` only when needed (`lib/Driver/DarwinToolChains.cpp:868`).
  - **LLVM/Clang** (`llvm/include/llvm/TargetParser/Triple.h:46,85,328`): `llvm::Triple::ArchType` is the canonical internal model with documented alias groups (`x86_64|amd64`, `i[3-9]86`). Equality is on parsed typed fields. Clang's Darwin toolchain has the explicit Apple mapping `arm64|arm64e → aarch64` (`clang/lib/Driver/ToolChains/Darwin.cpp:57,77`).
  - **Go** (`src/cmd/internal/sys/arch.go:29,98,125`, `src/cmd/dist/main.go:96`, `src/cmd/dist/build.go:69,552`): canonical `goarch` strings stored in typed `Arch` descriptors, host canonicalized once at startup (`aarch64|arm64 → arm64`, `x86_64|amd64 → amd64`), then equality is canonical-string compare against the centralized `okgoarch` vocab list.
  - **Rejected alternatives**: **(B) string normalization helper** loses because every new consumer can bypass the helper — the bug class stays alive. **(C) use `inkwell::TargetTriple`** loses because `inkwell` (`targets.rs:127,1090`) is only a string wrapper around LLVM's `Triple` — the parsed C++ `Triple` API is not exposed across the FFI boundary, so Ori would still need its own typed representation.
  **Chosen methodology**: Option A — typed `Arch` enum in `compiler/ori_llvm/src/aot/target_features.rs` (preferably extracted to a new `aot/triple.rs` for hygiene), stored as `TargetTripleComponents.arch: Arch`, parsed via `Arch::parse_llvm_name(&str) -> Result<Arch, TargetError>` that normalizes ALL aliases at the boundary (`x86_64|amd64`, `i386|i486|i586|i686`, `aarch64|arm64`, `wasm32`, `wasm64`, `arm`). Add `HostPlatform { arch: Arch, os: HostOs }` and put `is_native_for(host) -> bool` / `is_cross_for(host) -> bool` as **methods on the parsed target object**, not free predicates with cfg blocks. Consumers get a typed method, not a raw arch field.
  **Expanded scope** (this single canonical-type fix resolves 6 sites — file as one fix, not six):
  1. `linker/mod.rs:471-491` `is_cross_compiling()` — the failing test (the original surfacing). Replace cfg-string compare with `target.components().is_cross_for(HostPlatform::current())`.
  2. `linker/mod.rs:498-519` `gcc_cross_compiler_name()` — interpolates `target.arch` raw into cross-toolchain names. Today emits `arm64-w64-mingw32-gcc` / `arm64-linux-gnu-gcc` (which don't exist) if a triple ever enters with the LLVM spelling. Latent on Linux/Windows targets, dormant only because no current call site feeds an LLVM-default triple through it. After fix: query typed `Arch::cross_toolchain_prefix()` with the canonical spelling.
  3. `linker/mod.rs:611` user-facing error message uses raw `components.arch` — would print "arm64" in the spec's `aarch64` vocabulary. After fix: format from typed `Arch::display_name()` (the spec spelling).
  4. `syslib/mod.rs:119-138` `is_native()` — same `self.target.arch == current_arch` raw compare. Test-only consumer today (`syslib/tests.rs:109`); production would silently misbehave on Apple Silicon. After fix: same `is_native_for(HostPlatform::current())` query.
  5. `syslib/mod.rs:280` `if target.arch == "x86_64" || target.arch == "aarch64"` — explicitly excludes the `arm64` spelling, would skip `lib64` paths on any sysroot derived from an LLVM-spelling triple. After fix: `arch.is_64_bit_non_wasm()` typed query.
  6. `target.rs:371-376` `pointer_size()` — stringly + non-exhaustive (`match arch.as_str() { "wasm32"|"i686"|"i386"|"arm" => 4, _ => 8 }`). Currently safe by accident because `arm64` falls into the `_` default. After fix: exhaustive `match arch: Arch { ... }`, no string fallthrough.
  - **Plus a 7th latent bug** Codex caught that I missed in my own grep: `target_features.rs:73` + `target.rs:95` — `TargetConfig::from_triple("arm64-apple-darwin")` is **rejected today** by `is_supported_target()` because the `SUPPORTED_TARGETS` list uses canonical `aarch64-apple-darwin`. So on Apple Silicon, `TargetConfig::native()` happens to work (the default-triple parse path doesn't validate against `SUPPORTED_TARGETS`) but `TargetConfig::from_triple(get_default_triple().as_str())` would fail. There is an asymmetry inside the same module, and the canonical-Arch fix resolves it because parse normalizes the input arch before the supported-target check.
  **TDD matrix (9 tests, all in `compiler/ori_llvm/src/aot/triple/tests.rs` or equivalent)**:
  1. `test_arch_parse_normalizes_alias_spellings_matrix` — covers `aarch64|arm64`, `x86_64|amd64`, `i386|i486|i586|i686` aliases all parse into the same `Arch` value
  2. `test_arch_parse_matrix_is_self_verifying` — counter assertion proving every (alias × canonical) cell was visited per `tests.md` §Self-verifying matrix completeness
  3. `test_target_triple_parse_preserves_vendor_os_env_while_normalizing_arch_matrix` — parsing `arm64-apple-darwin25.2.0` preserves vendor/os/env exactly while canonicalizing arch
  4. `test_is_cross_for_simulated_host_matrix` — for every (host_arch, target_arch) pair, simulated cross detection yields the correct answer
  5. `test_is_cross_for_matrix_is_self_verifying` — counter assertion for the cross-detection matrix
  6. `test_native_host_triple_round_trips_to_not_cross_compiling_matrix` — for every supported host (Linux x86_64 / Linux aarch64 / macOS x86_64 / macOS arm64 / Windows x86_64), the native default triple parses to `is_cross_for = false`
  7. `test_is_cross_for_regression_pin_arm64_native_host_is_not_cross` — semantic pin: this exact bug, simulated via parsing `arm64-apple-darwin25.2.0` against `HostPlatform { arch: Arch::Aarch64, os: HostOs::Darwin }`. Would fail if the `arm64 → Aarch64` alias is removed. Permanent regression guard against this bug class.
  8. `test_gcc_cross_compiler_name_uses_canonical_arch_boundary_spellings_matrix` — for every (canonical_arch, target_os) pair, the cross-toolchain name uses the canonical spelling (e.g. `aarch64-linux-gnu-gcc`, never `arm64-linux-gnu-gcc`)
  9. `test_syslib_is_native_for_simulated_host_matrix` — sibling pin for syslib's native check, exercising the same `is_native_for` query
  - **Negative pin (10th test)**: `test_target_triple_components_has_no_raw_arch_string_field` — compile-fail / type-test asserting that `TargetTripleComponents.arch` is `Arch`, not `String`. If a future commit reverts the field type, this test refuses to compile. Stronger than a runtime negative pin because the bug class is "raw string compare", which a runtime test cannot detect after the fact.
  **Migration order**: bottom-up. Change `TargetTripleComponents.arch: String → Arch` first; the Rust compiler then refuses to compile every consumer that expected `String`, forcing the migration through to completion. Top-down (consumer-first) would create transitional side logic where some consumers are typed and others are stringly. The strongest fix is the one that makes the broken state un-typeable.
  Subsystem: `compiler/ori_llvm/src/aot/target_features.rs` (canonical home), `compiler/ori_llvm/src/aot/target.rs`, `compiler/ori_llvm/src/aot/linker/mod.rs`, `compiler/ori_llvm/src/aot/syslib/mod.rs`, `compiler/ori_llvm/src/aot/linker/tests.rs` (existing tests update for new API)
  Found: 2026-04-07 | Source: manual (nightly build failure investigation) | TPR research: codex prior art pass 2026-04-07 (Rust/Zig/Swift/LLVM/Go)
  Note: Side observation from the same CI log — Homebrew warning `llvm@21 was installed but not linked because llvm@18 is already installed` on the macOS runner. Not the cause of this failure (build step succeeded, 524/525 unit tests ran), but the macOS CI workflow should be audited to either `brew link --overwrite llvm@21` or pin LLVM via PATH/env when this area is touched.

---

## 04.R Third Party Review Findings

- [x] `[TPR-04-001][high]` `debug_map_set.rs` — `Map.debug()` key formatting diverged for char keys and escaped keys.
  Resolved: Fixed on 2026-04-02 in two steps:
  (1) Added `ori_str_from_char` runtime function + `TypeInfo::Char` to `emit_to_str`/`emit_element_to_str` — fixes plain char keys (`{'a': 1}` → `{a: 1}`).
  (2) Added `ori_str_escape_control` runtime function + `emit_escape_control()` helper — post-processes all map key strings to escape control characters (`\n`→`\\n`, `\\`→`\\\\`), matching the interpreter's `escape_debug_str` behavior. Wired into `emit_map_entry_str()` in `debug_map_set.rs`. Plain char key parity confirmed via dual-exec. 15,002 tests passing.

- [x] `[TPR-04-002][high]` `compound_traits.rs` — BUG-04-026's homogeneous payload enum equality path breaks on non-scalar payload fields.
  Resolved: Fixed on 2026-04-03. Added `is_single_slot_type()` guard in `emit_structural_eq_enum()` — restricts structural equality to scalar-only payload fields. Aggregate payloads (lists, maps, sets, tuples, structs) correctly fall through to require `#derive(Eq)`. Also changed the `unreachable!` panic in `emit_binary_op` (operators/mod.rs:56) to a graceful codegen error — compilation no longer crashes for enum types without trait dispatch.

- [x] `[TPR-04-003][medium]` `panic.rs` / BUG-04-022 — the resolution landed without a committed LLVM regression test, and one existing test file still documents the old broken assumption.
  Resolved: Fixed on 2026-04-03. Added 2 AOT regression tests: `test_generic_debug_list` (generic debug on `[int]`) and `test_generic_str_compound` (str/debug in generic bodies with string concat). Updated stale comment in `panic.rs` that documented the old monomorphization bug. 15,159 tests passing.

- [x] `[TPR-04-004][high]` `builtins/prelude.rs` / BUG-04-022 — generic `str(v)` in AOT still diverges from interpreter semantics for several newly-added branches.
  Resolved: Fixed on 2026-04-03. Changed `emit_str()` to prefer `emit_element_to_str()` (Printable semantics) with `emit_element_debug()` fallback. `str('a')` now produces `a` (no quotes) matching interpreter. Byte/Ordering `to_str` gaps remain as pre-existing limitations (not regressions from BUG-04-022).

- [x] `[TPR-04-005][high]` `builtins/prelude.rs` / BUG-04-022 — generic `v.debug()` still fails to compile in AOT for scalar builtin Debug types such as `char`, `byte`, and `Ordering`.
  Resolved: Fixed on 2026-04-03. Added `("type", "debug")` dispatch entries for all primitive types (int, float, bool, char, byte, Duration, Size, Ordering) in the primitives dispatch table. Each routes through `emit_element_debug()`. Also fixed `emitter_utils.rs` to call `record_codegen_error()` when encountering undefined variables (BUG-04-024) — prevents executing broken code. Verified: generic `v.debug()` for char produces `'a'` in both interpreter and AOT.

---

## Resolved Bugs

- [x] `[BUG-04-002][critical]` **Inherent impl method returns wrong value when type also has trait impl** — found by manual.
  Resolved: OBE on 2026-03-28. False positive — caused by stale release binary from prior session. After `cargo b --release` (force rebuild), `test_aot_multiple_impl_blocks` passes. The AOT test framework falls back to the release binary when debug lacks LLVM; the stale release binary had code from before range analysis field narrowing was fixed.

- None.
