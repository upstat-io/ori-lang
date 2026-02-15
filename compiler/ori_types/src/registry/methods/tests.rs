use super::*;

fn test_name(s: &str) -> Name {
    // Must match static_name() hash algorithm
    Name::from_raw(s.as_bytes().iter().fold(0u32, |acc, &b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    }))
}

#[test]
fn builtin_methods_registered() {
    let registry = MethodRegistry::new();

    // Check list methods
    assert!(registry.has_builtin(Tag::List, test_name("len")));
    assert!(registry.has_builtin(Tag::List, test_name("is_empty")));
    assert!(registry.has_builtin(Tag::List, test_name("first")));

    // Check string methods
    assert!(registry.has_builtin(Tag::Str, test_name("len")));
    assert!(registry.has_builtin(Tag::Str, test_name("trim")));
    assert!(registry.has_builtin(Tag::Str, test_name("to_uppercase")));

    // Check int methods
    assert!(registry.has_builtin(Tag::Int, test_name("abs")));
    assert!(registry.has_builtin(Tag::Int, test_name("to_float")));

    // Check float methods
    assert!(registry.has_builtin(Tag::Float, test_name("abs")));
    assert!(registry.has_builtin(Tag::Float, test_name("sqrt")));
    assert!(registry.has_builtin(Tag::Float, test_name("sin")));
}

#[test]
fn fixed_return_type() {
    let registry = MethodRegistry::new();
    let mut pool = Pool::new();

    let method = registry
        .get_builtin(Tag::Int, test_name("abs"))
        .expect("abs should exist");

    let ret = registry.builtin_return_type(&mut pool, Idx::INT, method);
    assert_eq!(ret, Idx::INT);
}

#[test]
fn element_return_type() {
    let registry = MethodRegistry::new();
    let mut pool = Pool::new();

    // Create option[int]
    let option_int = pool.option(Idx::INT);

    let method = registry
        .get_builtin(Tag::Option, test_name("unwrap"))
        .expect("unwrap should exist");

    let ret = registry.builtin_return_type(&mut pool, option_int, method);
    assert_eq!(ret, Idx::INT);
}

#[test]
fn wrap_option_transform() {
    let registry = MethodRegistry::new();
    let mut pool = Pool::new();

    // Create [int]
    let list_int = pool.list(Idx::INT);

    let method = registry
        .get_builtin(Tag::List, test_name("first"))
        .expect("first should exist");

    let ret = registry.builtin_return_type(&mut pool, list_int, method);

    // Should return option[int]
    assert_eq!(pool.tag(ret), Tag::Option);
    let inner = Idx::from_raw(pool.data(ret));
    assert_eq!(inner, Idx::INT);
}

#[test]
fn builtin_methods_for_tag() {
    let registry = MethodRegistry::new();

    let list_methods: Vec<_> = registry.builtin_methods_for_tag(Tag::List).collect();
    assert!(!list_methods.is_empty());

    let names: Vec<_> = list_methods.iter().map(|m| m.name).collect();
    assert!(names.contains(&test_name("len")));
    assert!(names.contains(&test_name("first")));
}
