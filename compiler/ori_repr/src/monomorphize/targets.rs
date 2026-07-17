//! Monomorphized call-target realization for post-lowering ARC functions.

use std::collections::HashMap;
use std::hash::BuildHasher;

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, MethodProducer, Pool};
use rustc_hash::FxHashMap;

use super::MonoFunction;

/// Lookup tables used to replace generic call names with concrete function names.
pub struct MonoTargetMaps {
    instances: FxHashMap<MonoInstanceId, Name>,
    generics: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    methods: FxHashMap<(MethodProducer, Idx), Name>,
}

impl MonoTargetMaps {
    /// Build deterministic lookup tables from realized monomorphizations.
    #[must_use]
    pub fn build(mono_functions: &[MonoFunction], pool: &Pool) -> Self {
        let mut mono_by_id = FxHashMap::default();
        let mut mono_by_generic: FxHashMap<Name, Vec<(Vec<Idx>, Name)>> = FxHashMap::default();
        let mut mono_by_method_producer = FxHashMap::default();
        for mono_function in mono_functions {
            for &instance_id in &mono_function.instance_ids {
                mono_by_id.insert(instance_id, mono_function.mangled_name);
            }
            let parameter_types = mono_function
                .sig
                .param_types
                .iter()
                .map(|&ty| pool.resolve_fully(ty))
                .collect();
            mono_by_generic
                .entry(mono_function.original_name)
                .or_default()
                .push((parameter_types, mono_function.mangled_name));
            let producer = match mono_function.origin {
                super::MonoFunctionOrigin::Impl(id) => Some(MethodProducer::Impl(id)),
                super::MonoFunctionOrigin::Derived(id) => Some(MethodProducer::Derived(id)),
                super::MonoFunctionOrigin::Source => None,
            };
            if let (Some(producer), Some(receiver)) = (producer, mono_function.receiver_type) {
                mono_by_method_producer
                    .entry((producer, receiver))
                    .or_insert(mono_function.mangled_name);
            }
        }
        Self {
            instances: mono_by_id,
            generics: mono_by_generic,
            methods: mono_by_method_producer,
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
}

/// Rewrite every cached function to use concrete monomorphized call targets.
pub fn rewrite_apply_targets_for_monos<S: BuildHasher>(
    arc_cache: &mut HashMap<Name, (ArcFunction, Vec<ArcFunction>), S>,
    mono_functions: &[MonoFunction],
    pool: &Pool,
    interner: &StringInterner,
) {
    let maps = MonoTargetMaps::build(mono_functions, pool);
    for (function, lambdas) in arc_cache.values_mut() {
        maps.rewrite_function(function, lambdas, pool, interner);
    }
}

fn rewrite_function_targets(
    function: &mut ArcFunction,
    mono_by_id: &FxHashMap<MonoInstanceId, Name>,
    mono_by_generic: &FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
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
            let ArcInstr::Apply {
                dst,
                func,
                args,
                mono_instance_id,
                ..
            } = instruction
            else {
                continue;
            };
            if let Some(target) =
                resolver.changed_target(*dst, *func, args, *mono_instance_id, function)
            {
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
            if let Some(target) =
                resolver.changed_target(*dst, *func, args, *mono_instance_id, function)
            {
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
    mono_by_id: &'a FxHashMap<MonoInstanceId, Name>,
    mono_by_generic: &'a FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
    mono_by_method_producer: &'a FxHashMap<(MethodProducer, Idx), Name>,
    pool: &'a Pool,
    interner: &'a StringInterner,
}

impl TargetResolver<'_> {
    fn changed_target(
        &self,
        destination: ArcVarId,
        callee: Name,
        arguments: &[ArcVarId],
        instance_id: Option<MonoInstanceId>,
        function: &ArcFunction,
    ) -> Option<Name> {
        let target = self.resolve_target(destination, callee, arguments, instance_id, function)?;
        (target != callee).then_some(target)
    }

    fn resolve_target(
        &self,
        destination: ArcVarId,
        callee: Name,
        arguments: &[ArcVarId],
        instance_id: Option<MonoInstanceId>,
        function: &ArcFunction,
    ) -> Option<Name> {
        if let Some(target) = instance_id.and_then(|id| self.mono_by_id.get(&id)).copied() {
            return Some(target);
        }
        if let Some(fact) = function.method_call_fact(destination) {
            return fact.producer.and_then(|producer| {
                self.mono_by_method_producer
                    .get(&(producer, fact.receiver_type))
                    .copied()
            });
        }
        let argument_types: Vec<Idx> = arguments
            .iter()
            .map(|argument| self.pool.resolve_fully(function.var_type(*argument)))
            .collect();
        let skip_self =
            callee_shadows_builtin_method(self.pool, self.interner, callee, arguments, function);
        self.mono_by_generic
            .get(&callee)?
            .iter()
            .find_map(|(parameters, target)| {
                let target_matches = (!skip_self || *target != function.name)
                    && parameters.len() == argument_types.len()
                    && parameters
                        .iter()
                        .zip(&argument_types)
                        .all(|(&parameter, &argument)| {
                            self.pool.structural_eq(parameter, argument)
                        });
                target_matches.then_some(*target)
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
    let receiver_type = pool.resolve_fully(function.var_type(receiver));
    let Some(tag) = pool.builtin_type_tag(receiver_type) else {
        return false;
    };
    ori_registry::has_method(tag, interner.lookup(callee))
}

fn apply_target_update(function: &mut ArcFunction, update: TargetUpdate) {
    let block = &mut function.blocks[update.block];
    if let Some(instruction_index) = update.instruction {
        if let ArcInstr::Apply { func, .. } = &mut block.body[instruction_index] {
            *func = update.target;
        }
    } else if let ArcTerminator::Invoke { func, .. } = &mut block.terminator {
        *func = update.target;
    }
}

#[cfg(test)]
mod tests {
    use ori_arc::{
        ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership,
        MethodCallFact, MethodCallForm,
    };
    use ori_ir::{DerivedImplId, Name, StringInterner};
    use ori_types::{FunctionSig, Idx, Pool};
    use rustc_hash::FxHashMap;

    use super::MonoTargetMaps;
    use crate::monomorphize::{MonoFunction, MonoFunctionOrigin};

    #[test]
    fn generated_method_fact_is_reserved_for_exact_receiver_rewrite() {
        let interner = StringInterner::new();
        let hash = interner.intern("hash");
        let concrete_hash = interner.intern("hash$m$3_int$im$");
        let mut function = ArcFunction {
            name: interner.intern("generated_outer_hash"),
            var_types: vec![Idx::INT, Idx::INT],
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::Apply {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    func: hash,
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(1),
                },
            }],
            method_call_facts: vec![MethodCallFact {
                destination: ArcVarId::new(1),
                receiver_type: Idx::INT,
                form: MethodCallForm::Instance,
                producer: None,
                derived_position: None,
            }],
            ..ArcFunction::default()
        };
        let mono = MonoFunction {
            mangled_name: concrete_hash,
            original_name: hash,
            origin: MonoFunctionOrigin::Derived(DerivedImplId::new(0)),
            sig: FunctionSig::synthetic(hash, vec![Name::from_raw(9)], vec![Idx::INT], Idx::INT),
            body_type_map: FxHashMap::default(),
            instance_ids: Vec::new(),
            is_imported: false,
            receiver_type: Some(Idx::INT),
            receiver_type_name: None,
        };

        MonoTargetMaps::build(&[mono], &Pool::new()).rewrite_function(
            &mut function,
            &mut [],
            &Pool::new(),
            &interner,
        );

        let ArcInstr::Apply { func, .. } = function.blocks[0].body[0] else {
            panic!("test fixture must remain an apply")
        };
        assert_eq!(func, hash);
    }
}
