//! Pure construction of backend-neutral executable-program inputs.

mod program;
mod repr;

pub use program::{realize_local_program, ProgramRealizationError, ProgramRealizationInput};

pub(crate) use repr::{
    collect_all_arc_functions, compute_module_repr_plan, lower_impl_methods_for_analysis,
    ImplMethodAnalysis,
};
