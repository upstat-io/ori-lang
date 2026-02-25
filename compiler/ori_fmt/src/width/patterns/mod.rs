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

/// Calculate width of a binding pattern (for `let` bindings — includes `$` for immutable).
///
/// Recursively calculates width for nested patterns. Includes all
/// delimiters, separators, `$` prefixes, and shorthand syntax.
pub(super) fn binding_pattern_width<I: StringLookup>(
    pattern: &BindingPattern,
    interner: &I,
) -> usize {
    binding_pattern_width_inner(pattern, interner, false)
}

/// Calculate width of a for-loop binding pattern — excludes `$` prefix.
///
/// Per spec (§05-variables.md), for-loop variables are inherently immutable.
/// The formatter never emits `$` in for-loop position, so the width calculation
/// must match by not counting the `$` prefix.
pub(super) fn for_binding_pattern_width<I: StringLookup>(
    pattern: &BindingPattern,
    interner: &I,
) -> usize {
    binding_pattern_width_inner(pattern, interner, true)
}

/// Shared width calculator with optional `$` suppression.
fn binding_pattern_width_inner<I: StringLookup>(
    pattern: &BindingPattern,
    interner: &I,
    suppress_dollar: bool,
) -> usize {
    match pattern {
        BindingPattern::Name { name, mutable } => {
            let prefix = if suppress_dollar {
                0
            } else {
                usize::from(mutable.is_immutable())
            };
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
                total += binding_pattern_width_inner(elem, interner, suppress_dollar);
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
                let dollar_w = if suppress_dollar {
                    0
                } else {
                    usize::from(field.mutable.is_immutable() && field.pattern.is_none())
                };
                if let Some(pat) = &field.pattern {
                    // "name: pattern"
                    total +=
                        name_w + 2 + binding_pattern_width_inner(pat, interner, suppress_dollar);
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
                total += binding_pattern_width_inner(elem, interner, suppress_dollar);
                if i < elements.len() - 1 {
                    total += COMMA_SEPARATOR_WIDTH;
                }
            }
            if let Some((rest_name, rest_mut)) = rest {
                if !elements.is_empty() {
                    total += COMMA_SEPARATOR_WIDTH;
                }
                // "..$rest" or "..rest"
                total += 2 + interner.lookup(*rest_name).len();
                if !suppress_dollar && rest_mut.is_immutable() {
                    total += 1; // "$" prefix
                }
            }
            total + 1 // "]"
        }
    }
}

#[cfg(test)]
mod tests;
