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

- [ ] `[BUG-04-001][high]` **Cross-compilation to Windows fails: host linker used instead of cross-linker** — found by manual.
  Repro: `ori build hello.ori --target=x86_64-pc-windows-msvc` on Linux host
  Error: `R_AMD64_IMAGEBASE with __ImageBase undefined` — GNU ld receives Windows COFF object
  Root cause: `LinkerFlavor::for_target()` correctly selects `Msvc`, but `LinkerDetection::is_available()` fails (no `link.exe`/`lld-link` on Linux), fallback cascades to host `cc`. Additionally, Linux-compiled `libori_rt.a` is linked with Linux system libraries (`-lgcc_s -lutil -lrt -lpthread -lm -ldl -lc`). Three issues: (1) no validation that cross-linker exists before attempting cross-compile, (2) no cross-compiled runtime for target, (3) system library selection ignores target OS.
  Subsystem: `compiler/ori_llvm/src/aot/linker/driver.rs`, `mod.rs` (fallback logic), `gcc.rs` (system libs)
  Found: 2026-03-28 | Source: manual
  Note: Also applies to `--target=x86_64-pc-windows-gnu` (needs `x86_64-w64-mingw32-gcc`).

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

- [ ] `[BUG-04-019][medium]` **Niche-encoded Option/Result extraction paths missing RC retain and tag guards** — found by tpr-review.
  Niche-encoded `Option.unwrap` (option_result_helpers.rs:44), `Result.unwrap`/`unwrap_err`/`unwrap_or` (option_result_helpers.rs:124), and `Result.expect`/`expect_err` (option_result_helpers.rs:127-131) all extract payload via raw `extract_value` without `inc_value_rc` or tag guards. **Not currently reachable**: niche encoding is disabled (`NICHE_CODEGEN_READY = false` in `ori_repr/src/canonical/type_repr.rs`). All Option/Result types use explicit i64 tags. This becomes relevant only when the niche encoding gate is enabled.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/builtins/option_result_helpers.rs`
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

- [ ] `[BUG-04-024][medium]` **ARC emitter "variable not yet defined var=2" in imported mono functions** — found by continue-roadmap.
  Repro: `ORI_LOG=ori_llvm=debug timeout 30 cargo run -- test --backend=llvm /tmp/test.ori` where test.ori uses `use std.testing { assert_eq }` and `assert_eq(actual: 42, expected: 42)`. ERROR log emitted consistently for every imported mono function compilation. Tests pass — emitter recovers via `ValueId::NONE` fallback.
  Root cause: Residual of BUG-04-011 layer 3. The `body_type_map` built from `per_module_cache` values + `scheme_var_ids` doesn't cover all internal ARC variables in the imported function's IR. Variable 2 (likely a comparison result or intermediate in `assert_eq`'s body) is referenced before being defined in the emitter's `var_map`. The substitution only covers types that appear in the per_module_cache (cross-module re-interned types), not purely-local variables created during ARC lowering.
  Subsystem: `compiler/ori_llvm/src/codegen/arc_emitter/emitter_utils.rs`, `compiler/oric/src/test/runner/llvm_backend.rs` (body_type_map construction)
  Found: 2026-04-02 | Source: continue-roadmap (imported-generic-mono verification)
  Note: Active work in `plans/imported-generic-mono/` directly related. Non-crashing — graceful recovery produces correct results for primitive type instantiations.
  Impact (2026-04-04): 6 narrowing AOT tests (`compiler/ori_llvm/tests/aot/fixtures/narrowing/`) used `use std.testing { assert_eq }` — calls silently dropped from IR. 4 tests SIGSEGV (block ends `unreachable` instead of `ret void`), 2 tests false-positive (pass without actually checking assertions). Converted all 6 to exit-code-based assertions as workaround.

- [ ] `[BUG-04-029][medium]` **LLVM backend missing shift overflow/negative count/bit width runtime checks** — found by continue-roadmap.
  Repro: `timeout 30 cargo run -q -p oric --bin ori -- test --backend=llvm tests/spec/expressions/operators_bitwise.ori` — 5 tests fail: `test_shl_overflow_panic`, `test_shl_bit_width_panic`, `test_shl_negative_count_panic`, `test_shr_bit_width_panic`, `test_shr_negative_count_panic`. All expect panics but operations succeed silently (UB in LLVM — shift by negative or >= bit width is poison).
  Root cause: `checked_ops.rs` implements checked div/rem (04.1 fix) but has no shift overflow/negative count/bit width guards. The interpreter has runtime checks; the LLVM backend emits raw `shl`/`ashr` without validation.
  Subsystem: `compiler/ori_llvm/src/codegen/ir_builder/checked_ops.rs`
  Found: 2026-04-03 | Source: continue-roadmap (JIT EH §04 verification)
  Note: Active work in roadmap section 15C (Literals & Operators) and 21A (LLVM Backend) touch this area.

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

- [ ] `[BUG-04-030][high]` **LLVM JIT spec tests: LCFails from multiple pre-existing codegen issues** — found by continue-roadmap.
  Repro: `ori test --backend=llvm tests/spec/` shows 2475 LCFail (down from 2656 baseline). NO CRASHES — spec test runner now completes normally (2026-04-06). Individual test files pass but complex files with many functions trigger failures.
  Root causes (4 distinct issues, all pre-existing before JIT EH work):
  (A) **Generalized Vars in generic function bodies** — type checker stores Unbound Vars in expr_types that get `VarState::Generalized` during let-polymorphism, breaking `VarState::Link` chains. These leak to codegen as unresolved types. Manifests as `ori_rc_dec({ i64, i64, ptr }, ptr)` type mismatch (passing struct instead of ptr). ~279 occurrences.
  (B) **Index out of bounds (u32::MAX)** — dominant pattern (~50+ files). `index out of bounds: the len is N but the index is 4294967295`. Likely a missing block/var ID sentinel from ARC IR emission producing `u32::MAX`.
  (C) **StructValue vs IntValue type confusion** — 4 files. LLVM emitter produces struct value where int value is expected. Likely a type resolution issue for parameter passing.
  (D) **Missing JIT runtime functions** — 2 files. `ori_iter_join` and `ori_iter_flatten` not marked `jit_allowed` in `RT_FUNCTIONS`.
  (E) **`find_concrete_copy_type` picks wrong Function type** — returns the FIRST concrete Function in `var_types` without matching against the target lambda's arity/type structure. When a module has multiple functions with different polymorphic lambda instantiations (e.g., `a -> b -> a + b` used with both `int` and `str`), the wrong concrete type is selected, causing type mismatches (str treated as int, outputs garbage numbers). Individual files pass; multi-function files fail. Found: 2026-04-03.
  (F) **List concat in monomorphized lambda crashes (segfault in AOT, works in JIT)** — `let $app = a -> b -> a + b; app([1, 2, 3])([4, 5, 6])` crashes in AOT with null pointer in `copy_nonoverlapping`. JIT execution succeeds. The closure capture RC issue (TPR-04B-014) was FIXED on 2026-04-04 — the capture-only repro (`a -> b -> a`) now works in all modes. The remaining crash is AOT-specific: list Add dispatch inside the monomorphized lambda body produces a null data pointer during concatenation. Found: 2026-04-03, partially fixed: 2026-04-04.
  Subsystem: `compiler/ori_types/src/unify/generalization.rs` (A), `compiler/ori_llvm/src/codegen/arc_emitter/` (B, C), `compiler/ori_llvm/src/codegen/runtime_decl/` (D), `compiler/ori_llvm/src/codegen/function_compiler/define_phase.rs` (E, F)
  Found: 2026-04-03 | Source: continue-roadmap (JIT EH plan §04B investigation)
  Note: Active work in JIT EH plan §04B and repr-opt plan touch this area. Root cause (A) is partially mitigated by Scheme-unwrapping fix in `ori_arc/src/lower/calls/lambda.rs` (polymorphic lambda params) and BoundVar resolution in `define_phase.rs` + `prepare.rs`. Root cause (D) is a trivial fix (add `jit_allowed: true` to the RT_FUNCTIONS entries). Root cause (E) needs structural matching — `find_concrete_copy_type` must verify the candidate Function's arity matches the lambda's param count. Fix in `define_phase.rs:find_partial_apply_concrete_type` path 1 added (validate params are concrete before returning), but `find_concrete_copy_type` still lacks arity matching.

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

- [ ] `[BUG-04-034][medium]` **Curried lambda capturing bool produces LLVM type mismatch (i1 vs i64)** — found by continue-roadmap.
  Repro: `let $fst = a -> b -> a; fst(true)(0)` with `--backend=llvm` → LLVM verification error: "Call parameter type does not match function signature! i1 vs i64". Only affects bool captures in curried lambdas — int/str/list captures work correctly.
  Root cause: Lambda mono type resolution resolves `forall t13` to `bool` (LLVM `i1`), but the callee's parameter ABI expects `i64` (Ori's canonical integer width for all scalar values). The wrapper function loads the bool capture as `i1` and passes it directly, but the lambda body was declared with `i64` params.
  Subsystem: `compiler/ori_llvm/src/codegen/function_compiler/lambda_mono/type_resolve.rs`
  Found: 2026-04-04 | Source: continue-roadmap (TPR-04B-014 test matrix writing)
  Note: Active work in JIT EH plan Section 04B touches this area.

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

- [ ] `[BUG-04-038][low]` **Flaky test: `test_source_hasher_caching` fails intermittently on temp file race** — found by continue-roadmap.
  Repro: `cargo test -p ori_llvm test_source_hasher_caching` — intermittent failure: `IoError { path: "/tmp/ori_hash_test_*.ori", message: "No such file or directory" }`. Race condition: temp file at `/tmp/ori_hash_test_*.ori` is cleaned up by OS or concurrent test before `SourceHasher` reads it.
  Subsystem: `compiler/ori_llvm/src/aot/incremental/hash/tests.rs:80`
  Found: 2026-04-06 | Source: continue-roadmap (pre-commit hook failure during hygiene-lexer work)

- [x] `[BUG-04-039][high]` **LLVM codegen: `join` on non-string iterators crashes (missing `to_str_fn` trampoline)** — found by continue-roadmap.
  Resolved: 2026-04-06. Generated `to_str` trampoline in `emit_iter_join` for int, float, bool, char, byte element types. Duration/Size/Ordering excluded from trampoline — they need proper Printable method dispatch (codegen error produced instead of wrong output).
  Fix: `plans/bug-tracker/fix-BUG-04-039.md` | 5 AOT tests added (`iter_join_int/float/bool/single_int/int_after_map`)

- [ ] `[BUG-04-040][medium]` **LLVM JIT spec test runner: path-dependent compilation context causes spurious LCFails** — found by tpr-review.
  Repro: Create a minimal file via heredoc (works in both bash and zsh):
  ```
  cat > /tmp/join_test.ori <<'EOF'
  use std.testing { assert_eq }
  @f () -> str = { ["a","b"].iter().join(separator: ", ") }
  @t tests @f () -> void = { assert_eq(actual: f(), expected: "a, b") }
  EOF
  ori test --backend=llvm /tmp/join_test.ori  # → 1 pass
  cp /tmp/join_test.ori tests/spec/traits/iterator/join_test.ori
  ori test --backend=llvm tests/spec/traits/iterator/join_test.ori  # → LCFail
  rm tests/spec/traits/iterator/join_test.ori  # cleanup
  ```
  Same content, different path, different result. Files under `tests/spec/` get a different compilation context that introduces unresolved type variables not present for standalone files.
  Subsystem: `compiler/oric/src/test/runner/llvm_backend.rs`
  Found: 2026-04-06 | Source: tpr-review (TPR-06-006)

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
