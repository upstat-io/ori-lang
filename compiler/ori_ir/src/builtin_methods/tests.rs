use super::*;

#[test]
fn test_find_method() {
    let Some(method) = find_method(BuiltinType::Int, "compare") else {
        panic!("int.compare should exist");
    };
    assert_eq!(method.name, "compare");
    assert_eq!(method.receiver, BuiltinType::Int);
    assert_eq!(method.trait_name, Some("Comparable"));
}

#[test]
fn test_find_method_not_found() {
    assert!(find_method(BuiltinType::Int, "nonexistent").is_none());
}

#[test]
fn test_methods_for_int() {
    let methods: Vec<_> = methods_for(BuiltinType::Int).collect();
    assert!(methods.len() > 5); // Should have compare, equals, clone, hash, to_str, etc.

    let names: Vec<_> = methods.iter().map(|m| m.name).collect();
    assert!(names.contains(&"compare"));
    assert!(names.contains(&"add"));
    assert!(names.contains(&"abs"));
}

#[test]
fn test_methods_for_ordering() {
    let methods: Vec<_> = methods_for(BuiltinType::Ordering).collect();
    let names: Vec<_> = methods.iter().map(|m| m.name).collect();

    assert!(names.contains(&"is_less"));
    assert!(names.contains(&"is_equal"));
    assert!(names.contains(&"is_greater"));
    assert!(names.contains(&"reverse"));
    assert!(names.contains(&"compare"));
}

#[test]
fn test_has_method() {
    assert!(has_method(BuiltinType::Duration, "nanoseconds"));
    assert!(has_method(BuiltinType::Duration, "hours"));
    assert!(!has_method(BuiltinType::Duration, "days")); // Not a method
}

#[test]
fn test_all_comparable_types_have_compare() {
    for builtin in [
        BuiltinType::Int,
        BuiltinType::Float,
        BuiltinType::Bool,
        BuiltinType::Char,
        BuiltinType::Byte,
        BuiltinType::Str,
        BuiltinType::Duration,
        BuiltinType::Size,
        BuiltinType::Ordering,
    ] {
        assert!(
            has_method(builtin, "compare"),
            "{builtin:?} should have compare method"
        );
    }
}

#[test]
fn test_return_types() {
    let Some(compare) = find_method(BuiltinType::Int, "compare") else {
        panic!("int.compare should exist");
    };
    assert_eq!(compare.returns, ReturnSpec::Type(BuiltinType::Ordering));

    let Some(abs) = find_method(BuiltinType::Int, "abs") else {
        panic!("int.abs should exist");
    };
    assert_eq!(abs.returns, ReturnSpec::SelfType);

    let Some(len) = find_method(BuiltinType::Str, "len") else {
        panic!("str.len should exist");
    };
    assert_eq!(len.returns, ReturnSpec::Type(BuiltinType::Int));
}
