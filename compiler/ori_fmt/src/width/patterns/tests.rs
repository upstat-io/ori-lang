use super::*;
use ori_ir::{FieldBinding, StringInterner};

#[test]
fn test_binding_pattern_name() {
    let interner = StringInterner::new();
    let name = interner.intern("foo");
    let pattern = BindingPattern::Name {
        name,
        mutable: true,
    };

    assert_eq!(binding_pattern_width(&pattern, &interner), 3);
}

#[test]
fn test_binding_pattern_immutable_name() {
    let interner = StringInterner::new();
    let name = interner.intern("foo");
    let pattern = BindingPattern::Name {
        name,
        mutable: false,
    };

    // "$foo" = 4
    assert_eq!(binding_pattern_width(&pattern, &interner), 4);
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
    let pattern = BindingPattern::Tuple(vec![
        BindingPattern::Name {
            name: a,
            mutable: true,
        },
        BindingPattern::Name {
            name: b,
            mutable: true,
        },
    ]);

    // "(a, b)" = 1 + 1 + 2 + 1 + 1 = 6
    assert_eq!(binding_pattern_width(&pattern, &interner), 6);
}

#[test]
fn test_binding_pattern_nested_tuple() {
    let interner = StringInterner::new();
    let a = interner.intern("a");
    let b = interner.intern("b");
    let inner = BindingPattern::Tuple(vec![
        BindingPattern::Name {
            name: a,
            mutable: true,
        },
        BindingPattern::Name {
            name: b,
            mutable: true,
        },
    ]);
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
        fields: vec![
            FieldBinding {
                name: x,
                mutable: true,
                pattern: None,
            },
            FieldBinding {
                name: y,
                mutable: true,
                pattern: None,
            },
        ],
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
        fields: vec![FieldBinding {
            name: x,
            mutable: true,
            pattern: Some(BindingPattern::Name {
                name: a,
                mutable: true,
            }),
        }],
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
        elements: vec![
            BindingPattern::Name {
                name: a,
                mutable: true,
            },
            BindingPattern::Name {
                name: b,
                mutable: true,
            },
        ],
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
        elements: vec![BindingPattern::Name {
            name: a,
            mutable: true,
        }],
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
