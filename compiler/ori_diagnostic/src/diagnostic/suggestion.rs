//! The [`Suggestion`] type — a structured fix proposal attached to a diagnostic.

use ori_ir::{Name, Span};

use super::{Applicability, Substitution};

/// A structured suggestion with substitutions and applicability.
///
/// Supports two forms:
/// - **Text-only**: A human-readable message with no code substitutions.
///   Created via `text()`, `text_with_names()`, `wrap_in()`.
/// - **Span-bearing**: A message with exact code substitutions for `ori fix`.
///   Created via `new()`, `machine_applicable()`, `maybe_incorrect()`, etc.
///
/// Suggestions have a `priority` field (lower = more likely relevant) used
/// for ordering when multiple suggestions are presented.
///
/// # Salsa Compatibility
/// Derives `Eq, PartialEq, Hash` for use in Salsa query results.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Suggestion {
    /// Human-readable message describing the fix.
    pub message: String,
    /// The text substitutions to make (empty for text-only suggestions).
    pub substitutions: Vec<Substitution>,
    /// How confident we are in this suggestion.
    pub applicability: Applicability,
    /// Priority (lower = more likely to be relevant).
    /// 0 = most likely, 1 = likely, 2 = possible, 3 = unlikely.
    pub priority: u8,
    /// Deferred-render `Name` operands. When non-empty, `{N}` placeholders in
    /// `message` are resolved at render time via the interner-backed
    /// `format_name` (so the raw `Name(..)` Debug form never reaches the user).
    /// Empty for plain-text suggestions (the `is_text_only` check is unaffected
    /// — names-bearing suggestions still carry empty `substitutions`).
    pub names: Vec<Name>,
}

impl Suggestion {
    /// Create a new suggestion with a single substitution.
    pub fn new(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
        applicability: Applicability,
        priority: u8,
    ) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: vec![Substitution::new(span, snippet)],
            applicability,
            priority,
            names: Vec::new(),
        }
    }

    /// Create a text-only suggestion (no code substitution).
    pub fn text(message: impl Into<String>, priority: u8) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: Vec::new(),
            applicability: Applicability::Unspecified,
            priority,
            names: Vec::new(),
        }
    }

    /// Create a text-only suggestion whose `{N}` placeholders are resolved at
    /// render time from `names` via the interner-backed `format_name`. The
    /// construction site carries the `Name`(s); no raw `Name(..)` Debug form is
    /// baked in. `{0}` resolves to `names[0]`, `{1}` to `names[1]`, etc.
    pub fn text_with_names(message: impl Into<String>, names: Vec<Name>, priority: u8) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: Vec::new(),
            applicability: Applicability::Unspecified,
            priority,
            names,
        }
    }

    /// Create a text-only suggestion with a single code replacement.
    pub fn text_with_replacement(
        message: impl Into<String>,
        priority: u8,
        span: Span,
        new_text: impl Into<String>,
    ) -> Self {
        Suggestion {
            message: message.into(),
            substitutions: vec![Substitution::new(span, new_text)],
            applicability: Applicability::MaybeIncorrect,
            priority,
            names: Vec::new(),
        }
    }

    /// Create a suggestion to wrap in something (priority 1).
    pub fn wrap_in(wrapper: &str, example: &str) -> Self {
        Self::text(format!("wrap the value in `{wrapper}`: `{example}`"), 1)
    }

    /// Create a machine-applicable suggestion (safe to auto-apply).
    pub fn machine_applicable(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::MachineApplicable, 0)
    }

    /// Create a suggestion that might be incorrect.
    pub fn maybe_incorrect(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::MaybeIncorrect, 0)
    }

    /// Create a suggestion with placeholders.
    pub fn has_placeholders(
        message: impl Into<String>,
        span: Span,
        snippet: impl Into<String>,
    ) -> Self {
        Self::new(message, span, snippet, Applicability::HasPlaceholders, 0)
    }

    /// Add another substitution to this suggestion.
    #[must_use]
    pub fn with_substitution(mut self, span: Span, snippet: impl Into<String>) -> Self {
        self.substitutions.push(Substitution::new(span, snippet));
        self
    }

    /// Check if this is a text-only suggestion (no code substitutions).
    pub fn is_text_only(&self) -> bool {
        self.substitutions.is_empty()
    }
}
