//! Monomorphized call-target realization for post-lowering ARC functions.

use std::collections::HashMap;
use std::hash::BuildHasher;

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
use ori_ir::canon::MonoInstanceId;
use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use super::MonoFunction;

/// Lookup tables used to replace generic call names with concrete function names.
pub struct MonoTargetMaps {
    mono_by_id: FxHashMap<MonoInstanceId, Name>,
    mono_by_generic: FxHashMap<Name, Vec<(Vec<Idx>, Name)>>,
}

impl MonoTargetMaps {
    /// Build deterministic lookup tables from realized monomorphizations.
    #[must_use]
    pub fn build(mono_functions: &[MonoFunction], pool: &Pool) -> Self {
        let mut mono_by_id = FxHashMap::default();
        let mut mono_by_generic: FxHashMap<Name, Vec<(Vec<Idx>, Name)>> = FxHashMap::default();
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
        }
        Self {
            mono_by_id,
            mono_by_generic,
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
            &self.mono_by_id,
            &self.mono_by_generic,
            pool,
            interner,
        );
        for lambda in lambdas {
            rewrite_function_targets(
                lambda,
                &self.mono_by_id,
                &self.mono_by_generic,
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
    pool: &Pool,
    interner: &StringInterner,
) {
    let resolver = TargetResolver {
        mono_by_id,
        mono_by_generic,
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
                func,
                args,
                mono_instance_id,
                ..
            } = instruction
            else {
                continue;
            };
            if let Some(target) = resolver.changed_target(*func, args, *mono_instance_id, function)
            {
                updates.push(TargetUpdate {
                    block: block_index,
                    instruction: Some(instruction_index),
                    target,
                });
            }
        }
        if let ArcTerminator::Invoke {
            func,
            args,
            mono_instance_id,
            ..
        } = &block.terminator
        {
            if let Some(target) = resolver.changed_target(*func, args, *mono_instance_id, function)
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
    pool: &'a Pool,
    interner: &'a StringInterner,
}

impl TargetResolver<'_> {
    fn changed_target(
        &self,
        callee: Name,
        arguments: &[ArcVarId],
        instance_id: Option<MonoInstanceId>,
        function: &ArcFunction,
    ) -> Option<Name> {
        let target = self.resolve_target(callee, arguments, instance_id, function)?;
        (target != callee).then_some(target)
    }

    fn resolve_target(
        &self,
        callee: Name,
        arguments: &[ArcVarId],
        instance_id: Option<MonoInstanceId>,
        function: &ArcFunction,
    ) -> Option<Name> {
        if let Some(target) = instance_id.and_then(|id| self.mono_by_id.get(&id)).copied() {
            return Some(target);
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
