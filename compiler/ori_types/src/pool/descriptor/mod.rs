//! Portable type descriptors for cross-module type reconstruction.
//!
//! Unlike Pool types (which use [`Idx`] references), [`TypeDescriptor`]s reference
//! children by their Merkle hashes. This makes them portable across [`Pool`]
//! instances — they carry no pool-local state.
//!
//! Descriptors enable zero-AST type reconstruction: an importing module can
//! reconstruct any type in its own Pool from a sequence of descriptors, without
//! accessing the originating Pool or walking AST nodes.
//!
//! # Topological ordering
//!
//! Descriptor sequences are topologically sorted: if descriptor D references
//! children with hashes H1, H2, ..., those children appear earlier in the
//! sequence. This enables single-pass bottom-up reconstruction.
//!
//! # Size estimates
//!
//! - Primitive: 2 bytes (tag only)
//! - Container: 10 bytes (tag + hash)
//! - Function with 3 params: ~34 bytes (3 × 8 + 8 + overhead)
//! - Struct with 5 fields: ~90 bytes (name + 5 × (name + hash))
//! - Typical imported function's types: 5-10 descriptors, ~100-300 bytes total

mod generation;
mod reconstruction;

use ori_ir::Name;

use crate::{Idx, Tag};

use super::Pool;

/// A self-contained type description that can reconstruct a type in any Pool.
///
/// All child type references use Merkle hashes (`u64`) instead of pool-local
/// `Idx` values. This makes descriptors portable — they can be transferred
/// between compilation units and reconstructed in any Pool.
///
/// See module docs for topological ordering guarantees.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeDescriptor {
    /// Primitive type (int, str, bool, etc.)
    /// Merkle hash: `hash(tag)`
    Primitive(Tag),

    /// Simple container (List<T>, Option<T>, Set<T>, Iterator<T>, etc.)
    /// Merkle hash: `hash(tag, child_hash)`
    Container { tag: Tag, child_hash: u64 },

    /// Two-child container (Map<K,V>, Result<T,E>)
    /// Merkle hash: `hash(tag, child1_hash, child2_hash)`
    TwoChild {
        tag: Tag,
        child1_hash: u64,
        child2_hash: u64,
    },

    /// Borrowed reference (&T with lifetime)
    /// Merkle hash: `hash(BORROWED, inner_hash, lifetime_id)`
    Borrowed { inner_hash: u64, lifetime_id: u32 },

    /// Function type `(P1, P2, ...) -> R`
    /// Merkle hash: `hash(FUNCTION, count, param_hashes..., return_hash)`
    Function {
        param_hashes: Vec<u64>,
        return_hash: u64,
    },

    /// Tuple type `(T1, T2, ...)`
    /// Merkle hash: `hash(TUPLE, count, element_hashes...)`
    Tuple { element_hashes: Vec<u64> },

    /// Struct type `struct Name { fields... }`
    /// Merkle hash: `hash(STRUCT, name, field_count, [name, type_hash]...)`
    Struct {
        name: Name,
        field_names: Vec<Name>,
        field_type_hashes: Vec<u64>,
    },

    /// Enum type `enum Name { variants... }`
    /// Merkle hash: `hash(ENUM, name, variant_count, [name, fc, type_hashes...]...)`
    Enum {
        name: Name,
        variants: Vec<VariantDescriptor>,
    },

    /// Applied generic type `Foo<A, B, ...>`
    /// Merkle hash: `hash(APPLIED, name, arg_count, arg_hashes...)`
    Applied { name: Name, arg_hashes: Vec<u64> },

    /// Named type reference (unresolved)
    /// Merkle hash: `hash(NAMED, name)`
    Named { name: Name },

    /// Type scheme `forall vars. body`
    /// Merkle hash: `hash(SCHEME, var_count, var_ids..., body_hash)`
    Scheme {
        var_count: usize,
        var_ids: Vec<u32>,
        body_hash: u64,
    },

    /// Type variable (`Var`, `BoundVar`, `RigidVar`)
    /// Merkle hash: `hash(tag, var_id)`
    Variable { tag: Tag, var_id: u32 },
}

/// Variant descriptor within an enum [`TypeDescriptor`].
///
/// Each variant carries its name and the Merkle hashes of its field types
/// (positional fields in order). Variants with no fields have an empty
/// `field_type_hashes` vec.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VariantDescriptor {
    /// Variant name (e.g., `Some`, `None`, `Ok`, `Err`).
    pub name: Name,
    /// Merkle hashes of the variant's field types, in order.
    pub field_type_hashes: Vec<u64>,
}

// Shared child traversal.
impl Pool {
    /// Visit all child Idx values of a type, calling `f` for each.
    ///
    /// This mirrors the child-discovery logic of `merkle_hash`/`merkle_hash_extra`
    /// but instead of hashing, it yields each child for recursive visitation.
    ///
    /// Promoted to `pub(crate)` for
    /// [`crate::check::validators::validate_body_types`] — the producer-side
    /// PC-2 enforcer reuses this walker rather than cloning a parallel
    /// tag-dispatch ladder. Treats `Named` / `Alias` / `Projection` as leaves
    /// (no child recursion); this is sound for the validator because PC-2
    /// guarantees no `Tag::Projection` survives in the typed IR by the time
    /// body inference completes.
    pub(crate) fn visit_children(&self, idx: Idx, mut f: impl FnMut(Idx)) {
        let tag = self.tag(idx);

        // Simple containers: single child in data
        if tag.has_child_in_data() {
            f(Idx::from_raw(self.data(idx)));
            return;
        }

        // Complex types with children in extra
        match tag {
            Tag::Map => {
                f(self.map_key(idx));
                f(self.map_value(idx));
            }
            Tag::Result => {
                f(self.result_ok(idx));
                f(self.result_err(idx));
            }
            Tag::Borrowed => {
                f(self.borrowed_inner(idx));
            }
            Tag::Function => {
                for p in self.function_params(idx) {
                    f(p);
                }
                f(self.function_return(idx));
            }
            Tag::Tuple => {
                for e in self.tuple_elems(idx) {
                    f(e);
                }
            }
            Tag::Struct => {
                for (_, ty) in self.struct_fields(idx) {
                    f(ty);
                }
            }
            Tag::Enum => {
                for (_, field_types) in self.enum_variants(idx) {
                    for ty in field_types {
                        f(ty);
                    }
                }
            }
            Tag::Applied => {
                for a in self.applied_args(idx) {
                    f(a);
                }
            }
            Tag::Scheme => {
                f(self.scheme_body(idx));
            }
            // Named, Alias, Projection — no child types
            // Leaves (primitives, variables) — no children
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
