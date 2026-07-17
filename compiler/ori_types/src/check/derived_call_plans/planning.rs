//! Structural call-position discovery and frozen-plan construction.

use super::{
    select_producer, AcceptedDerivedImpl, CallPositions, DerivedCallPlan, DerivedCallPosition,
    DerivedCallSelection, DerivedDirectCallSelection, DerivedTrait, Idx, MethodProducer, Name,
    PlanSelectionSources, Pool, RegistryPreludeIdentity, SelectionOutcome, StringInterner, Tag,
    TraitRegistry, TypeCheckError,
};

pub(super) fn build_plan(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    binder_substitutions: Vec<Idx>,
    traits: &TraitRegistry,
    interner: &StringInterner,
    pool: &mut Pool,
    errors: &mut Vec<TypeCheckError>,
) -> Option<DerivedCallPlan> {
    let positions = collect_call_positions(accepted, receiver, pool, errors)?;
    build_plan_from_positions(
        accepted,
        receiver,
        binder_substitutions,
        positions,
        PlanSelectionSources { traits, interner },
        pool,
        errors,
    )
}

fn collect_call_positions(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    pool: &Pool,
    errors: &mut Vec<TypeCheckError>,
) -> Option<CallPositions> {
    let mut method_positions = Vec::new();
    let mut direct_positions = Vec::new();
    let resolved = pool.resolve_fully(receiver);
    tracing::debug!(
        target: "ori_types::derived_call_plans",
        derived = ?accepted.id,
        trait_kind = ?accepted.trait_kind,
        receiver = ?receiver,
        receiver_tag = ?pool.tag(receiver),
        resolved = ?resolved,
        resolved_tag = ?pool.tag(resolved),
        "building frozen derived-call plan",
    );

    if pool.is_newtype_ctor(accepted.owner_name) {
        if derived_trait_emits_method_call(accepted.trait_kind, resolved, pool) {
            method_positions.push((DerivedCallPosition::Newtype, resolved));
        }
    } else {
        match pool.tag(resolved) {
            Tag::Struct => {
                for (field, (_, field_type)) in pool.struct_fields(resolved).into_iter().enumerate()
                {
                    let position = DerivedCallPosition::Field(index_u32(field)?);
                    if derived_trait_emits_method_call(accepted.trait_kind, field_type, pool) {
                        method_positions.push((position, field_type));
                    }
                    if accepted.trait_kind == DerivedTrait::Hashable
                        && pool.tag(pool.resolve_fully(field_type)) != Tag::Unit
                    {
                        direct_positions.push(DerivedCallPosition::FieldCombine(index_u32(field)?));
                    }
                }
            }
            Tag::Enum => {
                if accepted.trait_kind == DerivedTrait::Comparable {
                    method_positions.push((DerivedCallPosition::Discriminant, Idx::INT));
                }
                if accepted.trait_kind == DerivedTrait::Hashable {
                    direct_positions.push(DerivedCallPosition::DiscriminantCombine);
                }
                for (variant, (_, fields)) in pool.enum_variants(resolved).into_iter().enumerate() {
                    for (field, field_type) in fields.into_iter().enumerate() {
                        let position = DerivedCallPosition::VariantField {
                            variant: index_u32(variant)?,
                            field: index_u32(field)?,
                        };
                        if derived_trait_emits_method_call(accepted.trait_kind, field_type, pool) {
                            method_positions.push((position, field_type));
                        }
                        if accepted.trait_kind == DerivedTrait::Hashable
                            && pool.tag(pool.resolve_fully(field_type)) != Tag::Unit
                        {
                            direct_positions.push(DerivedCallPosition::VariantFieldCombine {
                                variant: index_u32(variant)?,
                                field: index_u32(field)?,
                            });
                        }
                    }
                }
            }
            _ if accepted.trait_kind == DerivedTrait::Clone => {}
            _ => {
                errors.push(TypeCheckError::unknown_method(
                    accepted.span,
                    receiver,
                    accepted.method_name,
                ));
                return None;
            }
        }
    }
    Some(CallPositions {
        methods: method_positions,
        direct: direct_positions,
    })
}

fn build_plan_from_positions(
    accepted: &AcceptedDerivedImpl,
    receiver: Idx,
    binder_substitutions: Vec<Idx>,
    positions: CallPositions,
    sources: PlanSelectionSources<'_>,
    pool: &mut Pool,
    errors: &mut Vec<TypeCheckError>,
) -> Option<DerivedCallPlan> {
    let mut calls = Vec::with_capacity(positions.methods.len());
    for (position, nested_receiver) in positions.methods {
        tracing::debug!(
            target: "ori_types::derived_call_plans",
            derived = ?accepted.id,
            ?position,
            nested_receiver = ?nested_receiver,
            nested_tag = ?pool.tag(nested_receiver),
            nested_resolved = ?pool.resolve_fully(nested_receiver),
            "selecting nested generated-call producer",
        );
        match select_producer(
            nested_receiver,
            accepted.trait_type,
            accepted.method_name,
            sources.traits,
            sources.interner,
            pool,
        ) {
            SelectionOutcome::Found(selection) => calls.push(DerivedCallSelection {
                position,
                receiver_type: nested_receiver,
                trait_type: accepted.trait_type,
                method_name: accepted.method_name,
                has_self: selection.has_self,
                producer: selection.producer,
            }),
            SelectionOutcome::Missing => errors.push(TypeCheckError::unknown_method(
                accepted.span,
                nested_receiver,
                accepted.method_name,
            )),
            SelectionOutcome::Ambiguous(count) => {
                let trait_name = sources
                    .traits
                    .get_trait_by_idx(accepted.trait_type)
                    .map_or(accepted.method_name, |entry| entry.name);
                errors.push(TypeCheckError::ambiguous_method(
                    accepted.span,
                    accepted.method_name,
                    vec![trait_name; count],
                ));
            }
        }
    }
    if calls.len() != method_call_count(accepted.trait_kind, receiver, accepted.owner_name, pool)? {
        return None;
    }

    let mut direct_calls = Vec::with_capacity(positions.direct.len());
    if !positions.direct.is_empty() {
        let function_name = sources.interner.intern("hash_combine");
        let Some(identity) = ori_registry::find_prelude_function_id("hash_combine") else {
            errors.push(TypeCheckError::unknown_ident(
                accepted.span,
                function_name,
                Vec::new(),
            ));
            return None;
        };
        let producer = MethodProducer::Prelude(RegistryPreludeIdentity::from_registered(identity));
        direct_calls.extend(positions.direct.into_iter().map(|position| {
            DerivedDirectCallSelection {
                position,
                function_name,
                producer: producer.clone(),
            }
        }));
    }

    Some(DerivedCallPlan {
        derived: accepted.id,
        binder_substitutions,
        calls,
        direct_calls,
    })
}

fn index_u32(index: usize) -> Option<u32> {
    u32::try_from(index).ok()
}

fn derived_trait_emits_method_call(trait_kind: DerivedTrait, receiver: Idx, pool: &Pool) -> bool {
    let resolved = pool.resolve_fully(receiver);
    match trait_kind {
        DerivedTrait::Clone => false,
        DerivedTrait::Eq => pool.builtin_type_tag(resolved).is_none(),
        DerivedTrait::Default => !matches!(
            pool.tag(resolved),
            Tag::Int
                | Tag::Byte
                | Tag::Float
                | Tag::Bool
                | Tag::Str
                | Tag::Char
                | Tag::Unit
                | Tag::Duration
                | Tag::Size
        ),
        DerivedTrait::Hashable | DerivedTrait::Comparable => pool.tag(resolved) != Tag::Unit,
        DerivedTrait::Printable | DerivedTrait::Debug => true,
    }
}

fn method_call_count(
    trait_kind: DerivedTrait,
    receiver: Idx,
    owner_name: Name,
    pool: &Pool,
) -> Option<usize> {
    if pool.is_newtype_ctor(owner_name) {
        return Some(usize::from(derived_trait_emits_method_call(
            trait_kind,
            pool.resolve_fully(receiver),
            pool,
        )));
    }
    let resolved = pool.resolve_fully(receiver);
    let mut count =
        usize::from(trait_kind == DerivedTrait::Comparable && pool.tag(resolved) == Tag::Enum);
    match pool.tag(resolved) {
        Tag::Struct => {
            count += pool
                .struct_fields(resolved)
                .into_iter()
                .filter(|(_, ty)| derived_trait_emits_method_call(trait_kind, *ty, pool))
                .count();
        }
        Tag::Enum => {
            count += pool
                .enum_variants(resolved)
                .into_iter()
                .flat_map(|(_, fields)| fields)
                .filter(|ty| derived_trait_emits_method_call(trait_kind, *ty, pool))
                .count();
        }
        _ if trait_kind == DerivedTrait::Clone => {}
        _ => return None,
    }
    Some(count)
}
