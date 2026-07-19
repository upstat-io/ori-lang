//! Function calls, returns, and panic propagation.

use ori_repr::executable::{CallableTarget, FunctionId};

use crate::bytecode::{Continuation, Pc};
use crate::{ExecutionError, IndexKind};

use super::frame::{Frame, ReturnTo};
use super::operands::{FrameSlot, OperandAccess};
use super::value::VmValue;
use super::Interpreter;

#[derive(Clone, Copy)]
pub(super) struct CallDispatch {
    pub(super) destination: FrameSlot,
    pub(super) callee: CallableTarget,
    pub(super) operands: usize,
    pub(super) normal: Continuation,
    pub(super) unwind: Option<Pc>,
}

#[derive(Clone, Copy)]
pub(super) struct ClosureCallDispatch {
    pub(super) destination: FrameSlot,
    pub(super) closure: VmValue,
    pub(super) operands: usize,
    pub(super) normal: Continuation,
    pub(super) unwind: Option<Pc>,
}

impl Interpreter<'_> {
    pub(in crate::execute) fn make_closure(
        &mut self,
        caller: usize,
        callee: FunctionId,
        captures: usize,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<VmValue, ExecutionError> {
        let capture_registers = &self.program.operands[captures];
        if capture_registers.len() > self.config.max_collection_elements {
            return Err(ExecutionError::ResourceLimit {
                resource: "closure captures",
                limit: self.config.max_collection_elements,
            });
        }
        let mut values = Vec::with_capacity(capture_registers.len());
        for &capture in capture_registers {
            values.push(operand_cursor.read(&self.frames[caller].registers, capture));
        }
        self.heap.allocate(
            super::heap::HeapObject::Closure {
                callee,
                captures: values,
            },
            self.config.max_heap_objects,
        )
    }

    pub(in crate::execute) fn call(
        &mut self,
        caller: usize,
        call: CallDispatch,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let CallDispatch {
            destination,
            callee,
            operands,
            normal,
            unwind,
        } = call;
        let normal = match normal {
            Continuation::Next => self.frames[caller].pc.next_verified(),
            Continuation::At(target) => target,
        };
        match callee {
            CallableTarget::Function(function) => self.push_call(
                caller,
                function,
                operands,
                ReturnTo {
                    destination,
                    normal,
                    unwind,
                },
                operand_cursor,
            ),
            CallableTarget::Runtime(runtime_call) => {
                match self.execute_runtime(
                    caller,
                    destination,
                    runtime_call,
                    operands,
                    operand_cursor,
                ) {
                    Ok(value) => {
                        self.set_slot(caller, destination, value);
                        self.frames[caller].pc = normal;
                        Ok(())
                    }
                    Err(error) => self.raise_runtime(caller, unwind, error),
                }
            }
            CallableTarget::External(external) => {
                Err(ExecutionError::ExternalCallTarget { external })
            }
        }
    }

    pub(in crate::execute) fn call_closure(
        &mut self,
        caller: usize,
        call: ClosureCallDispatch,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        let ClosureCallDispatch {
            destination,
            closure,
            operands,
            normal,
            unwind,
        } = call;
        self.ensure_frame_capacity()?;
        let normal = match normal {
            Continuation::Next => self.frames[caller].pc.next_verified(),
            Continuation::At(target) => target,
        };

        let (callee, stored_captures) = match &self.heap.get(closure)?.object {
            super::heap::HeapObject::Closure {
                callee,
                captures: stored,
            } => (*callee, stored.as_slice()),
            _ => return Err(ExecutionError::InvalidClosureObject),
        };

        let capture_count = self.program.functions[callee.index()].capture_count;
        let parameter_count = self.program.functions[callee.index()].params.len();
        let arguments = &self.program.call_arguments[operands];
        let expected_arguments =
            parameter_count
                .checked_sub(capture_count)
                .ok_or(ExecutionError::ClosureCallArity {
                    expected: 0,
                    actual: arguments.len(),
                })?;
        if stored_captures.len() != capture_count || arguments.len() != expected_arguments {
            return Err(ExecutionError::ClosureCallArity {
                expected: expected_arguments,
                actual: arguments.len(),
            });
        }
        let next_depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::ResourceLimit {
                resource: "call frames",
                limit: self.config.max_frames,
            })?;
        let entry = self.program.functions[callee.index()].entry;
        let register_count = self.layout.frame_slot_count(
            callee,
            self.program.functions[callee.index()].register_count,
        );

        // All validation that can reject a verified call happens before taking the reusable
        // buffer. Error exits therefore preserve both its capacity and the memory accounting
        // derived from that capacity.
        let mut values = std::mem::take(&mut self.closure_scratch);
        values.clear();
        values.extend_from_slice(stored_captures);
        for &argument in arguments {
            values.push(operand_cursor.read(&self.frames[caller].registers, argument.register()));
        }

        if let Err(error) = self.prepare_closure_adapter_values(callee, &mut values) {
            values.clear();
            self.closure_scratch = values;
            return Err(error);
        }

        let child = self.depth;
        let return_to = ReturnTo {
            destination,
            normal,
            unwind,
        };
        if child == self.frames.len() {
            self.frames.push(Frame::new_layout(
                callee,
                entry,
                register_count,
                Some(return_to),
            ));
        } else {
            self.frames[child].reset_layout(callee, entry, register_count, Some(return_to));
        }

        let parameters = &self.program.functions[callee.index()].params;
        let layout = self.layout;
        let (_, reusable_frames) = self.frames.split_at_mut(child);
        let child_registers = &mut reusable_frames[0].registers;
        for (&parameter, &value) in parameters.iter().zip(&values) {
            let destination = layout.frame_slot(callee, parameter);
            child_registers[destination.index()] = value;
        }
        self.commit_prepared_retains(1);
        self.depth = next_depth;
        self.peak_frames = self.peak_frames.max(self.depth);
        values.clear();
        self.closure_scratch = values;
        Ok(())
    }

    fn raise_runtime(
        &mut self,
        frame: usize,
        unwind: Option<Pc>,
        error: ExecutionError,
    ) -> Result<(), ExecutionError> {
        self.pending_panic = Some(error);
        if let Some(target) = unwind {
            self.frames[frame].pc = target;
            Ok(())
        } else {
            self.resume_panic(frame)
        }
    }

    pub(in crate::execute) fn push_root(&mut self) -> Result<(), ExecutionError> {
        self.ensure_frame_capacity()?;
        let function = self.program.main;
        let bytecode = self.function(function);
        let entry = bytecode.entry;
        let register_count = self
            .layout
            .frame_slot_count(function, bytecode.register_count);
        self.frames
            .push(Frame::new_layout(function, entry, register_count, None));
        self.depth = 1;
        self.peak_frames = 1;
        Ok(())
    }

    fn push_call(
        &mut self,
        caller: usize,
        function: FunctionId,
        operands: usize,
        return_to: ReturnTo,
        operand_cursor: &mut impl OperandAccess,
    ) -> Result<(), ExecutionError> {
        self.ensure_frame_capacity()?;
        let bytecode = self.function(function);
        let entry = bytecode.entry;
        let register_count = self
            .layout
            .frame_slot_count(function, bytecode.register_count);
        let child = self.depth;
        if child == self.frames.len() {
            self.frames.push(Frame::new_layout(
                function,
                entry,
                register_count,
                Some(return_to),
            ));
        } else {
            self.frames[child].reset_layout(function, entry, register_count, Some(return_to));
        }

        let arguments = &self.program.call_arguments[operands];
        let parameters = &self.program.functions[function.index()].params;
        let layout = self.layout;
        let (active_frames, reusable_frames) = self.frames.split_at_mut(child);
        let caller_registers = &active_frames[caller].registers;
        let child_registers = &mut reusable_frames[0].registers;
        for (&parameter, &argument) in parameters.iter().zip(arguments) {
            let value = operand_cursor.read(caller_registers, argument.register());
            let destination = layout.frame_slot(function, parameter);
            child_registers[destination.index()] = value;
        }
        self.depth = self
            .depth
            .checked_add(1)
            .ok_or(ExecutionError::ResourceLimit {
                resource: "call frames",
                limit: self.config.max_frames,
            })?;
        self.peak_frames = self.peak_frames.max(self.depth);
        Ok(())
    }

    pub(in crate::execute) fn return_from(
        &mut self,
        frame: usize,
        value: VmValue,
    ) -> Result<Option<VmValue>, ExecutionError> {
        let return_to = self.frames[frame].return_to;
        self.depth = self.depth.checked_sub(1).ok_or_else(|| {
            super::invalid_verified_index(IndexKind::Function, frame, self.frames.len())
        })?;
        let Some(return_to) = return_to else {
            return Ok(Some(value));
        };
        let caller = self.depth - 1;
        self.set_slot(caller, return_to.destination, value);
        self.frames[caller].pc = return_to.normal;
        Ok(None)
    }

    pub(in crate::execute) fn resume_panic(
        &mut self,
        mut frame: usize,
    ) -> Result<(), ExecutionError> {
        if self.pending_panic.is_none() {
            return Err(ExecutionError::ResumeWithoutPanic);
        }
        loop {
            let return_to = self.frames[frame].return_to;
            self.depth = self.depth.checked_sub(1).ok_or_else(|| {
                super::invalid_verified_index(IndexKind::Function, frame, self.frames.len())
            })?;
            let Some(return_to) = return_to else {
                return Err(self
                    .pending_panic
                    .take()
                    .ok_or(ExecutionError::ResumeWithoutPanic)?);
            };
            let caller = self.depth - 1;
            if let Some(unwind) = return_to.unwind {
                self.frames[caller].pc = unwind;
                return Ok(());
            }
            frame = caller;
        }
    }

    fn ensure_frame_capacity(&self) -> Result<(), ExecutionError> {
        if self.depth >= self.config.max_frames {
            Err(ExecutionError::ResourceLimit {
                resource: "call frames",
                limit: self.config.max_frames,
            })
        } else {
            Ok(())
        }
    }
}
