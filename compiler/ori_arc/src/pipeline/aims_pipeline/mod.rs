//! AIMS realization pipeline.

mod batch;
mod burden_emission;
mod execution;
mod metadata;
mod postprocess;
mod trmc;

pub(crate) use batch::{
    run_aims_pipeline_all_with_external_contracts, run_aims_pipeline_all_with_observer,
};
pub(super) use execution::trace_pipeline_checkpoint;
pub use execution::CheckpointObserver;
pub(crate) use execution::{run_aims_pipeline, AimsPipelineConfig};
pub(super) use metadata::{representation_metadata_errors, validate_variable_metadata};
