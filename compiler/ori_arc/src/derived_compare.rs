//! Shared ARC body generation for compiler-derived `Comparable` methods.
//!
//! Generated bodies use ordinary projections, calls, constructors, and control
//! flow, then enter the same AIMS pipeline as source functions. This module
//! does not emit ownership events or select a physical backend.

use ori_ir::{builtin_constants::ordering, Name};
use ori_types::{FunctionSig, Idx, Pool, Tag};

use crate::classify::ArcClassifier;
use crate::derived_body::{validate_concrete_type, RETURN_TYPE, SELF_PARAMETER};
use crate::ir::{
    compute_var_reprs, ArcBlockId, ArcFunction, ArcParam, ArcVarId, CtorKind, MethodCallForm,
};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

const OTHER_PARAMETER: &str = "other parameter";

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
        /// Integer conversion failure, when conversion caused the overflow.
        source: Option<std::num::TryFromIntError>,
    },
}

crate::derived_body::impl_concrete_type_error_conversion!(DerivedCompareBodyError);

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
                ..
            } => write!(
                formatter,
                "derived Comparable body for {receiver_type:?} has {index_kind} index {index}, which exceeds the ARC IR index range"
            ),
        }
    }
}

impl std::error::Error for DerivedCompareBodyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IndexOverflow {
                source: Some(source),
                ..
            } => Some(source),
            _ => None,
        }
    }
}

/// Build one concrete derived `Comparable.compare` body as shared ARC flow.
///
/// Struct fields compare left-to-right. Enum declaration ordinals compare
/// first; equal variants then compare their active payload fields in order.
/// Every comparison is an ordinary instance call carrying exact receiver
/// provenance. `Ordering` values are ordinary enum constructions or call
/// results, and AIMS remains responsible for all ownership and cleanup events.
#[must_use = "success or failure must be handled"]
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
    validate_concrete_type(pool, SELF_PARAMETER, self_type)
        .map_err(DerivedCompareBodyError::from)?;
    validate_concrete_type(pool, OTHER_PARAMETER, other_type)
        .map_err(DerivedCompareBodyError::from)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type)
        .map_err(DerivedCompareBodyError::from)?;
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

    {
        let mut emitter = BodyEmitter {
            builder: &mut builder,
            receiver,
            other,
            receiver_type: resolved_receiver,
            method_name,
            equal,
            pool,
        };
        match pool.tag(resolved_receiver) {
            Tag::Struct => emitter.emit_struct()?,
            Tag::Enum => emitter.emit_enum()?,
            tag => {
                return Err(DerivedCompareBodyError::UnsupportedReceiverType {
                    receiver_type: self_type,
                    tag,
                });
            }
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

struct BodyEmitter<'builder, 'pool> {
    builder: &'builder mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    equal: ArcBlockId,
    pool: &'pool Pool,
}

impl BodyEmitter<'_, '_> {
    fn emit_struct(&mut self) -> Result<(), DerivedCompareBodyError> {
        let fields: Vec<_> = self
            .pool
            .struct_fields(self.receiver_type)
            .into_iter()
            .map(|(_, field_type)| field_type)
            .collect();
        self.emit_field_chain(&fields, 0)
    }

    fn emit_enum(&mut self) -> Result<(), DerivedCompareBodyError> {
        let receiver_tag = self.builder.emit_project(Idx::INT, self.receiver, 0, None);
        let other_tag = self.builder.emit_project(Idx::INT, self.other, 0, None);
        let dispatch = self.builder.new_block();
        self.emit_compare_or_return(receiver_tag, other_tag, Idx::INT, dispatch);

        let invalid_tag = self.builder.new_block();
        let variants = self.pool.enum_variants(self.receiver_type);
        let mut cases = Vec::with_capacity(variants.len());
        let mut arms = Vec::with_capacity(variants.len());
        for (variant_index, (_, fields)) in variants.into_iter().enumerate() {
            let switch_value = u64::try_from(variant_index).map_err(|source| {
                DerivedCompareBodyError::IndexOverflow {
                    receiver_type: self.receiver_type,
                    index_kind: "variant",
                    index: variant_index,
                    source: Some(source),
                }
            })?;
            let block = self.builder.new_block();
            cases.push((switch_value, block));
            arms.push((block, fields));
        }

        self.builder.position_at(dispatch);
        self.builder
            .terminate_switch(receiver_tag, cases, invalid_tag);
        self.builder.position_at(invalid_tag);
        self.builder.terminate_unreachable();
        for (block, fields) in arms {
            self.builder.position_at(block);
            self.emit_field_chain(&fields, 1)?;
        }
        Ok(())
    }

    fn emit_field_chain(
        &mut self,
        fields: &[Idx],
        field_base: u32,
    ) -> Result<(), DerivedCompareBodyError> {
        let comparable_fields: Vec<_> = fields
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, field_type)| {
                self.pool.tag(self.pool.resolve_fully(*field_type)) != Tag::Unit
            })
            .collect();
        if comparable_fields.is_empty() {
            self.builder.terminate_jump(self.equal, Vec::new());
            return Ok(());
        }

        for (position, (field_index, field_type)) in comparable_fields.iter().copied().enumerate() {
            let offset = u32::try_from(field_index).map_err(|source| {
                DerivedCompareBodyError::IndexOverflow {
                    receiver_type: self.receiver_type,
                    index_kind: "field",
                    index: field_index,
                    source: Some(source),
                }
            })?;
            let projection =
                field_base
                    .checked_add(offset)
                    .ok_or(DerivedCompareBodyError::IndexOverflow {
                        receiver_type: self.receiver_type,
                        index_kind: "field",
                        index: field_index,
                        source: None,
                    })?;

            let receiver_field =
                self.builder
                    .emit_project(field_type, self.receiver, projection, None);

            let other_field = self
                .builder
                .emit_project(field_type, self.other, projection, None);
            let is_last = position + 1 == comparable_fields.len();
            let next = if is_last {
                self.equal
            } else {
                self.builder.new_block()
            };
            self.emit_compare_or_return(receiver_field, other_field, field_type, next);
            if !is_last {
                self.builder.position_at(next);
            }
        }
        Ok(())
    }

    fn emit_compare_or_return(
        &mut self,
        receiver: ArcVarId,
        other: ArcVarId,
        receiver_type: Idx,
        equal: ArcBlockId,
    ) {
        let comparison = self.builder.emit_invoke(
            Idx::ORDERING,
            self.method_name,
            vec![receiver, other],
            None,
            None,
        );

        self.builder
            .note_method_call(comparison, receiver_type, MethodCallForm::Instance);
        let comparison_tag = self.builder.emit_project(Idx::INT, comparison, 0, None);
        let non_equal = self.builder.new_block();
        self.builder.terminate_switch(
            comparison_tag,
            vec![(ordering::unsigned::EQUAL, equal)],
            non_equal,
        );
        self.builder.position_at(non_equal);
        self.builder.terminate_return(comparison);
    }
}

fn ordering_variant(value: i8) -> u32 {
    let Ok(variant) = u32::try_from(value) else {
        panic!("Ordering discriminants must be non-negative")
    };
    variant
}

#[cfg(test)]
mod tests;
