//! Packing strategies for containers.

use super::ConstructKind;

/// How to pack items in a container.
///
/// This enum represents the four fundamental packing strategies:
/// - Try to fit, break one per line if not
/// - Try to fit, pack multiple per line if not (for simple items)
/// - Always one per line (user indicated with trailing comma or comments)
/// - Always stacked (special constructs like run, try, match)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Packing {
    /// Try single line; if doesn't fit, one item per line.
    ///
    /// This is the default for most containers:
    /// - Function params/args
    /// - Struct fields
    /// - Generic params
    /// - Map entries
    ///
    /// Example:
    /// ```ori
    /// // Inline if fits:
    /// @foo (x: int, y: int) -> int
    ///
    /// // One per line if doesn't:
    /// @foo (
    ///     x: int,
    ///     y: int,
    ///     z: int,
    /// ) -> int
    /// ```
    #[default]
    FitOrOnePerLine,

    /// Try single line; if doesn't fit, pack multiple per line.
    ///
    /// Only used for simple lists (literals, identifiers).
    ///
    /// Example:
    /// ```ori
    /// // Inline if fits:
    /// [1, 2, 3, 4, 5]
    ///
    /// // Pack multiple per line if doesn't:
    /// [
    ///     1, 2, 3, 4, 5,
    ///     6, 7, 8, 9, 10,
    /// ]
    /// ```
    FitOrPackMultiple,

    /// Always one item per line (trailing comma present, or rule says so).
    ///
    /// User intent is preserved - if they put a trailing comma, we keep items
    /// on separate lines even if they'd fit inline.
    ///
    /// Example:
    /// ```ori
    /// [
    ///     1,
    ///     2,
    ///     3,
    /// ]
    /// ```
    AlwaysOnePerLine,

    /// Always stacked with specific formatting (run, try, match, etc.).
    ///
    /// These constructs NEVER go inline, regardless of width.
    ///
    /// Example:
    /// ```ori
    /// run(
    ///     let x = 1,
    ///     let y = 2,
    ///     x + y,
    /// )
    /// ```
    AlwaysStacked,
}

impl Packing {
    /// Check if this packing can try to fit inline.
    #[inline]
    pub fn can_try_inline(self) -> bool {
        matches!(self, Packing::FitOrOnePerLine | Packing::FitOrPackMultiple)
    }

    /// Check if this packing always forces multiline.
    #[inline]
    pub fn always_multiline(self) -> bool {
        matches!(self, Packing::AlwaysOnePerLine | Packing::AlwaysStacked)
    }

    /// Check if this packing allows packing multiple items per line.
    #[inline]
    pub fn allows_packing(self) -> bool {
        matches!(self, Packing::FitOrPackMultiple)
    }
}

/// Determine the packing strategy for a container.
///
/// # Arguments
///
/// * `construct` - What kind of container we're formatting
/// * `has_trailing_comma` - Whether source had a trailing comma (user intent)
/// * `has_comments` - Whether there are comments inside the container
/// * `has_empty_lines` - Whether there are blank lines between items
/// * `item_count` - Number of items in the container
///
/// # Returns
///
/// The packing strategy to use.
pub fn determine_packing(
    construct: ConstructKind,
    has_trailing_comma: bool,
    has_comments: bool,
    has_empty_lines: bool,
    _item_count: usize,
) -> Packing {
    // Determine the base strategy from the construct kind (exhaustive match
    // ensures new variants must make an explicit packing decision)
    let base = match construct {
        // Always stacked (spec lines 78-90)
        ConstructKind::RunTopLevel
        | ConstructKind::Try
        | ConstructKind::Match
        | ConstructKind::Recurse
        | ConstructKind::Parallel
        | ConstructKind::Spawn
        | ConstructKind::Nursery
        | ConstructKind::MatchArms => return Packing::AlwaysStacked,

        // Simple lists can pack multiple per line
        ConstructKind::ListSimple => Packing::FitOrPackMultiple,

        // Everything else: try inline, else one per line
        ConstructKind::FunctionParams
        | ConstructKind::FunctionArgs
        | ConstructKind::GenericParams
        | ConstructKind::WhereConstraints
        | ConstructKind::Capabilities
        | ConstructKind::StructFieldsDef
        | ConstructKind::StructFieldsLiteral
        | ConstructKind::SumVariants
        | ConstructKind::MapEntries
        | ConstructKind::TupleElements
        | ConstructKind::ImportItems
        | ConstructKind::ListComplex
        | ConstructKind::RunNested => Packing::FitOrOnePerLine,
    };

    // Metadata overrides: user intent signals that force multiline
    if has_empty_lines || has_trailing_comma || has_comments {
        return Packing::AlwaysOnePerLine;
    }

    base
}
