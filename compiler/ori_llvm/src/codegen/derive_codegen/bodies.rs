//! Strategy-driven body implementations for derived method codegen.
//!
//! **Struct bodies** — four functions map to the four `StructBody` variants:
//! - `compile_for_each_field` — Eq, Comparable, Hashable
//! - `compile_format_fields` — Printable, Debug
//! - `compile_clone_fields` — Clone
//! - `compile_default_construct` — Default
//!
//! **Enum bodies** — `compile_enum_match_variants` handles `SumBody::MatchVariants`:
//! - Tag comparison for Eq, Comparable, Hashable
//! - Clone is identity return (enums are value types in LLVM)
//!
//! The common scaffolding (signature, ABI, function declaration) is handled by
//! `setup_derive_function`; these functions only emit the body logic.

use ori_ir::{CombineOp, DerivedTrait, FieldOp, FormatOpen, Name, StructBody};
use ori_types::{FieldDef, Idx, VariantDef, VariantFields};
use tracing::warn;

use super::super::function_compiler::FunctionCompiler;
use super::super::type_info::TypeLayoutResolver;

use super::field_ops::emit_field_operation;
use super::string_helpers::{emit_field_to_string, emit_str_concat, emit_str_literal};
use super::{emit_derive_return, setup_derive_function, DeriveSetup};

/// FNV-1a offset basis (64-bit).
const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
/// FNV-1a prime (64-bit).
const FNV_PRIME: u64 = 1_099_511_628_211;

// ---------------------------------------------------------------------------
// ForEachField: Eq, Comparable, Hashable
// ---------------------------------------------------------------------------

/// Generate a derived method that applies a per-field operation and combines results.
///
/// Dispatches to per-`CombineOp` helpers:
/// - `AllTrue`: short-circuit AND (Eq)
/// - `Lexicographic`: first non-Equal ordering (Comparable)
/// - `HashCombine`: FNV-1a accumulation (Hashable)
pub(super) fn compile_for_each_field<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
    field_op: FieldOp,
    combine: CombineOp,
) {
    let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
    match combine {
        CombineOp::AllTrue => emit_all_true_body(fc, &setup, fields, field_op),
        CombineOp::Lexicographic => emit_lexicographic_body(fc, &setup, fields, field_op),
        CombineOp::HashCombine => emit_hash_combine_body(fc, &setup, fields, field_op),
    }
}

/// Short-circuit AND: compare each field, branch to `false_bb` on first mismatch.
fn emit_all_true_body<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    fields: &[FieldDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("AllTrue has self");
    let other_val = setup.other_val.expect("AllTrue has other");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("AllTrue needs str_ty_id");

    let true_bb = fc.builder_mut().append_block(func_id, "eq.true");
    let false_bb = fc.builder_mut().append_block(func_id, "eq.false");

    if fields.is_empty() {
        fc.builder_mut().br(true_bb);
    } else {
        for (i, field) in fields.iter().enumerate() {
            let field_name = fc.lookup_name(field.name).to_owned();
            let self_field =
                fc.builder_mut()
                    .extract_value(self_val, i as u32, &format!("self.{field_name}"));
            let other_field =
                fc.builder_mut()
                    .extract_value(other_val, i as u32, &format!("other.{field_name}"));

            let (Some(sf), Some(of)) = (self_field, other_field) else {
                warn!(field = %field_name, "extract_value failed in derive AllTrue");
                fc.builder_mut().br(false_bb);
                break;
            };

            let cmp = emit_field_operation(
                fc,
                field_op,
                sf,
                Some(of),
                field.ty,
                &format!("eq.{field_name}"),
                str_ty_id,
            );

            if i + 1 < fields.len() {
                let next_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("eq.check.{}", i + 1));
                fc.builder_mut().cond_br(cmp, next_bb, false_bb);
                fc.builder_mut().position_at_end(next_bb);
            } else {
                fc.builder_mut().cond_br(cmp, true_bb, false_bb);
            }
        }
    }

    fc.builder_mut().position_at_end(true_bb);
    let true_val = fc.builder_mut().const_bool(true);
    fc.builder_mut().ret(true_val);

    fc.builder_mut().position_at_end(false_bb);
    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut().ret(false_val);
}

/// Lexicographic: compare fields in order, short-circuit on first non-Equal.
fn emit_lexicographic_body<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    fields: &[FieldDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("Lexicographic has self");
    let other_val = setup.other_val.expect("Lexicographic has other");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Lexicographic needs str_ty_id");

    let equal_bb = fc.builder_mut().append_block(func_id, "cmp.equal");

    if fields.is_empty() {
        fc.builder_mut().br(equal_bb);
    } else {
        for (i, field) in fields.iter().enumerate() {
            let field_name = fc.lookup_name(field.name).to_owned();
            let self_field =
                fc.builder_mut()
                    .extract_value(self_val, i as u32, &format!("self.{field_name}"));
            let other_field =
                fc.builder_mut()
                    .extract_value(other_val, i as u32, &format!("other.{field_name}"));

            let (Some(sf), Some(of)) = (self_field, other_field) else {
                warn!(field = %field_name, "extract_value failed in derive Lexicographic");
                fc.builder_mut().br(equal_bb);
                break;
            };

            let ord = emit_field_operation(
                fc,
                field_op,
                sf,
                Some(of),
                field.ty,
                &format!("cmp.{field_name}"),
                str_ty_id,
            );

            // Check if this field's comparison is Equal (1)
            let one = fc.builder_mut().const_i8(1);
            let is_equal = fc
                .builder_mut()
                .icmp_eq(ord, one, &format!("cmp.{field_name}.is_eq"));

            if i + 1 < fields.len() {
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.ret.{field_name}"));
                let next_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.next.{}", i + 1));
                fc.builder_mut().cond_br(is_equal, next_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));

                fc.builder_mut().position_at_end(next_bb);
            } else {
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.ret.{field_name}"));
                fc.builder_mut().cond_br(is_equal, equal_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));
            }
        }
    }

    fc.builder_mut().position_at_end(equal_bb);
    let equal_val = fc.builder_mut().const_i8(1);
    emit_derive_return(fc, func_id, &setup.abi, Some(equal_val));
}

/// FNV-1a accumulation: `hash = (hash ^ field_as_i64) * prime` per field.
fn emit_hash_combine_body<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    fields: &[FieldDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("HashCombine has self");
    let str_ty_id = setup.str_ty_id.expect("HashCombine needs str_ty_id");

    let mut hash = fc.builder_mut().const_i64(FNV_OFFSET_BASIS as i64);
    let prime = fc.builder_mut().const_i64(FNV_PRIME as i64);

    for (i, field) in fields.iter().enumerate() {
        let field_name = fc.lookup_name(field.name).to_owned();
        let field_val =
            fc.builder_mut()
                .extract_value(self_val, i as u32, &format!("hash.{field_name}"));

        let Some(fv) = field_val else {
            warn!(field = %field_name, "extract_value failed in derive HashCombine");
            continue;
        };

        let field_as_i64 = emit_field_operation(
            fc,
            field_op,
            fv,
            None,
            field.ty,
            &format!("hash.{field_name}"),
            str_ty_id,
        );

        let xored = fc
            .builder_mut()
            .xor(hash, field_as_i64, &format!("hash.xor.{i}"));
        hash = fc.builder_mut().mul(xored, prime, &format!("hash.mul.{i}"));
    }

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(hash));
}

// ---------------------------------------------------------------------------
// FormatFields: Printable, Debug
// ---------------------------------------------------------------------------

/// Generate a derived method that formats fields into a string.
///
/// Builds a string like `"TypeName(val1, val2)"` (Printable) or
/// `"TypeName { f1: val1, f2: val2 }"` (Debug), parameterized by the
/// strategy's format settings.
pub(super) fn compile_format_fields<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
    open: FormatOpen,
    separator: &str,
    suffix: &str,
    include_names: bool,
) {
    let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
    let self_val = setup.self_val.expect("FormatFields has self");
    let str_ty_id = setup.str_ty_id.expect("FormatFields needs str_ty_id");

    let prefix = match open {
        FormatOpen::TypeNameParen => format!("{type_name_str}("),
        FormatOpen::TypeNameBrace => format!("{type_name_str} {{ "),
    };
    let mut result = emit_str_literal(fc, &prefix, "prefix", str_ty_id);

    for (i, field) in fields.iter().enumerate() {
        let field_name_str = fc.lookup_name(field.name).to_owned();

        if include_names {
            let label = format!("{field_name_str}: ");
            let label_str = emit_str_literal(fc, &label, &format!("label.{i}"), str_ty_id);
            result = emit_str_concat(fc, result, label_str, &format!("cat.label.{i}"), str_ty_id);
        }

        let field_val =
            fc.builder_mut()
                .extract_value(self_val, i as u32, &format!("fmt.{field_name_str}"));
        if let Some(fv) = field_val {
            let field_str = emit_field_to_string(
                fc,
                trait_kind,
                fv,
                field.ty,
                &format!("fmt.{field_name_str}"),
                str_ty_id,
            );
            result = emit_str_concat(fc, result, field_str, &format!("cat.val.{i}"), str_ty_id);
        }

        if i + 1 < fields.len() {
            let sep = emit_str_literal(fc, separator, &format!("sep.{i}"), str_ty_id);
            result = emit_str_concat(fc, result, sep, &format!("cat.sep.{i}"), str_ty_id);
        }
    }

    let suffix_str = emit_str_literal(fc, suffix, "suffix", str_ty_id);
    result = emit_str_concat(fc, result, suffix_str, "cat.suffix", str_ty_id);

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(result));
}

// ---------------------------------------------------------------------------
// CloneFields: Clone
// ---------------------------------------------------------------------------

/// Generate `clone(self: Self) -> Self` — identity return for value types.
pub(super) fn compile_clone_fields<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    _fields: &[FieldDef],
) {
    let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
    let self_val = setup.self_val.expect("CloneFields has self");
    emit_derive_return(fc, setup.func_id, &setup.abi, Some(self_val));
}

// ---------------------------------------------------------------------------
// DefaultConstruct: Default
// ---------------------------------------------------------------------------

/// Generate `default() -> Self` — zero-initialized struct construction.
pub(super) fn compile_default_construct<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    _fields: &[FieldDef],
) {
    let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);

    // `const_zero` on a struct type recursively zeros all fields:
    // int → 0, float → 0.0, bool → false, ptr → null, str → {0, 0, null}
    let struct_llvm_ty = fc.resolve_type(type_idx);
    let result = fc.builder_mut().const_zero(struct_llvm_ty);

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(result));
}

// ---------------------------------------------------------------------------
// Enum MatchVariants: Eq, Comparable, Hashable, Clone
// ---------------------------------------------------------------------------

/// Generate derived methods for enum types using `SumBody::MatchVariants`.
///
/// Dispatches on the `struct_body` strategy:
/// - `ForEachField`: Eq, Comparable, Hashable — with payload comparison
/// - `CloneFields`: identity return (enums are value types in LLVM)
/// - Other strategies: not yet implemented (trace warning)
///
/// For unit-only enums, tag comparison is sufficient. For payload enums,
/// per-variant field comparison is emitted via switch on tag value.
pub(super) fn compile_enum_match_variants<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    variants: &[VariantDef],
    struct_body: &StructBody,
) {
    match *struct_body {
        StructBody::ForEachField { combine, field_op } => {
            let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
            match combine {
                CombineOp::AllTrue => emit_enum_all_true(fc, &setup, variants, field_op),
                CombineOp::Lexicographic => emit_enum_lexicographic(fc, &setup, variants, field_op),
                CombineOp::HashCombine => emit_enum_hash_combine(fc, &setup, variants, field_op),
            }
        }
        StructBody::CloneFields => {
            // Clone on enum = identity return (value type in LLVM)
            let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
            let self_val = setup.self_val.expect("Clone has self");
            emit_derive_return(fc, setup.func_id, &setup.abi, Some(self_val));
        }
        _ => {
            tracing::trace!(
                name = %type_name_str,
                derive = %trait_kind.method_name(),
                "enum derive strategy not yet implemented for this struct_body"
            );
        }
    }
}

/// Extract field type indices from a `VariantFields`.
fn variant_field_types(fields: &VariantFields) -> Vec<Idx> {
    match fields {
        VariantFields::Unit => vec![],
        VariantFields::Tuple(types) => types.clone(),
        VariantFields::Record(field_defs) => field_defs.iter().map(|f| f.ty).collect(),
    }
}

/// Enum Eq: compare tags first, then per-variant payload comparison.
///
/// For unit-only enums, just compares tags. For payload enums, switches on
/// the tag value and compares each variant's fields individually.
fn emit_enum_all_true<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("AllTrue has self");
    let other_val = setup.other_val.expect("AllTrue has other");
    let func_id = setup.func_id;

    let true_bb = fc.builder_mut().append_block(func_id, "eq.true");
    let false_bb = fc.builder_mut().append_block(func_id, "eq.false");

    // Extract tags
    let tag_self = fc.builder_mut().extract_value(self_val, 0, "eq.tag.self");
    let tag_other = fc.builder_mut().extract_value(other_val, 0, "eq.tag.other");

    let (Some(ts), Some(to)) = (tag_self, tag_other) else {
        warn!("extract_value failed for enum tag in derive Eq");
        fc.builder_mut().br(false_bb);
        emit_enum_true_false_returns(fc, true_bb, false_bb);
        return;
    };

    let tags_eq = fc.builder_mut().icmp_eq(ts, to, "eq.tags");
    let has_payload = variants.iter().any(|v| !v.fields.is_unit());

    if has_payload {
        // Tags must match first, then per-variant payload comparison
        let tags_match_bb = fc.builder_mut().append_block(func_id, "eq.tags.match");
        fc.builder_mut().cond_br(tags_eq, tags_match_bb, false_bb);
        fc.builder_mut().position_at_end(tags_match_bb);

        emit_enum_payload_eq(fc, setup, variants, field_op, ts, true_bb, false_bb);
    } else {
        // All unit: tags match is sufficient
        fc.builder_mut().cond_br(tags_eq, true_bb, false_bb);
    }

    emit_enum_true_false_returns(fc, true_bb, false_bb);
}

/// Emit per-variant payload comparison for Eq via switch on tag.
fn emit_enum_payload_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
    tag_self: super::super::value_id::ValueId,
    true_bb: super::super::value_id::BlockId,
    false_bb: super::super::value_id::BlockId,
) {
    let self_val = setup.self_val.expect("Eq has self");
    let other_val = setup.other_val.expect("Eq has other");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Eq needs str_ty_id");

    // Alloca self and other for GEP-based payload extraction
    let enum_llvm_ty = fc.resolve_type(setup.type_idx);
    let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
    let self_alloca = fc.entry_alloca(enum_ty_id, "eq.self");
    let other_alloca = fc.entry_alloca(enum_ty_id, "eq.other");
    fc.builder_mut().store(self_val, self_alloca);
    fc.builder_mut().store(other_val, other_alloca);

    // GEP to payload arrays (struct field 1 = [M x i64])
    let self_payload = fc
        .builder_mut()
        .struct_gep(enum_ty_id, self_alloca, 1, "eq.self.payload");
    let other_payload =
        fc.builder_mut()
            .struct_gep(enum_ty_id, other_alloca, 1, "eq.other.payload");

    // Build switch cases — one block per variant
    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("eq.v.{variant_name}"));
        let tag_val = fc.builder_mut().const_i64(tag_idx as i64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    fc.builder_mut().switch(tag_self, false_bb, &cases);

    // Emit per-variant comparison logic
    let i64_ty = fc.builder_mut().i64_type();

    for (tag_idx, variant) in variants.iter().enumerate() {
        fc.builder_mut().position_at_end(variant_bbs[tag_idx]);

        let field_types = variant_field_types(&variant.fields);
        if field_types.is_empty() {
            // Unit variant: tags already match → true
            fc.builder_mut().br(true_bb);
            continue;
        }

        // Compare payload fields using GEP into [M x i64] array
        let mut i64_offset: u64 = 0;
        for (fi, &field_type) in field_types.iter().enumerate() {
            let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
            let self_slot = fc.builder_mut().gep(
                i64_ty,
                self_payload,
                &[slot_idx],
                &format!("eq.v{tag_idx}.self.f{fi}"),
            );
            let other_slot = fc.builder_mut().gep(
                i64_ty,
                other_payload,
                &[slot_idx],
                &format!("eq.v{tag_idx}.other.f{fi}"),
            );

            let field_llvm_ty = fc.resolve_type(field_type);
            let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);

            let self_field = fc.builder_mut().load(
                field_ty_id,
                self_slot,
                &format!("eq.v{tag_idx}.self.f{fi}.val"),
            );
            let other_field = fc.builder_mut().load(
                field_ty_id,
                other_slot,
                &format!("eq.v{tag_idx}.other.f{fi}.val"),
            );

            let cmp = emit_field_operation(
                fc,
                field_op,
                self_field,
                Some(other_field),
                field_type,
                &format!("eq.v{tag_idx}.f{fi}"),
                str_ty_id,
            );

            if fi + 1 < field_types.len() {
                let next_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("eq.v{tag_idx}.f{}", fi + 1));
                fc.builder_mut().cond_br(cmp, next_bb, false_bb);
                fc.builder_mut().position_at_end(next_bb);
            } else {
                fc.builder_mut().cond_br(cmp, true_bb, false_bb);
            }

            // Advance offset by field size in i64 slots
            let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
            i64_offset += field_bytes.div_ceil(8).max(1);
        }
    }
}

/// Emit the true/false return blocks for enum Eq.
fn emit_enum_true_false_returns<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    true_bb: super::super::value_id::BlockId,
    false_bb: super::super::value_id::BlockId,
) {
    fc.builder_mut().position_at_end(true_bb);
    let true_val = fc.builder_mut().const_bool(true);
    fc.builder_mut().ret(true_val);

    fc.builder_mut().position_at_end(false_bb);
    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut().ret(false_val);
}

/// Enum Comparable: compare tags first, then per-variant lexicographic ordering.
///
/// Returns `Ordering` (Less=0, Equal=1, Greater=2) based on tag values,
/// then per-variant field ordering if tags match.
fn emit_enum_lexicographic<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("Lexicographic has self");
    let other_val = setup.other_val.expect("Lexicographic has other");
    let func_id = setup.func_id;

    let tag_self = fc.builder_mut().extract_value(self_val, 0, "cmp.tag.self");
    let tag_other = fc
        .builder_mut()
        .extract_value(other_val, 0, "cmp.tag.other");

    let (Some(ts), Some(to)) = (tag_self, tag_other) else {
        warn!("extract_value failed for enum tag in derive Comparable");
        let equal = fc.builder_mut().const_i8(1);
        emit_derive_return(fc, func_id, &setup.abi, Some(equal));
        return;
    };

    // Compare tags as unsigned (variant declaration order)
    let tag_ord = fc
        .builder_mut()
        .emit_icmp_ordering(ts, to, "cmp.tags", false);

    let has_payload = variants.iter().any(|v| !v.fields.is_unit());

    if has_payload {
        // If tags differ, return tag ordering. If same, compare payloads.
        let one = fc.builder_mut().const_i8(1);
        let tags_equal = fc.builder_mut().icmp_eq(tag_ord, one, "cmp.tags.is_eq");

        let equal_bb = fc.builder_mut().append_block(func_id, "cmp.equal");
        let ret_tag_bb = fc.builder_mut().append_block(func_id, "cmp.ret.tag");
        fc.builder_mut().cond_br(tags_equal, equal_bb, ret_tag_bb);

        // Return tag ordering when tags differ
        fc.builder_mut().position_at_end(ret_tag_bb);
        emit_derive_return(fc, func_id, &setup.abi, Some(tag_ord));

        // Tags match — compare payloads
        fc.builder_mut().position_at_end(equal_bb);
        emit_enum_payload_cmp(fc, setup, variants, field_op, ts);
    } else {
        // All unit: tag ordering is the full ordering
        emit_derive_return(fc, func_id, &setup.abi, Some(tag_ord));
    }
}

/// Emit per-variant lexicographic comparison via switch on tag.
fn emit_enum_payload_cmp<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
    tag_self: super::super::value_id::ValueId,
) {
    let self_val = setup.self_val.expect("Comparable has self");
    let other_val = setup.other_val.expect("Comparable has other");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Comparable needs str_ty_id");

    let equal_result_bb = fc.builder_mut().append_block(func_id, "cmp.result.equal");

    // Alloca for GEP
    let enum_llvm_ty = fc.resolve_type(setup.type_idx);
    let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
    let self_alloca = fc.entry_alloca(enum_ty_id, "cmp.self");
    let other_alloca = fc.entry_alloca(enum_ty_id, "cmp.other");
    fc.builder_mut().store(self_val, self_alloca);
    fc.builder_mut().store(other_val, other_alloca);

    let self_payload = fc
        .builder_mut()
        .struct_gep(enum_ty_id, self_alloca, 1, "cmp.self.payload");
    let other_payload =
        fc.builder_mut()
            .struct_gep(enum_ty_id, other_alloca, 1, "cmp.other.payload");

    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("cmp.v.{variant_name}"));
        let tag_val = fc.builder_mut().const_i64(tag_idx as i64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    fc.builder_mut().switch(tag_self, equal_result_bb, &cases);

    let i64_ty = fc.builder_mut().i64_type();

    for (tag_idx, variant) in variants.iter().enumerate() {
        fc.builder_mut().position_at_end(variant_bbs[tag_idx]);

        let field_types = variant_field_types(&variant.fields);
        if field_types.is_empty() {
            fc.builder_mut().br(equal_result_bb);
            continue;
        }

        let mut i64_offset: u64 = 0;
        for (fi, &field_type) in field_types.iter().enumerate() {
            let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
            let self_slot = fc.builder_mut().gep(
                i64_ty,
                self_payload,
                &[slot_idx],
                &format!("cmp.v{tag_idx}.self.f{fi}"),
            );
            let other_slot = fc.builder_mut().gep(
                i64_ty,
                other_payload,
                &[slot_idx],
                &format!("cmp.v{tag_idx}.other.f{fi}"),
            );

            let field_llvm_ty = fc.resolve_type(field_type);
            let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);

            let self_field = fc.builder_mut().load(
                field_ty_id,
                self_slot,
                &format!("cmp.v{tag_idx}.self.f{fi}.val"),
            );
            let other_field = fc.builder_mut().load(
                field_ty_id,
                other_slot,
                &format!("cmp.v{tag_idx}.other.f{fi}.val"),
            );

            let ord = emit_field_operation(
                fc,
                field_op,
                self_field,
                Some(other_field),
                field_type,
                &format!("cmp.v{tag_idx}.f{fi}"),
                str_ty_id,
            );

            let one = fc.builder_mut().const_i8(1);
            let is_equal =
                fc.builder_mut()
                    .icmp_eq(ord, one, &format!("cmp.v{tag_idx}.f{fi}.is_eq"));

            if fi + 1 < field_types.len() {
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.v{tag_idx}.ret.f{fi}"));
                let next_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.v{tag_idx}.f{}", fi + 1));
                fc.builder_mut().cond_br(is_equal, next_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));

                fc.builder_mut().position_at_end(next_bb);
            } else {
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.v{tag_idx}.ret.f{fi}"));
                fc.builder_mut().cond_br(is_equal, equal_result_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));
            }

            let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
            i64_offset += field_bytes.div_ceil(8).max(1);
        }
    }

    fc.builder_mut().position_at_end(equal_result_bb);
    let equal_val = fc.builder_mut().const_i8(1);
    emit_derive_return(fc, func_id, &setup.abi, Some(equal_val));
}

/// Enum Hashable: FNV-1a hash of tag + per-variant payload fields.
///
/// Hashes the tag first, then switches on tag to hash variant-specific
/// payload fields.
fn emit_enum_hash_combine<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
) {
    let self_val = setup.self_val.expect("HashCombine has self");
    let func_id = setup.func_id;

    // Hash the tag first
    let mut hash = fc.builder_mut().const_i64(FNV_OFFSET_BASIS as i64);
    let prime = fc.builder_mut().const_i64(FNV_PRIME as i64);

    let tag = fc.builder_mut().extract_value(self_val, 0, "hash.tag");
    if let Some(tag_val) = tag {
        // Tag is already i64 in enum layout { i64, [M x i64] } — use directly
        let xored = fc.builder_mut().xor(hash, tag_val, "hash.xor.tag");
        hash = fc.builder_mut().mul(xored, prime, "hash.mul.tag");
    }

    let has_payload = variants.iter().any(|v| !v.fields.is_unit());

    if has_payload {
        emit_enum_payload_hash(fc, setup, variants, field_op, hash);
    } else {
        // All unit: tag hash is sufficient
        emit_derive_return(fc, func_id, &setup.abi, Some(hash));
    }
}

/// Emit per-variant payload hashing via switch on tag.
fn emit_enum_payload_hash<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    setup: &DeriveSetup,
    variants: &[VariantDef],
    field_op: FieldOp,
    tag_hash: super::super::value_id::ValueId,
) {
    let self_val = setup.self_val.expect("Hashable has self");
    let func_id = setup.func_id;
    let str_ty_id = setup.str_ty_id.expect("Hashable needs str_ty_id");

    let merge_bb = fc.builder_mut().append_block(func_id, "hash.merge");

    // Alloca for GEP
    let enum_llvm_ty = fc.resolve_type(setup.type_idx);
    let enum_ty_id = fc.builder_mut().register_type(enum_llvm_ty);
    let self_alloca = fc.entry_alloca(enum_ty_id, "hash.self");
    fc.builder_mut().store(self_val, self_alloca);

    let self_payload = fc
        .builder_mut()
        .struct_gep(enum_ty_id, self_alloca, 1, "hash.self.payload");

    // Extract tag for switch
    let tag = fc
        .builder_mut()
        .extract_value(self_val, 0, "hash.switch.tag")
        .expect("tag extraction for hash switch");

    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_bbs = Vec::with_capacity(variants.len());
    for (tag_idx, variant) in variants.iter().enumerate() {
        let variant_name = fc.lookup_name(variant.name).to_owned();
        let bb = fc
            .builder_mut()
            .append_block(func_id, &format!("hash.v.{variant_name}"));
        let tag_val = fc.builder_mut().const_i64(tag_idx as i64);
        cases.push((tag_val, bb));
        variant_bbs.push(bb);
    }

    fc.builder_mut().switch(tag, merge_bb, &cases);

    let i64_ty = fc.builder_mut().i64_type();
    let prime = fc.builder_mut().const_i64(FNV_PRIME as i64);

    // Collect (variant_bb_end, hash_result) for phi node
    let mut phi_incoming: Vec<(
        super::super::value_id::ValueId,
        super::super::value_id::BlockId,
    )> = Vec::new();

    for (tag_idx, variant) in variants.iter().enumerate() {
        fc.builder_mut().position_at_end(variant_bbs[tag_idx]);

        let field_types = variant_field_types(&variant.fields);
        if field_types.is_empty() {
            // Unit: just use tag hash
            phi_incoming.push((tag_hash, variant_bbs[tag_idx]));
            fc.builder_mut().br(merge_bb);
            continue;
        }

        let mut hash = tag_hash;
        let mut i64_offset: u64 = 0;

        for (fi, &field_type) in field_types.iter().enumerate() {
            let slot_idx = fc.builder_mut().const_i64(i64_offset as i64);
            let self_slot = fc.builder_mut().gep(
                i64_ty,
                self_payload,
                &[slot_idx],
                &format!("hash.v{tag_idx}.f{fi}"),
            );

            let field_llvm_ty = fc.resolve_type(field_type);
            let field_ty_id = fc.builder_mut().register_type(field_llvm_ty);

            let field_val = fc.builder_mut().load(
                field_ty_id,
                self_slot,
                &format!("hash.v{tag_idx}.f{fi}.val"),
            );

            let field_as_i64 = emit_field_operation(
                fc,
                field_op,
                field_val,
                None,
                field_type,
                &format!("hash.v{tag_idx}.f{fi}"),
                str_ty_id,
            );

            let xored =
                fc.builder_mut()
                    .xor(hash, field_as_i64, &format!("hash.v{tag_idx}.xor.{fi}"));
            hash = fc
                .builder_mut()
                .mul(xored, prime, &format!("hash.v{tag_idx}.mul.{fi}"));

            let field_bytes = TypeLayoutResolver::type_store_size(field_llvm_ty);
            i64_offset += field_bytes.div_ceil(8).max(1);
        }

        // Record the current block (may differ from variant_bbs[tag_idx] due to
        // field operation blocks emitted by emit_field_operation)
        let current_bb = fc
            .builder_mut()
            .current_block()
            .expect("current block in hash");
        phi_incoming.push((hash, current_bb));
        fc.builder_mut().br(merge_bb);
    }

    // Merge: phi from all variant arms
    fc.builder_mut().position_at_end(merge_bb);
    let phi_result = fc.builder_mut().phi(i64_ty, "hash.result");
    fc.builder_mut().add_phi_incoming(phi_result, &phi_incoming);
    emit_derive_return(fc, func_id, &setup.abi, Some(phi_result));
}
