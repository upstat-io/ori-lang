//! AOT tests for FFI type codegen (`CPtr`, `c_int`, `c_float`, etc.)
//!
//! Regression tests for FFI Named types without Pool resolutions
//! caused unresolved type variables at codegen, crashing the LLVM backend with
//! "Call parameter type does not match function signature" verification errors.
//!
//! The fix: `resolve_parsed_type` in the inference path now recognizes FFI type
//! names and creates Named Pool entries with resolutions to their concrete
//! primitive types (`CPtr`→int, `c_float`→float, etc.), so downstream phases
//! (ARC classifier, `TypeInfoStore`) can classify them as trivial scalars.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Semantic pin: Option<CPtr> = None must compile and run.
/// This is the exact repro from
#[test]
fn test_aot_option_cptr_none() {
    assert_aot_success(
        include_str!("fixtures/ffi_types/option_cptr_none.ori"),
        "option_cptr_none",
    );
}

/// Cross-type matrix: all FFI C types inside Option must compile.
#[test]
fn test_aot_option_c_types_none() {
    assert_aot_success(
        include_str!("fixtures/ffi_types/option_c_types_none.ori"),
        "option_c_types_none",
    );
}

/// Interaction test: FFI types as struct fields inside Option.
#[test]
fn test_aot_ffi_type_in_struct() {
    assert_aot_success(
        include_str!("fixtures/ffi_types/ffi_type_in_struct.ori"),
        "ffi_type_in_struct",
    );
}
