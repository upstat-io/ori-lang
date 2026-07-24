use crate::{compile_and_run, format_source, render_diagnostics, CompileConfig, ErrorPhase};
use ori_diagnostic::emitter::ColorMode;

fn default_config() -> CompileConfig {
    CompileConfig {
        file_path: "test.ori".to_string(),
    }
}

// compile_and_run tests

#[test]
fn run_simple_main_returns_int() {
    let output = compile_and_run("@main () -> int = 42;", &default_config());
    assert!(
        output.success,
        "expected success, got phase={:?}, diagnostics={:?}",
        output.error_phase, output.diagnostics
    );
    assert_eq!(output.output, "42");
    assert!(output.printed.is_empty());
    assert!(output.diagnostics.is_empty());
    assert!(output.error_phase.is_none());
}

#[test]
fn run_void_main_returns_empty_output() {
    // Use `print()` (keyword) rather than `println()` (prelude function),
    // since the portable pipeline doesn't load the standard library prelude.
    let output = compile_and_run(
        "@main () -> void = print(msg: \"hello\");",
        &default_config(),
    );
    assert!(
        output.success,
        "expected success, got phase={:?}, diagnostics={:?}",
        output.error_phase, output.diagnostics
    );
    assert!(output.output.is_empty());
    assert_eq!(output.printed, "hello\n");
}

#[test]
fn run_parse_error_reports_phase() {
    let output = compile_and_run("@main () -> int = {", &default_config());
    assert!(!output.success);
    assert_eq!(output.error_phase, Some(ErrorPhase::Parse));
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn run_type_error_reports_phase() {
    let output = compile_and_run("@main () -> int = \"not an int\";", &default_config());
    assert!(!output.success);
    assert_eq!(output.error_phase, Some(ErrorPhase::Type));
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn run_runtime_error_reports_phase() {
    let output = compile_and_run("@main () -> int = 1 / 0;", &default_config());
    assert!(!output.success);
    assert_eq!(output.error_phase, Some(ErrorPhase::Runtime));
    assert!(!output.diagnostics.is_empty());
    // Runtime diagnostics should have proper error codes (not generic E6099 for div-by-zero)
    assert_eq!(output.diagnostics[0].code, ori_diagnostic::ErrorCode::E6001);
}

#[test]
fn run_no_main_reports_runtime_error() {
    let output = compile_and_run("@add (a: int, b: int) -> int = a + b;", &default_config());
    assert!(!output.success);
    assert_eq!(output.error_phase, Some(ErrorPhase::Runtime));
    assert!(
        output.diagnostics[0].message.contains("@main"),
        "message: {}",
        output.diagnostics[0].message
    );
}

#[test]
fn run_captures_print_output_before_error() {
    let source = concat!(
        "@main () -> int = {\n",
        "    print(msg: \"before error\");\n",
        "    1 / 0\n",
        "}\n",
    );
    let output = compile_and_run(source, &default_config());
    assert!(!output.success, "expected failure");
    assert_eq!(
        output.error_phase,
        Some(ErrorPhase::Runtime),
        "diagnostics: {:?}",
        output.diagnostics
    );
    assert_eq!(output.printed, "before error\n");
}

// Block-body syntax (no semicolons needed for `{ }` bodies)

#[test]
fn run_block_body_no_semicolon() {
    let source = concat!("@main () -> int = {\n", "    42\n", "}\n",);
    let output = compile_and_run(source, &default_config());
    assert!(
        output.success,
        "expected success, got phase={:?}, diagnostics={:?}",
        output.error_phase, output.diagnostics
    );
    assert_eq!(output.output, "42");
}

// format_source tests

#[test]
fn format_valid_source() {
    let output = format_source("@main () -> int = 42;", None);
    assert!(
        output.success,
        "expected success, diagnostics={:?}",
        output.diagnostics
    );
    assert!(output.formatted.is_some());
    assert!(output.diagnostics.is_empty());
}

#[test]
fn format_invalid_source_returns_diagnostics() {
    let output = format_source("@main () -> int = {", None);
    assert!(!output.success);
    assert!(output.formatted.is_none());
    assert!(!output.diagnostics.is_empty());
}

// A multi-param signature that fits on one line at the default width (100) must
// stack one-param-per-line when a narrow `max_width` is requested. Pins the
// `max_width: Some(width)` branch of `format_source` (the `None` branch is
// covered above); a fix that ignored the width would reproduce the default
// single-line output and fail the inequality assertion.
#[test]
fn format_narrow_max_width_stacks_params() {
    let source = "@add (alpha: int, beta: int, gamma: int) -> int = alpha + beta + gamma;";

    let default_output = format_source(source, None);
    assert!(
        default_output.success,
        "default-width format failed: {:?}",
        default_output.diagnostics
    );
    let Some(default_formatted) = default_output.formatted else {
        panic!("default-width format produced no output");
    };

    let narrow_output = format_source(source, Some(20));
    assert!(
        narrow_output.success,
        "narrow-width format failed: {:?}",
        narrow_output.diagnostics
    );
    let Some(narrow_formatted) = narrow_output.formatted else {
        panic!("narrow-width format produced no output");
    };

    // The narrow width forces stacking the parameters onto separate lines, so
    // the narrow output has strictly more lines than the default single-line form.
    assert!(
        default_formatted.lines().count() < narrow_formatted.lines().count(),
        "narrow max_width did not change formatting\ndefault:\n{default_formatted}\nnarrow:\n{narrow_formatted}"
    );
    assert!(
        narrow_formatted.contains("alpha: int,\n"),
        "expected stacked params at narrow width, got:\n{narrow_formatted}"
    );
}

// render_diagnostics tests

#[test]
fn render_diagnostics_produces_output() {
    let source = "@main () -> int = 1 / 0;";
    let output = compile_and_run(source, &default_config());
    let rendered = render_diagnostics(source, "test.ori", &output.diagnostics, ColorMode::Never);
    assert!(!rendered.is_empty());
    assert!(rendered.contains("division by zero"));
}
