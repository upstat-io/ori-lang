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
        if self.pool.tag(scheme_idx) != Tag::Scheme {
            return scheme_idx; // Not a scheme, return as-is
        }

        let vars = self.pool.scheme_vars(scheme_idx).to_vec();
        let body = self.pool.scheme_body(scheme_idx);

        if vars.is_empty() {
            return body; // Monomorphic scheme
        }

        // Create fresh variables for each quantified variable
        let mut subst: FxHashMap<u32, Idx> = FxHashMap::default();
        for var_id in vars {
            let fresh = self.fresh_var();
            subst.insert(var_id, fresh);
        }

        // Substitute in the body
        self.substitute(body, &subst)
    }

    /// Substitute variables according to the given mapping.
    ///
    /// Returns the original type if no substitutions apply.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive Tag dispatch for type variable substitution across all type forms"
    )]
    fn substitute(&mut self, ty: Idx, subst: &FxHashMap<u32, Idx>) -> Idx {
        // Fast path: no variables to substitute
        if !self.pool.flags(ty).contains(TypeFlags::HAS_VAR) {
            return ty;
        }

        match self.pool.tag(ty) {
            Tag::Var => {
                let var_id = self.pool.data(ty);

                // Check if this variable should be substituted
                if let Some(&replacement) = subst.get(&var_id) {
                    return replacement;
                }

                // Follow link if present
                if let VarState::Link { target } = self.pool.var_state(var_id) {
                    return self.substitute(*target, subst);
                }

                // Check for generalized variable
                if let VarState::Generalized { id, .. } = self.pool.var_state(var_id) {
                    if let Some(&replacement) = subst.get(id) {
                        return replacement;
                    }
                }

                ty
            }

            Tag::List => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.list(new_child)
                }
            }

            Tag::Option => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.option(new_child)
                }
            }

            Tag::Set => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.set(new_child)
                }
            }

            Tag::Channel => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.channel(new_child)
                }
            }

            Tag::Range => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.range(new_child)
                }
            }

            Tag::Iterator => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.iterator(new_child)
                }
            }

            Tag::DoubleEndedIterator => {
                let child = Idx::from_raw(self.pool.data(ty));
                let new_child = self.substitute(child, subst);
                if new_child == child {
                    ty
                } else {
                    self.pool.double_ended_iterator(new_child)
                }
            }

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

            Tag::Function => {
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

            Tag::Tuple => {
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

            Tag::Applied => {
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

            // Schemes have their own bound variables, other types don't contain variables
            _ => ty,
        }
    }
}
