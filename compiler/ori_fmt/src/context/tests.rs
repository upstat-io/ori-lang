use super::*;

#[test]
fn context_basic_emit() {
    let mut ctx = FormatContext::new();
    ctx.emit("hello");
    assert_eq!(ctx.column(), 5);
    ctx.emit_space();
    assert_eq!(ctx.column(), 6);
    ctx.emit("world");
    assert_eq!(ctx.column(), 11);
    assert_eq!(ctx.output(), "hello world");
}

#[test]
fn context_newline_resets_column() {
    let mut ctx = FormatContext::new();
    ctx.emit("line1");
    assert_eq!(ctx.column(), 5);
    ctx.emit_newline();
    assert_eq!(ctx.column(), 0);
    ctx.emit("line2");
    assert_eq!(ctx.column(), 5);
    assert_eq!(ctx.output(), "line1\nline2");
}

#[test]
fn context_indentation() {
    let mut ctx = FormatContext::new();
    ctx.emit("level0");
    ctx.emit_newline();

    ctx.indent();
    ctx.emit_indent();
    assert_eq!(ctx.column(), 4);
    ctx.emit("level1");
    ctx.emit_newline();

    ctx.indent();
    ctx.emit_indent();
    assert_eq!(ctx.column(), 8);
    ctx.emit("level2");

    assert_eq!(ctx.output(), "level0\n    level1\n        level2");
}

#[test]
fn context_with_indent_scope() {
    let mut ctx = FormatContext::new();
    assert_eq!(ctx.indent_level(), 0);

    ctx.with_indent(|ctx| {
        assert_eq!(ctx.indent_level(), 1);
        ctx.with_indent(|ctx| {
            assert_eq!(ctx.indent_level(), 2);
        });
        assert_eq!(ctx.indent_level(), 1);
    });

    assert_eq!(ctx.indent_level(), 0);
}

#[test]
fn context_fits_check() {
    let mut ctx = FormatContext::new();
    ctx.emit("x".repeat(90).as_str());

    assert!(ctx.fits(10)); // 90 + 10 = 100
    assert!(!ctx.fits(11)); // 90 + 11 = 101 > 100
}

#[test]
fn context_would_exceed_limit() {
    let mut ctx = FormatContext::new();
    ctx.emit("x".repeat(50).as_str());

    assert!(!ctx.would_exceed_limit(50)); // 50 + 50 = 100
    assert!(ctx.would_exceed_limit(51)); // 50 + 51 = 101
}

#[test]
fn context_finalize() {
    let mut ctx = FormatContext::new();
    ctx.emit("content");
    ctx.emit_newline();
    ctx.emit_newline();
    ctx.emit_newline();

    let output = ctx.finalize();
    assert_eq!(output, "content\n");
}

#[test]
fn context_indent_width() {
    let mut ctx = FormatContext::new();
    assert_eq!(ctx.indent_width(), 0);
    ctx.indent();
    assert_eq!(ctx.indent_width(), 4);
    ctx.indent();
    assert_eq!(ctx.indent_width(), 8);
}

#[test]
fn context_newline_indent() {
    let mut ctx = FormatContext::new();
    ctx.indent();
    ctx.emit("first");
    ctx.emit_newline_indent();
    ctx.emit("second");

    assert_eq!(ctx.output(), "first\n    second");
}
