//! Compiler-derived Eq bodies.

use ori_ir::Name;
use ori_types::{FunctionSig, Idx, Pool, Tag};

use crate::classify::{ArcClassification, ArcClassifier};
use crate::ir::{
    compute_var_reprs, ArcBlockId, ArcFunction, ArcParam, ArcValue, ArcVarId, LitValue,
    MethodCallForm, PrimOp,
};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

use super::validation::{validate_concrete_type, RETURN_TYPE, SELF_PARAMETER};

/// Invalid input to a compiler-derived Eq body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedEqBodyError {
    /// The supplied signature still has generic parameters.
    GenericSignature {
        /// Number of type parameters in the signature.
        type_parameters: usize,
        /// Number of const parameters in the signature.
        const_parameters: usize,
    },
    /// Eq must have exactly two named parameters: self and other.
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
    ReceiverTypeMismatch { self_type: Idx, other_type: Idx },
    /// Eq must return bool.
    ReturnTypeMismatch { return_type: Idx },
    /// Derived Eq is defined only for concrete product and sum types.
    UnsupportedReceiverType { receiver_type: Idx, tag: Tag },
    /// A pool-supplied field or variant index cannot fit the ARC IR carrier.
    IndexOverflow {
        receiver_type: Idx,
        index_kind: &'static str,
        index: usize,
    },
}

crate::derived_body::impl_concrete_type_error_conversion!(DerivedEqBodyError);

struct DerivedEqInputs {
    self_type: Idx,
    other_type: Idx,
    resolved_receiver: Idx,
    owner_is_newtype: bool,
}

impl std::fmt::Display for DerivedEqBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericSignature {
                type_parameters,
                const_parameters,
            } => write!(
                formatter,
                "derived Eq body requires a concrete signature, found {type_parameters} type and {const_parameters} const parameters"
            ),
            Self::InvalidParameterShape {
                parameter_names,
                parameter_types,
            } => write!(
                formatter,
                "derived Eq body requires named self and other parameters, found {parameter_names} names and {parameter_types} types"
            ),
            Self::InvalidTypeIndex { position, ty } => write!(
                formatter,
                "derived Eq body {position} has an invalid type index {ty:?}"
            ),
            Self::NonConcreteType { position, ty } => write!(
                formatter,
                "derived Eq body {position} must be concrete, found {ty:?}"
            ),
            Self::ReceiverTypeMismatch {
                self_type,
                other_type,
            } => write!(
                formatter,
                "derived Eq parameters must have the same concrete type: {self_type:?} does not match {other_type:?}"
            ),
            Self::ReturnTypeMismatch { return_type } => write!(
                formatter,
                "derived Eq body must return bool, found {return_type:?}"
            ),
            Self::UnsupportedReceiverType { receiver_type, tag } => write!(
                formatter,
                "derived Eq body requires a concrete struct or enum receiver, found {receiver_type:?} with shape {tag:?}"
            ),
            Self::IndexOverflow {
                receiver_type,
                index_kind,
                index,
            } => write!(
                formatter,
                "derived Eq body for {receiver_type:?} has {index_kind} index {index}, which exceeds the ARC IR index range"
            ),
        }
    }
}

impl std::error::Error for DerivedEqBodyError {}

/// Build one concrete derived Eq body as ordinary shared ARC control/data flow.
///
/// Product fields compare scalar fields before managed fields, preserving
/// declaration order inside each cost class, and short-circuit on inequality.
/// Sum values compare tags before switching to the active payload shape.
/// Builtin field equality remains a typed primitive; user-defined fields carry
/// an exact method-call provenance fact for the shared target-rewrite seam.
/// Newtypes delegate exactly to their underlying value's equality. AIMS remains
/// the sole owner of argument, result, and unwind-cleanup ownership events.
pub fn build_derived_eq(
    executable_name: Name,
    owner_name: Name,
    method_name: Name,
    signature: &FunctionSig,
    pool: &Pool,
) -> Result<ArcFunction, DerivedEqBodyError> {
    let inputs = validate_derived_eq_inputs(owner_name, signature, pool)?;
    let receiver_tag = pool.tag(inputs.resolved_receiver);
    let classifier = ArcClassifier::new(pool);
    let mut builder = ArcIrBuilder::new();
    let entry = builder.entry_block();
    let receiver = builder.fresh_var(inputs.self_type);
    let other = builder.fresh_var(inputs.other_type);

    if inputs.owner_is_newtype {
        let underlying_receiver =
            builder.emit_let(inputs.resolved_receiver, ArcValue::Var(receiver), None);
        let underlying_other =
            builder.emit_let(inputs.resolved_receiver, ArcValue::Var(other), None);
        let result = emit_field_eq(
            &mut builder,
            underlying_receiver,
            underlying_other,
            inputs.resolved_receiver,
            method_name,
            pool,
        );
        builder.terminate_return(result);
    } else {
        let equal = builder.new_block();
        let unequal = builder.new_block();

        {
            let mut emitter = BodyEmitter {
                builder: &mut builder,
                receiver,
                other,
                receiver_type: inputs.resolved_receiver,
                method_name,
                equal,
                unequal,
                pool,
                classifier: &classifier,
            };
            match receiver_tag {
                Tag::Struct => emitter.emit_struct()?,
                Tag::Enum => emitter.emit_enum()?,
                tag => {
                    return Err(DerivedEqBodyError::UnsupportedReceiverType {
                        receiver_type: inputs.self_type,
                        tag,
                    });
                }
            }
        }

        emit_bool_return(&mut builder, equal, true);
        emit_bool_return(&mut builder, unequal, false);
    }
    let mut function = builder.finish(
        executable_name,
        vec![
            ArcParam {
                var: receiver,
                ty: inputs.self_type,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: other,
                ty: inputs.other_type,
                ownership: Ownership::Owned,
            },
        ],
        signature.return_type,
        entry,
        false,
    );

    let representations = compute_var_reprs(&function, &classifier, pool);
    function.replace_variable_representations(representations);
    Ok(function)
}

fn validate_derived_eq_inputs(
    owner_name: Name,
    signature: &FunctionSig,
    pool: &Pool,
) -> Result<DerivedEqInputs, DerivedEqBodyError> {
    if signature.is_generic() {
        return Err(DerivedEqBodyError::GenericSignature {
            type_parameters: signature.type_params.len(),
            const_parameters: signature.const_params.len(),
        });
    }
    if signature.param_names.len() != 2 || signature.param_types.len() != 2 {
        return Err(DerivedEqBodyError::InvalidParameterShape {
            parameter_names: signature.param_names.len(),
            parameter_types: signature.param_types.len(),
        });
    }

    let self_type = signature.param_types[0];
    let other_type = signature.param_types[1];
    validate_concrete_type(pool, SELF_PARAMETER, self_type).map_err(DerivedEqBodyError::from)?;
    validate_concrete_type(pool, "other parameter", other_type)
        .map_err(DerivedEqBodyError::from)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type)
        .map_err(DerivedEqBodyError::from)?;
    let owner_is_newtype = pool.is_newtype_ctor(owner_name);
    let receiver_types_match = if owner_is_newtype {
        self_type == other_type
    } else {
        pool.structural_eq(self_type, other_type)
    };
    if !receiver_types_match {
        return Err(DerivedEqBodyError::ReceiverTypeMismatch {
            self_type,
            other_type,
        });
    }
    if !pool.structural_eq(signature.return_type, Idx::BOOL) {
        return Err(DerivedEqBodyError::ReturnTypeMismatch {
            return_type: signature.return_type,
        });
    }

    Ok(DerivedEqInputs {
        self_type,
        other_type,
        resolved_receiver: pool.resolve_fully(self_type),
        owner_is_newtype,
    })
}

struct BodyEmitter<'builder, 'pool, 'classifier> {
    builder: &'builder mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: Name,
    equal: ArcBlockId,
    unequal: ArcBlockId,
    pool: &'pool Pool,
    classifier: &'classifier ArcClassifier<'pool>,
}

impl BodyEmitter<'_, '_, '_> {
    fn emit_struct(&mut self) -> Result<(), DerivedEqBodyError> {
        let fields: Vec<_> = self
            .pool
            .struct_fields(self.receiver_type)
            .into_iter()
            .map(|(_, field_type)| field_type)
            .collect();
        self.emit_field_chain(&fields, 0)
    }

    fn emit_enum(&mut self) -> Result<(), DerivedEqBodyError> {
        let receiver_tag = self.builder.emit_project(Idx::INT, self.receiver, 0, None);
        let other_tag = self.builder.emit_project(Idx::INT, self.other, 0, None);
        let tags_equal = emit_primitive_eq(self.builder, receiver_tag, other_tag);
        let dispatch = self.builder.new_block();
        self.builder
            .terminate_branch(tags_equal, dispatch, self.unequal);

        let variants = self.pool.enum_variants(self.receiver_type);
        let mut cases = Vec::with_capacity(variants.len());
        let mut variant_blocks = Vec::with_capacity(variants.len());
        for (variant_index, (_, fields)) in variants.iter().enumerate() {
            let switch_index =
                u64::try_from(variant_index).map_err(|_| DerivedEqBodyError::IndexOverflow {
                    receiver_type: self.receiver_type,
                    index_kind: "variant",
                    index: variant_index,
                })?;
            let block = self.builder.new_block();
            cases.push((switch_index, block));
            variant_blocks.push((block, fields.as_slice()));
        }

        self.builder.position_at(dispatch);
        self.builder
            .terminate_switch(receiver_tag, cases, self.unequal);
        for (block, fields) in variant_blocks {
            self.builder.position_at(block);
            self.emit_field_chain(fields, 1)?;
        }
        Ok(())
    }

    fn emit_field_chain(
        &mut self,
        fields: &[Idx],
        field_base: u32,
    ) -> Result<(), DerivedEqBodyError> {
        if fields.is_empty() {
            self.builder.terminate_jump(self.equal, Vec::new());
            return Ok(());
        }

        let mut comparison_order: Vec<_> = fields.iter().copied().enumerate().collect();
        comparison_order.sort_by_key(|(_, field_type)| !self.classifier.is_scalar(*field_type));

        for (position, (field_index, field_type)) in comparison_order.iter().copied().enumerate() {
            let offset =
                u32::try_from(field_index).map_err(|_| DerivedEqBodyError::IndexOverflow {
                    receiver_type: self.receiver_type,
                    index_kind: "field",
                    index: field_index,
                })?;
            let projection =
                field_base
                    .checked_add(offset)
                    .ok_or(DerivedEqBodyError::IndexOverflow {
                        receiver_type: self.receiver_type,
                        index_kind: "field",
                        index: field_index,
                    })?;

            let receiver_field =
                self.builder
                    .emit_project(field_type, self.receiver, projection, None);

            let other_field = self
                .builder
                .emit_project(field_type, self.other, projection, None);

            let field_equal = emit_field_eq(
                self.builder,
                receiver_field,
                other_field,
                field_type,
                self.method_name,
                self.pool,
            );
            let is_last = position + 1 == comparison_order.len();
            let next = if is_last {
                self.equal
            } else {
                self.builder.new_block()
            };
            self.builder
                .terminate_branch(field_equal, next, self.unequal);
            if !is_last {
                self.builder.position_at(next);
            }
        }
        Ok(())
    }
}

fn emit_field_eq(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    field_type: Idx,
    method_name: ori_ir::Name,
    pool: &Pool,
) -> ArcVarId {
    if pool.builtin_method_type_tag(field_type).is_some() {
        return emit_primitive_eq(builder, receiver, other);
    }

    let result = builder.emit_invoke(Idx::BOOL, method_name, vec![receiver, other], None, None);
    builder.note_method_call(result, field_type, MethodCallForm::Instance);
    result
}

fn emit_primitive_eq(builder: &mut ArcIrBuilder, receiver: ArcVarId, other: ArcVarId) -> ArcVarId {
    builder.emit_let(
        Idx::BOOL,
        ArcValue::PrimOp {
            op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
            args: vec![receiver, other],
        },
        None,
    )
}

fn emit_bool_return(builder: &mut ArcIrBuilder, block: ArcBlockId, value: bool) {
    builder.position_at(block);
    let result = builder.emit_let(Idx::BOOL, ArcValue::Literal(LitValue::Bool(value)), None);
    builder.terminate_return(result);
}
