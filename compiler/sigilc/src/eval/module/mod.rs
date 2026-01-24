//! Module loading and import resolution.
//!
//! This module handles:
//! - Loading modules and registering their functions
//! - Resolving import paths (relative and module paths)
//! - Parsing and validating imported files
//! - Managing module function captures

pub mod import;

pub use import::{resolve_import_path, load_imported_module, ImportError};
