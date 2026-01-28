//! Public API functions for type checking.

use super::types::TypedModule;
use super::TypeChecker;
use ori_diagnostic::queue::DiagnosticConfig;
use ori_ir::StringInterner;

/// Type check a parsed module.
pub fn type_check(parse_result: &ori_parse::ParseResult, interner: &StringInterner) -> TypedModule {
    let checker = TypeChecker::new(&parse_result.arena, interner);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with source code for diagnostic queue features.
///
/// When source is provided, error deduplication and limits are enabled.
pub fn type_check_with_source(
    parse_result: &ori_parse::ParseResult,
    interner: &StringInterner,
    source: String,
) -> TypedModule {
    let checker = TypeChecker::with_source(&parse_result.arena, interner, source);
    checker.check_module(&parse_result.module)
}

/// Type check a parsed module with source and custom diagnostic configuration.
pub fn type_check_with_config(
    parse_result: &ori_parse::ParseResult,
    interner: &StringInterner,
    source: String,
    config: DiagnosticConfig,
) -> TypedModule {
    let checker =
        TypeChecker::with_source_and_config(&parse_result.arena, interner, source, config);
    checker.check_module(&parse_result.module)
}
