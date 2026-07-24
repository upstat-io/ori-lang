//! Shared ARC body generation for compiler-derived `Hashable` methods.
//!
//! Generated bodies use ordinary projections and calls, then enter the same
//! AIMS pipeline as source functions. This module does not emit ownership
//! events or select a physical backend.

use ori_ir::Name;
use ori_types::{FunctionSig, Idx, Pool, Tag};

use crate::classify::ArcClassifier;
use crate::derived_body::{validate_concrete_type, RETURN_TYPE, SELF_PARAMETER};
use crate::ir::{
    compute_var_reprs, ArcFunction, ArcParam, ArcValue, ArcVarId, LitValue, MethodCallForm,
};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

/// Invalid input to a compiler-derived `Hashable.hash` body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedHashBodyError {
    /// The supplied signature still has generic parameters.
    GenericSignature {
        /// Number of type parameters in the signature.
        type_parameters: usize,
        /// Number of const parameters in the signature.
        const_parameters: usize,
    },
    /// `Hashable.hash` must have exactly one named receiver parameter.
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
    /// `Hashable.hash` must return `int`.
    ReturnTypeMismatch {
        /// Actual return type.
        return_type: Idx,
    },
    /// Derived `Hashable` is defined only for concrete products, sums, and newtypes.
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

crate::derived_body::impl_concrete_type_error_conversion!(DerivedHashBodyError);

impl std::fmt::Display for DerivedHashBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericSignature {
                type_parameters,
                const_parameters,
            } => write!(
                formatter,
                "derived Hashable body requires a concrete signature, found {type_parameters} type and {const_parameters} const parameters"
            ),
            Self::InvalidParameterShape {
                parameter_names,
                parameter_types,
            } => write!(
                formatter,
                "derived Hashable body requires one named self parameter, found {parameter_names} names and {parameter_types} types"
            ),
            Self::InvalidTypeIndex { position, ty } => write!(
                formatter,
                "derived Hashable body {position} has an invalid type index {ty:?}"
            ),
            Self::NonConcreteType { position, ty } => write!(
                formatter,
                "derived Hashable body {position} must be concrete, found {ty:?}"
            ),
            Self::ReturnTypeMismatch { return_type } => write!(
                formatter,
                "derived Hashable body must return int, found {return_type:?}"
            ),
            Self::UnsupportedReceiverType { receiver_type, tag } => write!(
                formatter,
                "derived Hashable body requires a concrete struct, enum, or newtype receiver, found {receiver_type:?} with shape {tag:?}"
            ),
            Self::IndexOverflow {
                receiver_type,
                index_kind,
                index,
                ..
            } => write!(
                formatter,
                "derived Hashable body for {receiver_type:?} has {index_kind} index {index}, which exceeds the ARC IR index range"
            ),
        }
    }
}

impl std::error::Error for DerivedHashBodyError {
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

/// Build one concrete derived `Hashable.hash` body as shared ARC flow.
///
/// Products fold non-Unit fields in declaration order from seed zero. Sums
/// first fold their zero-based declaration ordinal, then the active non-Unit
/// payload fields. Newtypes delegate directly to the underlying value's hash.
pub fn build_derived_hash(
    executable_name: Name,
    owner_name: Name,
    method_name: Name,
    hash_combine_name: Name,
    signature: &FunctionSig,
    pool: &Pool,
) -> Result<ArcFunction, DerivedHashBodyError> {
    if signature.is_generic() {
        return Err(DerivedHashBodyError::GenericSignature {
            type_parameters: signature.type_params.len(),
            const_parameters: signature.const_params.len(),
        });
    }
    if signature.param_names.len() != 1 || signature.param_types.len() != 1 {
        return Err(DerivedHashBodyError::InvalidParameterShape {
            parameter_names: signature.param_names.len(),
            parameter_types: signature.param_types.len(),
        });
    }

    let receiver_type = signature.param_types[0];
    validate_concrete_type(pool, SELF_PARAMETER, receiver_type)
        .map_err(DerivedHashBodyError::from)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type)
        .map_err(DerivedHashBodyError::from)?;
    if !pool.structural_eq(signature.return_type, Idx::INT) {
        return Err(DerivedHashBodyError::ReturnTypeMismatch {
            return_type: signature.return_type,
        });
    }

    let resolved_receiver = pool.resolve_fully(receiver_type);
    let mut builder = ArcIrBuilder::new();
    let entry = builder.entry_block();
    let receiver = builder.fresh_var(receiver_type);

    if pool.is_newtype_ctor(owner_name) {
        let underlying = builder.emit_let(resolved_receiver, ArcValue::Var(receiver), None);
        let result = emit_field_hash(&mut builder, underlying, resolved_receiver, method_name);
        builder.terminate_return(result);
    } else {
        match pool.tag(resolved_receiver) {
            Tag::Struct => {
                let seed = emit_zero(&mut builder);
                let fields: Vec<_> = pool
                    .struct_fields(resolved_receiver)
                    .into_iter()
                    .map(|(_, field_type)| field_type)
                    .collect();
                let result = emit_field_hash_fold(
                    &mut builder,
                    receiver,
                    resolved_receiver,
                    &fields,
                    0,
                    seed,
                    method_name,
                    hash_combine_name,
                    pool,
                )?;
                builder.terminate_return(result);
            }
            Tag::Enum => emit_enum_hash(
                &mut builder,
                receiver,
                resolved_receiver,
                method_name,
                hash_combine_name,
                pool,
            )?,
            tag => {
                return Err(DerivedHashBodyError::UnsupportedReceiverType { receiver_type, tag });
            }
        }
    }

    let mut function = builder.finish(
        executable_name,
        vec![ArcParam {
            var: receiver,
            ty: receiver_type,
            ownership: Ownership::Owned,
        }],
        signature.return_type,
        entry,
        false,
    );
    let classifier = ArcClassifier::new(pool);
    let representations = compute_var_reprs(&function, &classifier, pool);
    function.replace_variable_representations(representations);
    Ok(function)
}

fn emit_enum_hash(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    hash_combine_name: Name,
    pool: &Pool,
) -> Result<(), DerivedHashBodyError> {
    let tag = builder.emit_project(Idx::INT, receiver, 0, None);
    let seed = emit_zero(builder);
    let tagged = emit_hash_combine(builder, seed, tag, hash_combine_name);
    let invalid_tag = builder.new_block();
    let variants = pool.enum_variants(receiver_type);
    let mut cases = Vec::with_capacity(variants.len());
    let mut arms = Vec::with_capacity(variants.len());
    for (variant_index, (_, fields)) in variants.into_iter().enumerate() {
        let switch_value =
            u64::try_from(variant_index).map_err(|source| DerivedHashBodyError::IndexOverflow {
                receiver_type,
                index_kind: "variant",
                index: variant_index,
                source: Some(source),
            })?;
        let block = builder.new_block();
        cases.push((switch_value, block));
        arms.push((block, fields));
    }
    builder.terminate_switch(tag, cases, invalid_tag);
    builder.position_at(invalid_tag);
    builder.terminate_unreachable();

    for (block, fields) in arms {
        builder.position_at(block);
        let result = emit_field_hash_fold(
            builder,
            receiver,
            receiver_type,
            &fields,
            1,
            tagged,
            method_name,
            hash_combine_name,
            pool,
        )?;
        builder.terminate_return(result);
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "hash folding carries receiver shape, projection base, accumulator, and exact calls"
)]
fn emit_field_hash_fold(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    receiver_type: Idx,
    fields: &[Idx],
    field_base: u32,
    mut accumulator: ArcVarId,
    method_name: Name,
    hash_combine_name: Name,
    pool: &Pool,
) -> Result<ArcVarId, DerivedHashBodyError> {
    for (field_index, field_type) in fields.iter().copied().enumerate() {
        if pool.tag(pool.resolve_fully(field_type)) == Tag::Unit {
            continue;
        }
        let offset =
            u32::try_from(field_index).map_err(|source| DerivedHashBodyError::IndexOverflow {
                receiver_type,
                index_kind: "field",
                index: field_index,
                source: Some(source),
            })?;
        let projection =
            field_base
                .checked_add(offset)
                .ok_or(DerivedHashBodyError::IndexOverflow {
                    receiver_type,
                    index_kind: "field",
                    index: field_index,
                    source: None,
                })?;
        let field = builder.emit_project(field_type, receiver, projection, None);
        let field_hash = emit_field_hash(builder, field, field_type, method_name);
        accumulator = emit_hash_combine(builder, accumulator, field_hash, hash_combine_name);
    }
    Ok(accumulator)
}

fn emit_field_hash(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
) -> ArcVarId {
    let result = builder.emit_invoke(Idx::INT, method_name, vec![receiver], None, None);
    builder.note_method_call(result, receiver_type, MethodCallForm::Instance);
    result
}

fn emit_hash_combine(
    builder: &mut ArcIrBuilder,
    accumulated: ArcVarId,
    next: ArcVarId,
    hash_combine_name: Name,
) -> ArcVarId {
    builder.emit_apply(
        Idx::INT,
        hash_combine_name,
        vec![accumulated, next],
        None,
        None,
    )
}

fn emit_zero(builder: &mut ArcIrBuilder) -> ArcVarId {
    builder.emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None)
}

#[cfg(test)]
mod tests;
