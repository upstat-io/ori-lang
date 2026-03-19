//! Struct body implementations for derived method codegen.
//!
//! Four functions map to the four `StructBody` variants:
//! - `compile_for_each_field` — Eq, Comparable, Hashable
//! - `compile_format_fields` — Printable, Debug
//! - `compile_clone_fields` — Clone
//! - `compile_default_construct` — Default
//!
//! Enum bodies are in the sibling [`super::enum_bodies`] module.
//!
//! The common scaffolding (signature, ABI, function declaration) is handled by
//! `setup_derive_function`; these functions only emit the body logic.

use ori_ir::{CombineOp, DerivedTrait, FieldOp, FormatOpen, Name};
use ori_types::{FieldDef, Idx, Tag};
use tracing::warn;

use super::super::function_compiler::FunctionCompiler;
use super::super::{FunctionId, ValueId};

use super::field_ops::emit_field_operation;
use super::string_helpers::{emit_field_to_string, emit_str_concat, emit_str_literal};
use super::{emit_derive_return, setup_derive_function, DeriveSetup};

/// FNV-1a offset basis (64-bit).
const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
/// FNV-1a prime (64-bit).
const FNV_PRIME: u64 = 1_099_511_628_211;

// ForEachField: Eq, Comparable, Hashable

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

    if fields.is_empty() {
        // No fields → always equal
        let true_val = fc.builder_mut().const_bool(true);
        emit_derive_return(fc, func_id, &setup.abi, Some(true_val));
        return;
    }

    let true_bb = fc.builder_mut().append_block(func_id, "eq.true");
    let false_bb = fc.builder_mut().append_block(func_id, "eq.false");

    for (i, field) in fields.iter().enumerate() {
        let field_name = fc.lookup_name(field.name).to_owned();
        let self_field =
            fc.builder_mut()
                .extract_value(self_val, i as u32, &format!("eq.self.{field_name}"));
        let other_field =
            fc.builder_mut()
                .extract_value(other_val, i as u32, &format!("eq.other.{field_name}"));

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
                .append_block(func_id, &format!("eq.field.{}", i + 1));
            fc.builder_mut().cond_br(cmp, next_bb, false_bb);
            fc.builder_mut().position_at_end(next_bb);
        } else {
            fc.builder_mut().cond_br(cmp, true_bb, false_bb);
        }
    }

    fc.builder_mut().position_at_end(true_bb);
    let true_val = fc.builder_mut().const_bool(true);
    fc.builder_mut().ret(true_val);

    fc.builder_mut().position_at_end(false_bb);
    let false_val = fc.builder_mut().const_bool(false);
    fc.builder_mut().ret(false_val);
}

/// Lexicographic ordering: compare each field, return first non-Equal result.
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

    if fields.is_empty() {
        // No fields → always Equal (1)
        let equal = fc.builder_mut().const_i8(1);
        emit_derive_return(fc, func_id, &setup.abi, Some(equal));
        return;
    }

    for (i, field) in fields.iter().enumerate() {
        let field_name = fc.lookup_name(field.name).to_owned();
        let self_field =
            fc.builder_mut()
                .extract_value(self_val, i as u32, &format!("cmp.self.{field_name}"));
        let other_field =
            fc.builder_mut()
                .extract_value(other_val, i as u32, &format!("cmp.other.{field_name}"));

        let (Some(sf), Some(of)) = (self_field, other_field) else {
            warn!(field = %field_name, "extract_value failed in derive Lexicographic");
            let equal = fc.builder_mut().const_i8(1);
            emit_derive_return(fc, func_id, &setup.abi, Some(equal));
            return;
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

        if i + 1 < fields.len() {
            let one = fc.builder_mut().const_i8(1);
            let is_equal = fc
                .builder_mut()
                .icmp_eq(ord, one, &format!("cmp.{field_name}.is_eq"));

            let ret_bb = fc
                .builder_mut()
                .append_block(func_id, &format!("cmp.ret.{field_name}"));
            let next_bb = fc
                .builder_mut()
                .append_block(func_id, &format!("cmp.field.{}", i + 1));
            fc.builder_mut().cond_br(is_equal, next_bb, ret_bb);

            fc.builder_mut().position_at_end(ret_bb);
            emit_derive_return(fc, func_id, &setup.abi, Some(ord));

            fc.builder_mut().position_at_end(next_bb);
        } else {
            emit_derive_return(fc, func_id, &setup.abi, Some(ord));
        }
    }
}

/// FNV-1a accumulation: hash each field into a running hash value.
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
            .xor(hash, field_as_i64, &format!("hash.xor.{field_name}"));
        hash = fc
            .builder_mut()
            .mul(xored, prime, &format!("hash.mul.{field_name}"));
    }

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(hash));
}

// FormatFields: Printable, Debug

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

// CloneFields: Clone

/// Generate `clone(self: Self) -> Self`.
///
/// Clone returns the same struct value (shallow copy). For fields that are
/// heap-allocated (str, list, map, set, closures, nested fat structs), we
/// must RC-increment the field's data pointer so the original and clone
/// have independent RC lifecycles.
pub(super) fn compile_clone_fields<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
) {
    let setup = setup_derive_function(fc, trait_kind, type_name, type_idx, type_name_str);
    let self_val = setup.self_val.expect("CloneFields has self");

    // RC-increment each heap-allocated field so the clone has independent
    // ownership from the original.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "field count bounded by struct definition"
    )]
    for (i, field) in fields.iter().enumerate() {
        let pool = fc.type_info().pool();
        let resolved = pool.resolve_fully(field.ty);
        let tag = pool.tag(resolved);

        let field_val = fc
            .builder_mut()
            .extract_value(self_val, i as u32, &format!("clone.f.{i}"));
        let Some(fv) = field_val else {
            continue;
        };

        emit_clone_field_rc_inc(fc, setup.func_id, fv, tag, resolved, i);
    }

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(self_val));
}

/// Emit RC increment for a single field value based on its type tag.
///
/// `func_id` is the LLVM function being compiled (needed for creating blocks).
fn emit_clone_field_rc_inc<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    tag: Tag,
    resolved: Idx,
    field_idx: usize,
) {
    match tag {
        Tag::Str => emit_clone_rc_inc_str(fc, func_id, field_val, field_idx),
        Tag::List | Tag::Set => emit_clone_rc_inc_list(fc, field_val, field_idx),
        Tag::Map => emit_clone_rc_inc_data_ptr(fc, field_val, field_idx, "map"),
        Tag::Function => emit_clone_rc_inc_closure(fc, func_id, field_val, field_idx),
        Tag::Struct | Tag::Tuple | Tag::Option => {
            emit_clone_composite_rc_inc(fc, func_id, field_val, tag, resolved, field_idx);
        }
        _ => {} // Scalars: no RC needed
    }
}

/// Emit SSO-aware `ori_rc_inc` on a str field's data pointer.
fn emit_clone_rc_inc_str<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    field_idx: usize,
) {
    let Some(data_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 2, &format!("clone.str.data.{field_idx}"))
    else {
        return;
    };

    let do_inc = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.str.heap.{field_idx}"));
    let skip = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.str.skip.{field_idx}"));

    let i64_ty = fc.builder_mut().i64_type();
    let p2i = fc
        .builder_mut()
        .ptr_to_int(data_ptr, i64_ty, &format!("clone.str.p2i.{field_idx}"));
    let sso_mask = fc.builder_mut().const_i64(i64::MIN);
    let sso_flag = fc
        .builder_mut()
        .and(p2i, sso_mask, &format!("clone.str.sso.{field_idx}"));
    let zero = fc.builder_mut().const_i64(0);
    let is_sso = fc
        .builder_mut()
        .icmp_ne(sso_flag, zero, &format!("clone.str.is_sso.{field_idx}"));
    let is_null = fc
        .builder_mut()
        .icmp_eq(p2i, zero, &format!("clone.str.null.{field_idx}"));
    let skip_cond =
        fc.builder_mut()
            .or(is_sso, is_null, &format!("clone.str.skip_cond.{field_idx}"));
    fc.builder_mut().cond_br(skip_cond, skip, do_inc);

    fc.builder_mut().position_at_end(do_inc);
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[data_ptr],
        &format!("clone.str.inc.{field_idx}"),
    );
    fc.builder_mut().br(skip);

    fc.builder_mut().position_at_end(skip);
}

/// Emit `ori_list_rc_inc(data, cap)` for a list/set field.
fn emit_clone_rc_inc_list<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    field_val: ValueId,
    field_idx: usize,
) {
    let Some(data_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 2, &format!("clone.list.data.{field_idx}"))
    else {
        return;
    };
    let cap = fc
        .builder_mut()
        .extract_value(field_val, 1, &format!("clone.list.cap.{field_idx}"))
        .unwrap_or_else(|| fc.builder_mut().const_i64(0));
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_list_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[data_ptr, cap],
        &format!("clone.list.inc.{field_idx}"),
    );
}

/// Emit `ori_rc_inc` on a data pointer at field 2 (map or similar).
fn emit_clone_rc_inc_data_ptr<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    field_val: ValueId,
    field_idx: usize,
    label: &str,
) {
    let Some(data_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 2, &format!("clone.{label}.data.{field_idx}"))
    else {
        return;
    };
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[data_ptr],
        &format!("clone.{label}.inc.{field_idx}"),
    );
}

/// Emit null-checked `ori_rc_inc` on a closure env pointer (field 1).
fn emit_clone_rc_inc_closure<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    field_idx: usize,
) {
    let Some(env_ptr) =
        fc.builder_mut()
            .extract_value(field_val, 1, &format!("clone.closure.env.{field_idx}"))
    else {
        return;
    };

    if fc.builder_mut().is_const_null_ptr(env_ptr) {
        return;
    }

    let do_inc = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.env.inc.{field_idx}"));
    let skip = fc
        .builder_mut()
        .append_block(func_id, &format!("clone.env.skip.{field_idx}"));

    let i64_ty = fc.builder_mut().i64_type();
    let p2i = fc
        .builder_mut()
        .ptr_to_int(env_ptr, i64_ty, &format!("clone.env.p2i.{field_idx}"));
    let zero = fc.builder_mut().const_i64(0);
    let is_null = fc
        .builder_mut()
        .icmp_eq(p2i, zero, &format!("clone.env.null.{field_idx}"));
    fc.builder_mut().cond_br(is_null, skip, do_inc);

    fc.builder_mut().position_at_end(do_inc);
    let rc_inc_fn = fc.builder_mut().runtime_fn("ori_rc_inc");
    fc.builder_mut().call(
        rc_inc_fn,
        &[env_ptr],
        &format!("clone.env.call.{field_idx}"),
    );
    fc.builder_mut().br(skip);

    fc.builder_mut().position_at_end(skip);
}

/// Recurse into composite types (struct/tuple/option) for clone RC increments.
fn emit_clone_composite_rc_inc<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    field_val: ValueId,
    tag: Tag,
    resolved: Idx,
    field_idx: usize,
) {
    let pool = fc.type_info().pool();
    let sub_fields: Vec<(u32, Idx)> = match tag {
        Tag::Struct => {
            let fields = pool.struct_fields(resolved);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "field count bounded by struct definition"
            )]
            fields
                .into_iter()
                .enumerate()
                .map(|(i, (_, ty))| (i as u32, ty))
                .collect()
        }
        Tag::Tuple => {
            let elems = pool.tuple_elems(resolved);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "element count bounded by tuple arity"
            )]
            elems
                .into_iter()
                .enumerate()
                .map(|(i, ty)| (i as u32, ty))
                .collect()
        }
        Tag::Option => {
            let inner = pool.option_inner(resolved);
            vec![(1, inner)]
        }
        _ => return,
    };

    for (idx, inner_ty) in sub_fields {
        let pool = fc.type_info().pool();
        let inner_resolved = pool.resolve_fully(inner_ty);
        let inner_tag = pool.tag(inner_resolved);
        if !matches!(
            inner_tag,
            Tag::Str
                | Tag::List
                | Tag::Set
                | Tag::Map
                | Tag::Function
                | Tag::Struct
                | Tag::Tuple
                | Tag::Option
        ) {
            continue;
        }
        let inner_fv =
            fc.builder_mut()
                .extract_value(field_val, idx, &format!("clone.sub.{field_idx}.{idx}"));
        if let Some(ifv) = inner_fv {
            emit_clone_field_rc_inc(
                fc,
                func_id,
                ifv,
                inner_tag,
                inner_resolved,
                field_idx * 100 + idx as usize,
            );
        }
    }
}

// DefaultConstruct: Default

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
