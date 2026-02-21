use crate::{parse, ParseOutput};
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
