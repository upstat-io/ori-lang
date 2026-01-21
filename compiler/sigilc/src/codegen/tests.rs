// Comprehensive tests for the Sigil C code generator
//
// Test coverage:
// - Type conversion to C types
// - Expression generation
// - Function generation
// - Config generation
// - Runtime generation
// - Match expression generation
// - Snapshot tests for generated C code

use crate::codegen::generate;
use crate::lexer::tokenize;
use crate::parser::parse;
use crate::types::check;

// ============================================================================
// Helper Functions
// ============================================================================

fn gen_source(source: &str) -> Result<String, String> {
    let tokens = tokenize(source, "test.si")?;
    let module = parse(tokens, "test.si")?;
    let typed = check(module)?;
    generate(&typed)
}

fn gen_ok(source: &str) -> String {
    gen_source(source).expect("code generation should succeed")
}

fn gen_err(source: &str) -> String {
    gen_source(source).expect_err("code generation should fail")
}

// ============================================================================
// Header and Runtime Tests
// ============================================================================

#[test]
fn test_header_includes() {
    let code = gen_ok("@main () -> void = nil");
    assert!(code.contains("#include <stdio.h>"));
    assert!(code.contains("#include <stdlib.h>"));
    assert!(code.contains("#include <stdint.h>"));
    assert!(code.contains("#include <stdbool.h>"));
    assert!(code.contains("#include <string.h>"));
}

#[test]
fn test_runtime_string_type() {
    let code = gen_ok("@main () -> void = nil");
    assert!(code.contains("typedef struct { char* data; size_t len; } String;"));
}

#[test]
fn test_runtime_str_new() {
    let code = gen_ok("@main () -> void = nil");
    assert!(code.contains("String str_new(const char* s)"));
    assert!(code.contains("strdup(s)"));
}

#[test]
fn test_runtime_str_concat() {
    let code = gen_ok("@main () -> void = nil");
    assert!(code.contains("String str_concat(String a, String b)"));
}

#[test]
fn test_runtime_int_to_str() {
    let code = gen_ok("@main () -> void = nil");
    assert!(code.contains("String int_to_str(int64_t n)"));
}

// ============================================================================
// Literal Expression Tests
// ============================================================================

#[test]
fn test_int_literal() {
    let code = gen_ok("@f () -> int = 42");
    assert!(code.contains("return 42;"));
}

#[test]
fn test_negative_int_literal() {
    let code = gen_ok("@f () -> int = -42");
    assert!(code.contains("return (-42);"));
}

#[test]
fn test_float_literal() {
    let code = gen_ok("@f () -> float = 3.14");
    assert!(code.contains("return 3.14;"));
}

#[test]
fn test_bool_true_literal() {
    let code = gen_ok("@f () -> bool = true");
    assert!(code.contains("return true;"));
}

#[test]
fn test_bool_false_literal() {
    let code = gen_ok("@f () -> bool = false");
    assert!(code.contains("return false;"));
}

#[test]
fn test_string_literal() {
    let code = gen_ok(r#"@f () -> str = "hello""#);
    assert!(code.contains(r#"str_new("hello")"#));
}

#[test]
fn test_nil_literal() {
    let code = gen_ok("@f () -> void = nil");
    // void functions don't return nil, they just execute
}

// ============================================================================
// Binary Operator Tests
// ============================================================================

#[test]
fn test_add_int() {
    let code = gen_ok("@f () -> int = 1 + 2");
    assert!(code.contains("(1 + 2)"));
}

#[test]
fn test_sub_int() {
    let code = gen_ok("@f () -> int = 5 - 3");
    assert!(code.contains("(5 - 3)"));
}

#[test]
fn test_mul_int() {
    let code = gen_ok("@f () -> int = 4 * 5");
    assert!(code.contains("(4 * 5)"));
}

#[test]
fn test_div_int() {
    let code = gen_ok("@f () -> int = 10 / 2");
    assert!(code.contains("(10 / 2)"));
}

#[test]
fn test_mod_int() {
    let code = gen_ok("@f () -> int = 7 % 3");
    assert!(code.contains("(7 % 3)"));
}

#[test]
fn test_comparison_lt() {
    let code = gen_ok("@f () -> bool = 1 < 2");
    assert!(code.contains("(1 < 2)"));
}

#[test]
fn test_comparison_gt() {
    let code = gen_ok("@f () -> bool = 2 > 1");
    assert!(code.contains("(2 > 1)"));
}

#[test]
fn test_comparison_lte() {
    let code = gen_ok("@f () -> bool = 1 <= 2");
    assert!(code.contains("(1 <= 2)"));
}

#[test]
fn test_comparison_gte() {
    let code = gen_ok("@f () -> bool = 2 >= 1");
    assert!(code.contains("(2 >= 1)"));
}

#[test]
fn test_comparison_eq() {
    let code = gen_ok("@f () -> bool = 1 == 1");
    assert!(code.contains("(1 == 1)"));
}

#[test]
fn test_comparison_neq() {
    let code = gen_ok("@f () -> bool = 1 != 2");
    assert!(code.contains("(1 != 2)"));
}

#[test]
fn test_logical_and() {
    let code = gen_ok("@f () -> bool = true && false");
    assert!(code.contains("(true && false)"));
}

#[test]
fn test_logical_or() {
    let code = gen_ok("@f () -> bool = true || false");
    assert!(code.contains("(true || false)"));
}

#[test]
fn test_string_concat() {
    let code = gen_ok(r#"@f () -> str = "hello" + " world""#);
    assert!(code.contains("str_concat"));
}

// ============================================================================
// Unary Operator Tests
// ============================================================================

#[test]
fn test_unary_neg() {
    let code = gen_ok("@f (x: int) -> int = -x");
    assert!(code.contains("(-x)"));
}

#[test]
fn test_unary_not() {
    let code = gen_ok("@f (x: bool) -> bool = !x");
    assert!(code.contains("(!x)"));
}

// ============================================================================
// Function Generation Tests
// ============================================================================

#[test]
fn test_function_void_return() {
    let code = gen_ok(
        r#"
@greet () -> void = print("hello")
@test_greet tests @greet () -> void = nil
"#,
    );
    assert!(code.contains("void greet(void)"));
}

#[test]
fn test_function_int_return() {
    let code = gen_ok(
        r#"
@answer () -> int = 42
@test_answer tests @answer () -> void = nil
"#,
    );
    assert!(code.contains("int64_t answer(void)"));
    assert!(code.contains("return 42;"));
}

#[test]
fn test_function_with_params() {
    let code = gen_ok(
        r#"
@add (a: int, b: int) -> int = a + b
@test_add tests @add () -> void = nil
"#,
    );
    assert!(code.contains("int64_t add(int64_t a, int64_t b)"));
}

#[test]
fn test_function_forward_declaration() {
    let code = gen_ok(
        r#"
@helper () -> int = 1
@test_helper tests @helper () -> void = nil
"#,
    );
    // Forward declaration should appear before function
    let fwd_pos = code.find("int64_t helper();").unwrap_or(usize::MAX);
    let def_pos = code.find("int64_t helper(void) {").unwrap_or(0);
    assert!(fwd_pos < def_pos);
}

#[test]
fn test_main_function_special() {
    let code = gen_ok("@main () -> void = nil");
    assert!(code.contains("int main(void)"));
    assert!(code.contains("return 0;"));
}

#[test]
fn test_function_calling_function() {
    let code = gen_ok(
        r#"
@helper () -> int = 1
@caller () -> int = helper()
@test_helper tests @helper () -> void = nil
@test_caller tests @caller () -> void = nil
"#,
    );
    assert!(code.contains("return helper();"));
}

// ============================================================================
// Config Generation Tests
// ============================================================================

#[test]
fn test_config_int() {
    let code = gen_ok(
        r#"
$timeout = 5000
@main () -> void = nil
"#,
    );
    assert!(code.contains("const int64_t timeout = 5000;"));
}

#[test]
fn test_config_float() {
    let code = gen_ok(
        r#"
$rate = 0.05
@main () -> void = nil
"#,
    );
    assert!(code.contains("const double rate = 0.05;"));
}

#[test]
fn test_config_bool() {
    let code = gen_ok(
        r#"
$debug = true
@main () -> void = nil
"#,
    );
    assert!(code.contains("const bool debug = true;"));
}

#[test]
fn test_config_string() {
    let code = gen_ok(
        r#"
$name = "test"
@main () -> void = nil
"#,
    );
    // Strings use a different format
    assert!(code.contains("String name"));
}

#[test]
fn test_config_usage() {
    let code = gen_ok(
        r#"
$value: int = 10
@f () -> int = $value
@test_f tests @f () -> void = nil
"#,
    );
    assert!(code.contains("return value;"));
}

// ============================================================================
// If Expression Tests
// ============================================================================

#[test]
fn test_if_ternary() {
    let code = gen_ok("@f (x: int) -> int = if x > 0 :then 1 :else 0");
    assert!(code.contains("? 1 : 0"));
}

#[test]
fn test_if_no_else() {
    let code = gen_ok("@f (x: int) -> int = if x > 0 :then 1");
    // Without else, defaults to 0
    assert!(code.contains("? 1 : 0"));
}

// ============================================================================
// Match Expression Tests
// ============================================================================

#[test]
fn test_match_literal() {
    let code = gen_ok(r#"@f (x: int) -> str = match(x, 0: "zero", 1: "one", _: "other")"#);
    // Should generate nested ternaries
    assert!(code.contains("=="));
    assert!(code.contains("?"));
}

#[test]
fn test_match_wildcard() {
    let code = gen_ok(r#"@f (x: int) -> int = match(x, _: 0)"#);
    assert!(code.contains("return 0;"));
}

// ============================================================================
// Type Conversion Tests
// ============================================================================

#[test]
fn test_type_int_to_int64() {
    let code = gen_ok("@f () -> int = 0");
    assert!(code.contains("int64_t f"));
}

#[test]
fn test_type_float_to_double() {
    let code = gen_ok("@f () -> float = 0.0");
    assert!(code.contains("double f"));
}

#[test]
fn test_type_bool_to_bool() {
    let code = gen_ok("@f () -> bool = true");
    assert!(code.contains("bool f"));
}

#[test]
fn test_type_str_to_string() {
    let code = gen_ok(r#"@f () -> str = "test""#);
    assert!(code.contains("String f"));
}

// ============================================================================
// Print Statement Tests
// ============================================================================

#[test]
fn test_print_string() {
    let code = gen_ok(r#"@main () -> void = print("hello")"#);
    assert!(code.contains("printf(\"%s\\n\""));
}

#[test]
fn test_print_int() {
    let code = gen_ok("@main () -> void = print(42)");
    assert!(code.contains("printf(\"%ld\\n\""));
}

// ============================================================================
// Builtin Function Tests
// ============================================================================

#[test]
fn test_str_conversion() {
    let code = gen_ok(
        r#"
@f (x: int) -> str = str(x)
@test_f tests @f () -> void = nil
"#,
    );
    assert!(code.contains("int_to_str(x)"));
}

// ============================================================================
// Error Cases
// ============================================================================

#[test]
fn test_pipe_not_supported() {
    let err = gen_err(
        r#"
@f (x: int) -> int = x |> g
@g (y: int) -> int = y
@test_f tests @f () -> void = nil
@test_g tests @g () -> void = nil
"#,
    );
    assert!(err.contains("Pipe") || err.contains("not") || err.contains("supported"));
}

// ============================================================================
// Snapshot Tests for Complete Output
// ============================================================================

#[test]
fn test_snapshot_hello_world() {
    let code = gen_ok(r#"@main () -> void = print("Hello, World!")"#);
    // Verify key parts of the generated code structure
    assert!(code.contains("// Generated by Sigil compiler"));
    assert!(code.contains("int main(void)"));
    assert!(code.contains("printf"));
    assert!(code.contains("return 0;"));
}

#[test]
fn test_snapshot_simple_function() {
    let code = gen_ok(
        r#"
@add (a: int, b: int) -> int = a + b
@main () -> void = print(add(1, 2))
@test_add tests @add () -> void = nil
"#,
    );
    // Verify structure
    assert!(code.contains("int64_t add(int64_t a, int64_t b)"));
    assert!(code.contains("return (a + b)"));
    assert!(code.contains("add(1, 2)"));
}

#[test]
fn test_snapshot_conditional() {
    let code = gen_ok(
        r#"
@max (a: int, b: int) -> int = if a > b :then a :else b
@test_max tests @max () -> void = nil
"#,
    );
    assert!(code.contains("((a > b) ? a : b)"));
}

#[test]
fn test_snapshot_config() {
    let code = gen_ok(
        r#"
$version = 1
@get_version () -> int = $version
@main () -> void = print(get_version())
@test_get_version tests @get_version () -> void = nil
"#,
    );
    assert!(code.contains("const int64_t version = 1;"));
    assert!(code.contains("return version;"));
}

#[test]
fn test_multiple_functions() {
    let code = gen_ok(
        r#"
@a () -> int = 1
@b () -> int = a() + 1
@c () -> int = b() + 1
@test_a tests @a () -> void = nil
@test_b tests @b () -> void = nil
@test_c tests @c () -> void = nil
"#,
    );
    // All functions should have forward declarations
    assert!(code.contains("int64_t a();"));
    assert!(code.contains("int64_t b();"));
    assert!(code.contains("int64_t c();"));
}

#[test]
fn test_nested_expressions() {
    let code = gen_ok(
        r#"
@f (x: int) -> int = (x + 1) * (x - 1)
@test_f tests @f () -> void = nil
"#,
    );
    assert!(code.contains("((x + 1) * (x - 1))"));
}

#[test]
fn test_complex_condition() {
    let code = gen_ok(
        r#"
@f (a: int, b: int) -> bool = a > 0 && b > 0 || a < 0 && b < 0
@test_f tests @f () -> void = nil
"#,
    );
    // Should have proper operator grouping
    assert!(code.contains("&&"));
    assert!(code.contains("||"));
}
