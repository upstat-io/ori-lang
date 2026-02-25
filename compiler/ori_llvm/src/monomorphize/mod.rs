//! Monomorphization collection pass.
//!
//! Transforms [`MonoInstance`] records (from the type checker) into
//! [`MonoFunction`] values ready for the LLVM pipeline. Each `MonoFunction`
//! carries a mangled name and a fully-concrete [`FunctionSig`] — existing
//! `declare_function()` / `define_function_body()` work unchanged.

use rustc_hash::FxHashMap;

use ori_ir::{Name, StringInterner};
use ori_types::{ConstValue, FunctionSig, GenericArg, Idx, MonoInstance, Pool, Tag};

// MonoFunction

/// A monomorphized function ready for LLVM codegen.
///
/// Produced by [`collect_mono_functions`] from type-checker `MonoInstance` records.
/// The `mangled_name` is unique per (function, type args) combination.
#[derive(Clone, Debug)]
pub struct MonoFunction {
    /// Mangled name for the specialization (e.g., `identity$m$int`).
    pub mangled_name: Name,
    /// Original generic function name (for canonical IR body lookup).
    pub original_name: Name,
    /// Concrete function signature (non-generic: empty `type_params`).
    pub sig: FunctionSig,
    /// Generic `Idx` → concrete `Idx` map for ARC lowering.
    pub body_type_map: FxHashMap<Idx, Idx>,
}

// Collection

/// Collect monomorphized functions from type-checker `MonoInstance` records.
///
/// For each unique instance, looks up the generic function's signature,
/// builds a concrete (non-generic) signature with substituted types, and
/// computes a mangled name. Returns one `MonoFunction` per instance.
///
/// Instances whose original function cannot be found in `function_sigs` are
/// silently skipped (the generic function may be from an uncompiled module).
pub fn collect_mono_functions(
    mono_instances: &[MonoInstance],
    function_sigs: &[FunctionSig],
    interner: &StringInterner,
    pool: &Pool,
) -> Vec<MonoFunction> {
    // Build name → sig lookup for O(1) access.
    let sig_by_name: FxHashMap<Name, &FunctionSig> =
        function_sigs.iter().map(|s| (s.name, s)).collect();

    let mut result = Vec::with_capacity(mono_instances.len());

    for instance in mono_instances {
        let Some(generic_sig) = sig_by_name.get(&instance.fn_name) else {
            tracing::debug!(
                fn_name = ?instance.fn_name,
                "mono instance for unknown function — skipping"
            );
            continue;
        };

        let mangled_name =
            mangle_mono_name(instance.fn_name, &instance.generic_args, interner, pool);

        // Build concrete signature: same structure, but with substituted types
        // and empty type_params (making is_generic() return false).
        let concrete_sig = FunctionSig {
            name: mangled_name,
            type_params: vec![],
            const_params: vec![],
            param_names: generic_sig.param_names.clone(),
            param_types: instance.concrete_param_types.clone(),
            return_type: instance.concrete_return_type,
            capabilities: generic_sig.capabilities.clone(),
            is_public: false, // mono specializations are internal
            is_test: false,
            is_main: false,
            is_fbip: generic_sig.is_fbip,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            scheme_var_ids: vec![],
            required_params: generic_sig.required_params,
            param_defaults: generic_sig.param_defaults.clone(),
        };

        result.push(MonoFunction {
            mangled_name,
            original_name: instance.fn_name,
            sig: concrete_sig,
            body_type_map: instance.body_type_map.clone(),
        });
    }

    result
}

// Name mangling

/// Compute the mangled name for a monomorphized function.
///
/// Format: `{fn_name}$m${type1}_{type2}_{typeN}`
///
/// Example: `identity$m$int`, `make_pair$m$int_bool`, `filter$m$Lint`
fn mangle_mono_name(
    fn_name: Name,
    generic_args: &[GenericArg],
    interner: &StringInterner,
    pool: &Pool,
) -> Name {
    let base = interner.lookup(fn_name);
    let mut mangled = String::with_capacity(base.len() + 4 + generic_args.len() * 6);
    mangled.push_str(base);
    mangled.push_str("$m$");

    for (i, arg) in generic_args.iter().enumerate() {
        if i > 0 {
            mangled.push('_');
        }
        match arg {
            GenericArg::Type(idx) => encode_type(*idx, pool, interner, &mut mangled),
            GenericArg::Const(cv) => encode_const_value(cv, &mut mangled),
        }
    }

    interner.intern(&mangled)
}

/// Encode a type as a compact string for name mangling.
///
/// See `plans/monomorphization/00-overview.md` § Name Mangling Scheme.
fn encode_type(ty: Idx, pool: &Pool, interner: &StringInterner, out: &mut String) {
    let resolved = pool.resolve_fully(ty);
    let tag = pool.tag(resolved);

    match tag {
        Tag::Int => out.push_str("int"),
        Tag::Float => out.push_str("float"),
        Tag::Bool => out.push_str("bool"),
        Tag::Str => out.push_str("str"),
        Tag::Char => out.push_str("char"),
        Tag::Byte => out.push_str("byte"),
        Tag::Unit => out.push_str("void"),
        Tag::Never => out.push_str("never"),
        Tag::Duration => out.push_str("dur"),
        Tag::Size => out.push_str("size"),
        Tag::Ordering => out.push_str("ord"),

        Tag::List => {
            out.push('L');
            let elem = Idx::from_raw(pool.data(resolved));
            encode_type(elem, pool, interner, out);
        }
        Tag::Option => {
            out.push('O');
            let inner = Idx::from_raw(pool.data(resolved));
            encode_type(inner, pool, interner, out);
        }
        Tag::Set => {
            out.push_str("Se");
            let elem = Idx::from_raw(pool.data(resolved));
            encode_type(elem, pool, interner, out);
        }
        Tag::Range => {
            out.push_str("Rn");
            let elem = Idx::from_raw(pool.data(resolved));
            encode_type(elem, pool, interner, out);
        }
        Tag::Iterator => {
            out.push_str("It");
            let elem = Idx::from_raw(pool.data(resolved));
            encode_type(elem, pool, interner, out);
        }

        Tag::Map => {
            out.push('M');
            let key = pool.map_key(resolved);
            let value = pool.map_value(resolved);
            encode_type(key, pool, interner, out);
            out.push('_');
            encode_type(value, pool, interner, out);
        }
        Tag::Result => {
            out.push('R');
            let ok = pool.result_ok(resolved);
            let err = pool.result_err(resolved);
            encode_type(ok, pool, interner, out);
            out.push('_');
            encode_type(err, pool, interner, out);
        }

        Tag::Tuple => {
            out.push('T');
            let elems = pool.tuple_elems(resolved);
            for (i, &elem) in elems.iter().enumerate() {
                if i > 0 {
                    out.push('_');
                }
                encode_type(elem, pool, interner, out);
            }
        }

        Tag::Function => {
            out.push('F');
            let params = pool.function_params(resolved);
            for (i, &param) in params.iter().enumerate() {
                if i > 0 {
                    out.push('_');
                }
                encode_type(param, pool, interner, out);
            }
            out.push_str("_R");
            let ret = pool.function_return(resolved);
            encode_type(ret, pool, interner, out);
        }

        Tag::Struct => {
            out.push('S');
            let name = pool.struct_name(resolved);
            out.push_str(interner.lookup(name));
        }
        Tag::Enum => {
            out.push('E');
            let name = pool.enum_name(resolved);
            out.push_str(interner.lookup(name));
        }

        Tag::Applied => {
            // Named generic type: encode the name + args
            let name = pool.applied_name(resolved);
            out.push('A');
            out.push_str(interner.lookup(name));
            let args = pool.applied_args(resolved);
            for arg in args {
                out.push('_');
                encode_type(arg, pool, interner, out);
            }
        }

        // Fallback for types not yet handled (Named, Alias, etc.)
        _ => {
            out.push('U');
            out.push_str(&resolved.raw().to_string());
        }
    }
}

/// Encode a const value for name mangling (Phase 2+).
fn encode_const_value(cv: &ConstValue, out: &mut String) {
    match cv {
        ConstValue::Int(n) => {
            if *n < 0 {
                out.push_str("cn");
                out.push_str(&n.unsigned_abs().to_string());
            } else {
                out.push('c');
                out.push_str(&n.to_string());
            }
        }
        ConstValue::Bool(b) => {
            if *b {
                out.push_str("ctrue");
            } else {
                out.push_str("cfalse");
            }
        }
    }
}

#[cfg(test)]
mod tests;
