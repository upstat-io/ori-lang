//! Tagless single-variant enum (`EnumTag::None`) codegen.
//!
//! A tagless enum (e.g. `type W = Wrap(s: str)`, `type B = ToA(a: A)`) has no
//! discriminant and no niche field. Its LLVM type is a plain struct of the
//! single variant's non-void payload fields (see `resolve_enum_tagless`), so
//! Construct / Project / drop / RC treat it struct-like: direct field GEP at
//! the non-void struct index, with recursive back-edge fields boxed as an
//! 8-byte RC pointer per the boxing SSOT (`position_is_rc_boxed`).
//!
//! Distinct from niche encoding (which carries a sentinel in a payload field)
//! and explicit-tag encoding (`{ tag, [M x i64] payload }`).

use ori_arc::ir::ArcFunction;
use ori_arc::ir::ArcVarId;
use ori_types::{Idx, Tag};

use super::context::is_boxed_enum_field;
use super::ArcIrEmitter;
use crate::codegen::value_id::ValueId;

/// One non-void payload field of a tagless single-variant enum.
struct TaglessField {
    /// Index into the LLVM struct body (non-void fields only, in order).
    struct_index: u32,
    /// Declaration-order field index in the variant.
    decl_index: u32,
    /// The field's Ori type.
    field_type: Idx,
}

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Non-void payload fields of a tagless enum's single variant, paired with
    /// their LLVM struct index.
    ///
    /// Void (Unit/Never) fields are zero-sized and omitted from the tagless
    /// LLVM struct body (`resolve_enum_tagless`), so the struct index of a
    /// field is its position among the non-void fields, NOT its declaration
    /// index.
    fn tagless_fields(&self, ty: Idx) -> Vec<TaglessField> {
        let resolved = self.pool.resolve_fully(ty);
        let variants = self.pool.enum_variants(resolved);
        let Some((_, field_types)) = variants.first() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut struct_index = 0_u32;
        for (decl_index, &field_type) in field_types.iter().enumerate() {
            let rf = self.pool.resolve_fully(field_type);
            if matches!(self.pool.tag(rf), Tag::Unit | Tag::Never) {
                continue;
            }
            #[expect(clippy::cast_possible_truncation, reason = "field count fits u32")]
            let decl = decl_index as u32;
            out.push(TaglessField {
                struct_index,
                decl_index: decl,
                field_type,
            });
            struct_index += 1;
        }
        out
    }

    /// Construct a tagless single-variant enum value.
    ///
    /// Builds the struct directly: recursive back-edge fields are boxed (the
    /// slot holds an RC `ptr`), all other fields are inserted inline. The
    /// `arg_vals` are in declaration order (matching the variant's field
    /// order); void args do not appear because the ARC IR omits them for
    /// zero-sized payloads.
    pub(super) fn emit_tagless_variant_construct(
        &mut self,
        llvm_ty: super::super::value_id::LLVMTypeId,
        ty: Idx,
        arg_vals: &[ValueId],
        args: &[ArcVarId],
    ) -> ValueId {
        let fields = self.tagless_fields(ty);
        let mut result = self.builder.const_zero_ty(llvm_ty);
        for (i, field) in fields.iter().enumerate() {
            let Some(&val) = arg_vals.get(i) else {
                break;
            };
            let inserted = if is_boxed_enum_field(self.pool, ty, field.field_type) {
                // Recursive back-edge: box the inline value, store the RC ptr.
                self.box_recursive_field(val, field.field_type, args.get(i).copied())
            } else {
                val
            };
            result = self.builder.insert_value(
                result,
                inserted,
                field.struct_index,
                &format!("tagless.f{}", field.decl_index),
            );
        }
        result
    }

    /// Project a single field out of a tagless single-variant enum.
    ///
    /// `field` follows the tagged-union Project convention: field 0 is the
    /// discriminant (handled by the caller), payload field N is declaration
    /// index `N - 1`. A boxed recursive field is loaded through its RC `ptr`;
    /// all other fields are read directly from the struct slot.
    pub(super) fn emit_project_tagless_field(
        &mut self,
        dst: ArcVarId,
        field_type: Idx,
        val_ty: Idx,
        value: ArcVarId,
        field: u32,
        result_ty: super::super::value_id::LLVMTypeId,
        func: &ArcFunction,
    ) {
        let decl_index = field.saturating_sub(1);
        let Some(tf) = self
            .tagless_fields(val_ty)
            .into_iter()
            .find(|f| f.decl_index == decl_index)
        else {
            // Field maps to a void slot — zero-sized, no value to load.
            let zero = self.builder.const_zero_ty(result_ty);
            self.def_var_repr(dst, zero, func);
            return;
        };

        let val = self.var(value);
        let llvm_val_ty = self.resolve_type(val_ty);
        let alloca = self.builder.alloca(llvm_val_ty, "tagless.proj.alloca");
        self.builder.store(val, alloca);
        let gep = self.builder.struct_gep(
            llvm_val_ty,
            alloca,
            tf.struct_index,
            &format!("tagless.proj.{field}.gep"),
        );
        if is_boxed_enum_field(self.pool, val_ty, field_type) {
            let ptr_ty = self.builder.ptr_type();
            let box_ptr = self
                .builder
                .load(ptr_ty, gep, &format!("tagless.proj.{field}.box"));
            let loaded = self
                .builder
                .load(result_ty, box_ptr, &format!("tagless.proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        } else {
            let loaded = self
                .builder
                .load(result_ty, gep, &format!("tagless.proj.{field}"));
            self.def_var_repr(dst, loaded, func);
        }
    }

    /// Emit the drop body for a tagless single-variant enum.
    ///
    /// `data_ptr` points at the heap-allocated struct. Each RC-bearing payload
    /// field is dec'd in place (boxed recursive fields via the field-type drop
    /// fn through the loaded box pointer). The caller owns free + ret.
    pub(super) fn emit_drop_tagless(&mut self, ty: Idx, data_ptr: ValueId) {
        let enum_llvm_ty = self.resolve_type(ty);
        for field in self.tagless_fields(ty) {
            if !self.payload_needs_rc(self.pool.resolve_fully(ty), field.field_type) {
                continue;
            }
            let gep = self.builder.struct_gep(
                enum_llvm_ty,
                data_ptr,
                field.struct_index,
                &format!("tagless.drop.f{}.ptr", field.decl_index),
            );
            if is_boxed_enum_field(self.pool, ty, field.field_type) {
                let ptr_ty = self.builder.ptr_type();
                let rc_ptr = self.builder.load(
                    ptr_ty,
                    gep,
                    &format!("tagless.drop.f{}.rc", field.decl_index),
                );
                let drop_fn = self.get_or_generate_drop_fn(field.field_type);
                self.emit_drop_rc_dec_with_fn(rc_ptr, drop_fn);
            } else {
                let field_llvm_ty = self.resolve_type(field.field_type);
                let fval = self.builder.load(
                    field_llvm_ty,
                    gep,
                    &format!("tagless.drop.f{}", field.decl_index),
                );
                self.emit_drop_rc_dec(fval, field.field_type);
            }
        }
    }

    /// Emit inline RC inc/dec for a tagless single-variant enum value.
    ///
    /// The value is stored to a temporary alloca, then each RC-bearing payload
    /// field is inc'd/dec'd. Boxed recursive fields operate on the loaded box
    /// pointer directly (inc) or via the field-type drop fn (dec); inline
    /// fields recurse through `inc_value_rc` / `dec_value_rc`.
    pub(super) fn emit_inline_tagless_rc(
        &mut self,
        val: ValueId,
        ty: Idx,
        is_inc: bool,
        count: u32,
    ) {
        let fields = self.tagless_fields(ty);
        let resolved_ty = self.pool.resolve_fully(ty);
        let rc_fields: Vec<TaglessField> = fields
            .into_iter()
            .filter(|f| self.payload_needs_rc(resolved_ty, f.field_type))
            .collect();
        if rc_fields.is_empty() {
            return;
        }

        let dir = if is_inc { "rc_inc" } else { "rc_dec" };
        let enum_llvm_ty = self.resolve_type(ty);
        let alloca = self.builder.alloca(enum_llvm_ty, &format!("{dir}.tagless"));
        self.builder.store(val, alloca);

        // Dec walks payload fields in REVERSE declaration order (LIFO) per
        // `drop-trait-proposal.md §Drop and panic` — matching the struct field
        // walk (`dec_aggregate_fields`) so a user `@drop` on a payload field
        // observes consistent teardown order (and a sibling already dropped
        // before a later field's `@drop` panics). Inc keeps forward order
        // (unobservable for inc).
        // Order per the `emitter_utils::field_rc_walk_order` LIFO contract
        // (borrowed-field variant — collects `&TaglessField`, so it keeps its
        // own collect rather than the `Copy`-bounded helper).
        let ordered: Vec<&TaglessField> = if is_inc {
            rc_fields.iter().collect()
        } else {
            rc_fields.iter().rev().collect()
        };
        for field in ordered {
            let gep = self.builder.struct_gep(
                enum_llvm_ty,
                alloca,
                field.struct_index,
                &format!("{dir}.tagless.f{}.ptr", field.decl_index),
            );
            if is_boxed_enum_field(self.pool, ty, field.field_type) {
                let ptr_ty = self.builder.ptr_type();
                let rc_ptr = self.builder.load(
                    ptr_ty,
                    gep,
                    &format!("{dir}.tagless.f{}.rc", field.decl_index),
                );
                if is_inc {
                    self.call_rc_inc_all(&[rc_ptr], count);
                } else {
                    // Boxed field: its `_ori_drop$<field_ty>` carries the field
                    // type's user `@drop` (if any) inside the dec.
                    let drop_fn = self.get_or_generate_drop_fn(field.field_type);
                    self.call_rc_dec_all(&[rc_ptr], drop_fn);
                }
            } else {
                let field_llvm_ty = self.resolve_type(field.field_type);
                let fval = self.builder.load(
                    field_llvm_ty,
                    gep,
                    &format!("{dir}.tagless.f{}", field.decl_index),
                );
                if is_inc {
                    self.inc_value_rc(fval, field.field_type, count);
                } else {
                    // Inline payload field with a user `@drop` (e.g. a Drop
                    // struct in a single-variant enum payload): run its `@drop`
                    // before recursing into its own field walk — matching
                    // `dec_aggregate_fields`. A panic here propagates through
                    // the (may-unwind) enclosing drop/elem-dec fn's cleanup pad.
                    self.emit_user_drop_for_inline_value(field.field_type, fval);
                    self.dec_value_rc(fval, field.field_type);
                }
            }
        }
    }
}
