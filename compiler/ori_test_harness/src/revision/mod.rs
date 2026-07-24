//! Revision expansion system.
//!
//! Extracts revision names and per-revision compile-flags from directives.
//! Does NOT translate revision names into compiler flags — that is the
//! consumer's responsibility inside `TestStrategy::execute()`.

use crate::directive::{Directive, DirectiveLine};

/// A single test revision extracted from directives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionConfig {
    /// Revision name (e.g., "debug", "release", "no-repr-opt").
    /// Empty string for the default (no-revision) config.
    pub name: String,
    /// Explicit compile flags from `// @[name] compile-flags:` directives.
    pub compile_flags: Vec<String>,
}

/// Expand revisions from parsed directives.
///
/// - No `// @revisions:` → single default revision (empty name, no flags)
/// - `// @revisions: a b c` → one config per name, with revision-gated
///   compile-flags applied to the matching revision
pub fn expand_revisions(directives: &[DirectiveLine]) -> Vec<RevisionConfig> {
    // Find the revisions directive (if any)
    let revision_names: Option<&[String]> = directives.iter().find_map(|d| {
        if let Directive::Revisions { names } = &d.directive {
            Some(names.as_slice())
        } else {
            None
        }
    });

    match revision_names {
        None => {
            // No revisions directive — single default config
            vec![RevisionConfig {
                name: String::new(),
                compile_flags: collect_flags_for_revision(directives, ""),
            }]
        }
        Some(names) => names
            .iter()
            .map(|name| RevisionConfig {
                name: name.clone(),
                compile_flags: collect_flags_for_revision(directives, name),
            })
            .collect(),
    }
}

/// Collect compile-flags directives that apply to a specific revision.
fn collect_flags_for_revision(directives: &[DirectiveLine], revision: &str) -> Vec<String> {
    directives
        .iter()
        .filter(|d| matches!(&d.directive, Directive::CompileFlags { .. }))
        .filter(|d| {
            // Ungated or matching revision
            d.revision.is_none() || d.revision.as_deref() == Some(revision)
        })
        .flat_map(|d| {
            if let Directive::CompileFlags { flags } = &d.directive {
                flags.clone()
            } else {
                vec![]
            }
        })
        .collect()
}

/// Filter directives for a specific active revision.
///
/// Returns directives that are either ungated (no revision) or gated
/// to the active revision name. Used by the runner to pass only
/// relevant directives to `TestStrategy::execute()` and `verify()`.
pub fn filter_directives_for_revision<'a>(
    directives: &'a [DirectiveLine],
    revision: &str,
) -> Vec<&'a DirectiveLine> {
    directives
        .iter()
        .filter(|d| d.revision.is_none() || d.revision.as_deref() == Some(revision))
        .collect()
}

#[cfg(test)]
mod tests;
