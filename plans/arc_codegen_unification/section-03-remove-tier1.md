# Section 03 — Remove Tier 1

## Remove feature flag

- Delete `use_arc_codegen: bool` field and `set_arc_codegen()` method
- Make `annotated_sigs` and `arc_classifier` required (not `Option`)
- Simplify `define_function_body()` dispatch
- Delete `define_function_body_tier1()`
- Delete `emit_closure_cleanup()` and `emit_single_closure_cleanup()`
- Delete `bind_parameters()` (keep `load_param_values()` for derive_codegen)

## Delete Tier 1 files (~25 files, ~11K lines)

- `codegen/expr_lowerer.rs`
- `codegen/lower_*.rs` (10 files)
- `codegen/lower_builtin_methods/` (8 files)
- `codegen/lower_collection_methods/` (4 files)
- `codegen/lower_iterator_trampolines.rs`
- `codegen/scope/mod.rs` + `tests.rs`

## Update mod.rs

Remove all `mod` declarations and `pub use` re-exports for deleted files.

## Update callers

- `evaluator.rs` — remove `Option` wrappers
- `compile_common.rs` — remove `Option` wrappers
- `function_compiler/tests.rs` — create classifier + empty sigs
