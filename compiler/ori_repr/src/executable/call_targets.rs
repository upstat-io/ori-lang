//! Closed resolution and signature validation for direct callable references.

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, CtorKind};
use ori_ir::{Name, StringInterner};
use ori_types::Pool;
use rustc_hash::FxHashMap;

use super::{
    BlockIndex, CallPosition, CallSite, CallableTarget, ExternalCallable, ExternalFunctionId,
    FunctionId, RealizationError, RuntimeCall,
};

pub(super) fn validate_external_symbols(
    externals: &[ExternalCallable],
    symbols: &StringInterner,
    function_ids: &FxHashMap<Name, FunctionId>,
) -> Result<(), RealizationError> {
    for external in externals {
        if symbols.try_lookup(external.name()).is_none() {
            return Err(RealizationError::UnknownExternalName {
                name: external.name(),
            });
        }
        if function_ids.contains_key(&external.name()) {
            return Err(RealizationError::ExternalFunctionBodyCollision {
                name: external.name(),
            });
        }
    }
    Ok(())
}

pub(super) struct ResolvedCallTargets {
    pub(super) call_targets: FxHashMap<CallSite, CallableTarget>,
    pub(super) direct_call_targets: FxHashMap<(FunctionId, ori_arc::ArcVarId), CallableTarget>,
}

pub(super) fn build_call_targets(
    functions: &[ArcFunction],
    function_ids: &FxHashMap<Name, FunctionId>,
    external_functions: &[ExternalCallable],
    external_ids: &FxHashMap<Name, ExternalFunctionId>,
    symbols: &StringInterner,
    pool: &Pool,
) -> Result<ResolvedCallTargets, RealizationError> {
    let mut targets = FxHashMap::default();
    let mut direct_targets = FxHashMap::default();
    for function in functions {
        let function_id = function_ids.get(&function.name).copied().ok_or(
            RealizationError::MissingFunctionIdentity {
                name: function.name,
            },
        )?;
        for (block_index, block) in function.blocks.iter().enumerate() {
            let block_id = BlockIndex::new(block_index, function.name)?;
            for (instruction_index, instruction) in block.body.iter().enumerate() {
                let position = CallPosition::instruction(instruction_index, function.name)?;
                if let Some((name, arguments, closure_only, destination)) =
                    instruction_target(instruction)
                {
                    let target = resolve_callable(
                        function,
                        name,
                        destination,
                        function_ids,
                        external_ids,
                        symbols,
                        pool,
                    )?;
                    validate_closure_target(function.name, name, target, closure_only)?;
                    if let (
                        CallableTarget::External(external),
                        ArcInstr::Apply {
                            dst, arg_ownership, ..
                        },
                    ) = (target, instruction)
                    {
                        validate_external_call(
                            function,
                            arguments,
                            *dst,
                            arg_ownership,
                            &external_functions[external.index()],
                        )?;
                    }
                    targets.insert(CallSite::new(function_id, block_id, position), target);
                    if let Some(destination) = destination {
                        if direct_targets
                            .insert((function_id, destination), target)
                            .is_some()
                        {
                            return Err(RealizationError::DuplicateDirectCallResult {
                                function: function.name,
                                destination,
                            });
                        }
                    }
                }
            }
            if let Some((name, arguments, destination)) = terminator_target(&block.terminator) {
                let target = resolve_callable(
                    function,
                    name,
                    Some(destination),
                    function_ids,
                    external_ids,
                    symbols,
                    pool,
                )?;
                if let ArcTerminator::Invoke {
                    dst, arg_ownership, ..
                } = &block.terminator
                {
                    if let CallableTarget::External(external) = target {
                        validate_external_call(
                            function,
                            arguments,
                            *dst,
                            arg_ownership,
                            &external_functions[external.index()],
                        )?;
                    }
                }
                targets.insert(
                    CallSite::new(function_id, block_id, CallPosition::Terminator),
                    target,
                );
                if direct_targets
                    .insert((function_id, destination), target)
                    .is_some()
                {
                    return Err(RealizationError::DuplicateDirectCallResult {
                        function: function.name,
                        destination,
                    });
                }
            }
        }
    }
    Ok(ResolvedCallTargets {
        call_targets: targets,
        direct_call_targets: direct_targets,
    })
}

fn validate_external_call(
    caller: &ArcFunction,
    arguments: &[ori_arc::ArcVarId],
    result: ori_arc::ArcVarId,
    ownership: &[ori_arc::ArgOwnership],
    external: &ExternalCallable,
) -> Result<(), RealizationError> {
    let argument_types = arguments
        .iter()
        .map(|argument| caller.var_types.get(argument.index()).copied())
        .collect::<Option<Vec<_>>>();
    let result_type = caller.var_types.get(result.index()).copied();
    if argument_types.as_deref() != Some(external.parameter_types())
        || result_type != Some(external.return_type())
    {
        return Err(RealizationError::ExternalCallSignatureMismatch {
            caller: caller.name,
            callee: external.name(),
        });
    }
    let expected = external.contract().params.iter().map(|parameter| {
        if parameter.consumption == ori_arc::aims::lattice::Consumption::Dead
            || parameter.access == ori_arc::aims::lattice::AccessClass::Borrowed
        {
            ori_arc::ArgOwnership::Borrowed
        } else {
            ori_arc::ArgOwnership::Owned
        }
    });
    if ownership.iter().copied().ne(expected) {
        return Err(RealizationError::ExternalCallOwnershipMismatch {
            caller: caller.name,
            callee: external.name(),
        });
    }
    Ok(())
}

fn instruction_target(
    instruction: &ArcInstr,
) -> Option<(Name, &[ori_arc::ArcVarId], bool, Option<ori_arc::ArcVarId>)> {
    match instruction {
        ArcInstr::Apply {
            dst, func, args, ..
        } => Some((*func, args, false, Some(*dst))),
        ArcInstr::PartialApply { func, args, .. }
        | ArcInstr::Construct {
            ctor: CtorKind::Closure { func },
            args,
            ..
        } => Some((*func, args, true, None)),
        _ => None,
    }
}

fn terminator_target(
    terminator: &ArcTerminator,
) -> Option<(Name, &[ori_arc::ArcVarId], ori_arc::ArcVarId)> {
    match terminator {
        ArcTerminator::Invoke {
            dst, func, args, ..
        } => Some((*func, args, *dst)),
        _ => None,
    }
}

fn resolve_callable(
    caller: &ArcFunction,
    callee: Name,
    destination: Option<ori_arc::ArcVarId>,
    function_ids: &FxHashMap<Name, FunctionId>,
    external_ids: &FxHashMap<Name, ExternalFunctionId>,
    symbols: &StringInterner,
    pool: &Pool,
) -> Result<CallableTarget, RealizationError> {
    let missing_callable = || RealizationError::MissingCallable {
        caller: caller.name,
        callee,
        caller_symbol: symbols
            .try_lookup(caller.name)
            .unwrap_or("<unknown caller>")
            .into(),
        callee_symbol: symbols
            .try_lookup(callee)
            .unwrap_or("<unknown callee>")
            .into(),
    };
    if let Some(destination) = destination {
        if let Some(fact) = caller.direct_call_fact(destination) {
            if let ori_types::MethodProducer::Prelude(identity) = fact.producer {
                let runtime = identity
                    .resolve()
                    .map(RuntimeCall::RegistryPrelude)
                    .ok_or_else(&missing_callable)?;
                return Ok(CallableTarget::Runtime(runtime));
            }
        }
        if let Some(fact) = caller.method_call_fact(destination) {
            if let Some(ori_types::MethodProducer::Registry(identity)) = fact.producer {
                let runtime = identity
                    .resolve()
                    .map(RuntimeCall::RegistryMethod)
                    .ok_or_else(&missing_callable)?;
                return Ok(CallableTarget::Runtime(runtime));
            }
            // A source builtin method has no frozen producer yet, but its
            // receiver-qualified registry identity is still exact. Resolve
            // that identity before consulting same-spelled free functions;
            // otherwise `Error.trace()` can bind an unrelated global
            // `@trace`. A closed producerless user/derived/imported target,
            // however, already carries an exact function or external `Name`;
            // when no registry method owns that symbol, let the identity
            // tables below resolve it.
            if fact.producer.is_none() {
                let receiver = if pool.is_error_struct_receiver(fact.receiver_type) {
                    Some(ori_registry::TypeTag::Error)
                } else {
                    pool.builtin_method_type_tag(fact.receiver_type)
                };
                let symbol = symbols.try_lookup(callee);
                if let Some(runtime) =
                    symbol.and_then(|symbol| RuntimeCall::resolve(symbol, receiver))
                {
                    return Ok(CallableTarget::Runtime(runtime));
                }
                if receiver
                    .zip(symbol)
                    .is_some_and(|(receiver, symbol)| ori_registry::has_method(receiver, symbol))
                {
                    return Err(missing_callable());
                }
            }
        }
    }
    if let Some(&function) = function_ids.get(&callee) {
        return Ok(CallableTarget::Function(function));
    }
    if let Some(&external) = external_ids.get(&callee) {
        return Ok(CallableTarget::External(external));
    }
    // A producer-qualified user/derived/imported method must have been
    // rewritten to an exact function or external identity above. Never let a
    // stale method spelling bind an unrelated runtime/free callable.
    if destination.is_some_and(|destination| caller.method_call_fact(destination).is_some()) {
        return Err(missing_callable());
    }
    let runtime = symbols
        .try_lookup(callee)
        .and_then(|symbol| RuntimeCall::resolve(symbol, None))
        .ok_or_else(missing_callable)?;
    Ok(CallableTarget::Runtime(runtime))
}

fn validate_closure_target(
    caller: Name,
    callee: Name,
    target: CallableTarget,
    closure_only: bool,
) -> Result<(), RealizationError> {
    if closure_only && !matches!(target, CallableTarget::Function(_)) {
        Err(RealizationError::InvalidClosureTarget { caller, callee })
    } else {
        Ok(())
    }
}
