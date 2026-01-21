// Comprehensive tests for the Sigil parser
//
// Test coverage:
// - Function definitions
// - Type definitions
// - Config variables
// - Test definitions
// - Use statements
// - All expression types
// - Pattern syntax (fold, map, filter, recurse, etc.)
// - Match expressions
// - Type expressions
// - Operator precedence

#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::ast::*;
use crate::lexer::tokenize;
use crate::parser::parse;
use insta::assert_debug_snapshot;

// ============================================================================
// Helper Functions
// ============================================================================

fn parse_source(source: &str) -> Result<Module, String> {
    let tokens = tokenize(source, "test.si")?;
    parse(tokens, "test.si")
}

fn parse_ok(source: &str) -> Module {
    parse_source(source).expect("parsing should succeed")
}

fn parse_err(source: &str) -> String {
    parse_source(source).expect_err("parsing should fail")
}

fn first_item(module: &Module) -> &Item {
    module.items.first().expect("expected at least one item")
}

fn first_function(module: &Module) -> &FunctionDef {
    match first_item(module) {
        Item::Function(f) => f,
        _ => panic!("expected a function"),
    }
}

// ============================================================================
// Function Definition Tests
// ============================================================================

#[test]
fn test_simple_function() {
    let module = parse_ok("@hello () -> void = print(\"hi\")");
    let func = first_function(&module);
    assert_eq!(func.name, "hello");
    assert_eq!(func.params.len(), 0);
    assert!(matches!(func.return_type, TypeExpr::Named(ref s) if s == "void"));
}

#[test]
fn test_function_with_params() {
    let module = parse_ok("@add (a: int, b: int) -> int = a + b");
    let func = first_function(&module);
    assert_eq!(func.name, "add");
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0].name, "a");
    assert_eq!(func.params[1].name, "b");
}

#[test]
fn test_public_function() {
    let module = parse_ok("pub @greet (name: str) -> str = \"Hello, \" + name");
    let func = first_function(&module);
    assert!(func.public);
    assert_eq!(func.name, "greet");
}

#[test]
fn test_function_with_type_params() {
    let module = parse_ok("@identity<T> (x: T) -> T = x");
    let func = first_function(&module);
    assert_eq!(func.type_params, vec!["T"]);
}

#[test]
fn test_function_returning_list() {
    let module = parse_ok("@empty () -> [int] = []");
    let func = first_function(&module);
    assert!(matches!(func.return_type, TypeExpr::List(_)));
}

#[test]
fn test_function_returning_optional() {
    let module = parse_ok("@maybe () -> ?int = None");
    let func = first_function(&module);
    assert!(matches!(func.return_type, TypeExpr::Optional(_)));
}

// ============================================================================
// Config Definition Tests
// ============================================================================

#[test]
fn test_config_simple() {
    let module = parse_ok("$timeout = 5000");
    match first_item(&module) {
        Item::Config(c) => {
            assert_eq!(c.name, "timeout");
            assert!(c.ty.is_none());
        }
        _ => panic!("expected config"),
    }
}

#[test]
fn test_config_with_type() {
    let module = parse_ok("$name: str = \"default\"");
    match first_item(&module) {
        Item::Config(c) => {
            assert_eq!(c.name, "name");
            assert!(c.ty.is_some());
        }
        _ => panic!("expected config"),
    }
}

// ============================================================================
// Type Definition Tests
// ============================================================================

#[test]
fn test_type_alias() {
    let module = parse_ok("type UserId = str");
    match first_item(&module) {
        Item::TypeDef(td) => {
            assert_eq!(td.name, "UserId");
            assert!(matches!(td.kind, TypeDefKind::Alias(_)));
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_struct_type() {
    let module = parse_ok("type User { id: int, name: str }");
    match first_item(&module) {
        Item::TypeDef(td) => {
            assert_eq!(td.name, "User");
            if let TypeDefKind::Struct(fields) = &td.kind {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "id");
                assert_eq!(fields[1].name, "name");
            } else {
                panic!("expected struct");
            }
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_enum_type() {
    let module = parse_ok("type Status = | Active | Inactive");
    match first_item(&module) {
        Item::TypeDef(td) => {
            assert_eq!(td.name, "Status");
            if let TypeDefKind::Enum(variants) = &td.kind {
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].name, "Active");
                assert_eq!(variants[1].name, "Inactive");
            } else {
                panic!("expected enum");
            }
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_enum_with_fields() {
    let module = parse_ok("type MyResult = | Ok { value: int } | Err { msg: str }");
    match first_item(&module) {
        Item::TypeDef(td) => {
            if let TypeDefKind::Enum(variants) = &td.kind {
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0].fields.len(), 1);
                assert_eq!(variants[1].fields.len(), 1);
            } else {
                panic!("expected enum");
            }
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_generic_type() {
    let module = parse_ok("type Box<T> = T");
    match first_item(&module) {
        Item::TypeDef(td) => {
            assert_eq!(td.name, "Box");
            assert_eq!(td.params, vec!["T"]);
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_public_type() {
    let module = parse_ok("pub type Id = int");
    match first_item(&module) {
        Item::TypeDef(td) => {
            assert!(td.public);
        }
        _ => panic!("expected type def"),
    }
}

// ============================================================================
// Use Statement Tests
// ============================================================================

#[test]
fn test_use_simple() {
    let module = parse_ok("use math { add, sub }");
    match first_item(&module) {
        Item::Use(u) => {
            assert_eq!(u.path, vec!["math"]);
            assert_eq!(u.items.len(), 2);
            assert_eq!(u.items[0].name, "add");
            assert_eq!(u.items[1].name, "sub");
        }
        _ => panic!("expected use"),
    }
}

#[test]
fn test_use_with_path() {
    let module = parse_ok("use std.io { read, write }");
    match first_item(&module) {
        Item::Use(u) => {
            assert_eq!(u.path, vec!["std", "io"]);
        }
        _ => panic!("expected use"),
    }
}

#[test]
fn test_use_with_alias() {
    let module = parse_ok("use math { add as plus }");
    match first_item(&module) {
        Item::Use(u) => {
            assert_eq!(u.items[0].name, "add");
            assert_eq!(u.items[0].alias, Some("plus".to_string()));
        }
        _ => panic!("expected use"),
    }
}

#[test]
fn test_use_wildcard() {
    let module = parse_ok("use math { * }");
    match first_item(&module) {
        Item::Use(u) => {
            assert_eq!(u.items[0].name, "*");
        }
        _ => panic!("expected use"),
    }
}

// ============================================================================
// Test Definition Tests
// ============================================================================

#[test]
fn test_test_definition() {
    let module = parse_ok("@test_add tests @add () -> void = assert(add(1, 2) == 3)");
    match first_item(&module) {
        Item::Test(t) => {
            assert_eq!(t.name, "test_add");
            assert_eq!(t.target, "add");
        }
        _ => panic!("expected test"),
    }
}

// ============================================================================
// Expression Tests - Literals
// ============================================================================

#[test]
fn test_int_literal() {
    let module = parse_ok("@f () -> int = 42");
    let func = first_function(&module);
    assert!(matches!(func.body, Expr::Int(42)));
}

#[test]
#[allow(clippy::approx_constant)] // Testing that source literal "3.14" parses correctly
fn test_float_literal() {
    let module = parse_ok("@f () -> float = 3.14");
    let func = first_function(&module);
    assert!(matches!(func.body, Expr::Float(f) if (f - 3.14).abs() < 0.001));
}

#[test]
fn test_string_literal() {
    let module = parse_ok(r#"@f () -> str = "hello""#);
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::String(s) if s == "hello"));
}

#[test]
fn test_bool_true() {
    let module = parse_ok("@f () -> bool = true");
    let func = first_function(&module);
    assert!(matches!(func.body, Expr::Bool(true)));
}

#[test]
fn test_bool_false() {
    let module = parse_ok("@f () -> bool = false");
    let func = first_function(&module);
    assert!(matches!(func.body, Expr::Bool(false)));
}

#[test]
fn test_nil_literal() {
    let module = parse_ok("@f () -> void = nil");
    let func = first_function(&module);
    assert!(matches!(func.body, Expr::Nil));
}

// ============================================================================
// Expression Tests - Collections
// ============================================================================

#[test]
fn test_list_literal() {
    let module = parse_ok("@f () -> [int] = [1, 2, 3]");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::List(items) if items.len() == 3));
}

#[test]
fn test_empty_list() {
    let module = parse_ok("@f () -> [int] = []");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::List(items) if items.is_empty()));
}

#[test]
fn test_tuple_literal() {
    let module = parse_ok("@f () -> (int, str) = (1, \"a\")");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Tuple(items) if items.len() == 2));
}

#[test]
fn test_struct_literal() {
    let module = parse_ok("@f () -> Point = Point { x: 1, y: 2 }");
    let func = first_function(&module);
    assert!(
        matches!(&func.body, Expr::Struct { name, fields } if name == "Point" && fields.len() == 2)
    );
}

// ============================================================================
// Expression Tests - Operators
// ============================================================================

#[test]
fn test_binary_add() {
    let module = parse_ok("@f () -> int = 1 + 2");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn test_binary_sub() {
    let module = parse_ok("@f () -> int = 5 - 3");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Sub,
            ..
        }
    ));
}

#[test]
fn test_binary_mul() {
    let module = parse_ok("@f () -> int = 2 * 3");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Mul,
            ..
        }
    ));
}

#[test]
fn test_binary_div() {
    let module = parse_ok("@f () -> int = 6 / 2");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Div,
            ..
        }
    ));
}

#[test]
fn test_binary_mod() {
    let module = parse_ok("@f () -> int = 7 % 3");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Mod,
            ..
        }
    ));
}

#[test]
fn test_binary_eq() {
    let module = parse_ok("@f () -> bool = a == b");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Eq,
            ..
        }
    ));
}

#[test]
fn test_binary_not_eq() {
    let module = parse_ok("@f () -> bool = a != b");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::NotEq,
            ..
        }
    ));
}

#[test]
fn test_binary_lt() {
    let module = parse_ok("@f () -> bool = a < b");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Lt,
            ..
        }
    ));
}

#[test]
fn test_binary_and() {
    let module = parse_ok("@f () -> bool = a && b");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::And,
            ..
        }
    ));
}

#[test]
fn test_binary_or() {
    let module = parse_ok("@f () -> bool = a || b");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Binary {
            op: BinaryOp::Or,
            ..
        }
    ));
}

#[test]
fn test_unary_neg() {
    let module = parse_ok("@f () -> int = -x");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Unary {
            op: UnaryOp::Neg,
            ..
        }
    ));
}

#[test]
fn test_unary_not() {
    let module = parse_ok("@f () -> bool = !x");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Unary {
            op: UnaryOp::Not,
            ..
        }
    ));
}

// ============================================================================
// Operator Precedence Tests
// ============================================================================

#[test]
fn test_precedence_mul_over_add() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let module = parse_ok("@f () -> int = 1 + 2 * 3");
    let func = first_function(&module);
    if let Expr::Binary {
        op: BinaryOp::Add,
        right,
        ..
    } = &func.body
    {
        assert!(matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::Mul,
                ..
            }
        ));
    } else {
        panic!("expected add at top level");
    }
}

#[test]
fn test_precedence_and_over_or() {
    // a || b && c should parse as a || (b && c)
    let module = parse_ok("@f () -> bool = a || b && c");
    let func = first_function(&module);
    if let Expr::Binary {
        op: BinaryOp::Or,
        right,
        ..
    } = &func.body
    {
        assert!(matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::And,
                ..
            }
        ));
    } else {
        panic!("expected or at top level");
    }
}

#[test]
fn test_precedence_comparison_over_and() {
    // a < b && c < d should parse as (a < b) && (c < d)
    let module = parse_ok("@f () -> bool = a < b && c < d");
    let func = first_function(&module);
    if let Expr::Binary {
        op: BinaryOp::And,
        left,
        right,
    } = &func.body
    {
        assert!(matches!(
            left.as_ref(),
            Expr::Binary {
                op: BinaryOp::Lt,
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            Expr::Binary {
                op: BinaryOp::Lt,
                ..
            }
        ));
    } else {
        panic!("expected and at top level");
    }
}

// ============================================================================
// Expression Tests - Control Flow
// ============================================================================

#[test]
fn test_if_then_else() {
    let module = parse_ok("@f (x: int) -> int = if x > 0 :then 1 :else -1");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::If {
            else_branch: Some(_),
            ..
        }
    ));
}

#[test]
fn test_if_then_only() {
    let module = parse_ok("@f (x: int) -> int = if x > 0 :then 1");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::If {
            else_branch: None,
            ..
        }
    ));
}

#[test]
fn test_match_expression() {
    let module = parse_ok("@f (x: int) -> str = match(x, 0: \"zero\", _: \"other\")");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Match(_)));
}

// ============================================================================
// Expression Tests - Function Calls
// ============================================================================

#[test]
fn test_function_call() {
    let module = parse_ok("@f () -> int = add(1, 2)");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Call { .. }));
}

#[test]
fn test_method_call() {
    let module = parse_ok("@f (arr: [int]) -> int = arr.len()");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::MethodCall { method, .. } if method == "len"));
}

#[test]
fn test_field_access() {
    let module = parse_ok("@f (p: Point) -> int = p.x");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Field(_, ref name) if name == "x"));
}

#[test]
fn test_index_access() {
    let module = parse_ok("@f (arr: [int]) -> int = arr[0]");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Index(_, _)));
}

// ============================================================================
// Expression Tests - Lambdas
// ============================================================================

#[test]
fn test_lambda_single_param() {
    let module = parse_ok("@f () -> (int -> int) = x -> x + 1");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Lambda { params, .. } if params.len() == 1));
}

#[test]
fn test_lambda_multiple_params() {
    let module = parse_ok("@f () -> ((int, int) -> int) = (a, b) -> a + b");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Lambda { params, .. } if params.len() == 2));
}

#[test]
fn test_lambda_no_params() {
    let module = parse_ok("@f () -> (() -> int) = () -> 42");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Lambda { params, .. } if params.is_empty()));
}

// ============================================================================
// Expression Tests - Range
// ============================================================================

#[test]
fn test_range() {
    let module = parse_ok("@f () -> [int] = 1..10");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Range { .. }));
}

// ============================================================================
// Expression Tests - Result/Option Types
// ============================================================================

#[test]
fn test_ok_constructor() {
    let module = parse_ok("@f () -> Result<int, str> = Ok(42)");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Ok(_)));
}

#[test]
fn test_err_constructor() {
    let module = parse_ok("@f () -> Result<int, str> = Err(\"failed\")");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Err(_)));
}

#[test]
fn test_some_constructor() {
    let module = parse_ok("@f () -> ?int = Some(42)");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Some(_)));
}

#[test]
fn test_none_constructor() {
    let module = parse_ok("@f () -> ?int = None");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::None_));
}

#[test]
fn test_coalesce() {
    let module = parse_ok("@f (x: ?int) -> int = x ?? 0");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Coalesce { .. }));
}

// ============================================================================
// Expression Tests - Block/Run
// ============================================================================

#[test]
fn test_run_block() {
    let module = parse_ok("@f () -> void = run(print(\"a\"), print(\"b\"))");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Block(items) if items.len() == 2));
}

// ============================================================================
// Pattern Expression Tests
// ============================================================================

#[test]
fn test_fold_pattern() {
    let module = parse_ok("@sum (arr: [int]) -> int = fold(arr, 0, +)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Fold { .. })
    ));
}

#[test]
fn test_map_pattern() {
    let module = parse_ok("@double (arr: [int]) -> [int] = map(arr, x -> x * 2)");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Pattern(PatternExpr::Map { .. })));
}

#[test]
fn test_filter_pattern() {
    let module = parse_ok("@evens (arr: [int]) -> [int] = filter(arr, x -> x % 2 == 0)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Filter { .. })
    ));
}

#[test]
fn test_collect_pattern() {
    let module = parse_ok("@squares (n: int) -> [int] = collect(1..n, x -> x * x)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Collect { .. })
    ));
}

#[test]
fn test_recurse_pattern() {
    let module = parse_ok("@factorial (n: int) -> int = recurse(n <= 1, 1, n * self(n - 1))");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Recurse { memo: false, .. })
    ));
}

#[test]
fn test_recurse_pattern_with_memo() {
    let module =
        parse_ok("@fib (n: int) -> int = recurse(n <= 1, n, self(n - 1) + self(n - 2), true)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Recurse { memo: true, .. })
    ));
}

// ============================================================================
// Named Property Syntax Tests
// ============================================================================

#[test]
fn test_recurse_named_syntax() {
    let module = parse_ok("@fib (n: int) -> int = recurse(.cond: n <= 1, .base: n, .step: self(n-1) + self(n-2), .memo: true)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Recurse { memo: true, .. })
    ));
}

#[test]
fn test_fold_named_syntax() {
    let module = parse_ok("@sum (arr: [int]) -> int = fold(.over: arr, .init: 0, .op: +)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Fold { .. })
    ));
}

#[test]
fn test_map_named_syntax() {
    let module =
        parse_ok("@double (arr: [int]) -> [int] = map(.over: arr, .transform: x -> x * 2)");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Pattern(PatternExpr::Map { .. })));
}

#[test]
fn test_filter_named_syntax() {
    let module =
        parse_ok("@evens (arr: [int]) -> [int] = filter(.over: arr, .predicate: x -> x % 2 == 0)");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Filter { .. })
    ));
}

#[test]
fn test_parallel_named_syntax() {
    let module = parse_ok("@fetch () -> { a: int, b: int } = parallel(.a: getA(), .b: getB())");
    let func = first_function(&module);
    assert!(matches!(
        &func.body,
        Expr::Pattern(PatternExpr::Parallel { .. })
    ));
}

// ============================================================================
// Type Expression Tests
// ============================================================================

#[test]
fn test_type_named() {
    let module = parse_ok("@f () -> int = 0");
    let func = first_function(&module);
    assert!(matches!(func.return_type, TypeExpr::Named(ref s) if s == "int"));
}

#[test]
fn test_type_list() {
    let module = parse_ok("@f () -> [str] = []");
    let func = first_function(&module);
    assert!(matches!(&func.return_type, TypeExpr::List(_)));
}

#[test]
fn test_type_optional() {
    let module = parse_ok("@f () -> ?int = None");
    let func = first_function(&module);
    assert!(matches!(&func.return_type, TypeExpr::Optional(_)));
}

#[test]
fn test_type_tuple() {
    let module = parse_ok("@f () -> (int, str) = (1, \"a\")");
    let func = first_function(&module);
    assert!(matches!(&func.return_type, TypeExpr::Tuple(items) if items.len() == 2));
}

#[test]
fn test_type_function() {
    let module = parse_ok("@f () -> (int -> int) = x -> x");
    let func = first_function(&module);
    // Parenthesized function type should parse correctly
    assert!(matches!(&func.return_type, TypeExpr::Function(_, _)));
}

#[test]
fn test_type_generic() {
    let module = parse_ok("@f () -> Result<int, str> = Ok(0)");
    let func = first_function(&module);
    assert!(
        matches!(&func.return_type, TypeExpr::Generic(name, args) if name == "Result" && args.len() == 2)
    );
}

#[test]
fn test_type_map() {
    // Map type in return type position
    let module = parse_ok("type StringMap = {str: int}");
    match first_item(&module) {
        Item::TypeDef(td) => {
            if let TypeDefKind::Alias(ty) = &td.kind {
                assert!(matches!(ty, TypeExpr::Map(_, _)));
            } else {
                panic!("expected alias");
            }
        }
        _ => panic!("expected type def"),
    }
}

#[test]
fn test_type_record() {
    let module = parse_ok("@f () -> { x: int, y: int } = Point { x: 0, y: 0 }");
    let func = first_function(&module);
    assert!(matches!(&func.return_type, TypeExpr::Record(fields) if fields.len() == 2));
}

// ============================================================================
// Assignment Tests
// ============================================================================

#[test]
fn test_let_binding() {
    let module = parse_ok("@f () -> void = run(let x = 1, print(x))");
    let func = first_function(&module);
    if let Expr::Block(exprs) = &func.body {
        assert!(matches!(&exprs[0], Expr::Let { name, mutable: false, .. } if name == "x"));
    } else {
        panic!("expected block");
    }
}

#[test]
fn test_let_mut_binding() {
    let module = parse_ok("@f () -> void = run(let mut x = 1, x = 2)");
    let func = first_function(&module);
    if let Expr::Block(exprs) = &func.body {
        assert!(matches!(&exprs[0], Expr::Let { name, mutable: true, .. } if name == "x"));
        assert!(matches!(&exprs[1], Expr::Reassign { target, .. } if target == "x"));
    } else {
        panic!("expected block");
    }
}

// ============================================================================
// For Loop Tests
// ============================================================================

#[test]
fn test_for_loop() {
    let module = parse_ok("@f () -> void = for i in 1..10 { print(i) }");
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::For { binding, .. } if binding == "i"));
}

// ============================================================================
// Length Placeholder Tests
// ============================================================================

#[test]
fn test_length_placeholder() {
    let module = parse_ok("@last (arr: [int]) -> int = arr[# - 1]");
    let func = first_function(&module);
    if let Expr::Index(_, index) = &func.body {
        if let Expr::Binary { left, .. } = index.as_ref() {
            assert!(matches!(left.as_ref(), Expr::LengthPlaceholder));
        }
    }
}

// ============================================================================
// Multiline Expression Tests
// ============================================================================

#[test]
fn test_multiline_function() {
    let source = r#"
@add (a: int, b: int) -> int =
    a + b
"#;
    let module = parse_ok(source);
    let func = first_function(&module);
    assert_eq!(func.name, "add");
}

#[test]
fn test_multiline_block() {
    let source = r#"
@main () -> void = run(
    print("hello"),
    print("world")
)
"#;
    let module = parse_ok(source);
    let func = first_function(&module);
    assert!(matches!(&func.body, Expr::Block(items) if items.len() == 2));
}

// ============================================================================
// Error Tests
// ============================================================================

#[test]
fn test_error_missing_arrow() {
    let err = parse_err("@f () int = 0");
    assert!(err.contains("Expected"));
}

#[test]
fn test_error_missing_eq() {
    let err = parse_err("@f () -> int 0");
    assert!(err.contains("Expected"));
}

#[test]
fn test_error_unclosed_paren() {
    let err = parse_err("@f () -> int = (1 + 2");
    assert!(err.contains("Expected"));
}

#[test]
fn test_error_unclosed_bracket() {
    let err = parse_err("@f () -> [int] = [1, 2, 3");
    assert!(err.contains("Expected"));
}

// ============================================================================
// Snapshot Tests
// ============================================================================

#[test]
fn test_snapshot_simple_function() {
    let module = parse_ok("@add (a: int, b: int) -> int = a + b");
    assert_debug_snapshot!(module);
}

#[test]
fn test_snapshot_complex_function() {
    let module = parse_ok("@factorial (n: int) -> int = recurse(n <= 1, 1, n * self(n - 1))");
    assert_debug_snapshot!(module);
}

#[test]
fn test_snapshot_type_definition() {
    let module = parse_ok("type User { id: int, name: str, email: ?str }");
    assert_debug_snapshot!(module);
}

#[test]
fn test_snapshot_match_expression() {
    // Cond-style match: match(cond: body, cond: body, default)
    let module = parse_ok(
        "@classify (n: int) -> str = match(n < 0: \"negative\", n == 0: \"zero\", \"positive\")",
    );
    assert_debug_snapshot!(module);
}
