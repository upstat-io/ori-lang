//! Runtime function declarations for V2 codegen.
//!
//! Declares extern functions provided by the Ori runtime library (`ori_rt`).
//! These are resolved at link time (AOT) or via symbol mapping (JIT).
//!
//! # Lazy declaration (production)
//!
//! `declare_single()` declares one runtime function on demand, with the
//! correct signature and attributes from the `RT_FUNCTIONS` table.
//! Called via `IrBuilder::runtime_fn()`, which caches the result.
//!
//! A trivial `@main () -> int = 33` produces zero runtime declarations.
//!
//! # Eager declaration (tests)
//!
//! `declare_runtime()` eagerly declares all functions. Used by unit tests
//! that need the full set available in the LLVM module.

mod runtime_functions;

use runtime_functions::{Attr, RtFn, Ty, RT_FUNCTIONS};

use super::ir_builder::IrBuilder;
use super::value_id::{FunctionId, LLVMTypeId};

// ---------------------------------------------------------------------------
// Type and attribute resolution
// ---------------------------------------------------------------------------

/// Resolve a type descriptor to an LLVM type ID.
fn resolve_ty(builder: &mut IrBuilder, ty: Ty) -> LLVMTypeId {
    match ty {
        Ty::I64 => builder.i64_type(),
        Ty::I32 => builder.i32_type(),
        Ty::I8 => builder.i8_type(),
        Ty::F64 => builder.f64_type(),
        Ty::Bool => builder.bool_type(),
        Ty::Ptr => builder.ptr_type(),
        Ty::Str => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
        Ty::List => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
        Ty::Map => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i64().into(),
                    builder.scx().type_i64().into(),
                    builder.scx().type_ptr().into(),
                    builder.scx().type_ptr().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
        Ty::CharResult => {
            let st = builder.scx().type_struct(
                &[
                    builder.scx().type_i32().into(),
                    builder.scx().type_i64().into(),
                ],
                false,
            );
            builder.register_type(st.into())
        }
    }
}

/// Apply an attribute to a declared function.
fn apply_attr(builder: &mut IrBuilder, func: FunctionId, attr: Attr) {
    match attr {
        Attr::Nounwind => builder.add_nounwind_attribute(func),
        Attr::Cold => builder.add_cold_attribute(func),
        Attr::NoaliasReturn => builder.add_noalias_return_attribute(func),
        Attr::MemArgmemRW => builder.add_memory_argmem_readwrite_attribute(func),
    }
}

/// Declare a single runtime function from its specification.
fn declare_spec(builder: &mut IrBuilder, spec: &RtFn) -> FunctionId {
    let params: Vec<LLVMTypeId> = spec
        .params
        .iter()
        .map(|&t| resolve_ty(builder, t))
        .collect();
    let ret = spec.ret.map(|t| resolve_ty(builder, t));
    let id = builder.declare_extern_function(spec.name, &params, ret);
    for &attr in spec.attrs {
        apply_attr(builder, id, attr);
    }
    id
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Declare a single runtime function by name (lazy, on first use).
///
/// Looks up the function in `RT_FUNCTIONS`, declares it in the LLVM module
/// with the correct signature and attributes, and returns the `FunctionId`.
///
/// Called by `IrBuilder::runtime_fn()` which caches the result. If the
/// function already exists in the LLVM module, reuses the existing
/// declaration (via `declare_extern_function`'s idempotency).
///
/// # Panics
///
/// Panics if `name` is not a known runtime function.
pub fn declare_single(builder: &mut IrBuilder, name: &str) -> FunctionId {
    let spec = RT_FUNCTIONS
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("unknown runtime function: {name}"));
    declare_spec(builder, spec)
}

/// Like [`declare_single`], but returns `None` if the name is not a known
/// runtime function (instead of panicking).
///
/// Returns `(static_name, func_id)` where `static_name` is the `&'static str`
/// from the `RT_FUNCTIONS` table — suitable for use as a cache key in
/// `IrBuilder::try_runtime_fn()`.
pub fn try_declare_single(
    builder: &mut IrBuilder,
    name: &str,
) -> Option<(&'static str, FunctionId)> {
    let spec = RT_FUNCTIONS.iter().find(|f| f.name == name)?;
    let id = declare_spec(builder, spec);
    Some((spec.name, id))
}

/// Eagerly declare all runtime functions in the LLVM module.
///
/// Used by tests that need the full set available. Production code uses
/// `IrBuilder::runtime_fn()` for on-demand declaration.
pub fn declare_runtime(builder: &mut IrBuilder<'_, '_>) {
    for spec in RT_FUNCTIONS {
        declare_single(builder, spec.name);
    }
}

/// Returns the names of all known runtime functions.
///
/// Used by tests to verify sync with JIT/AOT-only function lists.
pub fn all_names() -> impl Iterator<Item = &'static str> {
    RT_FUNCTIONS.iter().map(|f| f.name)
}

/// Returns the number of runtime functions in the table.
#[cfg(test)]
pub fn count() -> usize {
    RT_FUNCTIONS.len()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
