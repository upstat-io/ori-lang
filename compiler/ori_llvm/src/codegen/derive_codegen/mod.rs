//! LLVM IR generation for derived trait methods.
//!
//! Generates synthetic LLVM functions for `#[derive(...)]` attributes on structs
//! and enums. Each derived method becomes a real LLVM function registered in
//! `method_functions`, so the existing `lower_method_call` dispatch finds them
//! with no special path.
//!
//! Dispatch is strategy-driven: `DerivedTrait::strategy()` returns a `DeriveStrategy`
//! describing the composition logic (field iteration, result combination), and this
//! module interprets the strategy in LLVM IR terms. Adding a new trait only requires
//! adding a strategy entry in `ori_ir` — no per-trait function needed here.
//!
//! **Struct derives** use `StructBody` (field iteration, formatting, cloning, default).
//! **Enum derives** use `SumBody::MatchVariants` (tag comparison for unit variants,
//! with payload comparison for payload variants planned).

mod bodies;
mod clone_rc;
mod enum_bodies;
mod field_ops;
mod instantiation;
mod scaffolding;
mod string_helpers;
#[cfg(test)]
mod tests;

use instantiation::{
    concrete_instantiations, nested_derive_instantiations, substitute_enum_variants,
    substitute_struct_fields,
};

use ori_ir::{DerivedTrait, Module, Name, StructBody, SumBody, TypeDecl, TypeDeclKind};
use ori_types::{FieldDef, Idx, Tag, TypeEntry, TypeKind, VariantDef};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{debug, trace, warn};

use super::function_compiler::FunctionCompiler;

use bodies::{
    compile_clone_fields, compile_default_construct, compile_for_each_field, compile_format_fields,
};
use enum_bodies::compile_enum_match_variants;
pub(in crate::codegen::derive_codegen) use scaffolding::{
    emit_boxed_self_method_call, emit_derive_return, emit_method_call_for_derive,
    setup_derive_function, verify_derive_function, DeriveSetup,
};

// Entry point

/// Compile derived trait methods for all types in the module.
///
/// Iterates `module.types`, finds types with `#[derive(...)]`, resolves their
/// fields from `user_types`, and generates synthetic LLVM functions for each
/// derived method. Results are registered in `method_functions` so that
/// `lower_method_call` finds them through normal dispatch.
pub fn compile_derives<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    module: &Module,
    user_types: &[TypeEntry],
) {
    let type_map: FxHashMap<Name, &TypeEntry> = user_types.iter().map(|te| (te.name, te)).collect();

    // Why: the closure walk below registers a method for an INNER instantiation
    // (`Wrap` in `Wrap<Wrap<int>>`) only when its `Applied` head names a
    // derive-bearing type, gated by this set.
    let derive_bearing: FxHashSet<Name> = module
        .types
        .iter()
        .filter(|td| !td.derives.is_empty())
        .map(|td| td.name)
        .collect();
    let decl_by_name: FxHashMap<Name, &TypeDecl> =
        module.types.iter().map(|td| (td.name, td)).collect();

    // Visited set keyed by the materialized concrete body `Idx` terminates the
    // closure on recursive / cyclic types AND dedups an instantiation reachable
    // through multiple nesting paths.
    let mut visited: FxHashSet<Idx> = FxHashSet::default();

    for type_decl in &module.types {
        if type_decl.derives.is_empty() {
            continue;
        }

        let Some(type_entry) = type_map.get(&type_decl.name) else {
            warn!(
                name = %fc.lookup_name(type_decl.name),
                "no TypeEntry for type with derives — skipping"
            );
            continue;
        };

        let type_name = type_decl.name;
        let type_idx = type_entry.idx;
        let type_name_str = fc.lookup_name(type_name).to_owned();

        // A generic composite has one concrete instantiation per
        // distinct arg set (`P3Pair<int,str>`, `Box<int>`, `Box<Box<int>>`).
        // Each instantiation's materialized body lives in `Pool.resolutions`;
        // emit a layout-correct derived method PER instantiation using its
        // concrete field/payload types. A non-generic type has no `Applied`
        // instantiation — fall through to the single declared-body path.
        let instantiations = concrete_instantiations(fc, type_name);

        if instantiations.is_empty() {
            // Non-generic type: single declared-body path, no closure.
            match (&type_decl.kind, &type_entry.kind) {
                (TypeDeclKind::Struct(_), TypeKind::Struct(struct_def)) => {
                    let fields = struct_def.fields.clone();
                    compile_struct_derives(
                        fc,
                        &type_decl.derives,
                        type_name,
                        type_idx,
                        &type_name_str,
                        &fields,
                        false,
                    );
                }
                (TypeDeclKind::Sum(_), TypeKind::Enum { variants }) => {
                    let variants = variants.clone();
                    compile_enum_derives(
                        fc,
                        &type_decl.derives,
                        type_name,
                        type_idx,
                        &type_name_str,
                        &variants,
                        false,
                    );
                }
                _ => {
                    trace!(name = %type_name_str, "skipping derives for unsupported type kind");
                }
            }
            continue;
        }

        // Generic type: seed the work-list with the top-level instantiations,
        // then transitively close over inner derive-bearing instantiations
        // reachable through field/payload types (`Wrap<int>` reached only by
        // nesting inside `Wrap<Wrap<int>>`, which top-level enumeration misses).
        for (applied_idx, concrete_idx) in instantiations {
            compile_instantiation_closure(
                fc,
                concrete_idx,
                applied_idx,
                type_name,
                &decl_by_name,
                &type_map,
                &derive_bearing,
                &mut visited,
            );
        }
    }
}

/// Compile a generic-composite instantiation's derived methods and transitively
/// close over every nested derive-bearing `Applied` instantiation reachable
/// through its field/payload types. Iterative work-list (no recursion holding a
/// `&mut fc` borrow) with a body-`Idx` visited set that terminates on recursive
/// / cyclic types and dedups a shared inner instantiation.
fn compile_instantiation_closure<'a>(
    fc: &mut FunctionCompiler<'_, 'a, 'a, '_>,
    seed_concrete_idx: Idx,
    seed_applied_idx: Idx,
    seed_type_name: Name,
    decl_by_name: &FxHashMap<Name, &TypeDecl>,
    type_map: &FxHashMap<Name, &TypeEntry>,
    derive_bearing: &FxHashSet<Name>,
    visited: &mut FxHashSet<Idx>,
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

        // The declared `FieldDef`/`VariantDef` shape (names/spans/visibility)
        // comes from the registered `TypeEntry`; `substitute_*` projects the
        // concrete instantiation's field/payload types onto it.
        match (&type_decl.kind, &type_entry.kind) {
            (TypeDeclKind::Struct(_), TypeKind::Struct(struct_def)) => {
                let fields = struct_def.fields.clone();
                let concrete_fields = substitute_struct_fields(fc, &fields, concrete_idx);
                compile_struct_derives(
                    fc,
                    &type_decl.derives,
                    type_name,
                    concrete_idx,
                    &type_name_str,
                    &concrete_fields,
                    true,
                );
            }
            (TypeDeclKind::Sum(_), TypeKind::Enum { variants }) => {
                let variants = variants.clone();
                let concrete_variants = substitute_enum_variants(fc, &variants, concrete_idx);
                compile_enum_derives(
                    fc,
                    &type_decl.derives,
                    type_name,
                    concrete_idx,
                    &type_name_str,
                    &concrete_variants,
                    true,
                );
            }
            _ => {
                trace!(name = %type_name_str, "skipping derives for unsupported type kind");
                continue;
            }
        }

        // Every call site whose receiver is this `Applied` Idx (or its resolved
        // body) must reach the instantiation's method via receiver-type dispatch.
        fc.map_type_idx_to_name(applied_idx, type_name);

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

    // PC-2 guard: derive_codegen synthesizes LLVM IR without going through
    // the ArcFunction pipeline, so the primary seam does not cover
    // `type_idx`. Always-on plus debug-assert for double-belt. The
    // debug_assert covers `Tag::Var`, `Tag::Projection`, AND `Tag::Infer`
    // (broader than the always-on guard, which targets the production
    // PC-2 surface via `assert_no_unresolved_idx`).
    debug_assert!(
        !matches!(
            fc.pool().tag(fc.pool().resolve_fully(type_idx)),
            Tag::Var | Tag::Projection | Tag::Infer
        ),
        "derive_codegen received unresolved type_idx for struct {type_name_str}"
    );
    if let Err(err) = ori_arc::assert_no_unresolved_idx(fc.pool(), type_idx, type_name) {
        tracing::error!(
            contract_violation = true,
            name = %type_name_str,
            error = ?err,
            "PC-2 violation in compile_struct_derives — skipping all derives for this type"
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

    // PC-2 guard: see compile_struct_derives above for rationale. The
    // debug_assert covers `Tag::Var`, `Tag::Projection`, AND `Tag::Infer`
    // (broader than the always-on guard).
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
