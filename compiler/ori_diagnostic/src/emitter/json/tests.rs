use super::*;
use crate::ErrorCode;
use ori_ir::Span;

fn sample_diagnostic() -> Diagnostic {
    Diagnostic::error(ErrorCode::E2001)
        .with_message("type mismatch: expected `int`, found `str`")
        .with_label(Span::new(10, 15), "expected `int`")
        .with_secondary_label(Span::new(0, 5), "defined here")
        .with_note("int and str are incompatible")
        .with_suggestion("use `int(x)` to convert")
}

#[test]
fn test_json_emitter() {
    let mut output = Vec::new();
    let mut emitter = JsonEmitter::new(&mut output);

    emitter.begin();
    emitter.emit(&sample_diagnostic());
    emitter.end();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("\"code\": \"E2001\""));
    assert!(text.contains("\"severity\": \"Error\""));
    assert!(text.contains("\"message\":"));
    assert!(text.contains("\"labels\":"));
    assert!(text.contains("\"start\":"));
    assert!(text.contains("\"end\":"));
}

#[test]
fn test_json_emitter_multiple() {
    let mut output = Vec::new();
    let mut emitter = JsonEmitter::new(&mut output);

    let diag1 = Diagnostic::error(ErrorCode::E1001).with_message("error 1");
    let diag2 = Diagnostic::warning(ErrorCode::E3001).with_message("warning 1");

    emitter.begin();
    emitter.emit(&diag1);
    emitter.emit(&diag2);
    emitter.end();
    emitter.flush();

    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("E1001"));
    assert!(text.contains("E3001"));
    assert!(text.contains("Error"));
    assert!(text.contains("Warning"));
}
