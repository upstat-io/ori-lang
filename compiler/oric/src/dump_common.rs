//! Shared rendering primitives for the phase-dump drivers.
//!
//! The `ast_dump` (PARSE) and `ir_dump` (TYPECK) drivers render the same
//! module-item headers — import paths and impl blocks — to the same stderr
//! format. These helpers are the single home for that rendering so the two
//! drivers cannot drift.

use ori_ir::ast::{ImplDef, ImportPath};
use ori_ir::{Name, StringInterner};

/// Join interned names with `.` (the dotted-path form used in dump output).
pub(crate) fn join_names(parts: &[Name], interner: &StringInterner) -> String {
    parts
        .iter()
        .map(|n| interner.lookup(*n))
        .collect::<Vec<_>>()
        .join(".")
}

/// Render an import path for a dump line: a relative path is its bare name; a
/// module path is its segments joined with `.`.
pub(crate) fn format_import_path(path: &ImportPath, interner: &StringInterner) -> String {
    match path {
        ImportPath::Relative(name) => interner.lookup(*name).to_string(),
        ImportPath::Module(parts) => join_names(parts, interner),
    }
}

/// Render the `{trait} for {self}` header of an impl block: the trait path
/// (or `(inherent)` when absent) joined with `.`, then the self path.
pub(crate) fn format_impl_header(imp: &ImplDef, interner: &StringInterner) -> String {
    let trait_str = imp.trait_path.as_ref().map_or_else(
        || "(inherent)".to_string(),
        |path| join_names(path, interner),
    );
    format!("{trait_str} for {}", join_names(&imp.self_path, interner))
}
