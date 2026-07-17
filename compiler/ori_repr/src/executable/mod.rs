//! Validated, closed program shared by executable backends.

mod callable_facts;
mod closure_adapters;
mod drop_plan;
mod effect_facts;
mod error;
mod external;
mod function_contracts;
mod function_families;
mod parameter_facts;
mod return_facts;
mod runtime;
mod validation;

use ori_arc::{
    ArcFunction, ArcInstr, ArcTerminator, ClosureAdapterPlan, CtorKind, EffectSummary,
    FreshSelfAllocationFacts, FunctionCallableFacts, FunctionEffectFacts, MemoryContract,
    ParamDisjointnessFacts, RetainPlanId, RetainPlanNode, RetainPlanTable,
};
use ori_ir::{Name, SharedInterner, StringInterner};
use ori_types::{Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::ReprPlan;

pub use drop_plan::{ExecutableDropPlan, ExecutableUserDrop, UserDropBinding};
pub use error::RealizationError;
pub use external::{
    validate_external_callables, ExternalCallable, ExternalCallableMetadata,
    ExternalFactIdentities, ExternalFunctionId, ExternalUnwind,
};
pub use function_families::FunctionFamilyTopology;
pub use runtime::{CompilerOperation, FormatRuntime, IteratorSource, RuntimeCall};

/// Schema version for the in-memory executable-program contract.
pub const EXECUTABLE_PROGRAM_VERSION: u32 = 9;

/// Stable index of a realized function in an [`ExecutableProgram`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionId(u32);

impl FunctionId {
    /// Return the function's zero-based index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    fn from_index(index: usize) -> Result<Self, RealizationError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| RealizationError::TooManyFunctions { count: index })
    }
}

/// Stable index of an ARC basic block within a function.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BlockIndex(u32);

impl BlockIndex {
    /// Construct a checked block index.
    pub fn new(index: usize, function: Name) -> Result<Self, RealizationError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| RealizationError::TooManyBlocks { function })
    }

    /// Return the zero-based block index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Position of a direct callable reference within an ARC block.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallPosition {
    /// An instruction in the block body.
    Instruction(u32),
    /// The block terminator.
    Terminator,
}

impl CallPosition {
    /// Construct a checked instruction position.
    pub fn instruction(index: usize, function: Name) -> Result<Self, RealizationError> {
        u32::try_from(index)
            .map(Self::Instruction)
            .map_err(|_| RealizationError::TooManyInstructions { function })
    }
}

/// Stable location of a direct call or closure target in the realized ARC program.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CallSite {
    function: FunctionId,
    block: BlockIndex,
    position: CallPosition,
}

impl CallSite {
    /// Construct a call-site identity from checked component indices.
    #[must_use]
    pub const fn new(function: FunctionId, block: BlockIndex, position: CallPosition) -> Self {
        Self {
            function,
            block,
            position,
        }
    }
}

/// Fully resolved destination of a direct callable reference.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableTarget {
    /// A realized ARC function body.
    Function(FunctionId),
    /// A backend-neutral runtime operation.
    Runtime(RuntimeCall),
    /// A typed callable supplied by another compiled unit.
    External(ExternalFunctionId),
}

/// Inputs to closed-program validation.
pub struct ExecutableProgramParts {
    /// Artifact schema version supplied by the realization owner.
    pub version: u32,
    /// Immutable shared symbol storage retained without a compiler database.
    pub symbols: SharedInterner,
    /// Type pool used by all realized functions and representation facts.
    pub pool: Pool,
    /// Every post-AIMS function body reachable from the module entry points.
    pub functions: Vec<ArcFunction>,
    /// Exact parent/lambda grouping for every realized function body.
    pub function_families: Vec<FunctionFamilyTopology>,
    /// Final AIMS contracts returned by the same whole-program realization.
    pub contracts: FxHashMap<Name, MemoryContract>,
    /// Final RL-30 facts returned by the same whole-program realization.
    pub function_effects: FxHashMap<Name, FunctionEffectFacts>,
    /// Final RL-29 facts returned by the same whole-program realization.
    pub fresh_return_facts: FxHashMap<Name, FreshSelfAllocationFacts>,
    /// Final RL-31 facts returned by the same whole-program realization.
    pub param_disjointness: FxHashMap<Name, ParamDisjointnessFacts>,
    /// Exact function-value signatures returned by the same whole-program
    /// realization, parallel to every function's SSA registers.
    pub callable_facts: FxHashMap<Name, FunctionCallableFacts>,
    /// Exact closure-call adapter plans returned by the same whole-program
    /// realization.
    pub closure_adapters: FxHashMap<Name, ClosureAdapterPlan>,
    /// Closed logical retain graph shared by every executable ownership
    /// operation, including closure adapters.
    pub retain_plans: RetainPlanTable,
    /// Explicit nonempty externally reachable function set.
    pub roots: Vec<Name>,
    /// Distinguished command-line entry, when this artifact is executable as
    /// a standalone process.
    pub cli_entry: Option<Name>,
    /// Producer-authored facts for callable bodies linked from other units.
    pub externals: Vec<ExternalCallable>,
    /// Exact logical-to-callable bindings for every user-defined drop burden.
    pub user_drop_bindings: Vec<UserDropBinding>,
    /// Transitional compiled-shaped representation plan. Production artifact
    /// closure carries neutral evidence plus separate validated VM and
    /// compiled plans; this field is not an AIMS fact or universal layout.
    pub repr_plan: ReprPlan,
    /// Closed type and burden metadata used by runtime lowering.
    pub type_registry: TypeRegistry,
}

/// Immutable post-AIMS artifact consumed by executable backend projections.
pub struct ExecutableProgram {
    version: u32,
    symbols: SharedInterner,
    pool: Pool,
    functions: Box<[ArcFunction]>,
    function_ids: FxHashMap<Name, FunctionId>,
    function_family_lambdas: FxHashMap<FunctionId, Box<[FunctionId]>>,
    function_contracts: Box<[MemoryContract]>,
    function_effects: Box<[FunctionEffectFacts]>,
    fresh_return_facts: Box<[FreshSelfAllocationFacts]>,
    param_disjointness: Box<[ParamDisjointnessFacts]>,
    callable_facts: Box<[FunctionCallableFacts]>,
    closure_adapters: Box<[Option<ClosureAdapterPlan>]>,
    retain_plans: RetainPlanTable,
    call_targets: FxHashMap<CallSite, CallableTarget>,
    direct_call_targets: FxHashMap<(FunctionId, ori_arc::ArcVarId), CallableTarget>,
    roots: Box<[FunctionId]>,
    cli_entry: Option<FunctionId>,
    external_functions: Box<[ExternalCallable]>,
    external_ids: FxHashMap<Name, ExternalFunctionId>,
    user_drop_plan: ExecutableDropPlan,
    repr_plan: ReprPlan,
    type_registry: TypeRegistry,
}

impl ExecutableProgram {
    /// Validate and close every direct callable reference in a program.
    pub fn validate(mut parts: ExecutableProgramParts) -> Result<Self, RealizationError> {
        validate_version(parts.version)?;
        validate_function_symbols(&parts.functions, &parts.symbols)?;
        validation::validate_function_metadata(&parts.functions, &parts.pool, &parts.symbols)?;
        parts.functions.sort_by(|left, right| {
            parts
                .symbols
                .lookup(left.name)
                .cmp(parts.symbols.lookup(right.name))
                .then_with(|| left.name.raw().cmp(&right.name.raw()))
        });
        let function_ids = build_function_ids(&parts.functions)?;
        freeze_executable_program(parts, function_ids)
    }

    /// Return the executable schema version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Return immutable symbol lookup without exposing interning mutation.
    #[must_use]
    pub fn symbols(&self) -> &dyn ori_ir::StringLookup {
        &*self.symbols
    }

    /// Return the shared type pool.
    #[must_use]
    pub const fn pool(&self) -> &Pool {
        &self.pool
    }

    /// Return all realized function bodies in stable order.
    #[must_use]
    pub fn functions(&self) -> &[ArcFunction] {
        &self.functions
    }

    /// Return one realized body by its stable executable identity.
    #[must_use]
    pub fn function(&self, function: FunctionId) -> &ArcFunction {
        &self.functions[function.index()]
    }

    /// Return the ordered lambda identities owned by a parent function.
    ///
    /// Returns `None` when `function` identifies a lambda rather than a
    /// parent. Every validated parent is present, including empty families.
    #[must_use]
    pub fn function_family_lambdas(&self, function: FunctionId) -> Option<&[FunctionId]> {
        self.function_family_lambdas
            .get(&function)
            .map(AsRef::as_ref)
    }

    /// Return every externally reachable root in stable caller-supplied order.
    #[must_use]
    pub fn roots(&self) -> &[FunctionId] {
        &self.roots
    }

    /// Return the distinguished command-line entry when one exists.
    #[must_use]
    pub const fn cli_entry(&self) -> Option<FunctionId> {
        self.cli_entry
    }

    /// Resolve a function name to its stable executable index.
    #[must_use]
    pub fn function_id(&self, name: Name) -> Option<FunctionId> {
        self.function_ids.get(&name).copied()
    }

    /// Return the final backend-neutral AIMS contract for a stable function.
    #[must_use]
    pub fn function_contract(&self, function: FunctionId) -> &MemoryContract {
        &self.function_contracts[function.index()]
    }

    /// Freeze the cross-module metadata for one realized callable export.
    ///
    /// The signature, contract, and unwind classification all come from this
    /// already-validated post-AIMS artifact. Frontend declarations cannot
    /// manufacture this carrier before whole-program realization closes.
    #[must_use]
    pub fn export_callable_metadata(
        &self,
        function: FunctionId,
        link_symbol: &str,
    ) -> ExternalCallableMetadata {
        let body = &self.functions[function.index()];
        let parameter_types: Vec<_> = body.params.iter().map(|parameter| parameter.ty).collect();
        let contract = self.function_contract(function).clone();
        let unwind = ExternalUnwind::from_effects(self.effect_summary(function));
        ExternalCallableMetadata::freeze(
            link_symbol,
            &parameter_types,
            body.return_type,
            contract,
            unwind,
            &self.pool,
        )
    }

    /// Return the frozen backend-neutral RL-30 facts for a stable function.
    #[must_use]
    pub fn function_effects(&self, function: FunctionId) -> FunctionEffectFacts {
        self.function_effects[function.index()]
    }

    /// Return the frozen backend-neutral RL-29 facts for a stable function.
    #[must_use]
    pub fn fresh_return_facts(&self, function: FunctionId) -> FreshSelfAllocationFacts {
        self.fresh_return_facts[function.index()]
    }

    /// Return the final function-level effect facts for a stable function.
    #[must_use]
    pub fn effect_summary(&self, function: FunctionId) -> EffectSummary {
        self.function_effects(function).effects()
    }

    /// Return the frozen backend-neutral RL-31 facts for a stable function.
    #[must_use]
    pub fn param_disjointness(&self, function: FunctionId) -> &ParamDisjointnessFacts {
        &self.param_disjointness[function.index()]
    }

    /// Return exact register-parallel callable signatures for a function.
    #[must_use]
    pub fn callable_facts(&self, function: FunctionId) -> &FunctionCallableFacts {
        &self.callable_facts[function.index()]
    }

    /// Return the shared adapter plan when this function is used as a closure
    /// target.
    #[must_use]
    pub fn closure_adapter(&self, function: FunctionId) -> Option<&ClosureAdapterPlan> {
        self.closure_adapters[function.index()].as_ref()
    }

    /// Resolve one backend-neutral retain topology by its stable identity.
    #[must_use]
    pub fn retain_plan(&self, plan: RetainPlanId) -> Option<&RetainPlanNode> {
        self.retain_plans.get(plan)
    }

    /// Return the complete closed logical retain-plan graph.
    #[must_use]
    pub const fn retain_plans(&self) -> &RetainPlanTable {
        &self.retain_plans
    }

    /// Return the pre-resolved target at a direct callable reference.
    #[must_use]
    pub fn call_target(&self, site: CallSite) -> Option<CallableTarget> {
        self.call_targets.get(&site).copied()
    }

    /// Return the frozen target of one direct call by its stable SSA result.
    ///
    /// Physical projections use this key when their emission API carries a
    /// function identity and destination register rather than source block
    /// coordinates. The mapping is derived from the same validated call-site
    /// table and cannot disagree with [`call_target`](Self::call_target).
    #[must_use]
    pub fn direct_call_target(
        &self,
        function: FunctionId,
        destination: ori_arc::ArcVarId,
    ) -> Option<CallableTarget> {
        self.direct_call_targets
            .get(&(function, destination))
            .copied()
    }

    /// Return a frozen external callable by stable artifact identity.
    #[must_use]
    pub fn external_function(&self, function: ExternalFunctionId) -> &ExternalCallable {
        &self.external_functions[function.index()]
    }

    /// Return every external callable in stable [`ExternalFunctionId`] order.
    #[must_use]
    pub fn external_functions(&self) -> &[ExternalCallable] {
        &self.external_functions
    }

    /// Resolve an import-local external name to its stable identity.
    #[must_use]
    pub fn external_function_id(&self, name: Name) -> Option<ExternalFunctionId> {
        self.external_ids.get(&name).copied()
    }

    /// Return the complete stable user-drop plan.
    #[must_use]
    pub const fn user_drop_plan(&self) -> &ExecutableDropPlan {
        &self.user_drop_plan
    }

    /// Resolve the exact user-drop operation for an artifact-local type.
    #[must_use]
    pub fn user_drop_for_type(&self, ty: ori_types::Idx) -> Option<&ExecutableUserDrop> {
        self.user_drop_plan.get(ty, &self.pool)
    }

    /// Return the transitional compiled-shaped representation plan.
    /// Physical backends must not treat it as the production universal layout.
    #[must_use]
    pub const fn repr_plan(&self) -> &ReprPlan {
        &self.repr_plan
    }

    /// Return closed type and burden metadata.
    #[must_use]
    pub const fn type_registry(&self) -> &TypeRegistry {
        &self.type_registry
    }
}

fn freeze_executable_program(
    mut parts: ExecutableProgramParts,
    function_ids: FxHashMap<Name, FunctionId>,
) -> Result<ExecutableProgram, RealizationError> {
    let FrozenFunctionMetadata {
        function_families,
        user_drop_plan,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
    } = freeze_function_metadata(&mut parts, &function_ids)?;
    let roots = freeze_roots(
        &parts.roots,
        parts.cli_entry,
        &function_ids,
        &function_families,
    )?;
    let cli_entry = parts
        .cli_entry
        .map(|name| {
            function_ids
                .get(&name)
                .copied()
                .ok_or(RealizationError::MissingEntryPoint { name })
        })
        .transpose()?;
    let (external_functions, external_ids) =
        external::freeze_external_callables(parts.externals, &parts.pool)?;
    validate_external_symbols(&external_functions, &parts.symbols, &function_ids)?;
    let ResolvedCallTargets {
        call_targets,
        direct_call_targets,
    } = build_call_targets(
        &parts.functions,
        &function_ids,
        &external_functions,
        &external_ids,
        &parts.symbols,
        &parts.pool,
    )?;
    Ok(ExecutableProgram {
        version: parts.version,
        symbols: parts.symbols,
        pool: parts.pool,
        functions: parts.functions.into_boxed_slice(),
        function_ids,
        function_family_lambdas: function_families.lambdas_by_parent,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
        call_targets,
        direct_call_targets,
        roots,
        cli_entry,
        external_functions,
        external_ids,
        user_drop_plan,
        repr_plan: parts.repr_plan,
        type_registry: parts.type_registry,
    })
}

struct FrozenFunctionMetadata {
    function_families: function_families::FrozenFunctionFamilies,
    user_drop_plan: ExecutableDropPlan,
    function_contracts: Box<[MemoryContract]>,
    function_effects: Box<[FunctionEffectFacts]>,
    fresh_return_facts: Box<[FreshSelfAllocationFacts]>,
    param_disjointness: Box<[ParamDisjointnessFacts]>,
    callable_facts: Box<[FunctionCallableFacts]>,
    closure_adapters: Box<[Option<ClosureAdapterPlan>]>,
    retain_plans: RetainPlanTable,
}

fn freeze_function_metadata(
    parts: &mut ExecutableProgramParts,
    function_ids: &FxHashMap<Name, FunctionId>,
) -> Result<FrozenFunctionMetadata, RealizationError> {
    let function_families = function_families::freeze_function_families(
        std::mem::take(&mut parts.function_families),
        &parts.functions,
        function_ids,
    )?;
    let user_drop_plan = drop_plan::freeze_user_drop_plan(
        std::mem::take(&mut parts.user_drop_bindings),
        &parts.type_registry,
        &parts.pool,
        &parts.functions,
        function_ids,
    )?;
    let function_contracts = function_contracts::freeze_function_contracts(
        &parts.functions,
        std::mem::take(&mut parts.contracts),
        &parts.symbols,
    )?;
    let function_effects = effect_facts::freeze_function_effects(
        &parts.functions,
        &function_contracts,
        std::mem::take(&mut parts.function_effects),
        &parts.symbols,
    )?;
    let fresh_return_facts = return_facts::freeze_fresh_return_facts(
        &parts.functions,
        &function_contracts,
        std::mem::take(&mut parts.fresh_return_facts),
        &parts.symbols,
    )?;
    let param_disjointness = parameter_facts::freeze_param_disjointness(
        &parts.functions,
        std::mem::take(&mut parts.param_disjointness),
        &parts.symbols,
    )?;
    let callable_facts = callable_facts::freeze_callable_facts(
        &parts.functions,
        std::mem::take(&mut parts.callable_facts),
        &parts.symbols,
    )?;
    let (closure_adapters, retain_plans) = closure_adapters::freeze_closure_adapters(
        &parts.functions,
        &function_contracts,
        &parts.closure_adapters,
        std::mem::take(&mut parts.retain_plans),
        &parts.symbols,
    )?;
    Ok(FrozenFunctionMetadata {
        function_families,
        user_drop_plan,
        function_contracts,
        function_effects,
        fresh_return_facts,
        param_disjointness,
        callable_facts,
        closure_adapters,
        retain_plans,
    })
}

fn validate_version(version: u32) -> Result<(), RealizationError> {
    if version == EXECUTABLE_PROGRAM_VERSION {
        Ok(())
    } else {
        Err(RealizationError::UnsupportedVersion {
            found: version,
            expected: EXECUTABLE_PROGRAM_VERSION,
        })
    }
}

fn validate_function_symbols(
    functions: &[ArcFunction],
    symbols: &StringInterner,
) -> Result<(), RealizationError> {
    for function in functions {
        if symbols.try_lookup(function.name).is_none() {
            return Err(RealizationError::UnknownFunctionName {
                name: function.name,
            });
        }
    }
    Ok(())
}

fn build_function_ids(
    functions: &[ArcFunction],
) -> Result<FxHashMap<Name, FunctionId>, RealizationError> {
    let mut ids = FxHashMap::default();
    for (index, function) in functions.iter().enumerate() {
        let id = FunctionId::from_index(index)?;
        if ids.insert(function.name, id).is_some() {
            return Err(RealizationError::DuplicateFunction {
                name: function.name,
            });
        }
    }
    Ok(ids)
}

fn freeze_roots(
    roots: &[Name],
    cli_entry: Option<Name>,
    function_ids: &FxHashMap<Name, FunctionId>,
    function_families: &function_families::FrozenFunctionFamilies,
) -> Result<Box<[FunctionId]>, RealizationError> {
    if roots.is_empty() {
        if function_ids.is_empty() && cli_entry.is_none() {
            return Ok(Box::new([]));
        }
        return Err(RealizationError::MissingProgramRoots);
    }
    let mut seen = rustc_hash::FxHashSet::default();
    let mut frozen = Vec::with_capacity(roots.len());
    for &name in roots {
        if !seen.insert(name) {
            return Err(RealizationError::DuplicateProgramRoot { name });
        }
        let function = function_ids
            .get(&name)
            .copied()
            .ok_or(RealizationError::MissingProgramRoot { name })?;
        if function_families.is_lambda(function) {
            return Err(RealizationError::ProgramRootIsLambda { name });
        }
        frozen.push(function);
    }
    if let Some(entry) = cli_entry {
        if !seen.contains(&entry) {
            return Err(RealizationError::CliEntryNotRoot { name: entry });
        }
    }
    Ok(frozen.into_boxed_slice())
}

fn validate_external_symbols(
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

struct ResolvedCallTargets {
    call_targets: FxHashMap<CallSite, CallableTarget>,
    direct_call_targets: FxHashMap<(FunctionId, ori_arc::ArcVarId), CallableTarget>,
}

fn build_call_targets(
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
    if let Some(destination) = destination {
        if let Some(fact) = caller.direct_call_fact(destination) {
            if let ori_types::MethodProducer::Prelude(identity) = fact.producer {
                let runtime = identity
                    .resolve()
                    .map(RuntimeCall::RegistryPrelude)
                    .ok_or_else(|| RealizationError::MissingCallable {
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
                    })?;
                return Ok(CallableTarget::Runtime(runtime));
            }
        }
        if let Some(fact) = caller.method_call_fact(destination) {
            if let Some(ori_types::MethodProducer::Registry(identity)) = fact.producer {
                let runtime = identity
                    .resolve()
                    .map(RuntimeCall::RegistryMethod)
                    .ok_or_else(|| RealizationError::MissingCallable {
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
                    })?;
                return Ok(CallableTarget::Runtime(runtime));
            }
        }
    }
    if let Some(&function) = function_ids.get(&callee) {
        return Ok(CallableTarget::Function(function));
    }
    if let Some(&external) = external_ids.get(&callee) {
        return Ok(CallableTarget::External(external));
    }
    let receiver = destination
        .and_then(|destination| caller.method_call_fact(destination))
        .and_then(|fact| pool.builtin_type_tag(pool.resolve_fully(fact.receiver_type)));
    let runtime = symbols
        .try_lookup(callee)
        .and_then(|symbol| RuntimeCall::resolve(symbol, receiver))
        .ok_or_else(|| RealizationError::MissingCallable {
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
        })?;
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

#[cfg(test)]
mod tests;
