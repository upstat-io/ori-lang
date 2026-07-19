//! Closed-program AIMS realization.

use rustc_hash::FxHashMap;

use ori_arc::ArcFunction;
use ori_ir::{Name, StringInterner};
use ori_types::{Pool, TypeRegistry};

use super::ProgramRealizationError;

pub(super) fn run_aims(
    functions: &mut [ArcFunction],
    pool: &Pool,
    interner: &StringInterner,
    type_registry: &TypeRegistry,
    external_contracts: &FxHashMap<Name, ori_arc::MemoryContract>,
    callable_boundaries: &ori_arc::CallableBoundaryFacts,
    verify_arc: bool,
) -> Result<ori_arc::ArcPipelineBatchOutcome, ProgramRealizationError> {
    let classifier = ori_arc::ArcClassifier::new(pool);
    let builtins = ori_arc::BuiltinOwnershipSets::new(interner);
    let outcome = ori_arc::realize_closed_program(
        functions,
        &ori_arc::ArcPipelineContext {
            classifier: &classifier,
            interner,
            pool,
            builtins: &builtins,
            type_registry,
            callable_boundaries,
            verify_arc,
            external_contracts,
        },
    )
    .map_err(|errors| ProgramRealizationError::ArcVerification {
        count: errors.len(),
        errors,
    })?;
    if outcome.problems.is_empty() {
        Ok(outcome)
    } else {
        Err(ProgramRealizationError::Aims {
            count: outcome.problems.len(),
            problems: outcome.problems,
        })
    }
}
