//! Pure preparation of verified bytecode for an identity physical layout.

mod analysis;
mod error;
mod facts;
mod flow;
mod layout;
mod metrics;
mod solver;
mod validation;

#[cfg(test)]
mod tests;

use crate::bytecode::VerifiedProgram;

use analysis::prepare_function;
pub use error::{PhysicalLayoutViolation, PrepareError};
use facts::PlanningScratchMetrics;
use layout::PhysicalFunctionPlan;
pub(crate) use layout::{
    PhysicalExecutionView, PhysicalImmediate, PhysicalLane, PhysicalPcView, PhysicalRead,
};
use metrics::{function_storage_metrics, plan_metrics};
pub use metrics::{PhysicalElementSizes, PhysicalFunctionStorageMetrics, PhysicalPlanMetrics};
use validation::validate_plan;

/// Controls proof-backed physical planning without changing bytecode semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalOptions {
    /// Replace proven integer and boolean register reads with immediate values.
    pub bind_immediates: bool,
    /// Elide copies whose source and destination already have one binding.
    pub coalesce_copies: bool,
}

impl Default for PhysicalOptions {
    fn default() -> Self {
        Self {
            bind_immediates: true,
            coalesce_copies: true,
        }
    }
}

/// Construction-validated immutable physical layout bound to one verified program.
#[derive(Debug)]
pub struct PhysicalVmPlan<'program> {
    program: &'program VerifiedProgram,
    functions: Box<[PhysicalFunctionPlan]>,
    options: PhysicalOptions,
    metrics: PhysicalPlanMetrics,
}

/// Mutable construction product consumed by validation before execution.
#[derive(Debug)]
struct PhysicalVmPlanCandidate<'program> {
    program: &'program VerifiedProgram,
    functions: Box<[PhysicalFunctionPlan]>,
    options: PhysicalOptions,
    planning: PlanningScratchMetrics,
}

impl<'program> PhysicalVmPlanCandidate<'program> {
    fn validate(self) -> Result<PhysicalVmPlan<'program>, PrepareError> {
        let validation = validate_plan(self.program, &self.functions, self.options)?;
        let metrics = plan_metrics(self.program, &self.functions, self.planning, validation)?;
        Ok(PhysicalVmPlan {
            program: self.program,
            functions: self.functions,
            options: self.options,
            metrics,
        })
    }
}

impl PhysicalVmPlan<'_> {
    /// Return persistent physical-plan size and transformation metrics.
    #[must_use]
    pub const fn metrics(&self) -> PhysicalPlanMetrics {
        self.metrics
    }

    /// Return exact host sizes used by physical-plan memory accounting.
    #[must_use]
    pub const fn element_sizes(&self) -> PhysicalElementSizes {
        PhysicalElementSizes::current()
    }

    /// Return the proof-backed options used to construct this plan.
    #[must_use]
    pub const fn options(&self) -> PhysicalOptions {
        self.options
    }

    /// Return exact retained table payload for one function.
    #[must_use = "physical storage metrics or their typed error must be handled"]
    pub fn function_storage_metrics(
        &self,
        function_index: usize,
    ) -> Result<PhysicalFunctionStorageMetrics, PrepareError> {
        let canonical = self.program.program.functions.get(function_index).ok_or(
            PrepareError::FunctionIndexOutOfBounds {
                function: function_index,
                function_count: self.functions.len(),
            },
        )?;
        let physical =
            self.functions
                .get(function_index)
                .ok_or(PrepareError::FunctionIndexOutOfBounds {
                    function: function_index,
                    function_count: self.functions.len(),
                })?;
        function_storage_metrics(function_index, canonical, physical)
    }

    /// Return the exact verified program whose identities the plan binds.
    #[must_use]
    pub const fn program(&self) -> &VerifiedProgram {
        self.program
    }

    /// Return the number of function layouts in the physical plan.
    #[must_use]
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Return the executor-only view of this construction-validated plan.
    pub(crate) fn execution_view(&self) -> PhysicalExecutionView<'_> {
        PhysicalExecutionView::new(&self.functions)
    }
}

/// Prepare and validate an immutable physical layout without executing bytecode.
#[must_use = "physical preparation must succeed before the plan can execute"]
pub fn prepare(
    program: &VerifiedProgram,
    options: PhysicalOptions,
) -> Result<PhysicalVmPlan<'_>, PrepareError> {
    prepare_candidate(program, options)?.validate()
}

fn prepare_candidate(
    program: &VerifiedProgram,
    options: PhysicalOptions,
) -> Result<PhysicalVmPlanCandidate<'_>, PrepareError> {
    let mut functions = Vec::with_capacity(program.program.functions.len());
    let mut planning = PlanningScratchMetrics::default();
    for (function_index, function) in program.program.functions.iter().enumerate() {
        let (physical, function_planning) =
            prepare_function(&program.program, function_index, function, options)?;
        functions.push(physical);
        planning = planning.then(function_planning)?;
    }
    let functions = functions.into_boxed_slice();
    Ok(PhysicalVmPlanCandidate {
        program,
        functions,
        options,
        planning,
    })
}
