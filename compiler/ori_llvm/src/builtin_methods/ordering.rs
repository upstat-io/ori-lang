//! Built-in method compilation for the Ordering type.
//!
//! Ordering is a sum type with three variants: Less, Equal, Greater.
//! In LLVM, it's represented as i8:
//! - 0 = Less
//! - 1 = Equal
//! - 2 = Greater
//!
//! # Methods
//!
//! Predicates (return bool):
//! - `is_less()`, `is_equal()`, `is_greater()`
//! - `is_less_or_equal()`, `is_greater_or_equal()`
//!
//! Transformations (return Ordering):
//! - `reverse()` - swap Less/Greater
//!
//! Trait methods:
//! - `equals(other)` - Eq trait
//! - `compare(other)` - Comparable trait
//! - `clone()` - Clone trait (identity)
//! - `hash()` - Hashable trait

use inkwell::values::BasicValueEnum;
use inkwell::IntPredicate;
use ori_ir::builtin_constants::ordering::unsigned as ord;

use crate::builder::Builder;

/// Compile a method call on an Ordering value.
///
/// The receiver is expected to be an i8 value representing the Ordering tag.
pub fn compile_ordering_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let tag = recv.into_int_value();
    let i8_ty = bx.cx().scx.type_i8();

    match method {
        // Predicate methods (return bool)
        "is_less" => {
            let val = i8_ty.const_int(ord::LESS, false);
            Some(bx.icmp(IntPredicate::EQ, tag, val, "is_less").into())
        }
        "is_equal" => {
            let val = i8_ty.const_int(ord::EQUAL, false);
            Some(bx.icmp(IntPredicate::EQ, tag, val, "is_equal").into())
        }
        "is_greater" => {
            let val = i8_ty.const_int(ord::GREATER, false);
            Some(bx.icmp(IntPredicate::EQ, tag, val, "is_greater").into())
        }
        "is_less_or_equal" => {
            // Less (0) or Equal (1) => tag != 2
            let val = i8_ty.const_int(ord::GREATER, false);
            Some(bx.icmp(IntPredicate::NE, tag, val, "is_le").into())
        }
        "is_greater_or_equal" => {
            // Equal (1) or Greater (2) => tag != 0
            let val = i8_ty.const_int(ord::LESS, false);
            Some(bx.icmp(IntPredicate::NE, tag, val, "is_ge").into())
        }

        // Transformation: reverse Less <-> Greater, Equal stays same
        // Less(0) -> Greater(2): 2 - 0 = 2
        // Equal(1) -> Equal(1): 2 - 1 = 1
        // Greater(2) -> Less(0): 2 - 2 = 0
        "reverse" => {
            let two = i8_ty.const_int(ord::GREATER, false);
            Some(bx.sub(two, tag, "reversed").into())
        }

        // Clone trait: Ordering is Copy, just return as-is
        "clone" => Some(recv),

        // Hash trait: use the tag value directly (extended to i64)
        "hash" => {
            let i64_ty = bx.cx().scx.type_i64();
            Some(bx.sext(tag, i64_ty, "hash").into())
        }

        // Eq trait: compare tags for equality
        "equals" => {
            let other = args.first()?.into_int_value();
            Some(bx.icmp(IntPredicate::EQ, tag, other, "equals").into())
        }

        // Comparable trait: compare Ordering values (Less < Equal < Greater)
        // This is comparing 0 < 1 < 2, so unsigned comparison
        "compare" => {
            let other = args.first()?.into_int_value();
            Some(compile_ordering_compare(bx, tag, other))
        }

        _ => None,
    }
}

/// Compile comparison between two Ordering values.
///
/// Returns an Ordering (i8) indicating which value is less/equal/greater.
fn compile_ordering_compare<'ll>(
    bx: &Builder<'_, 'll, '_>,
    lhs: inkwell::values::IntValue<'ll>,
    rhs: inkwell::values::IntValue<'ll>,
) -> BasicValueEnum<'ll> {
    let i8_ty = bx.cx().scx.type_i8();
    let less = i8_ty.const_int(ord::LESS, false);
    let equal = i8_ty.const_int(ord::EQUAL, false);
    let greater = i8_ty.const_int(ord::GREATER, false);

    // Unsigned comparison since 0 < 1 < 2
    let is_lt = bx.icmp(IntPredicate::ULT, lhs, rhs, "lt");
    let is_eq = bx.icmp(IntPredicate::EQ, lhs, rhs, "eq");

    let not_lt = bx.select(is_eq, equal.into(), greater.into(), "not_lt");
    bx.select(is_lt, less.into(), not_lt, "ordering")
}
