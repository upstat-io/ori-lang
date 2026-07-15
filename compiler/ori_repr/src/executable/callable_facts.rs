//! Structural closure of callable-value signatures.
//!
//! The semantic owner resolves function types once and supplies exact
//! register-parallel facts. This transport seam checks coverage and every
//! callable relationship expressible in ARC IR without querying the type pool.

use ori_arc::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ClosureValueSignature, CtorKind,
    FunctionCallableFacts,
};
use ori_ir::{Name, StringInterner};
use ori_types::Idx;
use rustc_hash::FxHashMap;

use super::RealizationError;

struct CallableValidationContext<'a> {
    enclosing: &'a ArcFunction,
    enclosing_facts: &'a FunctionCallableFacts,
    by_name: &'a FxHashMap<Name, &'a ArcFunction>,
    symbols: &'a StringInterner,
    function_symbol: &'a str,
}

/// Validate exact function/register coverage and order callable facts by slot.
pub(super) fn freeze_callable_facts(
    functions: &[ArcFunction],
    mut supplied: FxHashMap<Name, FunctionCallableFacts>,
    symbols: &StringInterner,
) -> Result<Box<[FunctionCallableFacts]>, RealizationError> {
    let mut frozen = Vec::with_capacity(functions.len());
    for function in functions {
        let function_symbol: Box<str> = symbols.lookup(function.name).into();
        let facts = supplied.remove(&function.name).ok_or_else(|| {
            RealizationError::MissingCallableFacts {
                function: function.name,
                function_symbol: function_symbol.clone(),
            }
        })?;
        if facts.register_signatures().len() != function.var_types.len() {
            return invalid(
                function,
                &function_symbol,
                "callable facts do not cover every SSA register",
            );
        }
        for (register, signature) in facts.register_signatures().iter().enumerate() {
            if signature
                .as_ref()
                .is_some_and(|signature| signature.ty() != function.var_types[register])
            {
                return invalid(
                    function,
                    &function_symbol,
                    "callable signature type identity disagrees with its SSA register",
                );
            }
        }
        frozen.push(facts);
    }
    if let Some(function) = supplied.keys().min_by_key(|name| name.raw()).copied() {
        return Err(RealizationError::UnexpectedCallableFacts { function });
    }

    validate_type_identity_consistency(functions, &frozen, symbols)?;
    let by_name: FxHashMap<_, _> = functions
        .iter()
        .map(|function| (function.name, function))
        .collect();
    for (function_index, function) in functions.iter().enumerate() {
        validate_function(function, &frozen[function_index], &by_name, symbols)?;
    }
    Ok(frozen.into_boxed_slice())
}

fn validate_type_identity_consistency(
    functions: &[ArcFunction],
    facts: &[FunctionCallableFacts],
    symbols: &StringInterner,
) -> Result<(), RealizationError> {
    let mut canonical: FxHashMap<Idx, ClosureValueSignature> = FxHashMap::default();
    for (function, function_facts) in functions.iter().zip(facts) {
        for signature in function_facts.register_signatures().iter().flatten() {
            match canonical.get(&signature.ty()) {
                Some(existing) if existing != signature => {
                    return invalid(
                        function,
                        symbols.lookup(function.name),
                        "one function-type identity carries conflicting callable signatures",
                    );
                }
                Some(_) => {}
                None => {
                    canonical.insert(signature.ty(), signature.clone());
                }
            }
        }
    }
    for (function, function_facts) in functions.iter().zip(facts) {
        for (&ty, signature) in function
            .var_types
            .iter()
            .zip(function_facts.register_signatures())
        {
            if canonical.contains_key(&ty) != signature.is_some() {
                return invalid(
                    function,
                    symbols.lookup(function.name),
                    "one function-type identity is not described consistently across SSA registers",
                );
            }
        }
    }
    Ok(())
}

fn validate_function(
    function: &ArcFunction,
    facts: &FunctionCallableFacts,
    by_name: &FxHashMap<Name, &ArcFunction>,
    symbols: &StringInterner,
) -> Result<(), RealizationError> {
    let function_symbol = symbols.lookup(function.name);
    let context = CallableValidationContext {
        enclosing: function,
        enclosing_facts: facts,
        by_name,
        symbols,
        function_symbol,
    };
    for block in &function.blocks {
        for instruction in &block.body {
            validate_instruction(&context, instruction)?;
        }
        validate_terminator(function, facts, &block.terminator, function_symbol)?;
    }
    Ok(())
}

fn validate_instruction(
    context: &CallableValidationContext<'_>,
    instruction: &ArcInstr,
) -> Result<(), RealizationError> {
    match instruction {
        ArcInstr::Let {
            dst,
            value: ArcValue::Var(source),
            ..
        } => validate_same_signature(
            context.enclosing,
            context.enclosing_facts,
            *dst,
            *source,
            context.function_symbol,
            "callable copy changes signature",
        ),
        ArcInstr::Select {
            dst,
            true_val,
            false_val,
            ..
        } => {
            validate_same_signature(
                context.enclosing,
                context.enclosing_facts,
                *dst,
                *true_val,
                context.function_symbol,
                "callable select true arm changes signature",
            )?;
            validate_same_signature(
                context.enclosing,
                context.enclosing_facts,
                *dst,
                *false_val,
                context.function_symbol,
                "callable select false arm changes signature",
            )
        }
        ArcInstr::PartialApply {
            dst, func, args, ..
        }
        | ArcInstr::Construct {
            dst,
            ctor: CtorKind::Closure { func },
            args,
            ..
        } => validate_constructed_signature(context, *dst, *func, args),
        ArcInstr::ApplyIndirect {
            dst,
            ty,
            closure,
            args,
            ..
        } => validate_indirect_signature(
            context.enclosing,
            context.enclosing_facts,
            *closure,
            args,
            *dst,
            *ty,
            context.function_symbol,
        ),
        _ => Ok(()),
    }
}

fn validate_terminator(
    function: &ArcFunction,
    facts: &FunctionCallableFacts,
    terminator: &ArcTerminator,
    function_symbol: &str,
) -> Result<(), RealizationError> {
    match terminator {
        ArcTerminator::Jump { target, args } => {
            let Some(target_block) = function.blocks.get(target.index()) else {
                return invalid(
                    function,
                    function_symbol,
                    "callable jump references a missing target block",
                );
            };
            if target_block.params.len() != args.len() {
                return invalid(
                    function,
                    function_symbol,
                    "callable jump argument count disagrees with its target block",
                );
            }
            for ((destination, _), source) in target_block.params.iter().zip(args) {
                validate_same_signature(
                    function,
                    facts,
                    *destination,
                    *source,
                    function_symbol,
                    "callable block argument changes signature",
                )?;
            }
            Ok(())
        }
        ArcTerminator::InvokeIndirect {
            dst,
            ty,
            closure,
            args,
            ..
        } => {
            validate_indirect_signature(function, facts, *closure, args, *dst, *ty, function_symbol)
        }
        _ => Ok(()),
    }
}

fn validate_constructed_signature(
    context: &CallableValidationContext<'_>,
    destination: ori_arc::ArcVarId,
    target: Name,
    captures: &[ori_arc::ArcVarId],
) -> Result<(), RealizationError> {
    let Some(target_function) = context.by_name.get(&target).copied() else {
        return invalid(
            context.enclosing,
            context.function_symbol,
            "closure construction has no realized target",
        );
    };
    let Some(signature) = context.enclosing_facts.signature(destination) else {
        return invalid(
            context.enclosing,
            context.function_symbol,
            "closure construction destination has no callable signature",
        );
    };
    let residual_parameters = target_function
        .params
        .get(captures.len()..)
        .unwrap_or_default()
        .iter()
        .map(|parameter| parameter.ty)
        .collect::<Vec<_>>();
    if captures.len() > target_function.params.len()
        || signature.parameters() != residual_parameters
        || signature.result() != target_function.return_type
    {
        return invalid(
            context.enclosing,
            context.function_symbol,
            format!(
                "closure construction at v{} targets '{}' with {} captures, but callable type {:?} has params {:?} -> {:?} while the realized residual target requires {:?} -> {:?}; this is a compiler-produced type-identity mismatch — report this complete line",
                destination.raw(),
                context.symbols.lookup(target),
                captures.len(),
                signature.ty(),
                signature.parameters(),
                signature.result(),
                residual_parameters,
                target_function.return_type,
            ),
        );
    }
    for (&capture, target_parameter) in captures.iter().zip(&target_function.params) {
        if context.enclosing.var_types.get(capture.index()) != Some(&target_parameter.ty) {
            return invalid(
                context.enclosing,
                context.function_symbol,
                "closure capture type identity disagrees with its target parameter",
            );
        }
    }
    Ok(())
}

fn validate_indirect_signature(
    function: &ArcFunction,
    facts: &FunctionCallableFacts,
    closure: ori_arc::ArcVarId,
    arguments: &[ori_arc::ArcVarId],
    destination: ori_arc::ArcVarId,
    result: Idx,
    function_symbol: &str,
) -> Result<(), RealizationError> {
    let Some(signature) = facts.signature(closure) else {
        return invalid(
            function,
            function_symbol,
            "indirect call operand has no callable signature",
        );
    };
    if signature.parameters().len() != arguments.len() {
        return invalid(
            function,
            function_symbol,
            "indirect call arity disagrees with its callable signature",
        );
    }
    for (&argument, &expected) in arguments.iter().zip(signature.parameters()) {
        if function.var_types.get(argument.index()) != Some(&expected) {
            return invalid(
                function,
                function_symbol,
                "indirect call argument type identity disagrees with its callable signature",
            );
        }
    }
    if signature.result() != result || function.var_types.get(destination.index()) != Some(&result)
    {
        return invalid(
            function,
            function_symbol,
            "indirect call result type identity disagrees with its callable signature",
        );
    }
    Ok(())
}

fn validate_same_signature(
    function: &ArcFunction,
    facts: &FunctionCallableFacts,
    left: ori_arc::ArcVarId,
    right: ori_arc::ArcVarId,
    function_symbol: &str,
    details: &'static str,
) -> Result<(), RealizationError> {
    let Some(left_signature) = facts.register_signatures().get(left.index()) else {
        return invalid(function, function_symbol, details);
    };
    let Some(right_signature) = facts.register_signatures().get(right.index()) else {
        return invalid(function, function_symbol, details);
    };
    if left_signature == right_signature {
        Ok(())
    } else {
        invalid(function, function_symbol, details)
    }
}

fn invalid<T>(
    function: &ArcFunction,
    function_symbol: &str,
    details: impl Into<Box<str>>,
) -> Result<T, RealizationError> {
    Err(RealizationError::InvalidCallableFacts {
        function: function.name,
        function_symbol: function_symbol.into(),
        details: details.into(),
    })
}

#[cfg(test)]
mod tests {
    use ori_arc::{ArcBlock, ArcBlockId, ArcParam, ArcVarId, ArgOwnership, Ownership};
    use ori_ir::SharedInterner;
    use ori_types::Pool;

    use super::*;

    struct Fixture {
        functions: Vec<ArcFunction>,
        facts: FxHashMap<Name, FunctionCallableFacts>,
        symbols: SharedInterner,
        main: Name,
        closure_ty: Idx,
    }

    fn fixture() -> Fixture {
        let symbols = SharedInterner::new();
        let main = symbols.intern("main");
        let lambda = symbols.intern("lambda");
        let mut pool = Pool::new();
        let closure_ty = pool.function(&[Idx::INT], Idx::INT);
        let main_function = ArcFunction {
            name: main,
            return_type: Idx::INT,
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    ArcInstr::PartialApply {
                        dst: ArcVarId::new(0),
                        ty: closure_ty,
                        func: lambda,
                        args: Vec::new(),
                    },
                    ArcInstr::ApplyIndirect {
                        dst: ArcVarId::new(2),
                        ty: Idx::INT,
                        closure: ArcVarId::new(0),
                        args: vec![ArcVarId::new(1)],
                        arg_ownership: vec![ArgOwnership::Borrowed],
                    },
                ],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            }],
            var_types: vec![closure_ty, Idx::INT, Idx::INT],
            ..ArcFunction::default()
        };
        let lambda_function = ArcFunction {
            name: lambda,
            params: vec![ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::INT,
                ownership: Ownership::Borrowed,
            }],
            return_type: Idx::INT,
            var_types: vec![Idx::INT],
            ..ArcFunction::default()
        };
        let functions = vec![main_function, lambda_function];
        let facts = ori_arc::freeze_function_callable_facts(&functions, &pool);
        Fixture {
            functions,
            facts,
            symbols,
            main,
            closure_ty,
        }
    }

    #[test]
    fn accepts_exact_residual_signature_without_pool_access() {
        let fixture = fixture();
        assert!(
            freeze_callable_facts(&fixture.functions, fixture.facts, &fixture.symbols,).is_ok()
        );
    }

    #[test]
    fn rejects_indirect_call_without_callable_signature() {
        let mut fixture = fixture();
        fixture.facts.insert(
            fixture.main,
            FunctionCallableFacts::from_register_signatures(vec![None, None, None]),
        );
        let Err(error) = freeze_callable_facts(&fixture.functions, fixture.facts, &fixture.symbols)
        else {
            panic!("an indirect call without a callable signature unexpectedly validated")
        };
        assert!(matches!(
            error,
            RealizationError::InvalidCallableFacts { details, .. }
                if details.contains("closure construction destination")
        ));
    }

    #[test]
    fn rejects_indirect_call_arity_drift_before_backend_projection() {
        let mut fixture = fixture();
        fixture.facts.insert(
            fixture.main,
            FunctionCallableFacts::from_register_signatures(vec![
                Some(ClosureValueSignature::from_parts(
                    fixture.closure_ty,
                    Vec::new(),
                    Idx::INT,
                )),
                None,
                None,
            ]),
        );
        let Err(error) = freeze_callable_facts(&fixture.functions, fixture.facts, &fixture.symbols)
        else {
            panic!("an indirect call with arity drift unexpectedly validated")
        };
        assert!(matches!(
            error,
            RealizationError::InvalidCallableFacts { details, .. }
                if details.contains("residual target")
        ));
    }
}
