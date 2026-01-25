//! Import Types
//!
//! Use/import statements and related types.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, PartialEq, Hash, Debug for Salsa requirements.

use crate::{Name, Span};

/// A use/import statement.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct UseDef {
    /// Import path - either relative ('./math', '../utils') or module (std.math)
    pub path: ImportPath,
    /// Items being imported
    pub items: Vec<UseItem>,
    /// Source span
    pub span: Span,
}

/// Import path type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ImportPath {
    /// Relative path: './math', '../utils/helpers'
    Relative(Name),
    /// Module path: std.math, std.collections
    Module(Vec<Name>),
}

/// A single imported item.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct UseItem {
    /// Name of the item being imported
    pub name: Name,
    /// Optional alias: `name as alias`
    pub alias: Option<Name>,
    /// Whether this is a private import (::name)
    pub is_private: bool,
}
