//! Type resolution for the type checker.
//!
//! Converts parsed type representations (`TypeId`, `ParsedType`) into the
//! type checker's internal `Type` representation. Handles primitive types,
//! well-known generics (Option, Result, Set, Range, Channel), associated
//! type projections, and user-defined named types.

use ori_ir::{Name, ParsedType, ParsedTypeId, ParsedTypeRange, TypeId};
use ori_types::Type;
use rustc_hash::FxHashMap;

use super::TypeChecker;

impl TypeChecker<'_> {
    /// Convert a `TypeId` to a Type.
    ///
    /// `TypeId` is the parsed type annotation representation for primitives.
    /// Type is the type checker's internal representation.
    pub(crate) fn type_id_to_type(&mut self, type_id: TypeId) -> Type {
        match type_id {
            TypeId::INT => Type::Int,
            TypeId::FLOAT => Type::Float,
            TypeId::BOOL => Type::Bool,
            TypeId::STR => Type::Str,
            TypeId::CHAR => Type::Char,
            TypeId::BYTE => Type::Byte,
            TypeId::VOID => Type::Unit,
            TypeId::NEVER => Type::Never,
            TypeId::INFER => self.inference.ctx.fresh_var(),
            _ => {
                // Look up compound types in the type registry
                if let Some(ty) = self.registries.types.to_type(type_id) {
                    ty
                } else {
                    // Unknown compound type - use a fresh var for error recovery
                    self.inference.ctx.fresh_var()
                }
            }
        }
    }

    /// Convert a `ParsedType` to a Type.
    ///
    /// `ParsedType` captures the full structure of type annotations as parsed.
    /// This method resolves them into the type checker's internal representation.
    pub(crate) fn parsed_type_to_type(&mut self, parsed: &ParsedType) -> Type {
        self.resolve_parsed_type_internal(parsed, None)
    }

    /// Resolve a `ParsedType` to a Type, substituting generic type variables.
    ///
    /// This is used when inferring function signatures where type annotations
    /// may refer to generic parameters (e.g., `T` in `@foo<T>(x: T) -> T`).
    pub(crate) fn resolve_parsed_type_with_generics(
        &mut self,
        parsed: &ParsedType,
        generic_type_vars: &FxHashMap<Name, Type>,
    ) -> Type {
        self.resolve_parsed_type_internal(parsed, Some(generic_type_vars))
    }

    /// Internal type resolution with optional generic substitutions.
    ///
    /// Consolidates the logic from `parsed_type_to_type` and `resolve_parsed_type_with_generics`
    /// to eliminate code duplication.
    fn resolve_parsed_type_internal(
        &mut self,
        parsed: &ParsedType,
        generic_type_vars: Option<&FxHashMap<Name, Type>>,
    ) -> Type {
        match parsed {
            ParsedType::Primitive(type_id) => self.type_id_to_type(*type_id),
            ParsedType::Infer => self.inference.ctx.fresh_var(),
            ParsedType::SelfType => {
                // Self type resolution is handled during impl checking.
                self.inference.ctx.fresh_var()
            }
            ParsedType::Named { name, type_args } => {
                // Check if this name refers to a generic parameter (when resolving with generics)
                if type_args.is_empty() {
                    if let Some(vars) = generic_type_vars {
                        if let Some(type_var) = vars.get(name) {
                            return type_var.clone();
                        }
                    }
                }
                // Handle well-known generic types
                self.resolve_well_known_generic_range(*name, *type_args, generic_type_vars)
            }
            ParsedType::List(inner_id) => {
                let elem_ty = self.resolve_parsed_type_id(*inner_id, generic_type_vars);
                Type::List(Box::new(elem_ty))
            }
            ParsedType::Tuple(elems) => {
                let types: Vec<Type> = self.resolve_type_range(*elems, generic_type_vars);
                Type::Tuple(types)
            }
            ParsedType::Function { params, ret } => {
                let param_types: Vec<Type> = self.resolve_type_range(*params, generic_type_vars);
                let ret_ty = self.resolve_parsed_type_id(*ret, generic_type_vars);
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret_ty),
                }
            }
            ParsedType::Map { key, value } => {
                let key_ty = self.resolve_parsed_type_id(*key, generic_type_vars);
                let value_ty = self.resolve_parsed_type_id(*value, generic_type_vars);
                Type::Map {
                    key: Box::new(key_ty),
                    value: Box::new(value_ty),
                }
            }
            ParsedType::AssociatedType { base, assoc_name } => {
                self.make_projection_type_id(*base, *assoc_name, generic_type_vars)
            }
        }
    }

    /// Resolve a parsed type by ID from the arena.
    fn resolve_parsed_type_id(
        &mut self,
        id: ParsedTypeId,
        generic_type_vars: Option<&FxHashMap<Name, Type>>,
    ) -> Type {
        let parsed = self.context.arena.get_parsed_type(id);
        self.resolve_parsed_type_internal(parsed, generic_type_vars)
    }

    /// Resolve a range of parsed types from the arena.
    fn resolve_type_range(
        &mut self,
        range: ParsedTypeRange,
        generic_type_vars: Option<&FxHashMap<Name, Type>>,
    ) -> Vec<Type> {
        let ids = self.context.arena.get_parsed_type_list(range);
        ids.iter()
            .map(|id| self.resolve_parsed_type_id(*id, generic_type_vars))
            .collect()
    }

    /// Resolve a well-known generic type using a type argument range.
    ///
    /// Returns the appropriate Type for known generic types, or a Named type for
    /// user-defined types and type parameters.
    fn resolve_well_known_generic_range(
        &mut self,
        name: Name,
        type_args: ParsedTypeRange,
        generic_type_vars: Option<&FxHashMap<Name, Type>>,
    ) -> Type {
        let name_str = self.context.interner.lookup(name);
        let arg_ids = self.context.arena.get_parsed_type_list(type_args);
        match name_str {
            "Option" => {
                let inner = if arg_ids.len() == 1 {
                    self.resolve_parsed_type_id(arg_ids[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Option(Box::new(inner))
            }
            "Result" => {
                let (ok, err) = if arg_ids.len() == 2 {
                    (
                        self.resolve_parsed_type_id(arg_ids[0], generic_type_vars),
                        self.resolve_parsed_type_id(arg_ids[1], generic_type_vars),
                    )
                } else {
                    (
                        self.inference.ctx.fresh_var(),
                        self.inference.ctx.fresh_var(),
                    )
                };
                Type::Result {
                    ok: Box::new(ok),
                    err: Box::new(err),
                }
            }
            "Set" => {
                let inner = if arg_ids.len() == 1 {
                    self.resolve_parsed_type_id(arg_ids[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Set(Box::new(inner))
            }
            "Range" => {
                let inner = if arg_ids.len() == 1 {
                    self.resolve_parsed_type_id(arg_ids[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Range(Box::new(inner))
            }
            "Channel" => {
                let inner = if arg_ids.len() == 1 {
                    self.resolve_parsed_type_id(arg_ids[0], generic_type_vars)
                } else {
                    self.inference.ctx.fresh_var()
                };
                Type::Channel(Box::new(inner))
            }
            _ => {
                // User-defined type or type parameter
                // Treat as a named type reference - resolution happens during unification
                Type::Named(name)
            }
        }
    }

    /// Create a projection type for an associated type (e.g., Self.Item or T.Item).
    ///
    /// Resolves the base type ID and creates a Projection type.
    fn make_projection_type_id(
        &mut self,
        base_id: ParsedTypeId,
        assoc_name: Name,
        generic_type_vars: Option<&FxHashMap<Name, Type>>,
    ) -> Type {
        // Associated type projection like Self.Item or T.Item
        // The base type is converted, and we create a Projection type.
        // The trait_name is not known at parse time in general; we use
        // a placeholder that will be resolved during impl checking or
        // when we have more context about which trait defines this associated type.
        let base_ty = self.resolve_parsed_type_id(base_id, generic_type_vars);

        // For now, use a placeholder trait name. In a more complete implementation,
        // we would look up which trait defines this associated type based on
        // the context (current trait definition or trait bounds on the base type).
        // Using the assoc_name as the trait_name placeholder for now.
        Type::Projection {
            base: Box::new(base_ty),
            trait_name: assoc_name, // Placeholder - resolved during impl checking
            assoc_name,
        }
    }
}
