//! Release-active proof that a physical plan is a faithful canonical projection.

use crate::bytecode::{
    walk_register_operands, BytecodeFunction, Op, RegisterOperand, VerifiedProgram,
};

use super::analysis::physical_read;
use super::facts::{KnownFacts, PlanningScratchMetrics};
use super::layout::{PhysicalExecutionView, PhysicalFunctionPlan, PhysicalFunctionView};
use super::solver::analyze;
use super::{PhysicalLayoutViolation, PhysicalOptions, PhysicalRead, PrepareError};

pub(super) fn validate_plan(
    program: &VerifiedProgram,
    functions: &[PhysicalFunctionPlan],
    options: PhysicalOptions,
) -> Result<PlanningScratchMetrics, PrepareError> {
    if functions.len() != program.program.functions.len() {
        return Err(PrepareError::FunctionCountMismatch {
            expected: program.program.functions.len(),
            actual: functions.len(),
        });
    }
    let mut planning = PlanningScratchMetrics::default();
    for (function_index, physical) in functions.iter().enumerate() {
        let canonical = &program.program.functions[function_index];
        planning = planning.then(validate_function(
            &program.program,
            function_index,
            canonical,
            physical,
            options,
        )?)?;
    }
    Ok(planning)
}

fn validate_function(
    program: &crate::bytecode::BytecodeProgram,
    function_index: usize,
    canonical: &BytecodeFunction,
    physical: &PhysicalFunctionPlan,
    options: PhysicalOptions,
) -> Result<PlanningScratchMetrics, PrepareError> {
    let function = validate_layout(function_index, canonical, physical)?;
    let analysis = analyze(program, function_index, canonical)?;
    let mut expected_read_start = 0_usize;
    let mut expected_write_start = 0_usize;
    let planning = analysis.replay(canonical, |pc_index, state| {
        let operation = canonical.ops[pc_index];
        let pc = function.canonical_pc_at(pc_index);
        if pc.read_start() != expected_read_start {
            return invalid(
                function_index,
                PhysicalLayoutViolation::ReadSpanOrder {
                    canonical_pc: pc_index,
                    expected_start: expected_read_start,
                    actual_start: pc.read_start(),
                },
            );
        }
        if pc.write_start() != expected_write_start {
            return invalid(
                function_index,
                PhysicalLayoutViolation::WriteSpanOrder {
                    canonical_pc: pc_index,
                    expected_start: expected_write_start,
                    actual_start: pc.write_start(),
                },
            );
        }
        OperandValidator {
            program,
            function_index,
            pc_index,
            physical,
            state,
            options,
            reads: pc.reads(),
            writes: pc.writes(),
        }
        .validate(operation)?;
        let expected_copy_is_noop =
            options.coalesce_copies && matches!(operation, Op::Copy { dst, src } if dst == src);
        if pc.copy_is_noop() != expected_copy_is_noop {
            return invalid(
                function_index,
                PhysicalLayoutViolation::CopyNoopProofMismatch {
                    canonical_pc: pc_index,
                    expected: expected_copy_is_noop,
                    actual: pc.copy_is_noop(),
                },
            );
        }
        expected_read_start = expected_read_start.checked_add(pc.reads().len()).ok_or(
            PrepareError::MetricOverflow {
                metric: "validated read coverage",
            },
        )?;
        expected_write_start = expected_write_start.checked_add(pc.writes().len()).ok_or(
            PrepareError::MetricOverflow {
                metric: "validated write coverage",
            },
        )?;
        Ok(())
    })?;
    if expected_read_start != function.read_count() {
        return invalid(
            function_index,
            PhysicalLayoutViolation::ReadCoverage {
                covered: expected_read_start,
                read_bindings: function.read_count(),
            },
        );
    }
    if expected_write_start != function.write_count() {
        return invalid(
            function_index,
            PhysicalLayoutViolation::WriteCoverage {
                covered: expected_write_start,
                write_bindings: function.write_count(),
            },
        );
    }
    Ok(planning)
}

fn validate_layout<'plan>(
    function_index: usize,
    canonical: &BytecodeFunction,
    physical: &'plan PhysicalFunctionPlan,
) -> Result<PhysicalFunctionView<'plan>, PrepareError> {
    let function = PhysicalExecutionView::new(core::slice::from_ref(physical)).function_at(0);
    if function.lane_count() != canonical.register_count {
        return invalid(
            function_index,
            PhysicalLayoutViolation::LaneCount {
                expected: canonical.register_count,
                actual: function.lane_count(),
            },
        );
    }
    if function.canonical_pc_count() != canonical.ops.len() {
        return invalid(
            function_index,
            PhysicalLayoutViolation::CanonicalOpCount {
                expected: canonical.ops.len(),
                actual: function.canonical_pc_count(),
            },
        );
    }
    function
        .validate_construction()
        .map_err(|violation| PrepareError::InvalidLayout {
            function: function_index,
            violation,
        })?;
    for lane_index in 0..function.lane_count() {
        let actual = function.lane_at(lane_index).index();
        if actual != lane_index {
            return invalid(
                function_index,
                PhysicalLayoutViolation::LaneIdentity {
                    lane: lane_index,
                    expected: lane_index,
                    actual,
                },
            );
        }
    }

    Ok(function)
}

struct OperandValidator<'a> {
    program: &'a crate::bytecode::BytecodeProgram,
    function_index: usize,
    pc_index: usize,
    physical: &'a PhysicalFunctionPlan,
    state: Option<&'a KnownFacts>,
    options: PhysicalOptions,
    reads: &'a [PhysicalRead],
    writes: &'a [super::PhysicalLane],
}

impl OperandValidator<'_> {
    fn validate(self, operation: Op) -> Result<(), PrepareError> {
        let mut read_count = 0_usize;
        let mut write_count = 0_usize;
        let mut first_violation = None;
        walk_register_operands(self.program, operation, |operand| match operand {
            RegisterOperand::Read(register) => {
                if let Some(actual) = self.reads.get(read_count) {
                    let expected = physical_read(
                        &self.physical.lanes,
                        self.state,
                        register,
                        self.options.bind_immediates,
                    );
                    if *actual != expected && first_violation.is_none() {
                        first_violation = Some(match (*actual, expected) {
                            (PhysicalRead::Lane(_), PhysicalRead::Lane(_)) => {
                                PhysicalLayoutViolation::ReadBindingMismatch {
                                    canonical_pc: self.pc_index,
                                    operand: read_count,
                                }
                            }
                            _ => PhysicalLayoutViolation::ImmediateProofMismatch {
                                canonical_pc: self.pc_index,
                                operand: read_count,
                            },
                        });
                    }
                }
                read_count += 1;
            }
            RegisterOperand::Write(register) => {
                if let Some(actual) = self.writes.get(write_count) {
                    let expected = self.physical.lanes[register.index()];
                    if *actual != expected && first_violation.is_none() {
                        first_violation = Some(PhysicalLayoutViolation::WriteBindingMismatch {
                            canonical_pc: self.pc_index,
                            operand: write_count,
                        });
                    }
                }
                write_count += 1;
            }
        })
        .map_err(|error| PrepareError::CanonicalOperandTable {
            function: self.function_index,
            canonical_pc: self.pc_index,
            table: error.table,
            index: error.index,
            bound: error.bound,
        })?;
        if read_count != self.reads.len() {
            return invalid(
                self.function_index,
                PhysicalLayoutViolation::ReadOperandCount {
                    canonical_pc: self.pc_index,
                    expected: read_count,
                    actual: self.reads.len(),
                },
            );
        }
        if write_count != self.writes.len() {
            return invalid(
                self.function_index,
                PhysicalLayoutViolation::WriteOperandCount {
                    canonical_pc: self.pc_index,
                    expected: write_count,
                    actual: self.writes.len(),
                },
            );
        }
        if let Some(violation) = first_violation {
            return invalid(self.function_index, violation);
        }
        Ok(())
    }
}

fn invalid<T>(function: usize, violation: PhysicalLayoutViolation) -> Result<T, PrepareError> {
    Err(PrepareError::InvalidLayout {
        function,
        violation,
    })
}
