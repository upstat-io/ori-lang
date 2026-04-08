use super::*;

use ori_lexer::lex;
use ori_parse::parse;

fn parse_and_check(source: &str) -> (TypeCheckResult, StringInterner) {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let result = check_module(&parsed.module, &parsed.arena, &interner);
    (result, interner)
}

#[test]
fn check_empty_module() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let module = Module::default();

    let result = check_module(&module, &arena, &interner);

    assert!(!result.has_errors());
    assert!(result.typed.functions.is_empty());
}

#[test]
fn check_module_with_pool_returns_pool() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let module = Module::default();

    let (result, pool) = check_module_with_pool(&module, &arena, &interner);

    assert!(!result.has_errors());
    // Pool should have pre-interned primitives
    assert_eq!(pool.tag(crate::Idx::INT), crate::Tag::Int);
}

// Regression: end-to-end test for trait_impl_fn_names plumbing.
// Verifies that the real TypeChecker → TypedModule pipeline correctly populates
// trait_impl_fn_names for trait impl methods, while excluding inherent methods.

/// Positive pin: trait impl methods appear in `trait_impl_fn_names`.
#[test]
fn trait_impl_method_registered_in_trait_impl_fn_names() {
    let (result, interner) = parse_and_check(
        r"
        trait Greet {
            @greet (self) -> str
        }

        type Person = { name: str }

        impl Greet for Person {
            @greet (self) -> str = self.name;
        }
        ",
    );

    // The type checker may report errors for missing prelude types, but the
    // trait_impl_fn_names plumbing should still work.
    let greet_name = interner.intern("greet");
    let names = &result.typed.trait_impl_fn_names;
    assert!(
        names.iter().any(|(_, n)| *n == greet_name),
        "trait impl method 'greet' should be in trait_impl_fn_names, got: {:?}",
        names
            .iter()
            .map(|(_, n)| interner.lookup(*n))
            .collect::<Vec<_>>()
    );
}

/// Negative pin: inherent impl methods do NOT appear in `trait_impl_fn_names`.
#[test]
fn inherent_impl_method_excluded_from_trait_impl_fn_names() {
    let (result, interner) = parse_and_check(
        r"
        type Counter = { value: int }

        impl Counter {
            @increment (self) -> Counter = Counter { value: self.value + 1 }
        }
        ",
    );

    let increment_name = interner.intern("increment");
    let names = &result.typed.trait_impl_fn_names;
    assert!(
        !names.iter().any(|(_, n)| *n == increment_name),
        "inherent impl method 'increment' should NOT be in trait_impl_fn_names, got: {:?}",
        names
            .iter()
            .map(|(_, n)| interner.lookup(*n))
            .collect::<Vec<_>>()
    );
}

/// Positive pin: unoverridden default trait methods are registered.
#[test]
fn default_trait_method_registered_in_trait_impl_fn_names() {
    let (result, interner) = parse_and_check(
        r#"
        trait Describable {
            @describe (self) -> str = "unknown";
        }

        type Widget = { id: int }

        impl Describable for Widget {
        }
        "#,
    );

    let describe_name = interner.intern("describe");
    let names = &result.typed.trait_impl_fn_names;
    assert!(
        names.iter().any(|(_, n)| *n == describe_name),
        "default trait method 'describe' should be in trait_impl_fn_names, got: {:?}",
        names
            .iter()
            .map(|(_, n)| interner.lookup(*n))
            .collect::<Vec<_>>()
    );
}

/// Combined test: trait impl vs inherent impl in the same module.
/// Verifies correct discrimination when both exist on the same type.
#[test]
fn trait_impl_fn_names_discriminates_trait_from_inherent() {
    let (result, interner) = parse_and_check(
        r#"
        trait Printable {
            @to_str (self) -> str
        }

        type Point = { x: int, y: int }

        impl Point {
            @distance (self) -> int = self.x + self.y;
        }

        impl Printable for Point {
            @to_str (self) -> str = "point";
        }
        "#,
    );

    let to_str_name = interner.intern("to_str");
    let distance_name = interner.intern("distance");
    let names = &result.typed.trait_impl_fn_names;

    assert!(
        names.iter().any(|(_, n)| *n == to_str_name),
        "trait impl method 'to_str' should be in trait_impl_fn_names"
    );
    assert!(
        !names.iter().any(|(_, n)| *n == distance_name),
        "inherent impl method 'distance' should NOT be in trait_impl_fn_names"
    );
}
