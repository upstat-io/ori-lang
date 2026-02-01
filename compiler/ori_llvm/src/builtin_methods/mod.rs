//! Built-in method compilation for LLVM backend.
//!
//! This module provides direct enum-based dispatch for built-in method calls,
//! mirroring the architecture in `ori_typeck/src/infer/builtin_methods/` and
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

use std::collections::HashMap;

use inkwell::values::{BasicValueEnum, FunctionValue};
use ori_ir::{ExprArena, ExprId, Name, TypeId};

use crate::builder::Builder;
use crate::LoopContext;

impl<'ll> Builder<'_, 'll, '_> {
    /// Try to compile a built-in method call.
    ///
    /// Returns `Some` if the method was handled as a built-in, `None` otherwise
    /// (indicating the caller should fall back to user method lookup).
    pub(crate) fn compile_builtin_method(
        &self,
        recv_val: BasicValueEnum<'ll>,
        receiver_type: TypeId,
        method: Name,
        arg_ids: &[ExprId],
        arena: &ExprArena,
        expr_types: &[TypeId],
        locals: &mut HashMap<Name, BasicValueEnum<'ll>>,
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

        // Dispatch based on receiver type.
        // Note: Duration(8) and Size(9) overlap with INFER/SELF_TYPE indices,
        // but that's OK since INFER/SELF_TYPE are never passed to LLVM backend.
        match receiver_type {
            TypeId::INT => numeric::compile_int_method(self, recv_val, method_str, &args),
            TypeId::FLOAT => numeric::compile_float_method(self, recv_val, method_str, &args),
            TypeId::BOOL => numeric::compile_bool_method(self, recv_val, method_str, &args),
            TypeId::CHAR => numeric::compile_char_method(self, recv_val, method_str, &args),
            TypeId::BYTE => numeric::compile_byte_method(self, recv_val, method_str, &args),
            TypeId::DURATION => units::compile_duration_method(self, recv_val, method_str, &args),
            TypeId::SIZE => units::compile_size_method(self, recv_val, method_str, &args),
            TypeId::ORDERING => {
                ordering::compile_ordering_method(self, recv_val, method_str, &args)
            }
            _ => None,
        }
    }
}
