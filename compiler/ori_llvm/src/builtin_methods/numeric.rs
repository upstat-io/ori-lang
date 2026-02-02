//! Built-in method compilation for numeric primitive types.
//!
//! Handles methods on: int, float, bool, char, byte
//!
//! # Ordering Representation
//!
//! The `compare()` method returns an Ordering value represented as i8:
//! - 0 = Less
//! - 1 = Equal
//! - 2 = Greater
//!
//! This matches the interpreter's representation and allows efficient
//! compilation to native comparison instructions.

use inkwell::values::BasicValueEnum;
use inkwell::IntPredicate;

use crate::builder::Builder;

/// Ordering tag constants (i8 representation).
const LESS: u64 = 0;
const EQUAL: u64 = 1;
const GREATER: u64 = 2;

/// Compile a method call on an int value.
///
/// Supported methods:
/// - `compare(other: int) -> Ordering`
pub fn compile_int_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let lhs = recv.into_int_value();

    match method {
        "compare" => {
            let rhs = args.first()?.into_int_value();
            Some(compile_int_compare(bx, lhs, rhs))
        }
        _ => None,
    }
}

/// Compile a method call on a float value.
///
/// Supported methods:
/// - `compare(other: float) -> Ordering`
///
/// Uses IEEE 754 total ordering via `fcmp` with ordered predicates.
/// NaN handling follows the spec: NaN > all other values.
pub fn compile_float_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let lhs = recv.into_float_value();

    match method {
        "compare" => {
            let rhs = args.first()?.into_float_value();
            Some(compile_float_compare(bx, lhs, rhs))
        }
        _ => None,
    }
}

/// Compile a method call on a bool value.
///
/// Supported methods:
/// - `compare(other: bool) -> Ordering`
///
/// Ordering: false < true
pub fn compile_bool_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let lhs = recv.into_int_value();

    match method {
        "compare" => {
            let rhs = args.first()?.into_int_value();
            // Extend bools to i8 for comparison (false=0, true=1)
            let lhs_ext = bx.zext(lhs, bx.cx().scx.type_i8(), "lhs_ext");
            let rhs_ext = bx.zext(rhs, bx.cx().scx.type_i8(), "rhs_ext");
            Some(compile_unsigned_compare(bx, lhs_ext, rhs_ext))
        }
        _ => None,
    }
}

/// Compile a method call on a char value.
///
/// Supported methods:
/// - `compare(other: char) -> Ordering`
///
/// Ordering: Unicode codepoint order (unsigned comparison)
pub fn compile_char_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let lhs = recv.into_int_value();

    match method {
        "compare" => {
            let rhs = args.first()?.into_int_value();
            // Chars are i32 Unicode codepoints - use unsigned comparison
            Some(compile_unsigned_compare(bx, lhs, rhs))
        }
        _ => None,
    }
}

/// Compile a method call on a byte value.
///
/// Supported methods:
/// - `compare(other: byte) -> Ordering`
///
/// Ordering: numeric order (unsigned comparison)
pub fn compile_byte_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let lhs = recv.into_int_value();

    match method {
        "compare" => {
            let rhs = args.first()?.into_int_value();
            Some(compile_unsigned_compare(bx, lhs, rhs))
        }
        _ => None,
    }
}

/// Compile signed integer comparison returning Ordering (i8).
///
/// Generates:
/// ```llvm
/// %lt = icmp slt %lhs, %rhs
/// %eq = icmp eq %lhs, %rhs
/// %not_lt = select i1 %eq, i8 1, i8 2  ; Equal or Greater
/// %result = select i1 %lt, i8 0, i8 %not_lt
/// ```
fn compile_int_compare<'ll>(
    bx: &Builder<'_, 'll, '_>,
    lhs: inkwell::values::IntValue<'ll>,
    rhs: inkwell::values::IntValue<'ll>,
) -> BasicValueEnum<'ll> {
    let i8_ty = bx.cx().scx.type_i8();
    let less = i8_ty.const_int(LESS, false);
    let equal = i8_ty.const_int(EQUAL, false);
    let greater = i8_ty.const_int(GREATER, false);

    // Compare using signed predicates for int
    let is_lt = bx.icmp(IntPredicate::SLT, lhs, rhs, "lt");
    let is_eq = bx.icmp(IntPredicate::EQ, lhs, rhs, "eq");

    // Build the three-way result
    let not_lt = bx.select(is_eq, equal.into(), greater.into(), "not_lt");
    bx.select(is_lt, less.into(), not_lt, "ordering")
}

/// Compile unsigned integer comparison returning Ordering (i8).
///
/// Used for char, byte, and bool comparisons.
fn compile_unsigned_compare<'ll>(
    bx: &Builder<'_, 'll, '_>,
    lhs: inkwell::values::IntValue<'ll>,
    rhs: inkwell::values::IntValue<'ll>,
) -> BasicValueEnum<'ll> {
    let i8_ty = bx.cx().scx.type_i8();
    let less = i8_ty.const_int(LESS, false);
    let equal = i8_ty.const_int(EQUAL, false);
    let greater = i8_ty.const_int(GREATER, false);

    // Compare using unsigned predicates
    let is_lt = bx.icmp(IntPredicate::ULT, lhs, rhs, "lt");
    let is_eq = bx.icmp(IntPredicate::EQ, lhs, rhs, "eq");

    // Build the three-way result
    let not_lt = bx.select(is_eq, equal.into(), greater.into(), "not_lt");
    bx.select(is_lt, less.into(), not_lt, "ordering")
}

/// Compile floating-point comparison returning Ordering (i8).
///
/// Uses ordered comparisons (OLT, OEQ) which handle NaN correctly:
/// - If either operand is NaN, OLT and OEQ return false
/// - This results in Greater for NaN comparisons (per spec: NaN > all)
fn compile_float_compare<'ll>(
    bx: &Builder<'_, 'll, '_>,
    lhs: inkwell::values::FloatValue<'ll>,
    rhs: inkwell::values::FloatValue<'ll>,
) -> BasicValueEnum<'ll> {
    use inkwell::FloatPredicate;

    let i8_ty = bx.cx().scx.type_i8();
    let less = i8_ty.const_int(LESS, false);
    let equal = i8_ty.const_int(EQUAL, false);
    let greater = i8_ty.const_int(GREATER, false);

    // Use ordered comparisons - false if either operand is NaN
    let is_lt = bx.fcmp(FloatPredicate::OLT, lhs, rhs, "lt");
    let is_eq = bx.fcmp(FloatPredicate::OEQ, lhs, rhs, "eq");

    // Build the three-way result
    // If NaN: both is_lt and is_eq are false, so result is Greater
    let not_lt = bx.select(is_eq, equal.into(), greater.into(), "not_lt");
    bx.select(is_lt, less.into(), not_lt, "ordering")
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use ori_ir::StringInterner;

    use crate::context::CodegenCx;

    fn setup_test_function<'ll>(
        cx: &CodegenCx<'ll, '_>,
    ) -> (
        inkwell::basic_block::BasicBlock<'ll>,
        inkwell::values::FunctionValue<'ll>,
    ) {
        let fn_type = cx.scx.type_i8().fn_type(&[], false);
        let function = cx.llmod().add_function("test_fn", fn_type, None);
        let entry = cx.llcx().append_basic_block(function, "entry");
        (entry, function)
    }

    #[test]
    fn test_int_compare() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");
        let (entry, _function) = setup_test_function(&cx);
        let bx = Builder::build(&cx, entry);

        let lhs = cx.scx.type_i64().const_int(5, false);
        let rhs = cx.scx.type_i64().const_int(10, false);

        let result = compile_int_compare(&bx, lhs, rhs);
        assert!(result.is_int_value());
    }

    #[test]
    fn test_float_compare() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");
        let (entry, _function) = setup_test_function(&cx);
        let bx = Builder::build(&cx, entry);

        let lhs = cx.scx.type_f64().const_float(3.5);
        let rhs = cx.scx.type_f64().const_float(2.5);

        let result = compile_float_compare(&bx, lhs, rhs);
        assert!(result.is_int_value());
    }

    #[test]
    fn test_unsigned_compare() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");
        let (entry, _function) = setup_test_function(&cx);
        let bx = Builder::build(&cx, entry);

        // Test byte comparison
        let lhs = cx.scx.type_i8().const_int(100, false);
        let rhs = cx.scx.type_i8().const_int(200, false);

        let result = compile_unsigned_compare(&bx, lhs, rhs);
        assert!(result.is_int_value());
    }
}
