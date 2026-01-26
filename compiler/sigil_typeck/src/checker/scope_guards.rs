//! RAII-style scope guards for TypeChecker context management.
//!
//! These helpers ensure context (capabilities, impl Self type) is properly
//! restored even on early returns, preventing bugs from forgotten restores.
//!
//! Note: The impl blocks on TypeChecker are defined in the main checker module.

use std::collections::HashSet;

use sigil_ir::Name;
use sigil_types::Type;

/// Saved capability context for restoration.
pub struct SavedCapabilityContext {
    /// The old capabilities to restore.
    pub old_caps: HashSet<Name>,
    /// The old provided capabilities to restore.
    pub old_provided: HashSet<Name>,
}

/// Saved impl context for restoration.
pub struct SavedImplContext {
    /// The previous Self type to restore.
    pub prev_self: Option<Type>,
}
