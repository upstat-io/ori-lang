//! Validated, closed program shared by executable backends.

mod call_targets;
mod callable_facts;
mod closure_adapters;
mod drop_plan;
mod effect_facts;
mod error;
mod external;
mod external_identities;
mod function_contracts;
mod function_families;
mod method_targets;
mod parameter_facts;
mod program_freeze;
mod return_facts;
mod runtime;
mod validation;

use ori_arc::{
    ArcFunction, ClosureAdapterPlan, EffectSummary, FreshSelfAllocationFacts,
    FunctionCallableFacts, FunctionEffectFacts, MemoryContract, ParamDisjointnessFacts,
    RetainPlanId, RetainPlanNode, RetainPlanTable,
};
use ori_ir::{Name, SharedInterner};
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
pub const EXECUTABLE_PROGRAM_VERSION: u32 = 10;

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
    /// Exact semantic receiver/method targets selected before ARC preparation.
    pub method_targets: FxHashMap<(ori_types::Idx, Name), Name>,
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
    method_targets: FxHashMap<(ori_types::Idx, Name), CallableTarget>,
    user_drop_plan: ExecutableDropPlan,
    repr_plan: ReprPlan,
    type_registry: TypeRegistry,
}

impl ExecutableProgram {
    /// Validate and close every direct callable reference in a program.
    pub fn validate(parts: ExecutableProgramParts) -> Result<Self, RealizationError> {
        program_freeze::validate(parts)
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

    /// Resolve the exact callable selected for one semantic receiver/method pair.
    #[must_use]
    pub fn method_target(&self, receiver: ori_types::Idx, method: Name) -> Option<CallableTarget> {
        self.method_targets.get(&(receiver, method)).copied()
    }

    /// Iterate every frozen semantic receiver/method target.
    pub fn method_targets(
        &self,
    ) -> impl Iterator<Item = (ori_types::Idx, Name, CallableTarget)> + '_ {
        self.method_targets
            .iter()
            .map(|(&(receiver, method), &target)| (receiver, method, target))
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

#[cfg(test)]
mod tests;
