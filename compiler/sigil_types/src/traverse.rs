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

use sigil_ir::{Name, TypeId};
use crate::core::Type;
use crate::data::{TypeData, TypeVar};
use crate::type_interner::TypeInterner;

/// Trait for transforming types via structural recursion.
///
/// Implement this trait to create type transformations. Override specific
/// `fold_*` methods to customize behavior for particular type variants.
/// The default `fold` method dispatches to variant-specific methods.
///
/// # Example
/// ```ignore
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
            Type::Projection { base, trait_name, assoc_name } => {
                self.fold_projection(base, *trait_name, *assoc_name)
            }
            // Leaf types - return as-is
            Type::Int | Type::Float | Type::Bool | Type::Str | Type::Char |
            Type::Byte | Type::Unit | Type::Never | Type::Duration | Type::Size |
            Type::Error => ty.clone(),
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

    /// Fold a list type. Default folds inner type.
    fn fold_list(&mut self, inner: &Type) -> Type {
        Type::List(Box::new(self.fold(inner)))
    }

    /// Fold an option type. Default folds inner type.
    fn fold_option(&mut self, inner: &Type) -> Type {
        Type::Option(Box::new(self.fold(inner)))
    }

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

    /// Fold a set type. Default folds inner type.
    fn fold_set(&mut self, inner: &Type) -> Type {
        Type::Set(Box::new(self.fold(inner)))
    }

    /// Fold a range type. Default folds inner type.
    fn fold_range(&mut self, inner: &Type) -> Type {
        Type::Range(Box::new(self.fold(inner)))
    }

    /// Fold a channel type. Default folds inner type.
    fn fold_channel(&mut self, inner: &Type) -> Type {
        Type::Channel(Box::new(self.fold(inner)))
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
}

/// Trait for visiting types without modification.
///
/// Implement this trait to traverse types and collect information.
/// Override specific `visit_*` methods to customize behavior.
///
/// # Example
/// ```ignore
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
            Type::Projection { base, trait_name, assoc_name } => {
                self.visit_projection(base, *trait_name, *assoc_name)
            }
            // Leaf types - no-op by default
            Type::Int | Type::Float | Type::Bool | Type::Str | Type::Char |
            Type::Byte | Type::Unit | Type::Never | Type::Duration | Type::Size |
            Type::Error => {}
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

    /// Visit a list type. Default visits inner type.
    fn visit_list(&mut self, inner: &Type) {
        self.visit(inner);
    }

    /// Visit an option type. Default visits inner type.
    fn visit_option(&mut self, inner: &Type) {
        self.visit(inner);
    }

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

    /// Visit a set type. Default visits inner type.
    fn visit_set(&mut self, inner: &Type) {
        self.visit(inner);
    }

    /// Visit a range type. Default visits inner type.
    fn visit_range(&mut self, inner: &Type) {
        self.visit(inner);
    }

    /// Visit a channel type. Default visits inner type.
    fn visit_channel(&mut self, inner: &Type) {
        self.visit(inner);
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
}

/// Trait for transforming interned types via structural recursion.
///
/// Similar to `TypeFolder`, but works with `TypeId` for O(1) equality.
/// Implementations must provide access to a `TypeInterner` for lookups
/// and interning new types.
///
/// # Example
/// ```ignore
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

    /// Fold a TypeId by dispatching to variant-specific methods.
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
            TypeData::Projection { base, trait_name, assoc_name } => {
                self.fold_projection(base, trait_name, assoc_name)
            }
            // Leaf types - return as-is
            TypeData::Int | TypeData::Float | TypeData::Bool | TypeData::Str |
            TypeData::Char | TypeData::Byte | TypeData::Unit | TypeData::Never |
            TypeData::Duration | TypeData::Size | TypeData::Error => id,
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

    /// Fold a list type. Default folds inner type.
    fn fold_list(&mut self, inner: TypeId) -> TypeId {
        let folded = self.fold(inner);
        self.interner().list(folded)
    }

    /// Fold an option type. Default folds inner type.
    fn fold_option(&mut self, inner: TypeId) -> TypeId {
        let folded = self.fold(inner);
        self.interner().option(folded)
    }

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

    /// Fold a set type. Default folds inner type.
    fn fold_set(&mut self, inner: TypeId) -> TypeId {
        let folded = self.fold(inner);
        self.interner().set(folded)
    }

    /// Fold a range type. Default folds inner type.
    fn fold_range(&mut self, inner: TypeId) -> TypeId {
        let folded = self.fold(inner);
        self.interner().range(folded)
    }

    /// Fold a channel type. Default folds inner type.
    fn fold_channel(&mut self, inner: TypeId) -> TypeId {
        let folded = self.fold(inner);
        self.interner().channel(folded)
    }

    /// Fold an applied generic type. Default folds args.
    fn fold_applied(&mut self, name: Name, args: &[TypeId]) -> TypeId {
        let folded_args: Vec<TypeId> = args.iter().map(|&a| self.fold(a)).collect();
        self.interner().applied(name, folded_args)
    }

    /// Fold a projection type. Default folds base type.
    fn fold_projection(&mut self, base: TypeId, trait_name: Name, assoc_name: Name) -> TypeId {
        let folded_base = self.fold(base);
        self.interner().projection(folded_base, trait_name, assoc_name)
    }
}

/// Trait for visiting interned types without modification.
///
/// Similar to `TypeVisitor`, but works with `TypeId` for efficiency.
/// Requires access to a `TypeInterner` for looking up type data.
///
/// # Example
/// ```ignore
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

    /// Visit a TypeId by dispatching to variant-specific methods.
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
            TypeData::Projection { base, trait_name, assoc_name } => {
                self.visit_projection(base, trait_name, assoc_name)
            }
            // Leaf types - no-op by default
            TypeData::Int | TypeData::Float | TypeData::Bool | TypeData::Str |
            TypeData::Char | TypeData::Byte | TypeData::Unit | TypeData::Never |
            TypeData::Duration | TypeData::Size | TypeData::Error => {}
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

    /// Visit a list type. Default visits inner type.
    fn visit_list(&mut self, inner: TypeId) {
        self.visit(inner);
    }

    /// Visit an option type. Default visits inner type.
    fn visit_option(&mut self, inner: TypeId) {
        self.visit(inner);
    }

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

    /// Visit a set type. Default visits inner type.
    fn visit_set(&mut self, inner: TypeId) {
        self.visit(inner);
    }

    /// Visit a range type. Default visits inner type.
    fn visit_range(&mut self, inner: TypeId) {
        self.visit(inner);
    }

    /// Visit a channel type. Default visits inner type.
    fn visit_channel(&mut self, inner: TypeId) {
        self.visit(inner);
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
}
