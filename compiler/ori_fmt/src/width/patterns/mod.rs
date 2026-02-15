//! Width calculation for binding patterns.
//!
//! Provides width calculation for destructuring patterns used in let bindings:
//! - Simple names (`foo`) or immutable (`$foo`)
//! - Wildcards (`_`)
//! - Tuple patterns (`(a, b)`) or `($a, $b)`
//! - Struct patterns (`{ x, y }`) or `{ $x, y }`
//! - List patterns with optional rest (`[a, b, ..rest]`)

use super::helpers::COMMA_SEPARATOR_WIDTH;
use ori_ir::{BindingPattern, StringLookup};

/// Calculate width of a binding pattern.
///
/// Recursively calculates width for nested patterns. Includes all
/// delimiters, separators, `$` prefixes, and shorthand syntax.
pub(super) fn binding_pattern_width<I: StringLookup>(
    pattern: &BindingPattern,
    interner: &I,
) -> usize {
    match pattern {
        BindingPattern::Name { name, mutable } => {
            let prefix = usize::from(!*mutable); // "$"
            prefix + interner.lookup(*name).len()
        }

        BindingPattern::Wildcard => 1, // "_"

        BindingPattern::Tuple(elements) => {
            if elements.is_empty() {
                return 2; // "()"
            }
            // "(" + elements + ")" + optional trailing comma for single element
            let mut total = 1;
            for (i, elem) in elements.iter().enumerate() {
                total += binding_pattern_width(elem, interner);
                if i < elements.len() - 1 {
                    total += COMMA_SEPARATOR_WIDTH;
                }
            }
            // Single-element tuples need trailing comma: (x,)
            if elements.len() == 1 {
                total += 1;
            }
            total + 1
        }

        BindingPattern::Struct { fields } => {
            if fields.is_empty() {
                return 2; // "{}"
            }
            // "{ " + fields + " }"
            let mut total = 2;
            for (i, field) in fields.iter().enumerate() {
                let name_w = interner.lookup(field.name).len();
                // Shorthand with $ prefix adds 1 for "$"
                let dollar_w = usize::from(!field.mutable && field.pattern.is_none());
                if let Some(pat) = &field.pattern {
                    // "name: pattern"
                    total += name_w + 2 + binding_pattern_width(pat, interner);
                } else {
                    // Shorthand: just "name" or "$name"
                    total += dollar_w + name_w;
                }
                if i < fields.len() - 1 {
                    total += COMMA_SEPARATOR_WIDTH;
                }
            }
            total + 2 // " }"
        }

        BindingPattern::List { elements, rest } => {
            // "[" + elements + "]"
            let mut total = 1;
            for (i, elem) in elements.iter().enumerate() {
                total += binding_pattern_width(elem, interner);
                if i < elements.len() - 1 {
                    total += COMMA_SEPARATOR_WIDTH;
                }
            }
            if let Some(rest_name) = rest {
                if !elements.is_empty() {
                    total += COMMA_SEPARATOR_WIDTH;
                }
                // "..rest"
                total += 2 + interner.lookup(*rest_name).len();
            }
            total + 1 // "]"
        }
    }
}

#[cfg(test)]
mod tests;
