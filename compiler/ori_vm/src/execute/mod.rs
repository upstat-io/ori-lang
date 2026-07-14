//! Quota-bounded bytecode interpreter.

mod calls;
mod frame;
mod heap;
mod objects;
mod primitives;
mod runtime;
mod value;

use ori_repr::executable::FunctionId;

use crate::bytecode::{Constant, Op, Pc, Register, VerifiedProgram};
use crate::error::invalid_verified_index;
use crate::ExecutionError;

use frame::Frame;
use heap::Heap;
pub use value::ExitValue;
use value::VmValue;

/// Deterministic resource limits for one VM session.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionConfig {
    /// Maximum number of dispatched bytecode instructions.
    pub max_steps: u64,
    /// Maximum number of simultaneously active function frames.
    pub max_frames: usize,
    /// Maximum number of live reference-counted heap objects.
    pub max_heap_objects: usize,
    /// Maximum elements in any list or builder and bytes in any VM string.
    pub max_collection_elements: usize,
    /// Maximum aggregate/iterator slots retained by one reusable frame.
    pub max_frame_values: usize,
    /// Maximum bytes captured from interpreted print operations.
    pub max_output_bytes: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_steps: 2_000_000_000,
            max_frames: 1_000_000,
            max_heap_objects: 10_000_000,
            max_collection_elements: 100_000_000,
            max_frame_values: 1_000_000,
            max_output_bytes: 64 * 1024 * 1024,
        }
    }
}

/// Runtime metrics from one isolated execution session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionMetrics {
    /// Number of dispatched bytecode instructions.
    pub steps: u64,
    /// Peak number of active call frames.
    pub peak_frames: usize,
    /// Peak number of live heap objects.
    pub peak_heap_objects: usize,
}

/// Observable result of a successful interpreted run.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionOutcome {
    /// Materialized return value of `main`.
    pub value: ExitValue,
    /// Bytes emitted by interpreted print operations.
    pub output: Vec<u8>,
    /// Deterministic execution metrics.
    pub metrics: ExecutionMetrics,
}

/// Execute verified bytecode in a fresh isolated VM session.
pub fn execute(
    program: &VerifiedProgram,
    config: ExecutionConfig,
) -> Result<ExecutionOutcome, ExecutionError> {
    Interpreter::new(program, config).run()
}

struct Interpreter<'a> {
    program: &'a crate::bytecode::BytecodeProgram,
    config: ExecutionConfig,
    frames: Vec<Frame>,
    depth: usize,
    heap: Heap,
    output: Vec<u8>,
    steps: u64,
    peak_frames: usize,
    pending_panic: Option<ExecutionError>,
}

impl<'a> Interpreter<'a> {
    fn new(program: &'a VerifiedProgram, config: ExecutionConfig) -> Self {
        Self {
            program: &program.program,
            config,
            frames: Vec::new(),
            depth: 0,
            heap: Heap::default(),
            output: Vec::new(),
            steps: 0,
            peak_frames: 0,
            pending_panic: None,
        }
    }

    fn run(mut self) -> Result<ExecutionOutcome, ExecutionError> {
        self.push_root();
        let mut returned = VmValue::UNIT;
        while self.depth > 0 {
            self.record_step()?;
            if let Some(value) = self.step()? {
                returned = value;
            }
        }
        let value = self.materialize(returned)?;
        Ok(ExecutionOutcome {
            value,
            output: self.output,
            metrics: ExecutionMetrics {
                steps: self.steps,
                peak_frames: self.peak_frames,
                peak_heap_objects: self.heap.peak_live,
            },
        })
    }

    fn record_step(&mut self) -> Result<(), ExecutionError> {
        self.steps = self
            .steps
            .checked_add(1)
            .ok_or(ExecutionError::StepCounterOverflow)?;
        if self.steps > self.config.max_steps {
            Err(ExecutionError::StepLimit {
                limit: self.config.max_steps,
            })
        } else {
            Ok(())
        }
    }

    #[inline]
    fn step(&mut self) -> Result<Option<VmValue>, ExecutionError> {
        let frame = self.depth - 1;
        let operation = self.current_operation(frame);
        match operation {
            Op::Const { dst, value } => {
                self.store_and_advance(frame, dst, Self::constant(value)?);
            }
            Op::Copy { dst, src } => self.copy_and_advance(frame, dst, src),
            Op::Binary { dst, op, lhs, rhs } => {
                self.binary_and_advance(frame, dst, op, lhs, rhs)?;
            }
            Op::IntBinary { dst, op, lhs, rhs } => {
                self.int_binary_and_advance(frame, dst, op, lhs, rhs)?;
            }
            Op::StringBinary { dst, op, lhs, rhs } => {
                self.string_binary_and_advance(frame, dst, op, lhs, rhs)?;
            }
            Op::Unary { dst, op, arg } => self.unary_and_advance(frame, dst, op, arg)?,
            Op::BoolNot { dst, arg } => {
                let value = VmValue::bool(!self.register(frame, arg).as_bool()?);
                self.store_and_advance(frame, dst, value);
            }
            Op::Call {
                dst,
                callee,
                args,
                normal,
                unwind,
            } => self.call(frame, dst, callee, args.index(), normal, unwind)?,
            Op::Construct { dst, ctor, args } => {
                self.construct_and_advance(frame, dst, ctor, args.index())?;
            }
            Op::Project { dst, value, field } => {
                self.project_and_advance(frame, dst, value, field)?;
            }
            Op::RcInc { var, count } => {
                self.heap.increment(self.register(frame, var), count)?;
                self.advance(frame);
            }
            Op::RcDec { var } => {
                self.heap.decrement(self.register(frame, var))?;
                self.advance(frame);
            }
            Op::IsShared { dst, var } => self.shared_and_advance(frame, dst, var)?,
            Op::Set { base, field, value } => {
                self.set_field(
                    self.register(frame, base),
                    field,
                    self.register(frame, value),
                )?;
                self.advance(frame);
            }
            Op::SetTag { base, tag } => {
                self.set_tag(self.register(frame, base), tag)?;
                self.advance(frame);
            }
            Op::Select {
                dst,
                cond,
                true_value,
                false_value,
            } => self.select_and_advance(frame, dst, cond, true_value, false_value)?,
            Op::Jump { target, moves } => {
                self.execute_moves(frame, moves.index());
                self.frames[frame].pc = target;
            }
            Op::Branch {
                cond,
                then_pc,
                else_pc,
            } => {
                self.frames[frame].pc = if self.register(frame, cond).as_bool()? {
                    then_pc
                } else {
                    else_pc
                };
            }
            Op::Switch {
                scrutinee,
                table,
                default_pc,
            } => self.execute_switch(frame, scrutinee, table.index(), default_pc)?,
            Op::Return { value } => {
                let value = self.register(frame, value);
                return self.return_from(frame, value);
            }
            Op::Resume => self.resume_panic(frame)?,
            Op::Unreachable => return Err(ExecutionError::ReachedUnreachable),
        }
        Ok(None)
    }

    fn store_and_advance(&mut self, frame: usize, destination: Register, value: VmValue) {
        self.set_register(frame, destination, value);
        self.advance(frame);
    }

    fn copy_and_advance(&mut self, frame: usize, destination: Register, source: Register) {
        let value = self.register(frame, source);
        self.store_and_advance(frame, destination, value);
    }

    fn binary_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        operation: ori_ir::BinaryOp,
        left: Register,
        right: Register,
    ) -> Result<(), ExecutionError> {
        let value = self.execute_binary(
            operation,
            self.register(frame, left),
            self.register(frame, right),
        )?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    fn int_binary_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        operation: crate::bytecode::IntBinaryOp,
        left: Register,
        right: Register,
    ) -> Result<(), ExecutionError> {
        let value = primitives::int_binary(
            operation,
            self.register(frame, left),
            self.register(frame, right),
        )?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    fn unary_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        operation: ori_ir::UnaryOp,
        argument: Register,
    ) -> Result<(), ExecutionError> {
        let value = primitives::unary(operation, self.register(frame, argument))?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    fn construct_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        constructor: ori_arc::CtorKind,
        operands: usize,
    ) -> Result<(), ExecutionError> {
        let value = self.construct(frame, destination, constructor, operands)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    fn project_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        source: Register,
        field: u32,
    ) -> Result<(), ExecutionError> {
        let value = self.project(self.register(frame, source), field)?;
        self.store_and_advance(frame, destination, value);
        Ok(())
    }

    fn shared_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        source: Register,
    ) -> Result<(), ExecutionError> {
        let shared = self.heap.is_shared(self.register(frame, source))?;
        self.store_and_advance(frame, destination, VmValue::bool(shared));
        Ok(())
    }

    fn select_and_advance(
        &mut self,
        frame: usize,
        destination: Register,
        condition: Register,
        true_value: Register,
        false_value: Register,
    ) -> Result<(), ExecutionError> {
        let selected = if self.register(frame, condition).as_bool()? {
            self.register(frame, true_value)
        } else {
            self.register(frame, false_value)
        };
        self.store_and_advance(frame, destination, selected);
        Ok(())
    }

    fn execute_switch(
        &mut self,
        frame: usize,
        scrutinee: Register,
        table: usize,
        default: Pc,
    ) -> Result<(), ExecutionError> {
        let key = self.discriminant(self.register(frame, scrutinee))?;
        self.frames[frame].pc = self
            .switch_table(table)
            .iter()
            .find_map(|(value, target)| (*value == key).then_some(*target))
            .unwrap_or(default);
        Ok(())
    }

    #[inline]
    fn register(&self, frame: usize, register: Register) -> VmValue {
        self.frames[frame].registers[register.index()]
    }

    fn set_register(&mut self, frame: usize, register: Register, value: VmValue) {
        self.frames[frame].registers[register.index()] = value;
    }

    #[inline]
    fn current_operation(&self, frame: usize) -> Op {
        let frame = &self.frames[frame];
        self.program.functions[frame.function.index()].ops[frame.pc.index()]
    }

    #[inline]
    fn advance(&mut self, frame: usize) {
        self.frames[frame].pc = self.frames[frame].pc.next_verified();
    }

    fn function(&self, function: FunctionId) -> &crate::bytecode::BytecodeFunction {
        &self.program.functions[function.index()]
    }

    fn operands(&self, index: usize) -> &[Register] {
        &self.program.operands[index]
    }

    fn switch_table(&self, index: usize) -> &[(u64, Pc)] {
        &self.program.switches[index]
    }

    fn constant(constant: Constant) -> Result<VmValue, ExecutionError> {
        match constant {
            Constant::Int(value) => Ok(VmValue::int(value)),
            Constant::Float(bits) => Ok(VmValue::float(bits)),
            Constant::Bool(value) => Ok(VmValue::bool(value)),
            Constant::String(value) => VmValue::constant_string(value.index()),
            Constant::Char(value) => Ok(VmValue::char(value)),
            Constant::Unit => Ok(VmValue::UNIT),
            Constant::Null => Ok(VmValue::null()),
        }
    }
}
