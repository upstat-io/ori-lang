//! Series combinator for parsing comma-separated lists.
//!
//! This module provides reusable infrastructure for parsing sequences of items
//! separated by a delimiter (typically comma). It unifies the common pattern
//! found throughout the parser:
//!
//! ```text
//! while !check(terminator) && !is_at_end() {
//!     parse_item();
//!     if !check(terminator) { expect(separator); }
//! }
//! ```
//!
//! # Reference
//!
//! Inspired by Gleam's `series_of()` combinator which handles comma-separated
//! lists with trailing separator support.

use ori_ir::TokenKind;

use crate::{ParseError, Parser};

/// Configuration for parsing a series of items.
#[derive(Clone, Debug)]
pub struct SeriesConfig {
    /// The separator token between items (usually Comma).
    pub separator: TokenKind,
    /// The terminator token that ends the series (e.g., `RParen`, `RBracket`).
    pub terminator: TokenKind,
    /// How to handle trailing separators.
    pub trailing: TrailingSeparator,
    /// Whether to skip newlines between items.
    pub skip_newlines: bool,
    /// Minimum number of items required.
    pub min_count: usize,
    /// Maximum number of items allowed (None = unlimited).
    pub max_count: Option<usize>,
}

/// Policy for trailing separators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingSeparator {
    /// Trailing separator is allowed but not required.
    /// Break loop when terminator found after separator.
    Allowed,
    /// Trailing separator is forbidden.
    /// Error if separator appears before terminator.
    Forbidden,
    /// Separator is required between items, but not after last.
    /// This matches most Ori syntax.
    Required,
}

impl SeriesConfig {
    /// Create a new config with comma separator and specified terminator.
    #[inline]
    pub fn comma(terminator: TokenKind) -> Self {
        Self {
            separator: TokenKind::Comma,
            terminator,
            trailing: TrailingSeparator::Allowed,
            skip_newlines: true,
            min_count: 0,
            max_count: None,
        }
    }

    /// Create a config for plus-separated bounds (e.g., `Trait + OtherTrait`).
    #[inline]
    pub fn plus_bounds() -> Self {
        Self {
            separator: TokenKind::Plus,
            terminator: TokenKind::Eof, // No explicit terminator
            trailing: TrailingSeparator::Forbidden,
            skip_newlines: false,
            min_count: 1,
            max_count: None,
        }
    }

    /// Set minimum item count.
    #[inline]
    #[must_use]
    pub fn min(mut self, count: usize) -> Self {
        self.min_count = count;
        self
    }

    /// Set maximum item count.
    #[inline]
    #[must_use]
    pub fn max(mut self, count: usize) -> Self {
        self.max_count = Some(count);
        self
    }

    /// Disable newline skipping.
    #[inline]
    #[must_use]
    pub fn no_newlines(mut self) -> Self {
        self.skip_newlines = false;
        self
    }

    /// Set trailing separator policy.
    #[inline]
    #[must_use]
    pub fn trailing(mut self, policy: TrailingSeparator) -> Self {
        self.trailing = policy;
        self
    }
}

impl Parser<'_> {
    /// Core series loop: separator/terminator/trailing logic shared between
    /// `series()` and `series_direct()`.
    ///
    /// The `parse_item` closure returns `Ok(true)` when an item was parsed
    /// (the caller is responsible for collecting it) or `Ok(false)` when no
    /// item is present. Returns the number of items parsed.
    fn series_core<F>(
        &mut self,
        config: &SeriesConfig,
        mut parse_item: F,
    ) -> Result<usize, ParseError>
    where
        F: FnMut(&mut Self) -> Result<bool, ParseError>,
    {
        let mut count = 0;

        loop {
            if config.skip_newlines {
                self.cursor.skip_newlines();
            }

            // Check for terminator
            if self.cursor.check(&config.terminator) || self.cursor.is_at_end() {
                break;
            }

            // Try to parse item
            if parse_item(self)? {
                count += 1;
            } else {
                // No item found — if we already have items, this is an error
                // (we expected an item after separator)
                if count > 0 && config.trailing == TrailingSeparator::Forbidden {
                    return Err(ParseError::expected_item(
                        self.cursor.current_span(),
                        &config.terminator,
                    ));
                }
                break;
            }

            if config.skip_newlines {
                self.cursor.skip_newlines();
            }

            // Check for separator
            if self.cursor.check(&config.separator) {
                self.cursor.advance();

                if config.skip_newlines {
                    self.cursor.skip_newlines();
                }

                // Check for trailing separator
                if self.cursor.check(&config.terminator) {
                    if config.trailing == TrailingSeparator::Forbidden {
                        return Err(ParseError::unexpected_trailing_separator(
                            self.cursor.previous_span(),
                            &config.separator,
                        ));
                    }
                    // Allowed or Required both break here
                    break;
                }
            } else if !self.cursor.check(&config.terminator) && !self.cursor.is_at_end() {
                // No separator and no terminator — error
                return Err(ParseError::expected_separator_or_terminator(
                    self.cursor.current_span(),
                    &config.separator,
                    &config.terminator,
                ));
            } else {
                // No separator but we have terminator — end of list
                break;
            }
        }

        // Validate count constraints
        if count < config.min_count {
            return Err(ParseError::too_few_items(
                self.cursor.current_span(),
                config.min_count,
                count,
            ));
        }
        if let Some(max) = config.max_count {
            if count > max {
                return Err(ParseError::too_many_items(
                    self.cursor.current_span(),
                    max,
                    count,
                ));
            }
        }

        Ok(count)
    }

    /// Parse a series of items with the given configuration.
    ///
    /// The `parse_item` function should:
    /// - Return `Ok(Some(item))` when an item was successfully parsed
    /// - Return `Ok(None)` when no item is present (but no error)
    /// - Return `Err(e)` when parsing failed with an error
    ///
    /// # Example
    ///
    /// ```ignore
    /// let args = self.series(
    ///     SeriesConfig::comma(TokenKind::RParen),
    ///     |p| {
    ///         if p.cursor.check(&TokenKind::RParen) {
    ///             Ok(None)
    ///         } else {
    ///             Ok(Some(p.parse_expr()?))
    ///         }
    ///     },
    /// )?;
    /// ```
    pub fn series<T, F>(
        &mut self,
        config: &SeriesConfig,
        mut parse_item: F,
    ) -> Result<Vec<T>, ParseError>
    where
        F: FnMut(&mut Self) -> Result<Option<T>, ParseError>,
    {
        let mut items = Vec::new();
        self.series_core(config, |p| {
            if let Some(item) = parse_item(p)? {
                items.push(item);
                Ok(true)
            } else {
                Ok(false)
            }
        })?;
        Ok(items)
    }

    // --- Convenience Methods ---

    /// Parse a comma-separated series in parentheses: `(item, item, ...)`
    ///
    /// Expects `(` to already be consumed. Consumes the closing `)`.
    pub fn paren_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: FnMut(&mut Self) -> Result<Option<T>, ParseError>,
    {
        let items = self.series(&SeriesConfig::comma(TokenKind::RParen), parse_item)?;
        self.cursor.expect(&TokenKind::RParen)?;
        Ok(items)
    }

    /// Parse a comma-separated series in brackets: `[item, item, ...]`
    ///
    /// Expects `[` to already be consumed. Consumes the closing `]`.
    pub fn bracket_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: FnMut(&mut Self) -> Result<Option<T>, ParseError>,
    {
        let items = self.series(&SeriesConfig::comma(TokenKind::RBracket), parse_item)?;
        self.cursor.expect(&TokenKind::RBracket)?;
        Ok(items)
    }

    /// Parse a comma-separated series in braces: `{item, item, ...}`
    ///
    /// Expects `{` to already be consumed. Consumes the closing `}`.
    pub fn brace_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: FnMut(&mut Self) -> Result<Option<T>, ParseError>,
    {
        let items = self.series(&SeriesConfig::comma(TokenKind::RBrace), parse_item)?;
        self.cursor.expect(&TokenKind::RBrace)?;
        Ok(items)
    }

    /// Parse a comma-separated series in angle brackets: `<item, item, ...>`
    ///
    /// Expects `<` to already be consumed. Consumes the closing `>`.
    pub fn angle_series<T, F>(&mut self, parse_item: F) -> Result<Vec<T>, ParseError>
    where
        F: FnMut(&mut Self) -> Result<Option<T>, ParseError>,
    {
        let items = self.series(&SeriesConfig::comma(TokenKind::Gt), parse_item)?;
        self.cursor.expect(&TokenKind::Gt)?;
        Ok(items)
    }

    // --- Direct Push Series ---
    //
    // These variants eliminate the intermediate Vec by having the closure push
    // directly into the arena. Same separator/terminator/trailing logic as
    // `series()`, but the closure returns `Ok(true)` for "item pushed" or
    // `Ok(false)` for "no item" instead of `Ok(Some(T))`/`Ok(None)`.

    /// Parse a series where the closure pushes items directly (no Vec).
    ///
    /// The closure should:
    /// - Parse an item and push it to the arena, then return `Ok(true)`
    /// - Return `Ok(false)` when no item is present (break)
    /// - Return `Err(e)` on parse failure
    ///
    /// Returns the number of items parsed (for count validation).
    pub fn series_direct<F>(
        &mut self,
        config: &SeriesConfig,
        mut parse_and_push: F,
    ) -> Result<usize, ParseError>
    where
        F: FnMut(&mut Self) -> Result<bool, ParseError>,
    {
        self.series_core(config, |p| parse_and_push(p))
    }

    /// Direct-push series in parentheses: `(item, item, ...)`
    ///
    /// Expects `(` to already be consumed. Consumes the closing `)`.
    pub fn paren_series_direct<F>(&mut self, parse_and_push: F) -> Result<usize, ParseError>
    where
        F: FnMut(&mut Self) -> Result<bool, ParseError>,
    {
        let count = self.series_direct(&SeriesConfig::comma(TokenKind::RParen), parse_and_push)?;
        self.cursor.expect(&TokenKind::RParen)?;
        Ok(count)
    }

    /// Direct-push series in brackets: `[item, item, ...]`
    ///
    /// Expects `[` to already be consumed. Consumes the closing `]`.
    pub fn bracket_series_direct<F>(&mut self, parse_and_push: F) -> Result<usize, ParseError>
    where
        F: FnMut(&mut Self) -> Result<bool, ParseError>,
    {
        let count =
            self.series_direct(&SeriesConfig::comma(TokenKind::RBracket), parse_and_push)?;
        self.cursor.expect(&TokenKind::RBracket)?;
        Ok(count)
    }

    /// Direct-push series in braces: `{item, item, ...}`
    ///
    /// Expects `{` to already be consumed. Consumes the closing `}`.
    pub fn brace_series_direct<F>(&mut self, parse_and_push: F) -> Result<usize, ParseError>
    where
        F: FnMut(&mut Self) -> Result<bool, ParseError>,
    {
        let count = self.series_direct(&SeriesConfig::comma(TokenKind::RBrace), parse_and_push)?;
        self.cursor.expect(&TokenKind::RBrace)?;
        Ok(count)
    }

    /// Direct-push series in angle brackets: `<item, item, ...>`
    ///
    /// Expects `<` to already be consumed. Consumes the closing `>`.
    pub fn angle_series_direct<F>(&mut self, parse_and_push: F) -> Result<usize, ParseError>
    where
        F: FnMut(&mut Self) -> Result<bool, ParseError>,
    {
        let count = self.series_direct(&SeriesConfig::comma(TokenKind::Gt), parse_and_push)?;
        self.cursor.expect(&TokenKind::Gt)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
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
}
