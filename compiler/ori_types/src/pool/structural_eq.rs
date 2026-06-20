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

impl Pool {
    /// Structural type equality: `true` iff `a` and `b` denote the same type
    /// structure, even when interned to distinct `Idx`.
    ///
    /// Both indices are resolved via [`Pool::resolve_fully`] first, so callers
    /// pass raw `Idx` without pre-resolving. `Idx`-identity is the fast path;
    /// otherwise children are compared recursively per the type's [`Tag`].
    pub fn structural_eq(&self, a: Idx, b: Idx) -> bool {
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
            t if t.has_child_in_data() => {
                self.structural_eq(Idx::from_raw(self.data(a)), Idx::from_raw(self.data(b)))
            }

            Tag::Map => {
                self.structural_eq(self.map_key(a), self.map_key(b))
                    && self.structural_eq(self.map_value(a), self.map_value(b))
            }
            Tag::Result => {
                self.structural_eq(self.result_ok(a), self.result_ok(b))
                    && self.structural_eq(self.result_err(a), self.result_err(b))
            }
            Tag::Borrowed => {
                self.borrowed_lifetime(a) == self.borrowed_lifetime(b)
                    && self.structural_eq(self.borrowed_inner(a), self.borrowed_inner(b))
            }

            Tag::Function => {
                self.structural_eq_slices(&self.function_params(a), &self.function_params(b))
                    && self.structural_eq(self.function_return(a), self.function_return(b))
            }
            Tag::Tuple => self.structural_eq_slices(&self.tuple_elems(a), &self.tuple_elems(b)),

            Tag::Struct => {
                if self.struct_name(a) != self.struct_name(b) {
                    return false;
                }
                let fa = self.struct_fields(a);
                let fb = self.struct_fields(b);
                fa.len() == fb.len()
                    && fa
                        .iter()
                        .zip(&fb)
                        .all(|((na, ta), (nb, tb))| na == nb && self.structural_eq(*ta, *tb))
            }
            // Nominal identity (TI-5): an enum's identity is its name. Two
            // distinctly-interned `Tag::Enum` entries denote the same nominal
            // type iff they carry the same name. Generic instantiations are
            // `Tag::Applied`, handled above; a bare `Tag::Enum` is the nominal
            // declaration.
            Tag::Enum => self.enum_name(a) == self.enum_name(b),

            Tag::Named => self.named_name(a) == self.named_name(b),
            Tag::Applied => {
                self.applied_name(a) == self.applied_name(b)
                    && self.structural_eq_slices(&self.applied_args(a), &self.applied_args(b))
            }

            Tag::Scheme => {
                self.scheme_vars(a) == self.scheme_vars(b)
                    && self.structural_eq(self.scheme_body(a), self.scheme_body(b))
            }

            // Type variables / remaining specials: compare `data` directly.
            // (A `Tag::Var` reaching here is a PC-2 phase-contract violation;
            // comparing `data` is the conservative answer.)
            _ => self.data(a) == self.data(b),
        }
    }

    /// Structural equality over two parallel `Idx` slices.
    fn structural_eq_slices(&self, sa: &[Idx], sb: &[Idx]) -> bool {
        sa.len() == sb.len() && sa.iter().zip(sb).all(|(a, b)| self.structural_eq(*a, *b))
    }
}

#[cfg(test)]
mod tests;
