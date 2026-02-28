//! Unit tests for the ARC DOT visualization module.

use super::node::escape_html;
use super::sanitize_dot_id;

// ── HTML escaping ──────────────────────────────────────────────────

#[test]
fn escape_html_ampersand() {
    assert_eq!(escape_html("a & b"), "a &amp; b");
}

#[test]
fn escape_html_angle_brackets() {
    assert_eq!(escape_html("<T>"), "&lt;T&gt;");
}

#[test]
fn escape_html_quotes() {
    assert_eq!(escape_html("\"hello\""), "&quot;hello&quot;");
}

#[test]
fn escape_html_mixed() {
    assert_eq!(
        escape_html("fn @foo<T>(x: &T) -> int"),
        "fn @foo&lt;T&gt;(x: &amp;T) -&gt; int"
    );
}

#[test]
fn escape_html_plain_text() {
    assert_eq!(escape_html("hello world 123"), "hello world 123");
}

#[test]
fn escape_html_empty() {
    assert_eq!(escape_html(""), "");
}

// ── DOT ID sanitization ───────────────────────────────────────────

#[test]
fn sanitize_simple_name() {
    assert_eq!(sanitize_dot_id("my_function"), "my_function");
}

#[test]
fn sanitize_name_with_dots() {
    assert_eq!(sanitize_dot_id("module.function"), "module_function");
}

#[test]
fn sanitize_name_with_special_chars() {
    assert_eq!(sanitize_dot_id("fn<T>()"), "fn_T___");
}

// ── Edge emission ──────────────────────────────────────────────────

use ori_arc::{ArcBlockId, ArcTerminator, ArcVarId};

#[test]
fn edges_from_jump() {
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![],
    };
    let mut out = String::new();
    super::emit_edges(&mut out, 0, &term);
    assert!(out.contains("bb0 -> bb1"), "jump edge missing: {out}");
    assert!(
        !out.contains("color"),
        "jump should have no color attribute"
    );
}

#[test]
fn edges_from_branch() {
    let term = ArcTerminator::Branch {
        cond: ArcVarId::new(0),
        then_block: ArcBlockId::new(1),
        else_block: ArcBlockId::new(2),
    };
    let mut out = String::new();
    super::emit_edges(&mut out, 0, &term);
    assert!(out.contains("bb0 -> bb1"), "true edge missing");
    assert!(out.contains("bb0 -> bb2"), "false edge missing");
    assert!(out.contains("green4"), "true edge should be green");
    assert!(out.contains("red3"), "false edge should be red");
}

#[test]
fn edges_from_switch() {
    let term = ArcTerminator::Switch {
        scrutinee: ArcVarId::new(0),
        cases: vec![(0, ArcBlockId::new(1)), (1, ArcBlockId::new(2))],
        default: ArcBlockId::new(3),
    };
    let mut out = String::new();
    super::emit_edges(&mut out, 0, &term);
    assert!(out.contains("bb0 -> bb1"), "case 0 edge missing");
    assert!(out.contains("bb0 -> bb2"), "case 1 edge missing");
    assert!(out.contains("bb0 -> bb3"), "default edge missing");
    assert!(out.contains("\"default\""), "default label missing");
}

#[test]
fn edges_from_return() {
    let term = ArcTerminator::Return {
        value: ArcVarId::new(0),
    };
    let mut out = String::new();
    super::emit_edges(&mut out, 0, &term);
    assert!(out.is_empty(), "return should have no edges: {out}");
}

#[test]
fn edges_from_unreachable() {
    let term = ArcTerminator::Unreachable;
    let mut out = String::new();
    super::emit_edges(&mut out, 0, &term);
    assert!(out.is_empty(), "unreachable should have no edges: {out}");
}
