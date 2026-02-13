//! Comment Classification and Normalization
//!
//! Classifies comments by their content and normalizes spacing.

use std::borrow::Cow;

use ori_ir::CommentKind;

/// Classify a comment by its content and return the normalized content.
///
/// Normalizes spacing: adds a space after `//` if missing, removes extra space
/// after doc markers.
///
/// Returns (`CommentKind`, `normalized_content`). Uses `Cow` to avoid allocation
/// when the content is already in the correct format (the common case for
/// regular comments with a leading space).
pub(crate) fn classify_and_normalize_comment(content: &str) -> (CommentKind, Cow<'_, str>) {
    // Trim leading whitespace to check for markers
    let trimmed = content.trim_start();

    // Check for doc comment markers (new `*` format first, then legacy)

    // Member (unified): `// * name: description` -> ` * name: description`
    // Must have `* ` followed by an identifier and `:` to distinguish from
    // regular comments that happen to start with `*` (e.g., bullet lists).
    if let Some(rest) = trimmed.strip_prefix('*') {
        if let Some(after_star) = rest.strip_prefix(' ') {
            // Check for "name:" pattern
            if let Some(colon_pos) = after_star.find(':') {
                let name_part = after_star[..colon_pos].trim();
                if !name_part.is_empty()
                    && name_part.chars().all(|c| c.is_alphanumeric() || c == '_')
                {
                    // Valid `* name: description` pattern
                    let desc = after_star[colon_pos + 1..].trim_start();
                    if desc.is_empty() {
                        return (CommentKind::DocMember, format!(" * {name_part}:").into());
                    }
                    return (
                        CommentKind::DocMember,
                        format!(" * {name_part}: {desc}").into(),
                    );
                }
            }
        }
    }

    if let Some(rest) = trimmed.strip_prefix('#') {
        // Description: `// #Text` -> ` #Text`
        let text = rest.trim_start();
        return (CommentKind::DocDescription, format!(" #{text}").into());
    }

    if let Some(rest) = trimmed.strip_prefix("@param") {
        // Legacy parameter: `// @param name desc` -> ` @param name desc`
        // Classified as DocMember for unified handling.
        let text = if rest.starts_with(char::is_whitespace) {
            rest.trim_start()
        } else {
            rest
        };
        return (CommentKind::DocMember, format!(" @param {text}").into());
    }

    if let Some(rest) = trimmed.strip_prefix("@field") {
        // Legacy field: `// @field name desc` -> ` @field name desc`
        // Classified as DocMember for unified handling.
        let text = if rest.starts_with(char::is_whitespace) {
            rest.trim_start()
        } else {
            rest
        };
        return (CommentKind::DocMember, format!(" @field {text}").into());
    }

    if let Some(rest) = trimmed.strip_prefix('!') {
        // Warning: `// !Text` -> ` !Text`
        let text = rest.trim_start();
        return (CommentKind::DocWarning, format!(" !{text}").into());
    }

    if let Some(rest) = trimmed.strip_prefix('>') {
        // Example: `// >example()` -> ` >example()`
        // Don't trim after > to preserve example formatting
        return (CommentKind::DocExample, format!(" >{rest}").into());
    }

    // Regular comment - ensure space after //
    if content.is_empty() {
        // Empty comment: just "//"
        (CommentKind::Regular, Cow::Borrowed(""))
    } else if content.starts_with(' ') {
        // Already has space: preserve as-is (zero-copy fast path)
        (CommentKind::Regular, Cow::Borrowed(content))
    } else {
        // Missing space: add one
        (CommentKind::Regular, format!(" {content}").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_regular_comment() {
        let (kind, content) = classify_and_normalize_comment(" regular text");
        assert_eq!(kind, CommentKind::Regular);
        assert_eq!(content, " regular text");
    }

    #[test]
    fn test_classify_doc_description() {
        let (kind, content) = classify_and_normalize_comment(" #Description");
        assert_eq!(kind, CommentKind::DocDescription);
        assert_eq!(content, " #Description");

        // With extra spaces
        let (kind, content) = classify_and_normalize_comment("  #Description");
        assert_eq!(kind, CommentKind::DocDescription);
        assert_eq!(content, " #Description");
    }

    #[test]
    fn test_classify_legacy_param_as_member() {
        let (kind, content) = classify_and_normalize_comment(" @param x value");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " @param x value");
    }

    #[test]
    fn test_classify_legacy_field_as_member() {
        let (kind, content) = classify_and_normalize_comment(" @field x coord");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " @field x coord");
    }

    #[test]
    fn test_classify_doc_warning() {
        let (kind, content) = classify_and_normalize_comment(" !Panics");
        assert_eq!(kind, CommentKind::DocWarning);
        assert_eq!(content, " !Panics");
    }

    #[test]
    fn test_classify_doc_example() {
        let (kind, content) = classify_and_normalize_comment(" >foo() -> 1");
        assert_eq!(kind, CommentKind::DocExample);
        // Preserve spacing after > exactly
        assert_eq!(content, " >foo() -> 1");
    }

    #[test]
    fn test_classify_doc_member() {
        let (kind, content) = classify_and_normalize_comment(" * x: The value");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " * x: The value");
    }

    #[test]
    fn test_classify_doc_member_no_description() {
        let (kind, content) = classify_and_normalize_comment(" * name:");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " * name:");
    }

    #[test]
    fn test_classify_doc_member_underscore_name() {
        let (kind, content) = classify_and_normalize_comment(" * my_param: A value");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " * my_param: A value");
    }

    #[test]
    fn test_classify_star_without_colon_is_regular() {
        // `* text` without a colon is a regular comment (bullet list)
        let (kind, _) = classify_and_normalize_comment(" * just a bullet");
        assert_eq!(kind, CommentKind::Regular);
    }

    #[test]
    fn test_classify_star_with_spaces_in_name_is_regular() {
        // `* two words: desc` is regular because "two words" has a space
        let (kind, _) = classify_and_normalize_comment(" * two words: desc");
        assert_eq!(kind, CommentKind::Regular);
    }

    #[test]
    fn test_classify_doc_member_extra_spaces() {
        // Extra leading spaces should still work
        let (kind, content) = classify_and_normalize_comment("  * x: value");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " * x: value");
    }

    #[test]
    fn test_classify_empty_comment() {
        let (kind, content) = classify_and_normalize_comment("");
        assert_eq!(kind, CommentKind::Regular);
        assert_eq!(content, "");
    }

    #[test]
    fn test_classify_no_space_adds_space() {
        let (kind, content) = classify_and_normalize_comment("no space");
        assert_eq!(kind, CommentKind::Regular);
        assert_eq!(content, " no space");
    }

    #[test]
    fn test_legacy_param_emits_doc_member() {
        let (kind, content) = classify_and_normalize_comment(" @param x The value");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " @param x The value");
    }

    #[test]
    fn test_classify_regular_borrows() {
        // The common case (regular comment with space) should borrow, not allocate
        let (kind, content) = classify_and_normalize_comment(" regular text");
        assert_eq!(kind, CommentKind::Regular);
        assert!(matches!(content, Cow::Borrowed(_)));
    }

    #[test]
    fn test_legacy_field_emits_doc_member() {
        let (kind, content) = classify_and_normalize_comment(" @field y The coord");
        assert_eq!(kind, CommentKind::DocMember);
        assert_eq!(content, " @field y The coord");
    }
}
