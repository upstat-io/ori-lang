//! Spec: grammar.ebnf:312 `trait_impl` — subject-first colon form `impl Type: Trait`
//! (approved `docs/ori_lang/proposals/approved/impl-colon-syntax-proposal.md`).
//! Covers colon-form parsing + E1019 rejection of the removed `impl Trait for Type` form.

use crate::{parse, ParseOutput};
use ori_diagnostic::ErrorCode;
use ori_ir::StringInterner;

fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    parse(&tokens, &interner)
}

#[test]
fn test_parse_def_impl_basic() {
    let source = r#"
def impl Http {
@get (url: str) -> str = "response";
}
"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.def_impls.len(), 1);

    let def_impl = &output.module.def_impls[0];
    assert_eq!(def_impl.methods.len(), 1);
    assert!(!def_impl.visibility.is_public());
}

#[test]
fn test_parse_def_impl_public() {
    let source = r#"
pub def impl Http {
@get (url: str) -> str = "response";
}
"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.def_impls.len(), 1);
    assert!(output.module.def_impls[0].visibility.is_public());
}

#[test]
fn test_parse_def_impl_multiple_methods() {
    let source = r#"
def impl Http {
@get (url: str) -> str = "get";
@post (url: str, body: str) -> str = "post";
@delete (url: str) -> void = ();
}
"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.def_impls.len(), 1);
    assert_eq!(output.module.def_impls[0].methods.len(), 3);
}

#[test]
fn test_parse_def_impl_empty() {
    // Empty def impl is valid (though semantically useless)
    let source = r"
def impl Http {
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.def_impls.len(), 1);
    assert_eq!(output.module.def_impls[0].methods.len(), 0);
}

#[test]
fn test_parse_multiple_def_impls() {
    let source = r#"
pub def impl Http {
@get (url: str) -> str = "response";
}

def impl FileSystem {
@read (path: str) -> str = "content";
}
"#;
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.def_impls.len(), 2);
}

// Colon trait_impl (`impl Type: Trait`) — the sole trait-impl form mandated by
// grammar.ebnf:312 trait_impl. The `impl Trait for Type` form has no grammar
// production and is rejected (E1019). `[T]`/list/tuple impl subjects are out of
// scope (parse_impl_type is path-only).

#[test]
fn test_parse_colon_trait_impl_named_subject_records_trait_and_self() {
    let source = r"
impl Point: Eq {
@equals (self, other: Self) -> bool = true;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    let imp = &output.module.impls[0];
    assert!(imp.is_trait_impl(), "colon form must record a trait_path");
    // subject-first: self_path is the pre-colon type, trait_path the post-colon trait
    assert_eq!(
        imp.self_path.len(),
        1,
        "self_path should be the subject `Point`"
    );
    assert_eq!(
        imp.trait_path.as_ref().map(Vec::len),
        Some(1),
        "trait_path should be the post-colon trait `Eq`"
    );
    assert_eq!(imp.methods.len(), 1);
}

#[test]
fn test_parse_colon_trait_impl_with_trait_type_args_extracts_from_post_colon_trait() {
    // trait_type_args (`<int>`) must come from the POST-COLON trait, not the subject.
    let source = r"
impl Vector2: Add<int> {
@add (self, other: Self) -> Self = self;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    let imp = &output.module.impls[0];
    assert!(imp.is_trait_impl());
    assert!(
        !imp.trait_type_args.is_empty(),
        "type args must be extracted from the post-colon trait `Add<int>`"
    );
}

#[test]
fn test_parse_colon_trait_impl_with_generic_named_subject_parses() {
    // Generics parse before the subject; subject is the generic named type `Box<T>`.
    let source = r"
impl<T: Clone> Box<T>: Clone {
@clone (self) -> Self = self;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    let imp = &output.module.impls[0];
    assert!(imp.is_trait_impl());
    assert!(
        !imp.generics.is_empty(),
        "impl-level generics `<T: Clone>` must parse"
    );
}

#[test]
fn test_parse_colon_trait_impl_with_where_clause_parses() {
    // The colon branch returns BEFORE the existing optional where_clause parse,
    // so the where-clause still parses after the colon-form trait.
    let source = r"
impl<T> Box<T>: Clone where T: Clone {
@clone (self) -> Self = self;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    let imp = &output.module.impls[0];
    assert!(imp.is_trait_impl());
    assert!(
        !imp.where_clauses.is_empty(),
        "where-clause must parse after the colon-form trait"
    );
}

#[test]
fn test_parse_colon_trait_impl_with_dotted_trait_path_captures_all_segments() {
    // Multi-segment trait path after `:` (parse_impl_type's parse_type_path
    // handles dotted paths).
    let source = r"
impl Point: some_mod.Trait {
@m (self) -> bool = true;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    let imp = &output.module.impls[0];
    assert_eq!(
        imp.trait_path.as_ref().map(Vec::len),
        Some(2),
        "dotted trait path `some_mod.Trait` should capture both segments"
    );
}

#[test]
fn test_parse_colon_trait_impl_multi_method_body_parses() {
    let source = r"
impl Point: Shape {
type Unit = int;
@area (self) -> int = 0;
@perimeter (self) -> int = 0;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    let imp = &output.module.impls[0];
    assert_eq!(imp.methods.len(), 2);
    assert_eq!(imp.assoc_types.len(), 1);
}

#[test]
fn test_parse_two_colon_trait_impls_each_record_own_trait_and_self() {
    // Two colon impls in one module each record their own subject + trait.
    let source = r"
impl Point: Eq {
@equals (self, other: Self) -> bool = true;
}

impl Line: Eq {
@equals (self, other: Self) -> bool = true;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 2);
    assert!(output.module.impls[0].is_trait_impl());
    assert!(output.module.impls[1].is_trait_impl());
    // Distinct subjects, same trait.
    assert_ne!(
        output.module.impls[0].self_path, output.module.impls[1].self_path,
        "the two impls have distinct subjects (Point vs Line)"
    );
    assert_eq!(
        output.module.impls[0].trait_path, output.module.impls[1].trait_path,
        "both impls target the same trait (Eq)"
    );
}

#[test]
fn test_parse_for_form_trait_impl_rejected_with_migration_error() {
    // Negative pin: the removed `impl Trait for Type` form is rejected with the
    // E1019 migration diagnostic (grammar.ebnf:312 has no `for`-form production).
    let source = r"
impl Eq for Point {
@equals (self, other: Self) -> bool = true;
}
";
    let output = parse_source(source);
    assert!(
        output.errors.iter().any(|e| e.code() == ErrorCode::E1019),
        "expected E1019 for the removed `impl Trait for Type` form, got: {:?}",
        output
            .errors
            .iter()
            .map(crate::ParseError::code)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_parse_inherent_impl_still_parses() {
    // Inherent impl (no `for`, no `:`) must stay unchanged.
    let source = r"
impl Point {
@new () -> Point = Point { x: 0 };
}
";
    let output = parse_source(source);
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    assert_eq!(output.module.impls.len(), 1);
    assert!(
        output.module.impls[0].is_inherent(),
        "inherent impl must have no trait_path"
    );
}

#[test]
fn test_parse_colon_trait_impl_without_trait_path_errors_e1002() {
    // Negative pin: `impl Point: { }` (colon, no trait path) must error cleanly
    // with E1002 via require! on the post-colon parse_impl_type — NOT fall into
    // the inherent branch, NOT ICE. (Pre-fix this errors with E1001 from the
    // inherent-fallthrough expect(LBrace); the fix routes it to E1002.)
    let source = r"
impl Point: {
}
";
    let output = parse_source(source);
    assert!(
        output.errors.iter().any(|e| e.code() == ErrorCode::E1002),
        "expected E1002 for a colon impl with no trait path, got: {:?}",
        output
            .errors
            .iter()
            .map(crate::ParseError::code)
            .collect::<Vec<_>>()
    );
}
