//! Pure construction of backend-neutral executable-program inputs.

mod arc_batch;
mod callable_census;
mod derived_mono_closure;
mod generic_mono_closure;
mod generic_mono_discovery;
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
pub(crate) use callable_census::{CallableCensusBuilder, CallableCensusError};
pub(crate) use derived_mono_closure::{
    lower_mono_functions_for_analysis, lower_new_mono_functions_for_analysis,
};
pub(crate) use generic_mono_closure::{
    close_generic_mono_targets, generic_type_param_map, GenericMonoClosureError,
    GenericMonoClosureInput, ImportedGenericTemplate,
};
pub(crate) use mono_inventory::{MonoFunctionInventory, MonoFunctionInventoryError};

pub(crate) use repr::{
    compute_module_repr_plan, extend_mono_method_targets, lower_impl_methods_for_analysis,
    lower_non_generic_derived_methods_for_analysis, method_receiver_key, DerivedMethodAnalysis,
    ImplMethodAnalysis, ModuleReprInput,
};
