//! Type construction helpers for the Pool.
//!
//! Provides ergonomic methods for creating compound types.

use crate::{Idx, LifetimeId, Pool, Rank, Tag, VarState};

/// Variant definition for creating enum types in the Pool.
///
/// Used by `Pool::enum_type()`. Stores variant name and field types
/// (no field names — codegen identifies fields by position).
#[derive(Clone, Debug)]
pub struct EnumVariant {
    /// Variant name (interned).
    pub name: ori_ir::Name,
    /// Field types (empty for unit variants).
    pub field_types: Vec<Idx>,
}

/// Default rank for new variables (same as [`Rank::FIRST`]).
pub const DEFAULT_RANK: Rank = Rank::FIRST;

impl Pool {
    // === Simple Container Constructors ===

    /// Create a list type `[elem]`.
    pub fn list(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::List, elem.raw())
    }

    /// Create an option type `elem?`.
    pub fn option(&mut self, inner: Idx) -> Idx {
        self.intern(Tag::Option, inner.raw())
    }

    /// Create a set type `{elem}`.
    pub fn set(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::Set, elem.raw())
    }

    /// Create a channel type `chan<elem>`.
    pub fn channel(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::Channel, elem.raw())
    }

    /// Create a range type `range<elem>`.
    pub fn range(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::Range, elem.raw())
    }

    /// Create an iterator type `Iterator<elem>`.
    pub fn iterator(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::Iterator, elem.raw())
    }

    /// Create a double-ended iterator type `DoubleEndedIterator<elem>`.
    ///
    /// Subtype of `Iterator<T>` — supports `next_back`, `rev`, `last`, `rfind`, `rfold`
    /// in addition to all Iterator methods.
    pub fn double_ended_iterator(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::DoubleEndedIterator, elem.raw())
    }

    // === Two-Child Container Constructors ===

    /// Create a map type `{key: value}`.
    pub fn map(&mut self, key: Idx, value: Idx) -> Idx {
        self.intern_complex(Tag::Map, &[key.raw(), value.raw()])
    }

    /// Create a result type `result<ok, err>`.
    pub fn result(&mut self, ok: Idx, err: Idx) -> Idx {
        self.intern_complex(Tag::Result, &[ok.raw(), err.raw()])
    }

    // === Borrowed Reference Constructor ===

    /// Create a borrowed reference type `&T` with a lifetime.
    ///
    /// Extra layout: `[inner_idx, lifetime_id]`.
    pub fn borrowed(&mut self, inner: Idx, lifetime: LifetimeId) -> Idx {
        self.intern_complex(Tag::Borrowed, &[inner.raw(), lifetime.raw()])
    }

    // === Function Constructor ===

    /// Create a function type `(params...) -> ret`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "param count fits u32 — pool layout uses u32 words"
    )]
    pub fn function(&mut self, params: &[Idx], ret: Idx) -> Idx {
        // Layout: [param_count, param0, param1, ..., return_type]
        let mut extra = Vec::with_capacity(params.len() + 2);
        extra.push(params.len() as u32);
        for &p in params {
            extra.push(p.raw());
        }
        extra.push(ret.raw());

        self.intern_complex(Tag::Function, &extra)
    }

    /// Create a unary function type `(param) -> ret`.
    pub fn function1(&mut self, param: Idx, ret: Idx) -> Idx {
        self.function(&[param], ret)
    }

    /// Create a binary function type `(p1, p2) -> ret`.
    pub fn function2(&mut self, p1: Idx, p2: Idx, ret: Idx) -> Idx {
        self.function(&[p1, p2], ret)
    }

    /// Create a nullary function type `() -> ret`.
    pub fn function0(&mut self, ret: Idx) -> Idx {
        self.function(&[], ret)
    }

    // === Tuple Constructor ===

    /// Create a tuple type `(elems...)`.
    ///
    /// Empty tuples return `Idx::UNIT`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "element count fits u32 — pool layout uses u32 words"
    )]
    pub fn tuple(&mut self, elems: &[Idx]) -> Idx {
        if elems.is_empty() {
            return Idx::UNIT;
        }

        // Layout: [elem_count, elem0, elem1, ...]
        let mut extra = Vec::with_capacity(elems.len() + 1);
        extra.push(elems.len() as u32);
        for &e in elems {
            extra.push(e.raw());
        }

        self.intern_complex(Tag::Tuple, &extra)
    }

    /// Create a pair type `(a, b)`.
    pub fn pair(&mut self, a: Idx, b: Idx) -> Idx {
        self.tuple(&[a, b])
    }

    /// Create a triple type `(a, b, c)`.
    pub fn triple(&mut self, a: Idx, b: Idx, c: Idx) -> Idx {
        self.tuple(&[a, b, c])
    }

    // === Scheme Constructor ===

    /// Create a type scheme (quantified type).
    ///
    /// Returns the body unchanged if no variables to quantify.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "var count fits u32 — pool layout uses u32 words"
    )]
    pub fn scheme(&mut self, vars: &[u32], body: Idx) -> Idx {
        if vars.is_empty() {
            return body; // Monomorphic, no scheme needed
        }

        // Layout: [var_count, var0, var1, ..., body_type]
        let mut extra = Vec::with_capacity(vars.len() + 2);
        extra.push(vars.len() as u32);
        for &v in vars {
            extra.push(v);
        }
        extra.push(body.raw());

        self.intern_complex(Tag::Scheme, &extra)
    }

    // === Type Variable Constructors ===

    /// Create a fresh unbound type variable.
    pub fn fresh_var(&mut self) -> Idx {
        self.fresh_var_with_rank(DEFAULT_RANK)
    }

    /// Create a fresh unbound type variable at a specific rank.
    pub fn fresh_var_with_rank(&mut self, rank: Rank) -> Idx {
        let id = self.next_var_id;
        self.next_var_id += 1;

        self.var_states.push(VarState::Unbound {
            id,
            rank,
            name: None,
        });

        self.intern(Tag::Var, id)
    }

    /// Create a fresh named type variable (for better error messages).
    pub fn fresh_named_var(&mut self, name: ori_ir::Name) -> Idx {
        self.fresh_named_var_with_rank(name, DEFAULT_RANK)
    }

    /// Create a fresh named type variable at a specific rank.
    pub fn fresh_named_var_with_rank(&mut self, name: ori_ir::Name, rank: Rank) -> Idx {
        let id = self.next_var_id;
        self.next_var_id += 1;

        self.var_states.push(VarState::Unbound {
            id,
            rank,
            name: Some(name),
        });

        self.intern(Tag::Var, id)
    }

    /// Create a rigid type variable (from type annotation).
    pub fn rigid_var(&mut self, name: ori_ir::Name) -> Idx {
        let id = self.next_var_id;
        self.next_var_id += 1;

        self.var_states.push(VarState::Rigid { name });

        self.intern(Tag::RigidVar, id)
    }

    // === Applied Type Constructor ===

    /// Create an applied generic type `T<args...>`.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Name bits and arg count fit u32 — pool layout uses u32 words"
    )]
    pub fn applied(&mut self, name: ori_ir::Name, args: &[Idx]) -> Idx {
        // Layout: [name_u64_lo, name_u64_hi, arg_count, arg0, arg1, ...]
        let name_bits = u64::from(name.raw());
        let mut extra = Vec::with_capacity(args.len() + 3);
        extra.push((name_bits & 0xFFFF_FFFF) as u32);
        extra.push((name_bits >> 32) as u32);
        extra.push(args.len() as u32);
        for &a in args {
            extra.push(a.raw());
        }

        self.intern_complex(Tag::Applied, &extra)
    }

    /// Create a named type reference.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Name u64 split into two u32 halves for pool storage"
    )]
    pub fn named(&mut self, name: ori_ir::Name) -> Idx {
        // Layout: [name_u64_lo, name_u64_hi]
        let name_bits = u64::from(name.raw());
        self.intern_complex(
            Tag::Named,
            &[(name_bits & 0xFFFF_FFFF) as u32, (name_bits >> 32) as u32],
        )
    }

    // === Struct Constructor ===

    /// Create a struct type with named fields.
    ///
    /// Extra layout: `[name_lo, name_hi, field_count, f0_name, f0_type, f1_name, f1_type, ...]`
    ///
    /// The type name is included in the hash to ensure nominal typing —
    /// two structs with the same field layout but different names produce
    /// different `Idx` values.
    #[allow(
        clippy::cast_possible_truncation,
        reason = "name bits and field count fit u32 — pool layout uses u32 words"
    )]
    pub fn struct_type(&mut self, name: ori_ir::Name, fields: &[(ori_ir::Name, Idx)]) -> Idx {
        let name_bits = u64::from(name.raw());
        let mut extra = Vec::with_capacity(3 + fields.len() * 2);
        extra.push((name_bits & 0xFFFF_FFFF) as u32);
        extra.push((name_bits >> 32) as u32);
        extra.push(fields.len() as u32);
        for &(field_name, field_ty) in fields {
            extra.push(field_name.raw());
            extra.push(field_ty.raw());
        }

        self.intern_complex(Tag::Struct, &extra)
    }

    // === Enum Constructor ===

    /// Create an enum type with variants.
    ///
    /// Extra layout: `[name_lo, name_hi, variant_count, v0_name, v0_field_count, v0_f0_type, ..., v1_name, ...]`
    ///
    /// Each variant stores its name and field types (no field names — codegen
    /// doesn't need them per `EnumVariantInfo`).
    #[allow(
        clippy::cast_possible_truncation,
        reason = "name bits and variant count fit u32 — pool layout uses u32 words"
    )]
    pub fn enum_type(&mut self, name: ori_ir::Name, variants: &[EnumVariant]) -> Idx {
        let name_bits = u64::from(name.raw());
        // Pre-compute capacity: 3 (header) + sum(2 + field_count per variant)
        let extra_len: usize = 3 + variants
            .iter()
            .map(|v| 2 + v.field_types.len())
            .sum::<usize>();
        let mut extra = Vec::with_capacity(extra_len);
        extra.push((name_bits & 0xFFFF_FFFF) as u32);
        extra.push((name_bits >> 32) as u32);
        extra.push(variants.len() as u32);
        for variant in variants {
            extra.push(variant.name.raw());
            extra.push(variant.field_types.len() as u32);
            for &field_ty in &variant.field_types {
                extra.push(field_ty.raw());
            }
        }

        self.intern_complex(Tag::Enum, &extra)
    }

    // === Common Patterns ===

    /// Create `[str]` (list of strings).
    pub fn list_str(&mut self) -> Idx {
        self.list(Idx::STR)
    }

    /// Create `[int]` (list of integers).
    pub fn list_int(&mut self) -> Idx {
        self.list(Idx::INT)
    }

    /// Create `int?` (optional integer).
    pub fn option_int(&mut self) -> Idx {
        self.option(Idx::INT)
    }

    /// Create `str?` (optional string).
    pub fn option_str(&mut self) -> Idx {
        self.option(Idx::STR)
    }

    /// Create `result<T, str>` (result with string error).
    pub fn result_str_err(&mut self, ok: Idx) -> Idx {
        self.result(ok, Idx::STR)
    }

    /// Create `{str: T}` (string-keyed map).
    pub fn map_str_key(&mut self, value: Idx) -> Idx {
        self.map(Idx::STR, value)
    }
}

#[cfg(test)]
mod tests;
