//! Backend-neutral name mangling for monomorphized functions.
//!
//! Encodes a monomorphized function's (name, type args, receiver) into a unique
//! mangled symbol. Each generic argument is length-prefixed (`<bytes>_<payload>`)
//! so the encoding is self-delimiting and collision-free across user identifiers.

use ori_ir::{Name, StringInterner, IMPL_METHOD_SEPARATOR, MONO_SEPARATOR};
use ori_types::{ConstValue, GenericArg, Idx, Pool, Tag};

/// Compute the mangled name for a monomorphized function.
///
/// Each generic argument is encoded with a length prefix (`<bytes>_<payload>`)
/// so the encoding is injection-bijective even when user identifiers contain
/// `_` or other characters that would otherwise collide with an inter-arg
/// separator. The length prefix makes the encoding self-delimiting — the
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
///    e.g., `impl Boxer { @pick<U> ... }`):
///    `<fn_name>$m$<L0_PREFIXED_receiver>$im$<L0_PREFIXED_method_args>` — the
///    receiver head is emitted after `$m$` (per the receiver-prepend rule below);
///    the impl-args section between it and `$im$` is empty. Example:
///    `pick$m$5_Boxer$im$3_int` for `impl Boxer { @pick<T> }` at `T = int`.
///
/// For a method instance carrying a `receiver_type`, the encoded receiver type
/// is emitted as the first length-prefixed payload after `$m$`, ahead of the
/// impl args. The receiver head qualifies the symbol so a method name shared by
/// two distinct generic types (`impl<T> Box<T> { @get }` and
/// `impl<T> Wrap<T> { @get }`) instantiated at the same arg does not collide on
/// one mangled name (which would mis-dedup the two specializations). Top-level
/// instances (`receiver_type = None`) are unchanged.
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
    receiver_type: Option<Idx>,
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

    // receiver_type.is_some() is the authoritative method discriminator;
    // non-generic-impl methods carry empty impl/method args but still a receiver.
    let is_method = receiver_type.is_some() || !impl_args.is_empty() || !method_args.is_empty();

    let mut mangled = String::with_capacity(
        base.len()
            + MONO_SEPARATOR.len()
            + IMPL_METHOD_SEPARATOR.len()
            + (generic_args.len() + impl_args.len() + method_args.len()) * 8,
    );
    mangled.push_str(base);
    mangled.push_str(MONO_SEPARATOR);

    if is_method {
        if let Some(recv) = receiver_type {
            let mut payload = String::new();
            encode_type(recv, pool, interner, &mut payload);
            mangled.push_str(&payload.len().to_string());
            mangled.push('_');
            mangled.push_str(&payload);
        }
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
