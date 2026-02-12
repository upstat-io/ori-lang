//! O(1) spacing rule lookup.
//!
//! Pre-computes a lookup table from token category pairs to spacing actions.

use rustc_hash::FxHashMap;
use std::sync::OnceLock;

use crate::spacing::rules::SPACE_RULES;

use super::{rules::SpaceRule, SpaceAction, TokenCategory, TokenMatcher};

/// Pre-computed O(1) lookup table for spacing rules.
///
/// Builds a hash map from (left, right) token category pairs to spacing actions.
/// Falls back to linear rule search for complex matchers (Any, Category predicates).
pub struct RulesMap {
    /// Direct lookup for exact (left, right) pairs.
    exact: FxHashMap<(TokenCategory, TokenCategory), SpaceAction>,

    /// Rules with Any or Category matchers (need linear scan).
    fallback_rules: Vec<&'static SpaceRule>,
}

impl RulesMap {
    /// Create a new rules map from the static rules.
    pub fn new() -> Self {
        let mut exact = FxHashMap::default();
        let mut fallback_rules = Vec::new();

        // Sort rules by priority for proper precedence
        let mut sorted_rules: Vec<_> = SPACE_RULES.iter().collect();
        sorted_rules.sort_by_key(|r| r.priority);

        // Process rules in priority order
        for rule in sorted_rules {
            match (&rule.left, &rule.right) {
                // Exact pairs can go directly in the map
                (TokenMatcher::Exact(left), TokenMatcher::Exact(right)) => {
                    // Only insert if not already present (higher priority rule wins)
                    exact.entry((*left, *right)).or_insert(rule.action);
                }

                // OneOf can be expanded into exact entries
                (TokenMatcher::Exact(left), TokenMatcher::OneOf(rights)) => {
                    for right in *rights {
                        exact.entry((*left, *right)).or_insert(rule.action);
                    }
                }
                (TokenMatcher::OneOf(lefts), TokenMatcher::Exact(right)) => {
                    for left in *lefts {
                        exact.entry((*left, *right)).or_insert(rule.action);
                    }
                }
                (TokenMatcher::OneOf(lefts), TokenMatcher::OneOf(rights)) => {
                    for left in *lefts {
                        for right in *rights {
                            exact.entry((*left, *right)).or_insert(rule.action);
                        }
                    }
                }

                // Complex matchers need fallback
                _ => {
                    fallback_rules.push(rule);
                }
            }
        }

        RulesMap {
            exact,
            fallback_rules,
        }
    }

    /// Look up the spacing action for a token pair.
    ///
    /// Returns the action from the highest-priority matching rule.
    #[inline]
    pub fn lookup(&self, left: TokenCategory, right: TokenCategory) -> SpaceAction {
        // Try exact lookup first (O(1))
        if let Some(&action) = self.exact.get(&(left, right)) {
            return action;
        }

        // Fall back to linear scan of complex rules
        for rule in &self.fallback_rules {
            if rule.matches(left, right) {
                return rule.action;
            }
        }

        // Default to no space
        SpaceAction::None
    }

    /// Get the number of exact entries in the lookup table.
    pub fn exact_entry_count(&self) -> usize {
        self.exact.len()
    }

    /// Get the number of fallback rules.
    pub fn fallback_rule_count(&self) -> usize {
        self.fallback_rules.len()
    }
}

impl Default for RulesMap {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton for the rules map
static GLOBAL_RULES_MAP: OnceLock<RulesMap> = OnceLock::new();

/// Get the global rules map (lazily initialized).
///
/// This is a singleton to avoid rebuilding the lookup table repeatedly.
pub fn global_rules_map() -> &'static RulesMap {
    GLOBAL_RULES_MAP.get_or_init(RulesMap::new)
}

/// Look up spacing between two token categories using the global rules map.
///
/// This is the primary API for spacing decisions.
#[inline]
pub fn lookup_spacing(left: TokenCategory, right: TokenCategory) -> SpaceAction {
    global_rules_map().lookup(left, right)
}
