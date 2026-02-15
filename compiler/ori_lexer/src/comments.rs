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
mod tests;
