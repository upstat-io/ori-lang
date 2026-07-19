//! Implementation block registration (Pass 0c, part 2).
//!
//! Registers inherent, trait, and imported impls while keeping method building,
//! default inheritance, validation, and builtin extensions in focused modules.

mod defaults;
mod extensions;
mod imported;
mod local;
mod methods;

pub use extensions::register_extensions;
pub(crate) use extensions::{extension_method_has_self, extension_type_params};
pub(crate) use imported::register_imported_impls;

use crate::check::bodies::allocate_rigid_var_map;
use crate::ModuleChecker;

/// Register implementation blocks.
///
/// For trait impls, also registers unoverridden default methods so they're
/// visible during method resolution in function body checking (Pass 2).
pub fn register_impls(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    for (impl_index, impl_def) in module.impls.iter().enumerate() {
        // Allocate this impl block's `RigidVar` substitution map in Pass 0c,
        // before body checking, so pass-3 monomorphization sees every binder.
        let impl_rigid_var_map = allocate_rigid_var_map(checker, impl_def.generics);
        checker.push_impl_rigid_var_map(impl_rigid_var_map);
        local::register_impl(checker, impl_def, &module.traits, impl_index);
    }
}
