//! Structural type unification and public unification entrypoints.

use crate::{Idx, Tag, TypeFlags};

use super::{ArityKind, UnifyContext, UnifyEngine, UnifyError};

impl UnifyEngine<'_> {
    // Unification

    /// Unify two types, making them equivalent.
    ///
    /// Returns `Ok(())` if unification succeeds.
    /// Returns `Err(UnifyError)` on failure.
    ///
    /// After successful unification, both types will resolve to the same type.
    pub fn unify(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        self.unify_with_context(a, b, UnifyContext::TopLevel)
    }

    /// Unify with explicit context for better error messages.
    pub fn unify_with_context(
        &mut self,
        a: Idx,
        b: Idx,
        context: UnifyContext,
    ) -> Result<(), UnifyError> {
        // Fast path: identical indices
        if a == b {
            return Ok(());
        }

        // Resolve both sides
        let a = self.resolve(a);
        let b = self.resolve(b);

        // After resolution, check again
        if a == b {
            return Ok(());
        }

        // Get flags for early exits
        let a_flags = self.pool.flags(a);
        let b_flags = self.pool.flags(b);

        let a_tag = self.pool.tag(a);
        let b_tag = self.pool.tag(b);

        // Error type propagates (don't report cascading errors). `Error` is a
        // legitimate type (TY-5: Error == Idx::ERROR) sharing the
        // poison slot, so `Result<str, Error>` carries HAS_ERROR. Suppressing the
        // whole unification here leaves variables nested alongside the error type
        // unbound, surfacing a spurious E2005 (e.g. `let r: Result<str, Error> =
        // Ok("x")` then `is_ok(result: r)` — the value leaves the error param free
        // and the annotation must bind it). Bind variables, but keep cascade
        // suppression: a variable on either side binds to the other side; matching
        // compound tags recurse structurally so nested variables bind, with any
        // concrete-vs-concrete mismatch discarded (still no cascading diagnostic).
        if a_flags.contains(TypeFlags::HAS_ERROR) || b_flags.contains(TypeFlags::HAS_ERROR) {
            if a_tag == Tag::Var {
                return self.unify_var_with(a, b, context);
            }
            if b_tag == Tag::Var {
                return self.unify_var_with(b, a, context);
            }
            // Recurse to bind nested variables; discard mismatch diagnostics —
            // any error here is a cascade from the poison/Error slot.
            let _ = self.unify_structural(a, b, context);
            return Ok(());
        }

        // Never type unifies with anything (bottom type)

        if a_tag == Tag::Never || b_tag == Tag::Never {
            return Ok(());
        }

        // Dispatch based on types
        match (a_tag, b_tag) {
            // Variable on left
            (Tag::Var, _) => self.unify_var_with(a, b, context),

            // Variable on right (swap to normalize)
            (_, Tag::Var) => self.unify_var_with(b, a, context),

            // Rigid variables
            (Tag::RigidVar, Tag::RigidVar) => self.unify_rigid_rigid(a, b),
            (Tag::RigidVar, _) => {
                let name = self.get_rigid_name(a);
                Err(UnifyError::RigidMismatch {
                    rigid_name: name,
                    concrete: b,
                })
            }
            (_, Tag::RigidVar) => {
                let name = self.get_rigid_name(b);
                Err(UnifyError::RigidMismatch {
                    rigid_name: name,
                    concrete: a,
                })
            }

            // Structural unification for concrete types
            _ => self.unify_structural(a, b, context),
        }
    }

    // Structural Unification

    /// Unify two concrete (non-variable) types structurally.
    fn unify_structural(
        &mut self,
        a: Idx,
        b: Idx,
        context: UnifyContext,
    ) -> Result<(), UnifyError> {
        let tag_a = self.pool.tag(a);
        let tag_b = self.pool.tag(b);

        // Tags must match (with DoubleEndedIterator ↔ Iterator coercion)
        if tag_a != tag_b {
            // DoubleEndedIterator coerces to Iterator: unify element types
            if tag_a.is_iterator() && tag_b.is_iterator() {
                let child_a = Idx::from_raw(self.pool.data(a));
                let child_b = Idx::from_raw(self.pool.data(b));
                return self.unify_with_context(child_a, child_b, UnifyContext::IteratorElement);
            }
            return Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context,
            });
        }

        match tag_a {
            // Primitives: same tag means equal
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Str
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => Ok(()),

            // Simple containers
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator => self.unify_single_child(a, b, tag_a),

            // Two-child containers
            Tag::Map | Tag::Result => self.unify_pair(a, b, tag_a),

            Tag::Borrowed => {
                let inner_a = self.pool.borrowed_inner(a);
                let inner_b = self.pool.borrowed_inner(b);
                let lt_a = self.pool.borrowed_lifetime(a);
                let lt_b = self.pool.borrowed_lifetime(b);

                if lt_a != lt_b {
                    return Err(UnifyError::Mismatch {
                        expected: a,
                        found: b,
                        context,
                    });
                }
                self.unify_with_context(inner_a, inner_b, UnifyContext::BorrowedInner)
            }

            // Functions
            Tag::Function => self.unify_function(a, b),

            // Tuples
            Tag::Tuple => self.unify_tuple(a, b),

            // Named types: must have same name
            Tag::Named => {
                let name_a = self.pool.named_name(a);
                let name_b = self.pool.named_name(b);

                if name_a == name_b {
                    Ok(())
                } else {
                    Err(UnifyError::Mismatch {
                        expected: a,
                        found: b,
                        context,
                    })
                }
            }

            // Applied types: same name and unify args
            Tag::Applied => self.unify_applied(a, b, context),

            // Other types: just check tag equality
            _ => Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context,
            }),
        }
    }

    fn unify_single_child(&mut self, a: Idx, b: Idx, tag: Tag) -> Result<(), UnifyError> {
        let context = match tag {
            Tag::List => UnifyContext::ListElement,
            Tag::Option => UnifyContext::OptionInner,
            Tag::Set => UnifyContext::SetElement,
            Tag::Channel => UnifyContext::ChannelElement,
            Tag::Range => UnifyContext::RangeElement,
            Tag::Iterator | Tag::DoubleEndedIterator => UnifyContext::IteratorElement,
            _ => unreachable!("single-child unification called for {tag:?}"),
        };
        self.unify_with_context(
            Idx::from_raw(self.pool.data(a)),
            Idx::from_raw(self.pool.data(b)),
            context,
        )
    }

    fn unify_pair(&mut self, a: Idx, b: Idx, tag: Tag) -> Result<(), UnifyError> {
        let (left_a, left_b, left_context, right_a, right_b, right_context) = match tag {
            Tag::Map => (
                self.pool.map_key(a),
                self.pool.map_key(b),
                UnifyContext::MapKey,
                self.pool.map_value(a),
                self.pool.map_value(b),
                UnifyContext::MapValue,
            ),
            Tag::Result => (
                self.pool.result_ok(a),
                self.pool.result_ok(b),
                UnifyContext::ResultOk,
                self.pool.result_err(a),
                self.pool.result_err(b),
                UnifyContext::ResultErr,
            ),
            _ => unreachable!("pair unification called for {tag:?}"),
        };
        self.unify_with_context(left_a, left_b, left_context)?;
        self.unify_with_context(right_a, right_b, right_context)
    }

    fn unify_function(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        let params_a = self.pool.function_params(a);
        let params_b = self.pool.function_params(b);
        if params_a.len() != params_b.len() {
            return Err(UnifyError::ArityMismatch {
                expected: params_a.len(),
                found: params_b.len(),
                kind: ArityKind::Function,
            });
        }
        for (index, (param_a, param_b)) in params_a.iter().zip(&params_b).enumerate() {
            self.unify_with_context(*param_a, *param_b, UnifyContext::param(index))?;
        }
        self.unify_with_context(
            self.pool.function_return(a),
            self.pool.function_return(b),
            UnifyContext::FunctionReturn,
        )
    }

    fn unify_tuple(&mut self, a: Idx, b: Idx) -> Result<(), UnifyError> {
        let elements_a = self.pool.tuple_elems(a);
        let elements_b = self.pool.tuple_elems(b);
        if elements_a.len() != elements_b.len() {
            return Err(UnifyError::ArityMismatch {
                expected: elements_a.len(),
                found: elements_b.len(),
                kind: ArityKind::Tuple,
            });
        }
        for (index, (element_a, element_b)) in elements_a.iter().zip(&elements_b).enumerate() {
            self.unify_with_context(*element_a, *element_b, UnifyContext::tuple_elem(index))?;
        }
        Ok(())
    }

    fn unify_applied(&mut self, a: Idx, b: Idx, context: UnifyContext) -> Result<(), UnifyError> {
        if self.pool.applied_name(a) != self.pool.applied_name(b) {
            return Err(UnifyError::Mismatch {
                expected: a,
                found: b,
                context,
            });
        }
        let args_a = self.pool.applied_args(a);
        let args_b = self.pool.applied_args(b);
        if args_a.len() != args_b.len() {
            return Err(UnifyError::ArityMismatch {
                expected: args_a.len(),
                found: args_b.len(),
                kind: ArityKind::TypeArgs,
            });
        }
        for (index, (arg_a, arg_b)) in args_a.iter().zip(&args_b).enumerate() {
            self.unify_with_context(*arg_a, *arg_b, UnifyContext::type_arg(index))?;
        }
        Ok(())
    }
}
