//! Canonical structural type equality.
//!
//! Structurally equal types may have distinct [`Idx`] values after cross-module
//! re-interning, so identity is only a fast path. Consumers use
//! [`Pool::structural_eq`] instead of maintaining private recursive comparisons.
//! The direct comparison deliberately avoids relying on the equivalent Merkle
//! hashes, preventing collisions from selecting the wrong specialization.

use std::hash::{Hash, Hasher};

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};

use crate::{Idx, Tag};

use super::Pool;

#[derive(Clone, Copy)]
enum EnumComparison {
    Nominal,
    Representation,
}

impl Pool {
    /// Structural type equality: `true` iff `a` and `b` denote the same type
    /// structure, even when interned to distinct `Idx`.
    ///
    /// Both indices are resolved via [`Pool::resolve_fully`] first, so callers
    /// pass raw `Idx` without pre-resolving. `Idx`-identity is the fast path;
    /// otherwise children are compared recursively per the type's [`Tag`].
    pub fn structural_eq(&self, a: Idx, b: Idx) -> bool {
        self.type_eq(a, b, EnumComparison::Nominal)
    }

    /// Hash the fully-resolved structural identity used by [`Self::structural_eq`].
    ///
    /// Unlike [`Self::hash`], this follows resolution links before hashing each
    /// child. It is therefore suitable as a candidate index when two aliases
    /// resolve to the same structure but retain different pool-local Merkle
    /// hashes. Hash equality is only a prefilter; callers must still confirm a
    /// candidate with [`Self::structural_eq`] so collisions cannot select a
    /// semantically different type.
    #[must_use]
    pub fn resolved_structural_hash(&self, idx: Idx) -> u64 {
        self.resolved_structural_hash_inner(
            idx,
            &mut FxHashMap::default(),
            &mut FxHashSet::default(),
        )
    }

    /// Physical representation equality after resolving semantic carriers.
    ///
    /// This differs from [`Self::structural_eq`] only for enums. Type equality
    /// treats one enum name as one nominal identity, while representation seams
    /// must compare every variant payload so distinct generic instantiations do
    /// not become interchangeable merely because they share an enum name.
    #[must_use]
    pub fn representation_eq(&self, a: Idx, b: Idx) -> bool {
        self.type_eq(a, b, EnumComparison::Representation)
    }

    fn type_eq(&self, a: Idx, b: Idx, enum_comparison: EnumComparison) -> bool {
        let a = self.resolve_fully(a);
        let b = self.resolve_fully(b);
        if a == b {
            return true;
        }

        let tag_a = self.tag(a);
        if tag_a != self.tag(b) {
            return false;
        }

        match tag_a {
            // Primitives / leaf specials: tag equality is sufficient.
            t if t.is_primitive() => true,

            // Simple containers (List/Option/Set/Iterator/Range/Channel/...):
            // single child carried in `data`.
            t if t.has_child_in_data() => self.type_eq(
                Idx::from_raw(self.data(a)),
                Idx::from_raw(self.data(b)),
                enum_comparison,
            ),

            Tag::Map => {
                self.type_eq(self.map_key(a), self.map_key(b), enum_comparison)
                    && self.type_eq(self.map_value(a), self.map_value(b), enum_comparison)
            }
            Tag::Result => {
                self.type_eq(self.result_ok(a), self.result_ok(b), enum_comparison)
                    && self.type_eq(self.result_err(a), self.result_err(b), enum_comparison)
            }
            Tag::Borrowed => self.borrowed_type_eq(a, b, enum_comparison),
            Tag::Function => self.function_type_eq(a, b, enum_comparison),
            Tag::Tuple => {
                self.type_eq_slices(&self.tuple_elems(a), &self.tuple_elems(b), enum_comparison)
            }
            Tag::Struct => self.struct_type_eq(a, b, enum_comparison),
            Tag::Enum => self.enum_type_eq(a, b, enum_comparison),
            Tag::Named => self.named_name(a) == self.named_name(b),
            Tag::Applied => self.applied_type_eq(a, b, enum_comparison),
            Tag::Scheme => {
                self.scheme_vars(a) == self.scheme_vars(b)
                    && self.type_eq(self.scheme_body(a), self.scheme_body(b), enum_comparison)
            }

            // Type variables / remaining specials: compare `data` directly.
            // (A `Tag::Var` reaching here is a PC-2 phase-contract violation;
            // comparing `data` is the conservative answer.)
            _ => self.data(a) == self.data(b),
        }
    }

    fn borrowed_type_eq(&self, a: Idx, b: Idx, enum_comparison: EnumComparison) -> bool {
        self.borrowed_lifetime(a) == self.borrowed_lifetime(b)
            && self.type_eq(
                self.borrowed_inner(a),
                self.borrowed_inner(b),
                enum_comparison,
            )
    }

    fn function_type_eq(&self, a: Idx, b: Idx, enum_comparison: EnumComparison) -> bool {
        self.type_eq_slices(
            &self.function_params(a),
            &self.function_params(b),
            enum_comparison,
        ) && self.type_eq(
            self.function_return(a),
            self.function_return(b),
            enum_comparison,
        )
    }

    fn struct_type_eq(&self, a: Idx, b: Idx, enum_comparison: EnumComparison) -> bool {
        if self.struct_name(a) != self.struct_name(b) {
            return false;
        }
        let fields_a = self.struct_fields(a);
        let fields_b = self.struct_fields(b);
        fields_a.len() == fields_b.len()
            && fields_a
                .iter()
                .zip(&fields_b)
                .all(|((name_a, ty_a), (name_b, ty_b))| {
                    name_a == name_b && self.type_eq(*ty_a, *ty_b, enum_comparison)
                })
    }

    fn enum_type_eq(&self, a: Idx, b: Idx, enum_comparison: EnumComparison) -> bool {
        if self.enum_name(a) != self.enum_name(b) {
            return false;
        }
        match enum_comparison {
            EnumComparison::Nominal => true,
            EnumComparison::Representation => {
                let variants_a = self.enum_variants(a);
                let variants_b = self.enum_variants(b);
                variants_a.len() == variants_b.len()
                    && variants_a.iter().zip(&variants_b).all(
                        |((name_a, fields_a), (name_b, fields_b))| {
                            name_a == name_b
                                && self.type_eq_slices(fields_a, fields_b, enum_comparison)
                        },
                    )
            }
        }
    }

    fn applied_type_eq(&self, a: Idx, b: Idx, enum_comparison: EnumComparison) -> bool {
        self.applied_name(a) == self.applied_name(b)
            && self.type_eq_slices(
                &self.applied_args(a),
                &self.applied_args(b),
                enum_comparison,
            )
    }

    /// Structural equality over two parallel `Idx` slices.
    fn type_eq_slices(&self, sa: &[Idx], sb: &[Idx], enum_comparison: EnumComparison) -> bool {
        sa.len() == sb.len()
            && sa
                .iter()
                .zip(sb)
                .all(|(a, b)| self.type_eq(*a, *b, enum_comparison))
    }

    fn resolved_structural_hash_inner(
        &self,
        idx: Idx,
        memo: &mut FxHashMap<Idx, u64>,
        visiting: &mut FxHashSet<Idx>,
    ) -> u64 {
        let idx = self.resolve_fully(idx);
        if let Some(&hash) = memo.get(&idx) {
            return hash;
        }
        if !visiting.insert(idx) {
            return self.resolved_cycle_hash(idx);
        }

        let tag = self.tag(idx);
        let mut hasher = FxHasher::default();
        tag.hash(&mut hasher);
        match tag {
            tag if tag.is_primitive() => {}
            tag if tag.has_child_in_data() => {
                self.resolved_structural_hash_inner(Idx::from_raw(self.data(idx)), memo, visiting)
                    .hash(&mut hasher);
            }
            Tag::Map => self.hash_resolved_children(
                [self.map_key(idx), self.map_value(idx)],
                memo,
                visiting,
                &mut hasher,
            ),
            Tag::Result => self.hash_resolved_children(
                [self.result_ok(idx), self.result_err(idx)],
                memo,
                visiting,
                &mut hasher,
            ),
            Tag::Borrowed => {
                self.borrowed_lifetime(idx).hash(&mut hasher);
                self.hash_resolved_children(
                    [self.borrowed_inner(idx)],
                    memo,
                    visiting,
                    &mut hasher,
                );
            }
            Tag::Function => {
                self.hash_resolved_children(self.function_params(idx), memo, visiting, &mut hasher);
                self.resolved_structural_hash_inner(self.function_return(idx), memo, visiting)
                    .hash(&mut hasher);
            }
            Tag::Tuple => {
                self.hash_resolved_children(self.tuple_elems(idx), memo, visiting, &mut hasher);
            }
            Tag::Struct => {
                self.struct_name(idx).hash(&mut hasher);
                let fields = self.struct_fields(idx);
                fields.len().hash(&mut hasher);
                for (name, ty) in fields {
                    name.hash(&mut hasher);
                    self.resolved_structural_hash_inner(ty, memo, visiting)
                        .hash(&mut hasher);
                }
            }
            Tag::Enum => self.enum_name(idx).hash(&mut hasher),
            Tag::Named => self.named_name(idx).hash(&mut hasher),
            Tag::Applied => {
                self.applied_name(idx).hash(&mut hasher);
                self.hash_resolved_children(self.applied_args(idx), memo, visiting, &mut hasher);
            }
            Tag::Scheme => {
                self.scheme_vars(idx).hash(&mut hasher);
                self.resolved_structural_hash_inner(self.scheme_body(idx), memo, visiting)
                    .hash(&mut hasher);
            }
            _ => self.data(idx).hash(&mut hasher),
        }

        visiting.remove(&idx);
        let hash = hasher.finish();
        memo.insert(idx, hash);
        hash
    }

    fn hash_resolved_children<I>(
        &self,
        children: I,
        memo: &mut FxHashMap<Idx, u64>,
        visiting: &mut FxHashSet<Idx>,
        hasher: &mut FxHasher,
    ) where
        I: IntoIterator<Item = Idx>,
        I::IntoIter: ExactSizeIterator,
    {
        let children = children.into_iter();
        children.len().hash(hasher);
        for child in children {
            self.resolved_structural_hash_inner(child, memo, visiting)
                .hash(hasher);
        }
    }

    fn resolved_cycle_hash(&self, idx: Idx) -> u64 {
        let mut hasher = FxHasher::default();
        "recursive-type".hash(&mut hasher);
        let tag = self.tag(idx);
        tag.hash(&mut hasher);
        match tag {
            Tag::Struct => self.struct_name(idx).hash(&mut hasher),
            Tag::Enum => self.enum_name(idx).hash(&mut hasher),
            Tag::Named => self.named_name(idx).hash(&mut hasher),
            Tag::Applied => self.applied_name(idx).hash(&mut hasher),
            _ => self.data(idx).hash(&mut hasher),
        }
        hasher.finish()
    }
}

#[cfg(test)]
mod tests;
