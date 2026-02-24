//! Tests for `BindingPattern::force_immutable`.

use super::{BindingPattern, FieldBinding, Mutability};
use crate::Name;

fn name(n: u32) -> Name {
    Name::new(0, n)
}

#[test]
fn force_immutable_simple_mutable() {
    let mut pat = BindingPattern::Name {
        name: name(1),
        mutable: Mutability::Mutable,
    };
    pat.force_immutable();
    assert_eq!(
        pat,
        BindingPattern::Name {
            name: name(1),
            mutable: Mutability::Immutable,
        }
    );
}

#[test]
fn force_immutable_already_immutable() {
    let mut pat = BindingPattern::Name {
        name: name(1),
        mutable: Mutability::Immutable,
    };
    pat.force_immutable();
    assert_eq!(
        pat,
        BindingPattern::Name {
            name: name(1),
            mutable: Mutability::Immutable,
        }
    );
}

#[test]
fn force_immutable_wildcard_noop() {
    let mut pat = BindingPattern::Wildcard;
    pat.force_immutable();
    assert_eq!(pat, BindingPattern::Wildcard);
}

#[test]
fn force_immutable_tuple_recursive() {
    let mut pat = BindingPattern::Tuple(vec![
        BindingPattern::Name {
            name: name(1),
            mutable: Mutability::Mutable,
        },
        BindingPattern::Name {
            name: name(2),
            mutable: Mutability::Mutable,
        },
    ]);
    pat.force_immutable();
    assert_eq!(
        pat,
        BindingPattern::Tuple(vec![
            BindingPattern::Name {
                name: name(1),
                mutable: Mutability::Immutable,
            },
            BindingPattern::Name {
                name: name(2),
                mutable: Mutability::Immutable,
            },
        ])
    );
}

#[test]
fn force_immutable_struct_shorthand() {
    let mut pat = BindingPattern::Struct {
        fields: vec![FieldBinding {
            name: name(1),
            mutable: Mutability::Mutable,
            pattern: None,
        }],
    };
    pat.force_immutable();
    let BindingPattern::Struct { fields } = &pat else {
        panic!("expected Struct");
    };
    assert_eq!(fields[0].mutable, Mutability::Immutable);
}

#[test]
fn force_immutable_struct_explicit_pattern() {
    let mut pat = BindingPattern::Struct {
        fields: vec![FieldBinding {
            name: name(1),
            mutable: Mutability::Mutable,
            pattern: Some(BindingPattern::Name {
                name: name(2),
                mutable: Mutability::Mutable,
            }),
        }],
    };
    pat.force_immutable();
    let BindingPattern::Struct { fields } = &pat else {
        panic!("expected Struct");
    };
    assert_eq!(fields[0].mutable, Mutability::Immutable);
    assert_eq!(
        fields[0].pattern,
        Some(BindingPattern::Name {
            name: name(2),
            mutable: Mutability::Immutable,
        })
    );
}

#[test]
fn force_immutable_list_with_rest() {
    let mut pat = BindingPattern::List {
        elements: vec![BindingPattern::Name {
            name: name(1),
            mutable: Mutability::Mutable,
        }],
        rest: Some((name(2), Mutability::Mutable)),
    };
    pat.force_immutable();
    let BindingPattern::List { elements, rest } = &pat else {
        panic!("expected List");
    };
    assert_eq!(
        elements[0],
        BindingPattern::Name {
            name: name(1),
            mutable: Mutability::Immutable,
        }
    );
    assert_eq!(*rest, Some((name(2), Mutability::Immutable)));
}
