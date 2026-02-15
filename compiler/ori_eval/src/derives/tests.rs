use super::*;
use ori_ir::SharedInterner;
use ori_lexer::lex;
use ori_parse::parse;

#[test]
fn test_process_struct_derives() {
    let interner = SharedInterner::default();
    let source = r#"
#[derive(Eq)]
type Point = { x: int, y: int }

@main () -> void = print(msg: "test")
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(
        !parse_result.has_errors(),
        "Parse errors: {:?}",
        parse_result.errors
    );

    let mut user_method_registry = UserMethodRegistry::new();

    process_derives(&parse_result.module, &mut user_method_registry, &interner);

    let point = interner.intern("Point");
    let eq = interner.intern("eq");

    // Should have registered an eq method for Point
    assert!(user_method_registry.has_method(point, eq));

    let info = user_method_registry.lookup_derived(point, eq).unwrap();
    assert_eq!(info.trait_kind, DerivedTrait::Eq);
    assert_eq!(info.field_names.len(), 2);
}

#[test]
fn test_process_multiple_derives() {
    let interner = SharedInterner::default();
    let source = r#"
#[derive(Eq, Clone, Printable)]
type Point = { x: int, y: int }

@main () -> void = print(msg: "test")
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(!parse_result.has_errors());

    let mut user_method_registry = UserMethodRegistry::new();

    process_derives(&parse_result.module, &mut user_method_registry, &interner);

    let point = interner.intern("Point");
    let eq = interner.intern("eq");
    let clone_method = interner.intern("clone");
    let to_string = interner.intern("to_string");

    // Should have all three methods registered
    assert!(user_method_registry.has_method(point, eq));
    assert!(user_method_registry.has_method(point, clone_method));
    assert!(user_method_registry.has_method(point, to_string));
}

#[test]
fn test_ignore_unknown_derives() {
    let interner = SharedInterner::default();
    let source = r#"
#[derive(Unknown, Eq)]
type Point = { x: int }

@main () -> void = print(msg: "test")
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(!parse_result.has_errors());

    let mut user_method_registry = UserMethodRegistry::new();

    process_derives(&parse_result.module, &mut user_method_registry, &interner);

    let point = interner.intern("Point");
    let eq = interner.intern("eq");
    let unknown = interner.intern("unknown");

    // Should have Eq but not Unknown
    assert!(user_method_registry.has_method(point, eq));
    assert!(!user_method_registry.has_method(point, unknown));
}
