//! Monomorphization collection pass.
//!
//! Transforms [`MonoInstance`] records (from the type checker) into
//! [`MonoFunction`] values ready for the LLVM pipeline. Each `MonoFunction`
//! carries a mangled name and a fully-concrete [`FunctionSig`] — existing
//! `declare_function()` / `define_function_body()` work unchanged.

use rustc_hash::{FxBuildHasher, FxHashMap};

use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, StringInterner, IMPL_METHOD_SEPARATOR, MONO_SEPARATOR};
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
    /// Source `MonoInstance` indices that dedup to this entry.
    ///
    /// Multiple `MonoInstance` records may collapse to one `MonoFunction` when
    /// they share the same mangled name (e.g., two call sites instantiating
    /// `identity` at `int`). Each id is the position of a `MonoInstance` in
    /// the slice passed to [`collect_mono_functions`]. Consumed by
    /// `declare_mono_functions` to populate `CodegenContext.mono_dispatch_by_id`
    /// for the abstract-index dispatch path.
    pub instance_ids: Vec<MonoInstanceId>,
}

// Collection

/// Collect monomorphized functions from type-checker `MonoInstance` records.
///
/// For each unique instance, looks up the generic function's signature,
/// builds a concrete (non-generic) signature with substituted types, and
/// computes a mangled name. Returns one `MonoFunction` per instance.
///
/// Instance shape determines which signature list is consulted:
/// - **Top-level instances** (`receiver_type = None`) look up
///   `function_sigs` by `fn_name`.
/// - **Method instances** (`receiver_type = Some(_)`) look up `impl_sigs`
///   by `fn_name`. Multiple impls may register the same method name; the
///   first match supplies metadata (param names, capabilities, defaults)
///   that does not depend on the receiver type.
///
/// Instances whose original function cannot be found in either list are
/// silently skipped (the generic function may be from an uncompiled module).
pub fn collect_mono_functions(
    mono_instances: &[MonoInstance],
    function_sigs: &[FunctionSig],
    impl_sigs: &[(Name, FunctionSig)],
    interner: &StringInterner,
    pool: &Pool,
) -> Vec<MonoFunction> {
    // Build name → sig lookups for O(1) access. The impl-side map keeps the
    // first registered signature per method name; receiver-type discrimination
    // is enforced upstream by `MonoInstance` dedup (`receiver_type` is part of
    // the dedup predicate), so any impl-side match supplies the same shared
    // metadata regardless of which receiver registered first.
    let fn_sig_by_name: FxHashMap<Name, &FunctionSig> =
        function_sigs.iter().map(|s| (s.name, s)).collect();
    let mut impl_sig_by_name: FxHashMap<Name, &FunctionSig> =
        FxHashMap::with_capacity_and_hasher(impl_sigs.len(), FxBuildHasher);
    for (name, sig) in impl_sigs {
        impl_sig_by_name.entry(*name).or_insert(sig);
    }

    let mut result: Vec<MonoFunction> = Vec::with_capacity(mono_instances.len());
    let mut name_to_index: FxHashMap<Name, usize> = FxHashMap::default();

    #[expect(
        clippy::cast_possible_truncation,
        reason = "MonoInstanceId is u32 by spec; mono_instances.len() bounded by source"
    )]
    for (idx, instance) in mono_instances.iter().enumerate() {
        let instance_id = MonoInstanceId(idx as u32);
        let lookup = if instance.receiver_type.is_some() {
            impl_sig_by_name.get(&instance.fn_name)
        } else {
            fn_sig_by_name.get(&instance.fn_name)
        };
        let Some(generic_sig) = lookup else {
            let name_str = interner.lookup(instance.fn_name);
            tracing::debug!(
                fn_name = ?instance.fn_name,
                name = name_str,
                is_method = instance.receiver_type.is_some(),
                "mono instance for unknown function — skipping"
            );
            continue;
        };

        let mangled_name = mangle_mono_name(
            instance.fn_name,
            &instance.generic_args,
            &instance.impl_args,
            &instance.method_args,
            interner,
            pool,
        );

        // Dedup specializations sharing a mangled name (same function + same
        // type args from multiple call sites). The first instance produces the
        // MonoFunction; later collisions append their id so the abstract-index
        // dispatch table can map every contributing instance to the survivor.
        if let Some(&existing) = name_to_index.get(&mangled_name) {
            result[existing].instance_ids.push(instance_id);
            continue;
        }

        // Build concrete signature: same structure, but with substituted types
        // and empty type_params (making is_generic() return false).
        // Compute Merkle hashes for concrete types
        let param_hashes: Vec<u64> = instance
            .concrete_param_types
            .iter()
            .map(|&idx| pool.hash(idx))
            .collect();
        let return_hash = pool.hash(instance.concrete_return_type);

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
            param_hashes,
            return_hash,
        };

        name_to_index.insert(mangled_name, result.len());
        result.push(MonoFunction {
            mangled_name,
            original_name: instance.fn_name,
            sig: concrete_sig,
            body_type_map: instance.body_type_map.iter().copied().collect(),
            instance_ids: vec![instance_id],
        });
    }

    result
}

// Name mangling

/// Compute the mangled name for a monomorphized function.
///
/// Each generic argument is encoded with a length prefix (`<bytes>_<payload>`)
/// so the encoding is injection-bijective even when user identifiers contain
/// `_` or other characters that would otherwise collide with an inter-arg
/// separator. The prefix follows Rust's `_ZN<N><name>E` convention — the
/// decoder reads the byte length, slices exactly that many bytes, and parses
/// the next argument unambiguously.
///
/// # Mangling shapes
///
/// The presence of [`IMPL_METHOD_SEPARATOR`] (`$im$`) distinguishes a method
/// instance from a top-level instance — top-level mangled names never contain
/// `$im$`. Four shapes are produced:
///
/// 1. **Top-level non-method** (no impl/method args, possibly with
///    generic args): `<fn_name>$m$<L0_PREFIXED_generic_args>`.
///    Example: `identity$m$3_int` for `identity<int>`,
///    `make_pair$m$3_int4_bool` for `make_pair<int, bool>`.
/// 2. **Method, impl-level only** (impl args, no method args):
///    `<fn_name>$m$<L0_PREFIXED_impl_args>$im$` (trailing `$im$` distinguishes
///    from top-level). Example: `hello$m$3_int$im$` for
///    `impl<T> Box<T> { @hello (...) }` instantiated at `T = int`.
/// 3. **Method with method-level generics**:
///    `<fn_name>$m$<L0_PREFIXED_impl_args>$im$<L0_PREFIXED_method_args>`.
///    Example: `bar$m$3_int$im$3_str` for
///    `impl<T> Foo<T> { @bar<U> (...) }` at `T=int`, `U=str`.
/// 4. **Method, no impl-level generics** (method-level only,
///    e.g., `impl Box<int> { @m<U> ... }`):
///    `<fn_name>$m$$im$<L0_PREFIXED_method_args>` (empty impl-args section).
///
/// # Panics
///
/// Panics if `fn_name` contains either reserved separator (`$m$` or `$im$`).
/// Ori identifier syntax excludes `$`, so this is a defensive guard against
/// an upstream bug rather than a path real input can take.
pub fn mangle_mono_name(
    fn_name: Name,
    generic_args: &[GenericArg],
    impl_args: &[GenericArg],
    method_args: &[GenericArg],
    interner: &StringInterner,
    pool: &Pool,
) -> Name {
    let base = interner.lookup(fn_name);
    assert!(
        !base.contains(MONO_SEPARATOR),
        "mangle_mono_name: fn_name {base:?} contains reserved separator {MONO_SEPARATOR:?}",
    );
    assert!(
        !base.contains(IMPL_METHOD_SEPARATOR),
        "mangle_mono_name: fn_name {base:?} contains reserved separator {IMPL_METHOD_SEPARATOR:?}",
    );

    let is_method = !impl_args.is_empty() || !method_args.is_empty();

    let mut mangled = String::with_capacity(
        base.len()
            + MONO_SEPARATOR.len()
            + IMPL_METHOD_SEPARATOR.len()
            + (generic_args.len() + impl_args.len() + method_args.len()) * 8,
    );
    mangled.push_str(base);
    mangled.push_str(MONO_SEPARATOR);

    if is_method {
        encode_args_length_prefixed(impl_args, pool, interner, &mut mangled);
        mangled.push_str(IMPL_METHOD_SEPARATOR);
        encode_args_length_prefixed(method_args, pool, interner, &mut mangled);
    } else {
        encode_args_length_prefixed(generic_args, pool, interner, &mut mangled);
    }

    interner.intern(&mangled)
}

/// Encode each generic argument with a `<byte_len>_<payload>` length prefix.
///
/// The byte length measures the encoded payload (after `encode_type` /
/// `encode_const_value` runs), not the source spelling. With the prefix in
/// place no inter-arg separator is needed — successive prefixed payloads
/// concatenate unambiguously (the decoder reads the length, slices, repeats).
fn encode_args_length_prefixed(
    args: &[GenericArg],
    pool: &Pool,
    interner: &StringInterner,
    out: &mut String,
) {
    let mut payload = String::new();
    for arg in args {
        payload.clear();
        match arg {
            GenericArg::Type(idx) => encode_type(*idx, pool, interner, &mut payload),
            GenericArg::Const(cv) => encode_const_value(cv, &mut payload),
        }
        out.push_str(&payload.len().to_string());
        out.push('_');
        out.push_str(&payload);
    }
}

/// Encode a type as a compact string for name mangling.
#[expect(
    clippy::too_many_lines,
    reason = "type encoding dispatch over all Tag variants for name mangling"
)]
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
