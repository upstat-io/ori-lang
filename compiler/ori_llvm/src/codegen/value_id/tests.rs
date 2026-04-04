use super::*;
use inkwell::context::Context;

#[test]
fn none_sentinels() {
    assert!(ValueId::NONE.is_none());
    assert!(LLVMTypeId::NONE.is_none());
    assert!(BlockId::NONE.is_none());
    assert!(FunctionId::NONE.is_none());

    // Non-NONE IDs should not be none.
    assert!(!ValueId(0).is_none());
    assert!(!LLVMTypeId(0).is_none());
    assert!(!BlockId(0).is_none());
    assert!(!FunctionId(0).is_none());
}

#[test]
fn value_arena_push_get_roundtrip() {
    let ctx = Context::create();
    let mut arena = ValueArena::new();

    // Values
    let i64_val = ctx.i64_type().const_int(42, false);
    let id = arena.push_value(i64_val.into());
    assert_eq!(arena.get_value(id).into_int_value(), i64_val);

    // Types
    let i64_ty = ctx.i64_type();
    let ty_id = arena.push_type(i64_ty.into());
    assert_eq!(arena.get_type(ty_id), i64_ty.into());

    // Blocks
    let module = ctx.create_module("test");
    let fn_type = ctx.void_type().fn_type(&[], false);
    let func = module.add_function("test_fn", fn_type, None);
    let bb = ctx.append_basic_block(func, "entry");
    let bb_id = arena.push_block(bb);
    assert_eq!(arena.get_block(bb_id), bb);

    // Functions
    let func_id = arena.push_function(func);
    assert_eq!(arena.get_function(func_id), func);
}

#[test]
fn multiple_values_get_distinct_ids() {
    let ctx = Context::create();
    let mut arena = ValueArena::new();

    let v1 = ctx.i64_type().const_int(1, false);
    let v2 = ctx.i64_type().const_int(2, false);

    let id1 = arena.push_value(v1.into());
    let id2 = arena.push_value(v2.into());

    assert_ne!(id1, id2);
    assert_eq!(arena.get_value(id1).into_int_value(), v1);
    assert_eq!(arena.get_value(id2).into_int_value(), v2);
}

#[test]
#[should_panic(expected = "out of bounds")]
fn value_out_of_bounds_panics_in_debug() {
    let arena = ValueArena::new();
    let _ = arena.get_value(ValueId(0));
}

#[test]
#[should_panic(expected = "out of bounds")]
fn type_out_of_bounds_panics_in_debug() {
    let arena = ValueArena::new();
    let _ = arena.get_type(LLVMTypeId(0));
}

#[test]
#[should_panic(expected = "out of bounds")]
fn block_out_of_bounds_panics_in_debug() {
    let arena = ValueArena::new();
    let _ = arena.get_block(BlockId(0));
}

#[test]
#[should_panic(expected = "out of bounds")]
fn function_out_of_bounds_panics_in_debug() {
    let arena = ValueArena::new();
    let _ = arena.get_function(FunctionId(0));
}

#[test]
fn raw_index_matches() {
    assert_eq!(ValueId(7).raw(), 7);
    assert_eq!(LLVMTypeId(3).raw(), 3);
    assert_eq!(BlockId(12).raw(), 12);
    assert_eq!(FunctionId(0).raw(), 0);
}
