use super::*;

#[test]
fn test_default_context() {
    let ctx = ParseContext::new();
    assert_eq!(ctx, ParseContext::NONE);
    assert!(ctx.allows_struct_lit());
    assert!(!ctx.in_pattern());
    assert!(!ctx.in_type());
    assert!(!ctx.in_loop());
}

#[test]
fn test_with_flag() {
    let ctx = ParseContext::new().with(ParseContext::IN_PATTERN);
    assert!(ctx.in_pattern());
    assert!(!ctx.in_type());
}

#[test]
fn test_without_flag() {
    let ctx = ParseContext::new()
        .with(ParseContext::IN_PATTERN)
        .with(ParseContext::IN_TYPE);
    assert!(ctx.in_pattern());
    assert!(ctx.in_type());

    let ctx = ctx.without(ParseContext::IN_PATTERN);
    assert!(!ctx.in_pattern());
    assert!(ctx.in_type());
}

#[test]
fn test_no_struct_lit() {
    let ctx = ParseContext::new();
    assert!(ctx.allows_struct_lit());

    let ctx = ctx.with(ParseContext::NO_STRUCT_LIT);
    assert!(!ctx.allows_struct_lit());
}

#[test]
fn test_multiple_flags() {
    let ctx = ParseContext::new()
        .with(ParseContext::IN_LOOP)
        .with(ParseContext::ALLOW_YIELD);

    assert!(ctx.in_loop());
    assert!(ctx.allows_yield());
    assert!(!ctx.in_pattern());
}

#[test]
fn test_union() {
    let ctx1 = ParseContext::new().with(ParseContext::IN_PATTERN);
    let ctx2 = ParseContext::new().with(ParseContext::IN_LOOP);
    let combined = ctx1.union(ctx2);

    assert!(combined.in_pattern());
    assert!(combined.in_loop());
}
