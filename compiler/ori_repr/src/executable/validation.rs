//! Structural validation for the closed executable artifact.

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, MethodCallForm};
use ori_ir::StringInterner;
use ori_types::Pool;

use super::{CallPosition, RealizationError};

pub(super) fn validate_function_metadata(
    functions: &[ArcFunction],
    pool: &Pool,
    symbols: &StringInterner,
) -> Result<(), RealizationError> {
    let classifier = ori_arc::ArcClassifier::new(pool);
    for function in functions {
        let function_symbol: Box<str> = symbols.lookup(function.name).into();
        if let Err(violation) = ori_arc::assert_no_unresolved_bound_vars(pool, function) {
            return Err(RealizationError::UnresolvedBoundVar {
                function: violation.function,
                function_symbol,
                var_id: violation.var_id,
                idx: violation.idx,
            });
        }
        validate_call_ownership(function, &function_symbol)?;
        if let Some(fact) = function.operator_call_facts.first() {
            return Err(RealizationError::InvalidGeneratedCallProvenance {
                function: function.name,
                function_symbol,
                details: format!(
                    "unresolved operator {:?} remains at register {:?}",
                    fact.operation, fact.destination
                )
                .into_boxed_str(),
            });
        }
        validate_method_call_facts(function, pool, &function_symbol)?;
        validate_direct_call_facts(function, &function_symbol)?;
        if let Err(errors) = ori_arc::validate_primitive_facts(function, &classifier) {
            let details = errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
                .into_boxed_str();
            return Err(RealizationError::InvalidPrimitiveFacts {
                function: function.name,
                function_symbol,
                details,
            });
        }
        let variables = function.var_types.len();
        if function.var_metadata_state != ori_arc::VariableMetadataState::Realized {
            return Err(RealizationError::VariableMetadataUnrealized {
                function: function.name,
                function_symbol,
            });
        }
        if function.var_reprs.len() != variables {
            return Err(RealizationError::RepresentationMetadata {
                function: function.name,
                function_symbol,
                variables,
                representations: function.var_reprs.len(),
            });
        }
        if function.var_rc_strategies.len() != variables {
            return Err(RealizationError::RcStrategyMetadata {
                function: function.name,
                function_symbol,
                variables,
                strategies: function.var_rc_strategies.len(),
            });
        }
        for (register, (&representation, &strategy)) in function
            .var_reprs
            .iter()
            .zip(&function.var_rc_strategies)
            .enumerate()
        {
            let shape_matches = match representation {
                ori_arc::ValueRepr::Scalar => strategy.is_none(),
                ori_arc::ValueRepr::RcPointer
                | ori_arc::ValueRepr::Aggregate
                | ori_arc::ValueRepr::FatValue => strategy.is_some(),
            };
            if !shape_matches {
                return Err(RealizationError::RcStrategyShape {
                    function: function.name,
                    function_symbol,
                    register,
                    representation,
                    strategy,
                });
            }
        }
        let expected = ori_arc::ir::compute_var_rc_strategies(function, pool);
        for (register, (&expected, &found)) in
            expected.iter().zip(&function.var_rc_strategies).enumerate()
        {
            if expected != found {
                return Err(RealizationError::RcStrategyCoherence {
                    function: function.name,
                    function_symbol,
                    register,
                    expected,
                    found,
                });
            }
        }
    }
    Ok(())
}

fn validate_method_call_facts(
    function: &ArcFunction,
    pool: &Pool,
    function_symbol: &str,
) -> Result<(), RealizationError> {
    let mut seen = rustc_hash::FxHashSet::default();
    let mut positions = rustc_hash::FxHashSet::default();
    for fact in &function.method_call_facts {
        if !seen.insert(fact.destination) {
            return Err(RealizationError::DuplicateMethodCallFact {
                function: function.name,
                function_symbol: function_symbol.into(),
                destination: fact.destination,
            });
        }
        let arguments = direct_call_arguments(function, fact.destination).ok_or_else(|| {
            RealizationError::OrphanMethodCallFact {
                function: function.name,
                function_symbol: function_symbol.into(),
                destination: fact.destination,
            }
        })?;
        match (&fact.producer, fact.derived_position) {
            (Some(ori_types::MethodProducer::Prelude(_)), Some(_)) => {
                return Err(RealizationError::InvalidGeneratedCallProvenance {
                    function: function.name,
                    function_symbol: function_symbol.into(),
                    details: format!(
                        "method-call register {:?} carries a free-function producer",
                        fact.destination
                    )
                    .into_boxed_str(),
                });
            }
            (Some(_), Some(position)) => {
                if !positions.insert(position) {
                    return Err(RealizationError::InvalidGeneratedCallProvenance {
                        function: function.name,
                        function_symbol: function_symbol.into(),
                        details: format!(
                            "multiple method calls claim structural position {position:?}"
                        )
                        .into_boxed_str(),
                    });
                }
            }
            (None, None) => {}
            _ => {
                return Err(RealizationError::InvalidGeneratedCallProvenance {
                    function: function.name,
                    function_symbol: function_symbol.into(),
                    details: format!(
                        "method-call register {:?} has only one half of producer/position provenance",
                        fact.destination
                    )
                    .into_boxed_str(),
                });
            }
        }
        if fact.form == MethodCallForm::Instance {
            let receiver =
                arguments
                    .first()
                    .ok_or_else(|| RealizationError::MissingMethodReceiver {
                        function: function.name,
                        function_symbol: function_symbol.into(),
                        destination: fact.destination,
                    })?;
            let found = function
                .var_types
                .get(receiver.index())
                .copied()
                .ok_or_else(|| RealizationError::MissingMethodReceiver {
                    function: function.name,
                    function_symbol: function_symbol.into(),
                    destination: fact.destination,
                })?;
            let expected = pool.resolve_fully(fact.receiver_type);
            let found = pool.resolve_fully(found);
            if expected != found {
                return Err(RealizationError::MethodReceiverMismatch {
                    function: function.name,
                    function_symbol: function_symbol.into(),
                    destination: fact.destination,
                    expected,
                    found,
                });
            }
        }
    }
    Ok(())
}

fn validate_direct_call_facts(
    function: &ArcFunction,
    function_symbol: &str,
) -> Result<(), RealizationError> {
    let method_destinations: rustc_hash::FxHashSet<_> = function
        .method_call_facts
        .iter()
        .map(|fact| fact.destination)
        .collect();
    let mut destinations = rustc_hash::FxHashSet::default();
    let mut positions = rustc_hash::FxHashSet::default();
    for fact in &function.direct_call_facts {
        if method_destinations.contains(&fact.destination) || !destinations.insert(fact.destination)
        {
            return Err(RealizationError::InvalidGeneratedCallProvenance {
                function: function.name,
                function_symbol: function_symbol.into(),
                details: format!("multiple call facts claim register {:?}", fact.destination)
                    .into_boxed_str(),
            });
        }
        if direct_call_arguments(function, fact.destination).is_none() {
            return Err(RealizationError::InvalidGeneratedCallProvenance {
                function: function.name,
                function_symbol: function_symbol.into(),
                details: format!(
                    "direct-call fact at register {:?} has no emitted call",
                    fact.destination
                )
                .into_boxed_str(),
            });
        }
        if !positions.insert(fact.derived_position) {
            return Err(RealizationError::InvalidGeneratedCallProvenance {
                function: function.name,
                function_symbol: function_symbol.into(),
                details: format!(
                    "multiple direct calls claim structural position {:?}",
                    fact.derived_position
                )
                .into_boxed_str(),
            });
        }
        if matches!(fact.producer, ori_types::MethodProducer::Registry(_)) {
            return Err(RealizationError::InvalidGeneratedCallProvenance {
                function: function.name,
                function_symbol: function_symbol.into(),
                details: format!(
                    "direct-call register {:?} carries a method producer",
                    fact.destination
                )
                .into_boxed_str(),
            });
        }
    }
    Ok(())
}

fn direct_call_arguments(function: &ArcFunction, destination: ArcVarId) -> Option<&[ArcVarId]> {
    for block in &function.blocks {
        for instruction in &block.body {
            if let ArcInstr::Apply { dst, args, .. } = instruction {
                if *dst == destination {
                    return Some(args);
                }
            }
        }
        if let ArcTerminator::Invoke { dst, args, .. } = &block.terminator {
            if *dst == destination {
                return Some(args);
            }
        }
    }
    None
}

fn validate_call_ownership(
    function: &ArcFunction,
    function_symbol: &str,
) -> Result<(), RealizationError> {
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.body.iter().enumerate() {
            let ownership = match instruction {
                ArcInstr::Apply {
                    args,
                    arg_ownership,
                    ..
                }
                | ArcInstr::ApplyIndirect {
                    args,
                    arg_ownership,
                    ..
                } => Some((args.len(), arg_ownership.len())),
                _ => None,
            };
            if let Some((arguments, ownership_entries)) = ownership {
                validate_call_ownership_lengths(
                    function,
                    function_symbol,
                    block_index,
                    CallPosition::instruction(instruction_index, function.name)?,
                    arguments,
                    ownership_entries,
                )?;
            }
            if let ArcInstr::ApplyIndirect { arg_ownership, .. } = instruction {
                validate_indirect_ownership(
                    function,
                    function_symbol,
                    block_index,
                    CallPosition::instruction(instruction_index, function.name)?,
                    arg_ownership,
                )?;
            }
        }
        let ownership = match &block.terminator {
            ArcTerminator::Invoke {
                args,
                arg_ownership,
                ..
            }
            | ArcTerminator::InvokeIndirect {
                args,
                arg_ownership,
                ..
            } => Some((args.len(), arg_ownership.len())),
            _ => None,
        };
        if let Some((arguments, ownership_entries)) = ownership {
            validate_call_ownership_lengths(
                function,
                function_symbol,
                block_index,
                CallPosition::Terminator,
                arguments,
                ownership_entries,
            )?;
        }
        if let ArcTerminator::InvokeIndirect { arg_ownership, .. } = &block.terminator {
            validate_indirect_ownership(
                function,
                function_symbol,
                block_index,
                CallPosition::Terminator,
                arg_ownership,
            )?;
        }
    }
    Ok(())
}

fn validate_indirect_ownership(
    function: &ArcFunction,
    function_symbol: &str,
    block: usize,
    position: CallPosition,
    ownership: &[ArgOwnership],
) -> Result<(), RealizationError> {
    if let Some(argument) = ownership
        .iter()
        .position(|entry| *entry != ArgOwnership::Borrowed)
    {
        Err(RealizationError::IndirectCallOwnership {
            function: function.name,
            function_symbol: function_symbol.into(),
            block,
            position,
            argument,
        })
    } else {
        Ok(())
    }
}

fn validate_call_ownership_lengths(
    function: &ArcFunction,
    function_symbol: &str,
    block: usize,
    position: CallPosition,
    arguments: usize,
    ownership_entries: usize,
) -> Result<(), RealizationError> {
    if arguments == ownership_entries {
        Ok(())
    } else {
        Err(RealizationError::CallOwnershipMetadata {
            function: function.name,
            function_symbol: function_symbol.into(),
            block,
            position,
            arguments,
            ownership_entries,
        })
    }
}
