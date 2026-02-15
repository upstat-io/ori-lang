//! Comment Formatting
//!
//! Handles comment preservation, association, and formatting during module formatting.
//!
//! # Comment Association
//!
//! Comments are associated with AST nodes based on source position. A comment
//! "belongs to" the AST node that immediately follows it.
//!
//! # Doc Comment Reordering
//!
//! Doc comments are reordered to match the canonical order:
//! 1. `// #Description` - Description (may span multiple lines)
//! 2. `// * name: desc` - Member docs (params/fields, in signature/struct order)
//! 3. `// !Warning` - Warnings and panics
//! 4. `// >example` - Examples
//!
//! Legacy `@param`/`@field` markers are also supported at the same sort order.
//!
//! Regular comments (`//`) are not reordered.

use ori_ir::{Comment, CommentKind, CommentList, Span, StringLookup};
use std::collections::{BTreeMap, HashMap};

/// Index of comments by position for efficient lookup.
///
/// Comments are indexed by the start position of the AST node they precede.
/// Multiple comments may be associated with a single position (forming a comment block).
pub struct CommentIndex {
    /// Map from position to comments that end before that position.
    /// Position is the start of the next non-comment token.
    comments_by_position: BTreeMap<u32, Vec<CommentRef>>,
    /// Track which comments have been consumed.
    consumed: Vec<bool>,
}

/// Reference to a comment with its kind for sorting.
#[derive(Clone, Debug)]
struct CommentRef {
    /// Index in the original comment list.
    index: usize,
    /// Kind of comment for sorting.
    kind: CommentKind,
}

impl CommentIndex {
    /// Build a comment index from a comment list and token positions.
    ///
    /// Associates each comment with the position of the token that follows it.
    pub fn new(comments: &CommentList, token_positions: &[u32]) -> Self {
        let mut comments_by_position: BTreeMap<u32, Vec<CommentRef>> = BTreeMap::new();
        let consumed = vec![false; comments.len()];

        for (index, comment) in comments.iter().enumerate() {
            // Find the first token position after this comment ends using binary search
            // (token_positions is sorted, so partition_point is O(log n) vs O(n) for find)
            let idx = token_positions.partition_point(|&pos| pos <= comment.span.end);
            let following_pos = token_positions.get(idx).copied().unwrap_or(u32::MAX);

            comments_by_position
                .entry(following_pos)
                .or_default()
                .push(CommentRef {
                    index,
                    kind: comment.kind,
                });
        }

        Self {
            comments_by_position,
            consumed,
        }
    }

    /// Get comments that should appear before a given position.
    ///
    /// Returns comments in the correct order (doc comments reordered, regular preserved).
    /// Marks the comments as consumed so they won't be returned again.
    ///
    /// Note: This only takes comments that are directly associated with `pos`, not all
    /// comments before it. This is important because the formatter may process items
    /// out of source order (e.g., all functions before all tests), and we don't want
    /// to steal comments that belong to items that appear earlier in source.
    pub fn take_comments_before(&mut self, pos: u32) -> Vec<usize> {
        let mut result = Vec::new();

        // Only take comments associated with this exact position
        if let Some(refs) = self.comments_by_position.remove(&pos) {
            // Sort doc comments, preserve regular comment order
            let mut sorted = sort_comments_by_kind(refs);
            for comment_ref in sorted.drain(..) {
                if !self.consumed[comment_ref.index] {
                    self.consumed[comment_ref.index] = true;
                    result.push(comment_ref.index);
                }
            }
        }

        result
    }

    /// Check if there are any remaining unconsumed comments.
    pub fn has_remaining(&self) -> bool {
        self.consumed.iter().any(|&c| !c)
    }

    /// Get all remaining unconsumed comment indices.
    pub fn remaining_indices(&self) -> Vec<usize> {
        self.consumed
            .iter()
            .enumerate()
            .filter(|(_, &consumed)| !consumed)
            .map(|(i, _)| i)
            .collect()
    }

    /// Get comments that should appear before a function, with @param reordering.
    ///
    /// Like `take_comments_before`, but additionally reorders `@param` doc comments
    /// to match the order of parameters in the function signature.
    ///
    /// Note: Only takes comments associated with the exact position `pos`.
    pub fn take_comments_before_function<I: StringLookup>(
        &mut self,
        pos: u32,
        param_names: &[&str],
        comments: &CommentList,
        interner: &I,
    ) -> Vec<usize> {
        let mut result = Vec::new();

        // Only take comments associated with this exact position
        if let Some(refs) = self.comments_by_position.remove(&pos) {
            // Sort doc comments by kind
            let sorted = sort_comments_by_kind(refs);

            // Separate param comments from others for reordering
            let mut param_indices = Vec::new();
            let mut other_indices = Vec::new();

            for comment_ref in sorted {
                if self.consumed[comment_ref.index] {
                    continue;
                }
                self.consumed[comment_ref.index] = true;

                if comment_ref.kind == CommentKind::DocMember {
                    param_indices.push(comment_ref.index);
                } else {
                    other_indices.push((comment_ref.kind.sort_order(), comment_ref.index));
                }
            }

            // Reorder member comments by function signature order
            let reordered_params =
                reorder_param_comments(&param_indices, comments, param_names, interner);

            // Merge: collect by sort order, insert members at their position (sort_order=1)
            let mut all_by_order: Vec<(u8, usize)> = other_indices;
            for idx in reordered_params {
                all_by_order.push((1, idx)); // DocMember has sort_order 1
            }
            all_by_order.sort_by_key(|(order, _)| *order);

            for (_, idx) in all_by_order {
                result.push(idx);
            }
        }

        result
    }

    /// Get comments that should appear before a type, with @field reordering.
    ///
    /// Like `take_comments_before`, but additionally reorders `@field` doc comments
    /// to match the order of fields in the struct definition.
    ///
    /// Note: Only takes comments associated with the exact position `pos`.
    pub fn take_comments_before_type<I: StringLookup>(
        &mut self,
        pos: u32,
        field_names: &[&str],
        comments: &CommentList,
        interner: &I,
    ) -> Vec<usize> {
        let mut result = Vec::new();

        // Only take comments associated with this exact position
        if let Some(refs) = self.comments_by_position.remove(&pos) {
            // Sort doc comments by kind
            let sorted = sort_comments_by_kind(refs);

            // Separate field comments from others for reordering
            let mut field_indices = Vec::new();
            let mut other_indices = Vec::new();

            for comment_ref in sorted {
                if self.consumed[comment_ref.index] {
                    continue;
                }
                self.consumed[comment_ref.index] = true;

                if comment_ref.kind == CommentKind::DocMember {
                    field_indices.push(comment_ref.index);
                } else {
                    other_indices.push((comment_ref.kind.sort_order(), comment_ref.index));
                }
            }

            // Reorder member comments by struct field order
            let reordered_fields =
                reorder_field_comments(&field_indices, comments, field_names, interner);

            // Merge: collect by sort order, insert members at their position (sort_order=1)
            let mut all_by_order: Vec<(u8, usize)> = other_indices;
            for idx in reordered_fields {
                all_by_order.push((1, idx)); // DocMember has sort_order 1
            }
            all_by_order.sort_by_key(|(order, _)| *order);

            for (_, idx) in all_by_order {
                result.push(idx);
            }
        }

        result
    }
}

/// Sort comments by kind while preserving relative order within kinds.
///
/// Doc comments are sorted: Description -> Param/Field -> Warning -> Example
/// Regular comments keep their original position.
fn sort_comments_by_kind(mut refs: Vec<CommentRef>) -> Vec<CommentRef> {
    // If all regular comments, preserve order
    if refs.iter().all(|r| r.kind == CommentKind::Regular) {
        return refs;
    }

    // If any doc comments, sort by kind while preserving relative order within kinds
    refs.sort_by_key(|r| r.kind.sort_order());
    refs
}

/// Format a comment for output.
///
/// Ensures proper formatting:
/// - Space after `//`
/// - Doc comment markers normalized
pub fn format_comment<I: StringLookup>(comment: &Comment, interner: &I) -> String {
    let content = interner.lookup(comment.content);
    format!("//{content}")
}

/// Reorder member doc comments to match function parameter order.
///
/// Takes a list of member comment indices and the parameter names in order.
/// Returns the indices reordered to match the parameter order.
/// Handles both `* name:` and legacy `@param name` content formats.
pub fn reorder_param_comments<I: StringLookup>(
    param_indices: &[usize],
    comments: &CommentList,
    param_names: &[&str],
    interner: &I,
) -> Vec<usize> {
    reorder_member_comments(param_indices, comments, param_names, interner)
}

/// Reorder member doc comments to match struct field order.
///
/// Handles both `* name:` and legacy `@field name` content formats.
pub fn reorder_field_comments<I: StringLookup>(
    field_indices: &[usize],
    comments: &CommentList,
    field_names: &[&str],
    interner: &I,
) -> Vec<usize> {
    reorder_member_comments(field_indices, comments, field_names, interner)
}

/// Reorder member doc comments to match a given name order.
///
/// Extracts the name from each comment using `extract_member_name_any`,
/// which handles `* name:`, `@param name`, and `@field name` formats.
fn reorder_member_comments<I: StringLookup>(
    indices: &[usize],
    comments: &CommentList,
    names: &[&str],
    interner: &I,
) -> Vec<usize> {
    if indices.is_empty() || names.is_empty() {
        return indices.to_vec();
    }

    // Build HashMap for O(1) lookup instead of O(n) linear scan per comment
    let name_to_order: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, &name)| (name, i))
        .collect();

    let mut ordered: Vec<(Option<usize>, usize)> = indices
        .iter()
        .map(|&idx| {
            let content = interner.lookup(comments[idx].content);
            let name = extract_member_name_any(content);
            let order = name_to_order.get(name).copied();
            (order, idx)
        })
        .collect();

    // Sort by name order (None = unknown names go at end)
    ordered.sort_by_key(|(order, _)| order.unwrap_or(usize::MAX));

    ordered.into_iter().map(|(_, idx)| idx).collect()
}

/// Extract the member name from any doc member comment format.
///
/// Handles:
/// - `* name: description` (new format)
/// - `@param name description` (legacy)
/// - `@field name description` (legacy)
fn extract_member_name_any(content: &str) -> &str {
    let trimmed = content.trim_start();

    // Try `* name:` format first
    if let Some(rest) = trimmed.strip_prefix('*') {
        let rest = rest.trim_start();
        if let Some(colon_pos) = rest.find(':') {
            let name = rest[..colon_pos].trim();
            if !name.is_empty() {
                return name;
            }
        }
    }

    // Try `@param name` format
    if let Some(rest) = trimmed.strip_prefix("@param") {
        let rest = rest.trim_start();
        return rest.split_whitespace().next().unwrap_or("");
    }

    // Try `@field name` format
    if let Some(rest) = trimmed.strip_prefix("@field") {
        let rest = rest.trim_start();
        return rest.split_whitespace().next().unwrap_or("");
    }

    ""
}

/// Group consecutive comments by their kind for reordering.
///
/// Returns groups of comment indices where each group can be reordered together.
pub fn group_comments_for_reordering(
    comment_indices: &[usize],
    comments: &CommentList,
) -> Vec<Vec<usize>> {
    if comment_indices.is_empty() {
        return vec![];
    }

    // All doc comments in a block are reordered together
    // Regular comments form their own groups
    let mut groups = Vec::new();
    let mut current_group = Vec::new();
    let mut current_is_doc = false;

    for &idx in comment_indices {
        let is_doc = comments[idx].kind.is_doc();

        if current_group.is_empty() {
            current_is_doc = is_doc;
            current_group.push(idx);
        } else if is_doc == current_is_doc {
            current_group.push(idx);
        } else {
            groups.push(current_group);
            current_group = vec![idx];
            current_is_doc = is_doc;
        }
    }

    if !current_group.is_empty() {
        groups.push(current_group);
    }

    groups
}

/// Collect all token positions from a span for comment indexing.
///
/// This is a helper for building the token position list needed by [`CommentIndex`].
pub fn collect_item_positions(spans: impl IntoIterator<Item = Span>) -> Vec<u32> {
    let mut positions: Vec<u32> = spans.into_iter().map(|s| s.start).collect();
    positions.sort_unstable();
    positions.dedup();
    positions
}

#[cfg(test)]
mod tests;
