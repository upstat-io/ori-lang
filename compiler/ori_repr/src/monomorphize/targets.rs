//! Monomorphized call-target realization for post-lowering ARC functions.

use std::collections::HashMap;
use std::hash::BuildHasher;

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId, CtorKind};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, MethodProducer, Pool};
use rustc_hash::FxHashMap;

use super::MonoFunction;

type MonoSignature = (Name, Vec<u64>, u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct MonoCandidate {
    original_name: Name,
    parameter_types: Vec<Idx>,
    return_type: Idx,
    target: Name,
    method_producer: Option<MethodProducer>,
}

impl MonoCandidate {
    fn matches_signature(&self, argument_types: &[Idx], return_type: Idx, pool: &Pool) -> bool {
        self.parameter_types.len() == argument_types.len()
            && self
                .parameter_types
                .iter()
                .zip(argument_types)
                .all(|(&parameter, &argument)| pool.structural_eq(parameter, argument))
            && pool.structural_eq(self.return_type, return_type)
    }
}

type MonoCandidates = Vec<MonoCandidate>;

/// Lookup tables used to replace generic call names with concrete function names.
pub struct MonoTargetMaps {
    instances: FxHashMap<MonoInstanceId, Option<MonoCandidate>>,
    generics: FxHashMap<MonoSignature, MonoCandidates>,
    methods: FxHashMap<(MethodProducer, Idx), Name>,
    method_representations: FxHashMap<(MethodProducer, Idx), Option<Name>>,
}

impl MonoTargetMaps {
    /// Create deterministic lookup tables from realized monomorphizations.
    #[must_use]
    pub fn new(mono_functions: &[MonoFunction], pool: &Pool) -> Self {
        let mut mono_by_id: FxHashMap<MonoInstanceId, Option<MonoCandidate>> = FxHashMap::default();
        let mut mono_by_generic: FxHashMap<MonoSignature, MonoCandidates> = FxHashMap::default();
        let mut mono_by_method_producer = FxHashMap::default();
        let mut mono_by_method_representation = FxHashMap::default();
        for mono_function in mono_functions {
            let parameter_types = mono_function
                .sig
                .param_types
                .iter()
                .map(|&ty| pool.resolve_fully(ty))
                .collect::<Vec<_>>();
            let signature_hashes = parameter_types
                .iter()
                .map(|&ty| pool.hash(ty))
                .collect::<Vec<_>>();
            let return_type = pool.resolve_fully(mono_function.sig.return_type);
            let return_hash = pool.hash(return_type);
            let producer = mono_function.identity.method_producer().cloned();
            let candidate = MonoCandidate {
                original_name: mono_function.identity.original_name(),
                parameter_types,
                return_type,
                target: mono_function.mangled_name,
                method_producer: producer.clone(),
            };
            for &instance_id in mono_function.identity.instance_ids() {
                mono_by_id
                    .entry(instance_id)
                    .and_modify(|existing| {
                        if existing.as_ref() != Some(&candidate) {
                            *existing = None;
                        }
                    })
                    .or_insert_with(|| Some(candidate.clone()));
            }
            if producer.is_none() {
                mono_by_generic
                    .entry((
                        mono_function.identity.original_name(),
                        signature_hashes,
                        return_hash,
                    ))
                    .or_default()
                    .push(candidate);
            }
            if let (Some(producer), Some(receiver), true) = (
                producer,
                mono_function.identity.receiver_type(),
                mono_function.identity.method_args().is_empty(),
            ) {
                let semantic_receiver = pool.method_receiver_key(receiver);
                mono_by_method_producer
                    .entry((producer.clone(), semantic_receiver))
                    .or_insert(mono_function.mangled_name);
                let representation_receiver = pool.method_receiver_type(receiver);
                mono_by_method_representation
                    .entry((producer, representation_receiver))
                    .and_modify(|target| {
                        if *target != Some(mono_function.mangled_name) {
                            *target = None;
                        }
                    })
                    .or_insert(Some(mono_function.mangled_name));
            }
        }
        Self {
            instances: mono_by_id,
            generics: mono_by_generic,
            methods: mono_by_method_producer,
            method_representations: mono_by_method_representation,
        }
    }

    /// Rewrite a function and its lambdas to concrete monomorphized targets.
    pub fn rewrite_function(
        &self,
        function: &mut ArcFunction,
        lambdas: &mut [ArcFunction],
        pool: &Pool,
        interner: &StringInterner,
    ) {
        rewrite_function_targets(
            function,
            &self.instances,
            &self.generics,
            &self.methods,
            pool,
            interner,
        );
        for lambda in lambdas {
            rewrite_function_targets(
                lambda,
                &self.instances,
                &self.generics,
                &self.methods,
                pool,
                interner,
            );
        }
    }

    /// Resolve one producer-qualified concrete method target.
    ///
    /// This is the same exact table used by the batch rewrite. Generated-body
    /// binding uses it to close a frozen derived selection while retaining the
    /// emitted operand's physical receiver type for executable validation.
    #[must_use]
    pub fn exact_method_target(
        &self,
        producer: &MethodProducer,
        receiver: Idx,
        pool: &Pool,
    ) -> Option<Name> {
        let semantic_receiver = pool.method_receiver_key(receiver);
        self.methods
            .get(&(producer.clone(), semantic_receiver))
            .copied()
            .or_else(|| {
                let representation_receiver = pool.method_receiver_type(receiver);
                self.method_representations
                    .get(&(producer.clone(), representation_receiver))
                    .copied()
                    .flatten()
            })
    }
}

/// Rewrite every cached function to use concrete monomorphized call targets.
pub fn rewrite_apply_targets_for_monos<S: BuildHasher>(
    arc_cache: &mut HashMap<Name, (ArcFunction, Vec<ArcFunction>), S>,
    mono_functions: &[MonoFunction],
    pool: &Pool,
    interner: &StringInterner,
) {
    let maps = MonoTargetMaps::new(mono_functions, pool);
    for (function, lambdas) in arc_cache.values_mut() {
        maps.rewrite_function(function, lambdas, pool, interner);
    }
}

fn rewrite_function_targets(
    function: &mut ArcFunction,
    mono_by_id: &FxHashMap<MonoInstanceId, Option<MonoCandidate>>,
    mono_by_generic: &FxHashMap<MonoSignature, MonoCandidates>,
    mono_by_method_producer: &FxHashMap<(MethodProducer, Idx), Name>,
    pool: &Pool,
    interner: &StringInterner,
) {
    let resolver = TargetResolver {
        mono_by_id,
        mono_by_generic,
        mono_by_method_producer,
        pool,
        interner,
    };
    let updates = collect_target_updates(function, &resolver);
    for update in updates {
        apply_target_update(function, update);
    }
}

#[derive(Clone, Copy)]
struct TargetUpdate {
    block: usize,
    instruction: Option<usize>,
    target: Name,
}

fn collect_target_updates(
    function: &ArcFunction,
    resolver: &TargetResolver<'_>,
) -> Vec<TargetUpdate> {
    let mut updates = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.body.iter().enumerate() {
            let target = match instruction {
                ArcInstr::Apply {
                    dst,
                    func,
                    args,
                    mono_instance_id,
                    ..
                } => resolver.changed(*dst, *func, args, *mono_instance_id, function),
                ArcInstr::PartialApply { ty, func, args, .. }
                | ArcInstr::Construct {
                    ty,
                    ctor: CtorKind::Closure { func },
                    args,
                    ..
                } => resolver.changed_function_value(*func, *ty, args, function),
                _ => None,
            };
            if let Some(target) = target {
                updates.push(TargetUpdate {
                    block: block_index,
                    instruction: Some(instruction_index),
                    target,
                });
            }
        }
        if let ArcTerminator::Invoke {
            dst,
            func,
            args,
            mono_instance_id,
            ..
        } = &block.terminator
        {
            if let Some(target) = resolver.changed(*dst, *func, args, *mono_instance_id, function) {
                updates.push(TargetUpdate {
                    block: block_index,
                    instruction: None,
                    target,
                });
            }
        }
    }
    updates
}

struct TargetResolver<'a> {
    mono_by_id: &'a FxHashMap<MonoInstanceId, Option<MonoCandidate>>,
    mono_by_generic: &'a FxHashMap<MonoSignature, MonoCandidates>,
    mono_by_method_producer: &'a FxHashMap<(MethodProducer, Idx), Name>,
    pool: &'a Pool,
    interner: &'a StringInterner,
}

impl TargetResolver<'_> {
    fn changed(
        &self,
        destination: ArcVarId,
        callee: Name,
        arguments: &[ArcVarId],
        instance_id: Option<MonoInstanceId>,
        function: &ArcFunction,
    ) -> Option<Name> {
        let target = self.resolve(destination, callee, arguments, instance_id, function)?;
        (target != callee).then_some(target)
    }

    fn resolve(
        &self,
        destination: ArcVarId,
        callee: Name,
        arguments: &[ArcVarId],
        instance_id: Option<MonoInstanceId>,
        function: &ArcFunction,
    ) -> Option<Name> {
        if function
            .operator_call_facts
            .iter()
            .any(|fact| fact.destination == destination)
        {
            // Operator placeholders are closed after lambda/type
            // specialization by the receiver-qualified realization map. A
            // name/signature fallback here could cross-wire same-spelled impls.
            return None;
        }
        let argument_types: Vec<Idx> = arguments
            .iter()
            .map(|argument| self.pool.resolve_fully(function.var_type(*argument)))
            .collect();
        let return_type = self.pool.resolve_fully(function.var_type(destination));
        if let Some(candidate) = instance_id
            .and_then(|id| self.mono_by_id.get(&id))
            .and_then(Option::as_ref)
            .filter(|candidate| candidate.original_name == callee)
        {
            if candidate.method_producer.is_some()
                || candidate.matches_signature(&argument_types, return_type, self.pool)
            {
                return Some(candidate.target);
            }
        }
        if let Some(fact) = function.method_call_fact(destination) {
            let receiver = self.pool.method_receiver_key(fact.receiver_type);
            return fact.producer.and_then(|producer| {
                self.mono_by_method_producer
                    .get(&(producer, receiver))
                    .copied()
            });
        }
        let skip_self =
            callee_shadows_builtin_method(self.pool, self.interner, callee, arguments, function);
        self.resolve_generic_target(
            callee,
            &argument_types,
            return_type,
            skip_self.then_some(function.name),
        )
    }

    fn changed_function_value(
        &self,
        callee: Name,
        function_type: Idx,
        captured: &[ArcVarId],
        function: &ArcFunction,
    ) -> Option<Name> {
        let function_type = self.pool.resolve_fully(function_type);
        if self.pool.tag(function_type) != ori_types::Tag::Function {
            return None;
        }
        let mut argument_types: Vec<_> = captured
            .iter()
            .map(|argument| self.pool.resolve_fully(function.var_type(*argument)))
            .collect();
        argument_types.extend(
            self.pool
                .function_params(function_type)
                .iter()
                .map(|&ty| self.pool.resolve_fully(ty)),
        );
        let target = self.resolve_generic_target(
            callee,
            &argument_types,
            self.pool.function_return(function_type),
            None,
        )?;
        (target != callee).then_some(target)
    }

    fn resolve_generic_target(
        &self,
        callee: Name,
        argument_types: &[Idx],
        return_type: Idx,
        excluded_target: Option<Name>,
    ) -> Option<Name> {
        let signature_hashes = argument_types
            .iter()
            .map(|&ty| self.pool.hash(ty))
            .collect::<Vec<_>>();
        let return_type = self.pool.resolve_fully(return_type);
        let return_hash = self.pool.hash(return_type);
        self.mono_by_generic
            .get(&(callee, signature_hashes, return_hash))?
            .iter()
            .find_map(|candidate| {
                let target_matches = excluded_target != Some(candidate.target)
                    && candidate.matches_signature(argument_types, return_type, self.pool);
                target_matches.then_some(candidate.target)
            })
    }
}

/// Return whether a call name denotes a builtin method on its receiver type.
#[must_use]
pub fn callee_shadows_builtin_method(
    pool: &Pool,
    interner: &StringInterner,
    callee: Name,
    arguments: &[ArcVarId],
    function: &ArcFunction,
) -> bool {
    let Some(&receiver) = arguments.first() else {
        return false;
    };
    let receiver_type = function.var_type(receiver);
    let Some(tag) = pool.builtin_method_type_tag(receiver_type) else {
        return false;
    };
    ori_registry::has_method(tag, interner.lookup(callee))
}

fn apply_target_update(function: &mut ArcFunction, update: TargetUpdate) {
    let block = &mut function.blocks[update.block];
    if let Some(instruction_index) = update.instruction {
        match &mut block.body[instruction_index] {
            ArcInstr::Apply { func, .. } | ArcInstr::PartialApply { func, .. } => {
                *func = update.target;
            }
            ArcInstr::Construct {
                ctor: CtorKind::Closure { func },
                ..
            } => *func = update.target,
            _ => {}
        }
    } else if let ArcTerminator::Invoke { func, .. } = &mut block.terminator {
        *func = update.target;
    }
}

#[cfg(test)]
mod tests;
