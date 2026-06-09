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

// Colon trait_impl (`impl Type: Trait`) — the sole trait-impl form per grammar.ebnf:312;
// the `impl Trait for Type` form is rejected (E1019). List/tuple impl subjects are out of
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
    // Negative pin: `impl Point: { }` (colon, no trait path) errors E1002 via
    // require! on the post-colon parse_impl_type — not the inherent branch, no ICE.
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

// Primitive type-name subjects in impl blocks (`impl str: Trait` / `impl str { }`).
// grammar.ebnf:312 makes a primitive a valid `trait_impl` subject; acceptance is scoped to
// the 6-primitive helper (trait-path stays Ident-only; Never/void reject).
use ori_ir::{ParsedType, TypeId};

fn sole_impl(output: &ParseOutput) -> &ori_ir::ImplDef {
    match output.module.impls.as_slice() {
        [imp] => imp,
        other => panic!("expected exactly one impl, got {}", other.len()),
    }
}

fn assert_e1002(output: &ParseOutput, ctx: &str) {
    assert!(
        output.errors.iter().any(|e| e.code() == ErrorCode::E1002),
        "{ctx}: expected E1002, got {:?}",
        output
            .errors
            .iter()
            .map(crate::ParseError::code)
            .collect::<Vec<_>>()
    );
}

// Cell #1 — the repro: str trait-impl with generic trait args. Semantic pin.
#[test]
fn test_parse_impl_str_trait_with_generic_arg_yields_primitive_subject() {
    let output =
        parse_source("impl str: Convert<MyErr> { @to_t (self) -> MyErr = MyErr { msg: self }; }");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let imp = sole_impl(&output);
    assert!(imp.is_trait_impl(), "colon form must record a trait_path");
    assert_eq!(
        imp.self_ty,
        ParsedType::Primitive(TypeId::STR),
        "subject self_ty"
    );
    assert_eq!(imp.self_path.len(), 1, "self_path is the primitive name");
    assert_eq!(
        imp.trait_path.as_ref().map(Vec::len),
        Some(1),
        "trait_path is the post-colon `Convert`"
    );
    assert!(!imp.trait_type_args.is_empty(), "trait carries <MyErr>");
}

// Cells #2-#6 — per-primitive coverage (int/bool/float/char/byte) + str.
#[test]
fn test_parse_impl_primitive_subjects_cover_all_six_type_keywords() {
    let cases = [
        ("int", TypeId::INT),
        ("bool", TypeId::BOOL),
        ("float", TypeId::FLOAT),
        ("char", TypeId::CHAR),
        ("byte", TypeId::BYTE),
        ("str", TypeId::STR),
    ];
    let mut count = 0;
    for (name, type_id) in cases {
        let output = parse_source(&format!("impl {name}: Marker {{ }}"));
        assert!(
            output.errors.is_empty(),
            "impl {name}: Marker — parse errors: {:?}",
            output.errors
        );
        let imp = sole_impl(&output);
        assert_eq!(
            imp.self_ty,
            ParsedType::Primitive(type_id),
            "{name} self_ty"
        );
        count += 1;
    }
    assert_eq!(count, 6, "all six primitive subjects visited");
}

// Cell #7 — inherent primitive impl (no colon, no trait).
#[test]
fn test_parse_inherent_impl_on_primitive_subject_has_no_trait_path() {
    let output = parse_source("impl str { @shout (self) -> str = self; }");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let imp = sole_impl(&output);
    assert!(
        imp.is_inherent(),
        "inherent primitive impl has no trait_path"
    );
    assert_eq!(
        imp.self_ty,
        ParsedType::Primitive(TypeId::STR),
        "subject self_ty"
    );
}

// Cell #8 — impl-level generics on a primitive subject.
#[test]
fn test_parse_impl_with_generics_on_primitive_subject_parses() {
    let output = parse_source("impl<T> str: Convert<T> { @to_t (self) -> T = todo(); }");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let imp = sole_impl(&output);
    assert!(
        !imp.generics.is_empty(),
        "impl-level generics consumed before subject"
    );
    assert_eq!(
        imp.self_ty,
        ParsedType::Primitive(TypeId::STR),
        "subject self_ty"
    );
}

// Cell #9 — regression guard: user-type subject still parses unchanged.
#[test]
fn test_parse_impl_user_type_subject_still_parses() {
    let output = parse_source("impl Point: Eq { @equals (self, other: Point) -> bool = true; }");
    assert!(
        output.errors.is_empty(),
        "Parse errors: {:?}",
        output.errors
    );
    let imp = sole_impl(&output);
    assert!(imp.is_trait_impl(), "user-type colon impl unchanged");
}

// Cell #10 — negative pin: primitive in TRAIT position stays parse-rejected.
#[test]
fn test_parse_impl_primitive_in_trait_position_rejects() {
    let output = parse_source("impl Foo: str { }");
    assert_e1002(&output, "impl Foo: str (primitive trait-position)");
}

// Cell #11 — negative pin: primitive subject, missing trait after colon.
#[test]
fn test_parse_impl_primitive_subject_missing_trait_rejects() {
    let output = parse_source("impl str: { }");
    assert_e1002(&output, "impl str: (missing trait)");
}

// Cell #12 — boundary: container subject stays unparseable (no list-subject path).
#[test]
fn test_parse_impl_container_subject_rejects() {
    let output = parse_source("impl [str]: Marker { }");
    assert!(
        !output.errors.is_empty(),
        "impl [str]: Marker — container subject must reject, got no errors"
    );
}

// Cell #13 — boundary: tuple subject stays unparseable (no tuple-subject path; the `(`
// hits parse_impl_type's expect_ident, like the list-subject cell #12).
#[test]
fn test_parse_impl_tuple_subject_rejects() {
    let output = parse_source("impl (str, int): Marker { }");
    assert_e1002(&output, "impl (str, int): Marker (tuple subject)");
}

// Cell #14 — negative pin: primitive subject with spurious type args (primitives take none).
#[test]
fn test_parse_impl_primitive_subject_with_type_args_rejects() {
    let output = parse_source("impl str<T>: Marker { }");
    assert!(
        !output.errors.is_empty(),
        "impl str<T>: — primitive subject takes no type args, must reject"
    );
}

// Cell #15 — negative pin: Never (capital -> NeverType) and void (-> Void) pass the 8-token
// check_type_keyword gate but are NOT in the 6-primitive helper, so they fall through to
// expect_ident()->E1002. (Lowercase `never` is an ordinary identifier.)
#[test]
fn test_parse_impl_never_void_subjects_reject() {
    for name in ["Never", "void"] {
        let output = parse_source(&format!("impl {name}: Marker {{ }}"));
        assert_e1002(&output, &format!("impl {name}: (outside helper domain)"));
    }
}
