//! Structural verification for compiled bytecode.

use ori_ir::Name;
use ori_repr::executable::CallableTarget;

use crate::bytecode::{
    walk_register_operands, BytecodeFunction, BytecodeProgram, CallArgument, Constant,
    Continuation, IntBinaryOp, Op, Pc, Register, RegisterClass, RegisterOperand, StringBinaryOp,
    TableKind, VerifiedProgram, VmCalleeOwnerDemand, VmClosureAdapterAction,
    VmClosureAdapterSource, VmRetainPlanKind,
};
use crate::{IndexKind, VerifyError};

fn verify_retain_plans(program: &BytecodeProgram) -> Result<(), VerifyError> {
    for (plan_index, plan) in program.retain_plans.iter().enumerate() {
        match &plan.kind {
            VmRetainPlanKind::SelfOwnedIdentity => {}
            VmRetainPlanKind::OwnedFields(edges) => {
                verify_retain_edges(edges, plan_index, program.retain_plans.len())?;
            }
            VmRetainPlanKind::OwnedVariants(variants) => {
                for edges in variants {
                    verify_retain_edges(edges, plan_index, program.retain_plans.len())?;
                }
            }
        }
    }
    Ok(())
}

fn verify_retain_edges(
    edges: &[crate::bytecode::VmRetainEdge],
    parent: usize,
    plan_count: usize,
) -> Result<(), VerifyError> {
    let mut previous = None;
    for edge in edges {
        if edge.child.index() >= plan_count {
            return Err(VerifyError::InvalidRetainPlanMetadata {
                plan: parent,
                details: "edge references a missing retain plan",
            });
        }
        if edge.child.index() >= parent {
            return Err(VerifyError::InvalidRetainPlanMetadata {
                plan: parent,
                details: "children are not in child-before-parent order",
            });
        }
        if previous.is_some_and(|field| field >= edge.field) {
            return Err(VerifyError::InvalidRetainPlanMetadata {
                plan: parent,
                details: "logical fields are not strictly ordered and unique",
            });
        }
        previous = Some(edge.field);
    }
    Ok(())
}

/// Verify every bytecode reference and consume the unverified artifact.
pub fn verify(program: BytecodeProgram) -> Result<VerifiedProgram, VerifyError> {
    verify_retain_plans(&program)?;
    let main = program.functions.get(program.main.index()).ok_or_else(|| {
        invalid_index(
            Name::EMPTY,
            None,
            IndexKind::Function,
            program.main.index(),
            program.functions.len(),
        )
    })?;
    if !main.params.is_empty() {
        return Err(VerifyError::MainHasParameters {
            function: main.name,
            parameters: main.params.len(),
        });
    }

    for function in &program.functions {
        Verifier::new(&program, function).verify()?;
    }
    Ok(VerifiedProgram { program })
}

struct Verifier<'a> {
    program: &'a BytecodeProgram,
    function: &'a BytecodeFunction,
}

impl<'a> Verifier<'a> {
    const fn new(program: &'a BytecodeProgram, function: &'a BytecodeFunction) -> Self {
        Self { program, function }
    }

    fn verify(&self) -> Result<(), VerifyError> {
        if self.function.register_classes.len() != self.function.register_count {
            return Err(VerifyError::RegisterMetadata {
                function: self.function.name,
                registers: self.function.register_count,
                classes: self.function.register_classes.len(),
            });
        }
        if self.function.register_types.len() != self.function.register_count {
            return Err(VerifyError::RegisterTypeMetadata {
                function: self.function.name,
                registers: self.function.register_count,
                types: self.function.register_types.len(),
            });
        }
        if self.function.register_closure_signatures.len() != self.function.register_count {
            return Err(VerifyError::CallableRegisterMetadata {
                function: self.function.name,
                registers: self.function.register_count,
                signatures: self.function.register_closure_signatures.len(),
            });
        }
        if self.function.register_rc_strategies.len() != self.function.register_count {
            return Err(VerifyError::RcRegisterMetadata {
                function: self.function.name,
                registers: self.function.register_count,
                strategies: self.function.register_rc_strategies.len(),
            });
        }
        if self.function.param_ownership.len() != self.function.params.len() {
            return Err(VerifyError::ParameterOwnershipMetadata {
                function: self.function.name,
                parameters: self.function.params.len(),
                ownership_entries: self.function.param_ownership.len(),
            });
        }
        if self.function.capture_count > self.function.params.len() {
            return Err(VerifyError::CaptureMetadata {
                function: self.function.name,
                captures: self.function.capture_count,
                parameters: self.function.params.len(),
            });
        }
        self.pc(None, self.function.entry)?;
        for &parameter in &self.function.params {
            self.register(None, parameter)?;
        }
        self.callable_metadata()?;
        self.closure_adapter()?;
        for (pc, operation) in self.function.ops.iter().enumerate() {
            self.operation(pc, *operation)?;
        }
        Ok(())
    }

    fn operation(&self, pc: usize, operation: Op) -> Result<(), VerifyError> {
        if operation.needs_next_pc() && pc + 1 >= self.function.ops.len() {
            return Err(VerifyError::InvalidFallthrough {
                function: self.function.name,
                pc,
            });
        }
        self.register_operands(pc, operation)?;
        match operation {
            Op::Const { value, .. } => self.constant(pc, value)?,
            Op::IntBinary { dst, op, lhs, rhs } => self.int_binary(pc, dst, op, lhs, rhs)?,
            Op::StringBinary { dst, op, lhs, rhs } => {
                self.string_binary(pc, dst, op, lhs, rhs)?;
            }
            Op::RuntimeBinary {
                dst,
                operator,
                lhs,
                rhs,
            } => self.runtime_binary(pc, dst, operator, lhs, rhs)?,
            Op::BoolNot { dst, arg } => self.bool_not(pc, dst, arg)?,
            Op::Call {
                callee,
                args,
                normal,
                unwind,
                ..
            } => self.call(pc, callee, args.index(), normal, unwind)?,
            Op::MakeClosure {
                dst,
                callee,
                captures,
            } => self.make_closure_operation(pc, dst, callee, captures.index())?,
            Op::CallClosure {
                dst,
                closure,
                args,
                normal,
                unwind,
            } => self.call_closure_operation(pc, dst, closure, args.index(), normal, unwind)?,
            Op::Construct { args, .. } => {
                self.operand_list(pc, args.index())?;
            }
            Op::RcInc { var, semantics, .. } | Op::RcDec { var, semantics } => {
                self.rc_semantics(pc, var, semantics)?;
            }
            Op::Jump { target, moves } => self.jump(pc, target, moves.index())?,
            Op::Branch {
                then_pc, else_pc, ..
            } => self.branch(pc, then_pc, else_pc)?,
            Op::Switch {
                table, default_pc, ..
            } => self.switch(pc, table.index(), default_pc)?,
            Op::Copy { dst, src } => {
                self.same_callable_signature(pc, dst, src, "callable copy changes signature")?;
            }
            Op::Select {
                dst,
                true_value,
                false_value,
                ..
            } => self.select(pc, dst, true_value, false_value)?,
            Op::Binary { .. }
            | Op::Unary { .. }
            | Op::Project { .. }
            | Op::IsShared { .. }
            | Op::Set { .. }
            | Op::SetTag { .. }
            | Op::Return { .. }
            | Op::Resume
            | Op::Unreachable => {}
        }
        Ok(())
    }

    fn int_binary(
        &self,
        pc: usize,
        destination: Register,
        operation: IntBinaryOp,
        left: Register,
        right: Register,
    ) -> Result<(), VerifyError> {
        let result_class = if operation.returns_bool() {
            RegisterClass::Bool
        } else {
            RegisterClass::Int
        };
        self.typed_binary(
            pc,
            destination,
            left,
            right,
            RegisterClass::Int,
            result_class,
        )
    }

    fn string_binary(
        &self,
        pc: usize,
        destination: Register,
        operation: StringBinaryOp,
        left: Register,
        right: Register,
    ) -> Result<(), VerifyError> {
        let result_class = if operation.returns_bool() {
            RegisterClass::Bool
        } else {
            RegisterClass::String
        };
        self.typed_binary(
            pc,
            destination,
            left,
            right,
            RegisterClass::String,
            result_class,
        )
    }

    fn runtime_binary(
        &self,
        pc: usize,
        destination: Register,
        operator: ori_registry::RuntimeOperator,
        left: Register,
        right: Register,
    ) -> Result<(), VerifyError> {
        if operator != ori_registry::RuntimeOperator::ListConcat {
            return Err(VerifyError::UnsupportedRuntimePrimitive {
                function: self.function.name,
                pc,
                operator,
            });
        }
        self.typed_binary(
            pc,
            destination,
            left,
            right,
            RegisterClass::Other,
            RegisterClass::Other,
        )
    }

    fn bool_not(
        &self,
        pc: usize,
        destination: Register,
        argument: Register,
    ) -> Result<(), VerifyError> {
        self.typed_register(pc, destination, RegisterClass::Bool)?;
        self.typed_register(pc, argument, RegisterClass::Bool)
    }

    fn make_closure_operation(
        &self,
        pc: usize,
        destination: Register,
        target: ori_repr::executable::FunctionId,
        captures: usize,
    ) -> Result<(), VerifyError> {
        self.typed_register(pc, destination, RegisterClass::Closure)?;
        self.make_closure(pc, destination, target, captures)
    }

    fn call_closure_operation(
        &self,
        pc: usize,
        destination: Register,
        closure: Register,
        operands: usize,
        normal: Continuation,
        unwind: Option<Pc>,
    ) -> Result<(), VerifyError> {
        self.typed_register(pc, closure, RegisterClass::Closure)?;
        self.call_closure(pc, destination, closure, operands, normal, unwind)
    }

    fn jump(&self, pc: usize, target: Pc, moves: usize) -> Result<(), VerifyError> {
        self.pc(Some(pc), target)?;
        for &(destination, source) in self.moves(pc, moves)? {
            self.same_callable_signature(
                pc,
                destination,
                source,
                "callable block argument changes signature",
            )?;
        }
        Ok(())
    }

    fn branch(&self, pc: usize, then_pc: Pc, else_pc: Pc) -> Result<(), VerifyError> {
        self.pc(Some(pc), then_pc)?;
        self.pc(Some(pc), else_pc)
    }

    fn switch(&self, pc: usize, table: usize, default_pc: Pc) -> Result<(), VerifyError> {
        self.pc(Some(pc), default_pc)?;
        for &(_, target) in self.switches(pc, table)? {
            self.pc(Some(pc), target)?;
        }
        Ok(())
    }

    fn select(
        &self,
        pc: usize,
        destination: Register,
        true_value: Register,
        false_value: Register,
    ) -> Result<(), VerifyError> {
        self.same_callable_signature(
            pc,
            destination,
            true_value,
            "callable select true arm changes signature",
        )?;
        self.same_callable_signature(
            pc,
            destination,
            false_value,
            "callable select false arm changes signature",
        )
    }

    fn register_operands(&self, pc: usize, operation: Op) -> Result<(), VerifyError> {
        let mut first_error = None;
        walk_register_operands(self.program, operation, |operand| {
            let register = match operand {
                RegisterOperand::Read(register) | RegisterOperand::Write(register) => register,
            };
            if first_error.is_none() {
                first_error = self.register(Some(pc), register).err();
            }
        })
        .map_err(|error| {
            invalid_index(
                self.function.name,
                Some(pc),
                index_kind(error.table),
                error.index,
                error.bound,
            )
        })?;
        first_error.map_or(Ok(()), Err)
    }

    fn rc_semantics(
        &self,
        pc: usize,
        register: Register,
        semantics: crate::bytecode::RcSemantics,
    ) -> Result<(), VerifyError> {
        if !semantics.has_supported_atomicity() {
            return Err(VerifyError::UnsupportedRcAtomicity {
                function: self.function.name,
                pc,
                atomicity: semantics.atomicity,
            });
        }
        if !semantics.has_supported_strategy() {
            return Err(VerifyError::UnsupportedRcStrategy {
                function: self.function.name,
                pc,
                strategy: semantics.strategy,
            });
        }
        let expected = self.function.register_rc_strategies[register.index()];
        if expected != Some(semantics.strategy) {
            return Err(VerifyError::RcStrategyMismatch {
                function: self.function.name,
                pc,
                register: register.index(),
                expected,
                found: semantics.strategy,
            });
        }
        Ok(())
    }

    fn constant(&self, pc: usize, value: Constant) -> Result<(), VerifyError> {
        if let Constant::String(string) = value {
            self.index(
                Some(pc),
                IndexKind::String,
                string.index(),
                self.program.strings.len(),
            )?;
        }
        Ok(())
    }

    fn typed_binary(
        &self,
        pc: usize,
        destination: Register,
        left: Register,
        right: Register,
        operand_class: RegisterClass,
        result_class: RegisterClass,
    ) -> Result<(), VerifyError> {
        self.typed_register(pc, left, operand_class)?;
        self.typed_register(pc, right, operand_class)?;
        self.typed_register(pc, destination, result_class)
    }

    fn call(
        &self,
        pc: usize,
        target: CallableTarget,
        operands: usize,
        normal: Continuation,
        unwind: Option<Pc>,
    ) -> Result<(), VerifyError> {
        let arguments = self.call_arguments(pc, operands)?;
        let expected = match target {
            CallableTarget::Function(function) => self
                .program
                .functions
                .get(function.index())
                .ok_or_else(|| {
                    invalid_index(
                        self.function.name,
                        Some(pc),
                        IndexKind::Function,
                        function.index(),
                        self.program.functions.len(),
                    )
                })?
                .params
                .len(),
            CallableTarget::Runtime(
                call @ (ori_repr::executable::RuntimeCall::RegisteredMethod(_)
                | ori_repr::executable::RuntimeCall::RegistryMethod(_)
                | ori_repr::executable::RuntimeCall::RegistryPrelude(_)
                | ori_repr::executable::RuntimeCall::Protocol(_)
                | ori_repr::executable::RuntimeCall::Compiler(_)),
            ) => {
                return Err(VerifyError::UnsupportedRuntimeCall {
                    function: self.function.name,
                    pc,
                    call,
                });
            }
            CallableTarget::Runtime(call) => call.arity(),
            CallableTarget::External(external) => {
                return Err(VerifyError::ExternalCallTarget {
                    function: self.function.name,
                    pc,
                    external,
                });
            }
        };
        if arguments.len() != expected {
            return Err(VerifyError::CallArity {
                function: self.function.name,
                pc,
                target,
                expected,
                actual: arguments.len(),
            });
        }
        self.continuations(pc, normal, unwind)
    }

    fn make_closure(
        &self,
        pc: usize,
        destination: Register,
        target: ori_repr::executable::FunctionId,
        captures: usize,
    ) -> Result<(), VerifyError> {
        let target_function = self.program.functions.get(target.index()).ok_or_else(|| {
            invalid_index(
                self.function.name,
                Some(pc),
                IndexKind::Function,
                target.index(),
                self.program.functions.len(),
            )
        })?;
        let captures = self.operand_list(pc, captures)?;
        if captures.len() != target_function.capture_count {
            return Err(VerifyError::ClosureCaptureArity {
                function: self.function.name,
                pc,
                target,
                expected: target_function.capture_count,
                actual: captures.len(),
            });
        }
        let Some(adapter) = target_function.closure_adapter.as_ref() else {
            return Err(
                self.invalid_callable(Some(pc), "closure target has no frozen adapter metadata")
            );
        };
        for (&capture, slot) in captures.iter().zip(&adapter.slots) {
            if self.function.register_types[capture.index()] != slot.ty {
                return Err(self.invalid_callable(
                    Some(pc),
                    "closure capture type disagrees with target adapter",
                ));
            }
        }
        let Some(signature) =
            self.function.register_closure_signatures[destination.index()].as_ref()
        else {
            return Err(self.invalid_callable(
                Some(pc),
                "closure destination has no residual callable signature",
            ));
        };
        let residual = &adapter.slots[adapter.capture_count..];
        if signature.parameters.len() != residual.len()
            || signature
                .parameters
                .iter()
                .zip(residual)
                .any(|(parameter, slot)| *parameter != slot.ty)
            || signature.result != target_function.return_type
        {
            return Err(self.invalid_callable(
                Some(pc),
                "closure destination signature disagrees with residual target",
            ));
        }
        Ok(())
    }

    fn call_closure(
        &self,
        pc: usize,
        destination: Register,
        closure: Register,
        operands: usize,
        normal: Continuation,
        unwind: Option<Pc>,
    ) -> Result<(), VerifyError> {
        let arguments = self.call_arguments(pc, operands)?;
        let Some(signature) = self.function.register_closure_signatures[closure.index()].as_ref()
        else {
            return Err(
                self.invalid_callable(Some(pc), "indirect call operand has no callable signature")
            );
        };
        if arguments.len() != signature.parameters.len() {
            return Err(self.invalid_callable(
                Some(pc),
                "indirect call arity disagrees with callable signature",
            ));
        }
        for (argument, &expected) in arguments.iter().zip(&signature.parameters) {
            let argument_index = argument.register().index();
            if argument.ownership() != ori_arc::ArgOwnership::Borrowed {
                return Err(VerifyError::ClosureArgumentOwnership {
                    function: self.function.name,
                    pc,
                    argument: argument_index,
                });
            }
            if self.function.register_types[argument_index] != expected {
                return Err(self.invalid_callable(
                    Some(pc),
                    "indirect call argument type disagrees with callable signature",
                ));
            }
        }
        if self.function.register_types[destination.index()] != signature.result {
            return Err(self.invalid_callable(
                Some(pc),
                "indirect call result type disagrees with callable signature",
            ));
        }
        self.continuations(pc, normal, unwind)
    }

    fn callable_metadata(&self) -> Result<(), VerifyError> {
        for (register, signature) in self.function.register_closure_signatures.iter().enumerate() {
            let is_closure = self.function.register_classes[register] == RegisterClass::Closure;
            if is_closure != signature.is_some() {
                return Err(self.invalid_callable(
                    None,
                    "closure register class and callable signature presence disagree",
                ));
            }
            if signature
                .as_ref()
                .is_some_and(|signature| signature.ty != self.function.register_types[register])
            {
                return Err(self.invalid_callable(
                    None,
                    "callable signature type identity disagrees with register metadata",
                ));
            }
        }
        Ok(())
    }

    fn closure_adapter(&self) -> Result<(), VerifyError> {
        let Some(adapter) = self.function.closure_adapter.as_ref() else {
            return Ok(());
        };
        if adapter.capture_count != self.function.capture_count
            || adapter.slots.len() != self.function.params.len()
        {
            return Err(
                self.invalid_adapter("adapter arity disagrees with function signature metadata")
            );
        }
        for (index, (slot, &parameter)) in
            adapter.slots.iter().zip(&self.function.params).enumerate()
        {
            let expected_source = if index < adapter.capture_count {
                VmClosureAdapterSource::EnvironmentCapture
            } else {
                VmClosureAdapterSource::BorrowedCallArgument
            };
            if slot.source != expected_source
                || slot.ty != self.function.register_types[parameter.index()]
            {
                return Err(self.invalid_adapter(
                    "adapter slot source or type disagrees with function parameters",
                ));
            }
            match (slot.demand, slot.action) {
                (VmCalleeOwnerDemand::Borrow, VmClosureAdapterAction::Borrow)
                | (
                    VmCalleeOwnerDemand::WholeValue | VmCalleeOwnerDemand::ProjectedField(_),
                    VmClosureAdapterAction::Copy,
                ) => {}
                (
                    VmCalleeOwnerDemand::WholeValue | VmCalleeOwnerDemand::ProjectedField(_),
                    VmClosureAdapterAction::Retain(plan),
                ) => {
                    let retain = self.program.retain_plans.get(plan.index()).ok_or_else(|| {
                        invalid_index(
                            self.function.name,
                            None,
                            IndexKind::RetainPlan,
                            plan.index(),
                            self.program.retain_plans.len(),
                        )
                    })?;
                    if retain.ty != slot.ty {
                        return Err(self
                            .invalid_adapter("adapter retain-plan type disagrees with its slot"));
                    }
                }
                _ => {
                    return Err(
                        self.invalid_adapter("adapter action disagrees with final owner demand")
                    );
                }
            }
        }
        Ok(())
    }

    fn same_callable_signature(
        &self,
        pc: usize,
        left: Register,
        right: Register,
        details: &'static str,
    ) -> Result<(), VerifyError> {
        if self.function.register_closure_signatures[left.index()]
            == self.function.register_closure_signatures[right.index()]
        {
            Ok(())
        } else {
            Err(self.invalid_callable(Some(pc), details))
        }
    }

    fn invalid_callable(&self, pc: Option<usize>, details: &'static str) -> VerifyError {
        VerifyError::InvalidCallableMetadata {
            function: self.function.name,
            pc,
            details,
        }
    }

    fn invalid_adapter(&self, details: &'static str) -> VerifyError {
        VerifyError::InvalidClosureAdapterMetadata {
            function: self.function.name,
            details,
        }
    }

    fn continuations(
        &self,
        pc: usize,
        normal: Continuation,
        unwind: Option<Pc>,
    ) -> Result<(), VerifyError> {
        if let Continuation::At(target) = normal {
            self.pc(Some(pc), target)?;
        }
        if let Some(target) = unwind {
            self.pc(Some(pc), target)?;
        }
        Ok(())
    }

    fn call_arguments(&self, pc: usize, index: usize) -> Result<&'a [CallArgument], VerifyError> {
        let arguments = self.program.call_arguments.get(index).ok_or_else(|| {
            invalid_index(
                self.function.name,
                Some(pc),
                IndexKind::CallArguments,
                index,
                self.program.call_arguments.len(),
            )
        })?;
        Ok(arguments)
    }

    fn operand_list(&self, pc: usize, index: usize) -> Result<&'a [Register], VerifyError> {
        let operands = self.program.operands.get(index).ok_or_else(|| {
            invalid_index(
                self.function.name,
                Some(pc),
                IndexKind::Operands,
                index,
                self.program.operands.len(),
            )
        })?;
        Ok(operands)
    }

    fn moves(&self, pc: usize, index: usize) -> Result<&'a [(Register, Register)], VerifyError> {
        self.program
            .moves
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                invalid_index(
                    self.function.name,
                    Some(pc),
                    IndexKind::Moves,
                    index,
                    self.program.moves.len(),
                )
            })
    }

    fn switches(&self, pc: usize, index: usize) -> Result<&'a [(u64, Pc)], VerifyError> {
        self.program
            .switches
            .get(index)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                invalid_index(
                    self.function.name,
                    Some(pc),
                    IndexKind::Switch,
                    index,
                    self.program.switches.len(),
                )
            })
    }

    fn register(&self, pc: Option<usize>, register: Register) -> Result<(), VerifyError> {
        self.index(
            pc,
            IndexKind::Register,
            register.index(),
            self.function.register_count,
        )
    }

    fn typed_register(
        &self,
        pc: usize,
        register: Register,
        expected: RegisterClass,
    ) -> Result<(), VerifyError> {
        let found = self.function.register_classes[register.index()];
        if found == expected {
            Ok(())
        } else {
            Err(VerifyError::TypedRegister {
                function: self.function.name,
                pc,
                register: register.index(),
                expected: expected.name(),
                found: found.name(),
            })
        }
    }

    fn pc(&self, source: Option<usize>, target: Pc) -> Result<(), VerifyError> {
        self.index(
            source,
            IndexKind::ProgramCounter,
            target.index(),
            self.function.ops.len(),
        )
    }

    fn index(
        &self,
        pc: Option<usize>,
        kind: IndexKind,
        index: usize,
        bound: usize,
    ) -> Result<(), VerifyError> {
        if index < bound {
            Ok(())
        } else {
            Err(invalid_index(self.function.name, pc, kind, index, bound))
        }
    }
}

fn invalid_index(
    function: Name,
    pc: Option<usize>,
    kind: IndexKind,
    index: usize,
    bound: usize,
) -> VerifyError {
    VerifyError::InvalidIndex {
        function,
        pc,
        kind,
        index,
        bound,
    }
}

const fn index_kind(table: TableKind) -> IndexKind {
    match table {
        TableKind::Operands => IndexKind::Operands,
        TableKind::CallArguments => IndexKind::CallArguments,
        TableKind::Moves => IndexKind::Moves,
        TableKind::Switches => IndexKind::Switch,
        TableKind::Strings => IndexKind::String,
    }
}
