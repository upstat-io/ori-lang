use ori_ir::Name;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use crate::{ArcFunction, ArcVarId};

/// Backend-neutral semantic signature of a function-typed SSA value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClosureValueSignature {
    ty: Idx,
    parameters: Box<[Idx]>,
    result: Idx,
}

impl ClosureValueSignature {
    /// Construct an unvalidated callable signature for artifact transport.
    /// Executable-program closure validates its exact type and use-site
    /// relationships before a backend can consume it.
    #[must_use]
    pub fn from_parts(ty: Idx, parameters: Vec<Idx>, result: Idx) -> Self {
        Self {
            ty,
            parameters: parameters.into_boxed_slice(),
            result,
        }
    }

    /// Exact function-type identity carried by the SSA register.
    #[must_use]
    pub const fn ty(&self) -> Idx {
        self.ty
    }

    /// Explicit residual parameters in call order.
    #[must_use]
    pub fn parameters(&self) -> &[Idx] {
        &self.parameters
    }

    /// Number of explicit residual arguments.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.parameters.len()
    }

    /// Semantic result type.
    #[must_use]
    pub const fn result(&self) -> Idx {
        self.result
    }
}

/// Function-local callable facts parallel to `ArcFunction::var_types`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionCallableFacts {
    register_signatures: Box<[Option<ClosureValueSignature>]>,
}

impl FunctionCallableFacts {
    /// Construct unvalidated register-parallel callable facts for artifact
    /// transport.
    #[must_use]
    pub fn from_register_signatures(
        register_signatures: Vec<Option<ClosureValueSignature>>,
    ) -> Self {
        Self {
            register_signatures: register_signatures.into_boxed_slice(),
        }
    }

    /// Signature facts for every SSA register, including explicit `None`
    /// entries for non-callable values.
    #[must_use]
    pub fn register_signatures(&self) -> &[Option<ClosureValueSignature>] {
        &self.register_signatures
    }

    /// Resolve the signature of one register.
    #[must_use]
    pub fn signature(&self, register: ArcVarId) -> Option<&ClosureValueSignature> {
        self.register_signatures
            .get(register.index())
            .and_then(Option::as_ref)
    }
}

/// Freeze exact callable signatures for every SSA register in every function.
///
/// The type pool is consulted once at the semantic owner. Executable artifact
/// closure and physical backends consume only these closed facts.
#[must_use]
pub fn freeze_function_callable_facts(
    functions: &[ArcFunction],
    pool: &Pool,
) -> FxHashMap<Name, FunctionCallableFacts> {
    functions
        .iter()
        .map(|function| {
            let register_signatures = function
                .var_types
                .iter()
                .map(|&ty| closure_signature(pool, ty))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            (
                function.name,
                FunctionCallableFacts {
                    register_signatures,
                },
            )
        })
        .collect()
}

fn closure_signature(pool: &Pool, ty: Idx) -> Option<ClosureValueSignature> {
    let resolved = pool.resolve_fully(ty);
    if resolved.raw() as usize >= pool.len() || pool.tag(resolved) != Tag::Function {
        return None;
    }
    Some(ClosureValueSignature {
        ty,
        parameters: pool.function_params(resolved).into_boxed_slice(),
        result: pool.function_return(resolved),
    })
}
