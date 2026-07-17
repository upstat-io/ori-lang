//! Binding frozen call plans to generated ARC bodies.

use ori_types::{DerivedCallPlan, Pool};
use oric::ir::Name;
use rustc_hash::FxHashSet;

pub(super) fn bind_derived_call_plan(
    function: &mut ori_arc::ArcFunction,
    plan: &DerivedCallPlan,
    pool: &Pool,
) -> Result<(), String> {
    if function.method_call_facts.len() != plan.calls.len() {
        return Err(format!(
            "generated body emitted {} method calls but its frozen plan contains {}",
            function.method_call_facts.len(),
            plan.calls.len(),
        ));
    }

    let emitted_calls = emitted_direct_calls(function);
    let mut claimed = FxHashSet::default();
    for (fact, selection) in function.method_call_facts.iter_mut().zip(&plan.calls) {
        let Some((_, emitted_name)) = emitted_calls
            .iter()
            .find(|(destination, _)| *destination == fact.destination)
        else {
            return Err(format!(
                "method-call fact at {:?} has no emitted direct call",
                fact.destination,
            ));
        };
        if *emitted_name != selection.method_name {
            return Err(format!(
                "method-call fact at {:?} emits {emitted_name:?}, frozen plan selects {:?}",
                fact.destination, selection.method_name,
            ));
        }
        if fact.receiver_type != selection.receiver_type {
            return Err(format!(
                "method-call fact at {:?} records receiver {:?}, frozen plan selects {:?}",
                fact.destination, fact.receiver_type, selection.receiver_type,
            ));
        }
        let expected_form = if selection.has_self {
            ori_arc::MethodCallForm::Instance
        } else {
            ori_arc::MethodCallForm::Associated
        };
        if fact.form != expected_form {
            return Err(format!(
                "method-call fact at {:?} records {:?}, frozen plan selects {:?}",
                fact.destination, fact.form, expected_form,
            ));
        }
        if fact.producer.is_some() || fact.derived_position.is_some() {
            return Err(format!(
                "method-call fact at {:?} was bound more than once",
                fact.destination,
            ));
        }
        fact.producer = Some(selection.producer.clone());
        fact.derived_position = Some(selection.position);
        claimed.insert(fact.destination);
    }

    function.direct_call_facts.clear();
    for selection in &plan.direct_calls {
        let Some(&(destination, _)) = emitted_calls.iter().find(|(destination, name)| {
            *name == selection.function_name && !claimed.contains(destination)
        }) else {
            return Err(format!(
                "frozen direct-call position {:?} for {:?} has no emitted call",
                selection.position, selection.function_name,
            ));
        };
        claimed.insert(destination);
        function.direct_call_facts.push(ori_arc::DirectCallFact {
            destination,
            producer: selection.producer.clone(),
            derived_position: selection.position,
        });
    }

    if emitted_calls.len() != claimed.len() {
        let unbound: Vec<_> = emitted_calls
            .iter()
            .filter(|(destination, _)| !claimed.contains(destination))
            .collect();
        return Err(format!(
            "generated body contains direct calls without frozen producers: {unbound:?}"
        ));
    }

    for fact in &function.method_call_facts {
        let Some(producer) = &fact.producer else {
            return Err(format!(
                "generated method-call fact at {:?} has no producer",
                fact.destination,
            ));
        };
        if matches!(producer, ori_types::MethodProducer::Prelude(_)) {
            return Err(format!(
                "generated method-call fact at {:?} carries a free-function producer",
                fact.destination,
            ));
        }
        if !pool.is_valid_idx(fact.receiver_type) {
            return Err(format!(
                "generated method-call fact at {:?} carries invalid receiver {:?}",
                fact.destination, fact.receiver_type,
            ));
        }
    }
    Ok(())
}

fn emitted_direct_calls(function: &ori_arc::ArcFunction) -> Vec<(ori_arc::ArcVarId, Name)> {
    let mut calls = Vec::new();
    for block in &function.blocks {
        for instruction in &block.body {
            if let ori_arc::ArcInstr::Apply { dst, func, .. } = instruction {
                calls.push((*dst, *func));
            }
        }
        if let ori_arc::ArcTerminator::Invoke { dst, func, .. } = &block.terminator {
            calls.push((*dst, *func));
        }
    }
    calls
}
