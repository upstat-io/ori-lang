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
    // Option — simple methods
    ("Option", "is_some") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "is_none") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "unwrap") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "unwrap_or") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "expect") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "debug") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Option", "clone") => emitter.emit_option_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    // Option — monadic/closure methods
    ("Option", "map") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Option", "and_then") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Option", "flat_map") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Option", "filter") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Option", "or") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Option", "or_else") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Option", "ok_or") => emitter.emit_option_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    // Option — iter (Iterable trait)
    ("Option", "iter") => {
        if let crate::codegen::type_info::TypeInfo::Option { inner } = ctx.type_info {
            emitter.emit_option_iter(ctx.arg_vals[0], *inner)
        } else {
            None
        }
    },
    // Result — simple methods
    ("Result", "is_ok") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "is_err") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "unwrap") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "unwrap_err") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "unwrap_or") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "expect") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "expect_err") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "debug") => emitter.emit_result_debug(ctx.arg_vals, ctx.receiver_ty),
    ("Result", "ok") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "err") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    ("Result", "clone") => emitter.emit_result_method(ctx.method, ctx.arg_vals, ctx.receiver_ty),
    // Result — monadic/closure methods
    ("Result", "map") => emitter.emit_result_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Result", "map_err") => emitter.emit_result_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Result", "and_then") => emitter.emit_result_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
    ("Result", "or_else") => emitter.emit_result_monadic(ctx.method, ctx.arg_vals, ctx.receiver_ty, ctx.arc_args, ctx.arc_func),
}

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Emit an Option method (`is_some`, `is_none`, `unwrap`, `expect`, `debug`).
    pub(crate) fn emit_option_method(
        &mut self,
        method: &str,
        arg_vals: &[ValueId],
        receiver_ty: Idx,
    ) -> Option<ValueId> {
        let receiver = arg_vals[0];

        // §07.2: Niche-encoded Option — payload IS the struct, no tag field.
        if let Some(encoding) = self.get_niche_encoding(receiver_ty) {
            return self.emit_option_niche(method, receiver, arg_vals, receiver_ty, &encoding);
        }

        // Explicit tag layout: {tag, payload}
        match method {
            "is_some" => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                Some(self.builder.icmp_eq(tag, some, "is_some"))
            }
            "is_none" => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let none = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_NONE as u64);
                Some(self.builder.icmp_eq(tag, none, "is_none"))
            }
            "unwrap" => self.builder.extract_value(receiver, 1, "opt.payload"),
            "unwrap_or" if arg_vals.len() >= 2 => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let payload = self.builder.extract_value(receiver, 1, "opt.payload")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "is_some");
                Some(
                    self.builder
                        .select(is_some, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "is_some");
                self.emit_expect_branch(is_some, arg_vals[1], "expect")?;
                self.builder.extract_value(receiver, 1, "opt.payload")
            }
            "debug" | "to_str" => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "is_some");
                let payload = self.builder.extract_value(receiver, 1, "opt.payload")?;
                let TypeInfo::Option { inner } = self.type_info.get(receiver_ty) else {
                    return None;
                };
                self.emit_option_debug_branch(is_some, payload, inner, method == "debug")
            }
            "clone" => Some(receiver),
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

        // §07.2: Niche-encoded Result — check niche field instead of tag.
        if let Some(encoding) = self.get_niche_encoding(receiver_ty) {
            return self.emit_result_niche(method, receiver, arg_vals, &encoding);
        }

        match method {
            "is_ok" => {
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let ok = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
                Some(self.builder.icmp_eq(tag, ok, "is_ok"))
            }
            "is_err" => {
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let err = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_ERR as u64);
                Some(self.builder.icmp_eq(tag, err, "is_err"))
            }
            "unwrap" => {
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)
            }
            "unwrap_err" => {
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    let payload = self.builder.extract_value(receiver, 1, "res.payload")?;
                    let ok = self
                        .builder
                        .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
                    let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");
                    return Some(
                        self.builder
                            .select(is_ok, payload, arg_vals[1], "unwrap_or"),
                    );
                };
                let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
                let ok = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
                let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");
                Some(
                    self.builder
                        .select(is_ok, payload, arg_vals[1], "unwrap_or"),
                )
            }
            "expect" if arg_vals.len() >= 2 => {
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let ok = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
                let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");
                self.emit_expect_branch(is_ok, arg_vals[1], "res_expect")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)
            }
            "expect_err" if arg_vals.len() >= 2 => {
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let err_tag = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_ERR as u64);
                let is_err = self.builder.icmp_eq(tag, err_tag, "is_err");
                self.emit_expect_branch(is_err, arg_vals[1], "res_expect_err")?;
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)
            }
            "ok" => {
                // Result<T, E>.ok() -> Option<T>
                // Tag convention matches: Ok(0)→Some(0), Err(1)→None(1).
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return None;
                };
                let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
                self.build_option_struct(tag, payload, ok_ty)
            }
            "err" => {
                // Result<T, E>.err() -> Option<E>
                // Tag must flip: Err(1)→Some(0), Ok(0)→None(1).
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return None;
                };
                let payload =
                    self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
                // Flip tag: 0→1, 1→0 via XOR with 1
                let one = self.builder.const_int_matching(tag, 1);
                let flipped_tag = self.builder.xor(tag, one, "err.flip_tag");
                self.build_option_struct(flipped_tag, payload, err_ty)
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
