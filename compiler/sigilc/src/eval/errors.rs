//! Error constructors for the evaluator.
//!
//! These are thin wrappers around `sigil_patterns` error constructors
//! that convert the error type to use local `EvalError`.

use super::evaluator::EvalError;

// Macro to generate wrapper functions that convert sigil_patterns::EvalError to local EvalError
macro_rules! wrap_error_fn {
    ($name:ident) => {
        #[cold]
        pub fn $name() -> EvalError {
            let e = sigil_patterns::$name();
            EvalError::new(e.message)
        }
    };
    ($name:ident, $($arg:ident: $ty:ty),+) => {
        #[cold]
        pub fn $name($($arg: $ty),+) -> EvalError {
            let e = sigil_patterns::$name($($arg),+);
            EvalError::new(e.message)
        }
    };
}

// Binary Operation Errors
wrap_error_fn!(invalid_binary_op, type_name: &str);
wrap_error_fn!(binary_type_mismatch, left: &str, right: &str);
wrap_error_fn!(division_by_zero);
wrap_error_fn!(modulo_by_zero);

// Method Call Errors
wrap_error_fn!(no_such_method, method: &str, type_name: &str);
wrap_error_fn!(wrong_arg_count, method: &str, expected: usize, got: usize);
wrap_error_fn!(wrong_arg_type, method: &str, expected: &str);

// Variable and Function Errors
wrap_error_fn!(undefined_variable, name: &str);
wrap_error_fn!(undefined_function, name: &str);
wrap_error_fn!(undefined_config, name: &str);
wrap_error_fn!(not_callable, type_name: &str);
wrap_error_fn!(wrong_function_args, expected: usize, got: usize);

// Index and Field Access Errors
wrap_error_fn!(index_out_of_bounds, index: i64);
wrap_error_fn!(key_not_found, key: &str);
wrap_error_fn!(cannot_index, receiver: &str, index: &str);
wrap_error_fn!(cannot_get_length, type_name: &str);
wrap_error_fn!(no_field_on_struct, field: &str);
wrap_error_fn!(invalid_tuple_field, field: &str);
wrap_error_fn!(tuple_index_out_of_bounds, index: usize);
wrap_error_fn!(cannot_access_field, type_name: &str);

// Type Conversion and Validation Errors
wrap_error_fn!(range_bound_not_int, bound: &str);
wrap_error_fn!(unbounded_range_end);
wrap_error_fn!(map_keys_must_be_strings);

// Control Flow Errors
wrap_error_fn!(non_exhaustive_match);
wrap_error_fn!(cannot_assign_immutable, name: &str);
wrap_error_fn!(invalid_assignment_target);
wrap_error_fn!(for_requires_iterable);

// Pattern Binding Errors
wrap_error_fn!(tuple_pattern_mismatch);
wrap_error_fn!(expected_tuple);
wrap_error_fn!(expected_struct);
wrap_error_fn!(expected_list);
wrap_error_fn!(list_pattern_too_long);
wrap_error_fn!(missing_struct_field);

// Miscellaneous Errors
wrap_error_fn!(self_outside_method);
wrap_error_fn!(parse_error);
wrap_error_fn!(hash_outside_index);
wrap_error_fn!(await_not_supported);
wrap_error_fn!(invalid_literal_pattern);
