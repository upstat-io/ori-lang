//! Test infrastructure and tests for infer/ modules.
//!
//! This module provides test utilities for testing type inference functions,
//! plus test files for specific inference components.

// Allow dead code for utility functions that may be used by future tests
#![allow(dead_code)]
#![allow(clippy::needless_raw_string_hashes)]

mod call_tests;
mod control_tests;
mod free_vars_tests;

use crate::checker::{TypeChecker, TypeCheckerBuilder};
use ori_ir::SharedInterner;
use ori_types::Type;

/// Parse source code and return a `TypeChecker` ready for testing.
///
/// This creates a checker with all modules initialized but before
/// the module has been checked, allowing tests to invoke inference
/// functions directly.
pub fn checker_from_source(source: &str) -> (TypeChecker<'static>, ori_parse::ParseOutput) {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    // Leak the interner and parsed output to get 'static lifetimes for testing.
    // This is acceptable in tests since memory is cleaned up at process exit.
    let interner: &'static SharedInterner = Box::leak(Box::new(interner));
    let parsed: &'static ori_parse::ParseOutput = Box::leak(Box::new(parsed));

    let checker = TypeCheckerBuilder::new(&parsed.arena, interner).build();

    (checker, parsed.clone())
}

/// Parse and type-check source code, returning the checker and results.
///
/// Unlike `checker_from_source`, this actually runs the type checker
/// on the module, useful for testing complete inference scenarios.
pub fn check_source(source: &str) -> CheckResult {
    let interner = SharedInterner::default();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    // Leak for 'static lifetime in tests
    let interner: &'static SharedInterner = Box::leak(Box::new(interner));
    let parsed: &'static ori_parse::ParseOutput = Box::leak(Box::new(parsed));

    let checker = TypeCheckerBuilder::new(&parsed.arena, interner).build();
    let typed = checker.check_module(&parsed.module);

    CheckResult {
        parsed: parsed.clone(),
        typed,
        interner,
    }
}

/// Result of checking source code, with access to all components.
pub struct CheckResult {
    pub parsed: ori_parse::ParseOutput,
    pub typed: crate::checker::types::TypedModule,
    pub interner: &'static SharedInterner,
}

impl CheckResult {
    /// Check if type checking produced any errors.
    pub fn has_errors(&self) -> bool {
        self.typed.has_errors()
    }

    /// Get the first error message, if any.
    pub fn first_error(&self) -> Option<&str> {
        self.typed.errors.first().map(|e| e.message.as_str())
    }

    /// Check if any error contains the given substring.
    pub fn has_error_containing(&self, substring: &str) -> bool {
        self.typed
            .errors
            .iter()
            .any(|e| e.message.contains(substring))
    }

    /// Get the type of the first function's body expression.
    pub fn first_function_body_type(&self) -> Option<ori_ir::TypeId> {
        self.parsed
            .module
            .functions
            .first()
            .map(|f| self.typed.expr_types[f.body.index()])
    }
}

/// Helper to assert a type matches expected.
pub fn assert_type_eq(actual: &Type, expected: &Type, context: &str) {
    assert_eq!(actual, expected, "Type mismatch in {context}");
}
