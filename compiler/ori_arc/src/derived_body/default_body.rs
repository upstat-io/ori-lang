//! Compiler-derived Default bodies.

use ori_ir::{DurationUnit, Name, SizeUnit, StringInterner};
use ori_types::{FunctionSig, Idx, Pool, Tag};

use crate::classify::ArcClassifier;
use crate::ir::{
    compute_var_reprs, ArcFunction, ArcValue, ArcVarId, CtorKind, LitValue, MethodCallForm,
};
use crate::lower::ArcIrBuilder;

use super::validation::{validate_concrete_type, RETURN_TYPE};

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

crate::derived_body::impl_concrete_type_error_conversion!(DerivedDefaultBodyError);

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
        .map_err(DerivedDefaultBodyError::from)?;
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
    let literal = if pool.is_newtype_type(field_type) {
        None
    } else {
        match pool.tag(resolved) {
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
        }
    };
    if let Some(literal) = literal {
        return builder.emit_let(field_type, ArcValue::Literal(literal), None);
    }

    let result = builder.emit_invoke(field_type, method_name, Vec::new(), None, None);
    builder.note_method_call(result, field_type, MethodCallForm::Associated);
    result
}
