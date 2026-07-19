//! Adapters from preparation/lowering failures into program realization errors.

use super::super::ArcBatchPreparationError;
use super::super::GenericMonoClosureError;
use super::ProgramRealizationError;

pub(super) fn map_arc_batch_error(error: ArcBatchPreparationError) -> ProgramRealizationError {
    match error {
        ArcBatchPreparationError::DuplicateParent { parent } => {
            ProgramRealizationError::DuplicateArcParent { parent }
        }
        ArcBatchPreparationError::DuplicateBody {
            body,
            first_parent,
            second_parent,
        } => ProgramRealizationError::DuplicateArcBody {
            body,
            first_parent,
            second_parent,
        },
        ArcBatchPreparationError::LambdaSpecialization { count, errors } => {
            ProgramRealizationError::LambdaSpecialization { count, errors }
        }
        ArcBatchPreparationError::OperatorCallResolution { count, errors } => {
            ProgramRealizationError::OperatorCallResolution { count, errors }
        }
        ArcBatchPreparationError::SelectedMethodProducerResolution { count, errors } => {
            ProgramRealizationError::SelectedMethodProducerResolution { count, errors }
        }
    }
}

pub(super) fn arc_lowering_error(problems: Vec<ori_arc::ArcProblem>) -> ProgramRealizationError {
    ProgramRealizationError::ArcLowering {
        count: problems.len(),
        problems,
    }
}

pub(super) fn map_generic_mono_closure_error(
    error: GenericMonoClosureError,
) -> ProgramRealizationError {
    match error {
        GenericMonoClosureError::LambdaSpecialization { count, errors } => {
            ProgramRealizationError::LambdaSpecialization { count, errors }
        }
        GenericMonoClosureError::ArcLowering { count, problems } => {
            ProgramRealizationError::ArcLowering { count, problems }
        }
        other @ (GenericMonoClosureError::MonoInventory { .. }
        | GenericMonoClosureError::NonConverging { .. }
        | GenericMonoClosureError::MissingImportedBody { .. }) => {
            ProgramRealizationError::GenericMonoClosure {
                message: other.to_string(),
            }
        }
    }
}
