//! Module loading and import resolution.
//!
//! This module handles:
//! - Loading modules and registering their functions
//! - Resolving import paths (relative and module paths)
//! - Parsing and validating imported files
//! - Managing module function captures
//!
//! All file access goes through `db.load_file()` for proper Salsa tracking.

pub mod import;

pub use import::{resolve_import, ResolvedImport, ImportError};
