//! Runtime function declarations for V2 codegen.
//!
//! Declares all extern functions provided by the Ori runtime library (`ori_rt`).
//! These are resolved at link time (AOT) or via symbol mapping (JIT).
//!
//! Replaces `CodegenCx::declare_runtime_functions()` with a standalone function
//! that operates on `IrBuilder` — no coupling to `CodegenCx`.

use super::ir_builder::IrBuilder;

/// Declare all Ori runtime functions as external linkage in the LLVM module.
///
/// Call this once per module before any function compilation. The declarations
/// make runtime functions available for codegen (e.g., `ori_print`, `ori_panic`).
pub fn declare_runtime(builder: &mut IrBuilder<'_, '_>) {
    let void = None;
    let i64_ty = builder.i64_type();
    let i32_ty = builder.i32_type();
    let f64_ty = builder.f64_type();
    let bool_ty = builder.bool_type();
    let ptr_ty = builder.ptr_type();

    // String type: { i64 len, ptr data }
    let str_ty = builder.register_type(
        builder
            .scx()
            .type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            )
            .into(),
    );

    // -- I/O functions --
    builder.declare_extern_function("ori_print", &[ptr_ty], void);
    builder.declare_extern_function("ori_print_int", &[i64_ty], void);
    builder.declare_extern_function("ori_print_float", &[f64_ty], void);
    builder.declare_extern_function("ori_print_bool", &[bool_ty], void);

    // -- Panic functions --
    builder.declare_extern_function("ori_panic", &[ptr_ty], void);
    builder.declare_extern_function("ori_panic_cstr", &[ptr_ty], void);

    // -- Assertion functions --
    builder.declare_extern_function("ori_assert", &[bool_ty], void);
    builder.declare_extern_function("ori_assert_eq_int", &[i64_ty, i64_ty], void);
    builder.declare_extern_function("ori_assert_eq_bool", &[bool_ty, bool_ty], void);
    builder.declare_extern_function("ori_assert_eq_str", &[ptr_ty, ptr_ty], void);

    // -- List functions --
    builder.declare_extern_function("ori_list_new", &[i64_ty, i64_ty], Some(ptr_ty));
    builder.declare_extern_function("ori_list_free", &[ptr_ty, i64_ty], void);
    builder.declare_extern_function("ori_list_len", &[ptr_ty], Some(i64_ty));

    // -- Comparison functions --
    builder.declare_extern_function("ori_compare_int", &[i64_ty, i64_ty], Some(i32_ty));
    builder.declare_extern_function("ori_min_int", &[i64_ty, i64_ty], Some(i64_ty));
    builder.declare_extern_function("ori_max_int", &[i64_ty, i64_ty], Some(i64_ty));

    // -- String functions --
    builder.declare_extern_function("ori_str_concat", &[ptr_ty, ptr_ty], Some(str_ty));
    builder.declare_extern_function("ori_str_eq", &[ptr_ty, ptr_ty], Some(bool_ty));
    builder.declare_extern_function("ori_str_ne", &[ptr_ty, ptr_ty], Some(bool_ty));

    // -- Type conversion functions --
    builder.declare_extern_function("ori_str_from_int", &[i64_ty], Some(str_ty));
    builder.declare_extern_function("ori_str_from_bool", &[bool_ty], Some(str_ty));
    builder.declare_extern_function("ori_str_from_float", &[f64_ty], Some(str_ty));

    // -- Closure boxing --
    builder.declare_extern_function("ori_closure_box", &[i64_ty], Some(ptr_ty));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimpleCx;
    use inkwell::context::Context;

    #[test]
    fn runtime_functions_declared() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_runtime");
        let mut builder = IrBuilder::new(&scx);

        declare_runtime(&mut builder);

        // Verify key runtime functions exist in the module
        let expected = [
            "ori_print",
            "ori_print_int",
            "ori_print_float",
            "ori_print_bool",
            "ori_panic",
            "ori_panic_cstr",
            "ori_assert",
            "ori_assert_eq_int",
            "ori_assert_eq_bool",
            "ori_assert_eq_str",
            "ori_list_new",
            "ori_list_free",
            "ori_list_len",
            "ori_compare_int",
            "ori_min_int",
            "ori_max_int",
            "ori_str_concat",
            "ori_str_eq",
            "ori_str_ne",
            "ori_str_from_int",
            "ori_str_from_bool",
            "ori_str_from_float",
            "ori_closure_box",
        ];

        for name in &expected {
            assert!(
                scx.llmod.get_function(name).is_some(),
                "runtime function '{name}' should be declared"
            );
        }
    }

    #[test]
    fn str_functions_return_struct_type() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_str_types");
        let mut builder = IrBuilder::new(&scx);

        declare_runtime(&mut builder);

        // ori_str_concat returns { i64, ptr } (string type)
        let concat = scx.llmod.get_function("ori_str_concat").unwrap();
        let ret_ty = concat.get_type().get_return_type().unwrap();
        assert!(
            ret_ty.is_struct_type(),
            "ori_str_concat should return a struct type, got {ret_ty}"
        );

        // ori_str_from_int also returns { i64, ptr }
        let from_int = scx.llmod.get_function("ori_str_from_int").unwrap();
        let ret_ty = from_int.get_type().get_return_type().unwrap();
        assert!(
            ret_ty.is_struct_type(),
            "ori_str_from_int should return a struct type, got {ret_ty}"
        );
    }

    #[test]
    fn void_functions_have_no_return() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_void_fns");
        let mut builder = IrBuilder::new(&scx);

        declare_runtime(&mut builder);

        // Void functions should have no return type
        let print = scx.llmod.get_function("ori_print").unwrap();
        assert!(
            print.get_type().get_return_type().is_none(),
            "ori_print should return void"
        );

        let panic = scx.llmod.get_function("ori_panic").unwrap();
        assert!(
            panic.get_type().get_return_type().is_none(),
            "ori_panic should return void"
        );
    }

    #[test]
    fn declare_runtime_is_idempotent() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_idempotent");
        let mut builder = IrBuilder::new(&scx);

        // Calling twice should not panic or duplicate
        declare_runtime(&mut builder);
        declare_runtime(&mut builder);

        assert!(scx.llmod.get_function("ori_print").is_some());
    }
}
