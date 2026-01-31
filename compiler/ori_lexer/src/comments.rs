//! Comment Classification and Normalization
//!
//! Classifies comments by their content and normalizes spacing.

use ori_ir::CommentKind;

/// Classify a comment by its content and return the normalized content.
///
/// Normalizes spacing: adds a space after `//` if missing, removes extra space
/// after doc markers.
///
/// Returns (`CommentKind`, `normalized_content`).
pub(crate) fn classify_and_normalize_comment(content: &str) -> (CommentKind, String) {
    // Trim leading whitespace to check for markers
    let trimmed = content.trim_start();

    // Check for doc comment markers
    if let Some(rest) = trimmed.strip_prefix('#') {
        // Description: `// #Text` -> ` #Text`
        let text = rest.trim_start();
        return (CommentKind::DocDescription, format!(" #{text}"));
    }

    if let Some(rest) = trimmed.strip_prefix("@param") {
        // Parameter: `// @param name desc` -> ` @param name desc`
        // Keep the space or lack thereof after @param
        let text = if rest.starts_with(char::is_whitespace) {
            rest.trim_start()
        } else {
            rest
        };
        return (CommentKind::DocParam, format!(" @param {text}"));
    }

    if let Some(rest) = trimmed.strip_prefix("@field") {
        // Field: `// @field name desc` -> ` @field name desc`
        let text = if rest.starts_with(char::is_whitespace) {
            rest.trim_start()
        } else {
            rest
        };
        return (CommentKind::DocField, format!(" @field {text}"));
    }

    if let Some(rest) = trimmed.strip_prefix('!') {
        // Warning: `// !Text` -> ` !Text`
        let text = rest.trim_start();
        return (CommentKind::DocWarning, format!(" !{text}"));
    }

    if let Some(rest) = trimmed.strip_prefix('>') {
        // Example: `// >example()` -> ` >example()`
        // Don't trim after > to preserve example formatting
        return (CommentKind::DocExample, format!(" >{rest}"));
    }

    // Regular comment - ensure space after //
    if content.is_empty() {
        // Empty comment: just "//"
        (CommentKind::Regular, String::new())
    } else if content.starts_with(' ') {
        // Already has space: preserve as-is
        (CommentKind::Regular, content.to_string())
    } else {
        // Missing space: add one
        (CommentKind::Regular, format!(" {content}"))
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
    fn test_classify_doc_param() {
        let (kind, content) = classify_and_normalize_comment(" @param x value");
        assert_eq!(kind, CommentKind::DocParam);
        assert_eq!(content, " @param x value");
    }

    #[test]
    fn test_classify_doc_field() {
        let (kind, content) = classify_and_normalize_comment(" @field x coord");
        assert_eq!(kind, CommentKind::DocField);
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
}
