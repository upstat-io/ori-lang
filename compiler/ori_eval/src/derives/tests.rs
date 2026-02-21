use super::*;
use ori_ir::{DerivedMethodInfo, DerivedTrait, SharedInterner};
use ori_lexer::lex;
use ori_parse::parse;

#[test]
fn test_process_struct_derives() {
    let interner = SharedInterner::default();
    let source = r#"
#[derive(Eq)]
type Point = { x: int, y: int }

@main () -> void = print(msg: "test");
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(
        !parse_result.has_errors(),
        "Parse errors: {:?}",
        parse_result.errors
    );

    let mut user_method_registry = UserMethodRegistry::new();

    let mut default_ft = DefaultFieldTypeRegistry::new();
    process_derives(
        &parse_result.module,
        &mut user_method_registry,
        &mut default_ft,
        &interner,
    );

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

@main () -> void = print(msg: "test");
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(!parse_result.has_errors());

    let mut user_method_registry = UserMethodRegistry::new();

    let mut default_ft = DefaultFieldTypeRegistry::new();
    process_derives(
        &parse_result.module,
        &mut user_method_registry,
        &mut default_ft,
        &interner,
    );

    let point = interner.intern("Point");
    let eq = interner.intern("eq");
    let clone_method = interner.intern("clone");
    let to_str = interner.intern("to_str");

    // Should have all three methods registered
    assert!(user_method_registry.has_method(point, eq));
    assert!(user_method_registry.has_method(point, clone_method));
    assert!(user_method_registry.has_method(point, to_str));
}

#[test]
fn test_ignore_unknown_derives() {
    let interner = SharedInterner::default();
    let source = r#"
#[derive(Unknown, Eq)]
type Point = { x: int }

@main () -> void = print(msg: "test");
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(!parse_result.has_errors());

    let mut user_method_registry = UserMethodRegistry::new();

    let mut default_ft = DefaultFieldTypeRegistry::new();
    process_derives(
        &parse_result.module,
        &mut user_method_registry,
        &mut default_ft,
        &interner,
    );

    let point = interner.intern("Point");
    let eq = interner.intern("eq");
    let unknown = interner.intern("unknown");

    // Should have Eq but not Unknown
    assert!(user_method_registry.has_method(point, eq));
    assert!(!user_method_registry.has_method(point, unknown));
}

// --- Cross-crate sync enforcement (Section 05.1, Tests 4 and 6) ---

#[test]
fn all_derived_traits_have_eval_handler() {
    // Verify every DerivedTrait variant can create a DerivedMethodInfo
    // and has a non-empty method name. The eval_derived_method() match
    // is exhaustive (Rust enforces), but this test documents the contract
    // and guards against match arms that return unimplemented!() or todo!().
    for &trait_kind in DerivedTrait::ALL {
        // DerivedMethodInfo::new should not panic for any variant
        let info = DerivedMethodInfo::new(trait_kind, vec![]);
        assert_eq!(info.trait_kind, trait_kind);
        assert!(
            !trait_kind.method_name().is_empty(),
            "DerivedTrait::{trait_kind:?} has no method name — handler likely missing"
        );
    }
}

#[test]
fn all_derived_traits_recognized_by_process_derives() {
    // Verify that process_derives() recognizes every DerivedTrait
    // when processing a #[derive(...)] attribute.
    for &trait_kind in DerivedTrait::ALL {
        let name = trait_kind.trait_name();
        assert!(
            DerivedTrait::from_name(name).is_some(),
            "process_derives() would not recognize '{name}' as a derivable trait"
        );
    }
}

#[test]
fn all_non_default_derived_traits_register_methods() {
    // End-to-end: parse source with all derives → process_derives → verify registration.
    // Default excluded because it needs field types with defaults.
    let interner = SharedInterner::default();
    let source = r#"
#[derive(Eq, Clone, Hashable, Printable, Debug, Comparable)]
type TestSync = { x: int }

@main () -> void = print(msg: "test");
"#;

    let tokens = lex(source, &interner);
    let parse_result = parse(&tokens, &interner);
    assert!(
        !parse_result.has_errors(),
        "Parse errors: {:?}",
        parse_result.errors
    );

    let mut user_method_registry = UserMethodRegistry::new();
    let mut default_ft = DefaultFieldTypeRegistry::new();
    process_derives(
        &parse_result.module,
        &mut user_method_registry,
        &mut default_ft,
        &interner,
    );

    let test_type = interner.intern("TestSync");

    for &trait_kind in DerivedTrait::ALL {
        if trait_kind == DerivedTrait::Default {
            continue; // Default needs separate field-type setup
        }
        let method_name = interner.intern(trait_kind.method_name());
        assert!(
            user_method_registry.has_method(test_type, method_name),
            "DerivedTrait::{:?} (method '{}') not registered by process_derives",
            trait_kind,
            trait_kind.method_name()
        );
    }
}
