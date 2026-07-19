//! Identifier inference — name resolution, constructors, and related lookups.

use ori_ir::{Name, Span};

use crate::{Idx, TypeCheckError, TypeKind, TypeRegistry, VariantFields};

use super::super::InferEngine;
use super::substitute_type_params_with_map;

/// Constructor info extracted from `TypeRegistry` (avoids borrow conflicts).
pub(crate) enum ConstructorInfo {
    Newtype {
        underlying: Idx,
        type_idx: Idx,
    },
    /// Unit variant (no fields).
    /// For generic enums (e.g., `MyNone` from `MyOption<T>`), we need the type params
    /// to instantiate fresh variables so that `MyNone` becomes `MyOption<$fresh>`.
    UnitVariant {
        enum_idx: Idx,
        enum_name: Name,
        type_params: Vec<Name>,
    },
    /// Tuple variant constructor with field types, base enum idx/name, and type parameter names.
    /// For generic enums (e.g., `MyOk(value: T)` from `MyResult<T, E>`), the field types
    /// may contain `Named(param_name)` indices that need substitution with fresh variables.
    TupleVariant {
        field_types: Vec<Idx>,
        enum_idx: Idx,
        enum_name: Name,
        type_params: Vec<Name>,
    },
}

/// Infer the type of an identifier reference.
pub(crate) fn infer_ident(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    // 1. Environment lookup (functions, parameters, let bindings)
    if let Some(scheme) = engine.env().lookup(name) {
        return engine.instantiate(scheme);
    }

    // 2. Resolve name to string for constructor/builtin matching
    let name_str = engine.lookup_name(name);

    // 2a. Special case for "self" - if not in env, check for recursive self_type
    // This handles `self()` calls inside recursive patterns like `recurse`.
    if name_str == Some("self") {
        if let Some(self_ty) = engine.self_type() {
            return self_ty;
        }
    }

    // Module constructors shadow universe builtins. In particular, a local
    // sum-type variant named `Error` must resolve before the builtin
    // `Error(str) -> Error` constructor below.
    if let Some(ctor) = resolve_variant_constructor_info(engine, name) {
        return infer_constructor(engine, ctor);
    }

    if let Some(s) = name_str {
        // 3. Built-in variant constructors (Option/Result are primitive types)
        match s {
            "Some" => {
                let t = engine.pool_mut().fresh_var();
                let opt_t = engine.pool_mut().option(t);
                return engine.pool_mut().function(&[t], opt_t);
            }
            "None" => {
                let t = engine.pool_mut().fresh_var();
                return engine.pool_mut().option(t);
            }
            "Ok" => {
                let t = engine.pool_mut().fresh_var();
                let e = engine.pool_mut().fresh_var();
                let res = engine.pool_mut().result(t, e);
                return engine.pool_mut().function(&[t], res);
            }
            "Err" => {
                let t = engine.pool_mut().fresh_var();
                let e = engine.pool_mut().fresh_var();
                let res = engine.pool_mut().result(t, e);
                return engine.pool_mut().function(&[e], res);
            }
            _ => {}
        }

        // 4. Prelude free functions (conversion, hash_combine, repeat)
        // Canonical signatures live in ori_registry::PRELUDE_FUNCTIONS.
        if let Some(prelude_fn) = ori_registry::find_prelude_function(s) {
            return prelude_function_to_idx(engine, prelude_fn);
        }

        // 5. Type names used as expression-level receivers for associated functions
        //    e.g., Duration.from_seconds(s: 5), Size.from_bytes(b: 100)
        match s {
            "Duration" | "duration" => return Idx::DURATION,
            "Size" | "size" => return Idx::SIZE,
            "Ordering" | "ordering" => return Idx::ORDERING,
            _ => {}
        }
    }

    // 5.5. Built-in `Error(message)` constructor.
    // MUST precede step 6: the user-facing `Error` is now a registered builtin
    // struct (`{ message: str }`), so step 6's `resolve_type_constructor_info`
    // would catch it as a bare `UnitVariant` type (not callable → "expected a
    // function, found Error"). The `Error(str) -> Error` constructor
    // builds `(str) -> named("Error")`, which resolves to the struct Idx.
    if name_str == Some("Error") {
        let error_type = engine.pool_mut().named(name);
        return engine.pool_mut().function(&[Idx::STR], error_type);
    }

    // 6. TypeRegistry: newtype constructors, enum variant constructors
    //    Extract data with immutable borrow, then release before pool_mut
    if let Some(ctor) = resolve_type_constructor_info(engine, name) {
        return infer_constructor(engine, ctor);
    }

    // 7. Unknown identifier — find similar names for typo suggestions.
    let similar = engine
        .env()
        .find_similar(name, 3, |n| engine.lookup_name(n));
    engine.push_error(TypeCheckError::unknown_ident(span, name, similar));
    Idx::ERROR
}

fn infer_constructor(engine: &mut InferEngine<'_>, ctor: ConstructorInfo) -> Idx {
    match ctor {
        ConstructorInfo::Newtype {
            underlying,
            type_idx,
        } => engine.pool_mut().function(&[underlying], type_idx),
        ConstructorInfo::UnitVariant {
            enum_idx,
            enum_name,
            type_params,
        } => {
            if type_params.is_empty() {
                return enum_idx;
            }

            let fresh_vars: Vec<Idx> = type_params
                .iter()
                .map(|_| engine.pool_mut().fresh_var())
                .collect();
            engine.pool_mut().applied(enum_name, &fresh_vars)
        }
        ConstructorInfo::TupleVariant {
            field_types,
            enum_idx,
            enum_name,
            type_params,
        } => {
            if type_params.is_empty() {
                return engine.pool_mut().function(&field_types, enum_idx);
            }

            let fresh_vars: Vec<Idx> = type_params
                .iter()
                .map(|_| engine.pool_mut().fresh_var())
                .collect();
            let subst_map: Vec<(Name, Idx)> = type_params
                .into_iter()
                .zip(fresh_vars.iter().copied())
                .collect();
            let substituted_fields: Vec<Idx> = field_types
                .iter()
                .map(|&field| substitute_type_params_with_map(engine, field, &subst_map))
                .collect();
            let return_type = engine.pool_mut().applied(enum_name, &fresh_vars);

            engine.pool_mut().function(&substituted_fields, return_type)
        }
    }
}

fn resolve_variant_constructor_info(
    engine: &InferEngine<'_>,
    name: Name,
) -> Option<ConstructorInfo> {
    let registry = engine.type_registry()?;
    let (type_entry, variant_def) = registry.lookup_variant_def(name)?;
    let enum_idx = type_entry.idx;
    let enum_name = type_entry.name;
    let type_params = type_entry.type_params.clone();

    Some(match &variant_def.fields {
        VariantFields::Unit => ConstructorInfo::UnitVariant {
            enum_idx,
            enum_name,
            type_params,
        },
        VariantFields::Tuple(types) => ConstructorInfo::TupleVariant {
            field_types: types.clone(),
            enum_idx,
            enum_name,
            type_params,
        },
        VariantFields::Record(fields) => ConstructorInfo::TupleVariant {
            field_types: fields.iter().map(|field| field.ty).collect(),
            enum_idx,
            enum_name,
            type_params,
        },
    })
}

/// Look up a name in the `TypeRegistry` to find constructor info.
///
/// Returns constructor info that can be used to build the appropriate type
/// after the registry borrow is released.
pub(crate) fn resolve_type_constructor_info(
    engine: &InferEngine<'_>,
    name: Name,
) -> Option<ConstructorInfo> {
    let registry = engine.type_registry()?;

    // Check if name is a type name
    if let Some(entry) = registry.get_by_name(name) {
        return match &entry.kind {
            TypeKind::Newtype { underlying } => Some(ConstructorInfo::Newtype {
                underlying: *underlying,
                type_idx: entry.idx,
            }),
            // Struct/Enum type names used as expressions: return as unit variant
            // (enables associated function calls like Type.new(...))
            TypeKind::Struct(_) | TypeKind::Enum { .. } => Some(ConstructorInfo::UnitVariant {
                enum_idx: entry.idx,
                enum_name: entry.name,
                type_params: entry.type_params.clone(),
            }),
            TypeKind::Alias { target } => Some(ConstructorInfo::UnitVariant {
                enum_idx: *target,
                enum_name: entry.name,
                type_params: entry.type_params.clone(),
            }),
        };
    }

    resolve_variant_constructor_info(engine, name)
}

/// Infer the type of a function reference (@name).
pub(crate) fn infer_function_ref(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    // Function references are looked up the same way as identifiers
    // but may have special handling for capability tracking
    infer_ident(engine, name, span)
}

/// Infer the type of self reference.
///
/// `self` can refer to:
/// - The current function type (for recursive calls in patterns like `recurse`)
/// - The impl `Self` type (in method bodies)
pub(crate) fn infer_self_ref(engine: &mut InferEngine<'_>, span: Span) -> Idx {
    if let Some(self_ty) = engine.self_type() {
        return self_ty;
    }
    engine.push_error(TypeCheckError::self_outside_impl(span));
    Idx::ERROR
}

/// Infer the type of a constant reference (`$name`).
///
/// Looks up the constant's registered type from the module-level `const_types` map.
/// If not found, emits an "undefined constant" error.
pub(crate) fn infer_const(engine: &mut InferEngine<'_>, name: Name, span: Span) -> Idx {
    if let Some(ty) = engine.const_type(name) {
        return ty;
    }
    engine.push_error(TypeCheckError::undefined_const(name, span));
    Idx::ERROR
}

/// Find type names similar to `target` in the type registry (for typo suggestions).
pub(crate) fn find_similar_type_names(
    engine: &InferEngine<'_>,
    type_registry: &TypeRegistry,
    target: Name,
) -> Vec<Name> {
    let Some(target_str) = engine.lookup_name(target) else {
        return Vec::new();
    };

    if target_str.is_empty() {
        return Vec::new();
    }

    let threshold = match target_str.len() {
        0 => return Vec::new(),
        1..=2 => 1,
        3..=5 => 2,
        _ => 3,
    };

    let mut matches: Vec<(Name, usize)> = type_registry
        .names()
        .filter(|&n| n != target)
        .filter_map(|candidate_name| {
            let candidate_str = engine.lookup_name(candidate_name)?;
            let len_diff = target_str.len().abs_diff(candidate_str.len());
            if len_diff > threshold {
                return None;
            }
            let distance = crate::edit_distance(target_str, candidate_str);
            (distance <= threshold).then_some((candidate_name, distance))
        })
        .collect();

    matches.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    matches.into_iter().take(3).map(|(n, _)| n).collect()
}

/// Convert a `PreludeFunctionDef` to a Pool function type.
///
/// Handles generic parameters (`Fresh` → fresh type variable) and
/// parameterized return types (`IteratorOf(Element)` → `Iterator<T>`
/// using the first param's fresh var).
fn prelude_function_to_idx(
    engine: &mut InferEngine<'_>,
    def: &ori_registry::PreludeFunctionDef,
) -> Idx {
    use ori_registry::ReturnTag;

    // Build parameter types, tracking fresh vars for generic params
    let mut param_types = Vec::with_capacity(def.params.len());
    let mut first_fresh: Option<Idx> = None;

    for param in def.params {
        let ty = match param.ty {
            ReturnTag::Fresh => {
                let var = engine.pool_mut().fresh_var();
                if first_fresh.is_none() {
                    first_fresh = Some(var);
                }
                var
            }
            ReturnTag::Concrete(tag) => super::registry_bridge::type_tag_to_idx(tag),
            _ => engine.pool_mut().fresh_var(),
        };
        param_types.push(ty);
    }

    // Build return type
    let ret = match def.returns {
        ReturnTag::Concrete(tag) => super::registry_bridge::type_tag_to_idx(tag),
        ReturnTag::Unit => Idx::UNIT,
        ReturnTag::IteratorOf(_) => {
            // For repeat: (T) -> Iterator<T>, use the first fresh var
            let elem = first_fresh.unwrap_or_else(|| engine.pool_mut().fresh_var());
            engine.pool_mut().iterator(elem)
        }
        _ => engine.pool_mut().fresh_var(),
    };

    engine.pool_mut().function(&param_types, ret)
}
