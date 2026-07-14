//! Lowering from a closed executable program to unverified bytecode.

mod instruction;
mod terminator;

use ori_arc::{ArcBlockId, ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_repr::{executable::ExecutableProgram, BuiltinType};

use crate::bytecode::{
    BytecodeFunction, BytecodeProgram, MoveListId, OperandListId, Pc, Register, RegisterClass,
    StringId, SwitchTableId, TableKind,
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
            operands: Vec::new(),
            moves: Vec::new(),
            switches: Vec::new(),
            strings: Vec::new(),
        }
    }

    fn compile(mut self) -> Result<BytecodeProgram, CompileError> {
        let mut functions = Vec::with_capacity(self.source.functions().len());
        for function in self.source.functions() {
            functions.push(self.compile_function(function)?);
        }
        Ok(BytecodeProgram::new(
            functions,
            self.operands,
            self.moves,
            self.switches,
            self.strings,
            self.source.main(),
        ))
    }

    fn compile_function(
        &mut self,
        function: &ArcFunction,
    ) -> Result<BytecodeFunction, CompileError> {
        let starts = block_starts(function)?;
        let capacity = function
            .blocks
            .iter()
            .try_fold(0_usize, |total, block| {
                total.checked_add(block.body.len().saturating_add(1))
            })
            .ok_or(CompileError::FunctionTooLarge {
                function: function.name,
                count: usize::MAX,
            })?;
        let mut ops = Vec::with_capacity(capacity);
        for (block_index, block) in function.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.body.iter().enumerate() {
                ops.push(self.compile_instruction(
                    function,
                    block_index,
                    instruction_index,
                    instruction,
                )?);
            }
            ops.push(self.compile_terminator(function, block_index, &starts, &block.terminator)?);
        }
        let entry = block_pc(function.name, &starts, function.entry)?;
        Ok(BytecodeFunction {
            name: function.name,
            params: function
                .params
                .iter()
                .map(|parameter| Register::from_arc(parameter.var))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            ops: ops.into_boxed_slice(),
            entry,
            register_count: function.var_types.len(),
            register_classes: function
                .var_types
                .iter()
                .map(|&ty| register_class(self.source.pool().builtin_type_tag(ty)))
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

const fn register_class(type_tag: Option<BuiltinType>) -> RegisterClass {
    match type_tag {
        Some(BuiltinType::Int) => RegisterClass::Int,
        Some(BuiltinType::Bool) => RegisterClass::Bool,
        Some(BuiltinType::Str) => RegisterClass::String,
        _ => RegisterClass::Other,
    }
}

fn block_starts(function: &ArcFunction) -> Result<Vec<Pc>, CompileError> {
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
    Ok(starts)
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
