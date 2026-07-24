use ori_arc::ir::{ArcFunction, ArcVarId, CtorKind, RcStrategy, ValueRepr};
use ori_types::Idx;

use super::super::context::{is_boxed_enum_field, EmittedValue};
use super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Releases every owned field except the transferred field ordinals.
    pub(super) fn emit_burden_dec_partial(
        &mut self,
        var: ArcVarId,
        skip_fields: &[u32],
        func: &ArcFunction,
    ) {
        let base_type = func.var_type(var);
        let Some(drop_info) = ori_arc::compute_drop_info(base_type, self.classifier, self.pool)
        else {
            return;
        };
        let base_value = self.var_field_base_ptr(var, base_type);

        match drop_info.kind {
            ori_arc::DropKind::Fields { fields, .. } => self.emit_drop_field_loop(
                base_value,
                base_type,
                &fields,
                Some(skip_fields),
                "burden_dec_partial",
            ),
            ori_arc::DropKind::Enum { variants, .. } => {
                let surviving: Vec<Vec<(u32, Idx)>> = variants
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, fields)| {
                        let Ok(ordinal) = u32::try_from(ordinal) else {
                            return fields;
                        };
                        if skip_fields.contains(&ordinal) {
                            Vec::new()
                        } else {
                            fields
                        }
                    })
                    .collect();
                self.emit_variant_burden_walk(
                    self.current_function,
                    base_value,
                    base_type,
                    &surviving,
                );
            }
            other => {
                debug_assert!(
                    false,
                    "BurdenDecPartial on unsupported drop shape: {other:?}"
                );
                self.builder.record_codegen_error_with_msg(format!(
                    "BurdenDecPartial on unsupported drop shape: {other:?}"
                ));
            }
        }
    }

    /// Releases the active payload before an enum tag mutation invalidates it.
    pub(super) fn emit_burden_dec_variant(&mut self, var: ArcVarId, func: &ArcFunction) {
        let base_type = func.var_type(var);
        let Some(drop_info) = ori_arc::compute_drop_info(base_type, self.classifier, self.pool)
        else {
            return;
        };
        let ori_arc::DropKind::Enum { variants, .. } = drop_info.kind else {
            debug_assert!(
                false,
                "BurdenDecVariant on non-enum drop shape: {:?}",
                drop_info.kind
            );
            self.builder.record_codegen_error_with_msg(format!(
                "BurdenDecVariant on non-enum drop shape: {:?}",
                drop_info.kind
            ));
            return;
        };

        let base_value = self.var_field_base_ptr(var, base_type);
        self.emit_variant_burden_walk(self.current_function, base_value, base_type, &variants);
    }

    /// Releases one owned aggregate field selected by declaration ordinal.
    pub(super) fn emit_burden_dec_field(&mut self, base: ArcVarId, field: u32, func: &ArcFunction) {
        let repr = super::super::emitter_utils::required_var_repr(base, func);
        if repr == ValueRepr::Scalar {
            tracing::trace!(
                base = base.raw(),
                field,
                ?repr,
                "BurdenDecField on scalar base — skipping (unreachable)"
            );
            return;
        }

        let base_type = func.var_type(base);
        let fields = self.pool.struct_fields(base_type);
        let field_index = usize::try_from(field).unwrap_or(usize::MAX);
        let Some(&(_, field_type)) = fields.get(field_index) else {
            self.builder.record_codegen_error_with_msg(format!(
                "BurdenDecField field {field} is outside the fields of v{}",
                base.raw()
            ));
            return;
        };
        let base_value = self.var_field_base_ptr(base, base_type);
        self.emit_drop_field_loop(
            base_value,
            base_type,
            &[(field, field_type)],
            None,
            "burden_dec_field",
        );
    }

    /// Defines `dst` from the runtime sharing state of an RC-pointer value.
    pub(super) fn emit_is_shared(&mut self, dst: ArcVarId, var: ArcVarId, func: &ArcFunction) {
        let repr = super::super::emitter_utils::required_var_repr(var, func);
        if repr != ValueRepr::RcPointer {
            tracing::trace!(
                var = var.raw(),
                ?repr,
                "IsShared on non-pointer value — emitting true"
            );
            let always_shared = self.builder.const_bool(true);
            self.def_var(dst, EmittedValue::Immediate(always_shared));
            return;
        }

        let data_pointer = self.var(var);
        let i8_type = self.builder.i8_type();
        let header_offset = self.builder.const_i64(-8);
        let rc_pointer = self
            .builder
            .gep(i8_type, data_pointer, &[header_offset], "rc_ptr");
        let i64_type = self.builder.i64_type();
        let ref_count = self.builder.load(i64_type, rc_pointer, "rc_val");
        let one = self.builder.const_i64(1);
        let is_shared = self.builder.icmp_sgt(ref_count, one, "is_shared");
        self.def_var(dst, EmittedValue::Immediate(is_shared));
    }

    /// Consumes a rejected reuse token before constructing its replacement value.
    pub(super) fn emit_reuse_fallback(
        &mut self,
        token: ArcVarId,
        dst: ArcVarId,
        ty: Idx,
        ctor: &CtorKind,
        args: &[ArcVarId],
        func: &ArcFunction,
    ) {
        tracing::debug!("Reuse was not expanded; using Construct fallback");
        let repr = super::super::emitter_utils::required_var_repr(token, func);
        let strategy = RcStrategy::from_repr(repr, self.pool, func.var_type(token));
        self.emit_rc_dec(token, strategy, func);
        let value = self.emit_construct(ty, ctor, args);
        self.def_var_repr(dst, value, func);
    }

    /// Stores a value into an RC-pointer aggregate using its compiled field layout.
    pub(super) fn emit_set_field(
        &mut self,
        base: ArcVarId,
        field: u32,
        value: ArcVarId,
        func: &ArcFunction,
    ) {
        let repr = super::super::emitter_utils::required_var_repr(base, func);
        if repr != ValueRepr::RcPointer {
            self.builder.record_codegen_error_with_msg(format!(
                "Set field {field} requires an RC-pointer base, but v{} has {repr:?}",
                base.raw()
            ));
            return;
        }

        let base_value = self.var(base);
        let new_value = self.var(value);
        let base_type = func.var_type(base);
        let llvm_type = self.resolve_type(base_type);
        assert!(
            self.get_tagged_ptr_encoding(base_type).is_none(),
            "compiled layout must resolve tagged-pointer Set before LLVM emission"
        );

        let memory_field = self.remap_struct_field(base_type, field);
        let field_index = usize::try_from(field).unwrap_or(usize::MAX);
        let Some(&(_, field_type)) = self.pool.struct_fields(base_type).get(field_index) else {
            self.builder.record_codegen_error_with_msg(format!(
                "Set field {field} is outside the fields of v{}",
                base.raw()
            ));
            return;
        };

        let stored_value = if is_boxed_enum_field(self.pool, base_type, field_type) {
            self.box_recursive_field(new_value, field_type, Some(value))
        } else {
            new_value
        };
        let field_pointer = self.builder.struct_gep(
            llvm_type,
            base_value,
            memory_field,
            &format!("set.{field}.ptr"),
        );
        self.builder.store(stored_value, field_pointer);
    }

    /// Stores a validated enum tag through the selected compiled encoding.
    pub(super) fn emit_set_tag(&mut self, base: ArcVarId, raw_tag: u64, func: &ArcFunction) {
        let base_value = self.var(base);
        let base_type = func.var_type(base);
        let llvm_type = self.resolve_type(base_type);
        let Ok(tag) = u32::try_from(raw_tag) else {
            self.builder.record_codegen_error_with_msg(format!(
                "SetTag value {raw_tag} exceeds the supported u32 tag range"
            ));
            return;
        };

        if self.get_tagged_ptr_encoding(base_type).is_some() {
            let pointer = self.tagged_ptr_decode_ptr(base_value, "set_tag.ptr");
            let updated = self.tagged_ptr_encode(pointer, tag, "set_tag");
            self.def_var(base, EmittedValue::Immediate(updated));
            return;
        }
        if self.is_tagless_enum(base_type) {
            return;
        }

        if let Some(encoding) = self.get_niche_encoding(base_type) {
            if !encoding.needs_tag_store(tag) {
                return;
            }
            let Some(niche_index) = encoding.niche_field_index() else {
                self.builder
                    .record_codegen_error_with_msg("niche encoding has no field index");
                return;
            };
            let niche_value = encoding.variant_to_tag_value(tag);
            if self.builder.is_struct_value(base_value) {
                let value =
                    self.builder
                        .const_int_for_struct_field(llvm_type, niche_index, niche_value);
                let updated =
                    self.builder
                        .insert_value(base_value, value, niche_index, "set.niche");
                self.def_var(base, EmittedValue::Aggregate(updated));
            } else {
                let pointer =
                    self.builder
                        .struct_gep(llvm_type, base_value, niche_index, "set.niche.ptr");
                let value =
                    self.builder
                        .const_int_for_struct_field(llvm_type, niche_index, niche_value);
                self.builder.store(value, pointer);
            }
            return;
        }

        let tag_pointer = self
            .builder
            .struct_gep(llvm_type, base_value, 0, "set.tag.ptr");
        let tag_value = self
            .builder
            .const_int_for_struct_field(llvm_type, 0, u64::from(tag));
        self.builder.store(tag_value, tag_pointer);
    }
}
