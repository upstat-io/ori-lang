//! Import Types
//!
//! Use/import statements and related types.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use super::super::Visibility;
use crate::{Name, Span};

/// A use/import statement.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct UseDef {
    /// Import path - either relative ('./math', '../utils') or module (std.math)
    pub path: ImportPath,
    /// Items being imported (empty when using module alias)
    pub items: Vec<UseItem>,
    /// Module alias for qualified access: `use std.net.http as http`
    ///
    /// When set, the entire module is imported under this alias name,
    /// enabling qualified access like `http.get()`. Items list must be empty.
    pub module_alias: Option<Name>,
    /// Visibility of this import.
    ///
    /// When public, imported items are re-exported from this module.
    pub visibility: Visibility,
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
///
/// Represents one entry in `use path { item1, item2, ... }`.
/// Grammar: `import_item = [ "::" ] identifier [ "without" "def" ] [ "as" identifier ] | "$" identifier .`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct UseItem {
    /// Name of the item being imported
    pub name: Name,
    /// Optional alias: `name as alias`
    pub alias: Option<Name>,
    /// Whether this is a private import (`::name`)
    pub is_private: bool,
    /// Whether this imports a trait without its default implementation (`Trait without def`)
    pub without_def: bool,
    /// Whether this is a constant/config import (`$NAME`)
    pub is_constant: bool,
}

/// An extension import statement.
///
/// Syntax: `[pub] extension path { Type.method, Type.method }`
/// Grammar: `extension_import = "extension" import_path "{" extension_item { "," extension_item } "}" .`
///
/// Extension imports bring specific extension methods into scope with
/// method-level granularity. Wildcards are prohibited.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExtensionImport {
    /// Module path containing the extension definitions
    pub path: ImportPath,
    /// Extension methods being imported (`Type.method` pairs)
    pub items: Vec<ExtensionImportItem>,
    /// Visibility (public for re-export)
    pub visibility: Visibility,
    /// Source span
    pub span: Span,
}

/// A single extension import item: `Type.method`.
///
/// Grammar: `extension_item = identifier "." identifier .`
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ExtensionImportItem {
    /// The type being extended (e.g., `Iterator`)
    pub type_name: Name,
    /// The method being imported (e.g., `count`)
    pub method_name: Name,
    /// Source span of this item
    pub span: Span,
}

/// Structured error kind for import resolution failures.
///
/// The canonical definition for import errors, used by both the import
/// resolver (`oric::imports`) and the type checker (`ori_types`). Having
/// a single enum eliminates lossy mapping between duplicate definitions.
///
/// # Salsa Compatibility
/// Derives `Copy, Clone, Eq, PartialEq, Hash, Debug` for Salsa query results.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum ImportErrorKind {
    /// Module file could not be found at any candidate path.
    ModuleNotFound,
    /// Specific item not found in the imported module.
    ItemNotFound,
    /// Attempt to import a private item without `::` prefix.
    PrivateAccess,
    /// Circular import detected during resolution.
    CircularImport,
    /// Empty module path (e.g., `use {} { ... }`).
    EmptyModulePath,
    /// Module alias import combined with individual items.
    ModuleAliasWithItems,
}
