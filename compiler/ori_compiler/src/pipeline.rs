//! Core compilation pipeline: lex → parse → typecheck → canonicalize → evaluate.
//!
//! Portable (no Salsa, no filesystem IO). Source comes in as `&str`, results
//! come out as [`CompileOutput`] or [`FormatOutput`].

use ori_eval::{buffer_handler, InterpreterBuilder, Value};
use ori_ir::SharedInterner;

use crate::output::{CompileOutput, ErrorPhase, FormatOutput};
use crate::setup::setup_module;

/// Configuration for a compilation run.
pub struct CompileConfig {
    /// Logical file path (used in diagnostics, not for IO).
    pub file_path: String,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            file_path: "input.ori".to_string(),
        }
    }
}

/// Full single-file pipeline: lex → parse → typecheck → canonicalize → evaluate.
///
/// Returns a [`CompileOutput`] with the result value, captured print output,
/// and any diagnostics. This is the portable equivalent of `oric`'s Salsa-driven
/// pipeline, suitable for WASM and embedding.
pub fn compile_and_run(source: &str, config: &CompileConfig) -> CompileOutput {
    let interner = SharedInterner::default();

    // Lex
    let tokens = ori_lexer::lex(source, &interner);

    // Parse
    let parse_result = ori_parse::parse(&tokens, &interner);
    if parse_result.has_errors() {
        let diagnostics: Vec<_> = parse_result
            .errors
            .iter()
            .map(ori_parse::ParseError::to_diagnostic)
            .collect();
        return CompileOutput {
            success: false,
            output: String::new(),
            printed: String::new(),
            diagnostics,
            error_phase: Some(ErrorPhase::Parse),
        };
    }

    // Type check
    let (type_result, pool) = ori_types::check_module_with_imports(
        &parse_result.module,
        &parse_result.arena,
        &interner,
        |_checker| {},
    );
    if type_result.has_errors() {
        let diagnostics = ori_types::render_type_errors(type_result.errors(), &pool, &interner);
        return CompileOutput {
            success: false,
            output: String::new(),
            printed: String::new(),
            diagnostics,
            error_phase: Some(ErrorPhase::Type),
        };
    }

    // Canonicalize
    let canon_result = ori_canon::lower_module(
        &parse_result.module,
        &parse_result.arena,
        &type_result,
        &pool,
        &interner,
    );
    let shared_canon = ori_ir::canon::SharedCanonResult::new(canon_result);

    // Build interpreter
    let print_handler = buffer_handler();
    let mut interpreter = InterpreterBuilder::new(&interner, &parse_result.arena)
        .print_handler(print_handler.clone())
        .canon(shared_canon.clone())
        .build();

    interpreter.register_prelude();
    setup_module(
        &mut interpreter,
        &parse_result,
        &interner,
        Some(&shared_canon),
    );

    // Find @main
    let main_name = interner.intern("main");
    let Some(can_id) = shared_canon.root_for(main_name) else {
        return CompileOutput {
            success: false,
            output: String::new(),
            printed: interpreter.get_print_output(),
            diagnostics: vec![no_main_diagnostic(&config.file_path)],
            error_phase: Some(ErrorPhase::Runtime),
        };
    };

    // Evaluate
    match interpreter.eval_can(can_id) {
        Ok(value) => {
            let output = format_value(&value);
            let printed = interpreter.get_print_output();
            CompileOutput {
                success: true,
                output,
                printed,
                diagnostics: Vec::new(),
                error_phase: None,
            }
        }
        Err(e) => {
            let printed = interpreter.get_print_output();
            let eval_error = e.into_eval_error();
            let diag = eval_error.to_diagnostic();
            CompileOutput {
                success: false,
                output: String::new(),
                printed,
                diagnostics: vec![diag],
                error_phase: Some(ErrorPhase::Runtime),
            }
        }
    }
}

/// Format Ori source code: lex (with comments) → parse → format.
///
/// Returns a [`FormatOutput`] with the formatted source or parse diagnostics.
pub fn format_source(source: &str, max_width: Option<usize>) -> FormatOutput {
    let interner = SharedInterner::default();

    // Lex with comments for comment-preserving formatting
    let lex_output = ori_lexer::lex_with_comments(source, &interner);

    // Parse
    let parse_result = ori_parse::parse(&lex_output.tokens, &interner);
    if parse_result.has_errors() {
        let diagnostics: Vec<_> = parse_result
            .errors
            .iter()
            .map(ori_parse::ParseError::to_diagnostic)
            .collect();
        return FormatOutput {
            success: false,
            formatted: None,
            diagnostics,
        };
    }

    // Build config
    let config = match max_width {
        Some(width) => ori_fmt::FormatConfig::with_max_width(width),
        None => ori_fmt::FormatConfig::default(),
    };

    // Format with comment preservation
    let formatted = ori_fmt::format_module_with_comments_and_config(
        &parse_result.module,
        &lex_output.comments,
        &parse_result.arena,
        &*interner,
        config,
    );

    FormatOutput {
        success: true,
        formatted: Some(formatted),
        diagnostics: Vec::new(),
    }
}

/// Format a `Value` for output display.
fn format_value(value: &Value) -> String {
    if matches!(value, Value::Void) {
        return String::new();
    }
    value.display_value()
}

/// Create a diagnostic for missing `@main` function.
fn no_main_diagnostic(file_path: &str) -> ori_diagnostic::Diagnostic {
    ori_diagnostic::Diagnostic::error(ori_diagnostic::ErrorCode::E6099)
        .with_message(format!("no @main function found in {file_path}"))
}
