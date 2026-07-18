//! Structural type equality — SSOT.
//!
//! Two `Idx` are structurally equal when they denote the same type STRUCTURE
//! (same [`Tag`] and same shape/children), even when they were interned to
//! distinct `Idx` values. Distinct interning of structurally-equal types is an
//! expected, load-bearing property of the content-addressed pool (e.g. a type
//! re-interned during cross-module merging, per `pool/re_intern`): two `[int]`
//! can carry distinct `Idx` in the same merged pool. `Idx`-identity equality
//! (`a == b`) answers a STRICTER question than structural equality and is
//! sound only as a fast path.
//!
//! This predicate is the canonical home for structural type comparison.
//! Consumers (mono-dispatch fallback resolution in `ori_llvm`, alias
//! propagation in `ori_repr`) call [`Pool::structural_eq`]; they SHALL NOT
//! maintain a private recursive `Tag`+children comparison (one canonical
//! home for structural type comparison).
//!
//! The tested invariant `merkle_hash == structural_eq` (see `pool/tests.rs`)
//! makes a per-`Idx` Merkle-hash comparison an equivalent answer, but this
//! predicate uses the direct recursive comparison so a hash collision can
//! never silently select the wrong monomorphized specialization.

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
            Tag::Borrowed => {
                self.borrowed_lifetime(a) == self.borrowed_lifetime(b)
                    && self.type_eq(
                        self.borrowed_inner(a),
                        self.borrowed_inner(b),
                        enum_comparison,
                    )
            }

            Tag::Function => {
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
            Tag::Tuple => {
                self.type_eq_slices(&self.tuple_elems(a), &self.tuple_elems(b), enum_comparison)
            }

            Tag::Struct => {
                if self.struct_name(a) != self.struct_name(b) {
                    return false;
                }
                let fa = self.struct_fields(a);
                let fb = self.struct_fields(b);
                fa.len() == fb.len()
                    && fa.iter().zip(&fb).all(|((na, ta), (nb, tb))| {
                        na == nb && self.type_eq(*ta, *tb, enum_comparison)
                    })
            }
            // Nominal identity (TI-5): an enum's identity is its name. Two
            // distinctly-interned `Tag::Enum` entries denote the same nominal
            // type iff they carry the same name. Generic instantiations are
            // `Tag::Applied`, handled above; a bare `Tag::Enum` is the nominal
            // declaration.
            Tag::Enum => {
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

            Tag::Named => self.named_name(a) == self.named_name(b),
            Tag::Applied => {
                self.applied_name(a) == self.applied_name(b)
                    && self.type_eq_slices(
                        &self.applied_args(a),
                        &self.applied_args(b),
                        enum_comparison,
                    )
            }

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

    /// Structural equality over two parallel `Idx` slices.
    fn type_eq_slices(&self, sa: &[Idx], sb: &[Idx], enum_comparison: EnumComparison) -> bool {
        sa.len() == sb.len()
            && sa
                .iter()
                .zip(sb)
                .all(|(a, b)| self.type_eq(*a, *b, enum_comparison))
    }
}

#[cfg(test)]
mod tests;
