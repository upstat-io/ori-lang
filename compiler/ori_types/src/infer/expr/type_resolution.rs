//! Type resolution — converting `ParsedType` AST nodes into pool `Idx` values.

use ori_ir::{ExprArena, ParsedType, ParsedTypeRange, TypeId};

use super::super::InferEngine;
use crate::Idx;

/// Resolve a `ParsedType` from the AST into a pool `Idx`.
///
/// This converts parsed type annotations into the pool representation.
/// The conversion is recursive for compound types (functions, containers, etc.).
///
/// # Type Mapping
///
/// | `ParsedType` | `Idx` |
/// |--------------|-------|
/// | `Primitive(TypeId::INT)` | `Idx::INT` |
/// | `Primitive(TypeId::UNIT)` | `Idx::UNIT` |
/// | `List(elem)` | `pool.list(resolve(elem))` |
/// | `Function { params, ret }` | `pool.function(...)` |
/// | `Named { name, args }` | lookup or fresh var |
/// | `Infer` | fresh variable |
/// | `SelfType` | fresh variable (TODO: context lookup) |
///
/// # Future Work
///
/// - Named type lookup requires `TypeRegistry` integration (section 07)
/// - `SelfType` requires trait/impl context
/// - `AssociatedType` requires projection support
pub fn resolve_parsed_type(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
) -> Idx {
    match parsed {
        // Primitive Types
        ParsedType::Primitive(type_id) => resolve_type_id(engine, *type_id),

        // Container Types
        ParsedType::List(elem_id) => {
            let elem = arena.get_parsed_type(*elem_id);
            let elem_ty = resolve_parsed_type(engine, arena, elem);
            engine.pool_mut().list(elem_ty)
        }

        ParsedType::FixedList { elem, capacity: _ } => {
            // Fixed lists are treated as regular lists for now
            // TODO: Add fixed list support when needed
            let elem_parsed = arena.get_parsed_type(*elem);
            let elem_ty = resolve_parsed_type(engine, arena, elem_parsed);
            engine.pool_mut().list(elem_ty)
        }

        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_ty = resolve_parsed_type(engine, arena, key_parsed);
            let value_ty = resolve_parsed_type(engine, arena, value_parsed);
            engine.pool_mut().map(key_ty, value_ty)
        }

        // Tuple Types
        ParsedType::Tuple(elems) => {
            if elems.is_empty() {
                Idx::UNIT
            } else {
                let elem_types = resolve_parsed_type_list(engine, arena, *elems);
                engine.pool_mut().tuple(&elem_types)
            }
        }

        // Function Types
        ParsedType::Function { params, ret } => {
            let param_types = resolve_parsed_type_list(engine, arena, *params);
            let ret_parsed = arena.get_parsed_type(*ret);
            let ret_ty = resolve_parsed_type(engine, arena, ret_parsed);
            engine.pool_mut().function(&param_types, ret_ty)
        }

        // Named Types
        ParsedType::Named { name, type_args } => {
            // Resolve type arguments if present
            let resolved_args: Vec<Idx> = if type_args.is_empty() {
                Vec::new()
            } else {
                resolve_parsed_type_list(engine, arena, *type_args)
            };

            // Check for well-known generic types that have dedicated Pool tags.
            // Must use the correct Pool constructors to match types created during inference.
            if !resolved_args.is_empty() {
                if let Some(name_str) = engine.lookup_name(*name) {
                    match (name_str, resolved_args.len()) {
                        ("Option", 1) => return engine.pool_mut().option(resolved_args[0]),
                        ("Result", 2) => {
                            return engine.pool_mut().result(resolved_args[0], resolved_args[1]);
                        }
                        ("Set", 1) => return engine.pool_mut().set(resolved_args[0]),
                        ("Channel" | "Chan", 1) => {
                            return engine.pool_mut().channel(resolved_args[0]);
                        }
                        ("Range", 1) => return engine.pool_mut().range(resolved_args[0]),
                        _ => {
                            // User-defined generic: Applied type
                            return engine.pool_mut().applied(*name, &resolved_args);
                        }
                    }
                }
                // No interner — create Applied type with name and args
                return engine.pool_mut().applied(*name, &resolved_args);
            }

            // No type args — check for builtin primitive names
            if let Some(name_str) = engine.lookup_name(*name) {
                match name_str {
                    "int" => return Idx::INT,
                    "float" => return Idx::FLOAT,
                    "bool" => return Idx::BOOL,
                    "str" => return Idx::STR,
                    "char" => return Idx::CHAR,
                    "byte" => return Idx::BYTE,
                    "void" | "()" => return Idx::UNIT,
                    "never" | "Never" => return Idx::NEVER,
                    "duration" => return Idx::DURATION,
                    "size" => return Idx::SIZE,
                    "ordering" | "Ordering" => return Idx::ORDERING,
                    _ => {}
                }
            }

            // Check if it's a known user-defined type in the TypeRegistry
            if let Some(registry) = engine.type_registry() {
                if registry.get_by_name(*name).is_some() {
                    return engine.pool_mut().named(*name);
                }
            }

            // Check if it's bound in the current environment (type parameter or local)
            if let Some(ty) = engine.env().lookup(*name) {
                return engine.instantiate(ty);
            }

            // Unknown type — create a named var for inference
            engine.fresh_named_var(*name)
        }

        // Inference Markers
        // Infer and ConstExpr both produce fresh variables (const eval not yet implemented).
        // Note: registration (check/registration.rs) uses Idx::ERROR for ConstExpr because
        // registration needs deterministic types. Inference can defer via fresh vars.
        ParsedType::Infer | ParsedType::ConstExpr(_) => engine.fresh_var(),

        ParsedType::SelfType => engine
            .impl_self_type()
            .unwrap_or_else(|| engine.fresh_var()),

        ParsedType::AssociatedType { base, assoc_name } => {
            let base_parsed = arena.get_parsed_type(*base);
            let base_ty = resolve_parsed_type(engine, arena, base_parsed);
            let resolved_base = engine.resolve(base_ty);

            // Search trait impls for the associated type
            if let Some(trait_registry) = engine.trait_registry() {
                for impl_entry in trait_registry.impls_for_type(resolved_base) {
                    if let Some(&assoc_ty) = impl_entry.assoc_types.get(assoc_name) {
                        return assoc_ty;
                    }
                }
            }

            // Not found — return fresh variable for deferred resolution
            engine.fresh_var()
        }

        ParsedType::TraitBounds(bounds) => {
            // Bounded trait object: Printable + Hashable
            // Resolve the first bound as the primary type for now;
            // full trait object dispatch will refine this later.
            let bound_ids = arena.get_parsed_type_list(*bounds);
            if let Some(&first_id) = bound_ids.first() {
                let first = arena.get_parsed_type(first_id);
                resolve_parsed_type(engine, arena, first)
            } else {
                engine.fresh_var()
            }
        }
    }
}

/// Resolve a list of parsed types into a vector of pool indices.
pub(crate) fn resolve_parsed_type_list(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    range: ParsedTypeRange,
) -> Vec<Idx> {
    let ids = arena.get_parsed_type_list(range);
    ids.iter()
        .map(|id| {
            let parsed = arena.get_parsed_type(*id);
            resolve_parsed_type(engine, arena, parsed)
        })
        .collect()
}

/// Resolve a `TypeId` primitive to an `Idx`.
///
/// Handles the mapping between `TypeId` constants (from `ori_ir`) and `Idx` constants.
///
/// # `TypeId` Overlap
///
/// `TypeId` and `Idx` now share the same index layout for primitives (0-11),
/// so this is an identity mapping. INFER (12) and `SELF_TYPE` (13) are markers
/// that become fresh inference variables.
pub(crate) fn resolve_type_id(engine: &mut InferEngine<'_>, type_id: TypeId) -> Idx {
    let raw = type_id.raw();
    if raw < TypeId::PRIMITIVE_COUNT {
        // Primitives 0-11 map by identity (TypeId and Idx share the same layout)
        Idx::from_raw(raw)
    } else {
        // INFER (12), SELF_TYPE (13), or unknown — create a fresh variable
        engine.fresh_var()
    }
}
