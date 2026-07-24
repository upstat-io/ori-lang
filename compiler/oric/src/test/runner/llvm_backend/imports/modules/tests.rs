use std::path::Path;

use ori_ir::SharedInterner;

use super::qualified_function_name;

#[test]
fn qualified_identity_distinguishes_same_named_functions_from_different_modules() {
    let interner = SharedInterner::new();
    let helper = interner.intern("helper");

    let left = qualified_function_name(&interner, Path::new("pkg/left.ori"), helper);
    let right = qualified_function_name(&interner, Path::new("pkg/right.ori"), helper);

    assert_ne!(left, right);
}

#[test]
fn qualified_identity_is_stable_for_one_normalized_module_path() {
    let interner = SharedInterner::new();
    let helper = interner.intern("helper");
    let path = Path::new("pkg/module.ori");

    let first = qualified_function_name(&interner, path, helper);
    let second = qualified_function_name(&interner, path, helper);

    assert_eq!(first, second);
}
