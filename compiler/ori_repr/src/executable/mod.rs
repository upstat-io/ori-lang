//! Validated, closed program shared by executable backends.

mod error;
mod runtime;

use ori_arc::{ArcFunction, ArcInstr, ArcTerminator, CtorKind};
use ori_ir::{Name, SharedInterner, StringInterner};
use ori_types::{Pool, TypeRegistry};
use rustc_hash::FxHashMap;

use crate::ReprPlan;

pub use error::RealizationError;
pub use runtime::RuntimeCall;

/// Schema version for the in-memory executable-program contract.
pub const EXECUTABLE_PROGRAM_VERSION: u32 = 1;

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
    /// Stable name of the program entry point.
    pub main: Name,
    /// Representation decisions computed once for all backends.
    pub repr_plan: ReprPlan,
    /// Closed type and burden metadata used by runtime lowering.
    pub type_registry: TypeRegistry,
}

/// Immutable executable artifact accepted by VM and native lowerers.
pub struct ExecutableProgram {
    version: u32,
    symbols: SharedInterner,
    pool: Pool,
    functions: Box<[ArcFunction]>,
    function_ids: FxHashMap<Name, FunctionId>,
    call_targets: FxHashMap<CallSite, CallableTarget>,
    main: FunctionId,
    repr_plan: ReprPlan,
    type_registry: TypeRegistry,
}

impl ExecutableProgram {
    /// Validate and close every direct callable reference in a program.
    pub fn validate(mut parts: ExecutableProgramParts) -> Result<Self, RealizationError> {
        validate_version(parts.version)?;
        validate_function_symbols(&parts.functions, &parts.symbols)?;
        parts.functions.sort_by(|left, right| {
            parts
                .symbols
                .lookup(left.name)
                .cmp(parts.symbols.lookup(right.name))
                .then_with(|| left.name.raw().cmp(&right.name.raw()))
        });
        let function_ids = build_function_ids(&parts.functions)?;
        let main = function_ids
            .get(&parts.main)
            .copied()
            .ok_or(RealizationError::MissingEntryPoint { name: parts.main })?;
        let call_targets =
            build_call_targets(&parts.functions, &function_ids, &parts.symbols, &parts.pool)?;
        Ok(Self {
            version: parts.version,
            symbols: parts.symbols,
            pool: parts.pool,
            functions: parts.functions.into_boxed_slice(),
            function_ids,
            call_targets,
            main,
            repr_plan: parts.repr_plan,
            type_registry: parts.type_registry,
        })
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

    /// Return the stable entry-point function index.
    #[must_use]
    pub const fn main(&self) -> FunctionId {
        self.main
    }

    /// Resolve a function name to its stable executable index.
    #[must_use]
    pub fn function_id(&self, name: Name) -> Option<FunctionId> {
        self.function_ids.get(&name).copied()
    }

    /// Return the pre-resolved target at a direct callable reference.
    #[must_use]
    pub fn call_target(&self, site: CallSite) -> Option<CallableTarget> {
        self.call_targets.get(&site).copied()
    }

    /// Return the representation plan shared by executable backends.
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

fn build_call_targets(
    functions: &[ArcFunction],
    function_ids: &FxHashMap<Name, FunctionId>,
    symbols: &StringInterner,
    pool: &Pool,
) -> Result<FxHashMap<CallSite, CallableTarget>, RealizationError> {
    let mut targets = FxHashMap::default();
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
                if let Some((name, arguments, closure_only)) = instruction_target(instruction) {
                    let target =
                        resolve_callable(function, name, arguments, function_ids, symbols, pool)?;
                    validate_closure_target(function.name, name, target, closure_only)?;
                    targets.insert(CallSite::new(function_id, block_id, position), target);
                }
            }
            if let Some((name, arguments)) = terminator_target(&block.terminator) {
                let target =
                    resolve_callable(function, name, arguments, function_ids, symbols, pool)?;
                targets.insert(
                    CallSite::new(function_id, block_id, CallPosition::Terminator),
                    target,
                );
            }
        }
    }
    Ok(targets)
}

fn instruction_target(instruction: &ArcInstr) -> Option<(Name, &[ori_arc::ArcVarId], bool)> {
    match instruction {
        ArcInstr::Apply { func, args, .. } => Some((*func, args, false)),
        ArcInstr::PartialApply { func, args, .. }
        | ArcInstr::Construct {
            ctor: CtorKind::Closure { func },
            args,
            ..
        } => Some((*func, args, true)),
        _ => None,
    }
}

fn terminator_target(terminator: &ArcTerminator) -> Option<(Name, &[ori_arc::ArcVarId])> {
    match terminator {
        ArcTerminator::Invoke { func, args, .. } => Some((*func, args)),
        _ => None,
    }
}

fn resolve_callable(
    caller: &ArcFunction,
    callee: Name,
    arguments: &[ori_arc::ArcVarId],
    function_ids: &FxHashMap<Name, FunctionId>,
    symbols: &StringInterner,
    pool: &Pool,
) -> Result<CallableTarget, RealizationError> {
    if let Some(&function) = function_ids.get(&callee) {
        return Ok(CallableTarget::Function(function));
    }
    let receiver = arguments
        .first()
        .and_then(|receiver| caller.var_types.get(receiver.index()))
        .and_then(|&receiver| pool.builtin_type_tag(pool.resolve_fully(receiver)));
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
    if closure_only && matches!(target, CallableTarget::Runtime(_)) {
        Err(RealizationError::InvalidClosureTarget { caller, callee })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
