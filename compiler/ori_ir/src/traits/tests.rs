use super::*;
use crate::{Name, Token, TokenKind};

#[test]
fn test_spanned_trait() {
    let token = Token::new(TokenKind::Int(42), Span::new(0, 2));
    assert_eq!(token.span().start, 0);
    assert_eq!(token.span().end, 2);
}

// Mock struct for testing Named trait
struct MockNamed {
    name: Name,
}

impl Named for MockNamed {
    fn name(&self) -> Name {
        self.name
    }
}

#[test]
#[expect(clippy::cast_possible_truncation, reason = "Test values fit in u32")]
fn test_named_trait() {
    let item = MockNamed {
        name: Name::new(1, 42),
    };
    assert_eq!(item.name().shard() as u32, 1);
    assert_eq!(item.name().local() as u32, 42);
}

#[test]
fn test_named_trait_equality() {
    let item1 = MockNamed {
        name: Name::new(0, 10),
    };
    let item2 = MockNamed {
        name: Name::new(0, 10),
    };
    let item3 = MockNamed {
        name: Name::new(0, 20),
    };

    assert_eq!(item1.name(), item2.name());
    assert_ne!(item1.name(), item3.name());
}

// Mock struct for testing Typed trait
struct MockTypedItem<T> {
    ty: T,
}

impl<T> Typed<T> for MockTypedItem<T> {
    fn ty(&self) -> &T {
        &self.ty
    }
}

#[test]
fn test_typed_trait() {
    let item = MockTypedItem { ty: "int" };
    assert_eq!(*item.ty(), "int");
}

#[test]
fn test_typed_trait_with_complex_type() {
    #[derive(Debug, PartialEq)]
    struct ComplexType {
        name: &'static str,
        is_generic: bool,
    }

    let item = MockTypedItem {
        ty: ComplexType {
            name: "Option",
            is_generic: true,
        },
    };

    assert_eq!(item.ty().name, "Option");
    assert!(item.ty().is_generic);
}

#[test]
fn test_spanned_trait_via_dyn() {
    let token = Token::new(TokenKind::True, Span::new(10, 14));

    // Test using dynamic dispatch
    let spanned: &dyn Spanned = &token;
    assert_eq!(spanned.span().start, 10);
    assert_eq!(spanned.span().end, 14);
    assert_eq!(spanned.span().len(), 4);
}
