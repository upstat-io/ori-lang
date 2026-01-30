//! Incremental Formatting
//!
//! Format only the declarations that overlap with a changed region,
//! rather than reformatting the entire file.
//!
//! # Use Cases
//!
//! - LSP format-on-type: format only the declaration being edited
//! - Large files: format only changed declarations after an edit
//!
//! # Limitations
//!
//! - Minimum unit is a complete top-level declaration
//! - Changes that affect multiple declarations format all affected ones
//! - Import/constant blocks are formatted as a unit

use crate::comments::CommentIndex;
use crate::declarations::ModuleFormatter;
use ori_ir::ast::items::Module;
use ori_ir::{CommentList, ExprArena, StringLookup};

/// A region of formatted text with its original position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedRegion {
    /// Original byte range start in the source
    pub original_start: usize,
    /// Original byte range end in the source
    pub original_end: usize,
    /// Formatted text for this region
    pub formatted: String,
}

/// Result of incremental formatting.
#[derive(Debug)]
pub enum IncrementalResult {
    /// Successfully formatted specific regions
    Regions(Vec<FormattedRegion>),
    /// Full format needed (e.g., change spans entire file or affects imports/configs)
    FullFormatNeeded,
    /// No formatting needed (change is in whitespace/comments between declarations)
    NoChangeNeeded,
}

/// A declaration with its span and kind for overlap detection.
#[derive(Debug, Clone, Copy)]
struct DeclInfo {
    start: u32,
    end: u32,
    kind: DeclKind,
    index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    Import,
    Config,
    Type,
    Trait,
    Impl,
    Function,
    Test,
}

/// Collect all declarations with their spans.
fn collect_declarations(module: &Module) -> Vec<DeclInfo> {
    let mut decls = Vec::new();

    for (i, import) in module.imports.iter().enumerate() {
        decls.push(DeclInfo {
            start: import.span.start,
            end: import.span.end,
            kind: DeclKind::Import,
            index: i,
        });
    }

    for (i, config) in module.configs.iter().enumerate() {
        decls.push(DeclInfo {
            start: config.span.start,
            end: config.span.end,
            kind: DeclKind::Config,
            index: i,
        });
    }

    for (i, type_decl) in module.types.iter().enumerate() {
        decls.push(DeclInfo {
            start: type_decl.span.start,
            end: type_decl.span.end,
            kind: DeclKind::Type,
            index: i,
        });
    }

    for (i, trait_def) in module.traits.iter().enumerate() {
        decls.push(DeclInfo {
            start: trait_def.span.start,
            end: trait_def.span.end,
            kind: DeclKind::Trait,
            index: i,
        });
    }

    for (i, impl_def) in module.impls.iter().enumerate() {
        decls.push(DeclInfo {
            start: impl_def.span.start,
            end: impl_def.span.end,
            kind: DeclKind::Impl,
            index: i,
        });
    }

    for (i, func) in module.functions.iter().enumerate() {
        decls.push(DeclInfo {
            start: func.span.start,
            end: func.span.end,
            kind: DeclKind::Function,
            index: i,
        });
    }

    for (i, test) in module.tests.iter().enumerate() {
        decls.push(DeclInfo {
            start: test.span.start,
            end: test.span.end,
            kind: DeclKind::Test,
            index: i,
        });
    }

    // Sort by start position
    decls.sort_by_key(|d| d.start);
    decls
}

/// Check if two ranges overlap.
fn ranges_overlap(a_start: u32, a_end: u32, b_start: u32, b_end: u32) -> bool {
    a_start < b_end && b_start < a_end
}

/// Find declarations that overlap with a given byte range.
fn find_overlapping_declarations(
    decls: &[DeclInfo],
    change_start: u32,
    change_end: u32,
) -> Vec<DeclInfo> {
    decls
        .iter()
        .filter(|d| ranges_overlap(d.start, d.end, change_start, change_end))
        .copied()
        .collect()
}

/// Incrementally format declarations that overlap with the changed region.
///
/// # Arguments
///
/// * `module` - The parsed module
/// * `comments` - Comments from the source
/// * `arena` - Expression arena
/// * `interner` - String interner
/// * `change_start` - Start byte offset of the changed region
/// * `change_end` - End byte offset of the changed region
///
/// # Returns
///
/// * `IncrementalResult::Regions` - Formatted regions to replace
/// * `IncrementalResult::FullFormatNeeded` - Full format required
/// * `IncrementalResult::NoChangeNeeded` - No formatting changes needed
pub fn format_incremental<I: StringLookup>(
    module: &Module,
    comments: &CommentList,
    arena: &ExprArena,
    interner: &I,
    change_start: usize,
    change_end: usize,
) -> IncrementalResult {
    let decls = collect_declarations(module);

    if decls.is_empty() {
        return IncrementalResult::NoChangeNeeded;
    }

    let change_start_u32 = change_start as u32;
    let change_end_u32 = change_end as u32;

    // Find overlapping declarations
    let overlapping = find_overlapping_declarations(&decls, change_start_u32, change_end_u32);

    if overlapping.is_empty() {
        // Change is between declarations (whitespace/comments only)
        return IncrementalResult::NoChangeNeeded;
    }

    // If change overlaps with imports or configs, we need to format all of them as a block
    let has_import = overlapping.iter().any(|d| d.kind == DeclKind::Import);
    let has_config = overlapping.iter().any(|d| d.kind == DeclKind::Config);

    // For simplicity, if the change affects imports or configs, do full format
    // (these are block-formatted and order matters)
    if has_import || has_config {
        return IncrementalResult::FullFormatNeeded;
    }

    // Format each overlapping declaration individually
    let mut regions = Vec::new();
    let positions: Vec<u32> = decls.iter().map(|d| d.start).collect();

    for decl in &overlapping {
        let mut comment_index = CommentIndex::new(comments, &positions);

        let formatted =
            format_single_declaration(module, decl, comments, &mut comment_index, arena, interner);

        // Expand the original range to include preceding comments
        let preceding_comment_start = find_preceding_comment_start(comments, decl.start);
        let actual_start = preceding_comment_start.unwrap_or(decl.start);

        regions.push(FormattedRegion {
            original_start: actual_start as usize,
            original_end: decl.end as usize,
            formatted,
        });
    }

    if regions.is_empty() {
        IncrementalResult::NoChangeNeeded
    } else {
        IncrementalResult::Regions(regions)
    }
}

/// Find the start of comments that precede a declaration.
fn find_preceding_comment_start(comments: &CommentList, decl_start: u32) -> Option<u32> {
    let mut earliest = None;

    for comment in comments.iter() {
        // Comment is before the declaration
        if comment.span.end <= decl_start {
            // Check if it's close enough (within reasonable distance)
            // Comments right before a declaration belong to it
            let gap = decl_start - comment.span.end;
            if gap < 100 {
                // Reasonable gap for preceding comments
                match earliest {
                    None => earliest = Some(comment.span.start),
                    Some(e) if comment.span.start < e => earliest = Some(comment.span.start),
                    _ => {}
                }
            }
        }
    }

    earliest
}

/// Format a single declaration.
fn format_single_declaration<I: StringLookup>(
    module: &Module,
    decl: &DeclInfo,
    comments: &CommentList,
    comment_index: &mut CommentIndex,
    arena: &ExprArena,
    interner: &I,
) -> String {
    let mut formatter = ModuleFormatter::new(arena, interner);

    match decl.kind {
        DeclKind::Type => {
            let type_decl = &module.types[decl.index];
            formatter.emit_comments_before_type(type_decl, comments, comment_index);
            formatter.format_type_decl(type_decl);
        }
        DeclKind::Trait => {
            let trait_def = &module.traits[decl.index];
            formatter.emit_comments_before(trait_def.span.start, comments, comment_index);
            formatter.format_trait(trait_def);
        }
        DeclKind::Impl => {
            let impl_def = &module.impls[decl.index];
            formatter.emit_comments_before(impl_def.span.start, comments, comment_index);
            formatter.format_impl(impl_def);
        }
        DeclKind::Function => {
            let func = &module.functions[decl.index];
            formatter.emit_comments_before_function(func, comments, comment_index);
            formatter.format_function(func);
        }
        DeclKind::Test => {
            let test = &module.tests[decl.index];
            formatter.emit_comments_before(test.span.start, comments, comment_index);
            formatter.format_test(test);
        }
        // Import and Config are handled as blocks, not individually
        DeclKind::Import | DeclKind::Config => {
            unreachable!("Import and Config should trigger full format")
        }
    }

    formatter.finish()
}

/// Apply incremental formatting results to source text.
///
/// Regions must be non-overlapping. They are applied from end to start
/// to preserve earlier byte offsets.
pub fn apply_regions(source: &str, mut regions: Vec<FormattedRegion>) -> String {
    if regions.is_empty() {
        return source.to_string();
    }

    // Sort by start position (descending) to apply from end to start
    // This preserves earlier byte offsets as we replace
    regions.sort_by(|a, b| b.original_start.cmp(&a.original_start));

    let mut result = source.to_string();

    for region in regions {
        let start = region.original_start;
        let end = region.original_end.min(result.len());

        if start <= end && start <= result.len() {
            result.replace_range(start..end, &region.formatted);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ranges_overlap() {
        // Overlapping
        assert!(ranges_overlap(0, 10, 5, 15));
        assert!(ranges_overlap(5, 15, 0, 10));
        assert!(ranges_overlap(0, 10, 0, 10)); // Same range

        // Contained
        assert!(ranges_overlap(0, 20, 5, 15));
        assert!(ranges_overlap(5, 15, 0, 20));

        // Not overlapping
        assert!(!ranges_overlap(0, 10, 10, 20)); // Adjacent
        assert!(!ranges_overlap(0, 10, 15, 20)); // Gap

        // Edge cases
        assert!(!ranges_overlap(0, 0, 0, 0)); // Empty ranges
        assert!(!ranges_overlap(5, 5, 5, 5)); // Empty ranges at same point
    }

    #[test]
    fn test_apply_regions_single() {
        let source = "hello world";
        let regions = vec![FormattedRegion {
            original_start: 0,
            original_end: 5,
            formatted: "goodbye".to_string(),
        }];

        assert_eq!(apply_regions(source, regions), "goodbye world");
    }

    #[test]
    fn test_apply_regions_multiple() {
        let source = "aaa bbb ccc";
        let regions = vec![
            FormattedRegion {
                original_start: 0,
                original_end: 3,
                formatted: "XXX".to_string(),
            },
            FormattedRegion {
                original_start: 8,
                original_end: 11,
                formatted: "ZZZ".to_string(),
            },
        ];

        assert_eq!(apply_regions(source, regions), "XXX bbb ZZZ");
    }

    #[test]
    fn test_apply_regions_empty() {
        let source = "hello world";
        let regions = vec![];
        assert_eq!(apply_regions(source, regions), "hello world");
    }

    #[test]
    fn test_apply_regions_size_change() {
        let source = "short text";
        let regions = vec![FormattedRegion {
            original_start: 0,
            original_end: 5,
            formatted: "very long replacement".to_string(),
        }];

        assert_eq!(apply_regions(source, regions), "very long replacement text");
    }
}
