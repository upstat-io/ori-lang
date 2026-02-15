use super::*;

#[test]
fn series_config_comma_defaults() {
    let config = SeriesConfig::comma(TokenKind::RParen);
    assert_eq!(config.separator, TokenKind::Comma);
    assert_eq!(config.terminator, TokenKind::RParen);
    assert_eq!(config.trailing, TrailingSeparator::Allowed);
    assert!(config.skip_newlines);
    assert_eq!(config.min_count, 0);
    assert_eq!(config.max_count, None);
}

#[test]
fn series_config_plus_bounds() {
    let config = SeriesConfig::plus_bounds();
    assert_eq!(config.separator, TokenKind::Plus);
    assert_eq!(config.trailing, TrailingSeparator::Forbidden);
    assert!(!config.skip_newlines);
    assert_eq!(config.min_count, 1);
}

#[test]
fn series_config_builder_methods() {
    let config = SeriesConfig::comma(TokenKind::RBracket)
        .min(1)
        .max(5)
        .no_newlines()
        .trailing(TrailingSeparator::Forbidden);

    assert_eq!(config.min_count, 1);
    assert_eq!(config.max_count, Some(5));
    assert!(!config.skip_newlines);
    assert_eq!(config.trailing, TrailingSeparator::Forbidden);
}
