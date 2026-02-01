//! Tests for method dispatch.
//!
//! Tests method calls on built-in types including list, string, range,
//! Option, and Result.

use ori_ir::StringInterner;

mod consistency;
mod edge_cases;
mod list;
mod option;
mod range;
mod result;
mod string;

/// Create a test interner for method dispatch tests.
fn test_interner() -> StringInterner {
    StringInterner::new()
}
