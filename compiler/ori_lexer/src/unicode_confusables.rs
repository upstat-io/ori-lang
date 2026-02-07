//! Unicode confusable character detection.
//!
//! Maps Unicode characters that look similar to ASCII to their ASCII equivalents.
//! When a confusable is found in source, the lexer emits a targeted error with
//! a substitution suggestion instead of a generic "invalid byte" error.
//!
//! This is NOT Unicode identifier support — Ori source is ASCII-only per spec.
//! This table exists solely for better error messages.
//!
//! Inspired by Rust's `unicode_chars.rs` and Go's curly quote detection.

/// Maps Unicode characters that are visually similar to ASCII to their
/// ASCII equivalents. Sorted by Unicode codepoint for binary search.
///
/// Format: `(found_char, suggested_ascii, unicode_name)`
const UNICODE_CONFUSABLES: &[(char, char, &str)] = &[
    // U+00xx: Latin-1 Supplement
    ('\u{00B7}', '.', "Middle Dot"),
    ('\u{00D7}', '*', "Multiplication Sign"),
    // U+200x: Zero-width and invisible characters
    ('\u{200B}', ' ', "Zero Width Space"),
    ('\u{200C}', ' ', "Zero Width Non-Joiner"),
    ('\u{200D}', ' ', "Zero Width Joiner"),
    // U+201x: Dashes
    ('\u{2010}', '-', "Hyphen"),
    ('\u{2011}', '-', "Non-Breaking Hyphen"),
    ('\u{2012}', '-', "Figure Dash"),
    ('\u{2013}', '-', "En Dash"),
    ('\u{2014}', '-', "Em Dash"),
    ('\u{2015}', '-', "Horizontal Bar"),
    // U+201x: Quotes (most common confusable — copy/paste from word processors)
    ('\u{2018}', '\'', "Left Single Quotation Mark"),
    ('\u{2019}', '\'', "Right Single Quotation Mark"),
    ('\u{201A}', ',', "Single Low-9 Quotation Mark"),
    ('\u{201C}', '"', "Left Double Quotation Mark"),
    ('\u{201D}', '"', "Right Double Quotation Mark"),
    ('\u{201E}', '"', "Double Low-9 Quotation Mark"),
    // U+202x-204x: Misc punctuation and operators
    ('\u{2024}', '.', "One Dot Leader"),
    ('\u{2044}', '/', "Fraction Slash"),
    // U+221x-223x: Mathematical operators
    ('\u{2212}', '-', "Minus Sign"),
    ('\u{2215}', '/', "Division Slash"),
    ('\u{2217}', '*', "Asterisk Operator"),
    ('\u{2219}', '.', "Bullet Operator"),
    ('\u{2223}', '|', "Divides"),
    ('\u{2236}', ':', "Ratio"),
    // U+232x: Technical symbols
    ('\u{2329}', '<', "Left-Pointing Angle Bracket"),
    ('\u{232A}', '>', "Right-Pointing Angle Bracket"),
    // U+276x: Ornamental brackets
    ('\u{2768}', '(', "Medium Left Parenthesis Ornament"),
    ('\u{2769}', ')', "Medium Right Parenthesis Ornament"),
    // U+FExx: Specials
    ('\u{FEFF}', ' ', "Zero Width No-Break Space (BOM)"),
    // U+FFxx: Fullwidth characters
    ('\u{FF01}', '!', "Fullwidth Exclamation Mark"),
    ('\u{FF08}', '(', "Fullwidth Left Parenthesis"),
    ('\u{FF09}', ')', "Fullwidth Right Parenthesis"),
    ('\u{FF0B}', '+', "Fullwidth Plus Sign"),
    ('\u{FF0C}', ',', "Fullwidth Comma"),
    ('\u{FF0D}', '-', "Fullwidth Hyphen-Minus"),
    ('\u{FF0E}', '.', "Fullwidth Full Stop"),
    ('\u{FF0F}', '/', "Fullwidth Solidus"),
    ('\u{FF1A}', ':', "Fullwidth Colon"),
    ('\u{FF1B}', ';', "Fullwidth Semicolon"),
    ('\u{FF1C}', '<', "Fullwidth Less-Than Sign"),
    ('\u{FF1D}', '=', "Fullwidth Equals Sign"),
    ('\u{FF1E}', '>', "Fullwidth Greater-Than Sign"),
    ('\u{FF1F}', '?', "Fullwidth Question Mark"),
    ('\u{FF20}', '@', "Fullwidth Commercial At"),
    ('\u{FF3B}', '[', "Fullwidth Left Square Bracket"),
    ('\u{FF3D}', ']', "Fullwidth Right Square Bracket"),
    ('\u{FF3F}', '_', "Fullwidth Low Line"),
    ('\u{FF5B}', '{', "Fullwidth Left Curly Bracket"),
    ('\u{FF5C}', '|', "Fullwidth Vertical Line"),
    ('\u{FF5D}', '}', "Fullwidth Right Curly Bracket"),
    ('\u{FF5E}', '~', "Fullwidth Tilde"),
];

/// Look up a character in the confusable table.
///
/// Returns `Some((suggested_ascii, unicode_name))` if the character is a
/// known confusable, or `None` otherwise.
///
/// Uses binary search on the sorted table for O(log n) lookup.
pub fn lookup_confusable(ch: char) -> Option<(char, &'static str)> {
    UNICODE_CONFUSABLES
        .binary_search_by_key(&ch, |&(found, _, _)| found)
        .ok()
        .map(|idx| {
            let (_, suggested, name) = UNICODE_CONFUSABLES[idx];
            (suggested, name)
        })
}

/// Check if a character is a "smart quote" (curly quote from word processors).
///
/// These are the most common confusable in practice and deserve an
/// especially clear error message.
pub fn is_smart_quote(ch: char) -> bool {
    matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}')
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted() {
        for window in UNICODE_CONFUSABLES.windows(2) {
            assert!(
                window[0].0 < window[1].0,
                "table not sorted: {:?} >= {:?}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn lookup_smart_quotes() {
        let (suggested, name) = lookup_confusable('\u{201C}').unwrap();
        assert_eq!(suggested, '"');
        assert_eq!(name, "Left Double Quotation Mark");

        let (suggested, _) = lookup_confusable('\u{201D}').unwrap();
        assert_eq!(suggested, '"');
    }

    #[test]
    fn lookup_en_dash() {
        let (suggested, name) = lookup_confusable('\u{2013}').unwrap();
        assert_eq!(suggested, '-');
        assert_eq!(name, "En Dash");
    }

    #[test]
    fn lookup_fullwidth() {
        let (suggested, name) = lookup_confusable('\u{FF0B}').unwrap();
        assert_eq!(suggested, '+');
        assert_eq!(name, "Fullwidth Plus Sign");
    }

    #[test]
    fn lookup_zero_width_space() {
        let (suggested, name) = lookup_confusable('\u{200B}').unwrap();
        assert_eq!(suggested, ' ');
        assert_eq!(name, "Zero Width Space");
    }

    #[test]
    fn lookup_unknown_returns_none() {
        assert!(lookup_confusable('a').is_none());
        assert!(lookup_confusable('Z').is_none());
        assert!(lookup_confusable('0').is_none());
    }

    #[test]
    fn smart_quote_detection() {
        assert!(is_smart_quote('\u{201C}'));
        assert!(is_smart_quote('\u{201D}'));
        assert!(is_smart_quote('\u{2018}'));
        assert!(is_smart_quote('\u{2019}'));
        assert!(!is_smart_quote('"'));
        assert!(!is_smart_quote('\''));
    }

    #[test]
    fn all_entries_have_ascii_suggestions() {
        for &(found, suggested, _) in UNICODE_CONFUSABLES {
            assert!(
                suggested.is_ascii(),
                "suggestion for {found:?} is not ASCII: {suggested:?}"
            );
        }
    }
}
