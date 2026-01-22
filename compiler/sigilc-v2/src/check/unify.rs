//! Type unification for type inference.
//!
//! Unification finds substitutions that make two types equal.
//! It handles type variables, structural types, and generics.

use crate::intern::{TypeId, TypeInterner, TypeKind};
use rustc_hash::FxHashMap;

/// Error during unification.
#[derive(Clone, Debug)]
pub enum UnifyError {
    /// Types are incompatible.
    Mismatch { left: TypeId, right: TypeId },
    /// Occurs check failed (infinite type).
    OccursCheck { var: TypeId, ty: TypeId },
    /// Arity mismatch (different number of type args).
    ArityMismatch { expected: usize, found: usize },
}

/// Type unifier with substitution tracking.
pub struct Unifier<'a> {
    /// Type interner for creating and looking up types.
    types: &'a TypeInterner,
    /// Substitutions: type variable -> concrete type.
    substitutions: FxHashMap<TypeId, TypeId>,
    /// Counter for fresh type variables.
    next_var: u32,
}

impl<'a> Unifier<'a> {
    /// Create a new unifier.
    pub fn new(types: &'a TypeInterner) -> Self {
        Unifier {
            types,
            substitutions: FxHashMap::default(),
            next_var: 0,
        }
    }

    /// Create a fresh type variable.
    pub fn fresh_var(&mut self) -> TypeId {
        let var = self.next_var;
        self.next_var += 1;
        self.types.intern(TypeKind::Infer(var))
    }

    /// Unify two types, updating substitutions.
    pub fn unify(&mut self, left: TypeId, right: TypeId) -> Result<TypeId, UnifyError> {
        let left = self.resolve(left);
        let right = self.resolve(right);

        // Same type - trivially unifiable
        if left == right {
            return Ok(left);
        }

        // Handle primitives
        if left.is_primitive() && right.is_primitive() {
            return Err(UnifyError::Mismatch { left, right });
        }

        // Handle inference variables
        let left_kind = self.types.lookup(left);
        let right_kind = self.types.lookup(right);

        match (left_kind, right_kind) {
            // Left is inference variable
            (Some(TypeKind::Infer(_)), _) => {
                self.bind(left, right)?;
                Ok(right)
            }

            // Right is inference variable
            (_, Some(TypeKind::Infer(_))) => {
                self.bind(right, left)?;
                Ok(left)
            }

            // Both are concrete - try structural unification
            (Some(left_kind), Some(right_kind)) => {
                self.unify_kinds(&left_kind, &right_kind, left, right)
            }

            // One side is primitive (not in interner), other is compound
            _ => Err(UnifyError::Mismatch { left, right }),
        }
    }

    /// Unify two type kinds structurally.
    fn unify_kinds(
        &mut self,
        left_kind: &TypeKind,
        right_kind: &TypeKind,
        left: TypeId,
        right: TypeId,
    ) -> Result<TypeId, UnifyError> {
        match (left_kind, right_kind) {
            // List types
            (TypeKind::List(l_elem), TypeKind::List(r_elem)) => {
                let elem = self.unify(*l_elem, *r_elem)?;
                Ok(self.types.intern_list(elem))
            }

            // Option types
            (TypeKind::Option(l_inner), TypeKind::Option(r_inner)) => {
                let inner = self.unify(*l_inner, *r_inner)?;
                Ok(self.types.intern_option(inner))
            }

            // Result types
            (
                TypeKind::Result { ok: l_ok, err: l_err },
                TypeKind::Result { ok: r_ok, err: r_err },
            ) => {
                let ok = self.unify(*l_ok, *r_ok)?;
                let err = self.unify(*l_err, *r_err)?;
                Ok(self.types.intern_result(ok, err))
            }

            // Map types
            (
                TypeKind::Map { key: l_key, value: l_value },
                TypeKind::Map { key: r_key, value: r_value },
            ) => {
                let key = self.unify(*l_key, *r_key)?;
                let value = self.unify(*l_value, *r_value)?;
                Ok(self.types.intern_map(key, value))
            }

            // Function types
            (
                TypeKind::Function { params: l_params, ret: l_ret },
                TypeKind::Function { params: r_params, ret: r_ret },
            ) => {
                let l_param_types = self.types.get_list(*l_params);
                let r_param_types = self.types.get_list(*r_params);

                if l_param_types.len() != r_param_types.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: l_param_types.len(),
                        found: r_param_types.len(),
                    });
                }

                let mut unified_params = Vec::with_capacity(l_param_types.len());
                for (l, r) in l_param_types.iter().zip(r_param_types.iter()) {
                    unified_params.push(self.unify(*l, *r)?);
                }

                let ret = self.unify(*l_ret, *r_ret)?;
                Ok(self.types.intern_function(&unified_params, ret))
            }

            // Tuple types
            (TypeKind::Tuple(l_elems), TypeKind::Tuple(r_elems)) => {
                let l_elem_types = self.types.get_list(*l_elems);
                let r_elem_types = self.types.get_list(*r_elems);

                if l_elem_types.len() != r_elem_types.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: l_elem_types.len(),
                        found: r_elem_types.len(),
                    });
                }

                let mut unified_elems = Vec::with_capacity(l_elem_types.len());
                for (l, r) in l_elem_types.iter().zip(r_elem_types.iter()) {
                    unified_elems.push(self.unify(*l, *r)?);
                }

                Ok(self.types.intern_tuple(&unified_elems))
            }

            // Named types with same name
            (
                TypeKind::Named { name: l_name, type_args: l_args },
                TypeKind::Named { name: r_name, type_args: r_args },
            ) if l_name == r_name => {
                let l_arg_types = self.types.get_list(*l_args);
                let r_arg_types = self.types.get_list(*r_args);

                if l_arg_types.len() != r_arg_types.len() {
                    return Err(UnifyError::ArityMismatch {
                        expected: l_arg_types.len(),
                        found: r_arg_types.len(),
                    });
                }

                let mut unified_args = Vec::with_capacity(l_arg_types.len());
                for (l, r) in l_arg_types.iter().zip(r_arg_types.iter()) {
                    unified_args.push(self.unify(*l, *r)?);
                }

                let args_range = self.types.alloc_list(unified_args);
                Ok(self.types.intern(TypeKind::Named {
                    name: *l_name,
                    type_args: args_range,
                }))
            }

            // Error type unifies with anything (for error recovery)
            (TypeKind::Error, _) => Ok(right),
            (_, TypeKind::Error) => Ok(left),

            // Everything else is a mismatch
            _ => Err(UnifyError::Mismatch { left, right }),
        }
    }

    /// Bind a type variable to a type.
    fn bind(&mut self, var: TypeId, ty: TypeId) -> Result<(), UnifyError> {
        // Occurs check: prevent infinite types
        if self.occurs(var, ty) {
            return Err(UnifyError::OccursCheck { var, ty });
        }

        self.substitutions.insert(var, ty);
        Ok(())
    }

    /// Check if a type variable occurs in a type (for occurs check).
    fn occurs(&self, var: TypeId, ty: TypeId) -> bool {
        let ty = self.resolve(ty);

        if var == ty {
            return true;
        }

        if let Some(kind) = self.types.lookup(ty) {
            match kind {
                TypeKind::List(elem) => self.occurs(var, elem),
                TypeKind::Option(inner) => self.occurs(var, inner),
                TypeKind::Result { ok, err } => self.occurs(var, ok) || self.occurs(var, err),
                TypeKind::Map { key, value } => self.occurs(var, key) || self.occurs(var, value),
                TypeKind::Function { params, ret } => {
                    let param_types = self.types.get_list(params);
                    param_types.iter().any(|p| self.occurs(var, *p)) || self.occurs(var, ret)
                }
                TypeKind::Tuple(elems) => {
                    let elem_types = self.types.get_list(elems);
                    elem_types.iter().any(|e| self.occurs(var, *e))
                }
                TypeKind::Named { type_args, .. } => {
                    let arg_types = self.types.get_list(type_args);
                    arg_types.iter().any(|a| self.occurs(var, *a))
                }
                _ => false,
            }
        } else {
            false
        }
    }

    /// Resolve a type through substitutions.
    pub fn resolve(&self, ty: TypeId) -> TypeId {
        let mut current = ty;
        while let Some(&subst) = self.substitutions.get(&current) {
            current = subst;
        }
        current
    }

    /// Apply all substitutions to a type, returning the fully resolved type.
    pub fn apply(&self, ty: TypeId) -> TypeId {
        let ty = self.resolve(ty);

        if ty.is_primitive() {
            return ty;
        }

        if let Some(kind) = self.types.lookup(ty) {
            match kind {
                TypeKind::List(elem) => {
                    let elem = self.apply(elem);
                    self.types.intern_list(elem)
                }
                TypeKind::Option(inner) => {
                    let inner = self.apply(inner);
                    self.types.intern_option(inner)
                }
                TypeKind::Result { ok, err } => {
                    let ok = self.apply(ok);
                    let err = self.apply(err);
                    self.types.intern_result(ok, err)
                }
                TypeKind::Map { key, value } => {
                    let key = self.apply(key);
                    let value = self.apply(value);
                    self.types.intern_map(key, value)
                }
                TypeKind::Function { params, ret } => {
                    let param_types = self.types.get_list(params);
                    let applied_params: Vec<_> = param_types.iter().map(|p| self.apply(*p)).collect();
                    let ret = self.apply(ret);
                    self.types.intern_function(&applied_params, ret)
                }
                TypeKind::Tuple(elems) => {
                    let elem_types = self.types.get_list(elems);
                    let applied_elems: Vec<_> = elem_types.iter().map(|e| self.apply(*e)).collect();
                    self.types.intern_tuple(&applied_elems)
                }
                TypeKind::Infer(_) => ty, // Unresolved variable
                _ => ty,
            }
        } else {
            ty
        }
    }

    /// Get all substitutions.
    pub fn substitutions(&self) -> &FxHashMap<TypeId, TypeId> {
        &self.substitutions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unify_same() {
        let types = TypeInterner::new();
        let mut unifier = Unifier::new(&types);

        assert_eq!(unifier.unify(TypeId::INT, TypeId::INT).unwrap(), TypeId::INT);
        assert_eq!(unifier.unify(TypeId::STR, TypeId::STR).unwrap(), TypeId::STR);
    }

    #[test]
    fn test_unify_mismatch() {
        let types = TypeInterner::new();
        let mut unifier = Unifier::new(&types);

        assert!(unifier.unify(TypeId::INT, TypeId::STR).is_err());
    }

    #[test]
    fn test_unify_variable() {
        let types = TypeInterner::new();
        let mut unifier = Unifier::new(&types);

        let var = unifier.fresh_var();
        assert_eq!(unifier.unify(var, TypeId::INT).unwrap(), TypeId::INT);
        assert_eq!(unifier.resolve(var), TypeId::INT);
    }

    #[test]
    fn test_unify_list() {
        let types = TypeInterner::new();
        let mut unifier = Unifier::new(&types);

        let list_int = types.intern_list(TypeId::INT);
        let list_int2 = types.intern_list(TypeId::INT);
        let list_str = types.intern_list(TypeId::STR);

        // Same element type unifies
        assert!(unifier.unify(list_int, list_int2).is_ok());

        // Different element types don't unify
        let mut unifier2 = Unifier::new(&types);
        assert!(unifier2.unify(list_int, list_str).is_err());
    }

    #[test]
    fn test_unify_list_with_var() {
        let types = TypeInterner::new();
        let mut unifier = Unifier::new(&types);

        let var = unifier.fresh_var();
        let list_var = types.intern_list(var);
        let list_int = types.intern_list(TypeId::INT);

        let result = unifier.unify(list_var, list_int).unwrap();
        assert_eq!(unifier.resolve(var), TypeId::INT);
    }

    #[test]
    fn test_occurs_check() {
        let types = TypeInterner::new();
        let mut unifier = Unifier::new(&types);

        let var = unifier.fresh_var();
        let list_var = types.intern_list(var);

        // Trying to unify var with [var] should fail
        assert!(matches!(
            unifier.unify(var, list_var),
            Err(UnifyError::OccursCheck { .. })
        ));
    }
}
