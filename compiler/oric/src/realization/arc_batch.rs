//! Ordered preparation of grouped ARC functions before executable realization.

use ori_arc::ArcFunction;
use ori_ir::{Name, StringInterner};
use ori_repr::executable::FunctionFamilyTopology;
use ori_types::{Idx, MethodProducer, Pool};
use rustc_hash::FxHashMap;

/// Realized concrete-receiver method dispatch targets, keyed by receiver type and method name.
pub(crate) type MethodTargetMap = FxHashMap<(Idx, Name), Name>;

/// Executable bodies plus their exact names-only family topology and method
/// dispatch targets, consumed after a [`PreparedArcBatch`] is closed.
pub(crate) type ExecutableParts = (
    Vec<ArcFunction>,
    Vec<FunctionFamilyTopology>,
    MethodTargetMap,
);

/// One lowered callable body and every lambda body it owns.
#[derive(Clone)]
pub(crate) struct ArcFunctionGroup {
    parent: ArcFunction,
    lambdas: Vec<ArcFunction>,
}

impl ArcFunctionGroup {
    /// Preserve one lowering result as an indivisible parent/lambda family.
    #[must_use]
    pub(crate) fn new(parent: ArcFunction, lambdas: Vec<ArcFunction>) -> Self {
        Self { parent, lambdas }
    }

    /// Recover the lowering pair while assembling a heterogeneous caller-owned
    /// inventory. This does not flatten the parent/lambda family.
    #[must_use]
    pub(crate) fn into_parts(self) -> (ArcFunction, Vec<ArcFunction>) {
        (self.parent, self.lambdas)
    }

    /// Iterate the parent and its owned lambdas without flattening ownership.
    pub(crate) fn bodies(&self) -> impl Iterator<Item = &ArcFunction> {
        std::iter::once(&self.parent).chain(&self.lambdas)
    }

    /// Returns the stable identity used to key this lowering family.
    pub(crate) fn parent_name(&self) -> Name {
        self.parent.name
    }
}

impl From<(ArcFunction, Vec<ArcFunction>)> for ArcFunctionGroup {
    fn from((parent, lambdas): (ArcFunction, Vec<ArcFunction>)) -> Self {
        Self::new(parent, lambdas)
    }
}

/// Grouped ARC bodies that have not crossed the shared preparation seam.
pub(crate) struct LoweredArcBatch {
    groups: Vec<ArcFunctionGroup>,
}

/// Prepared ARC bodies whose target identities, lambda types, and family
/// topology are closed for executable realization.
pub(crate) struct PreparedArcBatch {
    functions: Vec<ArcFunction>,
    function_families: Vec<FunctionFamilyTopology>,
    parent_order: Vec<Name>,
    method_targets: MethodTargetMap,
}

/// Failure while closing a lowered ARC batch for every executable backend.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ArcBatchPreparationError {
    /// Two lowering sources claimed the same parent callable identity.
    #[error(
        "ARC batch contains duplicate parent callable `{parent}` because multiple lowering sources claimed one executable body. Run with `ORI_LOG=oric::realization::arc_batch=debug` and report this compiler error"
    )]
    DuplicateParent { parent: String },
    /// One body identity appeared in more than one family position.
    #[error(
        "ARC batch body `{body}` appears under both `{first_parent}` and `{second_parent}`; every executable body must belong to exactly one family. Run with `ORI_LOG=oric::realization::arc_batch=debug` and report this compiler error"
    )]
    DuplicateBody {
        body: String,
        first_parent: String,
        second_parent: String,
    },
    /// Shared specialization left one or more lambda groups unresolved.
    #[error(
        "ARC batch lambda specialization failed for {count} parent/lambda group(s): {errors:?}. This is an internal compiler error; report this complete message"
    )]
    LambdaSpecialization {
        count: usize,
        errors: Vec<ori_arc::LambdaSpecializationError>,
    },
    /// Typed operator dispatch could not select an exact target after specialization.
    #[error(
        "ARC batch operator-call resolution failed at {count} site(s): {errors:?}. This is an internal compiler error; report this complete message"
    )]
    OperatorCallResolution {
        count: usize,
        errors: Vec<ori_arc::OperatorCallResolutionError>,
    },
    /// A canonical source-method handle did not resolve against the matching
    /// typed-module producer table.
    #[error(
        "ARC batch selected-method producer resolution failed at {count} site(s): {errors:?}. This is an internal compiler error; report this complete message"
    )]
    SelectedMethodProducerResolution { count: usize, errors: Vec<String> },
}

impl LoweredArcBatch {
    /// Construct an empty lowered batch whose insertions are duplicate-checked.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self { groups: Vec::new() }
    }

    /// Wrap independently lowered families without losing duplicate evidence.
    #[must_use = "success or failure must be handled"]
    pub(crate) fn try_from_groups(
        groups: impl IntoIterator<Item = ArcFunctionGroup>,
        interner: &StringInterner,
    ) -> Result<Self, ArcBatchPreparationError> {
        let mut batch = Self::new();
        for group in groups {
            batch.insert(group, interner)?;
        }
        Ok(batch)
    }

    /// Add one independently lowered parent/lambda group without overwriting.
    #[must_use = "success or failure must be handled"]
    pub(crate) fn insert(
        &mut self,
        group: ArcFunctionGroup,
        interner: &StringInterner,
    ) -> Result<(), ArcBatchPreparationError> {
        let parent = group.parent_name();
        if self
            .groups
            .iter()
            .any(|existing| existing.parent_name() == parent)
        {
            tracing::debug!(
                target: "oric::realization::arc_batch",
                callable = interner.lookup(parent),
                existing_groups = self.groups.len(),
                "rejected duplicate ARC parent before batch mutation"
            );
            return Err(ArcBatchPreparationError::DuplicateParent {
                parent: interner.lookup(parent).to_owned(),
            });
        }
        self.groups.push(group);
        Ok(())
    }

    /// Close every pre-AIMS type and target identity in one deterministic order.
    ///
    /// This is the sole compiler-driver ordering seam for all physical
    /// executors: mono target rewrite, grouped lambda specialization, typed
    /// operator resolution, and exact impl-target rewrite. Flattening is only
    /// exposed by the returned prepared state.
    #[must_use = "success or failure must be handled"]
    pub(crate) fn prepare(
        mut self,
        mono_functions: &[ori_repr::monomorphize::MonoFunction],
        impl_targets: &MethodTargetMap,
        impl_producer_targets: &FxHashMap<MethodProducer, Name>,
        method_producers: &[MethodProducer],
        pool: &Pool,
        interner: &StringInterner,
    ) -> Result<PreparedArcBatch, ArcBatchPreparationError> {
        self.groups.sort_by(|left, right| {
            interner
                .lookup(left.parent_name())
                .cmp(interner.lookup(right.parent_name()))
                .then_with(|| left.parent_name().raw().cmp(&right.parent_name().raw()))
        });
        validate_family_identities(&self.groups, interner)?;

        resolve_selected_method_producers(&mut self.groups, method_producers)?;
        specialize_lambdas(&mut self.groups, pool, interner)?;
        validate_family_identities(&self.groups, interner)?;

        // Signature fallback requires the exact concrete types produced by
        // lambda specialization, so target rewriting consumes specialized bodies.
        if !mono_functions.is_empty() {
            let maps = ori_repr::monomorphize::MonoTargetMaps::new(mono_functions, pool);
            for group in &mut self.groups {
                maps.rewrite_function(&mut group.parent, &mut group.lambdas, pool, interner);
            }
        }

        rewrite_operator_calls(&mut self.groups, impl_targets, pool, interner)?;

        for group in &mut self.groups {
            super::program::rewrite_impl_targets(
                std::slice::from_mut(&mut group.parent),
                impl_targets,
                impl_producer_targets,
                pool,
            );
            super::program::rewrite_impl_targets(
                &mut group.lambdas,
                impl_targets,
                impl_producer_targets,
                pool,
            );
        }

        let mut functions = Vec::new();
        let mut function_families = Vec::with_capacity(self.groups.len());
        let mut parent_order = Vec::with_capacity(self.groups.len());
        for group in self.groups {
            let parent = group.parent.name;
            let lambdas = group.lambdas.iter().map(|lambda| lambda.name).collect();
            parent_order.push(parent);
            function_families.push(FunctionFamilyTopology::new(parent, lambdas));
            functions.push(group.parent);
            functions.extend(group.lambdas);
        }

        Ok(PreparedArcBatch {
            functions,
            function_families,
            parent_order,
            method_targets: impl_targets.clone(),
        })
    }
}

fn specialize_lambdas(
    groups: &mut [ArcFunctionGroup],
    pool: &Pool,
    interner: &StringInterner,
) -> Result<(), ArcBatchPreparationError> {
    let mut errors = Vec::new();
    for group in groups {
        if let Err(error) = ori_arc::specialize_polymorphic_lambdas(
            &mut group.parent,
            &mut group.lambdas,
            pool,
            interner,
        ) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ArcBatchPreparationError::LambdaSpecialization {
            count: errors.len(),
            errors,
        })
    }
}

fn rewrite_operator_calls(
    groups: &mut [ArcFunctionGroup],
    impl_targets: &MethodTargetMap,
    pool: &Pool,
    interner: &StringInterner,
) -> Result<(), ArcBatchPreparationError> {
    let resolve_target = |receiver, method| {
        impl_targets
            .get(&(super::method_receiver_key(pool, receiver), method))
            .copied()
    };
    let mut errors = Vec::new();
    for group in groups {
        if let Err(mut group_errors) = ori_arc::rewrite_operator_trait_calls(
            std::slice::from_mut(&mut group.parent),
            pool,
            interner,
            &resolve_target,
        ) {
            errors.append(&mut group_errors);
        }
        if let Err(mut group_errors) = ori_arc::rewrite_operator_trait_calls(
            &mut group.lambdas,
            pool,
            interner,
            &resolve_target,
        ) {
            errors.append(&mut group_errors);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ArcBatchPreparationError::OperatorCallResolution {
            count: errors.len(),
            errors,
        })
    }
}

fn resolve_selected_method_producers(
    groups: &mut [ArcFunctionGroup],
    method_producers: &[MethodProducer],
) -> Result<(), ArcBatchPreparationError> {
    let mut errors = Vec::new();
    for function in groups
        .iter_mut()
        .flat_map(|group| std::iter::once(&mut group.parent).chain(group.lambdas.iter_mut()))
    {
        for fact in &mut function.method_call_facts {
            if let Err(error) =
                resolve_selected_method_producer(function.name, fact, method_producers)
            {
                errors.push(error);
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(ArcBatchPreparationError::SelectedMethodProducerResolution {
            count: errors.len(),
            errors,
        })
    }
}

fn resolve_selected_method_producer(
    function: Name,
    fact: &mut ori_arc::MethodCallFact,
    method_producers: &[MethodProducer],
) -> Result<(), String> {
    let Some(selected) = fact.selected_producer else {
        return Ok(());
    };
    let producer = method_producers.get(selected.index()).ok_or_else(|| {
        format!(
            "function {:?} call result {:?} references producer id {} outside the {}-entry table",
            function,
            fact.destination,
            selected.raw(),
            method_producers.len(),
        )
    })?;
    match &fact.producer {
        Some(existing) if existing != producer => Err(format!(
            "function {:?} call result {:?} carries conflicting selected producers {:?} and {:?}",
            function, fact.destination, existing, producer,
        )),
        Some(_) => Ok(()),
        None => {
            fact.producer = Some(producer.clone());
            Ok(())
        }
    }
}

impl PreparedArcBatch {
    /// Borrow the deterministic body inventory.
    #[must_use]
    pub(crate) fn functions(&self) -> &[ArcFunction] {
        &self.functions
    }

    /// Return deterministic parent roots without admitting owned lambdas.
    #[must_use]
    pub(crate) fn parent_roots(&self) -> Vec<Name> {
        self.parent_order.clone()
    }

    /// Consume the type-state seam into executable bodies plus their exact
    /// names-only family topology. No grouped body authority survives.
    pub(crate) fn into_executable_parts(self) -> ExecutableParts {
        (self.functions, self.function_families, self.method_targets)
    }
}

fn validate_family_identities(
    groups: &[ArcFunctionGroup],
    interner: &StringInterner,
) -> Result<(), ArcBatchPreparationError> {
    let mut owner_by_body = FxHashMap::default();
    for group in groups {
        let parent = group.parent.name;
        for body in std::iter::once(&group.parent).chain(&group.lambdas) {
            if let Some(first_parent) = owner_by_body.insert(body.name, parent) {
                return Err(ArcBatchPreparationError::DuplicateBody {
                    body: interner.lookup(body.name).to_owned(),
                    first_parent: interner.lookup(first_parent).to_owned(),
                    second_parent: interner.lookup(parent).to_owned(),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
