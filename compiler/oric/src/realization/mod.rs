//! Pure construction of backend-neutral executable-program inputs.

mod arc_batch;
mod mono_inventory;
mod program;
mod repr;

pub use program::{realize_local_program, ProgramRealizationError, ProgramRealizationInput};

pub(crate) use program::{
    collect_user_drop_bindings, realize_arc_program, ArcProgramRealizationInput,
};

pub(crate) use arc_batch::{
    ArcBatchPreparationError, ArcFunctionGroup, LoweredArcBatch, PreparedArcBatch,
};
pub(crate) use mono_inventory::{MonoFunctionInventory, MonoFunctionInventoryError};

pub(crate) use repr::{
    compute_module_repr_plan, lower_impl_methods_for_analysis, lower_mono_function_for_analysis,
    lower_non_generic_derived_methods_for_analysis, DerivedMethodAnalysis, ImplMethodAnalysis,
};
