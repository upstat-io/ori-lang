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
            "unwrap" => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "is_some");
                self.emit_unwrap_branch(
                    is_some,
                    "called `Option.unwrap()` on a `None` value",
                    "opt_unwrap",
                )?;
                // After unwrap branch, guaranteed Some — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 1, "opt.payload")?;
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                self.inc_value_rc(payload, inner_ty, 1);
                Some(payload)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                let tag = self.builder.extract_value(receiver, 0, "opt.tag")?;
                let payload = self.builder.extract_value(receiver, 1, "opt.payload")?;
                let some = self
                    .builder
                    .const_int_matching(tag, ori_ir::OPTION_TAG_SOME as u64);
                let is_some = self.builder.icmp_eq(tag, some, "is_some");

                // RC-retain payload when Some: the select copies payload bytes,
                // creating a second reference to inner RC data. Skip for scalar
                // payloads (no RC) to avoid creating extra blocks that break PHI
                // nodes when unwrap_or is inside a short-circuit `&&`/`||` branch.
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                let needs_rc = !self.classifier.is_scalar(inner_ty);
                if needs_rc {
                    let inc_bb = self.builder.append_block(self.current_function, "uor.inc");
                    let merge_bb = self
                        .builder
                        .append_block(self.current_function, "uor.merge");
                    self.builder.cond_br(is_some, inc_bb, merge_bb);
                    self.builder.position_at_end(inc_bb);
                    self.inc_value_rc(payload, inner_ty, 1);
                    self.builder.br(merge_bb);
                    self.builder.position_at_end(merge_bb);
                }

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
                // After expect branch, guaranteed Some — retain unconditionally.
                let payload = self.builder.extract_value(receiver, 1, "opt.payload")?;
                let inner_ty = self.pool.option_inner(self.pool.resolve_fully(receiver_ty));
                self.inc_value_rc(payload, inner_ty, 1);
                Some(payload)
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
            return self.emit_result_niche(method, receiver, arg_vals, receiver_ty, &encoding);
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
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let ok = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
                let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");
                self.emit_unwrap_branch(
                    is_ok,
                    "called `Result.unwrap()` on an `Err` value",
                    "res_unwrap",
                )?;
                // After unwrap branch, guaranteed Ok — retain unconditionally.
                let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
                self.inc_value_rc(payload, ok_ty, 1);
                Some(payload)
            }
            "unwrap_err" => {
                let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
                let err_tag = self
                    .builder
                    .const_int_matching(tag, ori_ir::RESULT_TAG_ERR as u64);
                let is_err = self.builder.icmp_eq(tag, err_tag, "is_err");
                self.emit_unwrap_branch(
                    is_err,
                    "called `Result.unwrap_err()` on an `Ok` value",
                    "res_unwrap_err",
                )?;
                // After unwrap branch, guaranteed Err — retain unconditionally.
                let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
                    return self.builder.extract_value(receiver, 1, "res.payload");
                };
                let payload =
                    self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
                self.inc_value_rc(payload, err_ty, 1);
                Some(payload)
            }
            "unwrap_or" if arg_vals.len() >= 2 => {
                self.emit_result_unwrap_or(receiver, receiver_ty, arg_vals[1])
            }
            "expect" if arg_vals.len() >= 2 => {
                self.emit_result_expect(receiver, receiver_ty, arg_vals[1])
            }
            "expect_err" if arg_vals.len() >= 2 => {
                self.emit_result_expect_err(receiver, receiver_ty, arg_vals[1])
            }
            "ok" => self.emit_result_ok_projection(receiver, receiver_ty),
            "err" => self.emit_result_err_projection(receiver, receiver_ty),
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

    /// `Result<T, E>.ok() -> Option<T>`
    ///
    /// Tag convention matches: Ok(0)→Some(0), Err(1)→None(1).
    /// Emits conditional RC retain on the Ok payload to prevent double-free
    /// when both the Result and the new Option share inner RC-managed data.
    fn emit_result_ok_projection(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
        let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
            return None;
        };
        let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;

        // RC-retain the Ok payload: building a new Option copies the
        // payload bytes, creating a second reference to inner RC data.
        // Only when tag==Ok — for Err, the payload field holds Err data
        // reinterpreted as ok_ty; incrementing it would corrupt RC state.
        let ok_const = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
        let is_ok = self.builder.icmp_eq(tag, ok_const, "ok.is_ok");
        let inc_bb = self
            .builder
            .append_block(self.current_function, "ok.rc_inc");
        let merge_bb = self.builder.append_block(self.current_function, "ok.merge");
        self.builder.cond_br(is_ok, inc_bb, merge_bb);
        self.builder.position_at_end(inc_bb);
        self.inc_value_rc(payload, ok_ty, 1);
        self.builder.br(merge_bb);
        self.builder.position_at_end(merge_bb);

        self.build_option_struct(tag, payload, ok_ty)
    }

    /// `Result<T, E>.err() -> Option<E>`
    ///
    /// Tag must flip: Err(1)→Some(0), Ok(0)→None(1).
    /// Emits conditional RC retain on the Err payload to prevent double-free
    /// when both the Result and the new Option share inner RC-managed data.
    fn emit_result_err_projection(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
        let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
            return None;
        };
        let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;

        // RC-retain the Err payload: building a new Option copies the
        // payload bytes, creating a second reference to inner RC data.
        // Only when tag==Err — for Ok, the payload field holds Ok data
        // reinterpreted as err_ty; incrementing it would corrupt RC state.
        let err_const = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_ERR as u64);
        let is_err = self.builder.icmp_eq(tag, err_const, "err.is_err");
        let inc_bb = self
            .builder
            .append_block(self.current_function, "err.rc_inc");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "err.merge");
        self.builder.cond_br(is_err, inc_bb, merge_bb);
        self.builder.position_at_end(inc_bb);
        self.inc_value_rc(payload, err_ty, 1);
        self.builder.br(merge_bb);
        self.builder.position_at_end(merge_bb);

        // Flip tag: 0→1, 1→0 via XOR with 1
        let one = self.builder.const_int_matching(tag, 1);
        let flipped_tag = self.builder.xor(tag, one, "err.flip_tag");
        self.build_option_struct(flipped_tag, payload, err_ty)
    }

    /// `Result<T, E>.unwrap_or(default)` with RC retain on Ok payload.
    fn emit_result_unwrap_or(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        default: ValueId,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
        let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
            let payload = self.builder.extract_value(receiver, 1, "res.payload")?;
            let ok = self
                .builder
                .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
            let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");
            return Some(self.builder.select(is_ok, payload, default, "unwrap_or"));
        };
        let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        let ok = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
        let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");

        // RC-retain payload when Ok: the select copies payload bytes.
        let inc_bb = self.builder.append_block(self.current_function, "ruor.inc");
        let merge_bb = self
            .builder
            .append_block(self.current_function, "ruor.merge");
        self.builder.cond_br(is_ok, inc_bb, merge_bb);
        self.builder.position_at_end(inc_bb);
        self.inc_value_rc(payload, ok_ty, 1);
        self.builder.br(merge_bb);
        self.builder.position_at_end(merge_bb);

        Some(self.builder.select(is_ok, payload, default, "unwrap_or"))
    }

    /// `Result<T, E>.expect(msg)` with RC retain on Ok payload.
    fn emit_result_expect(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        msg: ValueId,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
        let ok = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_OK as u64);
        let is_ok = self.builder.icmp_eq(tag, ok, "is_ok");
        self.emit_expect_branch(is_ok, msg, "res_expect")?;
        // After expect branch, guaranteed Ok — retain unconditionally.
        let TypeInfo::Result { ok: ok_ty, .. } = self.type_info.get(receiver_ty) else {
            return self.builder.extract_value(receiver, 1, "res.payload");
        };
        let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, ok_ty)?;
        self.inc_value_rc(payload, ok_ty, 1);
        Some(payload)
    }

    /// `Result<T, E>.expect_err(msg)` with RC retain on Err payload.
    fn emit_result_expect_err(
        &mut self,
        receiver: ValueId,
        receiver_ty: Idx,
        msg: ValueId,
    ) -> Option<ValueId> {
        let tag = self.builder.extract_value(receiver, 0, "res.tag")?;
        let err_tag = self
            .builder
            .const_int_matching(tag, ori_ir::RESULT_TAG_ERR as u64);
        let is_err = self.builder.icmp_eq(tag, err_tag, "is_err");
        self.emit_expect_branch(is_err, msg, "res_expect_err")?;
        // After expect branch, guaranteed Err — retain unconditionally.
        let TypeInfo::Result { err: err_ty, .. } = self.type_info.get(receiver_ty) else {
            return self.builder.extract_value(receiver, 1, "res.payload");
        };
        let payload = self.extract_tagged_union_payload(receiver, receiver_ty, 1, err_ty)?;
        self.inc_value_rc(payload, err_ty, 1);
        Some(payload)
    }
}
