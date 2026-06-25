use super::*;

// RcOpKind index mapping

#[test]
fn test_rc_op_kind_index_matches_array_position() {
    assert_eq!(RcOpKind::Alloc.index(), 0);
    assert_eq!(RcOpKind::Inc.index(), 1);
    assert_eq!(RcOpKind::Dec.index(), 2);
    assert_eq!(RcOpKind::Free.index(), 3);
    assert_eq!(RcOpKind::Cow.index(), 4);
}

// Core RC operation classification

#[test]
fn test_classify_rc_call_alloc_returns_alloc() {
    assert_eq!(classify_rc_call("ori_rc_alloc"), Some(RcOpKind::Alloc));
}

#[test]
fn test_classify_rc_call_inc_returns_inc() {
    assert_eq!(classify_rc_call("ori_rc_inc"), Some(RcOpKind::Inc));
}

#[test]
fn test_classify_rc_call_dec_returns_dec() {
    assert_eq!(classify_rc_call("ori_rc_dec"), Some(RcOpKind::Dec));
}

#[test]
fn test_classify_rc_call_free_returns_free() {
    assert_eq!(classify_rc_call("ori_rc_free"), Some(RcOpKind::Free));
}

#[test]
fn test_classify_rc_call_cow_function_returns_cow() {
    assert_eq!(classify_rc_call("ori_list_push_cow"), Some(RcOpKind::Cow));
}

#[test]
fn test_classify_rc_call_unrelated_returns_none() {
    assert_eq!(classify_rc_call("puts"), None);
}

// Typed RC operation classification (expanded coverage)

#[test]
fn test_classify_typed_alloc_wrappers_return_alloc() {
    assert_eq!(
        classify_rc_call("ori_list_alloc_data"),
        Some(RcOpKind::Alloc)
    );
    assert_eq!(
        classify_rc_call("ori_map_literal_alloc"),
        Some(RcOpKind::Alloc)
    );
    assert_eq!(
        classify_rc_call("ori_set_literal_alloc"),
        Some(RcOpKind::Alloc)
    );
}

#[test]
fn test_classify_typed_inc_returns_inc() {
    assert_eq!(classify_rc_call("ori_str_rc_inc"), Some(RcOpKind::Inc));
    assert_eq!(classify_rc_call("ori_list_rc_inc"), Some(RcOpKind::Inc));
}

#[test]
fn test_classify_typed_dec_returns_dec() {
    assert_eq!(classify_rc_call("ori_str_rc_dec"), Some(RcOpKind::Dec));
    assert_eq!(classify_rc_call("ori_buffer_rc_dec"), Some(RcOpKind::Dec));
    assert_eq!(
        classify_rc_call("ori_map_buffer_rc_dec"),
        Some(RcOpKind::Dec)
    );
    assert_eq!(
        classify_rc_call("ori_set_buffer_rc_dec"),
        Some(RcOpKind::Dec)
    );
}

#[test]
fn test_classify_unique_drop_returns_free() {
    assert_eq!(
        classify_rc_call("ori_buffer_drop_unique"),
        Some(RcOpKind::Free)
    );
    assert_eq!(
        classify_rc_call("ori_set_buffer_drop_unique"),
        Some(RcOpKind::Free)
    );
    assert_eq!(
        classify_rc_call("ori_map_buffer_drop_unique"),
        Some(RcOpKind::Free)
    );
}

#[test]
fn test_classify_free_wrappers_return_free() {
    assert_eq!(classify_rc_call("ori_list_free_data"), Some(RcOpKind::Free));
}

// COW variant coverage

#[test]
fn test_classify_various_cow_functions_return_cow() {
    assert_eq!(classify_rc_call("ori_list_pop_cow"), Some(RcOpKind::Cow));
    assert_eq!(classify_rc_call("ori_list_set_cow"), Some(RcOpKind::Cow));
    assert_eq!(classify_rc_call("ori_map_insert_cow"), Some(RcOpKind::Cow));
    assert_eq!(classify_rc_call("ori_set_insert_cow"), Some(RcOpKind::Cow));
    assert_eq!(classify_rc_call("ori_list_sort_cow"), Some(RcOpKind::Cow));
    assert_eq!(classify_rc_call("ori_set_union_cow"), Some(RcOpKind::Cow));
}

// Non-counting helpers (must return None)

#[test]
fn test_classify_non_counting_helpers_return_none() {
    assert_eq!(classify_rc_call("ori_rc_is_unique"), None);
    assert_eq!(classify_rc_call("ori_rc_is_unique_or_null"), None);
    assert_eq!(classify_rc_call("ori_rc_live_count"), None);
    assert_eq!(classify_rc_call("ori_rc_reset_live_count"), None);
    assert_eq!(classify_rc_call("ori_rc_realloc"), None);
    assert_eq!(classify_rc_call("ori_list_reset_buffer"), None);
}

// Unrelated function names

#[test]
fn test_classify_unrelated_names_return_none() {
    assert_eq!(classify_rc_call("ori_str_to_uppercase"), None);
    assert_eq!(classify_rc_call("main"), None);
    assert_eq!(classify_rc_call("printf"), None);
    assert_eq!(classify_rc_call("ori_str_len"), None);
}

// Module-level histogram tests with synthetic inkwell modules

#[test]
fn test_empty_module_produces_empty_histogram() {
    let context = inkwell::context::Context::create();
    let module = context.create_module("test");
    let options = AuditOptions::default();

    let result = collect_module_histogram(&module, &options);
    assert!(result.is_empty());
}

#[test]
fn test_module_with_rc_alloc_and_dec_counts_per_block() {
    let context = inkwell::context::Context::create();
    let module = context.create_module("test");

    // Declare RC runtime functions as externals.
    let ptr_type = context.ptr_type(inkwell::AddressSpace::default());
    let void_type = context.void_type();

    let alloc_fn = module.add_function("ori_rc_alloc", ptr_type.fn_type(&[], false), None);
    let dec_fn = module.add_function(
        "ori_rc_dec",
        void_type.fn_type(&[ptr_type.into()], false),
        None,
    );

    // Create a test function with two named blocks.
    let test_fn = module.add_function("test_fn", void_type.fn_type(&[], false), None);

    let entry = context.append_basic_block(test_fn, "entry");
    let exit = context.append_basic_block(test_fn, "exit");

    let builder = context.create_builder();

    // Entry block: call ori_rc_alloc.
    builder.position_at_end(entry);
    builder.build_call(alloc_fn, &[], "ptr").unwrap();
    builder.build_unconditional_branch(exit).unwrap();

    // Exit block: call ori_rc_dec (null ptr — histogram only counts calls).
    builder.position_at_end(exit);
    builder
        .build_call(dec_fn, &[ptr_type.const_null().into()], "")
        .unwrap();
    builder.build_return(None).unwrap();

    let options = AuditOptions::default();
    let result = collect_module_histogram(&module, &options);

    // Only test_fn has a body; alloc_fn and dec_fn are declarations.
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "test_fn");
    assert_eq!(result[0].blocks.len(), 2);

    // Entry block: 1 alloc, 0 inc, 0 dec, 0 free, 0 cow.
    assert_eq!(result[0].blocks[0].label, "entry");
    assert_eq!(result[0].blocks[0].counts, [1, 0, 0, 0, 0]);

    // Exit block: 0 alloc, 0 inc, 1 dec, 0 free, 0 cow.
    assert_eq!(result[0].blocks[1].label, "exit");
    assert_eq!(result[0].blocks[1].counts, [0, 0, 1, 0, 0]);
}

#[test]
fn test_unnamed_blocks_get_fallback_labels() {
    let context = inkwell::context::Context::create();
    let module = context.create_module("test");

    let void_type = context.void_type();
    let test_fn = module.add_function("test_fn", void_type.fn_type(&[], false), None);

    // Append blocks with empty names (unnamed).
    let bb0 = context.append_basic_block(test_fn, "");
    let bb1 = context.append_basic_block(test_fn, "");

    let builder = context.create_builder();
    builder.position_at_end(bb0);
    builder.build_unconditional_branch(bb1).unwrap();
    builder.position_at_end(bb1);
    builder.build_return(None).unwrap();

    let options = AuditOptions::default();
    let result = collect_module_histogram(&module, &options);

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].blocks[0].label, "bb_0");
    assert_eq!(result[0].blocks[1].label, "bb_1");
}

#[test]
fn test_function_filter_excludes_non_matching_functions() {
    let context = inkwell::context::Context::create();
    let module = context.create_module("test");

    let void_type = context.void_type();
    let fn_type = void_type.fn_type(&[], false);

    // Two functions with bodies.
    let fn_a = module.add_function("fn_a", fn_type, None);
    let fn_b = module.add_function("fn_b", fn_type, None);

    let builder = context.create_builder();

    let bb_a = context.append_basic_block(fn_a, "entry");
    builder.position_at_end(bb_a);
    builder.build_return(None).unwrap();

    let bb_b = context.append_basic_block(fn_b, "entry");
    builder.position_at_end(bb_b);
    builder.build_return(None).unwrap();

    let options = AuditOptions {
        function_filter: Some("fn_a".to_string()),
        ..Default::default()
    };

    let result = collect_module_histogram(&module, &options);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "fn_a");
}
