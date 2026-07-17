//! Compiler-derived Clone identity bodies.

use ori_ir::Name;
use ori_types::{FunctionSig, Idx, Pool};

use crate::classify::ArcClassifier;
use crate::ir::{compute_var_reprs, ArcFunction, ArcParam};
use crate::lower::ArcIrBuilder;
use crate::Ownership;

use super::validation::{validate_concrete_type, ConcreteTypeError, RETURN_TYPE, SELF_PARAMETER};

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
