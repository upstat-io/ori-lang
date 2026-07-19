//! Shared ARC body generation for compiler-derived `Comparable` methods.
//!
//! Generated bodies use ordinary projections, calls, constructors, and control
//! flow, then enter the same AIMS pipeline as source functions. This module
//! does not emit ownership events or select a physical backend.

use ori_ir::{builtin_constants::ordering, Name};
use ori_types::{FunctionSig, Idx, Pool, Tag, TypeFlags};

use crate::classify::ArcClassifier;
use crate::ir::{
    compute_var_reprs, ArcBlockId, ArcFunction, ArcParam, ArcVarId, CtorKind, MethodCallForm,
};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

const SELF_PARAMETER: &str = "self parameter";
const OTHER_PARAMETER: &str = "other parameter";
const RETURN_TYPE: &str = "return type";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConcreteTypeError {
    InvalidTypeIndex { position: &'static str, ty: Idx },
    NonConcreteType { position: &'static str, ty: Idx },
}

/// Invalid input to a compiler-derived `Comparable.compare` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedCompareBodyError {
    /// The supplied signature still has generic parameters.
    GenericSignature {
        /// Number of type parameters in the signature.
        type_parameters: usize,
        /// Number of const parameters in the signature.
        const_parameters: usize,
    },
    /// `Comparable.compare` must have exactly two named parameters.
    InvalidParameterShape {
        /// Number of parameter names in the signature.
        parameter_names: usize,
        /// Number of parameter types in the signature.
        parameter_types: usize,
    },
    /// A type index does not belong to the supplied type pool.
    InvalidTypeIndex {
        /// Signature position containing the invalid index.
        position: &'static str,
        /// Invalid type-pool index.
        ty: Idx,
    },
    /// A signature position contains an unresolved, generic, or poison type.
    NonConcreteType {
        /// Signature position containing the non-concrete type.
        position: &'static str,
        /// Non-concrete type-pool index.
        ty: Idx,
    },
    /// The self and other parameters do not denote the same concrete type.
    ReceiverTypeMismatch {
        /// Self parameter type.
        self_type: Idx,
        /// Other parameter type.
        other_type: Idx,
    },
    /// `Comparable.compare` must return `Ordering`.
    ReturnTypeMismatch {
        /// Actual return type.
        return_type: Idx,
    },
    /// Derived `Comparable` is defined only for concrete products and sums.
    UnsupportedReceiverType {
        /// Concrete receiver type.
        receiver_type: Idx,
        /// Resolved receiver shape.
        tag: Tag,
    },
    /// A field or variant index cannot fit the ARC IR carrier.
    IndexOverflow {
        /// Concrete receiver type whose shape exceeded the carrier.
        receiver_type: Idx,
        /// Kind of index that overflowed.
        index_kind: &'static str,
        /// Source shape index.
        index: usize,
    },
}

impl std::fmt::Display for DerivedCompareBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericSignature {
                type_parameters,
                const_parameters,
            } => write!(
                formatter,
                "derived Comparable body requires a concrete signature, found {type_parameters} type and {const_parameters} const parameters"
            ),
            Self::InvalidParameterShape {
                parameter_names,
                parameter_types,
            } => write!(
                formatter,
                "derived Comparable body requires named self and other parameters, found {parameter_names} names and {parameter_types} types"
            ),
            Self::InvalidTypeIndex { position, ty } => write!(
                formatter,
                "derived Comparable body {position} has an invalid type index {ty:?}"
            ),
            Self::NonConcreteType { position, ty } => write!(
                formatter,
                "derived Comparable body {position} must be concrete, found {ty:?}"
            ),
            Self::ReceiverTypeMismatch {
                self_type,
                other_type,
            } => write!(
                formatter,
                "derived Comparable parameters must have the same concrete type: {self_type:?} does not match {other_type:?}"
            ),
            Self::ReturnTypeMismatch { return_type } => write!(
                formatter,
                "derived Comparable body must return Ordering, found {return_type:?}"
            ),
            Self::UnsupportedReceiverType { receiver_type, tag } => write!(
                formatter,
                "derived Comparable body requires a concrete struct or enum receiver, found {receiver_type:?} with shape {tag:?}"
            ),
            Self::IndexOverflow {
                receiver_type,
                index_kind,
                index,
            } => write!(
                formatter,
                "derived Comparable body for {receiver_type:?} has {index_kind} index {index}, which exceeds the ARC IR index range"
            ),
        }
    }
}

impl std::error::Error for DerivedCompareBodyError {}

/// Build one concrete derived `Comparable.compare` body as shared ARC flow.
///
/// Struct fields compare left-to-right. Enum declaration ordinals compare
/// first; equal variants then compare their active payload fields in order.
/// Every comparison is an ordinary instance call carrying exact receiver
/// provenance. `Ordering` values are ordinary enum constructions or call
/// results, and AIMS remains responsible for all ownership and cleanup events.
pub fn build_derived_compare(
    executable_name: Name,
    ordering_name: Name,
    method_name: Name,
    signature: &FunctionSig,
    pool: &Pool,
) -> Result<ArcFunction, DerivedCompareBodyError> {
    if signature.is_generic() {
        return Err(DerivedCompareBodyError::GenericSignature {
            type_parameters: signature.type_params.len(),
            const_parameters: signature.const_params.len(),
        });
    }
    if signature.param_names.len() != 2 || signature.param_types.len() != 2 {
        return Err(DerivedCompareBodyError::InvalidParameterShape {
            parameter_names: signature.param_names.len(),
            parameter_types: signature.param_types.len(),
        });
    }

    let self_type = signature.param_types[0];
    let other_type = signature.param_types[1];
    validate_concrete_type(pool, SELF_PARAMETER, self_type).map_err(map_type_error)?;
    validate_concrete_type(pool, OTHER_PARAMETER, other_type).map_err(map_type_error)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type).map_err(map_type_error)?;
    if !pool.structural_eq(self_type, other_type) {
        return Err(DerivedCompareBodyError::ReceiverTypeMismatch {
            self_type,
            other_type,
        });
    }
    if !pool.structural_eq(signature.return_type, Idx::ORDERING) {
        return Err(DerivedCompareBodyError::ReturnTypeMismatch {
            return_type: signature.return_type,
        });
    }

    let resolved_receiver = pool.resolve_fully(self_type);
    let mut builder = ArcIrBuilder::new();
    let entry = builder.entry_block();
    let receiver = builder.fresh_var(self_type);
    let other = builder.fresh_var(other_type);
    let equal = builder.new_block();

    match pool.tag(resolved_receiver) {
        Tag::Struct => emit_struct_compare(
            &mut builder,
            receiver,
            other,
            resolved_receiver,
            method_name,
            equal,
            pool,
        )?,
        Tag::Enum => emit_enum_compare(
            &mut builder,
            receiver,
            other,
            resolved_receiver,
            method_name,
            equal,
            pool,
        )?,
        tag => {
            return Err(DerivedCompareBodyError::UnsupportedReceiverType {
                receiver_type: self_type,
                tag,
            });
        }
    }

    builder.position_at(equal);
    let result = builder.emit_construct(
        Idx::ORDERING,
        CtorKind::EnumVariant {
            enum_name: ordering_name,
            variant: ordering_variant(ordering::EQUAL),
        },
        Vec::new(),
        None,
    );
    builder.terminate_return(result);

    let mut function = builder.finish(
        executable_name,
        vec![
            ArcParam {
                var: receiver,
                ty: self_type,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: other,
                ty: other_type,
                ownership: Ownership::Owned,
            },
        ],
        signature.return_type,
        entry,
        false,
    );
    let classifier = ArcClassifier::new(pool);
    let representations = compute_var_reprs(&function, &classifier, pool);
    function.replace_variable_representations(representations);
    Ok(function)
}

fn emit_struct_compare(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    equal: ArcBlockId,
    pool: &Pool,
) -> Result<(), DerivedCompareBodyError> {
    let fields: Vec<_> = pool
        .struct_fields(receiver_type)
        .into_iter()
        .map(|(_, field_type)| field_type)
        .collect();
    emit_field_compare_chain(
        builder,
        receiver,
        other,
        receiver_type,
        &fields,
        0,
        method_name,
        equal,
        pool,
    )
}

fn emit_enum_compare(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    equal: ArcBlockId,
    pool: &Pool,
) -> Result<(), DerivedCompareBodyError> {
    let receiver_tag = builder.emit_project(Idx::INT, receiver, 0, None);
    let other_tag = builder.emit_project(Idx::INT, other, 0, None);
    let dispatch = builder.new_block();
    emit_compare_or_return(
        builder,
        receiver_tag,
        other_tag,
        Idx::INT,
        method_name,
        dispatch,
    );

    let invalid_tag = builder.new_block();
    let variants = pool.enum_variants(receiver_type);
    let mut cases = Vec::with_capacity(variants.len());
    let mut arms = Vec::with_capacity(variants.len());
    for (variant_index, (_, fields)) in variants.into_iter().enumerate() {
        let switch_value =
            u64::try_from(variant_index).map_err(|_| DerivedCompareBodyError::IndexOverflow {
                receiver_type,
                index_kind: "variant",
                index: variant_index,
            })?;
        let block = builder.new_block();
        cases.push((switch_value, block));
        arms.push((block, fields));
    }

    builder.position_at(dispatch);
    builder.terminate_switch(receiver_tag, cases, invalid_tag);
    builder.position_at(invalid_tag);
    builder.terminate_unreachable();
    for (block, fields) in arms {
        builder.position_at(block);
        emit_field_compare_chain(
            builder,
            receiver,
            other,
            receiver_type,
            &fields,
            1,
            method_name,
            equal,
            pool,
        )?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "one lexicographic field chain carries operands, shape, projection base, method, and exit"
)]
fn emit_field_compare_chain(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    fields: &[Idx],
    field_base: u32,
    method_name: Name,
    equal: ArcBlockId,
    pool: &Pool,
) -> Result<(), DerivedCompareBodyError> {
    let comparable_fields: Vec<_> = fields
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, field_type)| pool.tag(pool.resolve_fully(*field_type)) != Tag::Unit)
        .collect();
    if comparable_fields.is_empty() {
        builder.terminate_jump(equal, Vec::new());
        return Ok(());
    }

    for (position, (field_index, field_type)) in comparable_fields.iter().copied().enumerate() {
        let offset =
            u32::try_from(field_index).map_err(|_| DerivedCompareBodyError::IndexOverflow {
                receiver_type,
                index_kind: "field",
                index: field_index,
            })?;
        let projection =
            field_base
                .checked_add(offset)
                .ok_or(DerivedCompareBodyError::IndexOverflow {
                    receiver_type,
                    index_kind: "field",
                    index: field_index,
                })?;
        let receiver_field = builder.emit_project(field_type, receiver, projection, None);
        let other_field = builder.emit_project(field_type, other, projection, None);
        let is_last = position + 1 == comparable_fields.len();
        let next = if is_last { equal } else { builder.new_block() };
        emit_compare_or_return(
            builder,
            receiver_field,
            other_field,
            field_type,
            method_name,
            next,
        );
        if !is_last {
            builder.position_at(next);
        }
    }
    Ok(())
}

fn emit_compare_or_return(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    equal: ArcBlockId,
) {
    let comparison = builder.emit_invoke(
        Idx::ORDERING,
        method_name,
        vec![receiver, other],
        None,
        None,
    );
    builder.note_method_call(comparison, receiver_type, MethodCallForm::Instance);
    let comparison_tag = builder.emit_project(Idx::INT, comparison, 0, None);
    let non_equal = builder.new_block();
    builder.terminate_switch(
        comparison_tag,
        vec![(ordering::unsigned::EQUAL, equal)],
        non_equal,
    );
    builder.position_at(non_equal);
    builder.terminate_return(comparison);
}

fn ordering_variant(value: i8) -> u32 {
    let Ok(variant) = u32::try_from(value) else {
        panic!("Ordering discriminants must be non-negative")
    };
    variant
}

fn map_type_error(error: ConcreteTypeError) -> DerivedCompareBodyError {
    match error {
        ConcreteTypeError::InvalidTypeIndex { position, ty } => {
            DerivedCompareBodyError::InvalidTypeIndex { position, ty }
        }
        ConcreteTypeError::NonConcreteType { position, ty } => {
            DerivedCompareBodyError::NonConcreteType { position, ty }
        }
    }
}

fn validate_concrete_type(
    pool: &Pool,
    position: &'static str,
    ty: Idx,
) -> Result<(), ConcreteTypeError> {
    if !pool.is_valid_idx(ty) {
        return Err(ConcreteTypeError::InvalidTypeIndex { position, ty });
    }
    let resolved = pool.resolve_fully(ty);
    if !pool.is_valid_idx(resolved) {
        return Err(ConcreteTypeError::InvalidTypeIndex {
            position,
            ty: resolved,
        });
    }
    let flags = pool.flags(resolved);
    let unresolved = TypeFlags::HAS_SELF | TypeFlags::HAS_PROJECTION;
    if !flags.is_recordable()
        || flags.intersects(unresolved)
        || matches!(pool.tag(resolved), Tag::Scheme | Tag::ModuleNs)
    {
        return Err(ConcreteTypeError::NonConcreteType { position, ty });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use ori_ir::{builtin_constants::ordering, Name, StringInterner};
    use ori_types::{ConstParamInfo, EnumVariant, FunctionSig, Idx, Pool, Tag};

    use crate::ir::{ArcInstr, ArcTerminator, CtorKind, MethodCallFact, MethodCallForm};
    use crate::{Ownership, VariableMetadataState};

    use super::{build_derived_compare, DerivedCompareBodyError};

    const EXECUTABLE: Name = Name::from_raw(1);
    const METHOD: Name = Name::from_raw(2);
    const SELF_NAME: Name = Name::from_raw(3);
    const OTHER_NAME: Name = Name::from_raw(4);
    const SIGNATURE_NAME: Name = Name::from_raw(5);

    fn signature(receiver: Idx, other: Idx, return_type: Idx) -> FunctionSig {
        FunctionSig::synthetic(
            SIGNATURE_NAME,
            vec![SELF_NAME, OTHER_NAME],
            vec![receiver, other],
            return_type,
        )
    }

    fn body_or_panic(
        ordering_name: Name,
        signature: &FunctionSig,
        pool: &Pool,
    ) -> crate::ArcFunction {
        match build_derived_compare(EXECUTABLE, ordering_name, METHOD, signature, pool) {
            Ok(body) => body,
            Err(error) => panic!("concrete Comparable derive must produce a shared body: {error}"),
        }
    }

    fn invokes(body: &crate::ArcFunction) -> Vec<(crate::ArcVarId, Name)> {
        body.blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                ArcTerminator::Invoke { dst, func, .. } => Some((*dst, *func)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn struct_fields_compare_lexicographically_with_exact_provenance() {
        let interner = StringInterner::new();
        let ordering_name = interner.intern("Ordering");
        let mut pool = Pool::new();
        let receiver = pool.struct_type(
            interner.intern("Pair"),
            &[
                (interner.intern("first"), Idx::INT),
                (interner.intern("second"), Idx::STR),
            ],
        );
        let body = body_or_panic(
            ordering_name,
            &signature(receiver, receiver, Idx::ORDERING),
            &pool,
        );

        assert_eq!(body.params.len(), 2);
        assert!(body.params.iter().all(|parameter| {
            parameter.ty == receiver && parameter.ownership == Ownership::Owned
        }));
        assert_eq!(body.return_type, Idx::ORDERING);
        assert_eq!(
            body.var_metadata_state,
            VariableMetadataState::RepresentationsReady
        );

        let calls = invokes(&body);
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|(_, function)| *function == METHOD));
        assert_eq!(
            body.method_call_facts,
            vec![
                MethodCallFact {
                    destination: calls[0].0,
                    receiver_type: Idx::INT,
                    form: MethodCallForm::Instance,
                    producer: None,
                    selected_producer: None,
                    derived_position: None,
                },
                MethodCallFact {
                    destination: calls[1].0,
                    receiver_type: Idx::STR,
                    form: MethodCallForm::Instance,
                    producer: None,
                    selected_producer: None,
                    derived_position: None,
                },
            ]
        );
        assert_eq!(
            body.blocks
                .iter()
                .filter(|block| matches!(block.terminator, ArcTerminator::Switch { .. }))
                .count(),
            2
        );
        assert!(body
            .blocks
            .iter()
            .flat_map(|block| &block.body)
            .all(|instruction| matches!(
                instruction,
                ArcInstr::Project { .. } | ArcInstr::Construct { .. }
            )));
    }

    #[test]
    fn enum_compares_declaration_ordinal_before_active_payload() {
        let interner = StringInterner::new();
        let ordering_name = interner.intern("Ordering");
        let mut pool = Pool::new();
        let receiver = pool.enum_type(
            interner.intern("Choice"),
            &[
                EnumVariant {
                    name: interner.intern("Empty"),
                    field_types: Vec::new(),
                },
                EnumVariant {
                    name: interner.intern("Pair"),
                    field_types: vec![Idx::INT, Idx::BOOL],
                },
            ],
        );
        let body = body_or_panic(
            ordering_name,
            &signature(receiver, receiver, Idx::ORDERING),
            &pool,
        );

        let calls = invokes(&body);
        assert_eq!(calls.len(), 3);
        assert_eq!(
            body.method_call_facts,
            vec![
                MethodCallFact {
                    destination: calls[0].0,
                    receiver_type: Idx::INT,
                    form: MethodCallForm::Instance,
                    producer: None,
                    selected_producer: None,
                    derived_position: None,
                },
                MethodCallFact {
                    destination: calls[1].0,
                    receiver_type: Idx::INT,
                    form: MethodCallForm::Instance,
                    producer: None,
                    selected_producer: None,
                    derived_position: None,
                },
                MethodCallFact {
                    destination: calls[2].0,
                    receiver_type: Idx::BOOL,
                    form: MethodCallForm::Instance,
                    producer: None,
                    selected_producer: None,
                    derived_position: None,
                },
            ]
        );

        let variant_switch = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                ArcTerminator::Switch { cases, default, .. }
                    if cases.len() == 2 && cases[0].0 == 0 && cases[1].0 == 1 =>
                {
                    Some((cases, *default))
                }
                _ => None,
            });
        let Some((cases, default)) = variant_switch else {
            panic!("enum Comparable body must dispatch in declaration order")
        };
        assert!(matches!(
            body.blocks[default.index()].terminator,
            ArcTerminator::Unreachable
        ));
        assert!(matches!(
            body.blocks[cases[0].1.index()].terminator,
            ArcTerminator::Jump { .. }
        ));
        assert!(body.blocks[cases[1].1.index()]
            .body
            .iter()
            .any(|instruction| matches!(instruction, ArcInstr::Project { field: 1, .. })));
    }

    #[test]
    fn equal_result_is_an_ordinary_ordering_variant_construct() {
        let interner = StringInterner::new();
        let ordering_name = interner.intern("Ordering");
        let mut pool = Pool::new();
        let receiver = pool.struct_type(interner.intern("UnitLike"), &[]);
        let body = body_or_panic(
            ordering_name,
            &signature(receiver, receiver, Idx::ORDERING),
            &pool,
        );

        assert!(body.method_call_facts.is_empty());
        let construct = body
            .blocks
            .iter()
            .flat_map(|block| &block.body)
            .find_map(|instruction| match instruction {
                ArcInstr::Construct { ty, ctor, args, .. } => Some((*ty, *ctor, args)),
                _ => None,
            });
        assert!(matches!(
            construct,
            Some((
                Idx::ORDERING,
                CtorKind::EnumVariant {
                    enum_name,
                    variant,
                },
                args,
            )) if enum_name == ordering_name
                && u64::from(variant) == ordering::unsigned::EQUAL
                && args.is_empty()
        ));
        assert_eq!(ordering::unsigned::LESS, 0);
        assert_eq!(ordering::unsigned::EQUAL, 1);
        assert_eq!(ordering::unsigned::GREATER, 2);
    }

    #[test]
    fn unit_fields_are_equal_without_an_unregistered_method_call() {
        let interner = StringInterner::new();
        let ordering_name = interner.intern("Ordering");
        let mut pool = Pool::new();
        let receiver = pool.struct_type(
            interner.intern("UnitField"),
            &[(interner.intern("value"), Idx::UNIT)],
        );
        let body = body_or_panic(
            ordering_name,
            &signature(receiver, receiver, Idx::ORDERING),
            &pool,
        );

        assert!(body.method_call_facts.is_empty());
        assert!(invokes(&body).is_empty());
        assert!(body.blocks.iter().any(|block| matches!(
            block.terminator,
            ArcTerminator::Jump { target, .. }
                if body.blocks[target.index()].body.iter().any(|instruction| matches!(
                    instruction,
                    ArcInstr::Construct {
                        ty: Idx::ORDERING,
                        ..
                    }
                ))
        )));
    }

    #[test]
    fn invalid_signatures_fail_closed() {
        let interner = StringInterner::new();
        let ordering_name = interner.intern("Ordering");
        let mut pool = Pool::new();
        let receiver = pool.struct_type(interner.intern("Item"), &[]);
        let other = pool.struct_type(interner.intern("Other"), &[]);

        let mut generic = signature(receiver, receiver, Idx::ORDERING);
        generic.type_params.push(Name::from_raw(20));
        generic.const_params.push(ConstParamInfo {
            name: Name::from_raw(21),
            const_type: Idx::INT,
            default_value: None,
        });
        assert_eq!(
            build_derived_compare(EXECUTABLE, ordering_name, METHOD, &generic, &pool),
            Err(DerivedCompareBodyError::GenericSignature {
                type_parameters: 1,
                const_parameters: 1,
            })
        );
        assert!(matches!(
            build_derived_compare(
                EXECUTABLE,
                ordering_name,
                METHOD,
                &signature(receiver, other, Idx::ORDERING),
                &pool,
            ),
            Err(DerivedCompareBodyError::ReceiverTypeMismatch { .. })
        ));
        assert_eq!(
            build_derived_compare(
                EXECUTABLE,
                ordering_name,
                METHOD,
                &signature(receiver, receiver, Idx::BOOL),
                &pool,
            ),
            Err(DerivedCompareBodyError::ReturnTypeMismatch {
                return_type: Idx::BOOL,
            })
        );
        assert!(matches!(
            build_derived_compare(
                EXECUTABLE,
                ordering_name,
                METHOD,
                &signature(Idx::INT, Idx::INT, Idx::ORDERING),
                &pool,
            ),
            Err(DerivedCompareBodyError::UnsupportedReceiverType {
                receiver_type: Idx::INT,
                tag: Tag::Int,
            })
        ));
    }
}
