//! AOT smoke tests for LLVM emitters that previously handed the deferred
//! codegen-error guard a type-mismatched operand, tripping the
//! multiple-errors / skip-verification path before any code reached the
//! linked-runtime execution surface. Interpreter and JIT-LLVM parity for
//! these shapes is covered by the full spec matrix; this module proves the
//! same fixed emitters compile and run correctly through the real `ori
//! build` -> native-binary AOT pipeline, distinct from the JIT path.

use crate::util::assert_aot_success;

#[test]
fn test_set_element_eq_dispatch_succeeds() {
    assert_aot_success(
        include_str!("fixtures/deferred_codegen_error_emitters/set_element_eq.ori"),
        "set_element_eq_dispatch",
    );
}

#[test]
fn test_struct_multi_impl_index_dispatch_succeeds() {
    assert_aot_success(
        include_str!("fixtures/deferred_codegen_error_emitters/struct_multi_impl_index.ori"),
        "struct_multi_impl_index_dispatch",
    );
}

#[test]
fn test_associated_fn_name_collision_succeeds() {
    assert_aot_success(
        include_str!("fixtures/deferred_codegen_error_emitters/associated_fn_name_collision.ori"),
        "associated_fn_name_collision",
    );
}

#[test]
fn test_generic_extend_method_succeeds() {
    assert_aot_success(
        include_str!("fixtures/deferred_codegen_error_emitters/generic_extend_method.ori"),
        "generic_extend_method",
    );
}
