use super::super::TextEdit;
use super::*;
use crate::Diagnostic;
use ori_ir::Span;

struct AddSemicolonFix;

impl CodeFix for AddSemicolonFix {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E1001]
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        if let Some(span) = ctx.primary_span() {
            vec![CodeAction::new(
                "Add semicolon",
                vec![TextEdit::insert(span.end, ";")],
            )]
        } else {
            vec![]
        }
    }
}

struct RemoveExtraTokenFix;

impl CodeFix for RemoveExtraTokenFix {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E1001]
    }

    fn get_fixes(&self, ctx: &FixContext) -> Vec<CodeAction> {
        if let Some(span) = ctx.primary_span() {
            vec![CodeAction::new(
                "Remove extra token",
                vec![TextEdit::delete(span)],
            )]
        } else {
            vec![]
        }
    }
}

struct AddTypeAnnotationFix;

impl CodeFix for AddTypeAnnotationFix {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E2005]
    }

    fn get_fixes(&self, _ctx: &FixContext) -> Vec<CodeAction> {
        vec![CodeAction::new(
            "Add type annotation",
            vec![TextEdit::insert(0, ": Type")],
        )]
    }
}

struct MultiCodeFix;

impl CodeFix for MultiCodeFix {
    fn error_codes(&self) -> &'static [ErrorCode] {
        &[ErrorCode::E2001, ErrorCode::E2002, ErrorCode::E2003]
    }

    fn get_fixes(&self, _ctx: &FixContext) -> Vec<CodeAction> {
        vec![CodeAction::new("Multi-code fix", vec![])]
    }
}

#[test]
fn test_register_fix() {
    let mut registry = FixRegistry::new();
    registry.register(AddSemicolonFix);

    assert!(registry.has_fixes_for(ErrorCode::E1001));
    assert!(!registry.has_fixes_for(ErrorCode::E2001));
    assert_eq!(registry.fix_count(), 1);
}

#[test]
fn test_multiple_fixes_same_code() {
    let mut registry = FixRegistry::new();
    registry.register(AddSemicolonFix);
    registry.register(RemoveExtraTokenFix);

    assert!(registry.has_fixes_for(ErrorCode::E1001));
    assert_eq!(registry.fix_count(), 2);
    assert_eq!(registry.mapping_count(), 2); // both map to E1001
}

#[test]
fn test_get_fixes() {
    let mut registry = FixRegistry::new();
    registry.register(AddSemicolonFix);
    registry.register(RemoveExtraTokenFix);

    let diag = Diagnostic::error(ErrorCode::E1001)
        .with_message("unexpected token")
        .with_label(Span::new(10, 15), "here");

    let ctx = FixContext::new(&diag, "let x = 42");
    let actions = registry.get_fixes(&ctx);

    assert_eq!(actions.len(), 2);
    assert!(actions.iter().any(|a| a.title == "Add semicolon"));
    assert!(actions.iter().any(|a| a.title == "Remove extra token"));
}

#[test]
fn test_no_fixes_for_code() {
    let registry = FixRegistry::new();

    let diag = Diagnostic::error(ErrorCode::E9001)
        .with_message("internal error")
        .with_label(Span::new(0, 5), "here");

    let ctx = FixContext::new(&diag, "source");
    let actions = registry.get_fixes(&ctx);

    assert!(actions.is_empty());
}

#[test]
fn test_registry_default() {
    let registry = FixRegistry::default();
    assert_eq!(registry.fix_count(), 0);
}

#[test]
fn test_different_codes() {
    let mut registry = FixRegistry::new();
    registry.register(AddSemicolonFix);
    registry.register(AddTypeAnnotationFix);

    assert!(registry.has_fixes_for(ErrorCode::E1001));
    assert!(registry.has_fixes_for(ErrorCode::E2005));
    assert!(!registry.has_fixes_for(ErrorCode::E2001));
}

#[test]
fn test_multi_code_fix() {
    let mut registry = FixRegistry::new();
    registry.register(MultiCodeFix);

    assert!(registry.has_fixes_for(ErrorCode::E2001));
    assert!(registry.has_fixes_for(ErrorCode::E2002));
    assert!(registry.has_fixes_for(ErrorCode::E2003));
    assert_eq!(registry.fix_count(), 1); // only one fix registered
    assert_eq!(registry.mapping_count(), 3); // but 3 mappings
}
