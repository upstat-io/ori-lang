//! Registration passes for module type checking.
//!
//! These passes run before signature collection to populate the registries
//! with type definitions, traits, and implementations.
//!
//! # Cross-Reference
//!
//! - Trait features: `plans/roadmap/section-03-traits.md`
//! - Module checker design: `plans/types_v2/section-08b-module-checker.md`

use ori_ir::{
    DerivedTrait, ExprId, Module, Name, ParsedType, Span, TraitItem, Visibility as IrVisibility,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::ModuleChecker;
use crate::{
    EnumVariant, FieldDef, Idx, ImplEntry, ImplMethodDef, ImplSpecificity, TraitAssocTypeDef,
    TraitEntry, TraitMethodDef, TypeCheckError, VariantDef, VariantFields, Visibility,
    WhereConstraint,
};

// ============================================================================
// Pass 0a: Built-in Types
// ============================================================================

/// Register built-in types that user code may reference.
///
/// Currently registers:
/// - `Ordering` enum (Less, Equal, Greater)
///
/// Note: Primitive types (int, str, etc.) are pre-interned in the Pool.
pub fn register_builtin_types(checker: &mut ModuleChecker<'_>) {
    // Ordering enum - used by comparison operations
    // The variants are unit variants (no data)
    let ordering_name = checker.interner().intern("Ordering");
    let less_name = checker.interner().intern("Less");
    let equal_name = checker.interner().intern("Equal");
    let greater_name = checker.interner().intern("Greater");

    let ordering_idx = Idx::ORDERING;

    let variants = vec![
        VariantDef {
            name: less_name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: equal_name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
        VariantDef {
            name: greater_name,
            fields: VariantFields::Unit,
            span: Span::DUMMY,
        },
    ];

    // Create Pool enum entry for Ordering (used by TypeRegistry for variant definitions).
    // No set_resolution: Idx::ORDERING is a pre-interned primitive and should not have
    // a resolution entry. Variant lookup returns Idx::ORDERING directly.
    let pool_variants = vec![
        EnumVariant {
            name: less_name,
            field_types: vec![],
        },
        EnumVariant {
            name: equal_name,
            field_types: vec![],
        },
        EnumVariant {
            name: greater_name,
            field_types: vec![],
        },
    ];
    let _enum_idx = checker.pool_mut().enum_type(ordering_name, &pool_variants);

    checker.type_registry_mut().register_enum(
        ordering_name,
        ordering_idx,
        vec![], // No type params
        variants,
        Span::DUMMY,
        Visibility::Public,
    );
}

// ============================================================================
// Pass 0b: User-Defined Types
// ============================================================================

/// Register user-defined types (structs, enums, newtypes).
pub fn register_user_types(checker: &mut ModuleChecker<'_>, module: &Module) {
    for type_decl in &module.types {
        register_type_decl(checker, type_decl);
    }
}

/// Register a single type declaration.
fn register_type_decl(checker: &mut ModuleChecker<'_>, decl: &ori_ir::TypeDecl) {
    // Collect generic parameters
    let type_params = collect_generic_params(checker, decl.generics);

    // Create pool index for this type
    let idx = checker.pool_mut().named(decl.name);

    // Convert visibility
    let visibility = convert_visibility(decl.visibility);

    // Build and register based on declaration kind
    match &decl.kind {
        ori_ir::TypeDeclKind::Struct(fields) => {
            let field_defs: Vec<FieldDef> = fields
                .iter()
                .map(|f| {
                    let ty = resolve_field_type(checker, &f.ty, &type_params);
                    FieldDef {
                        name: f.name,
                        ty,
                        span: f.span,
                        visibility: Visibility::Public,
                    }
                })
                .collect();

            // E2019: Never type cannot appear as a struct field.
            // Direct comparison (not resolve_fully) — aliases may not be registered yet.
            for f in &field_defs {
                if f.ty == Idx::NEVER {
                    checker.push_error(TypeCheckError::uninhabited_struct_field(
                        f.span, decl.name, f.name,
                    ));
                }
            }

            // Create Pool struct entry BEFORE moving field_defs to TypeRegistry.
            // Extract (Name, Idx) pairs for the Pool's compact representation.
            let pool_fields: Vec<(ori_ir::Name, Idx)> =
                field_defs.iter().map(|f| (f.name, f.ty)).collect();
            let struct_idx = checker.pool_mut().struct_type(decl.name, &pool_fields);
            checker.pool_mut().set_resolution(idx, struct_idx);

            checker.type_registry_mut().register_struct(
                decl.name,
                idx,
                type_params,
                field_defs,
                decl.span,
                visibility,
            );
        }

        ori_ir::TypeDeclKind::Sum(variants) => {
            let variant_defs: Vec<VariantDef> = variants
                .iter()
                .map(|v| {
                    let fields = if v.fields.is_empty() {
                        VariantFields::Unit
                    } else {
                        let field_defs: Vec<FieldDef> = v
                            .fields
                            .iter()
                            .map(|f| {
                                let ty = resolve_field_type(checker, &f.ty, &type_params);
                                FieldDef {
                                    name: f.name,
                                    ty,
                                    span: f.span,
                                    visibility: Visibility::Public,
                                }
                            })
                            .collect();
                        VariantFields::Record(field_defs)
                    };

                    VariantDef {
                        name: v.name,
                        fields,
                        span: v.span,
                    }
                })
                .collect();

            // Create Pool enum entry BEFORE moving variant_defs to TypeRegistry.
            // Extract variant info for the Pool's compact representation.
            let pool_variants: Vec<EnumVariant> = variant_defs
                .iter()
                .map(|v| {
                    let field_types = match &v.fields {
                        VariantFields::Unit => vec![],
                        VariantFields::Tuple(types) => types.clone(),
                        VariantFields::Record(field_defs) => {
                            field_defs.iter().map(|f| f.ty).collect()
                        }
                    };
                    EnumVariant {
                        name: v.name,
                        field_types,
                    }
                })
                .collect();
            let enum_idx = checker.pool_mut().enum_type(decl.name, &pool_variants);
            checker.pool_mut().set_resolution(idx, enum_idx);

            checker.type_registry_mut().register_enum(
                decl.name,
                idx,
                type_params,
                variant_defs,
                decl.span,
                visibility,
            );
        }

        ori_ir::TypeDeclKind::Newtype(underlying) => {
            let underlying_ty = resolve_field_type(checker, underlying, &type_params);
            checker.type_registry_mut().register_newtype(
                decl.name,
                idx,
                type_params,
                underlying_ty,
                decl.span,
                visibility,
            );
        }
    }
}

/// Collect type generic parameter names from a generic param range.
///
/// Const generic parameters (`$N: int`) are filtered out — they are values,
/// not types, and should not be bound as type variables.
fn collect_generic_params(
    checker: &ModuleChecker<'_>,
    generics: ori_ir::GenericParamRange,
) -> Vec<Name> {
    checker
        .arena()
        .get_generic_params(generics)
        .iter()
        .filter(|param| !param.is_const)
        .map(|param| param.name)
        .collect()
}

/// Resolve a parsed type to an Idx, with generic parameters in scope.
///
/// This is a simplified version that handles common cases during type registration.
/// For full type resolution during inference, use the `resolve_parsed_type` function
/// from the `infer` module.
fn resolve_field_type(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    _type_params: &[Name],
) -> Idx {
    // We need to avoid borrow conflicts - get arena reference before borrowing pool
    // By calling through a helper that takes the arena by value (as ptr), we can
    // then borrow the pool mutably
    resolve_parsed_type_simple(checker, parsed)
}

/// Simplified type resolution for registration phase.
///
/// Handles primitives, lists, maps, tuples, functions, and named types.
/// Generic type arguments are not fully instantiated (deferred to inference).
pub(super) fn resolve_parsed_type_simple(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
) -> Idx {
    match parsed {
        ParsedType::Primitive(type_id) => {
            // TypeId uses a specific encoding - extract the primitive type
            match type_id.raw() & 0x0FFF_FFFF {
                0 => Idx::INT,
                1 => Idx::FLOAT,
                2 => Idx::BOOL,
                3 => Idx::STR,
                4 => Idx::CHAR,
                5 => Idx::BYTE,
                6 => Idx::UNIT,
                7 => Idx::NEVER,
                _ => Idx::ERROR,
            }
        }

        ParsedType::List(elem_id) => {
            let elem = checker.arena().get_parsed_type(*elem_id).clone();
            let elem_ty = resolve_parsed_type_simple(checker, &elem);
            checker.pool_mut().list(elem_ty)
        }

        ParsedType::Map { key, value } => {
            let key_parsed = checker.arena().get_parsed_type(*key).clone();
            let value_parsed = checker.arena().get_parsed_type(*value).clone();
            let key_ty = resolve_parsed_type_simple(checker, &key_parsed);
            let value_ty = resolve_parsed_type_simple(checker, &value_parsed);
            checker.pool_mut().map(key_ty, value_ty)
        }

        ParsedType::Tuple(elems) => {
            let elem_ids: Vec<_> = checker.arena().get_parsed_type_list(*elems).to_vec();
            let elem_types: Vec<Idx> = elem_ids
                .into_iter()
                .map(|elem_id| {
                    let elem = checker.arena().get_parsed_type(elem_id).clone();
                    resolve_parsed_type_simple(checker, &elem)
                })
                .collect();
            checker.pool_mut().tuple(&elem_types)
        }

        ParsedType::Function { params, ret } => {
            let param_ids: Vec<_> = checker.arena().get_parsed_type_list(*params).to_vec();
            let param_types: Vec<Idx> = param_ids
                .into_iter()
                .map(|param_id| {
                    let param = checker.arena().get_parsed_type(param_id).clone();
                    resolve_parsed_type_simple(checker, &param)
                })
                .collect();
            let ret_parsed = checker.arena().get_parsed_type(*ret).clone();
            let ret_ty = resolve_parsed_type_simple(checker, &ret_parsed);
            checker.pool_mut().function(&param_types, ret_ty)
        }

        ParsedType::Named { name, type_args } => {
            // Resolve type arguments if present
            let type_arg_ids: Vec<_> = checker.arena().get_parsed_type_list(*type_args).to_vec();
            let resolved_args: Vec<Idx> = type_arg_ids
                .into_iter()
                .map(|arg_id| {
                    let arg = checker.arena().get_parsed_type(arg_id).clone();
                    resolve_parsed_type_simple(checker, &arg)
                })
                .collect();

            // Well-known generic types must use their dedicated Pool constructors
            // to ensure type representations match between annotations and inference.
            if !resolved_args.is_empty() {
                let name_str = checker.interner().lookup(*name);
                match (name_str, resolved_args.len()) {
                    ("Option", 1) => return checker.pool_mut().option(resolved_args[0]),
                    ("Result", 2) => {
                        return checker
                            .pool_mut()
                            .result(resolved_args[0], resolved_args[1]);
                    }
                    ("Set", 1) => return checker.pool_mut().set(resolved_args[0]),
                    ("Channel" | "Chan", 1) => {
                        return checker.pool_mut().channel(resolved_args[0]);
                    }
                    ("Range", 1) => return checker.pool_mut().range(resolved_args[0]),
                    ("Iterator", 1) => return checker.pool_mut().iterator(resolved_args[0]),
                    ("DoubleEndedIterator", 1) => {
                        return checker.pool_mut().double_ended_iterator(resolved_args[0]);
                    }
                    _ => {
                        return checker.pool_mut().applied(*name, &resolved_args);
                    }
                }
            }

            // No type args — check for pre-interned primitives before falling
            // through to pool.named(). Without this, struct fields like
            // `order: Ordering` would get a fresh Named Idx instead of Idx::ORDERING,
            // causing the same duality bug that affected register_builtin_types.
            let name_str = checker.interner().lookup(*name);
            match name_str {
                "Ordering" | "ordering" => return Idx::ORDERING,
                "Duration" | "duration" => return Idx::DURATION,
                "Size" | "size" => return Idx::SIZE,
                _ => {}
            }
            checker.pool_mut().named(*name)
        }

        ParsedType::FixedList { elem, capacity: _ } => {
            // Treat as regular list for now
            let elem_parsed = checker.arena().get_parsed_type(*elem).clone();
            let elem_ty = resolve_parsed_type_simple(checker, &elem_parsed);
            checker.pool_mut().list(elem_ty)
        }

        // These types need special handling during inference.
        // ConstExpr uses ERROR here (not fresh_var) because registration needs
        // deterministic types for Pool interning. Inference (infer/expr.rs) uses
        // fresh_var instead to allow optimistic deferral.
        ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::AssociatedType { .. }
        | ParsedType::ConstExpr(_) => Idx::ERROR,

        // Bounded trait object: resolve first bound as primary type
        ParsedType::TraitBounds(bounds) => {
            let bound_ids: Vec<_> = checker.arena().get_parsed_type_list(*bounds).to_vec();
            if let Some(&first_id) = bound_ids.first() {
                let first = checker.arena().get_parsed_type(first_id).clone();
                resolve_parsed_type_simple(checker, &first)
            } else {
                Idx::ERROR
            }
        }
    }
}

/// Convert IR visibility to Types visibility.
fn convert_visibility(ir_vis: IrVisibility) -> Visibility {
    match ir_vis {
        IrVisibility::Public => Visibility::Public,
        IrVisibility::Private => Visibility::Private,
    }
}

// ============================================================================
// Pass 0c: Traits and Implementations
// ============================================================================

/// Register trait definitions.
pub fn register_traits(checker: &mut ModuleChecker<'_>, module: &Module) {
    for trait_def in &module.traits {
        register_trait(checker, trait_def);
    }
}

/// Register a single trait definition.
///
/// Converts an `ori_ir::TraitDef` to a `TraitEntry` and registers it in the
/// `TraitRegistry`. This enables method resolution and trait bound checking.
fn register_trait(checker: &mut ModuleChecker<'_>, trait_def: &ori_ir::TraitDef) {
    // 1. Collect generic parameters
    let type_params = collect_generic_params(checker, trait_def.generics);

    // 2. Create pool index for this trait
    let idx = checker.pool_mut().named(trait_def.name);

    // 3. Process trait items (methods and associated types)
    let mut methods = FxHashMap::default();
    let mut assoc_types = FxHashMap::default();

    for item in &trait_def.items {
        match item {
            TraitItem::MethodSig(sig) => {
                // Required method (no default implementation)
                let method_def = build_trait_method_sig(checker, sig, &type_params);
                methods.insert(sig.name, method_def);
            }
            TraitItem::DefaultMethod(default_method) => {
                // Method with default implementation
                let method_def = build_trait_default_method(checker, default_method, &type_params);
                methods.insert(default_method.name, method_def);
            }
            TraitItem::AssocType(assoc) => {
                // Associated type (with optional default)
                let assoc_def = build_trait_assoc_type(checker, assoc, &type_params);
                assoc_types.insert(assoc.name, assoc_def);
            }
        }
    }

    // 4. Resolve super-traits to pool indices
    let super_traits: Vec<Idx> = trait_def
        .super_traits
        .iter()
        .map(|bound| checker.pool_mut().named(bound.name()))
        .collect();

    // 5. Register in TraitRegistry
    let entry = TraitEntry {
        name: trait_def.name,
        idx,
        type_params,
        super_traits,
        methods,
        assoc_types,
        span: trait_def.span,
    };

    checker.trait_registry_mut().register_trait(entry);
}

/// Build a `TraitMethodDef` from a required method signature.
fn build_trait_method_sig(
    checker: &mut ModuleChecker<'_>,
    sig: &ori_ir::TraitMethodSig,
    type_params: &[Name],
) -> TraitMethodDef {
    // Resolve parameter types
    let params: Vec<_> = checker.arena().get_params(sig.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            p.ty.as_ref().map_or(Idx::ERROR, |ty| {
                resolve_type_with_params(checker, ty, type_params)
            })
        })
        .collect();

    // Resolve return type
    let return_ty = resolve_type_with_params(checker, &sig.return_ty, type_params);

    // Create function type for signature
    let signature = checker.pool_mut().function(&param_types, return_ty);

    TraitMethodDef {
        name: sig.name,
        signature,
        has_default: false,
        default_body: None,
        span: sig.span,
    }
}

/// Build a `TraitMethodDef` from a method with default implementation.
fn build_trait_default_method(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::TraitDefaultMethod,
    type_params: &[Name],
) -> TraitMethodDef {
    // Resolve parameter types
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            p.ty.as_ref().map_or(Idx::ERROR, |ty| {
                resolve_type_with_params(checker, ty, type_params)
            })
        })
        .collect();

    // Resolve return type
    let return_ty = resolve_type_with_params(checker, &method.return_ty, type_params);

    // Create function type for signature
    let signature = checker.pool_mut().function(&param_types, return_ty);

    TraitMethodDef {
        name: method.name,
        signature,
        has_default: true,
        default_body: Some(method.body),
        span: method.span,
    }
}

/// Build a `TraitAssocTypeDef` from an associated type declaration.
fn build_trait_assoc_type(
    checker: &mut ModuleChecker<'_>,
    assoc: &ori_ir::TraitAssocType,
    type_params: &[Name],
) -> TraitAssocTypeDef {
    // Resolve default type if present
    let default = assoc
        .default_type
        .as_ref()
        .map(|ty| resolve_type_with_params(checker, ty, type_params));

    // TODO: Resolve bounds on associated type
    let bounds = Vec::new();

    TraitAssocTypeDef {
        name: assoc.name,
        bounds,
        default,
        span: assoc.span,
    }
}

/// Resolve a parsed type with type parameters in scope.
///
/// Type parameters are looked up by name and replaced with fresh type variables
/// during inference. For registration, we just create a named type placeholder.
fn resolve_type_with_params(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
) -> Idx {
    match parsed {
        ParsedType::Named { name, .. } => {
            // Check if this is a type parameter
            if type_params.contains(name) {
                // Create a named type for the parameter
                // During inference, this will be replaced with a fresh type variable
                checker.pool_mut().named(*name)
            } else {
                // Regular named type
                resolve_parsed_type_simple(checker, parsed)
            }
        }
        ParsedType::SelfType => {
            // Self type - create a placeholder named type
            // Will be substituted with the actual implementing type during impl registration
            let self_name = checker.interner().intern("Self");
            checker.pool_mut().named(self_name)
        }
        _ => resolve_parsed_type_simple(checker, parsed),
    }
}

/// Register implementation blocks.
///
/// For trait impls, also registers unoverridden default methods so they're
/// visible during method resolution in function body checking (Pass 2).
pub fn register_impls(checker: &mut ModuleChecker<'_>, module: &Module) {
    for impl_def in &module.impls {
        register_impl(checker, impl_def, &module.traits);
    }
}

/// Register a single implementation.
///
/// Converts an `ori_ir::ImplDef` to an `ImplEntry` and registers it in the
/// `TraitRegistry`. Handles both inherent impls (`impl Type { ... }`) and
/// trait impls (`impl Trait for Type { ... }`).
fn register_impl(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
) {
    // 1. Collect generic parameters
    let type_params = collect_generic_params(checker, impl_def.generics);

    // 2. Resolve self type
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty);

    // 3. Resolve trait (if trait impl)
    let trait_idx = impl_def.trait_path.as_ref().map(|path| {
        // Use the last segment of the trait path as the trait name
        let trait_name = path
            .last()
            .copied()
            .unwrap_or_else(|| checker.interner().intern("<unknown>"));
        checker.pool_mut().named(trait_name)
    });

    // 4. Process explicitly defined methods
    let mut methods = FxHashMap::default();
    for impl_method in &impl_def.methods {
        let method_def = build_impl_method(checker, impl_method, &type_params, self_type);
        methods.insert(impl_method.name, method_def);
    }

    // 4b. For trait impls, register unoverridden default methods (direct + transitive)
    //
    // explicit_methods tracks methods from steps 3+4b (explicit impl methods + direct
    // trait defaults). Step 6c uses this to detect conflicting defaults — transitive
    // defaults must NOT be in this set, otherwise conflicts are silently masked.
    let explicit_methods: FxHashSet<Name>;
    if let Some(trait_path) = &impl_def.trait_path {
        // Step 1: Direct defaults from the AST trait definition
        if let Some(&trait_name) = trait_path.last() {
            if let Some(trait_def) = traits.iter().find(|t| t.name == trait_name) {
                for item in &trait_def.items {
                    if let ori_ir::TraitItem::DefaultMethod(default) = item {
                        methods.entry(default.name).or_insert_with(|| {
                            let as_impl = ori_ir::ImplMethod::from(default);
                            build_impl_method(checker, &as_impl, &type_params, self_type)
                        });
                    }
                }
            }
        }

        // Snapshot explicit methods BEFORE transitive defaults are added.
        explicit_methods = methods.keys().copied().collect();

        // Step 2: Transitive defaults from super-trait hierarchy via the registry.
        // Borrow dance: scope the immutable trait_registry borrow to extract the
        // data we need, then use checker mutably for build_impl_method.
        if let Some(t_idx) = trait_idx {
            let transitive_defaults: Vec<(Name, Idx, ExprId, Span)> = {
                let reg = checker.trait_registry();
                reg.collected_methods(t_idx)
                    .into_iter()
                    .filter_map(|(name, _owner, def)| {
                        let body = def.default_body?;
                        if !def.has_default {
                            return None;
                        }
                        Some((name, def.signature, body, def.span))
                    })
                    .collect()
            };

            for (name, signature, body, span) in transitive_defaults {
                methods.entry(name).or_insert(ImplMethodDef {
                    name,
                    signature,
                    has_self: true,
                    body,
                    span,
                });
            }
        }
    } else {
        // Non-trait impls: all methods are explicit
        explicit_methods = methods.keys().copied().collect();
    }

    // 5. Process associated type definitions
    let mut assoc_types = FxHashMap::default();
    for impl_assoc in &impl_def.assoc_types {
        let ty = resolve_type_with_self(checker, &impl_assoc.ty, &type_params, self_type);
        assoc_types.insert(impl_assoc.name, ty);
    }

    // 6. Process where clauses (const bounds filtered out — not yet evaluated)
    let where_clause: Vec<WhereConstraint> = impl_def
        .where_clauses
        .iter()
        .filter_map(|wc| build_where_constraint(checker, wc, &type_params, self_type))
        .collect();

    // 6b. Validate all required associated types are defined
    if let Some(t_idx) = trait_idx {
        if let Some(trait_entry) = checker.trait_registry().get_trait_by_idx(t_idx) {
            let trait_name = trait_entry.name;
            let required: Vec<Name> = trait_entry
                .assoc_types
                .iter()
                .filter(|(_, def)| def.default.is_none())
                .map(|(&name, _)| name)
                .collect();

            for name in required {
                if !assoc_types.contains_key(&name) {
                    checker.push_error(TypeCheckError::missing_assoc_type(
                        impl_def.span,
                        name,
                        trait_name,
                    ));
                }
            }
        }
    }

    // 6c. Check for conflicting default methods from super-traits
    if let Some(t_idx) = trait_idx {
        // Borrow dance: scope the registry borrow to extract conflict data
        let conflicts: Vec<(Name, Vec<Name>)> = {
            let reg = checker.trait_registry();
            reg.find_conflicting_defaults(t_idx)
                .into_iter()
                .map(|(method_name, provider_idxs)| {
                    let names: Vec<Name> = provider_idxs
                        .iter()
                        .filter_map(|&idx| reg.get_trait_by_idx(idx).map(|e| e.name))
                        .collect();
                    (method_name, names)
                })
                .collect()
        };

        for (method_name, provider_names) in conflicts {
            // Only report if the impl doesn't explicitly override the method.
            // Check against explicit_methods (step 3 + step 4b direct defaults),
            // NOT the full methods map which includes transitive defaults.
            if !explicit_methods.contains(&method_name) && provider_names.len() >= 2 {
                checker.push_error(TypeCheckError::conflicting_defaults(
                    impl_def.span,
                    method_name,
                    provider_names[0],
                    provider_names[1],
                ));
            }
        }
    }

    // 7. Check for coherence violations
    if let Some(t_idx) = trait_idx {
        // Borrow dance: extract existing impl span and trait name, then push error
        let existing: Option<(Span, Name)> = {
            let reg = checker.trait_registry();
            reg.find_impl(t_idx, self_type).and_then(|(_, entry)| {
                let trait_name = reg.get_trait_by_idx(t_idx).map(|t| t.name)?;
                Some((entry.span, trait_name))
            })
        };
        if let Some((first_span, trait_name)) = existing {
            checker.push_error(TypeCheckError::duplicate_impl(
                impl_def.span,
                first_span,
                trait_name,
            ));
            return;
        }
    }

    // 8. Compute specificity
    let specificity = if type_params.is_empty() {
        ImplSpecificity::Concrete
    } else if !where_clause.is_empty() {
        ImplSpecificity::Constrained
    } else {
        ImplSpecificity::Generic
    };

    // 9. Register in TraitRegistry
    let entry = ImplEntry {
        trait_idx,
        self_type,
        type_params,
        methods,
        assoc_types,
        where_clause,
        specificity,
        span: impl_def.span,
    };

    checker.trait_registry_mut().register_impl(entry);
}

/// Build an `ImplMethodDef` from an impl method.
fn build_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::ImplMethod,
    type_params: &[Name],
    self_type: Idx,
) -> ImplMethodDef {
    // Resolve parameter types, substituting Self with the actual type
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            let is_self = checker.interner().lookup(p.name) == "self";
            match p.ty.as_ref() {
                Some(ty) => resolve_type_with_self(checker, ty, type_params, self_type),
                None if is_self => self_type,
                None => Idx::ERROR,
            }
        })
        .collect();

    // Resolve return type (return_ty is a ParsedType, not Option)
    let return_ty = resolve_type_with_self(checker, &method.return_ty, type_params, self_type);

    // Detect whether the first parameter is `self` (instance method vs associated function)
    let has_self = params
        .first()
        .is_some_and(|p| checker.interner().lookup(p.name) == "self");

    // Create function type for signature
    let signature = checker.pool_mut().function(&param_types, return_ty);

    ImplMethodDef {
        name: method.name,
        signature,
        has_self,
        body: method.body,
        span: method.span,
    }
}

/// Build a `WhereConstraint` from a where clause.
///
/// Returns `None` for const bounds (not yet evaluated).
fn build_where_constraint(
    checker: &mut ModuleChecker<'_>,
    wc: &ori_ir::WhereClause,
    type_params: &[Name],
    self_type: Idx,
) -> Option<WhereConstraint> {
    let (param, _projection, bounds, _span) = wc.as_type_bound()?;

    // Resolve the constrained type parameter
    let ty = if type_params.contains(&param) {
        checker.pool_mut().named(param)
    } else if param == checker.interner().intern("Self") {
        self_type
    } else {
        // Fallback to named type
        checker.pool_mut().named(param)
    };

    // Resolve the trait bounds
    // TraitBound has `first` and `rest` fields for path segments
    // Use the `name()` method to get the last segment (the actual trait name)
    let resolved_bounds: Vec<Idx> = bounds
        .iter()
        .map(|bound| {
            // Use the name() method which returns the last segment (or first if rest is empty)
            checker.pool_mut().named(bound.name())
        })
        .collect();

    Some(WhereConstraint {
        ty,
        bounds: resolved_bounds,
    })
}

/// Resolve a parsed type with Self substitution.
///
/// Replaces `Self` references with the actual implementing type.
pub(super) fn resolve_type_with_self(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
    self_type: Idx,
) -> Idx {
    match parsed {
        ParsedType::SelfType => self_type,
        ParsedType::Named { name, .. } => {
            // Check if this is a type parameter
            if type_params.contains(name) {
                checker.pool_mut().named(*name)
            } else {
                resolve_parsed_type_simple(checker, parsed)
            }
        }
        ParsedType::List(elem_id) => {
            let elem = checker.arena().get_parsed_type(*elem_id).clone();
            let elem_ty = resolve_type_with_self(checker, &elem, type_params, self_type);
            checker.pool_mut().list(elem_ty)
        }
        ParsedType::Map { key, value } => {
            let key_parsed = checker.arena().get_parsed_type(*key).clone();
            let value_parsed = checker.arena().get_parsed_type(*value).clone();
            let key_ty = resolve_type_with_self(checker, &key_parsed, type_params, self_type);
            let value_ty = resolve_type_with_self(checker, &value_parsed, type_params, self_type);
            checker.pool_mut().map(key_ty, value_ty)
        }
        ParsedType::Tuple(elems) => {
            let elem_ids: Vec<_> = checker.arena().get_parsed_type_list(*elems).to_vec();
            let elem_types: Vec<Idx> = elem_ids
                .into_iter()
                .map(|elem_id| {
                    let elem = checker.arena().get_parsed_type(elem_id).clone();
                    resolve_type_with_self(checker, &elem, type_params, self_type)
                })
                .collect();
            checker.pool_mut().tuple(&elem_types)
        }
        ParsedType::Function { params, ret } => {
            let param_ids: Vec<_> = checker.arena().get_parsed_type_list(*params).to_vec();
            let param_types: Vec<Idx> = param_ids
                .into_iter()
                .map(|param_id| {
                    let param = checker.arena().get_parsed_type(param_id).clone();
                    resolve_type_with_self(checker, &param, type_params, self_type)
                })
                .collect();
            let ret_parsed = checker.arena().get_parsed_type(*ret).clone();
            let ret_ty = resolve_type_with_self(checker, &ret_parsed, type_params, self_type);
            checker.pool_mut().function(&param_types, ret_ty)
        }
        // Bounded trait object: resolve first bound with self-substitution
        ParsedType::TraitBounds(bounds) => {
            let bound_ids: Vec<_> = checker.arena().get_parsed_type_list(*bounds).to_vec();
            if let Some(&first_id) = bound_ids.first() {
                let first = checker.arena().get_parsed_type(first_id).clone();
                resolve_type_with_self(checker, &first, type_params, self_type)
            } else {
                Idx::ERROR
            }
        }
        _ => resolve_parsed_type_simple(checker, parsed),
    }
}

// ============================================================================
// Pass 0d: Derived Implementations
// ============================================================================

/// Register derived trait implementations.
pub fn register_derived_impls(checker: &mut ModuleChecker<'_>, module: &Module) {
    for type_decl in &module.types {
        for derive in &type_decl.derives {
            register_derived_impl(checker, type_decl, *derive);
        }
    }
}

/// Generate and register an implementation for a derived trait.
///
/// Creates an impl block with proper method signatures so the type checker can
/// resolve derived method calls. The actual method bodies are generated by the
/// evaluator (interpreter) or LLVM codegen — the type checker only needs the
/// signatures for return type resolution.
fn register_derived_impl(
    checker: &mut ModuleChecker<'_>,
    type_decl: &ori_ir::TypeDecl,
    trait_name: Name,
) {
    // 1. Get the trait index
    let trait_idx = checker.pool_mut().named(trait_name);

    // 2. Get the self type
    let self_type = checker.pool_mut().named(type_decl.name);

    // 3. Collect type parameters from the type declaration
    let type_params = collect_generic_params(checker, type_decl.generics);

    // 4. Check if this impl already exists (coherence check)
    if checker.trait_registry().has_impl(trait_idx, self_type) {
        // Already have an impl for this trait+type combination
        return;
    }

    // 5. Build method signatures for the derived trait
    let methods = build_derived_methods(checker, trait_name, self_type, type_decl.span);

    // 6. Create and register the impl entry (derived impls are always concrete)
    let entry = ImplEntry {
        trait_idx: Some(trait_idx),
        self_type,
        type_params,
        methods,
        assoc_types: FxHashMap::default(),
        where_clause: Vec::new(),
        specificity: ImplSpecificity::Concrete,
        span: type_decl.span,
    };

    checker.trait_registry_mut().register_impl(entry);
}

/// Build the method map for a derived trait implementation.
///
/// Each derived trait has a single method with a known signature:
/// - Eq: `eq(self: T, other: T) -> bool`
/// - Clone: `clone(self: T) -> T`
/// - Hashable: `hash(self: T) -> int`
/// - Printable: `to_string(self: T) -> str`
/// - Default: `default() -> T`
fn build_derived_methods(
    checker: &mut ModuleChecker<'_>,
    trait_name: Name,
    self_type: Idx,
    span: Span,
) -> FxHashMap<Name, ImplMethodDef> {
    let trait_str = checker.interner().lookup(trait_name);
    let Some(trait_kind) = DerivedTrait::from_name(trait_str) else {
        return FxHashMap::default();
    };

    let method_str = trait_kind.method_name();
    let method_name = checker.interner().intern(method_str);

    // Build function type: (params...) -> return_type
    let signature = match trait_kind {
        DerivedTrait::Eq => checker
            .pool_mut()
            .function2(self_type, self_type, Idx::BOOL),
        DerivedTrait::Clone => checker.pool_mut().function1(self_type, self_type),
        DerivedTrait::Hashable => checker.pool_mut().function1(self_type, Idx::INT),
        DerivedTrait::Printable => checker.pool_mut().function1(self_type, Idx::STR),
        DerivedTrait::Default => checker.pool_mut().function0(self_type),
    };

    let has_self = !matches!(trait_kind, DerivedTrait::Default);

    let mut methods = FxHashMap::default();
    methods.insert(
        method_name,
        ImplMethodDef {
            name: method_name,
            signature,
            has_self,
            body: ExprId::INVALID,
            span,
        },
    );
    methods
}

// ============================================================================
// Pass 0e: Config Variables
// ============================================================================

/// Register constant types.
pub fn register_consts(checker: &mut ModuleChecker<'_>, module: &Module) {
    for const_def in &module.consts {
        register_const(checker, const_def);
    }
}

/// Register a single constant.
fn register_const(checker: &mut ModuleChecker<'_>, const_def: &ori_ir::ConstDef) {
    // Infer type from the value expression
    let ty = infer_const_type(checker, const_def.value);
    checker.register_const_type(const_def.name, ty);
}

/// Infer the type of a constant value expression.
///
/// Uses full expression inference so that computed constant expressions
/// (arithmetic, comparison, logical, references to other constants) are
/// handled correctly — not just literals.
fn infer_const_type(checker: &mut ModuleChecker<'_>, value_id: ori_ir::ExprId) -> Idx {
    let arena = checker.arena();
    let mut engine = checker.create_engine();
    let ty = crate::infer_expr(&mut engine, arena, value_id);
    let errors = engine.take_errors();
    let warnings = engine.take_warnings();
    for err in errors {
        checker.push_error(err);
    }
    for warning in warnings {
        checker.push_warning(warning);
    }
    ty
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#[expect(clippy::expect_used, reason = "Tests use expect for clarity")]
mod tests;
