//! Canonical representation mapping.
//!
//! Maps every `Tag` variant to its canonical `MachineRepr` — the
//! representation before any optimization. This is the starting point
//! for the `ReprPlan`: every type gets its canonical repr first,
//! then §02–§11 narrow it.

use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashSet;

use crate::enum_repr::{EnumRepr, EnumTag, VariantRepr};
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};

/// Compute the canonical machine representation for a type.
///
/// Handles recursive types (e.g., `type Tree = Leaf(int) | Node(Tree, Tree)`)
/// via cycle detection — recursive positions are represented as
/// [`MachineRepr::RcPointer`] since they are always heap-allocated in Ori.
///
/// # Panics
///
/// Panics if an unresolved type variable (`Var`, `BoundVar`, `RigidVar`,
/// `Scheme`, `Projection`, `ModuleNs`, `Infer`, `SelfType`) reaches
/// this function — these indicate a type checker bug.
pub fn canonical(pool: &Pool, idx: Idx) -> MachineRepr {
    canonical_inner(pool, idx, &mut FxHashSet::default())
}

/// Inner canonicalization with cycle detection via `visiting` set.
///
/// When a type is encountered that is already being canonicalized
/// (i.e., present in `visiting`), we return an `RcPointer` — recursive
/// positions in Ori are always behind ARC pointers at runtime.
fn canonical_inner(pool: &Pool, idx: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    let resolved = pool.resolve_fully(idx);

    // Cycle detection: if already canonicalizing this type, it's a recursive
    // reference — return an RC pointer (recursive fields are heap-allocated).
    if !visiting.insert(resolved) {
        return MachineRepr::RcPointer(RcRepr {
            rc_width: IntWidth::I64,
            atomic: true,
            inner: Box::new(MachineRepr::OpaquePtr),
            stack_promotable: false,
        });
    }

    let tag = pool.tag(resolved);

    let result = match tag {
        // Primitives
        Tag::Int => MachineRepr::Int {
            width: IntWidth::I64,
            signed: true,
        },
        Tag::Float => MachineRepr::Float {
            width: FloatWidth::F64,
        },
        Tag::Bool => MachineRepr::Bool,
        Tag::Char => MachineRepr::Char,
        Tag::Byte => MachineRepr::Byte,
        Tag::Duration => MachineRepr::Duration,
        Tag::Size => MachineRepr::Size,
        Tag::Ordering => MachineRepr::Ordering,
        Tag::Unit => MachineRepr::Unit,
        Tag::Never => MachineRepr::Never,
        Tag::Str => MachineRepr::FatPointer(FatRepr::Str),
        Tag::Range => MachineRepr::Range,
        Tag::Iterator | Tag::DoubleEndedIterator | Tag::Channel => MachineRepr::OpaquePtr,

        // Collections — fat pointer {len, cap, data}
        Tag::List => canonical_collection(pool, pool.list_elem(resolved), visiting),
        Tag::Set => canonical_collection(pool, pool.set_elem(resolved), visiting),
        Tag::Map => canonical_map(pool, resolved, visiting),

        // Composite types
        Tag::Option => {
            canonical_option(canonical_inner(pool, pool.option_inner(resolved), visiting))
        }
        Tag::Result => {
            let ok = canonical_inner(pool, pool.result_ok(resolved), visiting);
            let err = canonical_inner(pool, pool.result_err(resolved), visiting);
            canonical_result(ok, err)
        }
        Tag::Function => canonical_function(pool, resolved, visiting),
        Tag::Tuple => canonical_tuple(pool, resolved, visiting),
        Tag::Struct => canonical_struct(pool, resolved, visiting),
        Tag::Enum => canonical_enum(pool, resolved, visiting),

        // Types that must not reach canonical — compiler bugs
        Tag::Named | Tag::Applied | Tag::Alias => panic!(
            "canonical: Named/Applied/Alias should be resolved by resolve_fully, \
             got {tag:?} at idx {resolved:?}"
        ),
        Tag::Borrowed | Tag::Error => {
            panic!("canonical: {tag:?} at idx {resolved:?} should not reach codegen")
        }
        Tag::Var | Tag::BoundVar | Tag::RigidVar => panic!(
            "canonical: unresolved type variable {tag:?} at idx {resolved:?} — \
             all variables must be resolved before codegen"
        ),
        Tag::Scheme | Tag::Projection | Tag::ModuleNs | Tag::Infer | Tag::SelfType => {
            panic!("canonical: special type {tag:?} at idx {resolved:?} should never reach codegen")
        }
    };

    visiting.remove(&resolved);
    result
}

/// Canonicalize a collection element into a fat pointer.
fn canonical_collection(pool: &Pool, elem_idx: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    MachineRepr::FatPointer(FatRepr::Collection {
        element_repr: Box::new(canonical_inner(pool, elem_idx, visiting)),
    })
}

/// Canonicalize a map into a fat pointer with key and value reprs.
fn canonical_map(pool: &Pool, resolved: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    MachineRepr::FatPointer(FatRepr::Map {
        key_repr: Box::new(canonical_inner(pool, pool.map_key(resolved), visiting)),
        value_repr: Box::new(canonical_inner(pool, pool.map_value(resolved), visiting)),
    })
}

/// Canonicalize a function type into a closure representation.
fn canonical_function(pool: &Pool, resolved: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    let params: Vec<MachineRepr> = pool
        .function_params(resolved)
        .into_iter()
        .map(|p| canonical_inner(pool, p, visiting))
        .collect();
    let ret = canonical_inner(pool, pool.function_return(resolved), visiting);
    MachineRepr::Closure(ClosureRepr {
        params,
        ret: Box::new(ret),
    })
}

/// Canonicalize a tuple into an anonymous struct with positional fields.
fn canonical_tuple(pool: &Pool, resolved: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    let fields: Vec<FieldRepr> = pool
        .tuple_elems(resolved)
        .into_iter()
        .enumerate()
        .map(|(i, elem_idx)| {
            let repr = canonical_inner(pool, elem_idx, visiting);
            let idx_u32 = u32::try_from(i).unwrap_or(u32::MAX);
            FieldRepr {
                name: Name::new(0, idx_u32),
                original_index: idx_u32,
                offset: 0, // Set by §06 layout
                repr,
            }
        })
        .collect();
    let trivial = fields.iter().all(|f| is_trivial_repr(&f.repr));
    TupleRepr::to_machine_repr(fields, trivial)
}

/// Canonicalize a struct type with named fields.
fn canonical_struct(pool: &Pool, resolved: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    let fields: Vec<FieldRepr> = pool
        .struct_fields(resolved)
        .into_iter()
        .enumerate()
        .map(|(i, (name, field_idx))| {
            let repr = canonical_inner(pool, field_idx, visiting);
            let idx_u32 = u32::try_from(i).unwrap_or(u32::MAX);
            FieldRepr {
                name,
                original_index: idx_u32,
                offset: 0, // Set by §06 layout
                repr,
            }
        })
        .collect();
    let trivial = fields.iter().all(|f| is_trivial_repr(&f.repr));
    let (size, align) = compute_field_layout(&fields);
    MachineRepr::Struct(StructRepr {
        fields,
        size,
        align,
        trivial,
    })
}

/// Canonicalize an enum type with explicit i64 tag.
fn canonical_enum(pool: &Pool, resolved: Idx, visiting: &mut FxHashSet<Idx>) -> MachineRepr {
    let variants: Vec<VariantRepr> = pool
        .enum_variants(resolved)
        .into_iter()
        .map(|(name, field_idxs)| {
            let fields: Vec<MachineRepr> = field_idxs
                .into_iter()
                .map(|fi| canonical_inner(pool, fi, visiting))
                .collect();
            let (size, alignment) = compute_payload_layout(&fields);
            VariantRepr {
                name,
                fields,
                size,
                alignment,
            }
        })
        .collect();

    let max_payload = variants.iter().map(|v| v.size).max().unwrap_or(0);
    let max_align = variants
        .iter()
        .map(|v| v.alignment)
        .max()
        .unwrap_or(1)
        .max(8); // tag is i64 → 8-byte aligned
    let size = 8 + round_up(max_payload, max_align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants,
        size,
        align: max_align,
    })
}

/// Build canonical `Option<T>` as a 2-variant enum: None (unit) + Some(T).
fn canonical_option(inner_repr: MachineRepr) -> MachineRepr {
    let none_variant = VariantRepr {
        name: Name::new(0, 0), // "None" — exact interning handled at call sites
        fields: vec![],
        size: 0,
        alignment: 1,
    };
    let some_size = field_size(&inner_repr);
    let some_align = field_align(&inner_repr);
    let some_variant = VariantRepr {
        name: Name::new(0, 1), // "Some"
        fields: vec![inner_repr],
        size: some_size,
        alignment: some_align,
    };

    let max_payload = some_size;
    let align = some_align.max(8); // i64 tag
    let size = 8 + round_up(max_payload, align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![none_variant, some_variant],
        size,
        align,
    })
}

/// Build canonical `Result<T, E>` as a 2-variant enum: Ok(T) + Err(E).
fn canonical_result(ok_repr: MachineRepr, err_repr: MachineRepr) -> MachineRepr {
    let ok_size = field_size(&ok_repr);
    let ok_align = field_align(&ok_repr);
    let ok_variant = VariantRepr {
        name: Name::new(0, 0), // "Ok"
        fields: vec![ok_repr],
        size: ok_size,
        alignment: ok_align,
    };

    let err_size = field_size(&err_repr);
    let err_align = field_align(&err_repr);
    let err_variant = VariantRepr {
        name: Name::new(0, 1), // "Err"
        fields: vec![err_repr],
        size: err_size,
        alignment: err_align,
    };

    let max_payload = ok_size.max(err_size);
    let align = ok_align.max(err_align).max(8); // i64 tag
    let size = 8 + round_up(max_payload, align);

    MachineRepr::Enum(EnumRepr {
        tag: EnumTag::Explicit {
            width: IntWidth::I64,
        },
        variants: vec![ok_variant, err_variant],
        size,
        align,
    })
}

/// Whether a repr is trivial (no RC operations needed).
///
/// Recursively checks compound types: a struct/tuple is trivial if all
/// fields are trivial; an enum is trivial if all variant fields are trivial.
fn is_trivial_repr(repr: &MachineRepr) -> bool {
    match repr {
        MachineRepr::Int { .. }
        | MachineRepr::Float { .. }
        | MachineRepr::Bool
        | MachineRepr::Char
        | MachineRepr::Byte
        | MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Ordering
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::Range => true,
        // Compound types: delegate to precomputed flag or check recursively
        MachineRepr::Struct(s) => s.trivial,
        MachineRepr::Tuple(t) => t.trivial,
        MachineRepr::Enum(e) => e
            .variants
            .iter()
            .all(|v| v.fields.iter().all(is_trivial_repr)),
        // Pointers, closures, collections — all need RC
        MachineRepr::FatPointer(_)
        | MachineRepr::Closure(_)
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr
        | MachineRepr::StackPromoted { .. } => false,
    }
}

/// Size of a repr when used as a field in an aggregate (struct/tuple/enum).
///
/// Unit and Never are zero-sized in aggregates — they contribute no bytes
/// to the containing struct/tuple/enum layout. For standalone by-value
/// lowering (LLVM i64 materialization), use [`repr_size`] instead.
fn field_size(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Unit | MachineRepr::Never => 0,
        other => repr_size(other),
    }
}

/// Alignment of a repr when used as a field in an aggregate.
///
/// Unit and Never have alignment 1 (minimum) in aggregates — they don't
/// inflate the containing type's alignment requirement.
fn field_align(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Unit | MachineRepr::Never => 1,
        other => repr_align(other),
    }
}

/// ABI size of a repr in bytes (for by-value lowering).
///
/// Unit and Never are 8 bytes here (LLVM i64 representation) because
/// they must be materializable as standalone values. For aggregate field
/// sizing, use [`field_size`] instead.
fn repr_size(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Int { width, .. } => width.size_bytes(),
        MachineRepr::Float { width } => width.size_bytes(),
        MachineRepr::Bool | MachineRepr::Byte | MachineRepr::Ordering => 1,
        MachineRepr::Char => 4,
        MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr => 8,
        MachineRepr::Struct(s) => s.size,
        MachineRepr::Enum(e) => e.size,
        MachineRepr::Tuple(t) => t.size,
        MachineRepr::FatPointer(_) => 24, // {i64, i64, ptr}
        MachineRepr::Closure(_) => 16,    // {ptr fn, ptr env}
        MachineRepr::Range => 32,         // {i64, i64, i64, i64}
        MachineRepr::StackPromoted { inner, .. } => repr_size(inner),
    }
}

/// ABI alignment of a repr in bytes.
fn repr_align(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Int { width, .. } => width.alignment(),
        MachineRepr::Float { width } => width.alignment(),
        MachineRepr::Bool | MachineRepr::Byte | MachineRepr::Ordering => 1,
        MachineRepr::Char => 4,
        MachineRepr::Duration
        | MachineRepr::Size
        | MachineRepr::Unit
        | MachineRepr::Never
        | MachineRepr::FatPointer(_)
        | MachineRepr::Closure(_)
        | MachineRepr::Range
        | MachineRepr::RcPointer(_)
        | MachineRepr::OpaquePtr => 8,
        MachineRepr::Struct(s) => s.align,
        MachineRepr::Enum(e) => e.align,
        MachineRepr::Tuple(t) => t.align,
        MachineRepr::StackPromoted { inner, .. } => repr_align(inner),
    }
}

/// Round `size` up to the next multiple of `align`.
fn round_up(size: u32, align: u32) -> u32 {
    if align == 0 {
        return size;
    }
    let remainder = size % align;
    if remainder == 0 {
        size
    } else {
        size + align - remainder
    }
}

/// Compute ABI-correct layout (size, alignment) for named/tuple fields.
///
/// Uses [`field_size`]/[`field_align`] so that zero-sized types (Unit, Never)
/// don't inflate the containing aggregate.
fn compute_field_layout(fields: &[FieldRepr]) -> (u32, u32) {
    let align = fields
        .iter()
        .map(|f| field_align(&f.repr))
        .max()
        .unwrap_or(1);
    let mut offset = 0u32;
    for f in fields {
        let fa = field_align(&f.repr);
        offset = round_up(offset, fa);
        offset += field_size(&f.repr);
    }
    (round_up(offset, align), align)
}

/// Compute ABI-correct layout (size, alignment) for bare repr fields
/// (used by enum variant payloads which store `Vec<MachineRepr>`).
fn compute_payload_layout(fields: &[MachineRepr]) -> (u32, u32) {
    let align = fields.iter().map(field_align).max().unwrap_or(1);
    let mut offset = 0u32;
    for f in fields {
        let fa = field_align(f);
        offset = round_up(offset, fa);
        offset += field_size(f);
    }
    (round_up(offset, align), align)
}

impl TupleRepr {
    /// Convert tuple fields into a `MachineRepr::Tuple` with ABI-correct layout.
    fn to_machine_repr(fields: Vec<FieldRepr>, trivial: bool) -> MachineRepr {
        let (size, align) = compute_field_layout(&fields);
        MachineRepr::Tuple(TupleRepr {
            elements: fields,
            size,
            align,
            trivial,
        })
    }
}
