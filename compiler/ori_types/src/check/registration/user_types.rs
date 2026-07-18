//! User-defined type registration (Pass 0b).
//!
//! Registers struct, enum (sum type), and newtype declarations from user code
//! into both the Pool (for type interning) and `TypeRegistry` (for field access
//! and type checking).

mod repr;

use rustc_hash::FxHashSet;

use super::burden_compute::{compute_enum_burden, compute_newtype_burden, compute_struct_burden};
use super::type_resolution::{collect_generic_params, convert_visibility, resolve_field_type};
use crate::registry::burden::UserBurdenSpec;
use crate::{
    EnumVariant, FieldDef, Idx, ModuleChecker, Pool, Tag, TypeCheckError, TypeRegistry, VariantDef,
    VariantFields, Visibility,
};
use repr::validate_and_merge_repr_attrs;

/// Register user-defined types (structs, enums, newtypes).
pub fn register_user_types(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    for type_decl in &module.types {
        register_type_decl(checker, type_decl);
    }
}

/// Register a single type declaration.
fn register_type_decl(checker: &mut ModuleChecker<'_>, decl: &ori_ir::TypeDecl) {
    // Collect generic parameters
    let type_params = collect_generic_params(checker.arena(), decl.generics);

    // Create pool index for this type
    let idx = checker.pool_mut().named(decl.name);

    // Convert visibility
    let visibility = convert_visibility(decl.visibility);

    // Build and register based on declaration kind
    match &decl.kind {
        ori_ir::TypeDeclKind::Struct(_) => {
            register_struct_decl(checker, decl, idx, type_params, visibility);
        }
        ori_ir::TypeDeclKind::Sum(_) => {
            register_enum_decl(checker, decl, idx, type_params, visibility);
        }
        ori_ir::TypeDeclKind::Newtype(_) => {
            register_newtype_decl(checker, decl, idx, type_params, visibility);
        }
    }

    // Validate and merge #repr attributes (E2041).
    let resolved_repr = validate_and_merge_repr_attrs(checker, decl);

    // Re-register the resolved repr if validation produced a merged value.
    // The registration above passed the raw list; update with the resolved single attr.
    if let Some(repr) = resolved_repr {
        checker.type_registry_mut().set_repr(idx, Some(repr));
    } else if !decl.repr_attrs.is_empty() {
        // Had attrs but all were rejected — clear any that were set during registration.
        checker.type_registry_mut().set_repr(idx, None);
    }

    // Value-marker wiring (Spec: Annex E §AIMS — Value marker semantics).
    //
    // When this declaration carries the `Value` marker in `decl.derives`,
    // it asserts inline storage, bitwise copy, and no ARC. Two consequences:
    //
    //   1. The `UserBurdenSpec` is empty — Value types carry no heap, so the
    //      drop walk has nothing to release. Register the empty spec eagerly
    //      so Phase 5 burden lookups consume the asserted shape.
    //   2. Value + Drop are mutually exclusive. If an `impl T: Drop` was
    //      already registered when this declaration lands, emit E2049 with
    //      the span pointing at this (second) declaration.
    populate_value_burden_if_applicable(checker, decl, idx);
}

fn register_struct_decl(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    idx: Idx,
    type_params: Vec<ori_ir::Name>,
    visibility: Visibility,
) {
    let ori_ir::TypeDeclKind::Struct(fields) = &decl.kind else {
        unreachable!("struct registration called for non-struct declaration");
    };
    let field_defs: Vec<FieldDef> = fields
        .iter()
        .map(|field| FieldDef {
            name: field.name,
            ty: resolve_field_type(checker, &field.ty),
            span: field.span,
            visibility: Visibility::Public,
        })
        .collect();
    for field in &field_defs {
        if field.ty == Idx::NEVER {
            checker.push_error(TypeCheckError::uninhabited_struct_field(
                field.span, decl.name, field.name,
            ));
        }
    }
    let pool_fields: Vec<_> = field_defs
        .iter()
        .map(|field| (field.name, field.ty))
        .collect();
    let struct_idx = checker.pool_mut().struct_type(decl.name, &pool_fields);
    checker.pool_mut().set_resolution(idx, struct_idx);
    let hash = checker.pool().hash(idx);
    let burden = compute_struct_burden(&field_defs, checker.pool());
    checker.type_registry_mut().register_struct(
        decl.name,
        idx,
        type_params,
        field_defs,
        decl.span,
        visibility,
        hash,
        None,
        burden.clone(),
    );
    if let Some(spec) = burden {
        checker
            .type_registry_mut()
            .register_user_burden(struct_idx, spec);
    }
}

fn register_enum_decl(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    idx: Idx,
    type_params: Vec<ori_ir::Name>,
    visibility: Visibility,
) {
    let ori_ir::TypeDeclKind::Sum(variants) = &decl.kind else {
        unreachable!("enum registration called for non-enum declaration");
    };
    let variant_defs: Vec<VariantDef> = variants
        .iter()
        .map(|variant| {
            let fields = if variant.fields.is_empty() {
                VariantFields::Unit
            } else {
                VariantFields::Record(
                    variant
                        .fields
                        .iter()
                        .map(|field| FieldDef {
                            name: field.name,
                            ty: resolve_field_type(checker, &field.ty),
                            span: field.span,
                            visibility: Visibility::Public,
                        })
                        .collect(),
                )
            };
            VariantDef {
                name: variant.name,
                fields,
                span: variant.span,
            }
        })
        .collect();
    let pool_variants: Vec<EnumVariant> = variant_defs
        .iter()
        .map(|variant| EnumVariant {
            name: variant.name,
            field_types: match &variant.fields {
                VariantFields::Unit => vec![],
                VariantFields::Tuple(types) => types.clone(),
                VariantFields::Record(fields) => fields.iter().map(|field| field.ty).collect(),
            },
        })
        .collect();
    let enum_idx = checker.pool_mut().enum_type(decl.name, &pool_variants);
    checker.pool_mut().set_resolution(idx, enum_idx);
    let hash = checker.pool().hash(idx);
    let burden = compute_enum_burden(&variant_defs, checker.pool());
    checker.type_registry_mut().register_enum(
        decl.name,
        idx,
        type_params,
        variant_defs,
        decl.span,
        visibility,
        hash,
        None,
        burden.clone(),
    );
    if let Some(spec) = burden {
        checker
            .type_registry_mut()
            .register_user_burden(enum_idx, spec);
    }
}

fn register_newtype_decl(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    idx: Idx,
    type_params: Vec<ori_ir::Name>,
    visibility: Visibility,
) {
    let ori_ir::TypeDeclKind::Newtype(underlying) = &decl.kind else {
        unreachable!("newtype registration called for other declaration");
    };
    let underlying_ty = resolve_field_type(checker, underlying);
    let hash = checker.pool().hash(idx);
    let burden = compute_newtype_burden(underlying_ty, checker.pool(), checker.type_registry());
    checker.type_registry_mut().register_newtype(
        decl.name,
        idx,
        type_params,
        underlying_ty,
        decl.span,
        visibility,
        hash,
        None,
        burden,
    );
    checker
        .pool_mut()
        .register_newtype_ctor(decl.name, underlying_ty);
    checker.pool_mut().set_resolution(idx, underlying_ty);
}

/// When the type declaration carries `Value`, register an empty
/// `UserBurdenSpec` and emit E2049 if a `Drop` impl is already registered.
///
/// "Carries Value" is detected via `decl.derives.contains(&value_name)` —
/// the parser routes both pre-proposal `#derive(Value)` and post-proposal
/// `type T: Value, ... = {...}` into the same `derives: Vec<Name>` field.
/// When the trait-bound surface lands as a separate frontmatter field,
/// this detector extends to that field; the conflict rule does not change.
fn populate_value_burden_if_applicable(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    idx: Idx,
) {
    let value_name = checker.interner().intern("Value");
    if !decl.derives.contains(&value_name) {
        return;
    }

    // Query TraitRegistry for an existing `impl T: Drop`. When present,
    // the Drop impl was registered before this type declaration; the
    // conflict surfaces at the second registration (this declaration).
    // `Drop` lookup gracefully no-ops when the prelude has not yet
    // registered the trait — early-bootstrap modules without `Drop` in
    // scope cannot collide with `Value` anyway.
    let drop_name = checker.interner().intern("Drop");
    let drop_trait_idx = checker
        .trait_registry()
        .get_trait_by_name(drop_name)
        .map(|entry| entry.idx);
    if let Some(drop_idx) = drop_trait_idx {
        if checker.trait_registry().has_impl(drop_idx, idx) {
            checker.push_error(TypeCheckError::value_drop_conflict(decl.span, decl.name));
            // Fall through: still populate the empty Value-burden so
            // downstream consumers see a consistent shape (the diagnostic
            // already gates codegen).
        }
    }

    // Field-constraint enforcement (Spec: Annex E §Built-in Type
    // Representations — `Value` requires all fields to be `Value`). Walk
    // every declared field; a non-`Value` field
    // (`str`/`[T]`/`{K:V}`/`Set<T>` or any heap-bearing user type) is the
    // latent silent-RC-drop — registering an empty `UserBurdenSpec` for a
    // type with a heap-bearing field would drop that field's RC obligation.
    // E2032 (field missing required trait) names the offending field + the
    // `Value` trait it fails to satisfy. The empty-burden registration below
    // is sound ONLY when this guard holds.
    enforce_value_field_constraint(checker, decl, idx);

    // Empty UserBurdenSpec — Value types have no heap, no owned fields,
    // no variant payloads, no compiled drop, no user drop. The default
    // `UserBurdenSpec` already satisfies these zeros; we register it
    // explicitly so `TypeRegistry::burden(idx)` returns `Some(_)` (vs the
    // `None` that signals "not yet computed") and downstream lookup-site
    // `debug_assert!`s are satisfied without re-deriving.
    let empty_value_spec = UserBurdenSpec::default();
    checker
        .type_registry_mut()
        .register_user_burden(idx, empty_value_spec);

    // Record the Value marker so `register_impl` can detect
    // late-arriving `impl T: Drop` blocks and emit E2049 at that
    // registration site (order-independent enforcement).
    checker.type_registry_mut().record_value_marker(idx);
}

/// Reject a `Value`-marked type whose declared fields are not all `Value`.
///
/// Reads the just-registered `TypeEntry` for `decl_idx` (registration ran
/// before this site), walks each field's resolved `Idx`, and emits E2032
/// (field missing required trait) naming the first non-`Value` field per
/// declaration position. The declaring type's own `Idx` is seeded into the
/// visited set as `Value`-in-progress so a self-referential `Value` struct
/// terminates without infinite recursion (Spec Annex E §Built-in Type
/// Representations).
fn enforce_value_field_constraint(
    checker: &mut ModuleChecker<'_>,
    decl: &ori_ir::TypeDecl,
    decl_idx: Idx,
) {
    let value_name = checker.interner().intern("Value");

    // Collect the (field_name, parsed_type, span) triples to walk, cloning the
    // parsed types out of the borrowed decl so the resolver can take a mutable
    // pool borrow without conflicting with an outstanding `&decl` borrow. Order
    // matches the source declaration so the FIRST offending field is reported.
    let raw_fields: Vec<(ori_ir::Name, ori_ir::ParsedType, ori_ir::Span)> = match &decl.kind {
        ori_ir::TypeDeclKind::Struct(struct_fields) => struct_fields
            .iter()
            .map(|f| (f.name, f.ty.clone(), f.span))
            .collect(),
        ori_ir::TypeDeclKind::Sum(variants) => variants
            .iter()
            .flat_map(|v| v.fields.iter().map(|f| (f.name, f.ty.clone(), f.span)))
            .collect(),
        // Newtypes are layout-transparent (`RP-24`); the `Value` field
        // constraint applies to the single wrapped type. Use the decl name +
        // span as the offending-field label since a newtype has no field name.
        ori_ir::TypeDeclKind::Newtype(underlying) => {
            vec![(decl.name, underlying.clone(), decl.span)]
        }
    };

    // Resolve each parsed field type to its pool Idx (same Idx the marker
    // lookup keys on), then test Value-membership.
    let fields: Vec<(ori_ir::Name, Idx, ori_ir::Span)> = raw_fields
        .into_iter()
        .map(|(name, parsed, span)| (name, resolve_field_type(checker, &parsed), span))
        .collect();

    for (field_name, field_idx, field_span) in fields {
        let mut visited: FxHashSet<Idx> = FxHashSet::default();
        // Seed the declaring type as Value-in-progress: a self-reference
        // resolves as Value (the cycle the visited-set memo short-circuits).
        visited.insert(decl_idx);
        if !field_idx_is_value(
            field_idx,
            checker.pool(),
            checker.type_registry(),
            &mut visited,
        ) {
            checker.push_error(TypeCheckError::field_missing_trait_in_derive(
                field_span, decl.name, value_name, field_name, field_idx,
            ));
        }
    }
}

/// Cycle-aware `Value`-membership test for a single resolved field `Idx`.
///
/// `Value` (true): scalar primitives (`int`/`float`/`bool`/`char`/`byte`/
/// `void`/`Duration`/`Size`/`Ordering`/`Never`) and any user type carrying
/// the `Value` marker (including the declaring type itself via the seeded
/// visited set). Compound `Value` aggregates are accepted by their marker —
/// their own registration already validated their fields, so no re-descent is
/// needed. NOT `Value` (false): `str`/`[T]`/`{K:V}`/`Set<T>`, function /
/// channel / iterator types, and any user type lacking the marker.
///
/// `visited` is keyed on pool `Idx`; a revisited Idx is treated as resolved
/// (membership already being computed) and short-circuits to `true`, so a
/// recursive `Value`-marked type terminates.
fn field_idx_is_value(
    field_idx: Idx,
    pool: &Pool,
    type_registry: &TypeRegistry,
    visited: &mut FxHashSet<Idx>,
) -> bool {
    // Revisit short-circuit: an Idx already in the visited set is being
    // resolved (this includes the declaring type, which `enforce_*` seeds as
    // Value-in-progress so a self-reference like `me: SelfRef` terminates) —
    // treat as Value. This single memo IS the cycle guard.
    if !visited.insert(field_idx) {
        return true;
    }
    // A user type carrying the Value marker is Value (its own registration
    // validated its fields). The marker is keyed on the unresolved nominal
    // Idx — the same Idx `resolve_field_type` produced for the field.
    if type_registry.carries_value_marker(field_idx) {
        return true;
    }
    // Classify by tag. `pool.tag` reads the item directly (no resolution
    // chase) so a nominal field is seen as `Tag::Named` and routed to the
    // marker path above, not its boxed struct representation. Value is the
    // closed set below (scalar primitives + Error-poison); everything else is
    // NOT Value — heap-bearing builtins (`str`/`[T]`/`{K:V}`/`Set<T>`,
    // channels, iterators, functions), nominal/applied types lacking the
    // marker, and all type variables / projections / schemes / tuples.
    match pool.tag(field_idx) {
        // Scalar primitives are implicitly Value. `Error` joins them: a field
        // whose type already failed resolution is poison and must not cascade
        // a second diagnostic here (error-recovery monotonicity).
        Tag::Int
        | Tag::Float
        | Tag::Bool
        | Tag::Char
        | Tag::Byte
        | Tag::Unit
        | Tag::Never
        | Tag::Duration
        | Tag::Size
        | Tag::Ordering
        | Tag::Error => true,
        _ => false,
    }
}
