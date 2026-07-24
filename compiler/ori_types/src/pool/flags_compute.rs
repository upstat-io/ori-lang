//! Flag computation for pool types.
//!
//! Called by `Pool::intern` + `Pool::intern_complex` at type-interning time
//! to precompute `TypeFlags`. Flags propagate from children using
//! `TypeFlags::propagate_from` per.

use crate::{Tag, TypeFlags};

use super::Pool;

impl Pool {
    /// Compute flags for a type. Dispatches to per-category helpers.
    pub(super) fn compute_flags(&self, tag: Tag, data: u32, extra: &[u32]) -> TypeFlags {
        match tag {
            // Primitives
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Str
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => {
                TypeFlags::IS_PRIMITIVE
                    | TypeFlags::IS_RESOLVED
                    | TypeFlags::IS_MONO
                    | TypeFlags::IS_COPYABLE
            }

            Tag::Never => TypeFlags::IS_PRIMITIVE | TypeFlags::IS_RESOLVED | TypeFlags::IS_MONO,

            Tag::Error => TypeFlags::IS_PRIMITIVE | TypeFlags::HAS_ERROR | TypeFlags::IS_RESOLVED,

            // Simple single-child containers (data = child idx)
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => self.compute_flags_simple_container(data),

            // Two-child containers (extra = [child0, child1] or [inner, lifetime])
            Tag::Map | Tag::Result | Tag::Borrowed => self.compute_flags_two_child(tag, extra),

            // Variables
            Tag::Var => TypeFlags::HAS_VAR | TypeFlags::NEEDS_SUBST,
            Tag::BoundVar => TypeFlags::HAS_BOUND_VAR | TypeFlags::NEEDS_SUBST,
            Tag::RigidVar => TypeFlags::HAS_RIGID_VAR | TypeFlags::NEEDS_SUBST,

            // Length-prefixed compound types
            Tag::Function | Tag::Tuple | Tag::Struct => self.compute_flags_complex(tag, extra),

            // Enum has its own per-variant walk
            Tag::Enum => self.compute_enum_flags(extra),

            // Named family
            Tag::Applied | Tag::Named | Tag::Alias => self.compute_flags_named(tag, extra),

            // Scheme
            Tag::Scheme => self.compute_flags_scheme(extra),

            // Special
            Tag::Projection => TypeFlags::HAS_PROJECTION,
            Tag::Infer => TypeFlags::HAS_INFER,
            Tag::SelfType => TypeFlags::HAS_SELF,
            Tag::ModuleNs => TypeFlags::empty(),
        }
    }

    /// Single-child container: `data` is the child Idx raw value.
    fn compute_flags_simple_container(&self, data: u32) -> TypeFlags {
        let child_flags = self.flags[data as usize];
        TypeFlags::IS_CONTAINER | TypeFlags::propagate_from(child_flags)
    }

    /// Two-child container flags: `Map`, `Result`, `Borrowed`.
    fn compute_flags_two_child(&self, tag: Tag, extra: &[u32]) -> TypeFlags {
        match tag {
            Tag::Map | Tag::Result => {
                let child1_flags = self.flags[extra[0] as usize];
                let child2_flags = self.flags[extra[1] as usize];
                TypeFlags::IS_CONTAINER
                    | TypeFlags::propagate_from(child1_flags)
                    | TypeFlags::propagate_from(child2_flags)
            }
            Tag::Borrowed => {
                let inner_flags = self.flags[extra[0] as usize];
                TypeFlags::IS_CONTAINER | TypeFlags::propagate_from(inner_flags)
            }
            _ => unreachable!("compute_flags_two_child: unexpected tag {:?}", tag),
        }
    }

    /// Length-prefixed compound: `Function`, `Tuple`, `Struct`.
    fn compute_flags_complex(&self, tag: Tag, extra: &[u32]) -> TypeFlags {
        match tag {
            // extra layout: [param_count, param0, ..., return_type]
            Tag::Function => {
                let param_count = extra[0] as usize;
                let mut flags = TypeFlags::IS_FUNCTION;

                for i in 0..param_count {
                    let param_idx = extra[1 + i] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[param_idx]);
                }

                let ret_idx = extra[1 + param_count] as usize;
                flags |= TypeFlags::propagate_from(self.flags[ret_idx]);

                flags
            }

            // extra layout: [elem_count, elem0, elem1, ...]
            Tag::Tuple => {
                let elem_count = extra[0] as usize;
                let mut flags = TypeFlags::IS_COMPOSITE;

                for i in 0..elem_count {
                    let elem_idx = extra[1 + i] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[elem_idx]);
                }

                flags
            }

            // extra layout: [name_lo, name_hi, field_count, f0_name, f0_type, ...]
            Tag::Struct => {
                let field_count = extra[2] as usize;
                let mut flags = TypeFlags::IS_COMPOSITE;

                for i in 0..field_count {
                    let field_type_idx = extra[3 + i * 2 + 1] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[field_type_idx]);
                }

                flags
            }

            _ => unreachable!("compute_flags_complex: unexpected tag {:?}", tag),
        }
    }

    /// Named family: `Applied`, `Named`, `Alias`.
    fn compute_flags_named(&self, tag: Tag, extra: &[u32]) -> TypeFlags {
        match tag {
            Tag::Applied => {
                // extra layout: [name_lo, name_hi, arg_count, arg0, arg1, ...]
                let arg_count = extra[2] as usize;
                let mut flags = TypeFlags::IS_NAMED;

                for i in 0..arg_count {
                    let arg_idx = extra[3 + i] as usize;
                    flags |= TypeFlags::propagate_from(self.flags[arg_idx]);
                }

                flags
            }
            Tag::Named | Tag::Alias => TypeFlags::IS_NAMED,
            _ => unreachable!("compute_flags_named: unexpected tag {:?}", tag),
        }
    }

    /// Scheme propagation. Extra layout:
    /// `[var_count, var_id_0, ..., var_id_{N-1}, body_idx]` per.
    /// Propagates `PROPAGATE_MASK` from the body (TF-3).
    fn compute_flags_scheme(&self, extra: &[u32]) -> TypeFlags {
        let var_count = extra[0] as usize;
        let body_idx = extra[1 + var_count] as usize;
        let mut flags = TypeFlags::IS_SCHEME;
        flags |= TypeFlags::propagate_from(self.flags[body_idx]);
        flags
    }

    /// Compute flags for an Enum type, propagating from variant field types.
    ///
    /// Extra layout: `[name_lo, name_hi, variant_count, v0_name, v0_fc, v0_f0, ..., v1_name, ...]`
    fn compute_enum_flags(&self, extra: &[u32]) -> TypeFlags {
        let variant_count = extra[2] as usize;
        let mut flags = TypeFlags::IS_COMPOSITE;
        let mut offset = 3;

        for _ in 0..variant_count {
            let field_count = extra[offset + 1] as usize;
            for j in 0..field_count {
                let field_type_idx = extra[offset + 2 + j] as usize;
                flags |= TypeFlags::propagate_from(self.flags[field_type_idx]);
            }
            offset += 2 + field_count;
        }

        flags
    }
}
