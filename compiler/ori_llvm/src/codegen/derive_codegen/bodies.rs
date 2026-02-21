//! Strategy-driven body implementations for derived method codegen.
//!
//! Four functions map to the four `StructBody` variants:
//! - `compile_for_each_field` — Eq, Comparable, Hashable
//! - `compile_format_fields` — Printable, Debug
//! - `compile_clone_fields` — Clone
//! - `compile_default_construct` — Default
//!
//! The common scaffolding (signature, ABI, function declaration) is handled by
//! `setup_derive_function`; these functions only emit the body logic.

use ori_ir::{CombineOp, DerivedTrait, FieldOp, FormatOpen, Name};
use ori_types::{FieldDef, Idx};
use tracing::warn;

use super::super::function_compiler::FunctionCompiler;

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
    // int → 0, float → 0.0, bool → false, ptr → null, str → {0, null}
    let struct_llvm_ty = fc.resolve_type(type_idx);
    let result = fc.builder_mut().const_zero(struct_llvm_ty);

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(result));
}
