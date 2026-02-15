use super::*;

/// Check if a character is a "smart quote" (curly quote from word processors).
///
/// These are the most common confusable in practice and deserve an
/// especially clear error message.
fn is_smart_quote(ch: char) -> bool {
    matches!(ch, '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}')
}

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
