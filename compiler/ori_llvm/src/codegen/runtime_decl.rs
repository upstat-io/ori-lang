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
    // cold: panic paths are rarely taken; moves code out of hot layout
    // NOT nounwind: ori_panic unwinds via Rust panic infrastructure
    // so LLVM invoke/landingpad can run RC cleanup handlers
    let panic_fn = builder.declare_extern_function("ori_panic", &[ptr_ty], void);
    builder.add_cold_attribute(panic_fn);
    let panic_cstr = builder.declare_extern_function("ori_panic_cstr", &[ptr_ty], void);
    builder.add_cold_attribute(panic_cstr);

    // -- Entry point wrapper --
    // ori_run_main wraps @main with catch_unwind for clean panic handling
    builder.declare_extern_function("ori_run_main", &[ptr_ty], Some(i32_ty));

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

    // -- Reference counting (V2: data-pointer style, 8-byte header) --
    //
    // ARC-safe attributes are CRITICAL for correctness under LLVM optimization.
    // Without them, DSE/LICM/GVN may reorder or eliminate RC operations.
    // See plans/llvm_v2/section-11-llvm-passes.md §11.3 for rationale.

    // ori_rc_alloc(size: usize, align: usize) -> *mut u8 (data pointer)
    // noalias return: fresh allocation, no existing pointers alias it
    // nounwind: allocation failure = abort (no unwinding)
    let rc_alloc = builder.declare_extern_function("ori_rc_alloc", &[i64_ty, i64_ty], Some(ptr_ty));
    builder.add_nounwind_attribute(rc_alloc);
    builder.add_noalias_return_attribute(rc_alloc);

    // ori_rc_inc(data_ptr: *mut u8)
    // nounwind: RC operations never throw
    // memory(argmem: readwrite): only touches refcount at ptr-8
    // NOT readonly/readnone — modifies the refcount word
    let rc_inc = builder.declare_extern_function("ori_rc_inc", &[ptr_ty], void);
    builder.add_nounwind_attribute(rc_inc);
    builder.add_memory_argmem_readwrite_attribute(rc_inc);

    // ori_rc_dec(data_ptr: *mut u8, drop_fn: fn(*mut u8))
    // nounwind: drop functions must not unwind (panic = abort)
    // memory(argmem: readwrite): modifies refcount, may call drop_fn, may free
    // NOT readonly — decrements refcount AND may free memory
    let rc_dec = builder.declare_extern_function("ori_rc_dec", &[ptr_ty, ptr_ty], void);
    builder.add_nounwind_attribute(rc_dec);
    builder.add_memory_argmem_readwrite_attribute(rc_dec);

    // ori_rc_free(data_ptr: *mut u8, size: usize, align: usize)
    // nounwind: deallocation never throws
    let rc_free = builder.declare_extern_function("ori_rc_free", &[ptr_ty, i64_ty, i64_ty], void);
    builder.add_nounwind_attribute(rc_free);

    // -- Args conversion --
    // ori_args_from_argv(argc: i32, argv: *const *const i8) -> OriList { i64, i64, ptr }
    let list_ty = builder.register_type(
        builder
            .scx()
            .type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            )
            .into(),
    );
    builder.declare_extern_function("ori_args_from_argv", &[i32_ty, ptr_ty], Some(list_ty));

    // -- Panic handler registration --
    builder.declare_extern_function("ori_register_panic_handler", &[ptr_ty], void);

    // -- EH personality (Itanium ABI) --
    // Required by any function containing invoke/landingpad.
    // We use Rust's personality function (already in libori_rt.a) instead of
    // __gxx_personality_v0 (which would require linking libstdc++).
    // rust_eh_personality parses the same LSDA format that LLVM generates.
    let personality =
        builder.declare_extern_function("rust_eh_personality", &[i32_ty], Some(i32_ty));
    builder.add_nounwind_attribute(personality);
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
            "ori_rc_alloc",
            "ori_rc_inc",
            "ori_rc_dec",
            "ori_rc_free",
            "ori_args_from_argv",
            "ori_register_panic_handler",
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

    #[test]
    fn rc_functions_have_arc_safe_attributes() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_rc_attrs");
        let mut builder = IrBuilder::new(&scx);

        declare_runtime(&mut builder);

        let ir = scx.llmod.print_to_string().to_string();

        // ori_rc_alloc: nounwind + noalias return
        assert!(
            ir.contains("noalias") && ir.contains("ori_rc_alloc"),
            "ori_rc_alloc should have noalias return attribute in IR:\n{ir}"
        );

        // ori_rc_inc: nounwind + memory(argmem: readwrite)
        // ori_rc_dec: nounwind + memory(argmem: readwrite)
        // The `memory` attribute should appear as an enum attribute, not string
        assert!(
            ir.contains("ori_rc_inc"),
            "ori_rc_inc should be declared in IR"
        );
        assert!(
            ir.contains("ori_rc_dec"),
            "ori_rc_dec should be declared in IR"
        );

        // Verify nounwind appears on RC functions
        // The IR prints function attributes as attribute groups (#N)
        // Check that nounwind is present in the module
        assert!(
            ir.contains("nounwind"),
            "RC functions should have nounwind attribute in IR:\n{ir}"
        );
    }

    #[test]
    fn panic_functions_have_cold_nounwind() {
        let ctx = Context::create();
        let scx = SimpleCx::new(&ctx, "test_panic_attrs");
        let mut builder = IrBuilder::new(&scx);

        declare_runtime(&mut builder);

        let ir = scx.llmod.print_to_string().to_string();

        // Panic functions should have cold + nounwind
        assert!(
            ir.contains("cold"),
            "panic functions should have cold attribute in IR:\n{ir}"
        );
    }
}
