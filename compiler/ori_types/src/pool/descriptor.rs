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

use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use ori_ir::Name;

use crate::{Idx, LifetimeId, Tag};

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

// === Descriptor Generation ===

impl Pool {
    /// Generate a portable [`TypeDescriptor`] for a single type.
    ///
    /// The descriptor references children by Merkle hash, not by Idx.
    /// Uses Pool accessor methods internally — does not depend on extra array layout.
    pub fn describe(&self, idx: Idx) -> TypeDescriptor {
        let tag = self.tag(idx);

        // Simple containers: data = child Idx
        if tag.has_child_in_data() {
            let child_idx = Idx::from_raw(self.data(idx));
            return TypeDescriptor::Container {
                tag,
                child_hash: self.hash(child_idx),
            };
        }

        // Complex types with extra data
        if tag.uses_extra() {
            return self.describe_complex(tag, idx);
        }

        // Leaf types: primitives, variables, specials
        if tag.is_type_variable() {
            return TypeDescriptor::Variable {
                tag,
                var_id: self.data(idx),
            };
        }

        // Primitives and other leaves (Error, Never, Infer, SelfType, ModuleNs)
        TypeDescriptor::Primitive(tag)
    }

    /// Generate a descriptor for a complex type (one that uses the extra array).
    fn describe_complex(&self, tag: Tag, idx: Idx) -> TypeDescriptor {
        match tag {
            // Two-child containers
            Tag::Map | Tag::Result => {
                let (c1, c2) = match tag {
                    Tag::Map => (self.map_key(idx), self.map_value(idx)),
                    Tag::Result => (self.result_ok(idx), self.result_err(idx)),
                    _ => unreachable!(),
                };
                TypeDescriptor::TwoChild {
                    tag,
                    child1_hash: self.hash(c1),
                    child2_hash: self.hash(c2),
                }
            }

            // Borrowed reference
            Tag::Borrowed => TypeDescriptor::Borrowed {
                inner_hash: self.hash(self.borrowed_inner(idx)),
                lifetime_id: self.borrowed_lifetime(idx).raw(),
            },

            // Function
            Tag::Function => {
                let params = self.function_params(idx);
                let ret = self.function_return(idx);
                TypeDescriptor::Function {
                    param_hashes: params.iter().map(|&p| self.hash(p)).collect(),
                    return_hash: self.hash(ret),
                }
            }

            // Tuple
            Tag::Tuple => {
                let elems = self.tuple_elems(idx);
                TypeDescriptor::Tuple {
                    element_hashes: elems.iter().map(|&e| self.hash(e)).collect(),
                }
            }

            // Struct
            Tag::Struct => {
                let name = self.struct_name(idx);
                let fields = self.struct_fields(idx);
                TypeDescriptor::Struct {
                    name,
                    field_names: fields.iter().map(|&(n, _)| n).collect(),
                    field_type_hashes: fields.iter().map(|&(_, t)| self.hash(t)).collect(),
                }
            }

            // Enum
            Tag::Enum => {
                let name = self.enum_name(idx);
                let variants = self.enum_variants(idx);
                TypeDescriptor::Enum {
                    name,
                    variants: variants
                        .into_iter()
                        .map(|(vname, field_types)| VariantDescriptor {
                            name: vname,
                            field_type_hashes: field_types.iter().map(|&t| self.hash(t)).collect(),
                        })
                        .collect(),
                }
            }

            // Applied generic
            Tag::Applied => {
                let name = self.applied_name(idx);
                let args = self.applied_args(idx);
                TypeDescriptor::Applied {
                    name,
                    arg_hashes: args.iter().map(|&a| self.hash(a)).collect(),
                }
            }

            // Named (unresolved reference)
            Tag::Named => TypeDescriptor::Named {
                name: self.named_name(idx),
            },

            // Scheme (quantified type)
            Tag::Scheme => {
                let vars = self.scheme_vars(idx);
                let body = self.scheme_body(idx);
                TypeDescriptor::Scheme {
                    var_count: vars.len(),
                    var_ids: vars.to_vec(),
                    body_hash: self.hash(body),
                }
            }

            // Alias, Projection — opaque/transient, treat as primitives
            Tag::Alias | Tag::Projection => TypeDescriptor::Primitive(tag),

            _ => unreachable!(
                "uses_extra() returned true for {tag:?} but describe_complex has no handler"
            ),
        }
    }

    /// Generate a topologically sorted descriptor sequence for a type and
    /// all its transitive dependencies.
    ///
    /// Leaves (primitives) appear first. Each descriptor's child hashes reference
    /// types that appear earlier in the sequence. Deduplicates by hash.
    ///
    /// Returns `(merkle_hash, descriptor)` pairs.
    pub fn describe_transitive(&self, idx: Idx) -> Vec<(u64, TypeDescriptor)> {
        let mut result = Vec::new();
        let mut visited = FxHashSet::default();
        self.describe_recursive(idx, &mut result, &mut visited);
        result
    }

    /// Recursively describe a type, visiting children first (topological order).
    ///
    /// Public to allow callers (e.g., export descriptor generation) to share
    /// a single `visited` set across multiple root types, avoiding duplicates.
    pub fn describe_recursive(
        &self,
        idx: Idx,
        result: &mut Vec<(u64, TypeDescriptor)>,
        visited: &mut FxHashSet<u64>,
    ) {
        let hash = self.hash(idx);
        if !visited.insert(hash) {
            return; // Already described
        }

        // Recurse into children first (topological order)
        self.visit_children(idx, |child| {
            self.describe_recursive(child, result, visited);
        });

        // Then add self
        result.push((hash, self.describe(idx)));
    }

    /// Visit all child Idx values of a type, calling `f` for each.
    ///
    /// This mirrors the child-discovery logic of `merkle_hash`/`merkle_hash_extra`
    /// but instead of hashing, it yields each child for recursive visitation.
    fn visit_children(&self, idx: Idx, mut f: impl FnMut(Idx)) {
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

// === Descriptor Reconstruction ===

impl Pool {
    /// Reconstruct types from a topologically sorted descriptor sequence.
    ///
    /// For each `(hash, descriptor)` pair:
    /// 1. Check if the type already exists in this Pool (by Merkle hash) → skip
    /// 2. If not: resolve child hashes to local [`Idx`] values, intern the type
    ///
    /// Returns a mapping from Merkle hash → local [`Idx`] for all types in the
    /// sequence (both pre-existing and newly created).
    ///
    /// # Panics
    ///
    /// Panics if a descriptor references a child hash not found in the mapping.
    /// This cannot happen with sequences produced by [`Pool::describe_transitive()`],
    /// which guarantees topological ordering.
    pub fn reconstruct(&mut self, descriptors: &[(u64, TypeDescriptor)]) -> FxHashMap<u64, Idx> {
        let mut hash_to_idx: FxHashMap<u64, Idx> =
            FxHashMap::with_capacity_and_hasher(descriptors.len(), FxBuildHasher);

        for (hash, desc) in descriptors {
            // Already exists in this pool?
            if let Some(idx) = self.lookup_by_hash(*hash) {
                hash_to_idx.insert(*hash, idx);
                continue;
            }

            let idx = self.reconstruct_one(desc, &hash_to_idx);
            hash_to_idx.insert(*hash, idx);
        }

        hash_to_idx
    }

    /// Reconstruct a single type from its descriptor, using `hash_to_idx` to
    /// resolve child references.
    fn reconstruct_one(&mut self, desc: &TypeDescriptor, hash_to_idx: &FxHashMap<u64, Idx>) -> Idx {
        match desc {
            TypeDescriptor::Primitive(tag) => {
                // Primitives are pre-interned at fixed indices — they should
                // always be found by lookup_by_hash above. If we reach here,
                // the pool is missing a primitive (shouldn't happen).
                self.intern(*tag, 0)
            }

            TypeDescriptor::Container { tag, child_hash } => {
                let child = hash_to_idx[child_hash];
                self.intern(*tag, child.raw())
            }

            TypeDescriptor::TwoChild {
                tag,
                child1_hash,
                child2_hash,
            } => {
                let c1 = hash_to_idx[child1_hash];
                let c2 = hash_to_idx[child2_hash];
                match tag {
                    Tag::Map => self.map(c1, c2),
                    Tag::Result => self.result(c1, c2),
                    _ => unreachable!("TwoChild with unexpected tag: {tag:?}"),
                }
            }

            TypeDescriptor::Borrowed {
                inner_hash,
                lifetime_id,
            } => {
                let inner = hash_to_idx[inner_hash];
                self.borrowed(inner, LifetimeId::from_raw(*lifetime_id))
            }

            TypeDescriptor::Function {
                param_hashes,
                return_hash,
            } => {
                let params: Vec<Idx> = param_hashes.iter().map(|h| hash_to_idx[h]).collect();
                let ret = hash_to_idx[return_hash];
                self.function(&params, ret)
            }

            TypeDescriptor::Tuple { element_hashes } => {
                let elems: Vec<Idx> = element_hashes.iter().map(|h| hash_to_idx[h]).collect();
                self.tuple(&elems)
            }

            TypeDescriptor::Struct {
                name,
                field_names,
                field_type_hashes,
            } => {
                let fields: Vec<(Name, Idx)> = field_names
                    .iter()
                    .zip(field_type_hashes)
                    .map(|(&n, h)| (n, hash_to_idx[h]))
                    .collect();
                self.struct_type(*name, &fields)
            }

            TypeDescriptor::Enum { name, variants } => {
                use super::EnumVariant;
                let enum_variants: Vec<EnumVariant> = variants
                    .iter()
                    .map(|vd| EnumVariant {
                        name: vd.name,
                        field_types: vd
                            .field_type_hashes
                            .iter()
                            .map(|h| hash_to_idx[h])
                            .collect(),
                    })
                    .collect();
                self.enum_type(*name, &enum_variants)
            }

            TypeDescriptor::Applied { name, arg_hashes } => {
                let args: Vec<Idx> = arg_hashes.iter().map(|h| hash_to_idx[h]).collect();
                self.applied(*name, &args)
            }

            TypeDescriptor::Named { name } => self.named(*name),

            TypeDescriptor::Scheme {
                var_count: _,
                var_ids,
                body_hash,
            } => {
                let body = hash_to_idx[body_hash];
                self.scheme(var_ids, body)
            }

            TypeDescriptor::Variable { tag, var_id } => self.intern(*tag, *var_id),
        }
    }
}

#[cfg(test)]
mod tests;
