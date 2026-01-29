//! Tests for the type checker.
//!
//! Tests are organized into sub-modules:
//! - `literal_tests`: Literals, collections, control flow
//! - `function_tests`: Functions, lambdas, type annotations
//! - `closure_tests`: Closure self-capture detection
//! - `struct_tests`: Type registry, structs, module namespaces

mod closure_tests;
mod function_tests;
mod literal_tests;
mod struct_tests;

use crate::checker::types::TypedModule;
use crate::checker::TypeCheckerBuilder;
use ori_ir::SharedInterner;
use ori_types::SharedTypeInterner;

/// Result of `check_source` including the type interner for verifying compound types.
struct CheckResult {
    parsed: ori_parse::ParseOutput,
    typed: TypedModule,
    type_interner: SharedTypeInterner,
}

fn check_source(source: &str) -> (ori_parse::ParseOutput, TypedModule) {
    let result = check_source_with_interner(source);
    (result.parsed, result.typed)
}

fn check_source_with_interner(source: &str) -> CheckResult {
    let interner = SharedInterner::default();
    let type_interner = SharedTypeInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);
    // Use builder to pass the type interner
    let checker = TypeCheckerBuilder::new(&parsed.arena, &interner)
        .with_type_interner(type_interner.clone())
        .build();
    let typed = checker.check_module(&parsed.module);
    CheckResult {
        parsed,
        typed,
        type_interner,
    }
}
