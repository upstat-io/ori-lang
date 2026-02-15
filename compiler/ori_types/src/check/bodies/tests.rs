use super::*;
use crate::{check::ModuleChecker, TypeEnv};
use ori_ir::{ExprArena, StringInterner};

#[test]
fn check_empty_module_bodies() {
    let arena = ExprArena::new();
    let interner = StringInterner::new();
    let mut checker = ModuleChecker::new(&arena, &interner);

    // Freeze base env (simulating Pass 1)
    checker.freeze_base_env(TypeEnv::new());

    let module = Module::default();

    // These should not panic with empty module
    check_function_bodies(&mut checker, &module);
    check_test_bodies(&mut checker, &module);
    check_impl_bodies(&mut checker, &module);
    check_def_impl_bodies(&mut checker, &module);

    assert!(!checker.has_errors());
}
