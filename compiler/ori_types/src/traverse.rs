//! Type traversal traits.
//!
//! Provides traversal traits for both the boxed `Type` representation and
//! the interned `TypeId` representation:
//!
//! - `TypeFolder` / `TypeVisitor`: Work with boxed `Type` (legacy)
//! - `TypeIdFolder` / `TypeIdVisitor`: Work with interned `TypeId` (preferred)
//!
//! The `TypeId` variants should be preferred for new code as they enable
//! O(1) equality comparisons and better cache locality.

use crate::core::Type;
use crate::data::{TypeData, TypeVar};
use crate::type_interner::TypeInterner;
use ori_ir::{Name, TypeId};

/// Match arm pattern for leaf `Type` variants that are returned as-is (clone).
macro_rules! type_leaf_pattern {
    () => {
        Type::Int
            | Type::Float
            | Type::Bool
            | Type::Str
            | Type::Char
            | Type::Byte
            | Type::Unit
            | Type::Never
            | Type::Duration
            | Type::Size
            | Type::Error
    };
}

/// Match arm pattern for leaf `TypeData` variants that are returned as-is.
macro_rules! type_data_leaf_pattern {
    () => {
        TypeData::Int
            | TypeData::Float
            | TypeData::Bool
            | TypeData::Str
            | TypeData::Char
            | TypeData::Byte
            | TypeData::Unit
            | TypeData::Never
            | TypeData::Duration
            | TypeData::Size
            | TypeData::Error
    };
}

/// Generate fold/visit methods for single-inner-type containers on `Type`-based traits.
///
/// For fold: `fn fold_<name>(&mut self, inner: &Type) -> Type`
/// For visit: `fn visit_<name>(&mut self, inner: &Type)`
macro_rules! impl_single_inner_type_methods {
    (fold: $( ($method:ident, $variant:ident, $doc:literal) ),+ $(,)?) => {
        $(
            #[doc = $doc]
            fn $method(&mut self, inner: &Type) -> Type {
                Type::$variant(Box::new(self.fold(inner)))
            }
        )+
    };
    (visit: $( ($method:ident, $variant:ident, $doc:literal) ),+ $(,)?) => {
        $(
            #[doc = $doc]
            fn $method(&mut self, inner: &Type) {
                self.visit(inner);
            }
        )+
    };
}

/// Generate fold/visit methods for single-inner-type containers on `TypeId`-based traits.
///
/// For fold: `fn fold_<name>(&mut self, inner: TypeId) -> TypeId`
/// For visit: `fn visit_<name>(&mut self, inner: TypeId)`
macro_rules! impl_single_inner_type_id_methods {
    (fold: $( ($method:ident, $interner_method:ident, $doc:literal) ),+ $(,)?) => {
        $(
            #[doc = $doc]
            fn $method(&mut self, inner: TypeId) -> TypeId {
                let folded = self.fold(inner);
                self.interner().$interner_method(folded)
            }
        )+
    };
    (visit: $( ($method:ident, $interner_method:ident, $doc:literal) ),+ $(,)?) => {
        $(
            #[doc = $doc]
            fn $method(&mut self, inner: TypeId) {
                self.visit(inner);
            }
        )+
    };
}

/// Trait for transforming types via structural recursion.
///
/// Implement this trait to create type transformations. Override specific
/// `fold_*` methods to customize behavior for particular type variants.
/// The default `fold` method dispatches to variant-specific methods.
///
/// # Example
///
/// ```text
/// struct Resolver<'a> {
///     substitutions: &'a HashMap<TypeVar, Type>,
/// }
///
/// impl TypeFolder for Resolver<'_> {
///     fn fold_var(&mut self, var: TypeVar) -> Type {
///         if let Some(resolved) = self.substitutions.get(&var) {
///             self.fold(resolved)
///         } else {
///             Type::Var(var)
///         }
///     }
/// }
/// ```
pub trait TypeFolder {
    /// Fold a type by dispatching to variant-specific methods.
    fn fold(&mut self, ty: &Type) -> Type {
        match ty {
            Type::Var(v) => self.fold_var(*v),
            Type::Named(name) => self.fold_named(*name),
            Type::Function { params, ret } => self.fold_function(params, ret),
            Type::Tuple(types) => self.fold_tuple(types),
            Type::List(inner) => self.fold_list(inner),
            Type::Option(inner) => self.fold_option(inner),
            Type::Result { ok, err } => self.fold_result(ok, err),
            Type::Map { key, value } => self.fold_map(key, value),
            Type::Set(inner) => self.fold_set(inner),
            Type::Range(inner) => self.fold_range(inner),
            Type::Channel(inner) => self.fold_channel(inner),
            Type::Applied { name, args } => self.fold_applied(*name, args),
            Type::Projection {
                base,
                trait_name,
                assoc_name,
            } => self.fold_projection(base, *trait_name, *assoc_name),
            Type::ModuleNamespace { items } => self.fold_module_namespace(items),
            type_leaf_pattern!() => ty.clone(),
        }
    }

    /// Fold a type variable. Override to customize variable handling.
    fn fold_var(&mut self, var: TypeVar) -> Type {
        Type::Var(var)
    }

    /// Fold a named type. Override to customize named type handling.
    fn fold_named(&mut self, name: Name) -> Type {
        Type::Named(name)
    }

    /// Fold a function type. Default folds children recursively.
    fn fold_function(&mut self, params: &[Type], ret: &Type) -> Type {
        Type::Function {
            params: params.iter().map(|p| self.fold(p)).collect(),
            ret: Box::new(self.fold(ret)),
        }
    }

    /// Fold a tuple type. Default folds children recursively.
    fn fold_tuple(&mut self, types: &[Type]) -> Type {
        Type::Tuple(types.iter().map(|t| self.fold(t)).collect())
    }

    impl_single_inner_type_methods!(fold:
        (fold_list, List, "Fold a list type. Default folds inner type."),
        (fold_option, Option, "Fold an option type. Default folds inner type."),
        (fold_set, Set, "Fold a set type. Default folds inner type."),
        (fold_range, Range, "Fold a range type. Default folds inner type."),
        (fold_channel, Channel, "Fold a channel type. Default folds inner type."),
    );

    /// Fold a result type. Default folds ok and err types.
    fn fold_result(&mut self, ok: &Type, err: &Type) -> Type {
        Type::Result {
            ok: Box::new(self.fold(ok)),
            err: Box::new(self.fold(err)),
        }
    }

    /// Fold a map type. Default folds key and value types.
    fn fold_map(&mut self, key: &Type, value: &Type) -> Type {
        Type::Map {
            key: Box::new(self.fold(key)),
            value: Box::new(self.fold(value)),
        }
    }

    /// Fold an applied generic type. Default folds args.
    fn fold_applied(&mut self, name: Name, args: &[Type]) -> Type {
        Type::Applied {
            name,
            args: args.iter().map(|a| self.fold(a)).collect(),
        }
    }

    /// Fold a projection type. Default folds base type.
    fn fold_projection(&mut self, base: &Type, trait_name: Name, assoc_name: Name) -> Type {
        Type::Projection {
            base: Box::new(self.fold(base)),
            trait_name,
            assoc_name,
        }
    }

    /// Fold a module namespace type. Default folds item types.
    fn fold_module_namespace(&mut self, items: &[(Name, Type)]) -> Type {
        Type::ModuleNamespace {
            items: items
                .iter()
                .map(|(name, ty)| (*name, self.fold(ty)))
                .collect(),
        }
    }
}

/// Trait for visiting types without modification.
///
/// Implement this trait to traverse types and collect information.
/// Override specific `visit_*` methods to customize behavior.
///
/// # Example
///
/// ```text
/// struct FreeVarCollector {
///     vars: Vec<TypeVar>,
/// }
///
/// impl TypeVisitor for FreeVarCollector {
///     fn visit_var(&mut self, var: TypeVar) {
///         if !self.vars.contains(&var) {
///             self.vars.push(var);
///         }
///     }
/// }
/// ```
pub trait TypeVisitor {
    /// Visit a type by dispatching to variant-specific methods.
    fn visit(&mut self, ty: &Type) {
        match ty {
            Type::Var(v) => self.visit_var(*v),
            Type::Named(name) => self.visit_named(*name),
            Type::Function { params, ret } => self.visit_function(params, ret),
            Type::Tuple(types) => self.visit_tuple(types),
            Type::List(inner) => self.visit_list(inner),
            Type::Option(inner) => self.visit_option(inner),
            Type::Result { ok, err } => self.visit_result(ok, err),
            Type::Map { key, value } => self.visit_map(key, value),
            Type::Set(inner) => self.visit_set(inner),
            Type::Range(inner) => self.visit_range(inner),
            Type::Channel(inner) => self.visit_channel(inner),
            Type::Applied { name, args } => self.visit_applied(*name, args),
            Type::Projection {
                base,
                trait_name,
                assoc_name,
            } => {
                self.visit_projection(base, *trait_name, *assoc_name);
            }
            Type::ModuleNamespace { items } => {
                self.visit_module_namespace(items);
            }
            type_leaf_pattern!() => {}
        }
    }

    /// Visit a type variable. Override to handle variables.
    fn visit_var(&mut self, _var: TypeVar) {}

    /// Visit a named type. Override to handle named types.
    fn visit_named(&mut self, _name: Name) {}

    /// Visit a function type. Default visits children.
    fn visit_function(&mut self, params: &[Type], ret: &Type) {
        for p in params {
            self.visit(p);
        }
        self.visit(ret);
    }

    /// Visit a tuple type. Default visits children.
    fn visit_tuple(&mut self, types: &[Type]) {
        for t in types {
            self.visit(t);
        }
    }

    impl_single_inner_type_methods!(visit:
        (visit_list, List, "Visit a list type. Default visits inner type."),
        (visit_option, Option, "Visit an option type. Default visits inner type."),
        (visit_set, Set, "Visit a set type. Default visits inner type."),
        (visit_range, Range, "Visit a range type. Default visits inner type."),
        (visit_channel, Channel, "Visit a channel type. Default visits inner type."),
    );

    /// Visit a result type. Default visits ok and err types.
    fn visit_result(&mut self, ok: &Type, err: &Type) {
        self.visit(ok);
        self.visit(err);
    }

    /// Visit a map type. Default visits key and value types.
    fn visit_map(&mut self, key: &Type, value: &Type) {
        self.visit(key);
        self.visit(value);
    }

    /// Visit an applied generic type. Default visits args.
    fn visit_applied(&mut self, _name: Name, args: &[Type]) {
        for a in args {
            self.visit(a);
        }
    }

    /// Visit a projection type. Default visits base type.
    fn visit_projection(&mut self, base: &Type, _trait_name: Name, _assoc_name: Name) {
        self.visit(base);
    }

    /// Visit a module namespace type. Default visits item types.
    fn visit_module_namespace(&mut self, items: &[(Name, Type)]) {
        for (_, ty) in items {
            self.visit(ty);
        }
    }
}

/// Trait for transforming interned types via structural recursion.
///
/// Similar to `TypeFolder`, but works with `TypeId` for O(1) equality.
/// Implementations must provide access to a `TypeInterner` for lookups
/// and interning new types.
///
/// # Example
///
/// ```text
/// struct TypeIdResolver<'a> {
///     interner: &'a TypeInterner,
///     substitutions: &'a HashMap<TypeVar, TypeId>,
/// }
///
/// impl TypeIdFolder for TypeIdResolver<'_> {
///     fn interner(&self) -> &TypeInterner { self.interner }
///
///     fn fold_var(&mut self, var: TypeVar) -> TypeId {
///         if let Some(&resolved) = self.substitutions.get(&var) {
///             self.fold(resolved)
///         } else {
///             self.interner.intern(TypeData::Var(var))
///         }
///     }
/// }
/// ```
pub trait TypeIdFolder {
    /// Get the type interner for lookups and creating new types.
    fn interner(&self) -> &TypeInterner;

    /// Fold a `TypeId` by dispatching to variant-specific methods.
    fn fold(&mut self, id: TypeId) -> TypeId {
        let data = self.interner().lookup(id);
        match data {
            TypeData::Var(v) => self.fold_var(v),
            TypeData::Named(name) => self.fold_named(name),
            TypeData::Function { params, ret } => self.fold_function(&params, ret),
            TypeData::Tuple(types) => self.fold_tuple(&types),
            TypeData::List(inner) => self.fold_list(inner),
            TypeData::Option(inner) => self.fold_option(inner),
            TypeData::Result { ok, err } => self.fold_result(ok, err),
            TypeData::Map { key, value } => self.fold_map(key, value),
            TypeData::Set(inner) => self.fold_set(inner),
            TypeData::Range(inner) => self.fold_range(inner),
            TypeData::Channel(inner) => self.fold_channel(inner),
            TypeData::Applied { name, args } => self.fold_applied(name, &args),
            TypeData::Projection {
                base,
                trait_name,
                assoc_name,
            } => self.fold_projection(base, trait_name, assoc_name),
            TypeData::ModuleNamespace { items } => self.fold_module_namespace(&items),
            type_data_leaf_pattern!() => id,
        }
    }

    /// Fold a type variable. Override to customize variable handling.
    fn fold_var(&mut self, var: TypeVar) -> TypeId {
        self.interner().intern(TypeData::Var(var))
    }

    /// Fold a named type. Override to customize named type handling.
    fn fold_named(&mut self, name: Name) -> TypeId {
        self.interner().named(name)
    }

    /// Fold a function type. Default folds children recursively.
    fn fold_function(&mut self, params: &[TypeId], ret: TypeId) -> TypeId {
        let folded_params: Vec<TypeId> = params.iter().map(|&p| self.fold(p)).collect();
        let folded_ret = self.fold(ret);
        self.interner().function(folded_params, folded_ret)
    }

    /// Fold a tuple type. Default folds children recursively.
    fn fold_tuple(&mut self, types: &[TypeId]) -> TypeId {
        let folded: Vec<TypeId> = types.iter().map(|&t| self.fold(t)).collect();
        self.interner().tuple(folded)
    }

    impl_single_inner_type_id_methods!(fold:
        (fold_list, list, "Fold a list type. Default folds inner type."),
        (fold_option, option, "Fold an option type. Default folds inner type."),
        (fold_set, set, "Fold a set type. Default folds inner type."),
        (fold_range, range, "Fold a range type. Default folds inner type."),
        (fold_channel, channel, "Fold a channel type. Default folds inner type."),
    );

    /// Fold a result type. Default folds ok and err types.
    fn fold_result(&mut self, ok: TypeId, err: TypeId) -> TypeId {
        let folded_ok = self.fold(ok);
        let folded_err = self.fold(err);
        self.interner().result(folded_ok, folded_err)
    }

    /// Fold a map type. Default folds key and value types.
    fn fold_map(&mut self, key: TypeId, value: TypeId) -> TypeId {
        let folded_key = self.fold(key);
        let folded_value = self.fold(value);
        self.interner().map(folded_key, folded_value)
    }

    /// Fold an applied generic type. Default folds args.
    fn fold_applied(&mut self, name: Name, args: &[TypeId]) -> TypeId {
        let folded_args: Vec<TypeId> = args.iter().map(|&a| self.fold(a)).collect();
        self.interner().applied(name, folded_args)
    }

    /// Fold a projection type. Default folds base type.
    fn fold_projection(&mut self, base: TypeId, trait_name: Name, assoc_name: Name) -> TypeId {
        let folded_base = self.fold(base);
        self.interner()
            .projection(folded_base, trait_name, assoc_name)
    }

    /// Fold a module namespace type. Default folds item types.
    fn fold_module_namespace(&mut self, items: &[(Name, TypeId)]) -> TypeId {
        let folded_items: Vec<(Name, TypeId)> = items
            .iter()
            .map(|(name, type_id)| (*name, self.fold(*type_id)))
            .collect();
        self.interner().module_namespace(folded_items)
    }
}

/// Trait for visiting interned types without modification.
///
/// Similar to `TypeVisitor`, but works with `TypeId` for efficiency.
/// Requires access to a `TypeInterner` for looking up type data.
///
/// # Example
///
/// ```text
/// struct FreeVarCollector<'a> {
///     interner: &'a TypeInterner,
///     vars: Vec<TypeVar>,
/// }
///
/// impl TypeIdVisitor for FreeVarCollector<'_> {
///     fn interner(&self) -> &TypeInterner { self.interner }
///
///     fn visit_var(&mut self, var: TypeVar) {
///         if !self.vars.contains(&var) {
///             self.vars.push(var);
///         }
///     }
/// }
/// ```
pub trait TypeIdVisitor {
    /// Get the type interner for lookups.
    fn interner(&self) -> &TypeInterner;

    /// Visit a `TypeId` by dispatching to variant-specific methods.
    fn visit(&mut self, id: TypeId) {
        let data = self.interner().lookup(id);
        match data {
            TypeData::Var(v) => self.visit_var(v),
            TypeData::Named(name) => self.visit_named(name),
            TypeData::Function { params, ret } => self.visit_function(&params, ret),
            TypeData::Tuple(types) => self.visit_tuple(&types),
            TypeData::List(inner) => self.visit_list(inner),
            TypeData::Option(inner) => self.visit_option(inner),
            TypeData::Result { ok, err } => self.visit_result(ok, err),
            TypeData::Map { key, value } => self.visit_map(key, value),
            TypeData::Set(inner) => self.visit_set(inner),
            TypeData::Range(inner) => self.visit_range(inner),
            TypeData::Channel(inner) => self.visit_channel(inner),
            TypeData::Applied { name, args } => self.visit_applied(name, &args),
            TypeData::Projection {
                base,
                trait_name,
                assoc_name,
            } => {
                self.visit_projection(base, trait_name, assoc_name);
            }
            TypeData::ModuleNamespace { items } => {
                self.visit_module_namespace(&items);
            }
            type_data_leaf_pattern!() => {}
        }
    }

    /// Visit a type variable. Override to handle variables.
    fn visit_var(&mut self, _var: TypeVar) {}

    /// Visit a named type. Override to handle named types.
    fn visit_named(&mut self, _name: Name) {}

    /// Visit a function type. Default visits children.
    fn visit_function(&mut self, params: &[TypeId], ret: TypeId) {
        for &p in params {
            self.visit(p);
        }
        self.visit(ret);
    }

    /// Visit a tuple type. Default visits children.
    fn visit_tuple(&mut self, types: &[TypeId]) {
        for &t in types {
            self.visit(t);
        }
    }

    impl_single_inner_type_id_methods!(visit:
        (visit_list, list, "Visit a list type. Default visits inner type."),
        (visit_option, option, "Visit an option type. Default visits inner type."),
        (visit_set, set, "Visit a set type. Default visits inner type."),
        (visit_range, range, "Visit a range type. Default visits inner type."),
        (visit_channel, channel, "Visit a channel type. Default visits inner type."),
    );

    /// Visit a result type. Default visits ok and err types.
    fn visit_result(&mut self, ok: TypeId, err: TypeId) {
        self.visit(ok);
        self.visit(err);
    }

    /// Visit a map type. Default visits key and value types.
    fn visit_map(&mut self, key: TypeId, value: TypeId) {
        self.visit(key);
        self.visit(value);
    }

    /// Visit an applied generic type. Default visits args.
    fn visit_applied(&mut self, _name: Name, args: &[TypeId]) {
        for &a in args {
            self.visit(a);
        }
    }

    /// Visit a projection type. Default visits base type.
    fn visit_projection(&mut self, base: TypeId, _trait_name: Name, _assoc_name: Name) {
        self.visit(base);
    }

    /// Visit a module namespace type. Default visits item types.
    fn visit_module_namespace(&mut self, items: &[(Name, TypeId)]) {
        for (_, type_id) in items {
            self.visit(*type_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_interner::TypeInterner;

    #[test]
    fn test_type_folder_transforms_all_variants() {
        // Identity folder - should preserve all types unchanged
        struct IdentityFolder;
        impl TypeFolder for IdentityFolder {}

        let mut folder = IdentityFolder;

        // Test primitives
        assert_eq!(folder.fold(&Type::Int), Type::Int);
        assert_eq!(folder.fold(&Type::Bool), Type::Bool);
        assert_eq!(folder.fold(&Type::Str), Type::Str);

        // Test containers
        let list = Type::List(Box::new(Type::Int));
        assert_eq!(folder.fold(&list), list);

        let option = Type::Option(Box::new(Type::Str));
        assert_eq!(folder.fold(&option), option);

        // Test function
        let func = Type::Function {
            params: vec![Type::Int, Type::Bool],
            ret: Box::new(Type::Str),
        };
        assert_eq!(folder.fold(&func), func);

        // Test tuple
        let tuple = Type::Tuple(vec![Type::Int, Type::Bool]);
        assert_eq!(folder.fold(&tuple), tuple);
    }

    #[test]
    fn test_type_visitor_visits_all_variants() {
        struct CountingVisitor {
            count: usize,
        }
        impl TypeVisitor for CountingVisitor {
            fn visit_var(&mut self, _var: TypeVar) {
                self.count += 1;
            }
        }

        let mut visitor = CountingVisitor { count: 0 };

        // Var should be visited
        let var = Type::Var(TypeVar::new(0));
        visitor.visit(&var);
        assert_eq!(visitor.count, 1);

        // Nested vars
        let func = Type::Function {
            params: vec![Type::Var(TypeVar::new(1))],
            ret: Box::new(Type::Var(TypeVar::new(2))),
        };
        visitor.visit(&func);
        assert_eq!(visitor.count, 3); // 1 + 2 more vars
    }

    #[test]
    fn test_type_id_folder_with_interner() {
        let interner = TypeInterner::new();

        struct IdentityIdFolder<'a> {
            interner: &'a TypeInterner,
        }

        impl TypeIdFolder for IdentityIdFolder<'_> {
            fn interner(&self) -> &TypeInterner {
                self.interner
            }
        }

        let mut folder = IdentityIdFolder {
            interner: &interner,
        };

        // Test that folding primitives returns the same TypeId
        let int_id = TypeId::INT;
        assert_eq!(folder.fold(int_id), int_id);

        // Test container
        let list_id = interner.list(TypeId::INT);
        assert_eq!(folder.fold(list_id), list_id);
    }

    #[test]
    fn test_custom_fold_var_override() {
        // Folder that replaces all vars with Int
        struct VarToIntFolder;
        impl TypeFolder for VarToIntFolder {
            fn fold_var(&mut self, _var: TypeVar) -> Type {
                Type::Int
            }
        }

        let mut folder = VarToIntFolder;

        let var = Type::Var(TypeVar::new(42));
        assert_eq!(folder.fold(&var), Type::Int);

        // Test in nested context
        let func = Type::Function {
            params: vec![Type::Var(TypeVar::new(0))],
            ret: Box::new(Type::Var(TypeVar::new(1))),
        };
        let expected = Type::Function {
            params: vec![Type::Int],
            ret: Box::new(Type::Int),
        };
        assert_eq!(folder.fold(&func), expected);
    }
}
