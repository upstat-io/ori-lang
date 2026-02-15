use super::*;

#[test]
fn test_text_edit_insert() {
    let edit = TextEdit::insert(10, "hello");

    assert_eq!(edit.span, Span::new(10, 10));
    assert_eq!(edit.new_text, "hello");
    assert!(edit.is_insert());
    assert!(!edit.is_delete());
    assert!(!edit.is_replace());
}

#[test]
fn test_text_edit_delete() {
    let edit = TextEdit::delete(Span::new(10, 20));

    assert_eq!(edit.span, Span::new(10, 20));
    assert!(edit.new_text.is_empty());
    assert!(!edit.is_insert());
    assert!(edit.is_delete());
    assert!(!edit.is_replace());
}

#[test]
fn test_text_edit_replace() {
    let edit = TextEdit::replace(Span::new(10, 20), "new content");

    assert_eq!(edit.span, Span::new(10, 20));
    assert_eq!(edit.new_text, "new content");
    assert!(!edit.is_insert());
    assert!(!edit.is_delete());
    assert!(edit.is_replace());
}

#[test]
fn test_code_action_builder() {
    let action = CodeAction::new("Fix it", vec![])
        .with_edit(TextEdit::insert(0, "// fixed\n"))
        .preferred();

    assert_eq!(action.title, "Fix it");
    assert!(action.is_preferred);
    assert_eq!(action.edits.len(), 1);
}

struct MockFix;

impl CodeFix for MockFix {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E2003]
    }

    fn get_fixes(&self, _ctx: &FixContext) -> Vec<CodeAction> {
        vec![CodeAction::new(
            "Mock fix",
            vec![TextEdit::insert(0, "fix")],
        )]
    }
}

#[test]
fn test_mock_fix() {
    let fix = MockFix;

    assert_eq!(fix.error_codes(), &[ErrorCode::E2003]);

    let diag = Diagnostic::error(ErrorCode::E2003).with_message("test");
    let ctx = FixContext::new(&diag, "source code");
    let actions = fix.get_fixes(&ctx);

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].title, "Mock fix");
}

#[test]
fn test_fix_context() {
    let diag = Diagnostic::error(ErrorCode::E2001)
        .with_message("test")
        .with_label(Span::new(5, 10), "here");
    let source = "hello world";
    let ctx = FixContext::new(&diag, source);

    assert_eq!(ctx.primary_span(), Some(Span::new(5, 10)));
    assert_eq!(ctx.text_at(Span::new(0, 5)), "hello");
    assert_eq!(ctx.text_at(Span::new(6, 11)), "world");
}
