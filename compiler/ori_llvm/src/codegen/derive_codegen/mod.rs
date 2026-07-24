//! LLVM IR generation for derived trait methods.
//!
//! # Registration
//!
//! Each derived method is a normal LLVM function registered in `method_functions`.
//!
//! # Dispatch
//!
//! [`DerivedTrait::strategy`] selects struct and enum body generation.
//! All submodules share the `ori_llvm::codegen::derive_codegen` tracing target.

mod bodies;
mod clone_rc;
mod enum_bodies;
mod field_ops;
mod instantiation;
mod scaffolding;
mod string_helpers;
#[cfg(test)]
mod tests;

use ori_ir::{DerivedTrait, Module, Name, StructBody, SumBody, TypeDecl, TypeDeclKind};
use ori_types::{FieldDef, Idx, Tag, TypeEntry, TypeKind, VariantDef};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{debug, trace, warn};

use super::function_compiler::FunctionCompiler;

use bodies::{
    compile_clone_fields, compile_default_construct, compile_for_each_field, compile_format_fields,
};
use enum_bodies::compile_enum_match_variants;
use instantiation::{
    concrete_instantiations, nested_derive_instantiations, substitute_enum_variants,
    substitute_struct_fields,
};
pub(in crate::codegen::derive_codegen) use scaffolding::{
    declare_derive_method, emit_boxed_self_method_call, emit_derive_return,
    emit_method_call_for_derive, setup_derive_function, verify_derive_function, DeriveSetup,
};

// Entry point

/// A concrete struct/enum body whose `#[derive(...)]` methods are emitted: the
/// substituted field/payload shape for one instantiation. Discovered once, then
/// DECLARED (Pass 1) and DEFINED (Pass 2) — the two-pass split lets a parent
/// body's field dispatch resolve an inner instantiation declared earlier.
enum DeriveBody {
    Struct(Vec<FieldDef>),
    Enum(Vec<VariantDef>),
}

struct DeriveWorkItem {
    type_name: Name,
    /// The registration + dispatch `Idx`: the materialized concrete body for a
    /// mono instantiation, the `TypeEntry.idx` for a non-generic type.
    type_idx: Idx,
    /// The `Applied` node mapped to `type_name` for receiver-type dispatch
    /// (`Some` for mono instantiations only).
    applied_idx: Option<Idx>,
    type_name_str: String,
    derives: Vec<Name>,
    body: DeriveBody,
    mono: bool,
}

/// Compile derived trait methods for all types in the module.
///
/// DECLARE-ALL-THEN-DEFINE-ALL: a discovery pass enumerates every reachable
/// derive instantiation (non-generic types + the transitive nested-generic
/// closure); Pass 1 declares + registers every method WITHOUT a body; Pass 2
/// emits all bodies. LLVM permits calling a declared-but-not-yet-defined
/// function, so a parent body's field-dispatch lookup
/// (`get_derived_method_for_type`) always resolves order-independently — and
/// recursive / mutually-recursive derive types need no cycle-breaking. Results
/// register in `method_functions` so `lower_method_call` finds them by normal
/// dispatch.
pub fn compile_derives<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    module: &Module,
    user_types: &[TypeEntry],
) {
    let work = collect_derive_work_items(fc, module, user_types);

    // PASS 1 — DECLARE + register every work item's derived methods, no bodies.
    // The strategy gate mirrors Pass 2 exactly so no method is declared whose
    // body Pass 2 would not emit (a dangling declaration is an LLVM link error).
    for item in &work {
        let is_enum = matches!(item.body, DeriveBody::Enum(_));
        for derive_name in &item.derives {
            let trait_name_str = fc.lookup_name(*derive_name);
            let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) else {
                continue;
            };
            if is_enum && !matches!(trait_kind.strategy().sum_body, SumBody::MatchVariants) {
                continue;
            }
            declare_derive_method(
                fc,
                trait_kind,
                item.type_name,
                item.type_idx,
                &item.type_name_str,
                item.mono,
            );
        }
    }

    // PASS 2 — emit all bodies; `setup_derive_function` reuses the Pass-1
    // declaration so a body's field lookup finds an already-registered callee.
    for item in &work {
        match &item.body {
            DeriveBody::Struct(fields) => compile_struct_derives(
                fc,
                &item.derives,
                item.type_name,
                item.type_idx,
                &item.type_name_str,
                fields,
                item.mono,
            ),
            DeriveBody::Enum(variants) => compile_enum_derives(
                fc,
                &item.derives,
                item.type_name,
                item.type_idx,
                &item.type_name_str,
                variants,
                item.mono,
            ),
        }
        // Every call site whose receiver is this `Applied` Idx (or its resolved
        // body) reaches the instantiation's method via receiver-type dispatch.
        if let Some(applied_idx) = item.applied_idx {
            fc.map_type_idx_to_name(applied_idx, item.type_name);
        }
    }
}

/// Discover every reachable derive work item: one per declared derive-bearing
/// type, plus the transitive nested-generic closure for each generic composite.
/// Visited keyed by the materialized concrete body `Idx` terminates the closure
/// on recursive / cyclic types AND dedups an instantiation reachable through
/// multiple nesting paths.
fn collect_derive_work_items<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    module: &Module,
    user_types: &[TypeEntry],
) -> Vec<DeriveWorkItem> {
    let type_map: FxHashMap<Name, &TypeEntry> = user_types.iter().map(|te| (te.name, te)).collect();

    // INVARIANT: only generic derive heads participate in nested instantiation closure.
    let derive_bearing: FxHashSet<Name> = module
        .types
        .iter()
        .filter(|td| !td.derives.is_empty() && !td.generics.is_empty())
        .map(|td| td.name)
        .collect();
    let decl_by_name: FxHashMap<Name, &TypeDecl> =
        module.types.iter().map(|td| (td.name, td)).collect();

    let mut visited: FxHashSet<Idx> = FxHashSet::default();
    let mut work: Vec<DeriveWorkItem> = Vec::new();

    for type_decl in &module.types {
        if type_decl.derives.is_empty() {
            continue;
        }

        let Some(type_entry) = type_map.get(&type_decl.name).copied() else {
            warn!(
                name = %fc.lookup_name(type_decl.name),
                "no TypeEntry for type with derives — skipping"
            );
            continue;
        };

        let type_name = type_decl.name;

        // A generic composite needs a layout-correct method per concrete
        // instantiation (each materialized body in `Pool.resolutions`); a
        // non-generic type has no `Applied` and falls through to the declared body.
        let instantiations = concrete_instantiations(fc, type_name);

        if instantiations.is_empty() {
            // Non-generic type: single declared-body work item, no closure.
            let type_name_str = fc.lookup_name(type_name).to_owned();
            if !derive_type_idx_resolved(fc, type_entry.idx, type_name, &type_name_str) {
                continue;
            }
            if let Some(body) = derive_body(fc, type_decl, type_entry, None) {
                work.push(DeriveWorkItem {
                    type_name,
                    type_idx: type_entry.idx,
                    applied_idx: None,
                    type_name_str,
                    derives: type_decl.derives.clone(),
                    body,
                    mono: false,
                });
            }
            continue;
        }

        // Top-level enumeration misses inner-only instantiations (`Wrap<int>`
        // reached only by nesting in `Wrap<Wrap<int>>`); the closure transitively
        // closes over field/payload-reachable derive-bearing bodies.
        for (applied_idx, concrete_idx) in instantiations {
            collect_instantiation_closure(
                fc,
                concrete_idx,
                applied_idx,
                type_name,
                &decl_by_name,
                &type_map,
                &derive_bearing,
                &mut visited,
                &mut work,
            );
        }
    }

    work
}

/// PC-2 gate: `true` iff `type_idx` is fully resolved (no surviving
/// `Tag::Var`/`Tag::Projection`/`Tag::Infer`). On violation records a codegen
/// error + returns `false` so the type's derives are skipped in BOTH passes (no
/// dangling Pass-1 declaration). The `ArcFunction` seam that normally enforces
/// PC-2 does not cover `derive_codegen`; the `debug_assert` adds `Tag::Infer`
/// over the always-on `assert_no_unresolved_idx` production guard.
fn derive_type_idx_resolved<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    type_idx: Idx,
    type_name: Name,
    type_name_str: &str,
) -> bool {
    debug_assert!(
        !matches!(
            fc.pool().tag(fc.pool().resolve_fully(type_idx)),
            Tag::Var | Tag::Projection | Tag::Infer
        ),
        "derive_codegen received unresolved type_idx for {type_name_str}"
    );
    if let Err(err) = ori_arc::assert_no_unresolved_idx(fc.pool(), type_idx, type_name) {
        tracing::error!(
            contract_violation = true,
            name = %type_name_str,
            error = ?err,
            "PC-2 violation in derive_codegen — skipping all derives for this type"
        );
        fc.builder_mut().record_codegen_error();
        return false;
    }
    true
}

/// Build the concrete `DeriveBody` for an instantiation: the declared
/// `FieldDef`/`VariantDef` shape (names/spans/visibility) from the `TypeEntry`,
/// with `substitute_*` projecting the concrete instantiation's field/payload
/// types onto it when `concrete_idx` is `Some` (mono); `None` keeps the declared
/// body (non-generic). `None` return = unsupported type kind.
fn derive_body<'a>(
    fc: &FunctionCompiler<'_, 'a, 'a, '_>,
    type_decl: &TypeDecl,
    type_entry: &TypeEntry,
    concrete_idx: Option<Idx>,
) -> Option<DeriveBody> {
    match (&type_decl.kind, &type_entry.kind) {
        (TypeDeclKind::Struct(_), TypeKind::Struct(struct_def)) => {
            let fields = struct_def.fields.clone();
            let fields = match concrete_idx {
                Some(ci) => substitute_struct_fields(fc, &fields, ci),
                None => fields,
            };
            Some(DeriveBody::Struct(fields))
        }
        (TypeDeclKind::Sum(_), TypeKind::Enum { variants }) => {
            let variants = variants.clone();
            let variants = match concrete_idx {
                Some(ci) => substitute_enum_variants(fc, &variants, ci),
                None => variants,
            };
            Some(DeriveBody::Enum(variants))
        }
        _ => {
            trace!(name = %fc.lookup_name(type_decl.name), "skipping derives for unsupported type kind");
            None
        }
    }
}

/// Discover a generic-composite instantiation's derive work item and transitively
/// close over every nested derive-bearing instantiation reachable through its
/// field/payload types, appending each as a [`DeriveWorkItem`]. Iterative
/// work-list (no recursion holding a `&mut fc` borrow) with a body-`Idx` visited
/// set that terminates on recursive / cyclic types and dedups a shared inner
/// instantiation.
#[expect(
    clippy::too_many_arguments,
    reason = "the walk threads three shared read-only lookup tables plus two \
              disjoint &mut accumulators (visited, work); bundling them into a \
              config struct would force a whole-struct &mut over the read-only maps"
)]
fn collect_instantiation_closure<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    seed_concrete_idx: Idx,
    seed_applied_idx: Idx,
    seed_type_name: Name,
    decl_by_name: &FxHashMap<Name, &TypeDecl>,
    type_map: &FxHashMap<Name, &TypeEntry>,
    derive_bearing: &FxHashSet<Name>,
    visited: &mut FxHashSet<Idx>,
    work: &mut Vec<DeriveWorkItem>,
) {
    let mut worklist: Vec<(Idx, Idx, Name)> =
        vec![(seed_concrete_idx, seed_applied_idx, seed_type_name)];

    while let Some((concrete_idx, applied_idx, type_name)) = worklist.pop() {
        if !visited.insert(concrete_idx) {
            continue;
        }

        let Some(type_decl) = decl_by_name.get(&type_name).copied() else {
            continue;
        };
        if type_decl.derives.is_empty() {
            continue;
        }
        let Some(type_entry) = type_map.get(&type_name).copied() else {
            warn!(name = %fc.lookup_name(type_name), "no TypeEntry for nested instantiation — skipping");
            continue;
        };
        let type_name_str = fc.lookup_name(type_name).to_owned();
        if !derive_type_idx_resolved(fc, concrete_idx, type_name, &type_name_str) {
            continue;
        }

        if let Some(body) = derive_body(fc, type_decl, type_entry, Some(concrete_idx)) {
            work.push(DeriveWorkItem {
                type_name,
                type_idx: concrete_idx,
                applied_idx: Some(applied_idx),
                type_name_str,
                derives: type_decl.derives.clone(),
                body,
                mono: true,
            });
        }

        // Enqueue the inner derive-bearing instantiations this body recurses
        // into through its concrete field/payload types.
        for (inner_applied, inner_concrete, inner_name) in
            nested_derive_instantiations(fc, concrete_idx, derive_bearing)
        {
            if !visited.contains(&inner_concrete) {
                worklist.push((inner_concrete, inner_applied, inner_name));
            }
        }
    }
}

/// Compile derived trait methods for a struct type.
fn compile_struct_derives<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    derives: &[Name],
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    fields: &[FieldDef],
    mono: bool,
) {
    debug!(
        name = %type_name_str,
        derives = derives.len(),
        fields = fields.len(),
        "compiling struct derived methods"
    );

    // INVARIANT: PC-2 — `type_idx` is resolved. Discovery already gated every
    // work item; this defends a direct caller (the helper is the SSOT).
    if !derive_type_idx_resolved(fc, type_idx, type_name, type_name_str) {
        return;
    }

    for derive_name in derives {
        let trait_name_str = fc.lookup_name(*derive_name);
        let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) else {
            warn!(derive = %trait_name_str, "unknown derive trait — skipping");
            continue;
        };

        let strategy = trait_kind.strategy();
        match strategy.struct_body {
            StructBody::ForEachField { field_op, combine } => {
                compile_for_each_field(
                    fc,
                    trait_kind,
                    type_name,
                    type_idx,
                    type_name_str,
                    fields,
                    field_op,
                    combine,
                    mono,
                );
            }
            StructBody::FormatFields {
                open,
                separator,
                suffix,
                include_names,
            } => {
                compile_format_fields(
                    fc,
                    trait_kind,
                    type_name,
                    type_idx,
                    type_name_str,
                    fields,
                    open,
                    separator,
                    suffix,
                    include_names,
                    mono,
                );
            }
            StructBody::CloneFields => {
                compile_clone_fields(
                    fc,
                    trait_kind,
                    type_name,
                    type_idx,
                    type_name_str,
                    fields,
                    mono,
                );
            }
            StructBody::DefaultConstruct => {
                compile_default_construct(
                    fc,
                    trait_kind,
                    type_name,
                    type_idx,
                    type_name_str,
                    fields,
                    mono,
                );
            }
        }
    }
}

/// Compile derived trait methods for an enum type.
fn compile_enum_derives<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    derives: &[Name],
    type_name: Name,
    type_idx: Idx,
    type_name_str: &str,
    variants: &[VariantDef],
    mono: bool,
) {
    debug!(
        name = %type_name_str,
        derives = derives.len(),
        variants = variants.len(),
        "compiling enum derived methods"
    );

    // INVARIANT: PC-2 — `type_idx` is resolved here (see compile_struct_derives).
    debug_assert!(
        !matches!(
            fc.pool().tag(fc.pool().resolve_fully(type_idx)),
            Tag::Var | Tag::Projection | Tag::Infer
        ),
        "derive_codegen received unresolved type_idx for enum {type_name_str}"
    );
    if let Err(err) = ori_arc::assert_no_unresolved_idx(fc.pool(), type_idx, type_name) {
        tracing::error!(
            contract_violation = true,
            name = %type_name_str,
            error = ?err,
            "PC-2 violation in compile_enum_derives — skipping all derives for this type"
        );
        fc.builder_mut().record_codegen_error();
        return;
    }

    for derive_name in derives {
        let trait_name_str = fc.lookup_name(*derive_name);
        let Some(trait_kind) = DerivedTrait::from_name(trait_name_str) else {
            warn!(derive = %trait_name_str, "unknown derive trait — skipping");
            continue;
        };

        let strategy = trait_kind.strategy();
        match strategy.sum_body {
            SumBody::MatchVariants => {
                compile_enum_match_variants(
                    fc,
                    trait_kind,
                    type_name,
                    type_idx,
                    type_name_str,
                    variants,
                    &strategy.struct_body,
                    mono,
                );
            }
            SumBody::NotSupported => {
                trace!(
                    name = %type_name_str,
                    derive = %trait_name_str,
                    "derive trait does not support sum types"
                );
            }
        }
    }
}
