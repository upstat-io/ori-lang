//! Built-in method compilation for Duration and Size types.
//!
//! # Representation
//!
//! - Duration: i64 nanoseconds (allows negative for time differences)
//! - Size: i64 bytes (stored as i64, but semantically non-negative)
//!
//! # Duration Methods
//!
//! Unit extraction (return int):
//! - `nanoseconds()`, `microseconds()`, `milliseconds()`
//! - `seconds()`, `minutes()`, `hours()`
//!
//! Trait methods:
//! - `compare(other)` - Comparable trait
//! - `equals(other)` - Eq trait
//! - `clone()` - Clone trait
//! - `hash()` - Hashable trait
//!
//! # Size Methods
//!
//! Unit extraction (return int):
//! - `bytes()`, `kilobytes()`, `megabytes()`
//! - `gigabytes()`, `terabytes()`
//!
//! Trait methods:
//! - `compare(other)` - Comparable trait
//! - `equals(other)` - Eq trait
//! - `clone()` - Clone trait
//! - `hash()` - Hashable trait

use inkwell::values::BasicValueEnum;
use inkwell::IntPredicate;
use ori_ir::builtin_constants::{duration::unsigned as dur, ordering::unsigned as ord, size};

use crate::builder::Builder;

/// Compile a method call on a Duration value.
///
/// Duration is stored as i64 nanoseconds.
pub fn compile_duration_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let ns = recv.into_int_value();
    let i64_ty = bx.cx().scx.type_i64();

    match method {
        // Unit extraction methods (signed division for Duration)
        "microseconds" => {
            let divisor = i64_ty.const_int(dur::NS_PER_US, false);
            Some(bx.sdiv(ns, divisor, "us").into())
        }
        "milliseconds" => {
            let divisor = i64_ty.const_int(dur::NS_PER_MS, false);
            Some(bx.sdiv(ns, divisor, "ms").into())
        }
        "seconds" => {
            let divisor = i64_ty.const_int(dur::NS_PER_S, false);
            Some(bx.sdiv(ns, divisor, "s").into())
        }
        "minutes" => {
            let divisor = i64_ty.const_int(dur::NS_PER_M, false);
            Some(bx.sdiv(ns, divisor, "m").into())
        }
        "hours" => {
            let divisor = i64_ty.const_int(dur::NS_PER_H, false);
            Some(bx.sdiv(ns, divisor, "h").into())
        }

        // nanoseconds() returns the raw value, clone() and hash() return identity
        "nanoseconds" | "clone" | "hash" => Some(recv),

        // Eq trait
        "equals" => {
            let other = args.first()?.into_int_value();
            Some(bx.icmp(IntPredicate::EQ, ns, other, "equals").into())
        }

        // Comparable trait: signed comparison for Duration
        "compare" => {
            let other = args.first()?.into_int_value();
            Some(compile_signed_compare(bx, ns, other))
        }

        _ => None,
    }
}

/// Compile a method call on a Size value.
///
/// Size is stored as i64 bytes (semantically non-negative).
pub fn compile_size_method<'ll>(
    bx: &Builder<'_, 'll, '_>,
    recv: BasicValueEnum<'ll>,
    method: &str,
    args: &[BasicValueEnum<'ll>],
) -> Option<BasicValueEnum<'ll>> {
    let bytes = recv.into_int_value();
    let i64_ty = bx.cx().scx.type_i64();

    match method {
        // Unit extraction methods (unsigned division for Size)
        "kilobytes" => {
            let divisor = i64_ty.const_int(size::BYTES_PER_KB, false);
            Some(bx.udiv(bytes, divisor, "kb").into())
        }
        "megabytes" => {
            let divisor = i64_ty.const_int(size::BYTES_PER_MB, false);
            Some(bx.udiv(bytes, divisor, "mb").into())
        }
        "gigabytes" => {
            let divisor = i64_ty.const_int(size::BYTES_PER_GB, false);
            Some(bx.udiv(bytes, divisor, "gb").into())
        }
        "terabytes" => {
            let divisor = i64_ty.const_int(size::BYTES_PER_TB, false);
            Some(bx.udiv(bytes, divisor, "tb").into())
        }

        // bytes() returns the raw value, clone() and hash() return identity
        "bytes" | "clone" | "hash" => Some(recv),

        // Eq trait
        "equals" => {
            let other = args.first()?.into_int_value();
            Some(bx.icmp(IntPredicate::EQ, bytes, other, "equals").into())
        }

        // Comparable trait: unsigned comparison for Size (always non-negative)
        "compare" => {
            let other = args.first()?.into_int_value();
            Some(compile_unsigned_compare(bx, bytes, other))
        }

        _ => None,
    }
}

/// Compile signed integer comparison returning Ordering (i8).
fn compile_signed_compare<'ll>(
    bx: &Builder<'_, 'll, '_>,
    lhs: inkwell::values::IntValue<'ll>,
    rhs: inkwell::values::IntValue<'ll>,
) -> BasicValueEnum<'ll> {
    let i8_ty = bx.cx().scx.type_i8();
    let less = i8_ty.const_int(ord::LESS, false);
    let equal = i8_ty.const_int(ord::EQUAL, false);
    let greater = i8_ty.const_int(ord::GREATER, false);

    let is_lt = bx.icmp(IntPredicate::SLT, lhs, rhs, "lt");
    let is_eq = bx.icmp(IntPredicate::EQ, lhs, rhs, "eq");

    let not_lt = bx.select(is_eq, equal.into(), greater.into(), "not_lt");
    bx.select(is_lt, less.into(), not_lt, "ordering")
}

/// Compile unsigned integer comparison returning Ordering (i8).
fn compile_unsigned_compare<'ll>(
    bx: &Builder<'_, 'll, '_>,
    lhs: inkwell::values::IntValue<'ll>,
    rhs: inkwell::values::IntValue<'ll>,
) -> BasicValueEnum<'ll> {
    let i8_ty = bx.cx().scx.type_i8();
    let less = i8_ty.const_int(ord::LESS, false);
    let equal = i8_ty.const_int(ord::EQUAL, false);
    let greater = i8_ty.const_int(ord::GREATER, false);

    let is_lt = bx.icmp(IntPredicate::ULT, lhs, rhs, "lt");
    let is_eq = bx.icmp(IntPredicate::EQ, lhs, rhs, "eq");

    let not_lt = bx.select(is_eq, equal.into(), greater.into(), "not_lt");
    bx.select(is_lt, less.into(), not_lt, "ordering")
}
