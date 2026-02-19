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
//! - **Default**: Zero-initialized struct construction (`default() -> Self`)
//! - **Comparable**: Lexicographic field comparison (`compare(self, other) -> Ordering`)
//!
//! Deferred to interpreter-only (not yet codegen'd):
//! - **Debug**: Debug string representation

mod field_ops;
mod string_helpers;

use ori_ir::{DerivedMethodShape, DerivedTrait, Module, Name, TypeDeclKind};
use ori_types::{FieldDef, Idx, TypeEntry, TypeKind};
use rustc_hash::FxHashMap;
use tracing::{debug, trace, warn};

use super::abi::{compute_function_abi, FunctionAbi, ReturnPassing};
use super::function_compiler::FunctionCompiler;
use super::value_id::{FunctionId, ValueId};

use field_ops::{coerce_to_i64, emit_field_compare, emit_field_eq};
use string_helpers::{emit_field_to_string, emit_str_concat, emit_str_literal};

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
                    compile_derive_default(fc, type_name, type_idx, &type_name_str, fields);
                }
                DerivedTrait::Debug => {
                    // TODO: LLVM codegen for Debug (deferred — interpreter-only for now)
                    debug!(derive = "Debug", type_name = %type_name_str, "Debug derive not yet implemented in LLVM codegen — skipping");
                }
                DerivedTrait::Comparable => {
                    compile_derive_comparable(fc, type_name, type_idx, &type_name_str, fields);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory: common derive scaffolding
// ---------------------------------------------------------------------------

/// Context returned by [`setup_derive_function`] for derive body emitters.
struct DeriveSetup {
    func_id: FunctionId,
    abi: FunctionAbi,
    /// Value for `self` parameter. `None` for nullary methods (Default).
    self_val: Option<ValueId>,
    /// Value for `other` parameter. `None` for unary/nullary methods.
    other_val: Option<ValueId>,
}

/// Common scaffolding for all derived trait codegen functions.
///
/// Handles: method name interning, signature construction (driven by
/// [`DerivedMethodShape`]), ABI computation, symbol mangling, and function
/// declaration. Returns a [`DeriveSetup`] with the function handle and
/// parameter values for the body to use.
fn setup_derive_function<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    trait_kind: DerivedTrait,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
) -> DeriveSetup {
    let method_name_str = trait_kind.method_name();
    let method_name = fc.intern(method_name_str);
    let shape = trait_kind.shape();

    let (param_names, param_types) = build_derive_params(fc, shape, type_idx);
    let return_type = derive_return_type(shape, type_idx);

    let sig = make_sig(method_name, param_names, param_types, return_type);
    let abi = compute_function_abi(&sig, fc.type_info());
    let symbol = fc.mangle_method(type_name_str, method_name_str);

    let (func_id, self_val, param_vals) =
        fc.declare_and_bind_derive(&symbol, &abi, type_name, method_name, type_idx);

    let self_opt = if shape.has_self() {
        Some(self_val)
    } else {
        None
    };
    let other_opt = if shape.has_other() {
        Some(param_vals[0])
    } else {
        None
    };

    DeriveSetup {
        func_id,
        abi,
        self_val: self_opt,
        other_val: other_opt,
    }
}

/// Build parameter names and types for a derived method from its shape.
fn build_derive_params<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    shape: DerivedMethodShape,
    type_idx: Idx,
) -> (Vec<Name>, Vec<Idx>) {
    match shape {
        DerivedMethodShape::BinaryPredicate | DerivedMethodShape::BinaryToOrdering => {
            let self_name = fc.intern("self");
            let other_name = fc.intern("other");
            (vec![self_name, other_name], vec![type_idx, type_idx])
        }
        DerivedMethodShape::UnaryIdentity
        | DerivedMethodShape::UnaryToInt
        | DerivedMethodShape::UnaryToStr => {
            let self_name = fc.intern("self");
            (vec![self_name], vec![type_idx])
        }
        DerivedMethodShape::Nullary => (vec![], vec![]),
    }
}

/// Determine the return type for a derived method from its shape.
fn derive_return_type(shape: DerivedMethodShape, type_idx: Idx) -> Idx {
    match shape {
        DerivedMethodShape::BinaryPredicate => Idx::BOOL,
        DerivedMethodShape::UnaryIdentity | DerivedMethodShape::Nullary => type_idx,
        DerivedMethodShape::UnaryToInt => Idx::INT,
        DerivedMethodShape::UnaryToStr => Idx::STR,
        DerivedMethodShape::BinaryToOrdering => Idx::ORDERING,
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
    let setup = setup_derive_function(fc, DerivedTrait::Eq, type_name, type_idx, type_name_str);
    let self_val = setup.self_val.expect("Eq has self");
    let other_val = setup.other_val.expect("Eq has other");
    let func_id = setup.func_id;

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

// ---------------------------------------------------------------------------
// Comparable: lexicographic field comparison
// ---------------------------------------------------------------------------

/// Generate `compare(self: Self, other: Self) -> Ordering`.
///
/// Lexicographic: compare fields in declaration order, short-circuit on first
/// non-Equal result. Returns Ordering (i8): 0=Less, 1=Equal, 2=Greater.
fn compile_derive_comparable<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
) {
    let setup = setup_derive_function(
        fc,
        DerivedTrait::Comparable,
        type_name,
        type_idx,
        type_name_str,
    );
    let self_val = setup.self_val.expect("Comparable has self");
    let other_val = setup.other_val.expect("Comparable has other");
    let func_id = setup.func_id;

    // Resolve str type once for string field comparisons
    let str_ty = fc.resolve_type(Idx::STR);
    let str_ty_id = fc.builder_mut().register_type(str_ty);

    let equal_bb = fc.builder_mut().append_block(func_id, "cmp.equal");

    if fields.is_empty() {
        // Empty struct: always Equal
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
                warn!(field = %field_name, "extract_value failed in derive Comparable");
                fc.builder_mut().br(equal_bb);
                break;
            };

            let ord = emit_field_compare(
                fc,
                sf,
                of,
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
                // More fields: if Equal, continue to next; otherwise return this Ordering
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.ret.{field_name}"));
                let next_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.next.{}", i + 1));
                fc.builder_mut().cond_br(is_equal, next_bb, ret_bb);

                // Return the non-Equal ordering
                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));

                fc.builder_mut().position_at_end(next_bb);
            } else {
                // Last field: if Equal, fall through to equal_bb; otherwise return
                let ret_bb = fc
                    .builder_mut()
                    .append_block(func_id, &format!("cmp.ret.{field_name}"));
                fc.builder_mut().cond_br(is_equal, equal_bb, ret_bb);

                fc.builder_mut().position_at_end(ret_bb);
                emit_derive_return(fc, func_id, &setup.abi, Some(ord));
            }
        }
    }

    // All fields Equal
    fc.builder_mut().position_at_end(equal_bb);
    let equal_val = fc.builder_mut().const_i8(1);
    emit_derive_return(fc, func_id, &setup.abi, Some(equal_val));
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
    let setup = setup_derive_function(fc, DerivedTrait::Clone, type_name, type_idx, type_name_str);
    let self_val = setup.self_val.expect("Clone has self");
    emit_derive_return(fc, setup.func_id, &setup.abi, Some(self_val));
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
    let setup = setup_derive_function(
        fc,
        DerivedTrait::Hashable,
        type_name,
        type_idx,
        type_name_str,
    );
    let self_val = setup.self_val.expect("Hashable has self");

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

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(hash));
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
    let setup = setup_derive_function(
        fc,
        DerivedTrait::Printable,
        type_name,
        type_idx,
        type_name_str,
    );
    let self_val = setup.self_val.expect("Printable has self");

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

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(result));
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
    _fields: &[FieldDef],
) {
    let setup = setup_derive_function(
        fc,
        DerivedTrait::Default,
        type_name,
        type_idx,
        type_name_str,
    );

    // Build a zero-initialized struct value.
    // `const_zero` on a struct type recursively zeros all fields:
    // int → 0, float → 0.0, bool → false, ptr → null, str → {0, null}
    let struct_llvm_ty = fc.resolve_type(type_idx);
    let result = fc.builder_mut().const_zero(struct_llvm_ty);

    emit_derive_return(fc, setup.func_id, &setup.abi, Some(result));
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
