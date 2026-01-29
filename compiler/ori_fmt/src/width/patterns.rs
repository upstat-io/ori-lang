//! Width calculation for binding patterns.
//!
//! Provides width calculation for destructuring patterns used in let bindings:
//! - Simple names (`foo`)
//! - Wildcards (`_`)
//! - Tuple patterns (`(a, b)`)
//! - Struct patterns (`{ x, y }`)
//! - List patterns with optional rest (`[a, b, ..rest]`)

use super::helpers::COMMA_SEPARATOR_WIDTH;
use ori_ir::{BindingPattern, StringLookup};

/// Calculate width of a binding pattern.
///
/// Recursively calculates width for nested patterns. Includes all
/// delimiters, separators, and shorthand syntax.
pub(super) fn binding_pattern_width<I: StringLookup>(
    pattern: &BindingPattern,
    interner: &I,
) -> usize {
    match pattern {
        BindingPattern::Name(name) => interner.lookup(*name).len(),

        BindingPattern::Wildcard => 1, // "_"

        BindingPattern::Tuple(elements) => {
            if elements.is_empty() {
                return 2; // "()"
            }
            // "(" + elements + ")"
            let mut total = 1;
            for (i, elem) in elements.iter().enumerate() {
                total += binding_pattern_width(elem, interner);
                if i < elements.len() - 1 {
                    total += COMMA_SEPARATOR_WIDTH;
                }
            }
            total + 1
        }

        BindingPattern::Struct { fields } => {
            if fields.is_empty() {
                return 2; // "{}"
            }
            // "{ " + fields + " }"
            let mut total = 2;
            for (i, (name, nested)) in fields.iter().enumerate() {
                let name_w = interner.lookup(*name).len();
                if let Some(pat) = nested {
                    // "name: pattern"
                    total += name_w + 2 + binding_pattern_width(pat, interner);
                } else {
                    // Shorthand: just "name"
                    total += name_w;
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
mod tests {
    use super::*;
    use ori_ir::StringInterner;

    #[test]
    fn test_binding_pattern_name() {
        let interner = StringInterner::new();
        let name = interner.intern("foo");
        let pattern = BindingPattern::Name(name);

        assert_eq!(binding_pattern_width(&pattern, &interner), 3);
    }

    #[test]
    fn test_binding_pattern_wildcard() {
        let interner = StringInterner::new();
        let pattern = BindingPattern::Wildcard;

        assert_eq!(binding_pattern_width(&pattern, &interner), 1);
    }

    #[test]
    fn test_binding_pattern_empty_tuple() {
        let interner = StringInterner::new();
        let pattern = BindingPattern::Tuple(vec![]);

        assert_eq!(binding_pattern_width(&pattern, &interner), 2); // "()"
    }

    #[test]
    fn test_binding_pattern_tuple() {
        let interner = StringInterner::new();
        let a = interner.intern("a");
        let b = interner.intern("b");
        let pattern = BindingPattern::Tuple(vec![BindingPattern::Name(a), BindingPattern::Name(b)]);

        // "(a, b)" = 1 + 1 + 2 + 1 + 1 = 6
        assert_eq!(binding_pattern_width(&pattern, &interner), 6);
    }

    #[test]
    fn test_binding_pattern_nested_tuple() {
        let interner = StringInterner::new();
        let a = interner.intern("a");
        let b = interner.intern("b");
        let inner = BindingPattern::Tuple(vec![BindingPattern::Name(a), BindingPattern::Name(b)]);
        let pattern = BindingPattern::Tuple(vec![inner, BindingPattern::Wildcard]);

        // "((a, b), _)" = 1 + 6 + 2 + 1 + 1 = 11
        assert_eq!(binding_pattern_width(&pattern, &interner), 11);
    }

    #[test]
    fn test_binding_pattern_empty_struct() {
        let interner = StringInterner::new();
        let pattern = BindingPattern::Struct { fields: vec![] };

        assert_eq!(binding_pattern_width(&pattern, &interner), 2); // "{}"
    }

    #[test]
    fn test_binding_pattern_struct_shorthand() {
        let interner = StringInterner::new();
        let x = interner.intern("x");
        let y = interner.intern("y");
        let pattern = BindingPattern::Struct {
            fields: vec![(x, None), (y, None)],
        };

        // "{ x, y }" = 2 + 1 + 2 + 1 + 2 = 8
        assert_eq!(binding_pattern_width(&pattern, &interner), 8);
    }

    #[test]
    fn test_binding_pattern_struct_with_rename() {
        let interner = StringInterner::new();
        let x = interner.intern("x");
        let a = interner.intern("a");
        let pattern = BindingPattern::Struct {
            fields: vec![(x, Some(BindingPattern::Name(a)))],
        };

        // "{ x: a }" = 2 + 1 + 2 + 1 + 2 = 8
        assert_eq!(binding_pattern_width(&pattern, &interner), 8);
    }

    #[test]
    fn test_binding_pattern_empty_list() {
        let interner = StringInterner::new();
        let pattern = BindingPattern::List {
            elements: vec![],
            rest: None,
        };

        assert_eq!(binding_pattern_width(&pattern, &interner), 2); // "[]"
    }

    #[test]
    fn test_binding_pattern_list() {
        let interner = StringInterner::new();
        let a = interner.intern("a");
        let b = interner.intern("b");
        let pattern = BindingPattern::List {
            elements: vec![BindingPattern::Name(a), BindingPattern::Name(b)],
            rest: None,
        };

        // "[a, b]" = 1 + 1 + 2 + 1 + 1 = 6
        assert_eq!(binding_pattern_width(&pattern, &interner), 6);
    }

    #[test]
    fn test_binding_pattern_list_with_rest() {
        let interner = StringInterner::new();
        let a = interner.intern("a");
        let rest_name = interner.intern("rest");
        let pattern = BindingPattern::List {
            elements: vec![BindingPattern::Name(a)],
            rest: Some(rest_name),
        };

        // "[a, ..rest]" = 1 + 1 + 2 + 2 + 4 + 1 = 11
        assert_eq!(binding_pattern_width(&pattern, &interner), 11);
    }

    #[test]
    fn test_binding_pattern_list_only_rest() {
        let interner = StringInterner::new();
        let rest_name = interner.intern("xs");
        let pattern = BindingPattern::List {
            elements: vec![],
            rest: Some(rest_name),
        };

        // "[..xs]" = 1 + 2 + 2 + 1 = 6
        assert_eq!(binding_pattern_width(&pattern, &interner), 6);
    }
}
