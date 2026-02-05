//! Type construction helpers for the Pool.
//!
//! Provides ergonomic methods for creating compound types.

use crate::{Idx, Pool, Rank, Tag, VarState};

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

    // === Two-Child Container Constructors ===

    /// Create a map type `{key: value}`.
    pub fn map(&mut self, key: Idx, value: Idx) -> Idx {
        self.intern_complex(Tag::Map, &[key.raw(), value.raw()])
    }

    /// Create a result type `result<ok, err>`.
    pub fn result(&mut self, ok: Idx, err: Idx) -> Idx {
        self.intern_complex(Tag::Result, &[ok.raw(), err.raw()])
    }

    // === Function Constructor ===

    /// Create a function type `(params...) -> ret`.
    #[allow(clippy::cast_possible_truncation)]
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
    #[allow(clippy::cast_possible_truncation)]
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
    #[allow(clippy::cast_possible_truncation)]
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
    #[allow(clippy::cast_possible_truncation)]
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
    #[allow(clippy::cast_possible_truncation)]
    pub fn named(&mut self, name: ori_ir::Name) -> Idx {
        // Layout: [name_u64_lo, name_u64_hi]
        let name_bits = u64::from(name.raw());
        self.intern_complex(
            Tag::Named,
            &[(name_bits & 0xFFFF_FFFF) as u32, (name_bits >> 32) as u32],
        )
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
mod tests {
    use super::*;

    #[test]
    fn list_construction() {
        let mut pool = Pool::new();
        let list_int = pool.list(Idx::INT);

        assert_eq!(pool.tag(list_int), Tag::List);
        assert_eq!(Idx::from_raw(pool.data(list_int)), Idx::INT);
    }

    #[test]
    fn option_construction() {
        let mut pool = Pool::new();
        let opt_str = pool.option(Idx::STR);

        assert_eq!(pool.tag(opt_str), Tag::Option);
        assert_eq!(Idx::from_raw(pool.data(opt_str)), Idx::STR);
    }

    #[test]
    fn map_construction() {
        let mut pool = Pool::new();
        let map_ty = pool.map(Idx::STR, Idx::INT);

        assert_eq!(pool.tag(map_ty), Tag::Map);
        assert_eq!(pool.map_key(map_ty), Idx::STR);
        assert_eq!(pool.map_value(map_ty), Idx::INT);
    }

    #[test]
    fn result_construction() {
        let mut pool = Pool::new();
        let res_ty = pool.result(Idx::INT, Idx::STR);

        assert_eq!(pool.tag(res_ty), Tag::Result);
        assert_eq!(pool.result_ok(res_ty), Idx::INT);
        assert_eq!(pool.result_err(res_ty), Idx::STR);
    }

    #[test]
    fn function_construction() {
        let mut pool = Pool::new();
        let fn_ty = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);

        assert_eq!(pool.tag(fn_ty), Tag::Function);

        let params = pool.function_params(fn_ty);
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], Idx::INT);
        assert_eq!(params[1], Idx::STR);

        assert_eq!(pool.function_return(fn_ty), Idx::BOOL);
    }

    #[test]
    fn tuple_construction() {
        let mut pool = Pool::new();

        // Empty tuple is unit
        let empty = pool.tuple(&[]);
        assert_eq!(empty, Idx::UNIT);

        // Non-empty tuple
        let tuple = pool.tuple(&[Idx::INT, Idx::STR, Idx::BOOL]);
        assert_eq!(pool.tag(tuple), Tag::Tuple);

        let elems = pool.tuple_elems(tuple);
        assert_eq!(elems.len(), 3);
        assert_eq!(elems[0], Idx::INT);
        assert_eq!(elems[1], Idx::STR);
        assert_eq!(elems[2], Idx::BOOL);
    }

    #[test]
    fn fresh_var_construction() {
        let mut pool = Pool::new();

        let var1 = pool.fresh_var();
        let var2 = pool.fresh_var();

        // Should be different variables
        assert_ne!(pool.data(var1), pool.data(var2));

        // Both should be Var type
        assert_eq!(pool.tag(var1), Tag::Var);
        assert_eq!(pool.tag(var2), Tag::Var);

        // Check var states
        match pool.var_state(pool.data(var1)) {
            VarState::Unbound { id, .. } => assert_eq!(*id, pool.data(var1)),
            _ => panic!("Expected Unbound"),
        }
    }

    #[test]
    fn scheme_construction() {
        let mut pool = Pool::new();

        // Monomorphic body returns body unchanged
        let mono = pool.scheme(&[], Idx::INT);
        assert_eq!(mono, Idx::INT);

        // Polymorphic scheme
        let var = pool.fresh_var();
        let var_id = pool.data(var);
        let fn_ty = pool.function(&[var], var);
        let scheme = pool.scheme(&[var_id], fn_ty);

        assert_eq!(pool.tag(scheme), Tag::Scheme);

        let vars = pool.scheme_vars(scheme);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0], var_id);

        assert_eq!(pool.scheme_body(scheme), fn_ty);
    }

    #[test]
    fn type_deduplication() {
        let mut pool = Pool::new();

        let list1 = pool.list(Idx::INT);
        let list2 = pool.list(Idx::INT);

        // Same type should return same index
        assert_eq!(list1, list2);

        // Pool size shouldn't increase
        let size_before = pool.len();
        let list3 = pool.list(Idx::INT);
        assert_eq!(pool.len(), size_before);
        assert_eq!(list1, list3);
    }

    #[test]
    fn nested_type_construction() {
        let mut pool = Pool::new();

        // [[int]]
        let inner = pool.list(Idx::INT);
        let outer = pool.list(inner);

        assert_eq!(pool.tag(outer), Tag::List);
        assert_eq!(Idx::from_raw(pool.data(outer)), inner);
    }

    #[test]
    fn applied_type_accessors() {
        let mut pool = Pool::new();

        // Create a name for testing (using raw value)
        let name = ori_ir::Name::from_raw(42);

        // Create Applied<int, str>
        let applied = pool.applied(name, &[Idx::INT, Idx::STR]);

        assert_eq!(pool.tag(applied), Tag::Applied);
        assert_eq!(pool.applied_name(applied), name);
        assert_eq!(pool.applied_arg_count(applied), 2);
        assert_eq!(pool.applied_arg(applied, 0), Idx::INT);
        assert_eq!(pool.applied_arg(applied, 1), Idx::STR);

        let args = pool.applied_args(applied);
        assert_eq!(args, vec![Idx::INT, Idx::STR]);
    }

    #[test]
    fn applied_type_no_args() {
        let mut pool = Pool::new();

        let name = ori_ir::Name::from_raw(99);
        let applied = pool.applied(name, &[]);

        assert_eq!(pool.applied_name(applied), name);
        assert_eq!(pool.applied_arg_count(applied), 0);
        assert!(pool.applied_args(applied).is_empty());
    }

    #[test]
    fn named_type_accessor() {
        let mut pool = Pool::new();

        let name = ori_ir::Name::from_raw(123);
        let named = pool.named(name);

        assert_eq!(pool.tag(named), Tag::Named);
        assert_eq!(pool.named_name(named), name);
    }
}
