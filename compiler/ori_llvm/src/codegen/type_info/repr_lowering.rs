//! `ReprPlan` → LLVM type conversion utilities.
//!
//! These methods on [`TypeLayoutResolver`] convert `ori_repr::MachineRepr`
//! entries to LLVM types without recursion through `TypeInfoStore`. Used
//! as fast paths in `resolve_inner()` when the `ReprPlan` has a decision.

use inkwell::types::BasicTypeEnum;

use super::layout_resolver::TypeLayoutResolver;

impl<'ll> TypeLayoutResolver<'_, 'll, '_> {
    /// Try to convert a `MachineRepr` to an LLVM type for non-recursive types.
    ///
    /// Returns `Some` for primitives, fat pointers, opaque pointers, closures,
    /// and ranges — these have fixed LLVM layouts independent of child type
    /// resolution. Returns `None` for recursive types (Struct, Enum, Tuple,
    /// `StackPromoted`) that require `TypeInfoStore`'s two-phase creation pattern.
    pub(super) fn try_repr_to_llvm_type(
        &self,
        repr: &ori_repr::MachineRepr,
    ) -> Option<BasicTypeEnum<'ll>> {
        use ori_repr::{FloatWidth, IntWidth, MachineRepr};

        match repr {
            MachineRepr::Int { width, .. } => Some(match width {
                IntWidth::I8 => self.scx.type_i8().into(),
                IntWidth::I16 => self.scx.type_i16().into(),
                IntWidth::I32 => self.scx.type_i32().into(),
                IntWidth::I64 => self.scx.type_i64().into(),
            }),
            MachineRepr::Float { width } => Some(match width {
                FloatWidth::F32 => self.scx.type_f32().into(),
                FloatWidth::F64 => self.scx.type_f64().into(),
            }),
            MachineRepr::Bool => Some(self.scx.type_i1().into()),
            MachineRepr::Char => Some(self.scx.type_i32().into()),
            MachineRepr::Byte | MachineRepr::Ordering => Some(self.scx.type_i8().into()),
            MachineRepr::Duration | MachineRepr::Size | MachineRepr::Unit | MachineRepr::Never => {
                Some(self.scx.type_i64().into())
            }
            MachineRepr::Range => Some(
                self.scx
                    .type_struct(
                        &[
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                        ],
                        false,
                    )
                    .into(),
            ),
            MachineRepr::FatPointer(_) => Some(
                self.scx
                    .type_struct(
                        &[
                            self.scx.type_i64().into(),
                            self.scx.type_i64().into(),
                            self.scx.type_ptr().into(),
                        ],
                        false,
                    )
                    .into(),
            ),
            MachineRepr::OpaquePtr | MachineRepr::UnmanagedPtr | MachineRepr::RcPointer(_) => {
                Some(self.scx.type_ptr().into())
            }
            MachineRepr::Closure(_) => Some(
                self.scx
                    .type_struct(
                        &[self.scx.type_ptr().into(), self.scx.type_ptr().into()],
                        false,
                    )
                    .into(),
            ),
            // Recursive types — require two-phase creation via TypeInfoStore.
            // Narrowed Struct/Tuple types are handled in resolve_inner() by
            // try_lower_narrowed_aggregate(), not here — that ensures only
            // actually-narrowed structs bypass the named struct path.
            MachineRepr::Struct(_)
            | MachineRepr::Enum(_)
            | MachineRepr::Tuple(_)
            | MachineRepr::StackPromoted { .. } => None,
        }
    }

    /// Try to lower a narrowed Struct/Tuple from the `ReprPlan` to an LLVM type.
    ///
    /// Returns `Some` only if the aggregate has at least one narrowed int field
    /// (`MachineRepr::Int { width != I64 }`) and all field types can be resolved
    /// without recursive type resolution. Non-narrowed aggregates return `None`
    /// to preserve the existing named-struct creation path.
    pub(super) fn try_lower_narrowed_aggregate(
        &self,
        repr: &ori_repr::MachineRepr,
    ) -> Option<BasicTypeEnum<'ll>> {
        use ori_repr::MachineRepr;

        let fields = match repr {
            MachineRepr::Struct(s) => &s.fields[..],
            MachineRepr::Tuple(t) => &t.elements[..],
            _ => return None,
        };

        // Only enter the narrowed path if at least one field was actually
        // narrowed. Check for both integer narrowing (Int width < I64)
        // and float narrowing (Float width F32).
        let has_narrowed = fields.iter().any(|f| {
            matches!(
                f.repr,
                MachineRepr::Int { width, .. } if width != ori_repr::IntWidth::I64
            ) || matches!(
                f.repr,
                MachineRepr::Float { width } if width != ori_repr::FloatWidth::F64
            )
        });
        if !has_narrowed {
            return None;
        }

        // Safety: only create narrowed LLVM types when ALL fields are
        // scalar types. Mixed structs (e.g., { str, int }) change the
        // overall struct size, which breaks element_store_size() and
        // elem_dec_fn assumptions in collection codegen (list element
        // GEP stride, RC cleanup offsets). Mixed-field narrowing requires
        // Integer narrowing phase C: element_store_size integration.
        let all_scalar_fields = fields.iter().all(|f| {
            matches!(
                f.repr,
                MachineRepr::Int { .. }
                    | MachineRepr::Float { .. }
                    | MachineRepr::Bool
                    | MachineRepr::Char
                    | MachineRepr::Byte
                    | MachineRepr::Unit
                    | MachineRepr::Duration
                    | MachineRepr::Size
                    | MachineRepr::Ordering
                    | MachineRepr::Never
            )
        });
        if !all_scalar_fields {
            return None;
        }

        // Resolve all field types. If any field can't be resolved (nested
        // Struct/Enum), return None to fall through to TypeInfoStore.
        let field_types: Option<Vec<BasicTypeEnum<'ll>>> = fields
            .iter()
            .map(|f| self.try_repr_to_llvm_type(&f.repr))
            .collect();
        field_types.map(|ft| self.scx.type_struct(&ft, false).into())
    }
}
