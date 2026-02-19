//! Shared emit utilities: icmp/fcmp ordering, string runtime calls, hash combine.

use ori_ir::canon::CanRange;
use ori_types::Idx;

use crate::codegen::expr_lowerer::ExprLowerer;
use crate::codegen::value_id::ValueId;

impl<'scx: 'ctx, 'ctx> ExprLowerer<'_, 'scx, 'ctx, '_> {
    /// Emit `icmp slt/sgt → select` chain returning Ordering i8.
    ///
    /// Delegates to `IrBuilder::emit_icmp_ordering`.
    pub(crate) fn emit_icmp_ordering(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
        signed: bool,
    ) -> ValueId {
        self.builder.emit_icmp_ordering(lhs, rhs, name, signed)
    }

    /// Emit `fcmp olt/ogt → select` chain returning Ordering i8.
    ///
    /// Delegates to `IrBuilder::emit_fcmp_ordering`.
    pub(super) fn emit_fcmp_ordering(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.builder.emit_fcmp_ordering(lhs, rhs, name)
    }

    /// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
    pub(super) fn emit_str_eq_call(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let lhs_ptr = self.alloca_and_store(lhs, &format!("{name}.lhs"));
        let rhs_ptr = self.alloca_and_store(rhs, &format!("{name}.rhs"));

        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let eq_fn = self
            .builder
            .get_or_declare_function("ori_str_eq", &[ptr_ty, ptr_ty], bool_ty);
        self.builder
            .call(eq_fn, &[lhs_ptr, rhs_ptr], name)
            .unwrap_or_else(|| self.builder.const_bool(false))
    }

    /// Call a string runtime function (compare or eq) via alloca+store pattern.
    ///
    /// `func_name` should be `"ori_str_compare"` (returns i8) or similar.
    pub(super) fn emit_str_runtime_call(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        func_name: &str,
        name: &str,
    ) -> ValueId {
        let lhs_ptr = self.alloca_and_store(lhs, &format!("{name}.lhs"));
        let rhs_ptr = self.alloca_and_store(rhs, &format!("{name}.rhs"));

        let ptr_ty = self.builder.ptr_type();
        let i8_ty = self.builder.i8_type();
        let cmp_fn = self
            .builder
            .get_or_declare_function(func_name, &[ptr_ty, ptr_ty], i8_ty);
        self.builder
            .call(cmp_fn, &[lhs_ptr, rhs_ptr], name)
            .unwrap_or_else(|| self.builder.const_i8(1))
    }

    /// Call `ori_str_concat(a: ptr, b: ptr) -> OriStr` via alloca+store pattern.
    pub(super) fn emit_str_concat_call(
        &mut self,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
    ) -> ValueId {
        let lhs_ptr = self.alloca_and_store(lhs, &format!("{name}.lhs"));
        let rhs_ptr = self.alloca_and_store(rhs, &format!("{name}.rhs"));

        let ptr_ty = self.builder.ptr_type();
        let str_ty = self.resolve_type(Idx::STR);
        let concat_fn =
            self.builder
                .get_or_declare_function("ori_str_concat", &[ptr_ty, ptr_ty], str_ty);
        self.builder
            .call(concat_fn, &[lhs_ptr, rhs_ptr], name)
            .unwrap_or_else(|| {
                let zero_len = self.builder.const_i64(0);
                let null_ptr = self.builder.const_null_ptr();
                self.builder
                    .build_struct(str_ty, &[zero_len, null_ptr], name)
            })
    }

    /// Call `ori_str_hash(s: ptr) -> i64` via alloca+store pattern.
    pub(super) fn emit_str_hash_call(&mut self, val: ValueId, name: &str) -> ValueId {
        let val_ptr = self.alloca_and_store(val, &format!("{name}.str"));

        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let hash_fn = self
            .builder
            .get_or_declare_function("ori_str_hash", &[ptr_ty], i64_ty);
        self.builder
            .call(hash_fn, &[val_ptr], name)
            .unwrap_or_else(|| self.builder.const_i64(0))
    }

    /// Emit inline `hash_combine`: `seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))`.
    ///
    /// This is the Boost hash combine algorithm, matching the evaluator's
    /// `function_val_hash_combine` and `hash_combine` in `ori_eval`.
    pub(crate) fn emit_hash_combine(
        &mut self,
        seed: ValueId,
        value: ValueId,
        name: &str,
    ) -> ValueId {
        let golden = self.builder.const_i64(0x9e37_79b9_i64);
        let six = self.builder.const_i64(6);
        let two = self.builder.const_i64(2);

        let seed_shl6 = self.builder.shl(seed, six, &format!("{name}.shl6"));
        let seed_shr2 = self.builder.lshr(seed, two, &format!("{name}.shr2"));

        // value + golden + (seed << 6) + (seed >> 2)
        let sum1 = self.builder.add(value, golden, &format!("{name}.add1"));
        let sum2 = self.builder.add(sum1, seed_shl6, &format!("{name}.add2"));
        let sum3 = self.builder.add(sum2, seed_shr2, &format!("{name}.add3"));

        // seed XOR sum
        self.builder.xor(seed, sum3, &format!("{name}.xor"))
    }

    /// Lower `hash_combine(seed, value)` → inline Boost hash combine.
    ///
    /// This is a free function (not a method), called from `lower_call` dispatch.
    pub(crate) fn lower_builtin_hash_combine(&mut self, args: CanRange) -> Option<ValueId> {
        let arg_ids = self.canon.arena.get_expr_list(args);
        let seed = self.lower(*arg_ids.first()?)?;
        let value = self.lower(*arg_ids.get(1)?)?;
        Some(self.emit_hash_combine(seed, value, "hash_combine"))
    }
}
