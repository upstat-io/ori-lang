//! Typed failures produced while closing an executable program.

use ori_arc::{ArcVarId, EffectSummary, RcStrategy, ValueRepr};
use ori_ir::Name;
use ori_registry::FnSym;
use ori_types::Idx;

use super::CallPosition;

/// A failure to construct a closed backend-neutral executable program.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RealizationError {
    /// The artifact schema version is not supported by this compiler.
    #[error("unsupported executable-program version {found}; expected {expected}")]
    UnsupportedVersion { found: u32, expected: u32 },
    /// The program contains more functions than its stable index can represent.
    #[error("executable program contains too many functions: {count}")]
    TooManyFunctions { count: usize },
    /// The external callable table exceeds its stable index representation.
    #[error("executable program contains too many external functions: {count}")]
    TooManyExternalFunctions { count: usize },
    /// A function name is absent from the supplied immutable symbol table.
    #[error("function name {name:?} is absent from the executable symbol table")]
    UnknownFunctionName { name: Name },
    /// Two realized function bodies use the same stable name.
    #[error("duplicate realized function {name:?}")]
    DuplicateFunction { name: Name },
    /// A validated function was not assigned an executable identity.
    #[error("realized function {name:?} has no executable function identity")]
    MissingFunctionIdentity { name: Name },
    /// A family topology names a parent without a realized body.
    #[error("function-family parent {parent:?} has no realized function body")]
    UnknownFunctionFamilyParent { parent: Name },
    /// A family topology names a lambda without a realized body.
    #[error(
        "function-family parent {parent:?} names lambda {lambda:?}, but that lambda has no realized function body"
    )]
    UnknownFunctionFamilyLambda { parent: Name, lambda: Name },
    /// One body appears in more than one parent/lambda family position.
    #[error("realized function {member:?} appears more than once in function-family topology")]
    DuplicateFunctionFamilyMember { member: Name },
    /// A realized body has no parent/lambda family position.
    #[error("realized function {function:?} has no function-family topology")]
    MissingFunctionFamily { function: Name },
    /// A realized function has no final AIMS contract from the batch pipeline.
    #[error("the ARC pipeline produced no final AIMS contract for function '{function_symbol}'; run whole-program AIMS realization before selecting an executable backend")]
    MissingFunctionContract {
        function: Name,
        function_symbol: Box<str>,
    },
    /// A final AIMS contract has no corresponding realized function body.
    #[error("final AIMS contract {function:?} has no realized function body")]
    UnexpectedFunctionContract { function: Name },
    /// A final AIMS contract does not cover the realized function parameters.
    #[error("the ARC pipeline produced an invalid final AIMS contract for function '{function_symbol}': {parameters} parameters but {contract_parameters} contract entries")]
    FunctionContractArity {
        function: Name,
        function_symbol: Box<str>,
        parameters: usize,
        contract_parameters: usize,
    },
    /// A realized function has no final RL-30 facts from the batch pipeline.
    #[error("the ARC pipeline produced no final RL-30 facts for function '{function_symbol}'")]
    MissingFunctionEffectFacts {
        function: Name,
        function_symbol: Box<str>,
    },
    /// Final RL-30 facts have no corresponding realized function body.
    #[error("final RL-30 facts for {function:?} have no realized function body")]
    UnexpectedFunctionEffectFacts { function: Name },
    /// Frozen RL-30 facts do not carry the final contract's exact effects.
    #[error("final RL-30 facts for function '{function_symbol}' carry effects {facts:?}, but the final contract carries {contract:?}")]
    FunctionEffectFactsMismatch {
        function: Name,
        function_symbol: Box<str>,
        contract: EffectSummary,
        facts: EffectSummary,
    },
    /// Frozen RL-30 facts claim read-only despite an inaccessible write.
    #[error("final RL-30 facts for function '{function_symbol}' claim read-only while inaccessible-memory writes remain possible")]
    InvalidFunctionEffectFacts {
        function: Name,
        function_symbol: Box<str>,
    },
    /// A realized function has no final RL-29 facts from the batch pipeline.
    #[error("the ARC pipeline produced no final RL-29 facts for function '{function_symbol}'")]
    MissingFreshReturnFacts {
        function: Name,
        function_symbol: Box<str>,
    },
    /// Final RL-29 facts have no corresponding realized function body.
    #[error("final RL-29 facts for {function:?} have no realized function body")]
    UnexpectedFreshReturnFacts { function: Name },
    /// Frozen RL-29 facts disagree with the final return contract.
    #[error("final RL-29 facts for function '{function_symbol}' carry fresh-self-allocation proof {facts}, but the final contract carries {contract}")]
    FreshReturnFactsMismatch {
        function: Name,
        function_symbol: Box<str>,
        contract: bool,
        facts: bool,
    },
    /// A realized function has no final RL-31 facts from the batch pipeline.
    #[error("the ARC pipeline produced no final RL-31 facts for function '{function_symbol}'")]
    MissingParamDisjointnessFacts {
        function: Name,
        function_symbol: Box<str>,
    },
    /// Final RL-31 facts have no corresponding realized function body.
    #[error("final RL-31 facts for {function:?} have no realized function body")]
    UnexpectedParamDisjointnessFacts { function: Name },
    /// Final RL-31 facts do not cover the realized parameter list.
    #[error("the ARC pipeline produced invalid final RL-31 facts for function '{function_symbol}': {parameters} parameters but {fact_parameters} fact entries")]
    ParamDisjointnessArity {
        function: Name,
        function_symbol: Box<str>,
        parameters: usize,
        fact_parameters: usize,
    },
    /// A realized function has no register-parallel callable facts.
    #[error("the ARC pipeline produced no callable facts for function '{function_symbol}'")]
    MissingCallableFacts {
        function: Name,
        function_symbol: Box<str>,
    },
    /// Callable facts have no corresponding realized function body.
    #[error("callable facts for {function:?} have no realized function body")]
    UnexpectedCallableFacts { function: Name },
    /// Callable facts disagree with a realized SSA relationship.
    #[error("invalid callable facts for function '{function_symbol}': {details}")]
    InvalidCallableFacts {
        function: Name,
        function_symbol: Box<str>,
        details: Box<str>,
    },
    /// A closure target has no adapter plan from the shared AIMS owner.
    #[error("the ARC pipeline produced no closure adapter facts for target '{target_symbol}'")]
    MissingClosureAdapterFacts {
        target: Name,
        target_symbol: Box<str>,
    },
    /// A supplied adapter plan does not match the realized target signature.
    #[error("closure adapter facts for target '{target_symbol}' disagree with the realized target signature")]
    ClosureAdapterFactsMismatch {
        target: Name,
        target_symbol: Box<str>,
    },
    /// Adapter facts were supplied for a function not used as a closure target.
    #[error("closure adapter facts for {target:?} have no closure construction site")]
    UnexpectedClosureAdapterFacts { target: Name },
    /// A closure target cannot be represented by the reusable borrowed-call ABI.
    #[error("invalid closure adapter facts for target '{target_symbol}': {details}")]
    InvalidClosureAdapterFacts {
        target: Name,
        target_symbol: Box<str>,
        details: Box<str>,
    },
    /// The supplied logical retain graph is not closed or deterministic.
    #[error("invalid executable retain-plan facts: {details}")]
    InvalidRetainPlanFacts { details: Box<str> },
    /// The selected entry point is not one of the realized functions.
    #[error("entry point {name:?} has no realized function body")]
    MissingEntryPoint { name: Name },
    /// A nonempty function body set has no externally reachable root.
    #[error("executable program must declare at least one root function")]
    MissingProgramRoots,
    /// A declared root has no realized body.
    #[error("program root {name:?} has no realized function body")]
    MissingProgramRoot { name: Name },
    /// Nested lambda bodies cannot be externally reachable program roots.
    #[error("program root {name:?} is a nested lambda; declare its owning parent as the root")]
    ProgramRootIsLambda { name: Name },
    /// The root set contains the same function more than once.
    #[error("program root {name:?} is declared more than once")]
    DuplicateProgramRoot { name: Name },
    /// A command-line entry must be one of the explicit program roots.
    #[error("command-line entry {name:?} is not an explicit program root")]
    CliEntryNotRoot { name: Name },
    /// A user-drop binding names an invalid artifact-local type.
    #[error("user-drop binding refers to invalid type {ty:?}")]
    InvalidUserDropType { ty: Idx },
    /// Two user-drop bindings resolve to the same nominal type.
    #[error("duplicate user-drop binding for type {ty:?}")]
    DuplicateUserDropBinding { ty: Idx },
    /// A supplied user-drop binding has no corresponding burden fact.
    #[error("user-drop binding for type {ty:?} has no user-drop burden")]
    UnexpectedUserDropBinding { ty: Idx },
    /// A burden-bearing type has no exact callable binding.
    #[error("user-drop burden for type {ty:?} has no realized callable binding")]
    MissingUserDropBinding { ty: Idx },
    /// Two nominal burden entries collapse to one pool identity.
    #[error("user-drop type {ty:?} collides at canonical type {canonical:?}")]
    UserDropTypeIdentityCollision { ty: Idx, canonical: Idx },
    /// The binding's logical operation identity disagrees with the burden.
    #[error("user-drop binding for type {ty:?} carries logical identity {found:?}, expected {expected:?}")]
    UserDropLogicalIdentityMismatch {
        ty: Idx,
        expected: FnSym,
        found: FnSym,
    },
    /// The exact callable target is absent from the realized function batch.
    #[error("user-drop binding for type {ty:?} targets unrealized function {target:?}")]
    MissingUserDropTarget { ty: Idx, target: Name },
    /// The exact callable does not implement `fn(Self) -> unit`.
    #[error("user-drop target {target:?} for type {ty:?} has invalid signature: {parameters} parameters, first parameter {parameter_type:?}, return {return_type:?}")]
    InvalidUserDropSignature {
        ty: Idx,
        target: Name,
        parameters: usize,
        parameter_type: Option<Idx>,
        return_type: Idx,
    },
    /// An external callable name is absent from immutable symbol storage.
    #[error("external callable name {name:?} is absent from the executable symbol table")]
    UnknownExternalName { name: Name },
    /// A method-target receiver refers outside the artifact-local type pool.
    #[error("method target for {method:?} refers to invalid receiver type {receiver:?}")]
    InvalidMethodTargetReceiver { receiver: Idx, method: Name },
    /// A method identity is absent from immutable symbol storage.
    #[error("method target name {method:?} is absent from the executable symbol table")]
    UnknownMethodTargetName { method: Name },
    /// A receiver/method target names neither a realized body nor an external callable.
    #[error("method target for receiver {receiver:?} and method {method:?} names unavailable callable {target:?}")]
    MissingMethodTarget {
        receiver: Idx,
        method: Name,
        target: Name,
    },
    /// A semantic method cannot target a nested lambda body.
    #[error("method target for receiver {receiver:?} and method {method:?} names nested lambda {target:?}")]
    MethodTargetIsLambda {
        receiver: Idx,
        method: Name,
        target: Name,
    },
    /// A local body and an external declaration claim the same callable name.
    #[error("callable {name:?} is declared as both a realized body and an external function")]
    ExternalFunctionBodyCollision { name: Name },
    /// Two external declarations use the same import-local name.
    #[error("duplicate external callable {name:?}")]
    DuplicateExternalFunction { name: Name },
    /// An external declaration has no physical link symbol.
    #[error("external callable {name:?} has an empty link symbol")]
    EmptyExternalLinkSymbol { name: Name },
    /// External signature and ownership facts cover different arities.
    #[error("external callable {name:?} has {parameters} signature parameters but {contract_parameters} ownership-contract entries")]
    ExternalContractArity {
        name: Name,
        parameters: usize,
        contract_parameters: usize,
    },
    /// The explicit unwind fact disagrees with final effects.
    #[error("external callable {name:?} has inconsistent effect and unwind facts")]
    ExternalUnwindMismatch { name: Name },
    /// An external signature refers to a type outside this artifact's pool.
    #[error("external callable {name:?} has an invalid semantic signature")]
    InvalidExternalSignature { name: Name },
    /// Imported compatibility hashes do not describe the supplied facts.
    #[error("external callable {name:?} carries missing or stale signature/ownership/effect/unwind facts")]
    StaleExternalFacts { name: Name },
    /// Aliases of one link symbol must agree on all logical facts.
    #[error("external callable aliases {first:?} and {second:?} share a link symbol but carry different facts")]
    ExternalAliasFactsMismatch { first: Name, second: Name },
    /// A direct external call disagrees with the producer signature facts.
    #[error(
        "call from {caller:?} to external callable {callee:?} disagrees with its frozen signature"
    )]
    ExternalCallSignatureMismatch { caller: Name, callee: Name },
    /// A direct external call disagrees with the producer ownership facts.
    #[error("call from {caller:?} to external callable {callee:?} disagrees with its frozen ownership contract")]
    ExternalCallOwnershipMismatch { caller: Name, callee: Name },
    /// The ARC owner did not mark the derived variable tables ready.
    #[error("the ARC pipeline did not finish variable metadata for function '{function_symbol}'; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug")]
    VariableMetadataUnrealized {
        function: Name,
        function_symbol: Box<str>,
    },
    /// A function's representation table does not cover every variable.
    #[error("the ARC pipeline produced incomplete variable metadata for function '{function_symbol}': {variables} variables but {representations} representation entries; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug")]
    RepresentationMetadata {
        function: Name,
        function_symbol: Box<str>,
        variables: usize,
        representations: usize,
    },
    /// A function's RC-strategy table does not cover every variable.
    #[error("the ARC pipeline produced incomplete variable metadata for function '{function_symbol}': {variables} variables but {strategies} RC-strategy entries; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug")]
    RcStrategyMetadata {
        function: Name,
        function_symbol: Box<str>,
        variables: usize,
        strategies: usize,
    },
    /// Representation and RC-strategy presence disagree for one variable.
    #[error("the ARC pipeline produced inconsistent variable metadata for function '{function_symbol}' register {register}: representation {representation:?} has RC strategy {strategy:?}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug")]
    RcStrategyShape {
        function: Name,
        function_symbol: Box<str>,
        register: usize,
        representation: ValueRepr,
        strategy: Option<RcStrategy>,
    },
    /// A cached strategy disagrees with the strategy derived during realization.
    #[error("the ARC pipeline produced inconsistent variable metadata for function '{function_symbol}' register {register}: cached RC strategy {found:?}, expected {expected:?}; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug")]
    RcStrategyCoherence {
        function: Name,
        function_symbol: Box<str>,
        register: usize,
        expected: Option<RcStrategy>,
        found: Option<RcStrategy>,
    },
    /// Post-AIMS call ownership does not cover every source argument.
    #[error("the ARC pipeline produced incomplete call ownership for function '{function_symbol}', block {block}, {position:?}: {arguments} arguments but {ownership_entries} ownership entries; rerun the same command with ORI_VERIFY_ARC=1 and report this compiler bug")]
    CallOwnershipMetadata {
        function: Name,
        function_symbol: Box<str>,
        block: usize,
        position: CallPosition,
        arguments: usize,
        ownership_entries: usize,
    },
    /// A residual indirect call violated the shared all-borrowed convention.
    #[error("the ARC pipeline produced non-borrowed indirect-call ownership for function '{function_symbol}', block {block}, {position:?}, argument {argument}; residual indirect arguments must remain Borrowed")]
    IndirectCallOwnership {
        function: Name,
        function_symbol: Box<str>,
        block: usize,
        position: CallPosition,
        argument: usize,
    },
    /// Two lowering facts claim one direct-call result register.
    #[error("the ARC pipeline produced duplicate method-call provenance for function '{function_symbol}' register {destination:?}")]
    DuplicateMethodCallFact {
        function: Name,
        function_symbol: Box<str>,
        destination: ArcVarId,
    },
    /// A lowering fact no longer has a corresponding direct call.
    #[error("the ARC pipeline produced orphaned method-call provenance for function '{function_symbol}' register {destination:?}; the transform that removed the call must remove its provenance fact")]
    OrphanMethodCallFact {
        function: Name,
        function_symbol: Box<str>,
        destination: ArcVarId,
    },
    /// An instance call has no explicit receiver operand.
    #[error("the ARC pipeline produced an instance-method call without a receiver in function '{function_symbol}' register {destination:?}")]
    MissingMethodReceiver {
        function: Name,
        function_symbol: Box<str>,
        destination: ArcVarId,
    },
    /// The explicit receiver disagrees with type-checker-selected provenance.
    #[error("the ARC pipeline produced incoherent method-call provenance in function '{function_symbol}' register {destination:?}: selected owner {expected:?}, receiver operand {found:?}")]
    MethodReceiverMismatch {
        function: Name,
        function_symbol: Box<str>,
        destination: ArcVarId,
        expected: Idx,
        found: Idx,
    },
    /// Frozen generated-call provenance is incomplete or internally inconsistent.
    #[error("the ARC pipeline produced invalid generated-call provenance for function '{function_symbol}': {details}")]
    InvalidGeneratedCallProvenance {
        function: Name,
        function_symbol: Box<str>,
        details: Box<str>,
    },
    /// Frozen primitive facts do not exactly cover or describe the realized body.
    #[error("the ARC pipeline produced invalid primitive facts for function '{function_symbol}': {details}; rerun whole-program AIMS realization and report this compiler bug")]
    InvalidPrimitiveFacts {
        function: Name,
        function_symbol: Box<str>,
        details: Box<str>,
    },
    /// A specialized ARC body still contains a quantified type variable.
    #[error("executable-program closure found an unresolved bound variable in function '{function_symbol}', register {var_id:?}, type index {idx:?}; run shared pre-AIMS lambda specialization before closing the backend-neutral artifact")]
    UnresolvedBoundVar {
        function: Name,
        function_symbol: Box<str>,
        var_id: ArcVarId,
        idx: Idx,
    },
    /// A direct call has no realized function or runtime descriptor.
    #[error(
        "executable-program realization cannot resolve call to '{callee_symbol}' from '{caller_symbol}': no realized function body or runtime operation is registered; add a realized body or RuntimeCall mapping for '{callee_symbol}' before selecting an executable backend"
    )]
    MissingCallable {
        /// Stable caller identity.
        caller: Name,
        /// Stable callee identity.
        callee: Name,
        /// Human-readable caller spelling captured before symbol storage moves.
        caller_symbol: Box<str>,
        /// Human-readable callee spelling captured before symbol storage moves.
        callee_symbol: Box<str>,
    },
    /// A closure constructor names a runtime descriptor instead of a function body.
    #[error("function {caller:?} captures non-function target {callee:?}")]
    InvalidClosureTarget { caller: Name, callee: Name },
    /// Two direct calls define the same stable result register.
    #[error("function {function:?} has duplicate direct-call result register {destination:?}")]
    DuplicateDirectCallResult {
        function: Name,
        destination: ArcVarId,
    },
    /// A block index cannot be represented in the executable call-site table.
    #[error("function {function:?} has too many basic blocks")]
    TooManyBlocks { function: Name },
    /// An instruction index cannot be represented in the executable call-site table.
    #[error("a basic block in function {function:?} has too many instructions")]
    TooManyInstructions { function: Name },
}
