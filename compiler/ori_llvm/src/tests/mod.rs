//! Test modules for LLVM codegen.

/// Macro to set up common test infrastructure.
///
/// Creates: `context`, `interner`, `codegen`, and `arena` variables.
///
/// # Example
/// ```ignore
/// #[test]
/// fn test_example() {
///     setup_test!(test_example);
///     // now use: context, interner, codegen, arena
/// }
/// ```
#[macro_export]
macro_rules! setup_test {
    ($name:ident) => {
        let context = inkwell::context::Context::create();
        let interner = ori_ir::StringInterner::new();
        let codegen =
            $crate::tests::helper::TestCodegen::new(&context, &interner, stringify!($name));
        let mut arena = ori_ir::ExprArena::new();
    };
}

mod advanced_control_flow_tests;
mod arithmetic_tests;
mod builtins_tests;
mod collection_tests;
mod control_flow_tests;
mod evaluator_tests;
mod function_call_tests;
mod function_exp_tests;
mod function_seq_tests;
mod function_tests;
mod matching_tests;
mod more_control_flow_tests;
mod operator_tests;
mod runtime_tests;
mod string_tests;
mod type_conversion_tests;

// Test helper for the new architecture
pub mod helper {
    use inkwell::basic_block::BasicBlock;
    use inkwell::context::Context;
    use inkwell::values::FunctionValue;
    use inkwell::OptimizationLevel;
    use ori_ir::{ExprArena, ExprId, Name, StringInterner};
    use ori_types::Idx;

    use crate::builder::{Builder, Locals};
    use crate::context::CodegenCx;
    use crate::runtime;

    /// Create a test context with a function returning i64.
    ///
    /// Returns `(CodegenCx, FunctionValue)` for low-level builder tests.
    /// This consolidates the common setup pattern used across multiple test files.
    pub fn setup_builder_test<'ll, 'tcx>(
        context: &'ll Context,
        interner: &'tcx StringInterner,
    ) -> (CodegenCx<'ll, 'tcx>, FunctionValue<'ll>) {
        let cx = CodegenCx::new(context, interner, "test");
        cx.declare_runtime_functions();

        let fn_type = cx.scx.type_i64().fn_type(&[], false);
        let function = cx.llmod().add_function("test_fn", fn_type, None);

        (cx, function)
    }

    /// Create a test context with a function and entry block.
    ///
    /// Returns `(CodegenCx, FunctionValue, BasicBlock)` for tests that need
    /// immediate access to the entry block.
    pub fn setup_builder_test_with_entry<'ll, 'tcx>(
        context: &'ll Context,
        interner: &'tcx StringInterner,
    ) -> (CodegenCx<'ll, 'tcx>, FunctionValue<'ll>, BasicBlock<'ll>) {
        let (cx, function) = setup_builder_test(context, interner);
        let entry_bb = cx.llcx().append_basic_block(function, "entry");
        (cx, function, entry_bb)
    }

    /// Test helper that provides a simple API for compiling and running functions.
    pub struct TestCodegen<'ll, 'tcx> {
        pub cx: CodegenCx<'ll, 'tcx>,
    }

    impl<'ll, 'tcx> TestCodegen<'ll, 'tcx> {
        pub fn new(
            context: &'ll Context,
            interner: &'tcx StringInterner,
            module_name: &str,
        ) -> Self {
            let cx = CodegenCx::new(context, interner, module_name);
            cx.declare_runtime_functions();
            Self { cx }
        }

        /// Compile a function with the given signature and body expression.
        pub fn compile_function(
            &self,
            name: Name,
            param_names: &[Name],
            param_types: &[Idx],
            return_type: Idx,
            body: ExprId,
            arena: &ExprArena,
            expr_types: &[Idx],
        ) {
            // Declare the function
            let func = self.cx.declare_fn(name, param_types, return_type);

            // Create entry block
            let entry_bb = self.cx.llcx().append_basic_block(func, "entry");

            // Create builder and compile body
            let builder = Builder::build(&self.cx, entry_bb);

            // Set up locals from parameters (function parameters are immutable)
            let mut locals = Locals::new();
            for (i, &param_name) in param_names.iter().enumerate() {
                let param = func.get_nth_param(i as u32).unwrap();
                param.set_name(self.cx.interner.lookup(param_name));
                locals.bind_immutable(param_name, param);
            }

            // Compile body
            let result = builder.compile_expr(body, arena, expr_types, &mut locals, func, None);

            // Return
            if return_type == Idx::UNIT {
                builder.ret_void();
            } else if let Some(val) = result {
                builder.ret(val);
            } else {
                let default = self.cx.default_value(return_type);
                builder.ret(default);
            }
        }

        /// Print LLVM IR to string.
        pub fn print_to_string(&self) -> String {
            self.cx.llmod().print_to_string().to_string()
        }

        /// JIT execute a function that returns i64.
        #[allow(unsafe_code)]
        pub fn jit_execute_i64(&self, fn_name: &str) -> Result<i64, String> {
            let ee = self
                .cx
                .llmod()
                .create_jit_execution_engine(OptimizationLevel::None)
                .map_err(|e| e.to_string())?;

            // Add runtime mappings
            self.add_runtime_mappings(&ee);

            unsafe {
                let func = ee
                    .get_function::<unsafe extern "C" fn() -> i64>(fn_name)
                    .map_err(|e| format!("Function '{fn_name}' not found: {e}"))?;
                Ok(func.call())
            }
        }

        /// JIT execute a function that returns bool.
        #[allow(unsafe_code)]
        pub fn jit_execute_bool(&self, fn_name: &str) -> Result<bool, String> {
            let ee = self
                .cx
                .llmod()
                .create_jit_execution_engine(OptimizationLevel::None)
                .map_err(|e| e.to_string())?;

            self.add_runtime_mappings(&ee);

            unsafe {
                let func = ee
                    .get_function::<unsafe extern "C" fn() -> bool>(fn_name)
                    .map_err(|e| format!("Function '{fn_name}' not found: {e}"))?;
                Ok(func.call())
            }
        }

        fn add_runtime_mappings(&self, ee: &inkwell::execution_engine::ExecutionEngine<'ll>) {
            let mappings: &[(&str, usize)] = &[
                ("ori_print", runtime::ori_print as *const () as usize),
                (
                    "ori_print_int",
                    runtime::ori_print_int as *const () as usize,
                ),
                (
                    "ori_print_float",
                    runtime::ori_print_float as *const () as usize,
                ),
                (
                    "ori_print_bool",
                    runtime::ori_print_bool as *const () as usize,
                ),
                ("ori_panic", runtime::ori_panic as *const () as usize),
                (
                    "ori_panic_cstr",
                    runtime::ori_panic_cstr as *const () as usize,
                ),
                ("ori_assert", runtime::ori_assert as *const () as usize),
                (
                    "ori_assert_eq_int",
                    runtime::ori_assert_eq_int as *const () as usize,
                ),
                (
                    "ori_assert_eq_bool",
                    runtime::ori_assert_eq_bool as *const () as usize,
                ),
                ("ori_list_new", runtime::ori_list_new as *const () as usize),
                (
                    "ori_list_free",
                    runtime::ori_list_free as *const () as usize,
                ),
                ("ori_list_len", runtime::ori_list_len as *const () as usize),
                (
                    "ori_compare_int",
                    runtime::ori_compare_int as *const () as usize,
                ),
                ("ori_min_int", runtime::ori_min_int as *const () as usize),
                ("ori_max_int", runtime::ori_max_int as *const () as usize),
                (
                    "ori_str_concat",
                    runtime::ori_str_concat as *const () as usize,
                ),
                ("ori_str_eq", runtime::ori_str_eq as *const () as usize),
                ("ori_str_ne", runtime::ori_str_ne as *const () as usize),
                (
                    "ori_assert_eq_str",
                    runtime::ori_assert_eq_str as *const () as usize,
                ),
            ];

            let module = self.cx.llmod();
            for &(name, addr) in mappings {
                if let Some(func) = module.get_function(name) {
                    ee.add_global_mapping(&func, addr);
                }
            }
        }
    }
}
