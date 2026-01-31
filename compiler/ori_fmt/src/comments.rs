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
//! 2. `// @param` - Parameter docs (in signature order)
//! 3. `// @field` - Field docs (in struct order)
//! 4. `// !Warning` - Warnings and panics
//! 5. `// >example` - Examples
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
    pub fn take_comments_before(&mut self, pos: u32) -> Vec<usize> {
        // Find all comment groups that end before this position
        let keys_to_take: Vec<u32> = self
            .comments_by_position
            .range(..=pos)
            .map(|(k, _)| *k)
            .collect();

        let mut result = Vec::new();

        for key in keys_to_take {
            if let Some(refs) = self.comments_by_position.remove(&key) {
                // Sort doc comments, preserve regular comment order
                let mut sorted = sort_comments_by_kind(refs);
                for comment_ref in sorted.drain(..) {
                    if !self.consumed[comment_ref.index] {
                        self.consumed[comment_ref.index] = true;
                        result.push(comment_ref.index);
                    }
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
    pub fn take_comments_before_function<I: StringLookup>(
        &mut self,
        pos: u32,
        param_names: &[&str],
        comments: &CommentList,
        interner: &I,
    ) -> Vec<usize> {
        // Find all comment groups that end before this position
        let keys_to_take: Vec<u32> = self
            .comments_by_position
            .range(..=pos)
            .map(|(k, _)| *k)
            .collect();

        let mut result = Vec::new();

        for key in keys_to_take {
            if let Some(refs) = self.comments_by_position.remove(&key) {
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

                    if comment_ref.kind == CommentKind::DocParam {
                        param_indices.push(comment_ref.index);
                    } else {
                        other_indices.push((comment_ref.kind.sort_order(), comment_ref.index));
                    }
                }

                // Reorder param comments by function signature order
                let reordered_params =
                    reorder_param_comments(&param_indices, comments, param_names, interner);

                // Merge: collect by sort order, insert params at their position (sort_order=1)
                let mut all_by_order: Vec<(u8, usize)> = other_indices;
                for idx in reordered_params {
                    all_by_order.push((1, idx)); // DocParam has sort_order 1
                }
                all_by_order.sort_by_key(|(order, _)| *order);

                for (_, idx) in all_by_order {
                    result.push(idx);
                }
            }
        }

        result
    }

    /// Get comments that should appear before a type, with @field reordering.
    ///
    /// Like `take_comments_before`, but additionally reorders `@field` doc comments
    /// to match the order of fields in the struct definition.
    pub fn take_comments_before_type<I: StringLookup>(
        &mut self,
        pos: u32,
        field_names: &[&str],
        comments: &CommentList,
        interner: &I,
    ) -> Vec<usize> {
        // Find all comment groups that end before this position
        let keys_to_take: Vec<u32> = self
            .comments_by_position
            .range(..=pos)
            .map(|(k, _)| *k)
            .collect();

        let mut result = Vec::new();

        for key in keys_to_take {
            if let Some(refs) = self.comments_by_position.remove(&key) {
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

                    if comment_ref.kind == CommentKind::DocField {
                        field_indices.push(comment_ref.index);
                    } else {
                        other_indices.push((comment_ref.kind.sort_order(), comment_ref.index));
                    }
                }

                // Reorder field comments by struct field order
                let reordered_fields =
                    reorder_field_comments(&field_indices, comments, field_names, interner);

                // Merge: collect by sort order, insert fields at their position (sort_order=1)
                let mut all_by_order: Vec<(u8, usize)> = other_indices;
                for idx in reordered_fields {
                    all_by_order.push((1, idx)); // DocField has sort_order 1
                }
                all_by_order.sort_by_key(|(order, _)| *order);

                for (_, idx) in all_by_order {
                    result.push(idx);
                }
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

/// Reorder `@param` doc comments to match function parameter order.
///
/// Takes a list of param comment indices and the parameter names in order.
/// Returns the indices reordered to match the parameter order.
pub fn reorder_param_comments<I: StringLookup>(
    param_indices: &[usize],
    comments: &CommentList,
    param_names: &[&str],
    interner: &I,
) -> Vec<usize> {
    if param_indices.is_empty() || param_names.is_empty() {
        return param_indices.to_vec();
    }

    // Build HashMap for O(1) lookup instead of O(n) linear scan per comment
    let name_to_order: HashMap<&str, usize> = param_names
        .iter()
        .enumerate()
        .map(|(i, &name)| (name, i))
        .collect();

    // Extract param name from each @param comment
    let mut param_to_index: Vec<(Option<usize>, usize)> = param_indices
        .iter()
        .map(|&idx| {
            let content = interner.lookup(comments[idx].content);
            // Format: " @param name description"
            let param_name = extract_param_name(content);
            let order = name_to_order.get(param_name).copied();
            (order, idx)
        })
        .collect();

    // Sort by parameter order (None = unknown params go at end)
    param_to_index.sort_by_key(|(order, _)| order.unwrap_or(usize::MAX));

    param_to_index.into_iter().map(|(_, idx)| idx).collect()
}

/// Reorder `@field` doc comments to match struct field order.
pub fn reorder_field_comments<I: StringLookup>(
    field_indices: &[usize],
    comments: &CommentList,
    field_names: &[&str],
    interner: &I,
) -> Vec<usize> {
    if field_indices.is_empty() || field_names.is_empty() {
        return field_indices.to_vec();
    }

    // Build HashMap for O(1) lookup instead of O(n) linear scan per comment
    let name_to_order: HashMap<&str, usize> = field_names
        .iter()
        .enumerate()
        .map(|(i, &name)| (name, i))
        .collect();

    let mut field_to_index: Vec<(Option<usize>, usize)> = field_indices
        .iter()
        .map(|&idx| {
            let content = interner.lookup(comments[idx].content);
            let field_name = extract_field_name(content);
            let order = name_to_order.get(field_name).copied();
            (order, idx)
        })
        .collect();

    field_to_index.sort_by_key(|(order, _)| order.unwrap_or(usize::MAX));

    field_to_index.into_iter().map(|(_, idx)| idx).collect()
}

/// Extract the parameter name from a @param comment content.
///
/// Input: " @param name description"
/// Output: "name"
fn extract_param_name(content: &str) -> &str {
    let trimmed = content.trim_start();
    if let Some(rest) = trimmed.strip_prefix("@param") {
        let rest = rest.trim_start();
        // Take until whitespace
        rest.split_whitespace().next().unwrap_or("")
    } else {
        ""
    }
}

/// Extract the field name from a @field comment content.
fn extract_field_name(content: &str) -> &str {
    let trimmed = content.trim_start();
    if let Some(rest) = trimmed.strip_prefix("@field") {
        let rest = rest.trim_start();
        rest.split_whitespace().next().unwrap_or("")
    } else {
        ""
    }
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
mod tests {
    use super::*;
    use ori_ir::StringInterner;

    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn test_extract_param_name() {
        assert_eq!(extract_param_name(" @param x The value"), "x");
        assert_eq!(extract_param_name(" @param foo description"), "foo");
        assert_eq!(extract_param_name(" @param "), "");
        assert_eq!(extract_param_name("not a param"), "");
    }

    #[test]
    fn test_extract_field_name() {
        assert_eq!(extract_field_name(" @field x The coordinate"), "x");
        assert_eq!(extract_field_name(" @field name description"), "name");
        assert_eq!(extract_field_name(" @field "), "");
    }

    #[test]
    fn test_format_comment() {
        let interner = test_interner();
        let comment = Comment::regular(interner.intern(" hello world"), Span::new(0, 15));

        let formatted = format_comment(&comment, &interner);
        assert_eq!(formatted, "// hello world");
    }

    #[test]
    fn test_format_comment_doc() {
        let interner = test_interner();
        let comment = Comment::new(
            interner.intern(" #Description"),
            Span::new(0, 15),
            CommentKind::DocDescription,
        );

        let formatted = format_comment(&comment, &interner);
        assert_eq!(formatted, "// #Description");
    }

    #[test]
    fn test_sort_comments_by_kind() {
        // Example before Description - should be reordered
        let refs = vec![
            CommentRef {
                index: 0,
                kind: CommentKind::DocExample,
            },
            CommentRef {
                index: 1,
                kind: CommentKind::DocDescription,
            },
        ];

        let sorted = sort_comments_by_kind(refs);

        // Description (sort_order=0) should come before Example (sort_order=3)
        assert_eq!(sorted[0].index, 1);
        assert_eq!(sorted[1].index, 0);
    }

    #[test]
    fn test_sort_regular_comments_preserved() {
        let refs = vec![
            CommentRef {
                index: 0,
                kind: CommentKind::Regular,
            },
            CommentRef {
                index: 1,
                kind: CommentKind::Regular,
            },
            CommentRef {
                index: 2,
                kind: CommentKind::Regular,
            },
        ];

        let sorted = sort_comments_by_kind(refs);

        // Order should be preserved
        assert_eq!(sorted[0].index, 0);
        assert_eq!(sorted[1].index, 1);
        assert_eq!(sorted[2].index, 2);
    }

    #[test]
    fn test_comment_index_basic() {
        let interner = test_interner();
        let comments = CommentList::from_vec(vec![
            Comment::regular(interner.intern(" first"), Span::new(0, 8)),
            Comment::regular(interner.intern(" second"), Span::new(10, 19)),
        ]);

        // Tokens at positions 9 (after first comment) and 20 (after second)
        let token_positions = vec![9, 20];

        let mut index = CommentIndex::new(&comments, &token_positions);

        // Get comments before position 9
        let before_9 = index.take_comments_before(9);
        assert_eq!(before_9, vec![0]);

        // Get comments before position 20
        let before_20 = index.take_comments_before(20);
        assert_eq!(before_20, vec![1]);
    }

    #[test]
    fn test_reorder_param_comments() {
        let interner = test_interner();
        let comments = CommentList::from_vec(vec![
            Comment::new(
                interner.intern(" @param b Second"),
                Span::new(0, 20),
                CommentKind::DocParam,
            ),
            Comment::new(
                interner.intern(" @param a First"),
                Span::new(21, 40),
                CommentKind::DocParam,
            ),
        ]);

        let param_names = ["a", "b"];
        let indices = vec![0, 1];

        let reordered = reorder_param_comments(&indices, &comments, &param_names, &interner);

        // Should be reordered to match param order: a (index 1) then b (index 0)
        assert_eq!(reordered, vec![1, 0]);
    }

    #[test]
    fn test_group_comments_for_reordering() {
        let interner = test_interner();
        let comments = CommentList::from_vec(vec![
            Comment::regular(interner.intern(" regular"), Span::new(0, 10)),
            Comment::new(
                interner.intern(" #Description"),
                Span::new(11, 25),
                CommentKind::DocDescription,
            ),
            Comment::new(
                interner.intern(" @param x"),
                Span::new(26, 35),
                CommentKind::DocParam,
            ),
            Comment::regular(interner.intern(" another regular"), Span::new(36, 55)),
        ]);

        let indices = vec![0, 1, 2, 3];
        let groups = group_comments_for_reordering(&indices, &comments);

        // Should have 3 groups: [0] (regular), [1,2] (doc), [3] (regular)
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0], vec![0]);
        assert_eq!(groups[1], vec![1, 2]);
        assert_eq!(groups[2], vec![3]);
    }
}
