use super::*;

#[test]
fn string_emitter_basic() {
    let mut emitter = StringEmitter::new();
    emitter.emit("hello");
    emitter.emit_space();
    emitter.emit("world");
    assert_eq!(emitter.output(), "hello world");
}

#[test]
fn string_emitter_newline() {
    let mut emitter = StringEmitter::new();
    emitter.emit("line1");
    emitter.emit_newline();
    emitter.emit("line2");
    assert_eq!(emitter.output(), "line1\nline2");
}

#[test]
fn string_emitter_indentation() {
    let mut emitter = StringEmitter::new();
    emitter.emit("fn main");
    emitter.emit_newline();
    emitter.emit_indent(1);
    emitter.emit("body");
    emitter.emit_newline();
    emitter.emit_indent(2);
    emitter.emit("nested");
    assert_eq!(emitter.output(), "fn main\n    body\n        nested");
}

#[test]
fn string_emitter_trailing_newline() {
    let mut emitter = StringEmitter::new();
    emitter.emit("content");
    emitter.ensure_trailing_newline();
    assert_eq!(emitter.output(), "content\n");
}

#[test]
fn string_emitter_trailing_newline_already_present() {
    let mut emitter = StringEmitter::new();
    emitter.emit("content");
    emitter.emit_newline();
    emitter.ensure_trailing_newline();
    assert_eq!(emitter.output(), "content\n");
}

#[test]
fn string_emitter_trim_trailing_blank_lines() {
    let mut emitter = StringEmitter::new();
    emitter.emit("content");
    emitter.emit_newline();
    emitter.emit_newline();
    emitter.emit_newline();
    emitter.trim_trailing_blank_lines();
    emitter.ensure_trailing_newline();
    assert_eq!(emitter.output(), "content\n");
}

#[test]
fn string_emitter_with_capacity() {
    let emitter = StringEmitter::with_capacity(1024);
    assert!(emitter.is_empty());
    assert_eq!(emitter.len(), 0);
}
