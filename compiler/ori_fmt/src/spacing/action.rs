//! Space action enum for spacing decisions.

/// What spacing to emit between two adjacent tokens.
///
/// This is the output of spacing rule evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpaceAction {
    /// No space between tokens: `foo()`, `list[0]`
    None,

    /// Single space between tokens: `a + b`, `x: int`
    Space,

    /// Line break between tokens (rarely used in token spacing)
    Newline,

    /// Preserve whatever spacing exists in source
    Preserve,
}

impl SpaceAction {
    /// Check if this action requires a space.
    #[inline]
    pub fn needs_space(self) -> bool {
        matches!(self, SpaceAction::Space)
    }

    /// Check if this action requires a newline.
    #[inline]
    pub fn needs_newline(self) -> bool {
        matches!(self, SpaceAction::Newline)
    }

    /// Check if spacing should be preserved from source.
    #[inline]
    pub fn preserves(self) -> bool {
        matches!(self, SpaceAction::Preserve)
    }
}

impl Default for SpaceAction {
    /// Default to no space - explicit rules add spaces.
    fn default() -> Self {
        SpaceAction::None
    }
}
