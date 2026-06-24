//! Enum `FormatFields` derive codegen: Printable (`to_str`) and Debug (`debug`).
//!
//! Switches on the variant tag; each variant block builds its own string.
//! Variant output mirrors the evaluator (`ori_eval` `eval_format_fields`
//! `Value::Variant` arm): `VariantName` for a payload-less variant, else
//! `VariantName(f0, f1, ...)` with `separator` between fields. The `open` /
//! `suffix` / `include_names` struct delimiters do NOT apply to variants — the
//! per-field rendering is the only Debug-vs-Printable difference, carried by
//! `trait_kind` through `emit_field_to_string` (Debug quotes strings).

use ori_ir::DerivedTrait;
use ori_types::VariantDef;

use super::super::super::function_compiler::FunctionCompiler;
use super::super::super::type_info::TypeLayoutResolver;
use super::super::super::value_id::{LLVMTypeId, ValueId};
use super::super::string_helpers::{
    emit_field_to_string, emit_str_concat, emit_str_literal, emit_str_rc_dec,
};
use super::super::{emit_derive_return, setup_derive_function, verify_derive_function};

use super::variant_non_void_field_types;

/// Generate a derived `FormatFields` method (Printable / Debug) for an enum.
///
/// Emits a switch on the variant tag; each variant block returns its own
/// formatted string. The default block is unreachable (tags are exhaustive)
/// and traps fail-loud via `unreachable`.
pub(super) fn compile_enum_format<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: ori_ir::Name,
    type_idx: ori_types::Idx,
    type_name_str: &str,
    variants: &[VariantDef],
    separator: &str,
    mono: bool,
) {
    let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str, mono);
    let self_val = setup.self_val.expect("FormatFields has self");
    let str_ty_id = setup.str_ty_id.expect("FormatFields needs str_ty_id");
    let func_id = setup.func_id;

    let Some(tag) = fc.builder_mut().extract_value(self_val, 0, "fmt.tag") else {
        tracing::warn!("extract_value failed for enum tag in derive FormatFields");
        let empty = emit_str_literal(fc, "", "fmt.empty", str_ty_id);
        emit_derive_return(fc, func_id, &setup.abi, Some(empty));
        return;
    };

    let default_bb = fc.builder_mut().append_block(func_id, "fmt.default");

    // A unit-only enum (no variant carries a non-void payload field) has a
    // tag-only LLVM layout with no payload slot at field index 1 — never
    // extract a payload for it.
    let has_payload = variants
        .iter()
        .any(|v| !variant_non_void_field_types(&v.fields, fc.pool()).is_empty());

    // Single-slot fast path mirrors enum_eq: extractvalue chains on the payload
    // aggregate when every variant field occupies exactly one i64 slot;
    // otherwise alloca+store+GEP loads multi-word fields (e.g. `str`).
    let all_single_slot = fc.builder_mut().is_struct_value(self_val)
        && variants.iter().all(|v| {
            variant_non_void_field_types(&v.fields, fc.pool())
                .iter()
                .all(|&ft| {
                    let llvm_ty = fc.resolve_type(ft);
                    let ty_id = fc.builder_mut().register_type(llvm_ty);
                    fc.builder_mut().is_single_slot_type(ty_id)
                })
        });

    // Build one switch case (block) per variant.
    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("fmt.v.{variant_name}"));
        let tag_val = fc.builder_mut().const_int_matching(tag, tag_idx as u64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    // Payload source for the chosen path. Only built when a variant carries a
    // payload; an absent source pairs with empty `field_types` in every block.
    let payload_src = if !has_payload {
        None
    } else if all_single_slot {
        fc.builder_mut()
            .extract_value(self_val, 1, "fmt.payload")
            .map(PayloadSrc::Aggregate)
    } else {
        let enum_llvm_ty = fc.resolve_type(setup.type_idx);
        let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
        let self_alloca = fc.entry_alloca(enum_ty_id, "fmt.self");
        fc.builder_mut().store(self_val, self_alloca);
        let payload_ptr = fc
            .builder_mut()
            .struct_gep(enum_ty_id, self_alloca, 1, "fmt.payload");
        Some(PayloadSrc::Pointer { ptr: payload_ptr })
    };

    fc.builder_mut().switch(tag, default_bb, &cases);

    for (tag_idx, variant) in variants.iter().enumerate() {
        fc.builder_mut().position_at_end(variant_bbs[tag_idx]);
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let field_types = variant_non_void_field_types(&variant.fields, fc.pool());

        if field_types.is_empty() {
            // Payload-less variant: just the variant name.
            let result = emit_str_literal(fc, &variant_name, &format!("fmt.v{tag_idx}"), str_ty_id);
            emit_derive_return(fc, func_id, &setup.abi, Some(result));
            continue;
        }

        let prefix = format!("{variant_name}(");
        let mut result = emit_str_literal(fc, &prefix, &format!("fmt.v{tag_idx}.pre"), str_ty_id);

        // `field_types` non-empty implies `has_payload`, so `payload_src` is Some.
        let Some(ref src) = payload_src else {
            emit_derive_return(fc, func_id, &setup.abi, Some(result));
            continue;
        };

        result = emit_variant_payload_fields(
            fc,
            trait_kind,
            setup.type_idx,
            src,
            &field_types,
            separator,
            str_ty_id,
            tag_idx,
            result,
        );

        let suffix = emit_str_literal(fc, ")", &format!("fmt.v{tag_idx}.suf"), str_ty_id);
        let old = result;
        result = emit_str_concat(
            fc,
            old,
            suffix,
            &format!("fmt.v{tag_idx}.catsuf"),
            str_ty_id,
        );
        emit_str_rc_dec(fc, old, &format!("fmt.v{tag_idx}.dec.presuf"));
        emit_str_rc_dec(fc, suffix, &format!("fmt.v{tag_idx}.dec.suf"));

        emit_derive_return(fc, func_id, &setup.abi, Some(result));
    }

    // Tags are exhaustive, so this is unreachable; trap fail-loud on tag
    // corruption (matches the sibling `enum_hashable`/`enum_eq` default arm).
    fc.builder_mut().position_at_end(default_bb);
    fc.builder_mut().unreachable();

    verify_derive_function(fc, func_id, "compile_enum_format");
}

/// Concatenate one variant's payload fields onto `result`, separator-joined.
///
/// Appends `f0`, `separator`, `f1`, ... to the running `result` string and
/// returns it; the caller wraps it with the variant-name prefix and `)` suffix.
#[expect(clippy::too_many_arguments, reason = "codegen emission context")]
fn emit_variant_payload_fields<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_idx: ori_types::Idx,
    src: &PayloadSrc,
    field_types: &[ori_types::Idx],
    separator: &str,
    str_ty_id: LLVMTypeId,
    tag_idx: usize,
    mut result: ValueId,
) -> ValueId {
    let mut i64_offset: u64 = 0;
    for (fi, &field_type) in field_types.iter().enumerate() {
        if fi > 0 {
            let sep =
                emit_str_literal(fc, separator, &format!("fmt.v{tag_idx}.sep{fi}"), str_ty_id);
            let old = result;
            result = emit_str_concat(
                fc,
                old,
                sep,
                &format!("fmt.v{tag_idx}.catsep{fi}"),
                str_ty_id,
            );
            emit_str_rc_dec(fc, old, &format!("fmt.v{tag_idx}.dec.presep{fi}"));
            emit_str_rc_dec(fc, sep, &format!("fmt.v{tag_idx}.dec.sep{fi}"));
        }

        // A boxed recursive back-edge field (`Tree` inside `Node`) occupies
        // ONE i64 slot holding an RC `ptr`; load that pointer and render via
        // the pointer path. A non-recursive field is loaded inline.
        let boxed = crate::codegen::type_info::repr_box_oracle::position_is_rc_boxed(
            fc.pool(),
            type_idx,
            field_type,
        );
        let field_val = load_variant_field(fc, src, i64_offset, field_type, tag_idx, fi, boxed);
        let field_str = emit_field_to_string(
            fc,
            trait_kind,
            field_val,
            field_type,
            &format!("fmt.v{tag_idx}.f{fi}"),
            str_ty_id,
            boxed,
        );
        let old = result;
        result = emit_str_concat(
            fc,
            old,
            field_str,
            &format!("fmt.v{tag_idx}.catval{fi}"),
            str_ty_id,
        );
        emit_str_rc_dec(fc, old, &format!("fmt.v{tag_idx}.dec.preval{fi}"));
        emit_str_rc_dec(fc, field_str, &format!("fmt.v{tag_idx}.dec.val{fi}"));

        // A boxed field is a single `ptr` slot; an inline field spans its
        // own i64-rounded store size.
        if boxed {
            i64_offset += 1;
        } else {
            let field_llvm_ty = fc.resolve_type(field_type);
            let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
            i64_offset += field_bytes.div_ceil(8).max(1);
        }
    }
    result
}

/// Where a variant's payload lives for field loading.
enum PayloadSrc {
    /// Single-slot fast path: the `[N x i64]` payload aggregate in registers.
    Aggregate(ValueId),
    /// Multi-word slow path: a pointer to the payload, GEP'd per field.
    Pointer { ptr: ValueId },
}

/// Load variant field `fi` (at `i64_offset` payload slots) as its concrete type.
///
/// When `boxed` the slot holds an RC `ptr` to a heap value (recursive back-edge
/// per `repr_box_oracle`); load that pointer rather than the inline value.
fn load_variant_field<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    payload_src: &PayloadSrc,
    i64_offset: u64,
    field_type: ori_types::Idx,
    tag_idx: usize,
    fi: usize,
    boxed: bool,
) -> ValueId {
    let field_ty_id = if boxed {
        fc.builder_mut().ptr_type()
    } else {
        let field_llvm_ty = fc.resolve_type(field_type);
        fc.builder_mut().register_type(field_llvm_ty)
    };
    match *payload_src {
        PayloadSrc::Aggregate(payload) => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "payload slot offset bounded by variant layout"
            )]
            let raw = fc.builder_mut().extract_value_any(
                payload,
                i64_offset as u32,
                &format!("fmt.v{tag_idx}.f{fi}.raw"),
            );
            fc.builder_mut().reinterpret_from_i64(
                raw,
                field_ty_id,
                &format!("fmt.v{tag_idx}.f{fi}.val"),
            )
        }
        PayloadSrc::Pointer { ptr, .. } => {
            let i64_ty = fc.builder_mut().i64_type();
            #[expect(
                clippy::cast_possible_wrap,
                reason = "payload slot offset bounded by variant layout"
            )]
            let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
            let slot = fc.builder_mut().gep(
                i64_ty,
                ptr,
                &[slot_idx],
                &format!("fmt.v{tag_idx}.f{fi}.slot"),
            );
            fc.builder_mut()
                .load(field_ty_id, slot, &format!("fmt.v{tag_idx}.f{fi}.val"))
        }
    }
}
