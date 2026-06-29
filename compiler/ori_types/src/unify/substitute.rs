//! Type instantiation and substitution.
//!
//! Handles instantiation of type schemes (replacing generalized variables
//! with fresh unbound variables) and general type variable substitution.

use rustc_hash::FxHashMap;

use crate::{Idx, Tag, TypeFlags, VarState};

use super::UnifyEngine;

impl UnifyEngine<'_> {
    /// Instantiate a type scheme with fresh variables.
    ///
    /// For each quantified variable in the scheme, creates a fresh unbound
    /// variable at the current rank, then substitutes throughout the body.
    ///
    /// Returns the type unchanged if it's not a scheme.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Given scheme: ∀a. a -> a
    /// let concrete = engine.instantiate(scheme);  // $1 -> $1 (fresh var)
    /// engine.unify(concrete_param, Idx::INT);     // $1 unified with int
    /// // Now concrete is: int -> int
    /// ```
    pub fn instantiate(&mut self, scheme_idx: Idx) -> Idx {
        self.instantiate_with_subst(scheme_idx).0
    }

    /// Instantiate a scheme and return both the substituted body AND the
    /// `scheme_var_id → fresh_var_idx` substitution map.
    ///
    /// Callers needing the map (e.g. `check_method_inline_bounds` to find the
    /// post-instantiation Var Idx of each method-level binder by its
    /// declared `var_id`) use this instead of `instantiate`. Non-scheme
    /// inputs return `(scheme_idx, empty_map)`; monomorphic schemes return
    /// `(body, empty_map)`.
    pub fn instantiate_with_subst(&mut self, scheme_idx: Idx) -> (Idx, FxHashMap<u32, Idx>) {
        if self.pool.tag(scheme_idx) != Tag::Scheme {
            return (scheme_idx, FxHashMap::default());
        }

        let vars = self.pool.scheme_vars(scheme_idx).to_vec();
        let body = self.pool.scheme_body(scheme_idx);

        if vars.is_empty() {
            return (body, FxHashMap::default());
        }

        // Create fresh variables for each quantified variable
        let mut subst: FxHashMap<u32, Idx> = FxHashMap::default();
        for var_id in vars {
            let fresh = self.fresh_var();
            subst.insert(var_id, fresh);
        }

        // Substitute in the body
        let new_body = self.substitute(body, &subst);
        (new_body, subst)
    }

    /// Substitute variables according to the given mapping.
    ///
    /// Returns the original type if no substitutions apply.
    fn substitute(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        // Fast path: no variables OR bound vars to substitute. The gate
        // includes HAS_BOUND_VAR — instantiation walks scheme bodies whose
        // leaves are `Tag::BoundVar` (HAS_BOUND_VAR=true, HAS_VAR=false).
        if !self
            .pool
            .flags(ty)
            .intersects(TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR)
        {
            return ty;
        }

        match self.pool.tag(ty) {
            Tag::Var => self.substitute_var(ty, subst),
            Tag::BoundVar => self.substitute_bound_var(ty, subst),

            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => self.substitute_simple_container(ty, subst),

            Tag::Map | Tag::Result | Tag::Borrowed => self.substitute_two_child(ty, subst),

            Tag::Function => self.substitute_function(ty, subst),
            Tag::Tuple => self.substitute_tuple(ty, subst),
            Tag::Applied => self.substitute_applied(ty, subst),
            Tag::Struct => self.substitute_struct(ty, subst),
            Tag::Enum => self.substitute_enum(ty, subst),

            // Schemes have their own bound variables, other types don't contain variables
            _ => ty,
        }
    }

    /// `Tag::Var` substitution. Direct `var_id` match replaces; otherwise follow
    /// a `VarState::Link` chain. `Generalized` / `Unbound` / `Rigid` fall
    /// through and return `ty` unchanged.
    fn substitute_var(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let var_id = self.pool.data(ty);

        if let Some(&replacement) = subst.get(&var_id) {
            return replacement;
        }

        if let VarState::Link { target } = self.pool.var_state(var_id) {
            return self.substitute(*target, subst);
        }

        ty
    }

    /// `Tag::BoundVar` substitution. `data` holds the scheme's declared
    /// `var_id` per; the substitution map provides the
    /// per-call-site fresh var.
    fn substitute_bound_var(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let var_id = self.pool.data(ty);
        if let Some(&replacement) = subst.get(&var_id) {
            return replacement;
        }
        ty
    }

    /// Single-child containers share the same pattern: recurse on the child,
    /// rebuild only when the child changed. Dispatches via tag for the
    /// rebuild constructor.
    fn substitute_simple_container(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let child = Idx::from_raw(self.pool.data(ty));
        let new_child = self.substitute(child, subst);
        if new_child == child {
            return ty;
        }
        match self.pool.tag(ty) {
            Tag::List => self.pool.list(new_child),
            Tag::Option => self.pool.option(new_child),
            Tag::Set => self.pool.set(new_child),
            Tag::Channel => self.pool.channel(new_child),
            Tag::Range => self.pool.range(new_child),
            Tag::Iterator => self.pool.iterator(new_child),
            Tag::DoubleEndedIterator => self.pool.double_ended_iterator(new_child),
            other => unreachable!("substitute_simple_container: unexpected tag {:?}", other),
        }
    }

    /// Two-child containers: `Map`, `Result`, `Borrowed` (lifetime is structural).
    fn substitute_two_child(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        match self.pool.tag(ty) {
            Tag::Map => {
                let key = self.pool.map_key(ty);
                let value = self.pool.map_value(ty);
                let new_key = self.substitute(key, subst);
                let new_value = self.substitute(value, subst);
                if new_key == key && new_value == value {
                    ty
                } else {
                    self.pool.map(new_key, new_value)
                }
            }
            Tag::Result => {
                let ok = self.pool.result_ok(ty);
                let err = self.pool.result_err(ty);
                let new_ok = self.substitute(ok, subst);
                let new_err = self.substitute(err, subst);
                if new_ok == ok && new_err == err {
                    ty
                } else {
                    self.pool.result(new_ok, new_err)
                }
            }
            Tag::Borrowed => {
                let inner = self.pool.borrowed_inner(ty);
                let lt = self.pool.borrowed_lifetime(ty);
                let new_inner = self.substitute(inner, subst);
                if new_inner == inner {
                    ty
                } else {
                    self.pool.borrowed(new_inner, lt)
                }
            }
            other => unreachable!("substitute_two_child: unexpected tag {:?}", other),
        }
    }

    /// Function type: recurse through params, then return. Rebuild if any
    /// child changed.
    fn substitute_function(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let params = self.pool.function_params(ty);
        let ret = self.pool.function_return(ty);

        let mut changed = false;
        let new_params: Vec<Idx> = params
            .iter()
            .map(|&p| {
                let new_p = self.substitute(p, subst);
                if new_p != p {
                    changed = true;
                }
                new_p
            })
            .collect();

        let new_ret = self.substitute(ret, subst);
        if new_ret != ret {
            changed = true;
        }

        if changed {
            self.pool.function(&new_params, new_ret)
        } else {
            ty
        }
    }

    /// Tuple: recurse through elements.
    fn substitute_tuple(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let elems = self.pool.tuple_elems(ty);

        let mut changed = false;
        let new_elems: Vec<Idx> = elems
            .iter()
            .map(|&e| {
                let new_e = self.substitute(e, subst);
                if new_e != e {
                    changed = true;
                }
                new_e
            })
            .collect();

        if changed {
            self.pool.tuple(&new_elems)
        } else {
            ty
        }
    }

    /// Struct: recurse through field types; preserve field names. A scheme-body
    /// composite whose field type is a `Tag::BoundVar`/`Tag::Var` needs its
    /// fields rewritten at instantiation (the pool Struct stores inline field
    /// `Idx`s). Substitutes through `var_subst` ONLY — NO
    /// materialization, NO `&TypeRegistry`: at instantiation the args are fresh
    /// type VARIABLES, so concrete-body materialization is premature (it is the
    /// pool mono-recording path's job where the `Applied` args are concrete).
    fn substitute_struct(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let name = self.pool.struct_name(ty);
        let fields = self.pool.struct_fields(ty);

        let mut changed = false;
        let new_fields: Vec<(ori_ir::Name, Idx)> = fields
            .iter()
            .map(|&(field_name, field_ty)| {
                let new_ty = self.substitute(field_ty, subst);
                if new_ty != field_ty {
                    changed = true;
                }
                (field_name, new_ty)
            })
            .collect();

        if changed {
            self.pool.struct_type(name, &new_fields)
        } else {
            ty
        }
    }

    /// Enum: recurse through variant payload types; preserve variant names. The
    /// `Tag::Enum` twin of [`Self::substitute_struct`]; substitutes through
    /// `var_subst` ONLY (no materialization, no `&TypeRegistry`).
    fn substitute_enum(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let name = self.pool.enum_name(ty);
        let variants = self.pool.enum_variants(ty);

        let mut changed = false;
        let new_variants: Vec<crate::pool::EnumVariant> = variants
            .iter()
            .map(|(variant_name, payloads)| {
                let new_payloads: Vec<Idx> = payloads
                    .iter()
                    .map(|&payload_ty| {
                        let new_ty = self.substitute(payload_ty, subst);
                        if new_ty != payload_ty {
                            changed = true;
                        }
                        new_ty
                    })
                    .collect();
                crate::pool::EnumVariant {
                    name: *variant_name,
                    field_types: new_payloads,
                }
            })
            .collect();

        if changed {
            self.pool.enum_type(name, &new_variants)
        } else {
            ty
        }
    }

    /// Applied generic: recurse through type arguments; preserve the name.
    fn substitute_applied(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        let name = self.pool.applied_name(ty);
        let args = self.pool.applied_args(ty);

        let mut changed = false;
        let new_args: Vec<Idx> = args
            .iter()
            .map(|&a| {
                let new_a = self.substitute(a, subst);
                if new_a != a {
                    changed = true;
                }
                new_a
            })
            .collect();

        if changed {
            self.pool.applied(name, &new_args)
        } else {
            ty
        }
    }
}
