//! Shared specialization of polymorphic lambda ARC bodies.
//!
//! This pass owns the concrete `BoundVar` substitution required before AIMS
//! freezes primitive descriptors and ownership facts. Every executable backend
//! consumes its output; no backend may specialize or guess unresolved types.

mod call_site;
mod multi_inst;
mod single_inst;
#[cfg(test)]
mod tests;
mod type_predicates;
mod type_resolve;

use ori_types::Tag;

use multi_inst::{detect_and_clone_multi_inst, remove_multi_inst_originals};
use single_inst::build_single_inst_mappings;
use type_predicates::{contains_bound_var, contains_var, first_bound_var};
use type_resolve::{
    apply_bound_var_map, apply_call_site_types, apply_concrete_param_types,
    apply_parent_partial_apply_type, find_all_instantiation_types, find_concrete_types_from_calls,
    find_partial_apply_dst, is_polymorphic_lambda, resolve_lambda_return_types, resolve_type_sites,
    TypeResolution,
};

/// One substituted compound identity that the type phase did not intern.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MissingTypeMaterialization {
    function: ori_ir::Name,
    var_id: crate::ArcVarId,
    source: ori_types::Idx,
}

impl MissingTypeMaterialization {
    pub(super) const fn new(
        function: ori_ir::Name,
        var_id: crate::ArcVarId,
        source: ori_types::Idx,
    ) -> Self {
        Self {
            function,
            var_id,
            source,
        }
    }

    /// Return the ARC function containing the missing type site.
    #[must_use]
    pub const fn function(self) -> ori_ir::Name {
        self.function
    }

    /// Return the ARC variable owning the missing site.
    #[must_use]
    pub const fn var_id(self) -> crate::ArcVarId {
        self.var_id
    }

    /// Return the source type whose concrete identity was absent.
    #[must_use]
    pub const fn source(self) -> ori_types::Idx {
        self.source
    }
}

/// Failure to close every lambda type before AIMS.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LambdaSpecializationError {
    unresolved: Vec<crate::UnresolvedBoundVar>,
    missing_materializations: Vec<MissingTypeMaterialization>,
}

impl LambdaSpecializationError {
    /// Return the first unresolved type site per lambda in deterministic order.
    #[must_use]
    pub fn unresolved(&self) -> &[crate::UnresolvedBoundVar] {
        &self.unresolved
    }

    /// Return type identities that the owning type phase failed to intern.
    #[must_use]
    pub fn missing_materializations(&self) -> &[MissingTypeMaterialization] {
        &self.missing_materializations
    }
}

impl std::fmt::Display for LambdaSpecializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "lambda specialization left {} unresolved bound type variable(s) and found {} concrete type identity/identities missing from the type-checker pool", self.unresolved.len(), self.missing_materializations.len())
    }
}

impl std::error::Error for LambdaSpecializationError {}

/// Specialize every polymorphic lambda owned by one parent ARC function.
///
/// Single-instantiation lambdas are substituted in place. Multi-instantiation
/// lambdas are cloned per concrete function type and the parent's closure sites
/// are rewritten to those exact clones. Surviving `BoundVar` parameters reject
/// the batch; this pass never substitutes a guessed scalar type.
pub fn specialize_polymorphic_lambdas(
    parent: &mut crate::ArcFunction,
    lambdas: &mut Vec<crate::ArcFunction>,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> Result<(), LambdaSpecializationError> {
    // This is the sole sanctioned pre-AIMS type rewrite. Lowering-derived
    // metadata describes the pre-specialized types, so invalidate it before
    // canonicalizing any site. Multi-instantiation clones must inherit this
    // unrealized state rather than stale representation tables.
    parent.invalidate_variable_metadata_for_type_rewrite();
    for lambda in lambdas.iter_mut() {
        lambda.invalidate_variable_metadata_for_type_rewrite();
    }

    // Resolve every type site to identities already materialized by the type
    // phase before deciding whether specialization is needed. ARC may select
    // those identities but cannot extend the canonical Pool.
    let mut resolution = TypeResolution::new(pool);
    let empty = rustc_hash::FxHashMap::default();
    resolve_type_sites(parent, &empty, &mut resolution);
    for lambda in lambdas.iter_mut() {
        resolve_type_sites(lambda, &empty, &mut resolution);
    }
    if resolution.has_missing() {
        return validate_specialized_batch(parent, lambdas, pool, resolution.into_missing());
    }

    remove_dead_non_capturing_templates(parent, lambdas, pool);

    let any_polymorphic = lambdas
        .iter()
        .any(|lambda| is_polymorphic_lambda(lambda, pool));
    let any_multi_inst = !any_polymorphic
        && lambdas
            .iter()
            .any(|lambda| find_all_instantiation_types(parent, lambda.name, pool).len() > 1);
    if !any_polymorphic && !any_multi_inst {
        return validate_specialized_batch(parent, lambdas, pool, resolution.into_missing());
    }

    let original_count = lambdas.len();
    let multi_inst_lambdas =
        detect_and_clone_multi_inst(parent, lambdas, interner, &mut resolution);
    let (global_map, return_types, concrete_function_types) =
        build_single_inst_mappings(parent, lambdas, original_count, &multi_inst_lambdas, pool);

    // Parent closure construction/copy sites carry the same quantified
    // children as their targets. Resolve them from the exact global map so
    // callable-fact validation sees one shared residual signature identity.
    apply_bound_var_map(parent, &global_map, &mut resolution);

    for (&index, &concrete_function) in &concrete_function_types {
        apply_parent_partial_apply_type(parent, lambdas[index].name, concrete_function);
    }

    for (index, lambda) in lambdas.iter_mut().enumerate() {
        if index < original_count && multi_inst_lambdas.contains(&index) {
            continue;
        }
        apply_bound_var_map(lambda, &global_map, &mut resolution);
        if let Some(&(schema, concrete)) = return_types.get(&index) {
            resolve_lambda_return_types(lambda, schema, concrete);
        }
        if let Some(&concrete_function) = concrete_function_types.get(&index) {
            apply_concrete_param_types(lambda, concrete_function, &mut resolution);
        }
    }

    #[expect(
        clippy::needless_range_loop,
        reason = "index connects specialization maps to mutable lambda entries"
    )]
    for index in 0..original_count {
        if multi_inst_lambdas.contains(&index) || concrete_function_types.contains_key(&index) {
            continue;
        }
        let lambda = &lambdas[index];
        let has_unresolved_container_params = lambda.params.iter().any(|parameter| {
            !matches!(pool.tag(parameter.ty), Tag::BoundVar | Tag::Var)
                && contains_var(pool, parameter.ty)
        });
        if !has_unresolved_container_params {
            continue;
        }
        if let Some(partial_apply) = find_partial_apply_dst(parent, lambda.name) {
            if let Some((arguments, result)) =
                find_concrete_types_from_calls(parent, partial_apply, pool)
            {
                apply_call_site_types(&mut lambdas[index], &arguments, result, &mut resolution);
            }
        }
    }

    remove_multi_inst_originals(lambdas, multi_inst_lambdas);

    // Follow any links exposed by return/call-site resolution through concrete
    // identities the type phase already interned.
    let empty = rustc_hash::FxHashMap::default();
    resolve_type_sites(parent, &empty, &mut resolution);
    for lambda in lambdas.iter_mut() {
        resolve_type_sites(lambda, &empty, &mut resolution);
    }
    validate_specialized_batch(parent, lambdas, pool, resolution.into_missing())
}

/// Remove an uninstantiated closure template only when its construction has
/// no captures and every produced closure value is completely unused.
///
/// This is semantic dead-code elimination, not type defaulting: a captured
/// value could carry observable finalization timing, and any surviving value
/// use could select a later instantiation. Both cases remain in the batch and
/// therefore fail closed if no exact specialization evidence exists.
fn remove_dead_non_capturing_templates(
    parent: &mut crate::ArcFunction,
    lambdas: &mut Vec<crate::ArcFunction>,
    pool: &ori_types::Pool,
) {
    let mut removals = Vec::new();
    for (lambda_index, lambda) in lambdas.iter().enumerate() {
        if lambda.num_captures != 0 || !is_polymorphic_lambda(lambda, pool) {
            continue;
        }
        let Some(constructions) =
            unused_non_capturing_constructions(parent, lambdas, lambda_index, lambda.name)
        else {
            continue;
        };
        removals.push((lambda_index, constructions));
    }

    for (_, constructions) in &removals {
        remove_constructions(parent, constructions);
    }
    for (lambda_index, _) in removals.into_iter().rev() {
        lambdas.remove(lambda_index);
    }
}

fn unused_non_capturing_constructions(
    parent: &crate::ArcFunction,
    lambdas: &[crate::ArcFunction],
    lambda_index: usize,
    lambda_name: ori_ir::Name,
) -> Option<Vec<crate::ArcVarId>> {
    for (index, sibling) in lambdas.iter().enumerate() {
        if index != lambda_index && function_references_callable(sibling, lambda_name) {
            return None;
        }
    }
    if function_has_direct_call(parent, lambda_name) {
        return None;
    }

    let constructions: Vec<_> = parent
        .blocks
        .iter()
        .flat_map(|block| &block.body)
        .filter_map(|instruction| match instruction {
            crate::ArcInstr::PartialApply {
                dst, func, args, ..
            } if *func == lambda_name && args.is_empty() => Some(*dst),
            _ => None,
        })
        .collect();
    if constructions.is_empty() {
        return None;
    }

    let has_other_construction = parent.blocks.iter().any(|block| {
        block.body.iter().any(|instruction| {
            matches!(
                instruction,
                crate::ArcInstr::PartialApply { func, args, .. }
                    if *func == lambda_name && !args.is_empty()
            )
        })
    });
    if has_other_construction {
        return None;
    }

    let every_result_unused = constructions.iter().all(|&result| {
        !parent
            .params
            .iter()
            .any(|parameter| parameter.var == result)
            && parent.blocks.iter().all(|block| {
                !block.params.iter().any(|(variable, _)| *variable == result)
                    && block
                        .body
                        .iter()
                        .all(|instruction| !instruction.uses_var(result))
                    && !block.terminator.uses_var(result)
            })
    });
    every_result_unused.then_some(constructions)
}

fn function_references_callable(function: &crate::ArcFunction, target: ori_ir::Name) -> bool {
    function_has_direct_call(function, target)
        || function.blocks.iter().any(|block| {
            block.body.iter().any(|instruction| {
                matches!(instruction, crate::ArcInstr::PartialApply { func, .. } if *func == target)
            })
        })
}

fn function_has_direct_call(function: &crate::ArcFunction, target: ori_ir::Name) -> bool {
    function.blocks.iter().any(|block| {
        block.body.iter().any(|instruction| {
            matches!(instruction, crate::ArcInstr::Apply { func, .. } if *func == target)
        }) || matches!(
            &block.terminator,
            crate::ArcTerminator::Invoke { func, .. } if *func == target
        )
    })
}

fn remove_constructions(parent: &mut crate::ArcFunction, constructions: &[crate::ArcVarId]) {
    for block_index in 0..parent.blocks.len() {
        let mut instruction_index = 0;
        while instruction_index < parent.blocks[block_index].body.len() {
            let remove = matches!(
                &parent.blocks[block_index].body[instruction_index],
                crate::ArcInstr::PartialApply { dst, .. } if constructions.contains(dst)
            );
            if remove {
                parent.blocks[block_index].body.remove(instruction_index);
                if let Some(spans) = parent.spans.get_mut(block_index) {
                    if instruction_index < spans.len() {
                        spans.remove(instruction_index);
                    }
                }
            } else {
                instruction_index += 1;
            }
        }
    }
    for &result in constructions {
        parent.var_types[result.index()] = ori_types::Idx::NEVER;
    }
}

fn validate_specialized_batch(
    parent: &crate::ArcFunction,
    lambdas: &[crate::ArcFunction],
    pool: &ori_types::Pool,
    missing_materializations: Vec<MissingTypeMaterialization>,
) -> Result<(), LambdaSpecializationError> {
    let mut unresolved = Vec::new();
    for function in std::iter::once(parent).chain(lambdas.iter()) {
        if let Err(error) = crate::assert_no_unresolved_bound_vars(pool, function) {
            unresolved.push(error);
        }
    }
    if unresolved.is_empty() && missing_materializations.is_empty() {
        Ok(())
    } else {
        Err(LambdaSpecializationError {
            unresolved,
            missing_materializations,
        })
    }
}

/// Return whether a type contains a bound variable at any nesting depth.
#[must_use]
pub fn type_contains_bound_var(pool: &ori_types::Pool, ty: ori_types::Idx) -> bool {
    contains_bound_var(pool, ty)
}

/// Return the first nested bound variable in a type, if one survives.
#[must_use]
pub fn first_unresolved_bound_var(
    pool: &ori_types::Pool,
    ty: ori_types::Idx,
) -> Option<ori_types::Idx> {
    first_bound_var(pool, ty)
}
