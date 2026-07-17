//! Lowering from a closed executable program to unverified bytecode.

mod instruction;
mod terminator;

#[cfg(test)]
mod tests;

use ori_arc::{ArcBlockId, ArcFunction, ArcVarId, ArgOwnership};
use ori_ir::Name;
use ori_repr::{
    executable::{CallableTarget, ExecutableProgram, IteratorSource, RuntimeCall},
    BuiltinType,
};

use crate::bytecode::{
    BytecodeFunction, BytecodeProgram, BytecodeProgramParts, CallArgument, CallArgumentListId,
    MoveListId, OperandListId, Pc, Register, RegisterClass, StringId, SwitchTableId, TableKind,
    VmCalleeOwnerDemand, VmClosureAdapterAction, VmClosureAdapterPlan, VmClosureAdapterSlot,
    VmClosureAdapterSource, VmClosureValueSignature, VmRetainEdge, VmRetainPlan, VmRetainPlanId,
    VmRetainPlanKind, VmTypeId,
};
use crate::CompileError;

/// Explicit bytecode-lowering experiment controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileOptions {
    /// Select verifier-backed typed primitive operations.
    pub typed_primitives: bool,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            typed_primitives: true,
        }
    }
}

/// Compile a backend-neutral executable artifact to bytecode.
///
/// The result remains non-executable until [`crate::verify`] succeeds.
pub fn compile(program: &ExecutableProgram) -> Result<BytecodeProgram, CompileError> {
    compile_with_options(program, CompileOptions::default())
}

/// Compile with explicit lowering controls for paired experiments.
pub fn compile_with_options(
    program: &ExecutableProgram,
    options: CompileOptions,
) -> Result<BytecodeProgram, CompileError> {
    Compiler::new(program, options).compile()
}

struct Compiler<'a> {
    source: &'a ExecutableProgram,
    options: CompileOptions,
    call_arguments: Vec<Box<[CallArgument]>>,
    operands: Vec<Box<[Register]>>,
    moves: Vec<Box<[(Register, Register)]>>,
    switches: Vec<Box<[(u64, Pc)]>>,
    strings: Vec<String>,
}

impl<'a> Compiler<'a> {
    fn new(source: &'a ExecutableProgram, options: CompileOptions) -> Self {
        Self {
            source,
            options,
            call_arguments: Vec::new(),
            operands: Vec::new(),
            moves: Vec::new(),
            switches: Vec::new(),
            strings: Vec::new(),
        }
    }

    fn compile(mut self) -> Result<BytecodeProgram, CompileError> {
        let entry = self
            .source
            .cli_entry()
            .ok_or(CompileError::MissingCliEntry)?;
        let retain_plans = compile_retain_plans(self.source.retain_plans());
        let mut functions = Vec::with_capacity(self.source.functions().len());
        for function in self.source.functions() {
            functions.push(self.compile_function(function)?);
        }
        Ok(BytecodeProgram::from_parts(BytecodeProgramParts {
            functions,
            call_arguments: self.call_arguments,
            operands: self.operands,
            moves: self.moves,
            switches: self.switches,
            strings: self.strings,
            retain_plans,
            main: entry,
        }))
    }

    fn compile_function(
        &mut self,
        function: &ArcFunction,
    ) -> Result<BytecodeFunction, CompileError> {
        let (starts, capacity) = block_starts(function)?;
        let register_rc_strategies = function.var_rc_strategies.clone().into_boxed_slice();
        let mut ops = Vec::with_capacity(capacity);
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.body.iter().enumerate() {
                ops.push(self.compile_instruction(
                    function,
                    block_index,
                    instruction_index,
                    instruction,
                    &register_rc_strategies,
                )?);
            }
            ops.push(self.compile_terminator(function, block_index, &starts, &block.terminator)?);
        }
        let entry = block_pc(function.name, &starts, function.entry)?;
        let function_id = self.source.function_id(function.name).ok_or(
            CompileError::MissingFunctionIdentity {
                function: function.name,
            },
        )?;
        Ok(BytecodeFunction {
            name: function.name,
            params: function
                .params
                .iter()
                .map(|parameter| Register::from_arc(parameter.var))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            return_type: VmTypeId::from_raw(function.return_type.raw()),
            param_ownership: function
                .params
                .iter()
                .map(|parameter| parameter.ownership)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            capture_count: function.num_captures,
            closure_adapter: self
                .source
                .closure_adapter(function_id)
                .map(compile_closure_adapter),
            ops: ops.into_boxed_slice(),
            entry,
            register_count: function.var_types.len(),
            register_types: function
                .var_types
                .iter()
                .map(|ty| VmTypeId::from_raw(ty.raw()))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            register_classes: function
                .var_types
                .iter()
                .zip(&function.var_rc_strategies)
                .map(|(&ty, &strategy)| {
                    register_class(self.source.pool().builtin_type_tag(ty), strategy)
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            register_rc_strategies,
            register_closure_signatures: self
                .source
                .callable_facts(function_id)
                .register_signatures()
                .iter()
                .map(|signature| signature.as_ref().map(compile_closure_signature))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        })
    }

    fn add_operands(&mut self, args: &[ArcVarId]) -> Result<OperandListId, CompileError> {
        let id = OperandListId::new(self.operands.len(), TableKind::Operands)?;
        self.operands.push(
            args.iter()
                .copied()
                .map(Register::from_arc)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        Ok(id)
    }

    fn add_call_arguments(
        &mut self,
        function: Name,
        args: &[ArcVarId],
        ownership: &[ArgOwnership],
    ) -> Result<CallArgumentListId, CompileError> {
        if args.len() != ownership.len() {
            return Err(CompileError::CallOwnershipArity {
                function,
                arguments: args.len(),
                ownership_entries: ownership.len(),
            });
        }
        let id = CallArgumentListId::new(self.call_arguments.len(), TableKind::CallArguments)?;
        self.call_arguments.push(
            args.iter()
                .copied()
                .zip(ownership.iter().copied())
                .map(|(argument, ownership)| {
                    CallArgument::new(Register::from_arc(argument), ownership)
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        );
        Ok(id)
    }

    fn add_moves(&mut self, moves: Vec<(Register, Register)>) -> Result<MoveListId, CompileError> {
        let id = MoveListId::new(self.moves.len(), TableKind::Moves)?;
        self.moves.push(moves.into_boxed_slice());
        Ok(id)
    }

    fn add_switch(&mut self, cases: Vec<(u64, Pc)>) -> Result<SwitchTableId, CompileError> {
        let id = SwitchTableId::new(self.switches.len(), TableKind::Switches)?;
        self.switches.push(cases.into_boxed_slice());
        Ok(id)
    }

    fn add_string(&mut self, value: String) -> Result<StringId, CompileError> {
        let id = StringId::new(self.strings.len(), TableKind::Strings)?;
        self.strings.push(value);
        Ok(id)
    }
}

fn compile_closure_adapter(plan: &ori_arc::ClosureAdapterPlan) -> VmClosureAdapterPlan {
    VmClosureAdapterPlan {
        capture_count: plan.capture_count(),
        slots: plan
            .slots()
            .iter()
            .map(|slot| VmClosureAdapterSlot {
                source: match slot.source {
                    ori_arc::ClosureAdapterSource::EnvironmentCapture => {
                        VmClosureAdapterSource::EnvironmentCapture
                    }
                    ori_arc::ClosureAdapterSource::BorrowedCallArgument => {
                        VmClosureAdapterSource::BorrowedCallArgument
                    }
                },
                ty: VmTypeId::from_raw(slot.ty.raw()),
                demand: match slot.demand {
                    ori_arc::CalleeOwnerDemand::Borrow => VmCalleeOwnerDemand::Borrow,
                    ori_arc::CalleeOwnerDemand::WholeValue => VmCalleeOwnerDemand::WholeValue,
                    ori_arc::CalleeOwnerDemand::ProjectedField(field) => {
                        VmCalleeOwnerDemand::ProjectedField(field)
                    }
                },
                action: match slot.action {
                    ori_arc::ClosureAdapterAction::Borrow => VmClosureAdapterAction::Borrow,
                    ori_arc::ClosureAdapterAction::Copy => VmClosureAdapterAction::Copy,
                    ori_arc::ClosureAdapterAction::Retain(plan) => {
                        VmClosureAdapterAction::Retain(VmRetainPlanId::from_shared(plan))
                    }
                },
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    }
}

fn compile_closure_signature(
    signature: &ori_arc::ClosureValueSignature,
) -> VmClosureValueSignature {
    VmClosureValueSignature {
        ty: VmTypeId::from_raw(signature.ty().raw()),
        parameters: signature
            .parameters()
            .iter()
            .map(|ty| VmTypeId::from_raw(ty.raw()))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        result: VmTypeId::from_raw(signature.result().raw()),
    }
}

fn compile_retain_plans(table: &ori_arc::RetainPlanTable) -> Vec<VmRetainPlan> {
    table
        .nodes()
        .iter()
        .map(|node| VmRetainPlan {
            ty: VmTypeId::from_raw(node.ty.raw()),
            kind: match &node.kind {
                ori_arc::RetainPlanKind::SelfOwnedIdentity => VmRetainPlanKind::SelfOwnedIdentity,
                ori_arc::RetainPlanKind::OwnedFields(edges) => {
                    VmRetainPlanKind::OwnedFields(compile_retain_edges(edges))
                }
                ori_arc::RetainPlanKind::OwnedVariants(variants) => {
                    VmRetainPlanKind::OwnedVariants(
                        variants
                            .iter()
                            .map(|edges| compile_retain_edges(edges))
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    )
                }
            },
        })
        .collect()
}

fn compile_retain_edges(edges: &[ori_arc::RetainPlanEdge]) -> Box<[VmRetainEdge]> {
    edges
        .iter()
        .map(|edge| VmRetainEdge {
            field: edge.field,
            child: VmRetainPlanId::from_shared(edge.child),
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn validate_vm_call_target(
    function: Name,
    target: CallableTarget,
) -> Result<CallableTarget, CompileError> {
    match target {
        CallableTarget::Runtime(RuntimeCall::Iter(
            source @ (IteratorSource::Str
            | IteratorSource::Map
            | IteratorSource::Set
            | IteratorSource::Option),
        )) => Err(CompileError::UnsupportedIteratorSource {
            function,
            iterator_source: source,
        }),
        CallableTarget::Runtime(
            RuntimeCall::Iter(IteratorSource::Range | IteratorSource::List)
            | RuntimeCall::ListNew
            | RuntimeCall::ListFree
            | RuntimeCall::IterNext
            | RuntimeCall::ListBuilderPush
            | RuntimeCall::ListPush
            | RuntimeCall::ListInsert
            | RuntimeCall::ListRemove
            | RuntimeCall::ListPrepend
            | RuntimeCall::IterDrop
            | RuntimeCall::ListTake
            | RuntimeCall::Index
            | RuntimeCall::ListSet
            | RuntimeCall::Length
            | RuntimeCall::ToString
            | RuntimeCall::Concat
            | RuntimeCall::StringContains
            | RuntimeCall::StringStartsWith
            | RuntimeCall::StringEndsWith
            | RuntimeCall::StringIsEmpty
            | RuntimeCall::StringTrim
            | RuntimeCall::StringUppercase
            | RuntimeCall::StringLowercase
            | RuntimeCall::StringSplit
            | RuntimeCall::Print
            | RuntimeCall::Panic,
        )
        | CallableTarget::Function(_) => Ok(target),
        CallableTarget::Runtime(
            call @ (RuntimeCall::RegisteredMethod(_)
            | RuntimeCall::RegistryMethod(_)
            | RuntimeCall::RegistryPrelude(_)
            | RuntimeCall::Protocol(_)
            | RuntimeCall::Compiler(_)),
        ) => Err(CompileError::UnsupportedRuntimeCall { function, call }),
        CallableTarget::External(external) => {
            Err(CompileError::UnsupportedExternalCall { function, external })
        }
    }
}

const fn register_class(
    type_tag: Option<BuiltinType>,
    rc_strategy: Option<ori_arc::RcStrategy>,
) -> RegisterClass {
    if matches!(rc_strategy, Some(ori_arc::RcStrategy::Closure)) {
        return RegisterClass::Closure;
    }
    match type_tag {
        Some(BuiltinType::Int) => RegisterClass::Int,
        Some(BuiltinType::Bool) => RegisterClass::Bool,
        Some(BuiltinType::Str) => RegisterClass::String,
        _ => RegisterClass::Other,
    }
}

fn block_starts(function: &ArcFunction) -> Result<(Vec<Pc>, usize), CompileError> {
    let mut starts = Vec::with_capacity(function.blocks.len());
    let mut next = 0_usize;
    for block in &function.blocks {
        starts.push(Pc::new(next, function.name)?);
        next = next
            .checked_add(block.body.len())
            .and_then(|value| value.checked_add(1))
            .ok_or(CompileError::FunctionTooLarge {
                function: function.name,
                count: usize::MAX,
            })?;
    }
    Pc::new(next, function.name)?;
    Ok((starts, next))
}

fn block_pc(function: Name, starts: &[Pc], block: ArcBlockId) -> Result<Pc, CompileError> {
    starts
        .get(block.index())
        .copied()
        .ok_or(CompileError::InvalidBlock {
            function,
            block: block.index(),
        })
}
