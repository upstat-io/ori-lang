//! Function calls, returns, and panic propagation.

use ori_repr::executable::{CallableTarget, FunctionId};

use crate::bytecode::{Continuation, Pc, Register};
use crate::{ExecutionError, IndexKind};

use super::frame::{Frame, ReturnTo};
use super::value::VmValue;
use super::Interpreter;

impl Interpreter<'_> {
    pub(in crate::execute) fn call(
        &mut self,
        caller: usize,
        destination: Register,
        callee: CallableTarget,
        operands: usize,
        normal: Continuation,
        unwind: Option<Pc>,
    ) -> Result<(), ExecutionError> {
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
            ),
            CallableTarget::Runtime(call) => {
                match self.execute_runtime(caller, destination, call, operands) {
                    Ok(value) => {
                        self.set_register(caller, destination, value);
                        self.frames[caller].pc = normal;
                        Ok(())
                    }
                    Err(error) => self.raise_runtime(caller, unwind, error),
                }
            }
        }
    }

    fn raise_runtime(
        &mut self,
        frame: usize,
        unwind: Option<Pc>,
        error: ExecutionError,
    ) -> Result<(), ExecutionError> {
        if let Some(target) = unwind {
            self.pending_panic = Some(error);
            self.frames[frame].pc = target;
            Ok(())
        } else {
            Err(error)
        }
    }

    pub(in crate::execute) fn push_root(&mut self) {
        let function = self.program.main;
        let bytecode = self.function(function);
        self.frames.push(Frame::new(function, bytecode, None));
        self.depth = 1;
        self.peak_frames = 1;
    }

    fn push_call(
        &mut self,
        caller: usize,
        function: FunctionId,
        operands: usize,
        return_to: ReturnTo,
    ) -> Result<(), ExecutionError> {
        if self.depth >= self.config.max_frames {
            return Err(ExecutionError::ResourceLimit {
                resource: "call frames",
                limit: self.config.max_frames,
            });
        }
        let bytecode = self.function(function);
        let arguments = self.operands(operands);
        let values = arguments
            .iter()
            .map(|&argument| self.register(caller, argument))
            .collect::<Vec<_>>();
        let parameters = bytecode.params.to_vec();
        let entry = bytecode.entry;
        let register_count = bytecode.register_count;
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
        for (parameter, value) in parameters.into_iter().zip(values) {
            self.set_register(child, parameter, value);
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
        let value = self.promote_escaping(value, frame, caller, Some(return_to.destination))?;
        self.set_register(caller, return_to.destination, value);
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
}
