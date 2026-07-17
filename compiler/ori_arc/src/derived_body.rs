//! ARC bodies for compiler-derived trait methods.
//!
//! Derived bodies enter the same ARC/AIMS pipeline as source functions. This
//! module constructs only their semantic control and data flow; ownership
//! events remain the responsibility of AIMS realization.

use ori_ir::{DurationUnit, Name, SizeUnit, StringInterner};
use ori_types::{FunctionSig, Idx, Pool, Tag, TypeFlags};

use crate::classify::ArcClassifier;
use crate::ir::{
    compute_var_reprs, ArcBlockId, ArcFunction, ArcParam, ArcValue, ArcVarId, CtorKind, LitValue,
    MethodCallForm, PrimOp,
};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

const SELF_PARAMETER: &str = "self parameter";
const RETURN_TYPE: &str = "return type";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConcreteTypeError {
    InvalidTypeIndex { position: &'static str, ty: Idx },
    NonConcreteType { position: &'static str, ty: Idx },
}

/// Invalid input to a compiler-derived Clone identity body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedCloneBodyError {
    /// The supplied signature still has generic parameters.
    GenericSignature {
        /// Number of type parameters in the signature.
        type_parameters: usize,
        /// Number of const parameters in the signature.
        const_parameters: usize,
    },
    /// Clone must have exactly one named positional receiver.
    InvalidSelfParameterShape {
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
    /// The receiver and return types do not denote the same concrete type.
    ReturnTypeMismatch {
        /// Receiver type-pool index.
        self_type: Idx,
        /// Return type-pool index.
        return_type: Idx,
    },
}

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

struct DerivedEqInputs {
    self_type: Idx,
    other_type: Idx,
    resolved_receiver: Idx,
    owner_is_newtype: bool,
}

/// Invalid input to a compiler-derived Default body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DerivedDefaultBodyError {
    /// The supplied signature still has generic parameters.
    GenericSignature {
        /// Number of type parameters in the signature.
        type_parameters: usize,
        /// Number of const parameters in the signature.
        const_parameters: usize,
    },
    /// Default is an associated nullary method.
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
    /// Derived Default is defined only for concrete product types.
    UnsupportedReturnType { return_type: Idx, tag: Tag },
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

impl std::fmt::Display for DerivedDefaultBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericSignature {
                type_parameters,
                const_parameters,
            } => write!(
                formatter,
                "derived Default body requires a concrete signature, found {type_parameters} type and {const_parameters} const parameters"
            ),
            Self::InvalidParameterShape {
                parameter_names,
                parameter_types,
            } => write!(
                formatter,
                "derived Default body requires no parameters, found {parameter_names} names and {parameter_types} types"
            ),
            Self::InvalidTypeIndex { position, ty } => write!(
                formatter,
                "derived Default body {position} has an invalid type index {ty:?}"
            ),
            Self::NonConcreteType { position, ty } => write!(
                formatter,
                "derived Default body {position} must be concrete, found {ty:?}"
            ),
            Self::UnsupportedReturnType { return_type, tag } => write!(
                formatter,
                "derived Default body requires a concrete struct return type, found {return_type:?} with shape {tag:?}"
            ),
        }
    }
}

impl std::error::Error for DerivedDefaultBodyError {}

impl std::fmt::Display for DerivedCloneBodyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GenericSignature {
                type_parameters,
                const_parameters,
            } => write!(
                formatter,
                "derived Clone body requires a concrete signature, found {type_parameters} type and {const_parameters} const parameters"
            ),
            Self::InvalidSelfParameterShape {
                parameter_names,
                parameter_types,
            } => write!(
                formatter,
                "derived Clone body requires one named self parameter, found {parameter_names} names and {parameter_types} types"
            ),
            Self::InvalidTypeIndex { position, ty } => write!(
                formatter,
                "derived Clone body {position} has an invalid type index {ty:?}"
            ),
            Self::NonConcreteType { position, ty } => write!(
                formatter,
                "derived Clone body {position} must be concrete, found {ty:?}"
            ),
            Self::ReturnTypeMismatch {
                self_type,
                return_type,
            } => write!(
                formatter,
                "derived Clone body must return its self type: {self_type:?} is not structurally equal to {return_type:?}"
            ),
        }
    }
}

impl std::error::Error for DerivedCloneBodyError {}

/// Build the semantic identity body for one concrete derived Clone method.
///
/// The function name is the executable identity selected by the compiler,
/// which may differ from the source-level method name stored in `signature`.
/// The resulting ARC function contains one receiver variable, no instructions,
/// and `Return(receiver)`. Representation metadata is ready for the ordinary
/// closed-program AIMS pipeline; this constructor does not place ownership
/// events itself.
pub fn build_derived_clone_identity(
    executable_name: Name,
    signature: &FunctionSig,
    pool: &Pool,
) -> Result<ArcFunction, DerivedCloneBodyError> {
    if signature.is_generic() {
        return Err(DerivedCloneBodyError::GenericSignature {
            type_parameters: signature.type_params.len(),
            const_parameters: signature.const_params.len(),
        });
    }
    if signature.param_names.len() != 1 || signature.param_types.len() != 1 {
        return Err(DerivedCloneBodyError::InvalidSelfParameterShape {
            parameter_names: signature.param_names.len(),
            parameter_types: signature.param_types.len(),
        });
    }

    let self_type = signature.param_types[0];
    validate_concrete_type(pool, SELF_PARAMETER, self_type).map_err(map_clone_type_error)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type)
        .map_err(map_clone_type_error)?;
    if !pool.structural_eq(self_type, signature.return_type) {
        return Err(DerivedCloneBodyError::ReturnTypeMismatch {
            self_type,
            return_type: signature.return_type,
        });
    }

    let mut builder = ArcIrBuilder::new();
    let entry = builder.entry_block();
    let receiver = builder.fresh_var(self_type);
    builder.terminate_return(receiver);
    let mut function = builder.finish(
        executable_name,
        vec![ArcParam {
            var: receiver,
            ty: self_type,
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

/// Build one concrete derived Eq body as ordinary shared ARC control/data flow.
///
/// Product fields are compared left-to-right with short-circuiting. Sum values
/// compare tags before switching to the active payload shape. Builtin field
/// equality remains a typed primitive; user-defined fields carry an exact
/// method-call provenance fact for the shared target-rewrite seam. Newtypes
/// delegate exactly to their underlying value's equality. AIMS remains the sole
/// owner of argument, result, and unwind-cleanup ownership events.
pub fn build_derived_eq(
    executable_name: Name,
    owner_name: Name,
    method_name: Name,
    signature: &FunctionSig,
    pool: &Pool,
) -> Result<ArcFunction, DerivedEqBodyError> {
    let inputs = validate_derived_eq_inputs(owner_name, signature, pool)?;
    let receiver_tag = pool.tag(inputs.resolved_receiver);
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

        match receiver_tag {
            Tag::Struct => emit_struct_eq(
                &mut builder,
                receiver,
                other,
                inputs.resolved_receiver,
                method_name,
                equal,
                unequal,
                pool,
            )?,
            Tag::Enum => emit_enum_eq(
                &mut builder,
                receiver,
                other,
                inputs.resolved_receiver,
                method_name,
                equal,
                unequal,
                pool,
            )?,
            tag => {
                return Err(DerivedEqBodyError::UnsupportedReceiverType {
                    receiver_type: inputs.self_type,
                    tag,
                });
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

    let classifier = ArcClassifier::new(pool);
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
    validate_concrete_type(pool, SELF_PARAMETER, self_type).map_err(map_eq_type_error)?;
    validate_concrete_type(pool, "other parameter", other_type).map_err(map_eq_type_error)?;
    validate_concrete_type(pool, RETURN_TYPE, signature.return_type).map_err(map_eq_type_error)?;
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

/// Build one concrete derived Default body as ordinary shared ARC data flow.
///
/// Primitive fields are materialized as typed zero values. Every remaining
/// field is obtained through an exact associated `default` call, allowing
/// nested generated bodies and registry-owned defaults to close through the
/// same target-resolution seam as source calls. The final product construct
/// enters the ordinary AIMS and executable-artifact pipeline.
pub fn build_derived_default(
    executable_name: Name,
    owner_name: Name,
    method_name: Name,
    signature: &FunctionSig,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<ArcFunction, DerivedDefaultBodyError> {
    if signature.is_generic() {
        return Err(DerivedDefaultBodyError::GenericSignature {
            type_parameters: signature.type_params.len(),
            const_parameters: signature.const_params.len(),
        });
    }
    if !signature.param_names.is_empty() || !signature.param_types.is_empty() {
        return Err(DerivedDefaultBodyError::InvalidParameterShape {
            parameter_names: signature.param_names.len(),
            parameter_types: signature.param_types.len(),
        });
    }

    validate_concrete_type(pool, RETURN_TYPE, signature.return_type)
        .map_err(map_default_type_error)?;
    let resolved_return = pool.resolve_fully(signature.return_type);
    let return_tag = pool.tag(resolved_return);
    if return_tag != Tag::Struct {
        return Err(DerivedDefaultBodyError::UnsupportedReturnType {
            return_type: signature.return_type,
            tag: return_tag,
        });
    }

    let mut builder = ArcIrBuilder::new();
    let entry = builder.entry_block();
    let empty_string = interner.intern("");
    let fields = pool.struct_fields(resolved_return);
    let mut values = Vec::with_capacity(fields.len());
    for (_, field_type) in fields {
        values.push(emit_default_field(
            &mut builder,
            method_name,
            field_type,
            empty_string,
            pool,
        ));
    }
    let result = builder.emit_construct(
        signature.return_type,
        CtorKind::Struct(owner_name),
        values,
        None,
    );
    builder.terminate_return(result);
    let mut function = builder.finish(
        executable_name,
        Vec::new(),
        signature.return_type,
        entry,
        false,
    );

    let classifier = ArcClassifier::new(pool);
    let representations = compute_var_reprs(&function, &classifier, pool);
    function.replace_variable_representations(representations);
    Ok(function)
}

fn emit_default_field(
    builder: &mut ArcIrBuilder,
    method_name: Name,
    field_type: Idx,
    empty_string: Name,
    pool: &Pool,
) -> ArcVarId {
    let resolved = pool.resolve_fully(field_type);
    let literal = match pool.tag(resolved) {
        Tag::Int | Tag::Byte => Some(LitValue::Int(0)),
        Tag::Float => Some(LitValue::Float(0.0f64.to_bits())),
        Tag::Bool => Some(LitValue::Bool(false)),
        Tag::Str => Some(LitValue::String(empty_string)),
        Tag::Char => Some(LitValue::Char('\0')),
        Tag::Unit => Some(LitValue::Unit),
        Tag::Duration => Some(LitValue::Duration {
            value: 0,
            unit: DurationUnit::Nanoseconds,
        }),
        Tag::Size => Some(LitValue::Size {
            value: 0,
            unit: SizeUnit::Bytes,
        }),
        _ => None,
    };
    if let Some(literal) = literal {
        return builder.emit_let(field_type, ArcValue::Literal(literal), None);
    }

    let result = builder.emit_invoke(field_type, method_name, Vec::new(), None, None);
    builder.note_method_call(result, field_type, MethodCallForm::Associated);
    result
}

#[expect(
    clippy::too_many_arguments,
    reason = "derived-body emission carries both operands, exact owner, method, exits, and pool"
)]
fn emit_struct_eq(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: ori_ir::Name,
    equal: ArcBlockId,
    unequal: ArcBlockId,
    pool: &Pool,
) -> Result<(), DerivedEqBodyError> {
    let fields: Vec<_> = pool
        .struct_fields(receiver_type)
        .into_iter()
        .map(|(_, field_type)| field_type)
        .collect();
    emit_field_eq_chain(
        builder,
        receiver,
        other,
        receiver_type,
        &fields,
        0,
        method_name,
        equal,
        unequal,
        pool,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "derived-body emission carries both operands, exact owner, method, exits, and pool"
)]
fn emit_enum_eq(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    method_name: ori_ir::Name,
    equal: ArcBlockId,
    unequal: ArcBlockId,
    pool: &Pool,
) -> Result<(), DerivedEqBodyError> {
    let receiver_tag = builder.emit_project(Idx::INT, receiver, 0, None);
    let other_tag = builder.emit_project(Idx::INT, other, 0, None);
    let tags_equal = emit_primitive_eq(builder, receiver_tag, other_tag);
    let dispatch = builder.new_block();
    builder.terminate_branch(tags_equal, dispatch, unequal);

    let variants = pool.enum_variants(receiver_type);
    let mut cases = Vec::with_capacity(variants.len());
    let mut variant_blocks = Vec::with_capacity(variants.len());
    for (variant_index, (_, fields)) in variants.iter().enumerate() {
        let switch_index =
            u64::try_from(variant_index).map_err(|_| DerivedEqBodyError::IndexOverflow {
                receiver_type,
                index_kind: "variant",
                index: variant_index,
            })?;
        let block = builder.new_block();
        cases.push((switch_index, block));
        variant_blocks.push((block, fields.as_slice()));
    }

    builder.position_at(dispatch);
    builder.terminate_switch(receiver_tag, cases, unequal);
    for (block, fields) in variant_blocks {
        builder.position_at(block);
        emit_field_eq_chain(
            builder,
            receiver,
            other,
            receiver_type,
            fields,
            1,
            method_name,
            equal,
            unequal,
            pool,
        )?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "one structural field chain carries operands, shape, projection base, method, exits, and pool"
)]
fn emit_field_eq_chain(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    receiver_type: Idx,
    fields: &[Idx],
    field_base: u32,
    method_name: ori_ir::Name,
    equal: ArcBlockId,
    unequal: ArcBlockId,
    pool: &Pool,
) -> Result<(), DerivedEqBodyError> {
    if fields.is_empty() {
        builder.terminate_jump(equal, Vec::new());
        return Ok(());
    }

    for (field_index, &field_type) in fields.iter().enumerate() {
        let offset = u32::try_from(field_index).map_err(|_| DerivedEqBodyError::IndexOverflow {
            receiver_type,
            index_kind: "field",
            index: field_index,
        })?;
        let projection =
            field_base
                .checked_add(offset)
                .ok_or(DerivedEqBodyError::IndexOverflow {
                    receiver_type,
                    index_kind: "field",
                    index: field_index,
                })?;
        let receiver_field = builder.emit_project(field_type, receiver, projection, None);
        let other_field = builder.emit_project(field_type, other, projection, None);
        let field_equal = emit_field_eq(
            builder,
            receiver_field,
            other_field,
            field_type,
            method_name,
            pool,
        );
        let is_last = field_index + 1 == fields.len();
        let next = if is_last { equal } else { builder.new_block() };
        builder.terminate_branch(field_equal, next, unequal);
        if !is_last {
            builder.position_at(next);
        }
    }
    Ok(())
}

fn emit_field_eq(
    builder: &mut ArcIrBuilder,
    receiver: ArcVarId,
    other: ArcVarId,
    field_type: Idx,
    method_name: ori_ir::Name,
    pool: &Pool,
) -> ArcVarId {
    let resolved = pool.resolve_fully(field_type);
    if pool.builtin_type_tag(resolved).is_some() {
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

fn map_clone_type_error(error: ConcreteTypeError) -> DerivedCloneBodyError {
    match error {
        ConcreteTypeError::InvalidTypeIndex { position, ty } => {
            DerivedCloneBodyError::InvalidTypeIndex { position, ty }
        }
        ConcreteTypeError::NonConcreteType { position, ty } => {
            DerivedCloneBodyError::NonConcreteType { position, ty }
        }
    }
}

fn map_eq_type_error(error: ConcreteTypeError) -> DerivedEqBodyError {
    match error {
        ConcreteTypeError::InvalidTypeIndex { position, ty } => {
            DerivedEqBodyError::InvalidTypeIndex { position, ty }
        }
        ConcreteTypeError::NonConcreteType { position, ty } => {
            DerivedEqBodyError::NonConcreteType { position, ty }
        }
    }
}

fn map_default_type_error(error: ConcreteTypeError) -> DerivedDefaultBodyError {
    match error {
        ConcreteTypeError::InvalidTypeIndex { position, ty } => {
            DerivedDefaultBodyError::InvalidTypeIndex { position, ty }
        }
        ConcreteTypeError::NonConcreteType { position, ty } => {
            DerivedDefaultBodyError::NonConcreteType { position, ty }
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
    use ori_ir::{BinaryOp, Name, StringInterner};
    use ori_types::{ConstParamInfo, FunctionSig, Idx, Pool};

    use crate::{
        build_derived_clone_identity, build_derived_default, build_derived_eq, ArcInstr,
        ArcTerminator, ArcValue, ArcVarId, CtorKind, DerivedCloneBodyError,
        DerivedDefaultBodyError, DerivedEqBodyError, LitValue, MethodCallForm, Ownership, PrimOp,
        ValueRepr, VariableMetadataState,
    };

    const EXECUTABLE: Name = Name::from_raw(1);
    const METHOD: Name = Name::from_raw(2);
    const SELF_NAME: Name = Name::from_raw(3);
    const OTHER_NAME: Name = Name::from_raw(4);
    const EQ_SIGNATURE_NAME: Name = Name::from_raw(5);

    fn signature(self_type: Idx, return_type: Idx) -> FunctionSig {
        FunctionSig::synthetic(METHOD, vec![SELF_NAME], vec![self_type], return_type)
    }

    fn eq_signature(self_type: Idx, other_type: Idx, return_type: Idx) -> FunctionSig {
        FunctionSig::synthetic(
            EQ_SIGNATURE_NAME,
            vec![SELF_NAME, OTHER_NAME],
            vec![self_type, other_type],
            return_type,
        )
    }

    fn default_signature(return_type: Idx) -> FunctionSig {
        FunctionSig::synthetic(METHOD, Vec::new(), Vec::new(), return_type)
    }

    fn clone_body_or_panic(
        signature: &FunctionSig,
        pool: &Pool,
        expectation: &str,
    ) -> crate::ArcFunction {
        match build_derived_clone_identity(EXECUTABLE, signature, pool) {
            Ok(body) => body,
            Err(error) => panic!("{expectation}: {error}"),
        }
    }

    fn eq_body_or_panic(
        owner: Name,
        signature: &FunctionSig,
        pool: &Pool,
        expectation: &str,
    ) -> crate::ArcFunction {
        match build_derived_eq(EXECUTABLE, owner, METHOD, signature, pool) {
            Ok(body) => body,
            Err(error) => panic!("{expectation}: {error}"),
        }
    }

    fn default_body_or_panic(
        owner: Name,
        method: Name,
        signature: &FunctionSig,
        interner: &StringInterner,
        pool: &Pool,
        expectation: &str,
    ) -> crate::ArcFunction {
        match build_derived_default(EXECUTABLE, owner, method, signature, interner, pool) {
            Ok(body) => body,
            Err(error) => panic!("{expectation}: {error}"),
        }
    }

    #[test]
    fn scalar_identity_is_representation_ready_and_instruction_free() {
        let pool = Pool::new();
        let body = clone_body_or_panic(
            &signature(Idx::INT, Idx::INT),
            &pool,
            "concrete identity signature must produce a derived Clone body",
        );

        assert_eq!(body.name, EXECUTABLE);
        assert_eq!(body.params.len(), 1);
        assert_eq!(body.params[0].var, ArcVarId::new(0));
        assert_eq!(body.params[0].ty, Idx::INT);
        assert_eq!(body.params[0].ownership, Ownership::Owned);
        assert_eq!(body.return_type, Idx::INT);
        assert_eq!(body.var_types, vec![Idx::INT]);
        assert_eq!(body.var_reprs, vec![ValueRepr::Scalar]);
        assert_eq!(
            body.var_metadata_state,
            VariableMetadataState::RepresentationsReady
        );
        assert_eq!(body.blocks.len(), 1);
        assert!(body.blocks[0].body.is_empty());
        assert_eq!(
            body.blocks[0].terminator,
            ArcTerminator::Return {
                value: ArcVarId::new(0)
            }
        );
    }

    #[test]
    fn managed_identity_has_shape_metadata_but_no_ownership_instructions() {
        let pool = Pool::new();
        let body = clone_body_or_panic(
            &signature(Idx::STR, Idx::STR),
            &pool,
            "concrete managed identity signature must produce a derived Clone body",
        );

        assert_eq!(body.var_reprs, vec![ValueRepr::FatValue]);
        assert!(body.blocks[0].body.is_empty());
        assert_eq!(
            body.blocks[0].terminator,
            ArcTerminator::Return {
                value: ArcVarId::new(0)
            }
        );
    }

    #[test]
    fn structurally_equal_alias_and_concrete_return_are_accepted() {
        let mut pool = Pool::new();
        let type_name = Name::from_raw(10);
        let field_name = Name::from_raw(11);
        let named = pool.named(type_name);
        let concrete = pool.struct_type(type_name, &[(field_name, Idx::STR)]);
        pool.set_resolution(named, concrete);

        assert_ne!(named, concrete);
        let body = clone_body_or_panic(
            &signature(named, concrete),
            &pool,
            "structurally equal alias and concrete return must produce a derived Clone body",
        );

        assert_eq!(body.params[0].ty, named);
        assert_eq!(body.return_type, concrete);
        assert_eq!(body.var_reprs, vec![ValueRepr::Aggregate]);
        assert!(body.blocks[0].body.is_empty());
    }

    #[test]
    fn structurally_different_return_is_rejected() {
        let pool = Pool::new();
        let Err(error) =
            build_derived_clone_identity(EXECUTABLE, &signature(Idx::INT, Idx::STR), &pool)
        else {
            panic!("derived Clone accepted a return type different from Self")
        };

        assert_eq!(
            error,
            DerivedCloneBodyError::ReturnTypeMismatch {
                self_type: Idx::INT,
                return_type: Idx::STR,
            }
        );
    }

    #[test]
    fn missing_or_extra_receiver_is_rejected() {
        let pool = Pool::new();
        let missing = FunctionSig::synthetic(METHOD, Vec::new(), Vec::new(), Idx::INT);
        let extra = FunctionSig::synthetic(
            METHOD,
            vec![SELF_NAME, Name::from_raw(4)],
            vec![Idx::INT, Idx::INT],
            Idx::INT,
        );

        assert!(matches!(
            build_derived_clone_identity(EXECUTABLE, &missing, &pool),
            Err(DerivedCloneBodyError::InvalidSelfParameterShape {
                parameter_names: 0,
                parameter_types: 0,
            })
        ));
        assert!(matches!(
            build_derived_clone_identity(EXECUTABLE, &extra, &pool),
            Err(DerivedCloneBodyError::InvalidSelfParameterShape {
                parameter_names: 2,
                parameter_types: 2,
            })
        ));
    }

    #[test]
    fn generic_and_unresolved_signatures_are_rejected() {
        let mut pool = Pool::new();
        let mut generic = signature(Idx::INT, Idx::INT);
        generic.type_params.push(Name::from_raw(5));
        assert!(matches!(
            build_derived_clone_identity(EXECUTABLE, &generic, &pool),
            Err(DerivedCloneBodyError::GenericSignature {
                type_parameters: 1,
                const_parameters: 0,
            })
        ));

        let unresolved = pool.fresh_var();
        assert!(matches!(
            build_derived_clone_identity(
                EXECUTABLE,
                &signature(unresolved, unresolved),
                &pool
            ),
            Err(DerivedCloneBodyError::NonConcreteType {
                position: "self parameter",
                ty,
            }) if ty == unresolved
        ));
    }

    #[test]
    fn foreign_pool_index_is_rejected_without_indexing_the_pool() {
        let pool = Pool::new();
        let invalid = Idx::from_raw(u32::MAX);
        assert!(matches!(
            build_derived_clone_identity(EXECUTABLE, &signature(invalid, invalid), &pool),
            Err(DerivedCloneBodyError::InvalidTypeIndex {
                position: "self parameter",
                ty,
            }) if ty == invalid
        ));
    }

    #[test]
    fn scalar_struct_eq_is_fieldwise_short_circuit_arc() {
        let mut pool = Pool::new();
        let struct_type = pool.struct_type(
            Name::from_raw(20),
            &[
                (Name::from_raw(21), Idx::INT),
                (Name::from_raw(22), Idx::BOOL),
            ],
        );
        let body = eq_body_or_panic(
            Name::from_raw(20),
            &eq_signature(struct_type, struct_type, Idx::BOOL),
            &pool,
            "concrete scalar-shaped struct Eq signature must produce a derived Eq body",
        );

        assert_eq!(body.name, EXECUTABLE);
        assert_eq!(body.params.len(), 2);
        assert_eq!(body.params[0].var, ArcVarId::new(0));
        assert_eq!(body.params[0].ty, struct_type);
        assert_eq!(body.params[0].ownership, Ownership::Owned);
        assert_eq!(body.params[1].var, ArcVarId::new(1));
        assert_eq!(body.params[1].ty, struct_type);
        assert_eq!(body.params[1].ownership, Ownership::Owned);
        assert_eq!(body.return_type, Idx::BOOL);
        assert_eq!(body.var_types.len(), 10);
        assert!(body.var_reprs.iter().all(|repr| *repr == ValueRepr::Scalar));
        assert_eq!(
            body.var_metadata_state,
            VariableMetadataState::RepresentationsReady
        );
        assert_eq!(body.blocks.len(), 4);
        let instructions: Vec<_> = body.blocks.iter().flat_map(|block| &block.body).collect();
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(instruction, ArcInstr::Project { .. }))
                .count(),
            4
        );
        assert_eq!(
            instructions
                .iter()
                .filter(|instruction| matches!(
                    instruction,
                    ArcInstr::Let {
                        value: ArcValue::PrimOp {
                            op: PrimOp::Binary(BinaryOp::Eq),
                            ..
                        },
                        ..
                    }
                ))
                .count(),
            2
        );
        assert!(!instructions.iter().any(|instruction| matches!(
            instruction,
            ArcInstr::Let {
                value: ArcValue::PrimOp {
                    op: PrimOp::Binary(BinaryOp::Eq),
                    args,
                },
                ..
            } if args == &[ArcVarId::new(0), ArcVarId::new(1)]
        )));
        assert_eq!(
            body.blocks
                .iter()
                .filter(|block| matches!(block.terminator, ArcTerminator::Branch { .. }))
                .count(),
            2
        );
        assert!(body.method_call_facts.is_empty());
    }

    #[test]
    fn nested_user_field_is_an_exact_rewriteable_invoke() {
        let mut pool = Pool::new();
        let inner = pool.struct_type(Name::from_raw(50), &[(Name::from_raw(51), Idx::INT)]);
        let outer = pool.struct_type(Name::from_raw(52), &[(Name::from_raw(53), inner)]);
        let body = eq_body_or_panic(
            Name::from_raw(52),
            &eq_signature(outer, outer, Idx::BOOL),
            &pool,
            "nested concrete struct Eq signature must produce a derived Eq body",
        );

        let invoke = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                ArcTerminator::Invoke {
                    dst, func, args, ..
                } => Some((*dst, *func, args.as_slice())),
                _ => None,
            });
        let Some((destination, function, arguments)) = invoke else {
            panic!("user-defined field equality must preserve an unwind edge");
        };
        assert_eq!(function, METHOD);
        assert_eq!(arguments.len(), 2);
        assert_eq!(
            body.method_call_facts,
            vec![crate::MethodCallFact {
                destination,
                receiver_type: inner,
                form: MethodCallForm::Instance,
                producer: None,
                derived_position: None,
            }]
        );
        assert!(body
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, ArcTerminator::Resume)));
    }

    #[test]
    fn newtype_eq_with_primitive_underlying_uses_typed_equality() {
        let owner = Name::from_raw(60);
        let mut pool = Pool::new();
        let receiver_type = pool.named(owner);
        pool.register_newtype_ctor(owner, Idx::STR);
        pool.set_resolution(receiver_type, Idx::STR);

        let body = eq_body_or_panic(
            owner,
            &eq_signature(receiver_type, receiver_type, Idx::BOOL),
            &pool,
            "newtype Eq must delegate to the underlying equality target",
        );
        let Some((destination, arguments)) = body.blocks.iter().find_map(|block| {
            block.body.iter().find_map(|instruction| match instruction {
                ArcInstr::Let {
                    dst,
                    ty: Idx::BOOL,
                    value:
                        ArcValue::PrimOp {
                            op: PrimOp::Binary(BinaryOp::Eq),
                            args,
                        },
                } => Some((*dst, args.as_slice())),
                _ => None,
            })
        }) else {
            panic!("primitive newtype Eq must use typed primitive equality")
        };

        assert_eq!(arguments.len(), 2);
        assert!(body.method_call_facts.is_empty());
        assert_eq!(
            body.blocks
                .iter()
                .flat_map(|block| &block.body)
                .filter(|instruction| matches!(instruction, ArcInstr::Let { .. }))
                .count(),
            3
        );
        assert!(body
            .blocks
            .iter()
            .all(|block| !matches!(block.terminator, ArcTerminator::Invoke { .. })));
        assert!(body.blocks.iter().any(|block| matches!(
            block.terminator,
            ArcTerminator::Return { value } if value == destination
        )));
    }

    #[test]
    fn newtype_eq_with_user_underlying_keeps_exact_method_dispatch() {
        let owner = Name::from_raw(61);
        let mut pool = Pool::new();
        let receiver_type = pool.named(owner);
        let underlying = pool.struct_type(Name::from_raw(62), &[(Name::from_raw(63), Idx::INT)]);
        pool.register_newtype_ctor(owner, underlying);
        pool.set_resolution(receiver_type, underlying);

        let body = eq_body_or_panic(
            owner,
            &eq_signature(receiver_type, receiver_type, Idx::BOOL),
            &pool,
            "newtype Eq must delegate to the user-defined underlying target",
        );
        let Some((destination, function, arguments)) = body.blocks.iter().find_map(|block| {
            let ArcTerminator::Invoke {
                dst, func, args, ..
            } = &block.terminator
            else {
                return None;
            };
            Some((*dst, *func, args.as_slice()))
        }) else {
            panic!("user-defined underlying Eq must preserve its unwind edge")
        };

        assert_eq!(function, METHOD);
        assert_eq!(arguments.len(), 2);
        assert_eq!(
            body.method_call_facts,
            vec![crate::MethodCallFact {
                destination,
                receiver_type: underlying,
                form: MethodCallForm::Instance,
                producer: None,
                derived_position: None,
            }]
        );
        assert_eq!(
            body.blocks
                .iter()
                .flat_map(|block| &block.body)
                .filter(|instruction| matches!(
                    instruction,
                    ArcInstr::Let {
                        ty,
                        value: ArcValue::Var(_),
                        ..
                    } if *ty == underlying
                ))
                .count(),
            2
        );
        assert!(!body
            .blocks
            .iter()
            .flat_map(|block| &block.body)
            .any(|instruction| matches!(instruction, ArcInstr::Project { .. })));
        assert!(body.blocks.iter().any(|block| matches!(
            block.terminator,
            ArcTerminator::Return { value } if value == destination
        )));
    }

    #[test]
    fn generic_eq_signature_is_rejected() {
        let pool = Pool::new();
        let mut generic = eq_signature(Idx::INT, Idx::INT, Idx::BOOL);
        generic.type_params.push(Name::from_raw(30));
        generic.const_params.push(ConstParamInfo {
            name: Name::from_raw(31),
            const_type: Idx::INT,
            default_value: None,
        });

        assert_eq!(
            build_derived_eq(EXECUTABLE, METHOD, METHOD, &generic, &pool),
            Err(DerivedEqBodyError::GenericSignature {
                type_parameters: 1,
                const_parameters: 1,
            })
        );
    }

    #[test]
    fn wrong_eq_arity_is_rejected() {
        let pool = Pool::new();
        let missing = FunctionSig::synthetic(METHOD, vec![SELF_NAME], vec![Idx::INT], Idx::BOOL);
        let extra = FunctionSig::synthetic(
            METHOD,
            vec![SELF_NAME, OTHER_NAME, Name::from_raw(40)],
            vec![Idx::INT, Idx::INT, Idx::INT],
            Idx::BOOL,
        );

        assert_eq!(
            build_derived_eq(EXECUTABLE, METHOD, METHOD, &missing, &pool),
            Err(DerivedEqBodyError::InvalidParameterShape {
                parameter_names: 1,
                parameter_types: 1,
            })
        );
        assert_eq!(
            build_derived_eq(EXECUTABLE, METHOD, METHOD, &extra, &pool),
            Err(DerivedEqBodyError::InvalidParameterShape {
                parameter_names: 3,
                parameter_types: 3,
            })
        );
    }

    #[test]
    fn mismatched_eq_receiver_types_are_rejected() {
        let pool = Pool::new();

        assert_eq!(
            build_derived_eq(
                EXECUTABLE,
                METHOD,
                METHOD,
                &eq_signature(Idx::INT, Idx::STR, Idx::BOOL),
                &pool,
            ),
            Err(DerivedEqBodyError::ReceiverTypeMismatch {
                self_type: Idx::INT,
                other_type: Idx::STR,
            })
        );
    }

    #[test]
    fn distinct_newtypes_with_one_representation_are_rejected_as_eq_receivers() {
        let left_name = Name::from_raw(70);
        let right_name = Name::from_raw(71);
        let mut pool = Pool::new();
        let left = pool.named(left_name);
        let right = pool.named(right_name);
        pool.register_newtype_ctor(left_name, Idx::STR);
        pool.register_newtype_ctor(right_name, Idx::STR);
        pool.set_resolution(left, Idx::STR);
        pool.set_resolution(right, Idx::STR);

        assert_eq!(
            build_derived_eq(
                EXECUTABLE,
                left_name,
                METHOD,
                &eq_signature(left, right, Idx::BOOL),
                &pool,
            ),
            Err(DerivedEqBodyError::ReceiverTypeMismatch {
                self_type: left,
                other_type: right,
            })
        );
    }

    #[test]
    fn non_bool_eq_return_is_rejected() {
        let pool = Pool::new();

        assert_eq!(
            build_derived_eq(
                EXECUTABLE,
                METHOD,
                METHOD,
                &eq_signature(Idx::INT, Idx::INT, Idx::INT),
                &pool,
            ),
            Err(DerivedEqBodyError::ReturnTypeMismatch {
                return_type: Idx::INT,
            })
        );
    }

    #[test]
    fn unresolved_and_invalid_eq_types_are_rejected_without_pool_indexing() {
        let mut pool = Pool::new();
        let unresolved = pool.fresh_var();
        let invalid = Idx::from_raw(u32::MAX);

        assert_eq!(
            build_derived_eq(
                EXECUTABLE,
                METHOD,
                METHOD,
                &eq_signature(unresolved, unresolved, Idx::BOOL),
                &pool,
            ),
            Err(DerivedEqBodyError::NonConcreteType {
                position: "self parameter",
                ty: unresolved,
            })
        );
        assert_eq!(
            build_derived_eq(
                EXECUTABLE,
                METHOD,
                METHOD,
                &eq_signature(Idx::INT, invalid, Idx::BOOL),
                &pool,
            ),
            Err(DerivedEqBodyError::InvalidTypeIndex {
                position: "other parameter",
                ty: invalid,
            })
        );
        assert_eq!(
            build_derived_eq(
                EXECUTABLE,
                METHOD,
                METHOD,
                &eq_signature(Idx::INT, Idx::INT, invalid),
                &pool,
            ),
            Err(DerivedEqBodyError::InvalidTypeIndex {
                position: "return type",
                ty: invalid,
            })
        );
    }

    #[test]
    fn default_struct_materializes_primitive_zeroes_and_constructs_owner() {
        let interner = StringInterner::new();
        let owner = interner.intern("Config");
        let mut pool = Pool::new();
        let config = pool.struct_type(
            owner,
            &[
                (interner.intern("name"), Idx::STR),
                (interner.intern("count"), Idx::INT),
                (interner.intern("enabled"), Idx::BOOL),
            ],
        );
        let body = default_body_or_panic(
            owner,
            METHOD,
            &default_signature(config),
            &interner,
            &pool,
            "concrete product Default must produce a shared body",
        );

        assert!(body.params.is_empty());
        assert_eq!(body.return_type, config);
        assert!(body.method_call_facts.is_empty());
        assert_eq!(body.blocks.len(), 1);
        assert!(matches!(
            &body.blocks[0].body[0],
            ArcInstr::Let {
                ty,
                value: ArcValue::Literal(LitValue::String(name)),
                ..
            } if *ty == Idx::STR && interner.lookup(*name).is_empty()
        ));
        assert!(matches!(
            &body.blocks[0].body[1],
            ArcInstr::Let {
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(0)),
                ..
            }
        ));
        assert!(matches!(
            &body.blocks[0].body[2],
            ArcInstr::Let {
                ty: Idx::BOOL,
                value: ArcValue::Literal(LitValue::Bool(false)),
                ..
            }
        ));
        assert!(matches!(
            &body.blocks[0].body[3],
            ArcInstr::Construct {
                ty,
                ctor: CtorKind::Struct(name),
                args,
                ..
            } if *ty == config && *name == owner && args.len() == 3
        ));
    }

    #[test]
    fn nested_default_is_an_exact_associated_call() {
        let interner = StringInterner::new();
        let inner_name = interner.intern("Inner");
        let outer_name = interner.intern("Outer");
        let mut pool = Pool::new();
        let inner = pool.struct_type(inner_name, &[(interner.intern("value"), Idx::INT)]);
        let outer = pool.struct_type(outer_name, &[(interner.intern("inner"), inner)]);
        let mut concrete_signature = default_signature(outer);
        concrete_signature.name = Name::from_raw(99);
        let body = default_body_or_panic(
            outer_name,
            METHOD,
            &concrete_signature,
            &interner,
            &pool,
            "nested product Default must preserve associated owner provenance",
        );

        let Some((dst, func, args)) = body.blocks.iter().find_map(|block| {
            let ArcTerminator::Invoke {
                dst, func, args, ..
            } = &block.terminator
            else {
                return None;
            };
            Some((*dst, *func, args))
        }) else {
            panic!("nested field must be produced by an associated Default call")
        };
        assert_eq!(func, METHOD);
        assert!(args.is_empty());
        assert_eq!(
            body.method_call_facts,
            vec![crate::MethodCallFact {
                destination: dst,
                receiver_type: inner,
                form: MethodCallForm::Associated,
                producer: None,
                derived_position: None,
            }]
        );
    }

    #[test]
    fn default_rejects_parameters_and_non_product_results() {
        let interner = StringInterner::new();
        let pool = Pool::new();
        let parameterized =
            FunctionSig::synthetic(METHOD, vec![SELF_NAME], vec![Idx::INT], Idx::INT);

        assert_eq!(
            build_derived_default(
                EXECUTABLE,
                Name::from_raw(20),
                METHOD,
                &parameterized,
                &interner,
                &pool
            ),
            Err(DerivedDefaultBodyError::InvalidParameterShape {
                parameter_names: 1,
                parameter_types: 1,
            })
        );
        assert_eq!(
            build_derived_default(
                EXECUTABLE,
                Name::from_raw(20),
                METHOD,
                &default_signature(Idx::INT),
                &interner,
                &pool,
            ),
            Err(DerivedDefaultBodyError::UnsupportedReturnType {
                return_type: Idx::INT,
                tag: ori_types::Tag::Int,
            })
        );
    }
}
