//! Option and Result builtin methods.
//!
//! ARC lowering convention (definition order):
//! Option layout: `{i64 tag, T payload}` — tag 0=Some, 1=None
//! Result layout: `{i64 tag, payload}`   — tag 0=Ok, 1=Err
//!
//! **Result payload caveat**: The payload slot is sized for the *larger*
//! of Ok/Err. When extracting the smaller variant's payload, we must use
//! alloca + GEP + load (pointer reinterpretation) instead of
//! `extract_value` which returns the storage type, not the variant type.

declare_builtins! { emitter, ctx;
    // Option methods
    ("Option", "is_some", borrow: true) => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "is_none", borrow: true) => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "unwrap", borrow: true) => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "unwrap_or", borrow: true) => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "clone", borrow: true) => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    // Result methods
    ("Result", "is_ok", borrow: true) => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "is_err", borrow: true) => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "unwrap", borrow: true) => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "unwrap_err", borrow: true) => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "unwrap_or", borrow: true) => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "clone", borrow: true) => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
}

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an Option method (`is_some`, `is_none`, `unwrap`).
    pub(crate) fn emit_option_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        _receiver_ty: Idx,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];

        match method {
            "is_some" => {
                // Some = variant 0 → tag == 0
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(tag, zero, "is_some"))
            }
            "is_none" => {
                // None = variant 1 → tag == 1
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let one = self.builder.const_i64(1);
                Some(self.builder.icmp_eq(tag, one, "is_none"))
            }
            "unwrap" => {
                // Extract payload (field 1). Panics on None (handled by ARC IR).
                self.builder.extract_value(receiver, 1, "opt.payload")
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                // tag == 0 (Some) → payload, else → default
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let payload = self.builder.extract_value(receiver, 1, "opt.payload")?;
                let zero = self.builder.const_i64(0);
                let is_some = self.builder.icmp_eq(tag, zero, "is_some");
                Some(
                    self.builder
                        .select(is_some, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "clone" => {
                // Option clone: clone the whole aggregate
                Some(receiver)
            }
            _ => None,
        }
    }

    /// Emit a Result method (`is_ok`, `is_err`, `unwrap`, `unwrap_err`).
    ///
    /// Uses alloca-based extraction for payload fields because the storage
    /// slot may be larger than the variant's actual type.
    pub(crate) fn emit_result_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];

        match method {
            "is_ok" => {
                // Ok = variant 0 → tag == 0
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let zero = self.builder.const_i64(0);
                Some(self.builder.icmp_eq(tag, zero, "is_ok"))
            }
            "is_err" => {
                // Err = variant 1 → tag == 1
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let one = self.builder.const_i64(1);
                Some(self.builder.icmp_eq(tag, one, "is_err"))
            }
            "unwrap" => {
                // Extract Ok payload (field 1) via alloca for type safety.
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)
            }
            "unwrap_err" => {
                // Extract Err payload (field 1) via alloca for type safety.
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                // tag == 0 (Ok) → ok_payload, else → default
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    let payload = self.builder.extract_value(receiver, 1, "res.payload")?;
                    let zero = self.builder.const_i64(0);
                    let is_ok = self.builder.icmp_eq(tag, zero, "is_ok");
                    return Some(
                        self.builder
                            .select(is_ok, payload, arg_vals[1], "unwrap_or"),
                    );
                };
                let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
                let zero = self.builder.const_i64(0);
                let is_ok = self.builder.icmp_eq(tag, zero, "is_ok");
                Some(
                    self.builder
                        .select(is_ok, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "clone" => Some(receiver),
            _ => None,
        }
    }

    /// Extract a payload from a tagged union (Result/Enum) via alloca + GEP.
    ///
    /// This handles the case where the storage type of `field` differs from
    /// the actual variant's payload type (e.g., `int` stored in a `{i64, ptr}`
    /// slot of `Result<int, str>`).
    pub(in crate::codegen::arc_emitter) fn extract_tagged_union_payload(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        field: u32,
        payload_ty: Idx,
    ) -> Option<ValueId> {
        let llvm_recv_ty = self.resolve_type(receiver_ty);
        let llvm_payload_ty = self.resolve_type(payload_ty);
        let alloca = self.builder.alloca(llvm_recv_ty, "res.alloca");
        self.builder.store(receiver, alloca);
        let gep = self
            .builder
            .struct_gep(llvm_recv_ty, alloca, field, "res.payload.gep");
        Some(self.builder.load(llvm_payload_ty, gep, "res.payload"))
    }
}
