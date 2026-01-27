//! Type system for Ori.
//!
//! Per design spec 02-design-principlesmd:
//! - All types have Clone, Eq, Hash for Salsa compatibility
//! - Interned type representations for efficiency
//! - Flat structures for cache locality
//!
//! # Type Interning
//!
//! This crate provides two type representations:
//! - `Type`: The traditional boxed representation for compatibility
//! - `TypeData`/`TypeId`: The interned representation for O(1) equality
//!
//! Use `TypeInterner` to intern types and get `TypeId` handles.

mod context;
mod core;
mod data;
mod env;
mod error;
mod traverse;
mod type_interner;

// Re-export all public types
pub use context::{InferenceContext, TypeContext};
pub use core::{Type, TypeScheme, TypeSchemeId};
pub use env::TypeEnv;
pub use error::TypeError;
pub use traverse::{TypeFolder, TypeIdFolder, TypeIdVisitor, TypeVisitor};

// Type interning exports
pub use data::{TypeData, TypeVar};
pub use type_interner::{SharedTypeInterner, TypeInterner, TypeLookup};

// Size assertions to prevent accidental regressions.
// Type is used throughout type checking and stored in query results.
#[cfg(target_pointer_width = "64")]
mod size_asserts {
    use super::{Type, TypeVar};
    // Type enum: largest variant is Applied with Name (8) + Vec<Type> (24) = 32 bytes + discriminant = 40 bytes
    // Applied variant has: name: Name (u64 = 8) + args: Vec<Type> (24) = 32 bytes
    ori_ir::static_assert_size!(Type, 40);
    // TypeVar is just a u32 wrapper
    ori_ir::static_assert_size!(TypeVar, 4);
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
    use super::*;
    use ori_ir::{SharedInterner, Span};

    #[test]
    fn test_type_primitives() {
        assert!(Type::Int.is_primitive());
        assert!(Type::Float.is_primitive());
        assert!(Type::Bool.is_primitive());
        assert!(!Type::List(Box::new(Type::Int)).is_primitive());
    }

    #[test]
    fn test_type_display() {
        let interner = SharedInterner::default();

        assert_eq!(Type::Int.display(&interner), "int");
        assert_eq!(Type::List(Box::new(Type::Int)).display(&interner), "[int]");
        assert_eq!(
            Type::Function {
                params: vec![Type::Int, Type::Bool],
                ret: Box::new(Type::Str),
            }
            .display(&interner),
            "(int, bool) -> str"
        );
    }

    #[test]
    fn test_type_env_scoping() {
        let interner = SharedInterner::default();
        let x = interner.intern("x");
        let y = interner.intern("y");

        let mut env = TypeEnv::new();
        env.bind(x, Type::Int);

        // x is visible
        assert_eq!(env.lookup(x), Some(Type::Int));
        // y is not visible
        assert_eq!(env.lookup(y), None);

        // Create child scope
        let mut child = env.child();
        child.bind(y, Type::Bool);

        // x is still visible (from parent)
        assert_eq!(child.lookup(x), Some(Type::Int));
        // y is now visible
        assert_eq!(child.lookup(y), Some(Type::Bool));

        // y is not visible in parent
        assert_eq!(env.lookup(y), None);
    }

    #[test]
    fn test_unify_same_types() {
        let mut ctx = InferenceContext::new();

        assert!(ctx.unify(&Type::Int, &Type::Int).is_ok());
        assert!(ctx.unify(&Type::Bool, &Type::Bool).is_ok());
    }

    #[test]
    fn test_unify_different_types() {
        let mut ctx = InferenceContext::new();

        assert!(ctx.unify(&Type::Int, &Type::Bool).is_err());
    }

    #[test]
    fn test_unify_type_var() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        assert!(ctx.unify(&var, &Type::Int).is_ok());
        assert_eq!(ctx.resolve(&var), Type::Int);
    }

    #[test]
    fn test_unify_functions() {
        let mut ctx = InferenceContext::new();

        let f1 = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Bool),
        };
        let f2 = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Bool),
        };

        assert!(ctx.unify(&f1, &f2).is_ok());
    }

    #[test]
    fn test_unify_functions_mismatch() {
        let mut ctx = InferenceContext::new();

        let f1 = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Bool),
        };
        let f2 = Type::Function {
            params: vec![Type::Int, Type::Int],
            ret: Box::new(Type::Bool),
        };

        assert!(matches!(
            ctx.unify(&f1, &f2),
            Err(TypeError::ArgCountMismatch { .. })
        ));
    }

    #[test]
    fn test_unify_lists() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        let list1 = Type::List(Box::new(var.clone()));
        let list2 = Type::List(Box::new(Type::Int));

        assert!(ctx.unify(&list1, &list2).is_ok());
        assert_eq!(ctx.resolve(&var), Type::Int);
    }

    #[test]
    fn test_occurs_check() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        // Try to unify ?0 with [?0] - should fail
        let list = Type::List(Box::new(var.clone()));

        assert!(matches!(
            ctx.unify(&var, &list),
            Err(TypeError::InfiniteType)
        ));
    }

    #[test]
    fn test_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();

        set.insert(Type::Int);
        set.insert(Type::Int); // duplicate
        set.insert(Type::Bool);
        set.insert(Type::List(Box::new(Type::Int)));

        assert_eq!(set.len(), 3);
    }

    #[test]
    fn test_type_error_to_diagnostic() {
        let interner = SharedInterner::default();
        let err = TypeError::TypeMismatch {
            expected: Type::Int,
            found: Type::Bool,
        };

        let diag = err.to_diagnostic(Span::new(0, 10), &interner);
        assert!(diag.message.contains("int"));
        assert!(diag.message.contains("bool"));
    }

    #[test]
    fn test_type_scheme_mono() {
        let scheme = TypeScheme::mono(Type::Int);
        assert!(scheme.is_mono());
        assert!(scheme.vars.is_empty());
        assert_eq!(scheme.ty, Type::Int);
    }

    #[test]
    fn test_type_scheme_poly() {
        let var = TypeVar::new(0);
        let scheme = TypeScheme::poly(
            vec![var],
            Type::Function {
                params: vec![Type::Var(var)],
                ret: Box::new(Type::Var(var)),
            },
        );
        assert!(!scheme.is_mono());
        assert_eq!(scheme.vars.len(), 1);
    }

    #[test]
    fn test_free_vars() {
        let mut ctx = InferenceContext::new();
        let var1 = ctx.fresh_var();
        let var2 = ctx.fresh_var();

        // Simple variable has itself as free var
        let free = ctx.free_vars(&var1);
        assert_eq!(free.len(), 1);

        // Unified variables resolve
        ctx.unify(&var1, &Type::Int).unwrap();
        let free = ctx.free_vars(&var1);
        assert!(free.is_empty()); // Int has no free vars

        // Compound type
        let fn_ty = Type::Function {
            params: vec![var2.clone()],
            ret: Box::new(var2.clone()),
        };
        let free = ctx.free_vars(&fn_ty);
        assert_eq!(free.len(), 1); // Only var2
    }

    #[test]
    fn test_generalize() {
        let mut ctx = InferenceContext::new();
        let var = ctx.fresh_var();

        // Function with free variable
        let fn_ty = Type::Function {
            params: vec![var.clone()],
            ret: Box::new(var.clone()),
        };

        // Generalize with empty environment free vars
        let scheme = ctx.generalize(&fn_ty, &[]);
        assert!(!scheme.is_mono());
        assert_eq!(scheme.vars.len(), 1);
    }

    #[test]
    fn test_generalize_with_env_vars() {
        let mut ctx = InferenceContext::new();
        let var1 = ctx.fresh_var();
        let var2 = ctx.fresh_var();

        // Extract the TypeVars
        let Type::Var(tv1) = var1 else { panic!() };
        let Type::Var(tv2) = var2 else { panic!() };

        // Function using both vars
        let fn_ty = Type::Function {
            params: vec![var1.clone()],
            ret: Box::new(var2.clone()),
        };

        // Generalize but tv1 is in the environment (shouldn't be quantified)
        let scheme = ctx.generalize(&fn_ty, &[tv1]);

        // Only tv2 should be quantified
        assert!(!scheme.is_mono());
        assert_eq!(scheme.vars.len(), 1);
        assert_eq!(scheme.vars[0], tv2);
    }

    #[test]
    fn test_instantiate() {
        let mut ctx = InferenceContext::new();

        // Create a polymorphic identity scheme: ∀a. a -> a
        let var = TypeVar::new(0);
        let scheme = TypeScheme::poly(
            vec![var],
            Type::Function {
                params: vec![Type::Var(var)],
                ret: Box::new(Type::Var(var)),
            },
        );

        // Instantiate twice - should get different fresh variables
        let ty1 = ctx.instantiate(&scheme);
        let ty2 = ctx.instantiate(&scheme);

        // Both should be function types
        assert!(matches!(ty1, Type::Function { .. }));
        assert!(matches!(ty2, Type::Function { .. }));

        // They should have different type variables
        if let (
            Type::Function {
                params: p1,
                ret: r1,
            },
            Type::Function {
                params: p2,
                ret: r2,
            },
        ) = (ty1, ty2)
        {
            // Each instantiation gets fresh variables
            assert_ne!(p1[0], p2[0]);
            // But within each instantiation, param and return should be same var
            assert_eq!(p1[0], *r1);
            assert_eq!(p2[0], *r2);
        } else {
            panic!("Expected function types");
        }
    }

    #[test]
    fn test_instantiate_mono() {
        let mut ctx = InferenceContext::new();
        let scheme = TypeScheme::mono(Type::Int);

        let ty = ctx.instantiate(&scheme);
        assert_eq!(ty, Type::Int);
    }

    #[test]
    fn test_type_context_list_dedup() {
        let mut ctx = TypeContext::new();

        let list1 = ctx.list_type(Type::Int);
        let list2 = ctx.list_type(Type::Int);
        let list3 = ctx.list_type(Type::Bool);

        // Same type args should return equal types
        assert_eq!(list1, list2);
        // Different type args should return different types
        assert_ne!(list1, list3);
    }

    #[test]
    fn test_type_context_option_dedup() {
        let mut ctx = TypeContext::new();

        let opt1 = ctx.option_type(Type::Str);
        let opt2 = ctx.option_type(Type::Str);
        let opt3 = ctx.option_type(Type::Int);

        assert_eq!(opt1, opt2);
        assert_ne!(opt1, opt3);
    }

    #[test]
    fn test_type_context_result_dedup() {
        let mut ctx = TypeContext::new();

        let res1 = ctx.result_type(Type::Int, Type::Str);
        let res2 = ctx.result_type(Type::Int, Type::Str);
        let res3 = ctx.result_type(Type::Bool, Type::Str);

        assert_eq!(res1, res2);
        assert_ne!(res1, res3);
    }

    #[test]
    fn test_type_context_map_dedup() {
        let mut ctx = TypeContext::new();

        let map1 = ctx.map_type(Type::Str, Type::Int);
        let map2 = ctx.map_type(Type::Str, Type::Int);
        let map3 = ctx.map_type(Type::Int, Type::Int);

        assert_eq!(map1, map2);
        assert_ne!(map1, map3);
    }

    #[test]
    fn test_type_context_lookup_insert() {
        let mut ctx = TypeContext::new();

        let var = TypeVar::new(100);
        let origin = TypeScheme::poly(
            vec![var],
            Type::Function {
                params: vec![Type::Var(var)],
                ret: Box::new(Type::Var(var)),
            },
        );
        let targs = vec![Type::Int];
        let instance = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Int),
        };

        // Insert and get back
        let result = ctx.insert(origin.clone(), targs.clone(), instance.clone());
        assert_eq!(result, instance);

        // Lookup should find it
        let found = ctx.lookup(&origin, &targs);
        assert_eq!(found, Some(&instance));

        // Different targs should not find it
        let not_found = ctx.lookup(&origin, &[Type::Bool]);
        assert!(not_found.is_none());
    }
}
