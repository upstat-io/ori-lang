//! Import Types
//!
//! Use/import statements and related types.
//!
//! # Salsa Compatibility
//! All types have Clone, Eq, `PartialEq`, Hash, Debug for Salsa requirements.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::super::Visibility;
use crate::{CfgAttr, Name, Span, TargetAttr};

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
    /// Item-level target attribute: `#target(os: "linux") use "./linux/io" { epoll_create }`
    /// Spec §25.4: Conditional compilation on imports.
    pub target_attr: Option<TargetAttr>,
    /// Item-level cfg attribute: `#cfg(debug) use std.debug { trace }`
    /// Spec §25.4: Conditional compilation on imports.
    pub cfg_attr: Option<CfgAttr>,
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

/// Build the qualified local name for an alias-qualified call (`alias.func`).
///
/// The single canonical definition of the `"alias.func"` naming convention.
/// Every site that must produce the identical interned qualified `Name` calls
/// this: the type checker's alias-call recording (`ori_types`), the
/// import-wiring synthesis (`oric::imports`), and the eval-backend qualified
/// binding (`oric::eval::module::import`). Each caller interns the returned
/// string with its own interner; centralizing the separator + ordering makes
/// the cross-crate agreement structural, not convention-enforced.
#[must_use]
pub fn qualified_alias_name(alias: &str, func: &str) -> String {
    format!("{alias}.{func}")
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

/// Cycle detector for import-graph traversal.
///
/// The single canonical definition, shared by every consumer that walks a
/// module's transitive imports: the AOT multi-file loader (`ori_llvm`) and
/// the typecheck resolver's recursion into cross-module type checking
/// (`oric`). Tracks an ordered in-progress stack (for cycle-path
/// reconstruction) plus an O(1) membership set, keeping the two-set
/// discipline (in-progress vs fully-visited) so a diamond-shaped import
/// graph never false-positives as a cycle.
#[derive(Debug, Default)]
pub struct ImportCycleGuard {
    /// Ordered stack of paths currently being loaded.
    stack: Vec<PathBuf>,
    /// O(1) membership mirror of `stack`.
    in_progress: HashSet<PathBuf>,
    /// Paths that finished loading without participating in a cycle.
    visited: HashSet<PathBuf>,
}

impl ImportCycleGuard {
    /// Create an empty guard.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `path` is on the in-progress stack (loading it now would cycle).
    #[must_use]
    pub fn would_cycle(&self, path: &Path) -> bool {
        self.in_progress.contains(path)
    }

    /// Whether `path` already finished loading in this guard's session.
    #[must_use]
    pub fn is_visited(&self, path: &Path) -> bool {
        self.visited.contains(path)
    }

    /// The cycle path (current stack plus `path`) if loading `path` now
    /// would cycle, else `None`. A pure read — does not mutate the guard.
    #[must_use]
    pub fn cycle_path(&self, path: &Path) -> Option<Vec<PathBuf>> {
        if !self.would_cycle(path) {
            return None;
        }
        let mut cycle: Vec<PathBuf> = self.stack.clone();
        cycle.push(path.to_path_buf());
        Some(cycle)
    }

    /// Push `path` onto the in-progress stack unconditionally.
    ///
    /// For callers that already checked [`Self::would_cycle`] (or whose own
    /// invocation IS the point a cycle would have been rejected at, one
    /// layer up) before deciding to proceed.
    pub fn push(&mut self, path: PathBuf) {
        self.in_progress.insert(path.clone());
        self.stack.push(path);
    }

    /// Push `path` onto the in-progress stack.
    ///
    /// Returns `Err` with the full cycle path (the stack plus `path`) when
    /// `path` would cycle; the caller constructs its own domain error from
    /// it. Returns `Ok` and records `path` as in-progress otherwise.
    pub fn start_loading(&mut self, path: PathBuf) -> Result<(), Vec<PathBuf>> {
        if let Some(cycle) = self.cycle_path(&path) {
            return Err(cycle);
        }
        self.push(path);
        Ok(())
    }

    /// Pop the most recently pushed path off the in-progress stack, marking
    /// `path` visited.
    pub fn finish_loading(&mut self, path: &Path) {
        if let Some(popped) = self.stack.pop() {
            self.in_progress.remove(&popped);
        }
        self.visited.insert(path.to_path_buf());
    }
}
