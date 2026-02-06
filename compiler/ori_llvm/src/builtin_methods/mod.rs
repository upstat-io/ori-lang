//! Built-in method compilation for LLVM backend.
//!
//! This module provides direct enum-based dispatch for built-in method calls,
//! mirroring the architecture in `ori_types/src/registry/methods.rs` and
//! `ori_eval/src/methods.rs`.
//!
//! Uses static dispatch via enum matching (not trait objects) for:
//! - Exhaustiveness checking at compile time
//! - Better inlining opportunities
//! - Zero vtable overhead
//!
//! # Supported Methods
//!
//! Primitive types:
//! - `int.compare(other:)` -> Ordering
//! - `float.compare(other:)` -> Ordering
//! - `bool.compare(other:)` -> Ordering
//! - `char.compare(other:)` -> Ordering
//! - `byte.compare(other:)` -> Ordering
//!
//! Ordering type:
//! - `is_less()`, `is_equal()`, `is_greater()` -> bool
//! - `is_less_or_equal()`, `is_greater_or_equal()` -> bool
//! - `reverse()` -> Ordering
//! - `equals(other:)`, `compare(other:)`, `clone()`, `hash()` -> trait methods
//!
//! Duration type:
//! - `nanoseconds()`, `microseconds()`, `milliseconds()` -> int
//! - `seconds()`, `minutes()`, `hours()` -> int
//! - `equals(other:)`, `compare(other:)`, `clone()`, `hash()` -> trait methods
//!
//! Size type:
//! - `bytes()`, `kilobytes()`, `megabytes()` -> int
//! - `gigabytes()`, `terabytes()` -> int
//! - `equals(other:)`, `compare(other:)`, `clone()`, `hash()` -> trait methods

mod numeric;
mod ordering;
mod units;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name};
use ori_types::Idx;

use crate::builder::{Builder, Locals};
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Try to compile a built-in method call.
    ///
    /// Returns `Some` if the method was handled as a built-in, `None` otherwise
    /// (indicating the caller should fall back to user method lookup).
    pub(crate) fn compile_builtin_method(
        &self,
        recv_val: BasicValueEnum<'ll>,
        receiver_type: Idx,
        method: Name,
        arg_ids: &[ExprId],
        arena: &ExprArena,
        expr_types: &[Idx],
        locals: &mut Locals<'ll>,
        function: FunctionValue<'ll>,
        loop_ctx: Option<&LoopContext<'ll>>,
    ) -> Option<BasicValueEnum<'ll>> {
        let method_str = self.cx().interner.lookup(method);

        // Compile arguments
        let args: Vec<BasicValueEnum<'ll>> = arg_ids
            .iter()
            .filter_map(|&arg_id| {
                self.compile_expr(arg_id, arena, expr_types, locals, function, loop_ctx)
            })
            .collect();

        // Verify LLVM value type matches receiver type to prevent panics
        let is_float_val = matches!(recv_val, BasicValueEnum::FloatValue(_));
        let is_int_val = matches!(recv_val, BasicValueEnum::IntValue(_));
        let is_struct_val = matches!(recv_val, BasicValueEnum::StructValue(_));

        // Dispatch based on receiver type.
        // Note: Duration(8) and Size(9) overlap with INFER/SELF_TYPE indices,
        // but that's OK since INFER/SELF_TYPE are never passed to LLVM backend.
        match receiver_type {
            Idx::INT if is_int_val => {
                numeric::compile_int_method(self, recv_val, method_str, &args)
            }
            Idx::FLOAT if is_float_val => {
                numeric::compile_float_method(self, recv_val, method_str, &args)
            }
            Idx::BOOL if is_int_val => {
                numeric::compile_bool_method(self, recv_val, method_str, &args)
            }
            Idx::CHAR if is_int_val => {
                numeric::compile_char_method(self, recv_val, method_str, &args)
            }
            Idx::BYTE if is_int_val => {
                numeric::compile_byte_method(self, recv_val, method_str, &args)
            }
            Idx::DURATION if is_int_val => {
                units::compile_duration_method(self, recv_val, method_str, &args)
            }
            Idx::SIZE if is_int_val => {
                units::compile_size_method(self, recv_val, method_str, &args)
            }
            Idx::ORDERING if is_int_val || is_struct_val => {
                ordering::compile_ordering_method(self, recv_val, method_str, &args)
            }
            // Type mismatch or unknown type - fall back to user method lookup
            _ => None,
        }
    }
}
