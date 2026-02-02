//! Token Spacing Rules (Layer 1)
//!
//! Declarative O(1) lookup for spacing between adjacent tokens.
//!
//! # Architecture
//!
//! This module implements the first layer of the 5-layer formatter architecture:
//!
//! 1. **`SpaceAction`**: What spacing to emit (none, space, newline, preserve)
//! 2. **`TokenCategory`**: Abstract token types for matching (ignores literal values)
//! 3. **`SpaceRule`**: A declarative rule matching left/right tokens to an action
//! 4. **`RulesMap`**: O(1) lookup table for rule evaluation
//!
//! # Usage
//!
//! ```ignore
//! let rules_map = RulesMap::new();
//! let action = rules_map.lookup(
//!     TokenCategory::Ident,
//!     TokenCategory::Plus,
//!     &ctx,
//! );
//! ```
//!
//! # Spec Reference
//!
//! - Lines 25-47: Spacing table
//! - Lines 902-936: Comment normalization

mod action;
mod category;
mod lookup;
mod matcher;
mod rules;

pub use action::SpaceAction;
pub use category::TokenCategory;
pub use lookup::{global_rules_map, lookup_spacing, RulesMap};
pub use matcher::TokenMatcher;
pub use rules::{find_rule, rule_count, spacing_between, SpaceRule, SPACE_RULES};

#[cfg(test)]
mod tests;
