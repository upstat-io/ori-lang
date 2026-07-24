//! Ori compiler CLI handlers and shared frontend-reporting utilities.

#[cfg(feature = "llvm")]
mod backend;
pub mod build;
pub mod build_options;
mod check;
#[cfg(feature = "llvm")]
mod codegen_pipeline;
#[cfg(feature = "llvm")]
mod compile_common;
mod debug;
mod demangle;
mod emit_aims_state;
mod emit_scip;
mod explain;
mod explain_idx;
mod fmt;
mod frontend;
mod provenance;
mod run;
mod target;
mod targets;
mod test;
mod watch;

pub use build::build_file;
pub use build_options::{
    accumulate_build_options, accumulate_build_options_with_env, parse_build_options, BuildOptions,
    DebugLevel, EmitType, LinkMode, LtoMode, OptLevel,
};
pub use check::check_file;
pub use debug::{lex_file, parse_file};
pub use demangle::demangle_symbol;
pub use emit_aims_state::emit_aims_state_file;
pub use emit_scip::emit_scip_file;
pub use explain::explain_error;
pub use explain_idx::explain_idx;
pub use fmt::run_format;
pub use frontend::TestEnforcement;
pub use run::{run_file, run_file_compiled};
pub use target::{add_target, list_installed_targets, remove_target, TargetSubcommand};
pub use targets::{list_targets, TargetFilter};
pub use test::run_tests;
pub use watch::watch_file;

pub(super) use frontend::{
    emit_const_eval_problems, print_check_success, read_file, report_frontend_errors,
    run_post_frontend_checks,
};

#[cfg(feature = "llvm")]
pub(crate) use codegen_pipeline::imported_mono::{
    build_imported_mono_functions as build_imported_mono_functions_for_test_runner,
    collect_imported_impl_templates,
    register_prelude_generic_sigs as register_prelude_generic_sigs_for_test_runner,
    ImportedImplTemplate, ImportedImplTemplateSource, ImportedMonoBody, ImportedMonoFn,
    ImportedPreludeSource, ImportedSurfaces, PoolReinternState,
};
