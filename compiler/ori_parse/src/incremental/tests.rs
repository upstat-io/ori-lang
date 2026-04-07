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
        target_attr: None,
        cfg_attr: None,
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
        is_fbip: false,
        target_attr: None,
        cfg_attr: None,
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
        is_fbip: false,
        target_attr: None,
        cfg_attr: None,
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
        is_fbip: false,
        target_attr: None,
        cfg_attr: None,
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
            is_fbip: false,
            target_attr: None,
            cfg_attr: None,
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

// Incremental parsing must preserve file-level attributes.
#[test]
fn test_incremental_preserves_file_attr_target() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Source with file-level #!target attribute
    // "#!target(os: \"linux\")\n\n@first () -> int = 42;\n\n@second () -> int = 100;"
    //                                                                          ^pos 68
    let source = "#!target(os: \"linux\")\n\n@first () -> int = 42;\n\n@second () -> int = 100;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert!(
        old_result.module.file_attr.is_some(),
        "full parse must preserve file_attr"
    );
    assert_eq!(old_result.module.functions.len(), 2);

    // Modify second function body: change 100 to 200
    let new_source =
        "#!target(os: \"linux\")\n\n@first () -> int = 42;\n\n@second () -> int = 200;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // "100" is at position 68, 3 chars, replaced with "200" (3 chars)
    let change = TextChange::new(68, 71, 3);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert!(
        new_result.module.file_attr.is_some(),
        "incremental parse must preserve file_attr (TPR-01-060)"
    );
    assert_eq!(new_result.module.functions.len(), 2);
}

#[test]
fn test_incremental_preserves_file_attr_cfg() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Source with file-level #!cfg attribute
    // "#!cfg(debug)\n\n@f () -> int = 1;"
    //                               ^pos 29
    let source = "#!cfg(debug)\n\n@f () -> int = 1;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert!(
        old_result.module.file_attr.is_some(),
        "full parse must preserve file_attr"
    );

    // Change function body: 1 to 2
    let new_source = "#!cfg(debug)\n\n@f () -> int = 2;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // "1" at position 29, 1 char, replaced with "2" (1 char)
    let change = TextChange::new(29, 30, 1);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert!(
        new_result.module.file_attr.is_some(),
        "incremental parse must preserve #!cfg file_attr (TPR-01-060)"
    );
}

// Incremental parsing must thread leftover attrs to first declaration.
#[test]
fn test_incremental_preserves_first_decl_target_attr() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Source with #target on the first (and only) function — no imports
    // "#target(os: \"linux\")\n@platform_f () -> int = 42;"
    //                                                 ^pos 45
    let source = "#target(os: \"linux\")\n@platform_f () -> int = 42;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 1);
    assert!(
        old_result.module.functions[0].target_attr.is_some(),
        "full parse must preserve target_attr on first function"
    );

    // Change function body: 42 to 99
    let new_source = "#target(os: \"linux\")\n@platform_f () -> int = 99;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // "42" at position 45, 2 chars, replaced with "99" (2 chars)
    let change = TextChange::new(45, 47, 2);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 1);
    assert!(
        new_result.module.functions[0].target_attr.is_some(),
        "incremental parse must preserve target_attr on first function (TPR-01-061)"
    );
}

#[test]
fn test_incremental_preserves_first_decl_cfg_attr_after_import() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Source with import, then #cfg on the first function
    // "use std.math { sqrt };\n\n#cfg(debug)\n@debug_f () -> int = 42;"
    //                                                             ^pos 59
    let source = "use std.math { sqrt };\n\n#cfg(debug)\n@debug_f () -> int = 42;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.imports.len(), 1);
    assert_eq!(old_result.module.functions.len(), 1);
    assert!(
        old_result.module.functions[0].cfg_attr.is_some(),
        "full parse must preserve cfg_attr on first function after imports"
    );

    // Change function body: 42 to 99
    let new_source = "use std.math { sqrt };\n\n#cfg(debug)\n@debug_f () -> int = 99;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // "42" at position 59, 2 chars, replaced with "99" (2 chars)
    let change = TextChange::new(59, 61, 2);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 1);
    assert!(
        new_result.module.functions[0].cfg_attr.is_some(),
        "incremental parse must preserve cfg_attr on first function (TPR-01-061)"
    );
}

// Incremental parsing must not leak leftover attrs to reparsed declarations.
// When the first declaration is reused (outside change region), leftover_attrs from
// parse_imports() must be consumed on the reuse path so they don't leak to the next
// fresh-parsed declaration.
#[test]
fn test_incremental_reuse_consumes_leftover_target_attrs() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Source: #target on first function, two functions.
    // Byte positions:
    //   #target(os: "linux") = bytes 0..20
    //   \n                    = byte 20
    //   @first () -> int = 42; = bytes 21..43
    //   \n\n                  = bytes 43..45
    //   @second () -> int = 100; = bytes 45..69
    let source = "#target(os: \"linux\")\n@first () -> int = 42;\n\n@second () -> int = 100;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 2);
    assert!(
        old_result.module.functions[0].target_attr.is_some(),
        "full parse: first function has target_attr"
    );
    assert!(
        old_result.module.functions[1].target_attr.is_none(),
        "full parse: second function has NO target_attr"
    );

    // Edit second function body: 100 -> 200. First function is outside
    // the change region and will be reused by the incremental parser.
    let new_source = "#target(os: \"linux\")\n@first () -> int = 42;\n\n@second () -> int = 200;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // "100" is at bytes [65, 68), replaced with "200" (3 bytes, same length)
    let change = TextChange::new(65, 68, 3);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 2);
    assert!(
        new_result.module.functions[0].target_attr.is_some(),
        "incremental: first function keeps target_attr (reused)"
    );
    // Semantic pin: this assertion ONLY passes when leftover_attrs is consumed
    // on the reuse path. Without the fix, leftover_attrs leaks to @second.
    assert!(
        new_result.module.functions[1].target_attr.is_none(),
        "TPR-01-063: second function must NOT have target_attr — leftover attrs must not leak"
    );
}

#[test]
fn test_incremental_reuse_consumes_leftover_cfg_attrs() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Same pattern with #cfg instead of #target.
    let source = "#cfg(debug)\n@first () -> int = 42;\n\n@second () -> int = 100;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 2);
    assert!(
        old_result.module.functions[0].cfg_attr.is_some(),
        "full parse: first function has cfg_attr"
    );
    assert!(
        old_result.module.functions[1].cfg_attr.is_none(),
        "full parse: second function has NO cfg_attr"
    );

    // Edit second function body: 100 -> 200.
    let new_source = "#cfg(debug)\n@first () -> int = 42;\n\n@second () -> int = 200;";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // #cfg(debug)\n = 12 bytes, @first () -> int = 42;\n\n = 24 bytes
    // @second () -> int = = 20 bytes, "100" at [56, 59)
    let change = TextChange::new(56, 59, 3);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(!new_result.has_errors());
    assert_eq!(new_result.module.functions.len(), 2);
    assert!(
        new_result.module.functions[0].cfg_attr.is_some(),
        "incremental: first function keeps cfg_attr (reused)"
    );
    assert!(
        new_result.module.functions[1].cfg_attr.is_none(),
        "TPR-01-063: second function must NOT have cfg_attr — leftover attrs must not leak"
    );
}

// Orphaned attrs at EOF must produce errors on the incremental path.
#[test]
fn test_incremental_orphan_attrs_at_eof_produce_error() {
    use crate::{parse, parse_incremental};
    use ori_ir::StringInterner;

    let interner = StringInterner::new();

    // Original: function that will be replaced with orphaned attrs.
    let source = "@f () -> int = 42;";
    let tokens = ori_lexer::lex(source, &interner);
    let old_result = parse(&tokens, &interner);

    assert!(!old_result.has_errors());
    assert_eq!(old_result.module.functions.len(), 1);

    // New source: replace function with just an orphaned attr at EOF.
    let new_source = "#target(os: \"linux\")\n";
    let new_tokens = ori_lexer::lex(new_source, &interner);

    // Replace entire original content (0..18) with new content (21 bytes).
    let change = TextChange::new(0, 18, 21);

    let new_result = parse_incremental(&new_tokens, &interner, &old_result, change);

    assert!(
        new_result.has_errors(),
        "TPR-01-062: orphaned attrs at EOF must produce error on incremental path"
    );
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
