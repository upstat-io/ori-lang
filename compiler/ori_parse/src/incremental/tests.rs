use super::*;
use ori_ir::incremental::{ChangeMarker, TextChange};
use ori_ir::{ConstDef, ExprArena, ExprId, Function, Module, Name, Span};

#[test]
fn test_collect_declarations_empty() {
    let module = Module::new();
    let decls = collect_declarations(&module);
    assert!(decls.is_empty());
}

#[test]
fn test_collect_declarations_sorted() {
    // Create a module with declarations in various orders
    let mut module = Module::new();

    // Add consts, functions in non-sorted order
    module.consts.push(ConstDef {
        name: ori_ir::Name::EMPTY,
        ty: None,
        value: ExprId::INVALID,
        span: Span::new(100, 150),
        visibility: ori_ir::Visibility::Private,
    });

    module.functions.push(Function {
        name: ori_ir::Name::EMPTY,
        generics: ori_ir::GenericParamRange::EMPTY,
        params: ori_ir::ParamRange::EMPTY,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        pre_contracts: Vec::new(),
        post_contracts: Vec::new(),
        body: ExprId::INVALID,
        span: Span::new(50, 80),
        visibility: ori_ir::Visibility::Private,
    });

    let decls = collect_declarations(&module);
    assert_eq!(decls.len(), 2);
    // Should be sorted by start position
    assert_eq!(decls[0].span.start, 50);
    assert_eq!(decls[1].span.start, 100);
}

#[test]
fn test_syntax_cursor_find_at() {
    let mut module = Module::new();

    module.functions.push(Function {
        name: ori_ir::Name::EMPTY,
        generics: ori_ir::GenericParamRange::EMPTY,
        params: ori_ir::ParamRange::EMPTY,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        pre_contracts: Vec::new(),
        post_contracts: Vec::new(),
        body: ExprId::INVALID,
        span: Span::new(0, 50),
        visibility: ori_ir::Visibility::Private,
    });

    module.functions.push(Function {
        name: ori_ir::Name::EMPTY,
        generics: ori_ir::GenericParamRange::EMPTY,
        params: ori_ir::ParamRange::EMPTY,
        return_ty: None,
        capabilities: Vec::new(),
        where_clauses: Vec::new(),
        guard: None,
        pre_contracts: Vec::new(),
        post_contracts: Vec::new(),
        body: ExprId::INVALID,
        span: Span::new(100, 150),
        visibility: ori_ir::Visibility::Private,
    });

    let arena = ExprArena::new();
    // Change affects positions 60-80, so first function is reusable, second might be
    let change = TextChange::new(60, 80, 30);
    let marker = ChangeMarker::from_change(&change, 55);
    let mut cursor = SyntaxCursor::new(&module, &arena, marker);

    // First function (0-50) doesn't intersect the change (60-80)
    let Some(first) = cursor.find_at(0) else {
        panic!("should find first declaration");
    };
    assert_eq!(first.kind, DeclKind::Function);
    assert_eq!(first.span.start, 0);
}

#[test]
fn test_incremental_stats() {
    let mut stats = IncrementalStats::default();
    #[allow(
        clippy::float_cmp,
        reason = "exact zero comparison is intentional for default state"
    )]
    {
        assert_eq!(stats.reuse_rate(), 0.0);
    }

    stats.reused_count = 8;
    stats.reparsed_count = 2;
    assert!((stats.reuse_rate() - 80.0).abs() < 0.001);
}

#[test]
fn test_cursor_stats() {
    let mut module = Module::new();

    // Add three functions at non-overlapping spans
    for (start, end) in [(0, 30), (40, 70), (80, 110)] {
        module.functions.push(Function {
            name: Name::EMPTY,
            generics: ori_ir::GenericParamRange::EMPTY,
            params: ori_ir::ParamRange::EMPTY,
            return_ty: None,
            capabilities: Vec::new(),
            where_clauses: Vec::new(),
            guard: None,
            pre_contracts: Vec::new(),
            post_contracts: Vec::new(),
            body: ExprId::INVALID,
            span: Span::new(start, end),
            visibility: ori_ir::Visibility::Private,
        });
    }

    let arena = ExprArena::new();
    // Change at position 50 (inside second function)
    let change = TextChange::new(50, 55, 10);
    let marker = ChangeMarker::from_change(&change, 45);
    let mut cursor = SyntaxCursor::new(&module, &arena, marker);

    // Stats should be zero initially
    assert_eq!(cursor.stats().lookups, 0);
    assert_eq!(cursor.stats().skipped, 0);

    // Find first declaration (should succeed)
    let first = cursor.find_at(0);
    assert!(first.is_some());
    assert_eq!(cursor.stats().lookups, 1);
    assert_eq!(cursor.stats().skipped, 0); // No skipping needed

    // Advance and look for second (which intersects change)
    cursor.advance();
    let second = cursor.find_at(40);
    assert!(second.is_none()); // Can't reuse, intersects change
    assert_eq!(cursor.stats().lookups, 2);
    assert_eq!(cursor.stats().intersected, 1);

    // Total declarations
    assert_eq!(cursor.total_declarations(), 3);
}

#[test]
fn test_parse_incremental_basic() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Original source with two functions
    let source = "@first () -> int = 42;\n\n@second () -> int = 100;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 2);

    // Now modify the first function: change 42 to 99
    // The source is: "@first () -> int = 42;\n\n@second () -> int = 100;"
    //                 ^^^^^^^^^^^^^^^^^^^^ position 19-21 is "42"
    let new_source = "@first () -> int = 99;\n\n@second () -> int = 100;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // Create a change: replace "42" (2 chars at position 19) with "99" (2 chars)
    let change = TextChange::new(19, 21, 2);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 2);
}

#[test]
fn test_parse_incremental_insert() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Original source
    let source = "@add (x: int) -> int = x + 1;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 1);

    // Insert a newline and new function at the end
    let new_source = "@add (x: int) -> int = x + 1;\n\n@sub (x: int) -> int = x - 1;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // Insert at position 28 (after original source)
    let change = TextChange::insert(28, 30); // "\n\n@sub (x: int) -> int = x - 1" is 30 chars

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 2);
}

#[test]
fn test_parse_incremental_fresh_parse_on_overlap() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Original source with one function
    let source = "@compute (x: int, y: int) -> int = x + y;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 1);

    // Modify the function signature (change "y: int" to "y: float")
    // Position of "y: int" is approximately at position 14-20
    let new_source = "@compute (x: int, y: float) -> int = x + y;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // Change: "int" (3 chars) to "float" (5 chars) at position ~18-21
    let change = TextChange::new(18, 21, 5);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 1);
}
