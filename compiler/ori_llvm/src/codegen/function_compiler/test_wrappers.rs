//! Test wrapper function compilation.
//!
//! On platforms with Itanium EH each test produces an inner body plus an
//! outer `invoke`/`landingpad` wrapper for panic capture; on Windows JIT the
//! wrapper is skipped and panic recovery is handled by `jit_run_protected`.

use ori_ir::{Name, TestDef};
use ori_types::Idx;
use rustc_hash::FxHashMap;
use tracing::{debug, warn};

use super::FunctionCompiler;
use crate::codegen::abi::{select_call_conv, CallConvSite, FunctionAbi, ReturnAbi, ReturnPassing};

impl<'scx: 'ctx, 'ctx> FunctionCompiler<'_, 'scx, 'ctx, '_> {
    /// Compile test definitions as wrapper functions.
    ///
    /// On platforms with Itanium EH (Linux, macOS), each test produces two
    /// layers:
    /// 1. **Inner body** (`_ori_test_<name>_body`): the actual test code compiled
    ///    through the full ARC pipeline.
    /// 2. **Outer wrapper** (`_ori_test_<name>`): uses `invoke` to call the inner
    ///    body with a catch-all `landingpad`. Uncaught panics are caught here
    ///    and stored via `ori_catch_cleanup` so the JIT runner can read the
    ///    panic message.
    ///
    /// On Windows JIT (MSVC target with Itanium EH model), the LLVM JIT cannot
    /// compile Itanium-style `landingpad` for an MSVC target, so we emit a
    /// single function without the invoke/landingpad wrapper. The JIT runner
    /// uses `jit_run_protected` (C++ try/catch) for panic recovery instead.
    ///
    /// Returns a map of `test_name → wrapper_function_name` for the JIT to call.
    pub fn compile_tests(&mut self, tests: &[&TestDef]) -> FxHashMap<Name, String> {
        let mut test_wrappers = FxHashMap::default();

        // On Windows JIT, landingpad with Itanium EH on an MSVC target causes
        // stack overflow during LLVM JIT compilation. Skip the invoke wrapper.
        let use_invoke_wrapper = !(self.builder.is_jit() && cfg!(target_os = "windows"));

        for test in tests {
            let test_name_str = self.interner.lookup(test.name);
            let wrapper_name = self
                .mangler
                .mangle_function(self.module_path, &format!("test_{test_name_str}"));

            debug!(name = test_name_str, wrapper = %wrapper_name, "compiling test");

            let abi = FunctionAbi {
                params: vec![],
                return_abi: ReturnAbi {
                    ty: Idx::UNIT,
                    passing: ReturnPassing::Void,
                },
                call_conv: select_call_conv(CallConvSite::TestWrapper),
            };

            let Some((arc_func, lambdas)) = self.clone_bound_family(test.name, &abi) else {
                continue;
            };

            let emitted = if use_invoke_wrapper {
                self.compile_test_with_invoke_wrapper(test, &wrapper_name, &abi, arc_func, lambdas)
            } else {
                self.compile_test_without_invoke_wrapper(
                    test,
                    &wrapper_name,
                    &abi,
                    arc_func,
                    lambdas,
                )
            };

            if emitted {
                test_wrappers.insert(test.name, wrapper_name);
            }
        }

        test_wrappers
    }

    /// Compile a single test with the Itanium EH invoke/landingpad wrapper.
    ///
    /// Returns `true` if the test was successfully emitted (both inner body
    /// and outer wrapper); `false` if the PC-2 contract check on the inner
    /// body fired (per-test failure — outer wrapper skipped).
    fn compile_test_with_invoke_wrapper(
        &mut self,
        test: &TestDef,
        wrapper_name: &str,
        abi: &FunctionAbi,
        arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) -> bool {
        let test_name_str = self.interner.lookup(test.name);
        let body_name = format!("{wrapper_name}_body");

        // Inner body function (the actual test code)
        let body_func_id = self.builder.declare_void_function(&body_name, &[]);
        self.builder.set_ccc(body_func_id);
        self.builder.set_current_function(body_func_id);

        if let Err(err) = self.emit_arc_function(test.name, body_func_id, abi, arc_func, lambdas) {
            // PC-2 contract violation — error already recorded via
            // `record_codegen_error()` inside the hook. Skip this test's
            // outer wrapper; the suite continues past this failure.
            warn!(
                name = test_name_str,
                ?err,
                "PC-2 contract violation — skipping test body"
            );
            return false;
        }

        // Outer wrapper with catch-all exception handling
        let outer_func_id = self.builder.declare_void_function(wrapper_name, &[]);
        self.builder.set_ccc(outer_func_id);
        self.builder.set_current_function(outer_func_id);

        let eh_model = self.builder.eh_model();
        let personality_name = eh_model.personality_name();
        let personality_id = self.builder.runtime_fn(personality_name);
        self.builder.set_personality(outer_func_id, personality_id);

        let entry_block = self.builder.append_block(outer_func_id, "entry");
        let normal_block = self.builder.append_block(outer_func_id, "normal");
        let catch_block = self.builder.append_block(outer_func_id, "catch");

        self.builder.position_at_end(entry_block);
        self.builder
            .invoke(body_func_id, &[], normal_block, catch_block, "");

        self.builder.position_at_end(normal_block);
        self.builder.ret_void();

        self.builder.position_at_end(catch_block);
        let lp = self.builder.landingpad_catch_all(personality_id, "lp.test");
        if let Some(exc_ptr) = self.builder.extract_value(lp, 0, "exc.ptr") {
            let cleanup_fn = self.builder.runtime_fn("ori_catch_cleanup");
            self.builder.call(cleanup_fn, &[exc_ptr], "");
        }
        let report_panic = self.builder.runtime_fn("ori_report_uncaught_panic");
        self.builder.call(report_panic, &[], "");
        self.builder.ret_void();

        // Function-level LLVM IR verification for the outer wrapper.
        if self.verify_arc {
            let outer_fn_val = self.builder.get_function_value(outer_func_id);
            if !outer_fn_val.verify(true) {
                tracing::error!(
                    name = test_name_str,
                    "LLVM IR verification failed (compile_tests outer wrapper)"
                );
                self.builder.record_codegen_error();
            }
        }

        true
    }

    /// Compile a single test without the invoke/landingpad wrapper
    /// (Windows JIT path; panic recovery handled by `jit_run_protected`).
    ///
    /// Returns `true` if the test was successfully emitted; `false` if the
    /// PC-2 contract check fired.
    fn compile_test_without_invoke_wrapper(
        &mut self,
        test: &TestDef,
        wrapper_name: &str,
        abi: &FunctionAbi,
        arc_func: ori_arc::ArcFunction,
        lambdas: Vec<ori_arc::ArcFunction>,
    ) -> bool {
        let test_name_str = self.interner.lookup(test.name);
        let func_id = self.builder.declare_void_function(wrapper_name, &[]);
        self.builder.set_ccc(func_id);
        self.builder.set_current_function(func_id);

        if let Err(err) = self.emit_arc_function(test.name, func_id, abi, arc_func, lambdas) {
            // PC-2 contract violation — error already recorded via
            // `record_codegen_error()` inside the hook. Skip wrapper
            // insertion so the test harness does not try to invoke an
            // incompletely-emitted function.
            warn!(
                name = test_name_str,
                ?err,
                "PC-2 contract violation — skipping test (Windows JIT)"
            );
            return false;
        }

        true
    }
}
