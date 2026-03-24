//! Layout computation utilities for `MachineRepr`.
//!
//! Provides ABI-correct size, alignment, and layout computation for
//! machine representations. Used by [`crate::canonical`] during
//! initial population and by §06 (struct layout optimization).

use crate::repr::MachineRepr;
use crate::struct_repr::{FieldRepr, TupleRepr};

/// Whether a repr is trivial (no RC operations needed).
///
/// Recursively checks compound types: a struct/tuple is trivial if all
/// fields are trivial; an enum is trivial if all variant fields are trivial.
///
/// This is the **single canonical definition** of triviality at the
/// `MachineRepr` level — used by both `canonical::populate_canonical()`
/// and `ReprPlan::is_trivial()`.
pub(crate) fn is_trivial_repr(repr: &MachineRepr) -> bool {
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
        MachineRepr::Struct(s) => s.trivial,
        MachineRepr::Tuple(t) => t.trivial,
        MachineRepr::Enum(e) => e
            .variants
            .iter()
            .all(|v| v.fields.iter().all(is_trivial_repr)),
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
pub(crate) fn field_size(repr: &MachineRepr) -> u32 {
    match repr {
        MachineRepr::Unit | MachineRepr::Never => 0,
        other => repr_size(other),
    }
}

/// Alignment of a repr when used as a field in an aggregate.
///
/// Unit and Never have alignment 1 (minimum) in aggregates — they don't
/// inflate the containing type's alignment requirement.
pub(crate) fn field_align(repr: &MachineRepr) -> u32 {
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
pub(crate) fn repr_size(repr: &MachineRepr) -> u32 {
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
pub(crate) fn repr_align(repr: &MachineRepr) -> u32 {
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
pub(crate) fn round_up(size: u32, align: u32) -> u32 {
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
pub(crate) fn compute_field_layout(fields: &[FieldRepr]) -> (u32, u32) {
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
pub(crate) fn compute_payload_layout(fields: &[MachineRepr]) -> (u32, u32) {
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
    pub(crate) fn to_machine_repr(fields: Vec<FieldRepr>, trivial: bool) -> MachineRepr {
        let (size, align) = compute_field_layout(&fields);
        MachineRepr::Tuple(TupleRepr {
            elements: fields,
            size,
            align,
            trivial,
        })
    }
}
