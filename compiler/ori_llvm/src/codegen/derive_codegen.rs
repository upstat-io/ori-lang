//! LLVM IR generation for derived trait methods.
//!
//! Generates synthetic LLVM functions for `#[derive(...)]` attributes on structs.
//! Each derived method becomes a real LLVM function registered in `method_functions`,
//! so the existing `lower_method_call` dispatch finds them with no special path.
//!
//! Supported traits:
//! - **Eq**: Field-by-field structural equality (`eq(self, other) -> bool`)
//! - **Clone**: Identity return for value types (`clone(self) -> Self`)
//! - **Hashable**: FNV-1a hash in pure LLVM IR (`hash(self) -> int`)
//! - **Printable**: String representation via runtime concat (`to_str(self) -> str`)

use ori_ir::{DerivedTrait, Module, Name, TypeDeclKind};
use ori_types::{FieldDef, Idx, TypeEntry, TypeKind};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use super::abi::{compute_function_abi, FunctionAbi, ReturnPassing};
use super::function_compiler::FunctionCompiler;
use super::type_info::TypeInfo;
use super::value_id::{FunctionId, LLVMTypeId, ValueId};

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Compile derived trait methods for all types in the module.
///
/// Iterates `module.types`, finds types with `#[derive(...)]`, resolves their
/// fields from `user_types`, and generates synthetic LLVM functions for each
/// derived method. Results are registered in `method_functions` so that
/// `lower_method_call` finds them through normal dispatch.
pub fn compile_derives<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    module: &Module,
    user_types: &[TypeEntry],
) {
    let type_map: FxHashMap<Name, &TypeEntry> = user_types.iter().map(|te| (te.name, te)).collect();

    for type_decl in &module.types {
        if type_decl.derives.is_empty() {
            continue;
        }

        let TypeDeclKind::Struct(_) = &type_decl.kind else {
            trace!(
                name = %fc.lookup_name(type_decl.name),
                "skipping derives for non-struct type"
            );
            continue;
        };

        let Some(type_entry) = type_map.get(&type_decl.name) else {
            warn!(
                name = %fc.lookup_name(type_decl.name),
                "no TypeEntry for type with derives — skipping"
            );
            continue;
        };

        let TypeKind::Struct(struct_def) = &type_entry.kind else {
            warn!(
                name = %fc.lookup_name(type_decl.name),
                "TypeEntry is not a struct — skipping derives"
            );
            continue;
        };

        let type_name = type_decl.name;
        let type_idx = type_entry.idx;
        let fields = &struct_def.fields;
        let type_name_str = fc.lookup_name(type_name).to_owned();

        debug!(
            name = %type_name_str,
            derives = type_decl.derives.len(),
            fields = fields.len(),
            "compiling derived methods"
        );

        for derive_name in &type_decl.derives {
            let trait_name_str = fc.lookup_name(*derive_name);
            let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) else {
                warn!(derive = %trait_name_str, "unknown derive trait — skipping");
                continue;
            };

            match trait_kind {
                DerivedTrait::Eq => {
                    compile_derive_eq(fc, type_name, type_idx, &type_name_str, fields);
                }
                DerivedTrait::Clone => {
                    compile_derive_clone(fc, type_name, type_idx, &type_name_str, fields);
                }
                DerivedTrait::Hashable => {
                    compile_derive_hash(fc, type_name, type_idx, &type_name_str, fields);
                }
                DerivedTrait::Printable => {
                    compile_derive_printable(fc, type_name, type_idx, &type_name_str, fields);
                }
                DerivedTrait::Default => {
                    compile_derive_default(fc, type_name, type_idx, &type_name_str);
                }
                DerivedTrait::Debug => {
                    // TODO: LLVM codegen for Debug (deferred — interpreter-only for now)
                    debug!(derive = "Debug", type_name = %type_name_str, "Debug derive not yet implemented in LLVM codegen — skipping");
                }
                DerivedTrait::Comparable => {
                    // TODO: LLVM codegen for Comparable (deferred — interpreter-only for now)
                    debug!(derive = "Comparable", type_name = %type_name_str, "Comparable derive not yet implemented in LLVM codegen — skipping");
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Eq: field-by-field structural equality
// ---------------------------------------------------------------------------

/// Generate `eq(self: Self, other: Self) -> bool`.
///
/// Short-circuit AND: compare each field, branch to `false_bb` on first
/// mismatch, fall through to `ret true`.
fn compile_derive_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
) {
    let method_name_str = "eq";
    let method_name = fc.intern(method_name_str);
    let other_name = fc.intern("other");

    let sig = make_sig(
        method_name,
        vec![fc.intern("self"), other_name],
        vec![type_idx, type_idx],
        Idx::BOOL,
    );

    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, self_val, param_vals) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    let other_val = param_vals[0];

    let true_bb = fc.builder_mut().append_block(func_id, "eq.true");
    let false_bb = fc.builder_mut().append_block(func_id, "eq.false");

    // Resolve str type once for string field comparisons
    let str_ty = fc.resolve_type(Idx::STR);
    let str_ty_id = fc.builder_mut().register_type(str_ty);

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
                warn!(field = %field_name, "extract_value failed in derive Eq");
                fc.builder_mut().br(false_bb);
                break;
            };

            let cmp = emit_field_eq(fc, sf, of, field.ty, &format!("eq.{field_name}"), str_ty_id);

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

/// Emit an equality comparison for a single field based on its type.
fn emit_field_eq<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int
        | TypeInfo::Char
        | TypeInfo::Byte
        | TypeInfo::Bool
        | TypeInfo::Duration
        | TypeInfo::Size
        | TypeInfo::Ordering => fc.builder_mut().icmp_eq(lhs, rhs, name),

        TypeInfo::Float => fc.builder_mut().fcmp_oeq(lhs, rhs, name),

        TypeInfo::Str => emit_str_eq_call(fc, lhs, rhs, name, str_ty_id),

        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let eq_name = fc.intern("eq");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, eq_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[lhs, rhs], name);
                }
            }
            fc.builder_mut().icmp_eq(lhs, rhs, name)
        }

        _ => {
            trace!(
                ?info,
                "unsupported field type for derive Eq — using icmp eq"
            );
            fc.builder_mut().icmp_eq(lhs, rhs, name)
        }
    }
}

/// Call `ori_str_eq(a: ptr, b: ptr) -> bool` via alloca+store pattern.
fn emit_str_eq_call<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();
    let bool_ty = fc.builder_mut().bool_type();

    let lhs_alloca = fc.builder_mut().alloca(str_ty_id, "lhs_str");
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.builder_mut().alloca(str_ty_id, "rhs_str");
    fc.builder_mut().store(rhs, rhs_alloca);

    let eq_fn = fc
        .builder_mut()
        .get_or_declare_function("ori_str_eq", &[ptr_ty, ptr_ty], bool_ty);
    fc.builder_mut()
        .call(eq_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| fc.builder_mut().const_bool(false))
}

// ---------------------------------------------------------------------------
// Clone: identity return for value types
// ---------------------------------------------------------------------------

/// Generate `clone(self: Self) -> Self`.
///
/// For value-type structs, clone is identity — just return self.
/// ABI handles sret for large structs automatically.
fn compile_derive_clone<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    _fields: &[FieldDef],
) {
    let method_name_str = "clone";
    let method_name = fc.intern(method_name_str);

    let sig = make_sig(
        method_name,
        vec![fc.intern("self")],
        vec![type_idx],
        type_idx,
    );

    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, self_val, _) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    emit_derive_return(fc, func_id, &abi, Some(self_val));
}

// ---------------------------------------------------------------------------
// Hashable: FNV-1a hash in pure LLVM IR
// ---------------------------------------------------------------------------

/// FNV-1a offset basis (64-bit).
const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
/// FNV-1a prime (64-bit).
const FNV_PRIME: u64 = 1_099_511_628_211;

/// Generate `hash(self: Self) -> int`.
///
/// FNV-1a in pure LLVM IR: `hash = (hash XOR field_as_i64) * FNV_PRIME` per field.
fn compile_derive_hash<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
) {
    let method_name_str = "hash";
    let method_name = fc.intern(method_name_str);

    let sig = make_sig(
        method_name,
        vec![fc.intern("self")],
        vec![type_idx],
        Idx::INT,
    );

    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, self_val, _) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    let mut hash = fc.builder_mut().const_i64(FNV_OFFSET_BASIS as i64);
    let prime = fc.builder_mut().const_i64(FNV_PRIME as i64);

    for (i, field) in fields.iter().enumerate() {
        let field_name = fc.lookup_name(field.name).to_owned();
        let field_val =
            fc.builder_mut()
                .extract_value(self_val, i as u32, &format!("hash.{field_name}"));

        let Some(fv) = field_val else {
            warn!(field = %field_name, "extract_value failed in derive Hash");
            continue;
        };

        let field_as_i64 = coerce_to_i64(fc, fv, field.ty, &format!("hash.{field_name}"));

        let xored = fc
            .builder_mut()
            .xor(hash, field_as_i64, &format!("hash.xor.{i}"));
        hash = fc.builder_mut().mul(xored, prime, &format!("hash.mul.{i}"));
    }

    emit_derive_return(fc, func_id, &abi, Some(hash));
}

/// Coerce a field value to i64 for hashing.
fn coerce_to_i64<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    field_type: Idx,
    name: &str,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => val,

        TypeInfo::Char | TypeInfo::Byte | TypeInfo::Ordering => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().sext(val, i64_ty, name)
        }

        TypeInfo::Bool => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().zext(val, i64_ty, name)
        }

        TypeInfo::Float => {
            let i64_ty = fc.builder_mut().i64_type();
            fc.builder_mut().bitcast(val, i64_ty, name)
        }

        TypeInfo::Str => fc
            .builder_mut()
            .extract_value(val, 0, &format!("{name}.len"))
            .unwrap_or_else(|| fc.builder_mut().const_i64(0)),

        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let hash_name = fc.intern("hash");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, hash_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[val], name);
                }
            }
            fc.builder_mut().const_i64(0)
        }

        _ => {
            trace!(?info, "unsupported field type for derive Hash — using 0");
            fc.builder_mut().const_i64(0)
        }
    }
}

// ---------------------------------------------------------------------------
// Printable: string representation via runtime concat
// ---------------------------------------------------------------------------

/// Generate `to_str(self: Self) -> str`.
///
/// Builds `"TypeName(val1, val2)"` per spec §7 — human-readable format
/// with type name and field values (no field names).
/// Uses runtime string functions: `ori_str_from_int`, `ori_str_from_bool`,
/// `ori_str_from_float`, `ori_str_concat`.
fn compile_derive_printable<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
) {
    let method_name_str = "to_str";
    let method_name = fc.intern(method_name_str);

    let sig = make_sig(
        method_name,
        vec![fc.intern("self")],
        vec![type_idx],
        Idx::STR,
    );

    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, self_val, _) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    // Resolve str type once for all string operations
    let str_ty = fc.resolve_type(Idx::STR);
    let str_ty_id = fc.builder_mut().register_type(str_ty);

    // Build opening: "TypeName("
    let prefix = format!("{type_name_str}(");
    let mut result = emit_str_literal(fc, &prefix, "prefix", str_ty_id);

    for (i, field) in fields.iter().enumerate() {
        let field_name_str = fc.lookup_name(field.name).to_owned();

        let field_val =
            fc.builder_mut()
                .extract_value(self_val, i as u32, &format!("ts.{field_name_str}"));
        if let Some(fv) = field_val {
            let field_str =
                emit_field_to_string(fc, fv, field.ty, &format!("ts.{field_name_str}"), str_ty_id);
            result = emit_str_concat(fc, result, field_str, &format!("cat.val.{i}"), str_ty_id);
        }

        if i + 1 < fields.len() {
            let sep = emit_str_literal(fc, ", ", &format!("sep.{i}"), str_ty_id);
            result = emit_str_concat(fc, result, sep, &format!("cat.sep.{i}"), str_ty_id);
        }
    }

    let suffix = emit_str_literal(fc, ")", "suffix", str_ty_id);
    result = emit_str_concat(fc, result, suffix, "cat.suffix", str_ty_id);

    emit_derive_return(fc, func_id, &abi, Some(result));
}

/// Emit a string literal as an Ori str value `{i64 len, ptr data}`.
fn emit_str_literal<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    s: &str,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let data_ptr = fc
        .builder_mut()
        .build_global_string_ptr(s, &format!("{name}.data"));
    let len = fc.builder_mut().const_i64(s.len() as i64);
    fc.builder_mut()
        .build_struct(str_ty_id, &[len, data_ptr], name)
}

/// Call `ori_str_concat(a: ptr, b: ptr) -> str` (alloca+store pattern).
fn emit_str_concat<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    lhs: ValueId,
    rhs: ValueId,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let ptr_ty = fc.builder_mut().ptr_type();

    let lhs_alloca = fc.builder_mut().alloca(str_ty_id, &format!("{name}.lhs"));
    fc.builder_mut().store(lhs, lhs_alloca);
    let rhs_alloca = fc.builder_mut().alloca(str_ty_id, &format!("{name}.rhs"));
    fc.builder_mut().store(rhs, rhs_alloca);

    let concat_fn =
        fc.builder_mut()
            .get_or_declare_function("ori_str_concat", &[ptr_ty, ptr_ty], str_ty_id);
    fc.builder_mut()
        .call(concat_fn, &[lhs_alloca, rhs_alloca], name)
        .unwrap_or_else(|| emit_str_literal(fc, "", "empty", str_ty_id))
}

/// Convert a field value to its string representation.
fn emit_field_to_string<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    val: ValueId,
    field_type: Idx,
    name: &str,
    str_ty_id: LLVMTypeId,
) -> ValueId {
    let info = fc.type_info().get(field_type);
    match &info {
        TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
            let i64_ty = fc.builder_mut().i64_type();
            let f =
                fc.builder_mut()
                    .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty_id);
            fc.builder_mut()
                .call(f, &[val], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<int>", name, str_ty_id))
        }
        TypeInfo::Float => {
            let f64_ty = fc.builder_mut().f64_type();
            let f = fc.builder_mut().get_or_declare_function(
                "ori_str_from_float",
                &[f64_ty],
                str_ty_id,
            );
            fc.builder_mut()
                .call(f, &[val], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<float>", name, str_ty_id))
        }
        TypeInfo::Bool => {
            let bool_ty = fc.builder_mut().bool_type();
            let f = fc.builder_mut().get_or_declare_function(
                "ori_str_from_bool",
                &[bool_ty],
                str_ty_id,
            );
            fc.builder_mut()
                .call(f, &[val], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<bool>", name, str_ty_id))
        }
        TypeInfo::Str => val,
        TypeInfo::Char => {
            let i64_ty = fc.builder_mut().i64_type();
            let char_as_i64 = fc.builder_mut().sext(val, i64_ty, &format!("{name}.sext"));
            let f =
                fc.builder_mut()
                    .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty_id);
            fc.builder_mut()
                .call(f, &[char_as_i64], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<char>", name, str_ty_id))
        }
        TypeInfo::Byte | TypeInfo::Ordering => {
            let i64_ty = fc.builder_mut().i64_type();
            let as_i64 = fc.builder_mut().sext(val, i64_ty, &format!("{name}.sext"));
            let f =
                fc.builder_mut()
                    .get_or_declare_function("ori_str_from_int", &[i64_ty], str_ty_id);
            fc.builder_mut()
                .call(f, &[as_i64], name)
                .unwrap_or_else(|| emit_str_literal(fc, "<byte>", name, str_ty_id))
        }
        TypeInfo::Struct { .. } => {
            let nested_name = fc.type_idx_to_name(field_type);
            let ts_name = fc.intern("to_str");
            if let Some(type_name) = nested_name {
                if let Some((fid, abi)) = fc.get_method_function(type_name, ts_name) {
                    return emit_method_call_for_derive(fc, fid, &abi, &[val], name);
                }
            }
            emit_str_literal(fc, "<struct>", name, str_ty_id)
        }
        _ => emit_str_literal(fc, "<?>", name, str_ty_id),
    }
}

// ---------------------------------------------------------------------------
// Default: zero-initialized struct construction
// ---------------------------------------------------------------------------

/// Generate `default() -> Self` (static method, no self parameter).
///
/// Constructs a zero-initialized struct by building each field's default
/// value and assembling them via `build_struct`. Uses `const_zero` for the
/// struct's LLVM type, which recursively zero-inits all fields — producing
/// correct defaults for int(0), float(0.0), bool(false), and str({0, null}).
fn compile_derive_default<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
) {
    let method_name_str = "default";
    let method_name = fc.intern(method_name_str);

    // No parameters — default() is a static method
    let sig = make_sig(method_name, vec![], vec![], type_idx);

    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, _, _) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    // Build a zero-initialized struct value.
    // `const_zero` on a struct type recursively zeros all fields:
    // int → 0, float → 0.0, bool → false, ptr → null, str → {0, null}
    let struct_llvm_ty = fc.resolve_type(type_idx);
    let result = fc.builder_mut().const_zero(struct_llvm_ty);

    emit_derive_return(fc, func_id, &abi, Some(result));
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build a `FunctionSig` for a derived method (no generics, no capabilities).
fn make_sig(
    name: Name,
    param_names: Vec<Name>,
    param_types: Vec<Idx>,
    return_type: Idx,
) -> ori_types::FunctionSig {
    ori_types::FunctionSig::synthetic(name, param_names, param_types, return_type)
}

/// Emit return instruction respecting ABI (direct, sret, or void).
fn emit_derive_return<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    result: Option<ValueId>,
) {
    match &abi.return_abi.passing {
        ReturnPassing::Sret { .. } => {
            if let Some(val) = result {
                let sret_ptr = fc.builder_mut().get_param(func_id, 0);
                fc.builder_mut().store(val, sret_ptr);
            }
            fc.builder_mut().ret_void();
        }
        ReturnPassing::Direct => {
            if let Some(val) = result {
                fc.builder_mut().ret(val);
            } else {
                fc.builder_mut().ret_void();
            }
        }
        ReturnPassing::Void => {
            fc.builder_mut().ret_void();
        }
    }
}

/// Emit a method call for a derived method (handles sret return).
fn emit_method_call_for_derive<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    func_id: FunctionId,
    abi: &FunctionAbi,
    args: &[ValueId],
    name: &str,
) -> ValueId {
    match &abi.return_abi.passing {
        ReturnPassing::Sret { .. } => {
            let ret_ty = fc.resolve_type(abi.return_abi.ty);
            let ret_ty_id = fc.builder_mut().register_type(ret_ty);
            fc.builder_mut()
                .call_with_sret(func_id, args, ret_ty_id, name)
                .unwrap_or_else(|| fc.builder_mut().const_i64(0))
        }
        _ => fc
            .builder_mut()
            .call(func_id, args, name)
            .unwrap_or_else(|| fc.builder_mut().const_i64(0)),
    }
}
