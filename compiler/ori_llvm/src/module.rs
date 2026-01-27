//! Module-level LLVM compilation.
//!
//! Compiles an entire Ori module (all functions, tests) to LLVM IR.

use std::collections::HashMap;

use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::values::FunctionValue;
use inkwell::AddressSpace;

use ori_ir::{ExprArena, Function, Name, StringInterner, TestDef, TypeId};

use crate::LLVMCodegen;

/// Compiler for a complete Ori module.
pub struct ModuleCompiler<'ctx> {
    codegen: LLVMCodegen<'ctx>,
    /// Map from function names to their LLVM function values.
    functions: HashMap<Name, FunctionValue<'ctx>>,
    /// Map from test names to their LLVM function values.
    tests: HashMap<Name, FunctionValue<'ctx>>,
}

impl<'ctx> ModuleCompiler<'ctx> {
    /// Create a new module compiler.
    pub fn new(context: &'ctx Context, interner: &'ctx StringInterner, module_name: &str) -> Self {
        let codegen = LLVMCodegen::new(context, interner, module_name);

        Self {
            codegen,
            functions: HashMap::new(),
            tests: HashMap::new(),
        }
    }

    /// Get the underlying codegen.
    pub fn codegen(&self) -> &LLVMCodegen<'ctx> {
        &self.codegen
    }

    /// Get the LLVM module.
    pub fn module(&self) -> &inkwell::module::Module<'ctx> {
        self.codegen.module()
    }

    /// Declare runtime functions that Ori code can call.
    pub fn declare_runtime(&self) {
        let context = self.codegen.context;
        let module = self.codegen.module();

        let void_type = context.void_type();
        let i64_type = context.i64_type();
        let i32_type = context.i32_type();
        let _i8_type = context.i8_type();
        let f64_type = context.f64_type();
        let bool_type = context.bool_type();
        let ptr_type = context.ptr_type(AddressSpace::default());

        // void ori_print(OriStr*)
        let print_type = void_type.fn_type(&[ptr_type.into()], false);
        module.add_function("ori_print", print_type, Some(Linkage::External));

        // void ori_print_int(i64)
        let print_int_type = void_type.fn_type(&[i64_type.into()], false);
        module.add_function("ori_print_int", print_int_type, Some(Linkage::External));

        // void ori_print_float(f64)
        let print_float_type = void_type.fn_type(&[f64_type.into()], false);
        module.add_function("ori_print_float", print_float_type, Some(Linkage::External));

        // void ori_print_bool(bool)
        let print_bool_type = void_type.fn_type(&[bool_type.into()], false);
        module.add_function("ori_print_bool", print_bool_type, Some(Linkage::External));

        // void ori_panic(OriStr*)
        let panic_type = void_type.fn_type(&[ptr_type.into()], false);
        module.add_function("ori_panic", panic_type, Some(Linkage::External));

        // void ori_panic_cstr(i8*)
        let panic_cstr_type = void_type.fn_type(&[ptr_type.into()], false);
        module.add_function("ori_panic_cstr", panic_cstr_type, Some(Linkage::External));

        // void ori_assert(bool)
        let assert_type = void_type.fn_type(&[bool_type.into()], false);
        module.add_function("ori_assert", assert_type, Some(Linkage::External));

        // void ori_assert_eq_int(i64, i64)
        let assert_eq_int_type = void_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        module.add_function("ori_assert_eq_int", assert_eq_int_type, Some(Linkage::External));

        // void ori_assert_eq_bool(bool, bool)
        let assert_eq_bool_type = void_type.fn_type(&[bool_type.into(), bool_type.into()], false);
        module.add_function("ori_assert_eq_bool", assert_eq_bool_type, Some(Linkage::External));

        // OriList* ori_list_new(i64 capacity, i64 elem_size)
        let list_new_type = ptr_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        module.add_function("ori_list_new", list_new_type, Some(Linkage::External));

        // void ori_list_free(OriList*, i64 elem_size)
        let list_free_type = void_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        module.add_function("ori_list_free", list_free_type, Some(Linkage::External));

        // i64 ori_list_len(OriList*)
        let list_len_type = i64_type.fn_type(&[ptr_type.into()], false);
        module.add_function("ori_list_len", list_len_type, Some(Linkage::External));

        // i32 ori_compare_int(i64, i64)
        let compare_type = i32_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        module.add_function("ori_compare_int", compare_type, Some(Linkage::External));

        // i64 ori_min_int(i64, i64)
        let min_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        module.add_function("ori_min_int", min_type, Some(Linkage::External));

        // i64 ori_max_int(i64, i64)
        let max_type = i64_type.fn_type(&[i64_type.into(), i64_type.into()], false);
        module.add_function("ori_max_int", max_type, Some(Linkage::External));

        // String type is { i64, ptr }
        let str_type = context.struct_type(&[i64_type.into(), ptr_type.into()], false);

        // OriStr ori_str_concat(OriStr*, OriStr*)
        let str_concat_type = str_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        module.add_function("ori_str_concat", str_concat_type, Some(Linkage::External));

        // bool ori_str_eq(OriStr*, OriStr*)
        let str_eq_type = bool_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        module.add_function("ori_str_eq", str_eq_type, Some(Linkage::External));

        // bool ori_str_ne(OriStr*, OriStr*)
        let str_ne_type = bool_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        module.add_function("ori_str_ne", str_ne_type, Some(Linkage::External));

        // void ori_assert_eq_str(OriStr*, OriStr*)
        let assert_eq_str_type = void_type.fn_type(&[ptr_type.into(), ptr_type.into()], false);
        module.add_function("ori_assert_eq_str", assert_eq_str_type, Some(Linkage::External));
    }

    /// Compile a function definition.
    pub fn compile_function(
        &mut self,
        func: &Function,
        arena: &ExprArena,
        expr_types: &[TypeId],
    ) {
        // Get parameter names and types
        let params = arena.get_params(func.params);
        let param_names: Vec<Name> = params.iter().map(|p| p.name).collect();

        // For now, use INT for all types (proper type mapping would come from type checker)
        let param_types: Vec<TypeId> = params.iter().map(|_| TypeId::INT).collect();

        // Get return type (default to INT)
        let return_type = TypeId::INT;

        // Compile the function
        let llvm_func = self.codegen.compile_function(
            func.name,
            &param_names,
            &param_types,
            return_type,
            func.body,
            arena,
            expr_types,
        );

        self.functions.insert(func.name, llvm_func);
    }

    /// Compile a test definition.
    ///
    /// Tests are compiled as void functions that call assertions.
    pub fn compile_test(
        &mut self,
        test: &TestDef,
        arena: &ExprArena,
        expr_types: &[TypeId],
    ) {
        // Tests are void -> void functions
        let llvm_func = self.codegen.compile_function(
            test.name,
            &[],
            &[],
            TypeId::VOID,
            test.body,
            arena,
            expr_types,
        );

        self.tests.insert(test.name, llvm_func);
    }

    /// Get a compiled function by name.
    pub fn get_function(&self, name: Name) -> Option<FunctionValue<'ctx>> {
        self.functions.get(&name).copied()
    }

    /// Get a compiled test by name.
    pub fn get_test(&self, name: Name) -> Option<FunctionValue<'ctx>> {
        self.tests.get(&name).copied()
    }

    /// Get all compiled tests.
    pub fn tests(&self) -> &HashMap<Name, FunctionValue<'ctx>> {
        &self.tests
    }

    /// Print LLVM IR to string.
    pub fn print_to_string(&self) -> String {
        self.codegen.print_to_string()
    }

    /// Create JIT execution engine and run a test.
    ///
    /// Returns Ok(()) if test passed, Err(message) if failed.
    #[allow(unsafe_code)]
    pub fn run_test(&self, test_name: &str) -> Result<(), String> {
        use inkwell::OptimizationLevel;

        // Reset panic state before running
        crate::runtime::reset_panic_state();

        // Create JIT execution engine
        let ee = self.codegen.module()
            .create_jit_execution_engine(OptimizationLevel::None)
            .map_err(|e| e.to_string())?;

        // Add runtime function mappings
        self.add_runtime_mappings(&ee);

        // Find the test function
        // SAFETY: We trust LLVM's JIT to correctly compile and execute.
        // The function must exist with signature () -> void.
        unsafe {
            let test_fn = ee
                .get_function::<unsafe extern "C" fn()>(test_name)
                .map_err(|e| format!("Test function '{test_name}' not found: {e}"))?;

            // Run the test
            test_fn.call();
        }

        // Check if panic occurred
        if crate::runtime::did_panic() {
            let msg = crate::runtime::get_panic_message()
                .unwrap_or_else(|| "unknown panic".to_string());
            Err(msg)
        } else {
            Ok(())
        }
    }

    /// Add runtime function mappings to the execution engine.
    fn add_runtime_mappings(&self, ee: &inkwell::execution_engine::ExecutionEngine<'ctx>) {
        use crate::runtime;

        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_print").unwrap_or_else(|| panic!("ori_print not declared")),
            runtime::ori_print as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_print_int").unwrap_or_else(|| panic!("ori_print_int not declared")),
            runtime::ori_print_int as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_print_float").unwrap_or_else(|| panic!("ori_print_float not declared")),
            runtime::ori_print_float as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_print_bool").unwrap_or_else(|| panic!("ori_print_bool not declared")),
            runtime::ori_print_bool as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_panic").unwrap_or_else(|| panic!("ori_panic not declared")),
            runtime::ori_panic as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_panic_cstr").unwrap_or_else(|| panic!("ori_panic_cstr not declared")),
            runtime::ori_panic_cstr as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_assert").unwrap_or_else(|| panic!("ori_assert not declared")),
            runtime::ori_assert as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_assert_eq_int").unwrap_or_else(|| panic!("ori_assert_eq_int not declared")),
            runtime::ori_assert_eq_int as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_assert_eq_bool").unwrap_or_else(|| panic!("ori_assert_eq_bool not declared")),
            runtime::ori_assert_eq_bool as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_list_new").unwrap_or_else(|| panic!("ori_list_new not declared")),
            runtime::ori_list_new as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_list_free").unwrap_or_else(|| panic!("ori_list_free not declared")),
            runtime::ori_list_free as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_list_len").unwrap_or_else(|| panic!("ori_list_len not declared")),
            runtime::ori_list_len as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_compare_int").unwrap_or_else(|| panic!("ori_compare_int not declared")),
            runtime::ori_compare_int as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_min_int").unwrap_or_else(|| panic!("ori_min_int not declared")),
            runtime::ori_min_int as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_max_int").unwrap_or_else(|| panic!("ori_max_int not declared")),
            runtime::ori_max_int as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_str_concat").unwrap_or_else(|| panic!("ori_str_concat not declared")),
            runtime::ori_str_concat as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_str_eq").unwrap_or_else(|| panic!("ori_str_eq not declared")),
            runtime::ori_str_eq as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_str_ne").unwrap_or_else(|| panic!("ori_str_ne not declared")),
            runtime::ori_str_ne as usize,
        );
        ee.add_global_mapping(
            &self.codegen.module().get_function("ori_assert_eq_str").unwrap_or_else(|| panic!("ori_assert_eq_str not declared")),
            runtime::ori_assert_eq_str as usize,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::ast::{Expr, ExprKind, BinaryOp};
    use ori_ir::{GenericParamRange, Param};

    #[test]
    fn test_module_compiler_basic() {
        let context = Context::create();
        let interner = StringInterner::new();
        let mut compiler = ModuleCompiler::new(&context, &interner, "test_module");

        // Declare runtime functions
        compiler.declare_runtime();

        // Create a simple function: fn add(a: int, b: int) -> int { a + b }
        let mut arena = ExprArena::new();

        let a_name = interner.intern("a");
        let b_name = interner.intern("b");

        let a_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(a_name),
            span: ori_ir::Span::new(0, 1),
        });
        let b_ref = arena.alloc_expr(Expr {
            kind: ExprKind::Ident(b_name),
            span: ori_ir::Span::new(0, 1),
        });
        let add_body = arena.alloc_expr(Expr {
            kind: ExprKind::Binary {
                op: BinaryOp::Add,
                left: a_ref,
                right: b_ref,
            },
            span: ori_ir::Span::new(0, 1),
        });

        let add_name = interner.intern("add");
        let params = arena.alloc_params([
            Param {
                name: a_name,
                ty: None,
                span: ori_ir::Span::new(0, 1),
            },
            Param {
                name: b_name,
                ty: None,
                span: ori_ir::Span::new(0, 1),
            },
        ]);

        let func = Function {
            name: add_name,
            generics: GenericParamRange::EMPTY,
            params,
            return_ty: None,
            capabilities: vec![],
            where_clauses: vec![],
            body: add_body,
            span: ori_ir::Span::new(0, 1),
            is_public: false,
        };

        let expr_types = vec![TypeId::INT; 10];
        compiler.compile_function(&func, &arena, &expr_types);

        println!("Module IR:\n{}", compiler.print_to_string());

        // Verify function was compiled
        assert!(compiler.get_function(add_name).is_some());
    }

    #[test]
    fn test_module_with_test() {
        let context = Context::create();
        let interner = StringInterner::new();
        let mut compiler = ModuleCompiler::new(&context, &interner, "test_module");

        // Declare runtime functions
        compiler.declare_runtime();

        // Create a test: @test_add () -> void = run(assert_eq(actual: 1 + 1, expected: 2))
        // Simplified: just assert 1 == 1
        let mut arena = ExprArena::new();

        // true literal (represents 1 == 1 simplified)
        let condition = arena.alloc_expr(Expr {
            kind: ExprKind::Bool(true),
            span: ori_ir::Span::new(0, 1),
        });

        // Call ori_assert(true)
        // For now, just use the condition directly as the body
        let test_body = condition;

        let test_name = interner.intern("test_simple");
        let empty_params = arena.alloc_params([]);
        let test_def = TestDef {
            name: test_name,
            targets: vec![],
            params: empty_params,
            return_ty: None,
            body: test_body,
            span: ori_ir::Span::new(0, 1),
            skip_reason: None,
            expected_errors: vec![],
            fail_expected: None,
        };

        let expr_types = vec![TypeId::BOOL];
        compiler.compile_test(&test_def, &arena, &expr_types);

        println!("Test Module IR:\n{}", compiler.print_to_string());

        // Verify test was compiled
        assert!(compiler.get_test(test_name).is_some());
    }
}
