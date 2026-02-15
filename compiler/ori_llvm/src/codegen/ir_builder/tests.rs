use super::*;
use inkwell::context::Context;

/// Helper: create a `SimpleCx` for testing.
fn test_scx(ctx: &Context) -> SimpleCx<'_> {
    SimpleCx::new(ctx, "ir_builder_test")
}

/// Helper: set up an `IrBuilder` with a function and entry block.
fn setup_builder(irb: &mut IrBuilder<'_, '_>) {
    let i64_ty = irb.i64_type();
    let func = irb.declare_function("test_fn", &[], i64_ty);
    let entry = irb.append_block(func, "entry");
    irb.set_current_function(func);
    irb.position_at_end(entry);
}

// -- Constant creation --

#[test]
fn const_i64_roundtrip() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let id = irb.const_i64(42);
    let val = irb.raw_value(id);
    assert!(val.is_int_value());
    assert_eq!(val.into_int_value().get_zero_extended_constant(), Some(42));
    drop(irb);
}

#[test]
fn const_f64_roundtrip() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let id = irb.const_f64(3.14);
    let val = irb.raw_value(id);
    assert!(val.is_float_value());
    drop(irb);
}

#[test]
fn const_bool_roundtrip() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let t = irb.const_bool(true);
    let f = irb.const_bool(false);
    assert_eq!(
        irb.raw_value(t)
            .into_int_value()
            .get_zero_extended_constant(),
        Some(1)
    );
    assert_eq!(
        irb.raw_value(f)
            .into_int_value()
            .get_zero_extended_constant(),
        Some(0)
    );
    drop(irb);
}

// -- Arithmetic --

#[test]
fn integer_arithmetic() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let a = irb.const_i64(10);
    let b = irb.const_i64(3);

    let sum = irb.add(a, b, "sum");
    let diff = irb.sub(a, b, "diff");
    let prod = irb.mul(a, b, "prod");
    let quot = irb.sdiv(a, b, "quot");
    let rem = irb.srem(a, b, "rem");
    let n = irb.neg(a, "neg");

    assert_ne!(sum, diff);
    assert_ne!(prod, quot);
    assert!(irb.raw_value(sum).is_int_value());
    assert!(irb.raw_value(rem).is_int_value());
    assert!(irb.raw_value(n).is_int_value());
    drop(irb);
}

#[test]
fn float_arithmetic() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let a = irb.const_f64(2.5);
    let b = irb.const_f64(1.5);

    let sum = irb.fadd(a, b, "fsum");
    let diff = irb.fsub(a, b, "fdiff");
    let prod = irb.fmul(a, b, "fprod");
    let quot = irb.fdiv(a, b, "fquot");
    let rem = irb.frem(a, b, "frem");
    let n = irb.fneg(a, "fneg");

    assert!(irb.raw_value(sum).is_float_value());
    assert!(irb.raw_value(diff).is_float_value());
    assert!(irb.raw_value(prod).is_float_value());
    assert!(irb.raw_value(quot).is_float_value());
    assert!(irb.raw_value(rem).is_float_value());
    assert!(irb.raw_value(n).is_float_value());
    drop(irb);
}

// -- Memory --

#[test]
fn alloca_load_store_roundtrip() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let i64_ty = irb.i64_type();
    let ptr = irb.alloca(i64_ty, "x");
    let val = irb.const_i64(99);
    irb.store(val, ptr);
    let loaded = irb.load(i64_ty, ptr, "x_loaded");

    assert!(irb.raw_value(ptr).is_pointer_value());
    assert!(irb.raw_value(loaded).is_int_value());
    drop(irb);
}

#[test]
fn create_entry_alloca_inserts_at_entry() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("entry_test", &[], i64_ty);
    let _entry = irb.append_block(func, "entry");
    let second = irb.append_block(func, "second");
    irb.set_current_function(func);

    // Position in second block.
    irb.position_at_end(second);
    let saved = irb.current_block();
    assert_eq!(saved, Some(second));

    // Create entry alloca — should insert in entry, then restore to second.
    let ptr = irb.create_entry_alloca(func, "entry_var", i64_ty);
    assert!(irb.raw_value(ptr).is_pointer_value());
    assert_eq!(irb.current_block(), Some(second));
    drop(irb);
}

// -- Block management --

#[test]
fn block_creation_and_positioning() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("block_test", &[], i64_ty);
    let bb1 = irb.append_block(func, "bb1");
    let bb2 = irb.append_block(func, "bb2");

    assert_ne!(bb1, bb2);

    irb.position_at_end(bb1);
    assert_eq!(irb.current_block(), Some(bb1));

    irb.position_at_end(bb2);
    assert_eq!(irb.current_block(), Some(bb2));
    drop(irb);
}

#[test]
fn current_block_terminated() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    assert!(!irb.current_block_terminated());

    let val = irb.const_i64(0);
    irb.ret(val);

    assert!(irb.current_block_terminated());
    drop(irb);
}

// -- Phi nodes --

#[test]
fn phi_from_incoming_zero() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let i64_ty = irb.i64_type();
    let result = irb.phi_from_incoming(i64_ty, &[], "empty");
    assert!(result.is_none());
    drop(irb);
}

#[test]
fn phi_from_incoming_single() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let i64_ty = irb.i64_type();
    let val = irb.const_i64(42);
    let current = irb.current_block().unwrap();

    let result = irb.phi_from_incoming(i64_ty, &[(val, current)], "single");
    assert_eq!(result, Some(val));
    drop(irb);
}

#[test]
fn phi_from_incoming_multiple() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("phi_test", &[], i64_ty);
    let bb1 = irb.append_block(func, "bb1");
    let bb2 = irb.append_block(func, "bb2");
    let merge = irb.append_block(func, "merge");
    irb.set_current_function(func);

    irb.position_at_end(bb1);
    let v1 = irb.const_i64(1);
    irb.br(merge);

    irb.position_at_end(bb2);
    let v2 = irb.const_i64(2);
    irb.br(merge);

    irb.position_at_end(merge);
    let result = irb.phi_from_incoming(i64_ty, &[(v1, bb1), (v2, bb2)], "merged");
    assert!(result.is_some());
    let phi_id = result.unwrap();
    assert_ne!(phi_id, v1);
    assert_ne!(phi_id, v2);
    drop(irb);
}

// -- Position save/restore --

#[test]
fn position_save_restore() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("pos_test", &[], i64_ty);
    let bb1 = irb.append_block(func, "bb1");
    let bb2 = irb.append_block(func, "bb2");

    irb.position_at_end(bb1);
    let saved = irb.save_position();
    assert_eq!(saved, Some(bb1));

    irb.position_at_end(bb2);
    assert_eq!(irb.current_block(), Some(bb2));

    irb.restore_position(saved);
    assert_eq!(irb.current_block(), Some(bb1));
    drop(irb);
}

// -- Type registration --

#[test]
fn type_registration() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let bool_ty = irb.bool_type();
    let i8_ty = irb.i8_type();
    let i32_ty = irb.i32_type();
    let i64_ty = irb.i64_type();
    let f64_ty = irb.f64_type();
    let ptr_ty = irb.ptr_type();
    let unit_ty = irb.unit_type();

    assert_ne!(bool_ty, f64_ty);
    assert_ne!(i8_ty, i64_ty);

    assert_eq!(irb.raw_type(bool_ty), scx.type_i1().into());
    assert_eq!(irb.raw_type(i8_ty), scx.type_i8().into());
    assert_eq!(irb.raw_type(i32_ty), scx.type_i32().into());
    assert_eq!(irb.raw_type(i64_ty), scx.type_i64().into());
    assert_eq!(irb.raw_type(f64_ty), scx.type_f64().into());
    assert_eq!(irb.raw_type(ptr_ty), scx.type_ptr().into());
    assert_eq!(irb.raw_type(unit_ty), scx.type_i64().into());
    drop(irb);
}

// -- Select instruction --

#[test]
fn select_instruction() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let cond = irb.const_bool(true);
    let then_val = irb.const_i64(1);
    let else_val = irb.const_i64(2);

    let result = irb.select(cond, then_val, else_val, "sel");
    assert!(irb.raw_value(result).is_int_value());
    drop(irb);
}

// -- Function management --

#[test]
fn declare_and_get_function() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("my_func", &[i64_ty, i64_ty], i64_ty);

    let val = irb.get_function_value(func);
    assert_eq!(val.get_name().to_str().unwrap(), "my_func");
    assert_eq!(val.count_params(), 2);
    drop(irb);
}

#[test]
fn get_or_declare_function_idempotent() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let f1 = irb.get_or_declare_function("idempotent_fn", &[i64_ty], i64_ty);
    let f2 = irb.get_or_declare_function("idempotent_fn", &[i64_ty], i64_ty);

    assert_eq!(irb.get_function_value(f1), irb.get_function_value(f2));
    drop(irb);
}

// -- Comparisons --

#[test]
fn integer_comparisons() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let a = irb.const_i64(5);
    let b = irb.const_i64(10);

    let eq = irb.icmp_eq(a, b, "eq");
    let ne = irb.icmp_ne(a, b, "ne");
    let slt = irb.icmp_slt(a, b, "slt");
    let sgt = irb.icmp_sgt(a, b, "sgt");

    assert!(irb.raw_value(eq).is_int_value());
    assert!(irb.raw_value(ne).is_int_value());
    assert!(irb.raw_value(slt).is_int_value());
    assert!(irb.raw_value(sgt).is_int_value());
    drop(irb);
}

#[test]
fn float_comparisons() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let a = irb.const_f64(1.0);
    let b = irb.const_f64(2.0);

    let oeq = irb.fcmp_oeq(a, b, "oeq");
    let olt = irb.fcmp_olt(a, b, "olt");
    let ogt = irb.fcmp_ogt(a, b, "ogt");

    assert!(irb.raw_value(oeq).is_int_value());
    assert!(irb.raw_value(olt).is_int_value());
    assert!(irb.raw_value(ogt).is_int_value());
    drop(irb);
}

// -- Conversions --

#[test]
fn integer_conversions() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);
    setup_builder(&mut irb);

    let i32_ty = irb.i32_type();
    let i64_ty = irb.i64_type();

    let val32 = irb.const_i32(42);
    let extended = irb.sext(val32, i64_ty, "sext");
    assert!(irb.raw_value(extended).is_int_value());

    let val64 = irb.const_i64(42);
    let truncated = irb.trunc(val64, i32_ty, "trunc");
    assert!(irb.raw_value(truncated).is_int_value());

    let zexted = irb.zext(val32, i64_ty, "zext");
    assert!(irb.raw_value(zexted).is_int_value());
    drop(irb);
}

// -- Intern helpers --

#[test]
fn intern_raw_values() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let raw_val: BasicValueEnum = scx.type_i64().const_int(77, false).into();
    let id = irb.intern_value(raw_val);
    assert_eq!(irb.raw_value(id), raw_val);
    drop(irb);
}

// -- Void function declaration --

#[test]
fn declare_void_function() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_void_function("void_fn", &[i64_ty]);
    let val = irb.get_function_value(func);

    assert_eq!(val.get_name().to_str().unwrap(), "void_fn");
    assert_eq!(val.count_params(), 1);
    // Void return type → function returns void
    assert!(val.get_type().get_return_type().is_none());
    drop(irb);
}

// -- Calling conventions --

#[test]
fn set_fastcc_and_ccc() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func_fast = irb.declare_function("fast_fn", &[], i64_ty);
    irb.set_fastcc(func_fast);

    let func_c = irb.declare_function("c_fn", &[], i64_ty);
    irb.set_ccc(func_c);

    // Verify conventions were set (8 = fastcc, 0 = ccc)
    assert_eq!(irb.get_function_value(func_fast).get_call_conventions(), 8);
    assert_eq!(irb.get_function_value(func_c).get_call_conventions(), 0);
    drop(irb);
}

// -- sret attribute --

#[test]
fn sret_attribute_applied() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let ptr_ty = irb.ptr_type();
    let i64_ty = irb.i64_type();
    let struct_ty = irb.register_type(
        scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        )
        .into(),
    );

    // Declare void function with ptr param (the sret pointer)
    let func = irb.declare_void_function("sret_fn", &[ptr_ty, i64_ty]);
    irb.add_sret_attribute(func, 0, struct_ty);
    irb.add_noalias_attribute(func, 0);

    // Verify function has correct shape
    let val = irb.get_function_value(func);
    assert_eq!(val.count_params(), 2);
    assert!(val.get_type().get_return_type().is_none());
    drop(irb);
}

// -- declare_extern_function --

#[test]
fn declare_extern_function_basic() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let ptr_ty = irb.ptr_type();
    let func = irb.declare_extern_function("ori_print", &[ptr_ty], None);
    let val = irb.get_function_value(func);

    assert_eq!(val.get_name().to_str().unwrap(), "ori_print");
    assert_eq!(val.count_params(), 1);
    assert!(val.get_type().get_return_type().is_none());
    drop(irb);
}

#[test]
fn declare_extern_function_with_return() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let ptr_ty = irb.ptr_type();
    let func = irb.declare_extern_function("ori_list_len", &[ptr_ty], Some(i64_ty));
    let val = irb.get_function_value(func);

    assert_eq!(val.get_name().to_str().unwrap(), "ori_list_len");
    assert!(val.get_type().get_return_type().is_some());
    drop(irb);
}

#[test]
fn declare_extern_function_idempotent() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let ptr_ty = irb.ptr_type();
    let f1 = irb.declare_extern_function("ori_print", &[ptr_ty], None);
    let f2 = irb.declare_extern_function("ori_print", &[ptr_ty], None);

    assert_eq!(irb.get_function_value(f1), irb.get_function_value(f2));
    drop(irb);
}

// -- Tail calls --

#[test]
fn call_tail_marks_instruction() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();

    // Declare a fastcc function that calls itself
    let func = irb.declare_function("recursive_fn", &[i64_ty], i64_ty);
    irb.set_fastcc(func);
    let entry = irb.append_block(func, "entry");
    irb.set_current_function(func);
    irb.position_at_end(entry);

    // Build a tail call to itself
    let arg = irb.const_i64(1);
    let result = irb.call_tail(func, &[arg], "recurse");
    assert!(result.is_some());

    irb.ret(result.unwrap());

    // Verify the IR contains "tail call"
    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        ir.contains("tail call"),
        "Expected 'tail call' in IR, got:\n{ir}"
    );
    drop(irb);
}

#[test]
fn call_without_tail_has_no_tail_attribute() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("normal_fn", &[i64_ty], i64_ty);
    let entry = irb.append_block(func, "entry");
    irb.set_current_function(func);
    irb.position_at_end(entry);

    // Build a regular (non-tail) call
    let arg = irb.const_i64(1);
    let result = irb.call(func, &[arg], "normal");
    assert!(result.is_some());

    irb.ret(result.unwrap());

    // Verify the IR does NOT contain "tail call"
    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        !ir.contains("tail call"),
        "Expected no 'tail call' in IR, got:\n{ir}"
    );
    drop(irb);
}

// -- call_with_sret --

#[test]
fn call_with_sret_creates_alloca_and_load() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    // Set up: declare a caller function and position in it
    let i64_ty = irb.i64_type();
    let caller = irb.declare_function("caller", &[], i64_ty);
    let entry = irb.append_block(caller, "entry");
    irb.set_current_function(caller);
    irb.position_at_end(entry);

    // Declare an sret callee: void fn(ptr sret, i64)
    let struct_ty = irb.register_type(
        scx.type_struct(
            &[
                scx.type_i64().into(),
                scx.type_i64().into(),
                scx.type_ptr().into(),
            ],
            false,
        )
        .into(),
    );
    let ptr_ty = irb.ptr_type();
    let callee = irb.declare_void_function("sret_callee", &[ptr_ty, i64_ty]);

    // Call with sret
    let arg = irb.const_i64(42);
    let result = irb.call_with_sret(callee, &[arg], struct_ty, "result");

    // Result should be a value (loaded from sret alloca)
    assert!(result.is_some());
    drop(irb);
}

// -- Exception handling --

/// Helper: set up a function with entry, then, and catch blocks for invoke tests.
fn setup_invoke_blocks(irb: &mut IrBuilder<'_, '_>) -> (FunctionId, BlockId, BlockId, BlockId) {
    let i64_ty = irb.i64_type();
    let func = irb.declare_function("invoke_test_fn", &[i64_ty], i64_ty);
    let entry = irb.append_block(func, "entry");
    let then_block = irb.append_block(func, "then");
    let catch_block = irb.append_block(func, "catch");
    irb.set_current_function(func);
    irb.position_at_end(entry);
    (func, entry, then_block, catch_block)
}

#[test]
fn invoke_produces_invoke_instruction() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let (func, _entry, then_block, catch_block) = setup_invoke_blocks(&mut irb);

    let arg = irb.const_i64(42);
    let result = irb.invoke(func, &[arg], then_block, catch_block, "inv_result");
    assert!(result.is_some());

    // The invoke terminates the entry block.
    assert!(irb.current_block_terminated());

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains("invoke"), "Expected 'invoke' in IR, got:\n{ir}");
    drop(irb);
}

#[test]
fn invoke_void_returns_none() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let caller = irb.declare_function("invoke_void_caller", &[], i64_ty);
    let entry = irb.append_block(caller, "entry");
    let then_block = irb.append_block(caller, "then");
    let catch_block = irb.append_block(caller, "catch");
    irb.set_current_function(caller);
    irb.position_at_end(entry);

    // Declare a void callee.
    let ptr_ty = irb.ptr_type();
    let void_fn = irb.declare_extern_function("void_callee", &[ptr_ty], None);

    let arg = irb.const_i64(0);
    let ptr_val = irb.int_to_ptr(arg, "as_ptr");
    let result = irb.invoke(void_fn, &[ptr_val], then_block, catch_block, "");
    assert!(result.is_none(), "void invoke should return None");
    drop(irb);
}

#[test]
fn landingpad_produces_struct_value() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let (func, _entry, then_block, catch_block) = setup_invoke_blocks(&mut irb);

    // Declare personality function.
    let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);
    irb.set_personality(func, personality);

    // Invoke in entry, then landingpad in catch.
    let arg = irb.const_i64(1);
    irb.invoke(func, &[arg], then_block, catch_block, "inv");

    irb.position_at_end(catch_block);
    let lp = irb.landingpad(personality, true, "lp");

    // The landing pad value is a struct { ptr, i32 }.
    let lp_val = irb.raw_value(lp);
    assert!(lp_val.is_struct_value());

    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        ir.contains("landingpad"),
        "Expected 'landingpad' in IR, got:\n{ir}"
    );
    drop(irb);
}

#[test]
fn resume_terminates_block() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let (func, _entry, then_block, catch_block) = setup_invoke_blocks(&mut irb);

    let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);
    irb.set_personality(func, personality);

    let arg = irb.const_i64(1);
    irb.invoke(func, &[arg], then_block, catch_block, "inv");

    // Build landingpad + resume in catch block.
    irb.position_at_end(catch_block);
    let lp = irb.landingpad(personality, true, "lp");
    assert!(!irb.current_block_terminated());
    irb.resume(lp);
    assert!(irb.current_block_terminated());

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains("resume"), "Expected 'resume' in IR, got:\n{ir}");
    drop(irb);
}

#[test]
fn full_invoke_landingpad_resume_flow() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();

    // Declare personality.
    let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);

    // Declare a callee that might throw.
    let callee = irb.declare_function("may_throw", &[i64_ty], i64_ty);

    // Build the caller function.
    let caller = irb.declare_function("caller", &[i64_ty], i64_ty);
    irb.set_personality(caller, personality);
    let entry = irb.append_block(caller, "entry");
    let normal = irb.append_block(caller, "normal");
    let unwind = irb.append_block(caller, "unwind");
    irb.set_current_function(caller);

    // Entry: invoke callee → normal or unwind.
    irb.position_at_end(entry);
    let arg = irb.const_i64(42);
    let result = irb.invoke(callee, &[arg], normal, unwind, "result");
    assert!(result.is_some());

    // Normal: return the invoke result.
    irb.position_at_end(normal);
    irb.ret(result.unwrap());

    // Unwind: landingpad + resume.
    irb.position_at_end(unwind);
    let lp = irb.landingpad(personality, true, "lp");
    irb.resume(lp);

    // Verify the complete EH flow in the IR.
    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains("invoke"), "Missing invoke in IR:\n{ir}");
    assert!(ir.contains("landingpad"), "Missing landingpad in IR:\n{ir}");
    assert!(ir.contains("resume"), "Missing resume in IR:\n{ir}");
    assert!(ir.contains("cleanup"), "Missing cleanup flag in IR:\n{ir}");
    assert!(
        ir.contains("to label %normal unwind label %unwind"),
        "Missing invoke branch targets in IR:\n{ir}"
    );
    drop(irb);
}

#[test]
fn set_personality_on_function() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let func = irb.declare_function("personality_test", &[], i64_ty);
    let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);
    irb.set_personality(func, personality);

    let ir = scx.llmod.print_to_string().to_string();
    assert!(
        ir.contains("personality"),
        "Expected 'personality' in IR, got:\n{ir}"
    );
    drop(irb);
}

#[test]
fn invoke_indirect_produces_invoke() {
    let ctx = Context::create();
    let scx = test_scx(&ctx);
    let mut irb = IrBuilder::new(&scx);

    let i64_ty = irb.i64_type();
    let caller = irb.declare_function("indirect_invoke_test", &[], i64_ty);
    let entry = irb.append_block(caller, "entry");
    let then_block = irb.append_block(caller, "then");
    let catch_block = irb.append_block(caller, "catch");
    irb.set_current_function(caller);
    irb.position_at_end(entry);

    // Get a function pointer to invoke indirectly.
    let target = irb.declare_function("target_fn", &[i64_ty], i64_ty);
    let fn_ptr = irb.get_function_ptr(target);

    let arg = irb.const_i64(7);
    let result = irb.invoke_indirect(
        i64_ty,
        &[i64_ty],
        fn_ptr,
        &[arg],
        then_block,
        catch_block,
        "indirect_inv",
    );
    assert!(result.is_some());
    assert!(irb.current_block_terminated());

    let ir = scx.llmod.print_to_string().to_string();
    assert!(ir.contains("invoke"), "Expected 'invoke' in IR, got:\n{ir}");
    drop(irb);
}
