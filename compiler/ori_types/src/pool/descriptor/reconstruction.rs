//! Descriptor reconstruction: rebuilding types in a target [`Pool`] from a
//! topologically-sorted descriptor sequence.

use rustc_hash::{FxBuildHasher, FxHashMap};

use ori_ir::Name;

use crate::pool::EnumVariant;
use crate::{Idx, LifetimeId, Tag};

use super::{Pool, TypeDescriptor};

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
