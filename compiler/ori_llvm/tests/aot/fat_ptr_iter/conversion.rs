//! Collection conversion tests — F12: `map.keys()`, `map.values()`, `set.to_list()`, `str.split()`.

use crate::util::assert_aot_success;

// map.keys() on {str: int} → [str]

#[test]
fn test_map_keys_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/map_keys_str.ori"),
        "map_keys_str",
    );
}

// map.values() on {int: str} → [str]

#[test]
fn test_map_values_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/map_values_str.ori"),
        "map_values_str",
    );
}

// str.split() → [str]

#[test]
fn test_str_split() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split.ori"),
        "str_split",
    );
}

// Semantic pin: slice strings in tuples
// str.split() produces seamless slice strings (SLICE_FLAG in cap).
// When stored in [(str, int)], the per-field RC inc/dec in rc_value_traversal
// must use ori_str_rc_inc/ori_str_rc_dec (slice-aware), not ori_rc_inc/ori_rc_dec.
// Without the fix: misaligned pointer dereference crash on RC inc of interior ptr.

#[test]
fn test_str_split_in_tuple_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split_in_tuple_list.ori"),
        "str_split_in_tuple_list",
    );
}

// Semantic pin: slice strings in Option
// Similar to tuple test but exercises Option variant dispatch path.

#[test]
fn test_str_split_in_option_list() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split_in_option_list.ori"),
        "str_split_in_option_list",
    );
}

// Semantic pin: top-level slice string passed to/from function
// str.split() result elements are slice strings. When a slice string is passed
// as a top-level str parameter (FatPointer RC strategy), emit_rc_inc_fat must
// use ori_str_rc_inc(data, cap) not ori_rc_inc(data).

#[test]
fn test_str_split_function_param() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split_function_param.ori"),
        "str_split_function_param",
    );
}

// map.keys() then use original map — verify key_inc_fn prevents dangling

#[test]
fn test_map_keys_then_use_map() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/map_keys_then_use_map.ori"),
        "map_keys_then_use_map",
    );
}

// Semantic pin: slice-string clone
// str.split() produces seamless slice strings. When clone() is called on a
// slice str, emit_rc_inc_clone → emit_slice_aware_rc_inc must use
// ori_str_rc_inc(data, cap), not ori_rc_inc(data).
// Without the fix: misaligned pointer dereference crash on ori_rc_inc of
// interior pointer.

#[test]
fn test_str_split_clone() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split_clone.ori"),
        "str_split_clone",
    );
}

// Semantic pin: slice-string auto-iteration
// When an iterator method (e.g., .map, .any, .count) is called on a str
// from str.split(), emit_auto_iter → emit_slice_aware_rc_inc must use
// ori_str_rc_inc(data, cap). Without the fix: crash on auto-iter RcInc
// of interior pointer.

#[test]
fn test_str_split_auto_iter() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split_auto_iter.ori"),
        "str_split_auto_iter",
    );
}

// F12: set.to_list() on Set<str> — exercises ori_set_to_list with elem_dec_fn.

#[test]
fn test_set_to_list_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/set_to_list_str.ori"),
        "set_to_list_str",
    );
}

// Semantic pin: splitting a substring (slice-backed string) must not crash.
// ori_str_split now receives str_cap to detect SLICE_FLAG and compute the original
// allocation base for RC inc. Without the fix: misaligned pointer dereference.

#[test]
fn test_str_split_on_substring() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/str_split_on_substring.ori"),
        "str_split_on_substring",
    );
}

// Semantic pin: derived Clone on struct with slice-string field
// emit_clone_rc_inc_str() must use ori_str_rc_inc(data, cap), not ori_rc_inc(data).
// Without the fix: misaligned pointer dereference on ori_rc_inc of interior pointer
// from str.split() slice strings.

#[test]
fn test_derive_clone_slice_str_struct() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/derive_clone_slice_str_struct.ori"),
        "derive_clone_slice_str_struct",
    );
}

// Semantic pin: derived Clone on struct with Option<str> field
// emit_clone_composite_rc_inc recursively processes Option fields, eventually
// calling emit_clone_rc_inc_str on the inner str. Slice strings must be
// handled with ori_str_rc_inc(data, cap).

#[test]
fn test_derive_clone_slice_str_option() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/derive_clone_slice_str_option.ori"),
        "derive_clone_slice_str_option",
    );
}

// Semantic pin: derived Clone on struct with (str, int) tuple field
// emit_clone_composite_rc_inc recursively processes Tuple fields, eventually
// calling emit_clone_rc_inc_str on the str element. Slice strings must be
// handled with ori_str_rc_inc(data, cap).

#[test]
fn test_derive_clone_slice_str_tuple() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/derive_clone_slice_str_tuple.ori"),
        "derive_clone_slice_str_tuple",
    );
}

// Semantic pin: derived Clone on struct with Result<str, str> field.
// emit_clone_field_rc_inc must handle Tag::Result by branching on the tag and
// RC-incrementing the correct variant's payload. Without the fix, Result payloads
// are never RC-incremented → double-free when both original and clone are dropped.

#[test]
fn test_derive_clone_result_str_str() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/derive_clone_result_str_str.ori"),
        "derive_clone_result_str_str",
    );
}

// Semantic pin: derived Clone on struct with Result<str, int> field.
// Tests the heterogeneous case where Ok type needs RC but Err type is scalar.

#[test]
fn test_derive_clone_result_str_int() {
    assert_aot_success(
        include_str!("../fixtures/fat_ptr_iter/conversion/derive_clone_result_str_int.ori"),
        "derive_clone_result_str_int",
    );
}
