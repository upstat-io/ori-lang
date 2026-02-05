//! Function and Global Declaration Helpers
//!
//! Follows Rust's `rustc_codegen_llvm/src/declare.rs` pattern.
//!
//! This module provides helpers for declaring functions and globals in LLVM.
//! Declarations create symbols without bodies - the bodies are filled in
//! during the define phase.
//!
//! Two-phase codegen:
//! 1. **Predefine**: Declare all symbols (functions, globals)
//! 2. **Define**: Generate function bodies
//!
//! This separation allows forward references (A calls B before B is defined).

use inkwell::module::Linkage;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::FunctionValue;

use ori_ir::Name;
use ori_types::Idx;

use crate::context::CodegenCx;

impl<'ll> CodegenCx<'ll, '_> {
    /// Declare a function with the given name and signature.
    ///
    /// This creates a function declaration (no body) that can be called
    /// or defined later. If a function with this name already exists,
    /// returns the existing declaration.
    pub fn declare_fn(
        &self,
        name: Name,
        param_types: &[Idx],
        return_type: Idx,
    ) -> FunctionValue<'ll> {
        let fn_name = self.interner.lookup(name);

        // Check if already declared
        if let Some(func) = self.scx.llmod.get_function(fn_name) {
            return func;
        }

        // Build parameter types
        let param_llvm_types: Vec<BasicMetadataTypeEnum<'ll>> = param_types
            .iter()
            .map(|&t| self.llvm_type(t).into())
            .collect();

        // Build function type
        let fn_type = if return_type == Idx::UNIT {
            self.scx.type_void_func(&param_llvm_types)
        } else {
            self.scx
                .type_func(&param_llvm_types, self.llvm_type(return_type))
        };

        // Add function to module
        let func = self.scx.llmod.add_function(fn_name, fn_type, None);

        // Cache the function
        self.register_function(name, func);

        func
    }

    /// Declare a function with explicit LLVM types.
    ///
    /// Used when the caller has already computed the LLVM types.
    pub fn declare_fn_with_types(
        &self,
        name: &str,
        param_types: &[BasicMetadataTypeEnum<'ll>],
        return_type: Option<BasicTypeEnum<'ll>>,
    ) -> FunctionValue<'ll> {
        // Check if already declared
        if let Some(func) = self.scx.llmod.get_function(name) {
            return func;
        }

        // Build function type
        let fn_type = match return_type {
            Some(ret) => self.scx.type_func(param_types, ret),
            None => self.scx.type_void_func(param_types),
        };

        // Add function to module
        self.scx.llmod.add_function(name, fn_type, None)
    }

    /// Declare an external function (from runtime library).
    ///
    /// External functions have `External` linkage and are resolved at link time.
    pub fn declare_extern_fn(
        &self,
        name: &str,
        param_types: &[BasicMetadataTypeEnum<'ll>],
        return_type: Option<BasicTypeEnum<'ll>>,
    ) -> FunctionValue<'ll> {
        // Check if already declared
        if let Some(func) = self.scx.llmod.get_function(name) {
            return func;
        }

        // Build function type
        let fn_type = match return_type {
            Some(ret) => self.scx.type_func(param_types, ret),
            None => self.scx.type_void_func(param_types),
        };

        // Add function with external linkage
        self.scx
            .llmod
            .add_function(name, fn_type, Some(Linkage::External))
    }

    /// Declare all runtime functions.
    ///
    /// These are external functions provided by the Ori runtime library.
    /// They are declared with external linkage and resolved at link/JIT time.
    pub fn declare_runtime_functions(&self) {
        let void = None;
        let i64_ty: BasicTypeEnum<'ll> = self.scx.type_i64().into();
        let i32_ty: BasicTypeEnum<'ll> = self.scx.type_i32().into();
        let f64_ty: BasicTypeEnum<'ll> = self.scx.type_f64().into();
        let bool_ty: BasicTypeEnum<'ll> = self.scx.type_i1().into();
        let ptr_ty: BasicTypeEnum<'ll> = self.scx.type_ptr().into();
        let str_ty: BasicTypeEnum<'ll> = self.string_type().into();

        // I/O functions
        self.declare_extern_fn("ori_print", &[ptr_ty.into()], void);
        self.declare_extern_fn("ori_print_int", &[i64_ty.into()], void);
        self.declare_extern_fn("ori_print_float", &[f64_ty.into()], void);
        self.declare_extern_fn("ori_print_bool", &[bool_ty.into()], void);

        // Panic functions
        self.declare_extern_fn("ori_panic", &[ptr_ty.into()], void);
        self.declare_extern_fn("ori_panic_cstr", &[ptr_ty.into()], void);

        // Assertion functions
        self.declare_extern_fn("ori_assert", &[bool_ty.into()], void);
        self.declare_extern_fn("ori_assert_eq_int", &[i64_ty.into(), i64_ty.into()], void);
        self.declare_extern_fn(
            "ori_assert_eq_bool",
            &[bool_ty.into(), bool_ty.into()],
            void,
        );
        self.declare_extern_fn("ori_assert_eq_str", &[ptr_ty.into(), ptr_ty.into()], void);

        // List functions
        self.declare_extern_fn(
            "ori_list_new",
            &[i64_ty.into(), i64_ty.into()],
            Some(ptr_ty),
        );
        self.declare_extern_fn("ori_list_free", &[ptr_ty.into(), i64_ty.into()], void);
        self.declare_extern_fn("ori_list_len", &[ptr_ty.into()], Some(i64_ty));

        // Comparison functions
        self.declare_extern_fn(
            "ori_compare_int",
            &[i64_ty.into(), i64_ty.into()],
            Some(i32_ty),
        );
        self.declare_extern_fn("ori_min_int", &[i64_ty.into(), i64_ty.into()], Some(i64_ty));
        self.declare_extern_fn("ori_max_int", &[i64_ty.into(), i64_ty.into()], Some(i64_ty));

        // String functions
        self.declare_extern_fn(
            "ori_str_concat",
            &[ptr_ty.into(), ptr_ty.into()],
            Some(str_ty),
        );
        self.declare_extern_fn("ori_str_eq", &[ptr_ty.into(), ptr_ty.into()], Some(bool_ty));
        self.declare_extern_fn("ori_str_ne", &[ptr_ty.into(), ptr_ty.into()], Some(bool_ty));

        // Type conversion functions
        self.declare_extern_fn("ori_str_from_int", &[i64_ty.into()], Some(str_ty));
        self.declare_extern_fn("ori_str_from_bool", &[bool_ty.into()], Some(str_ty));
        self.declare_extern_fn("ori_str_from_float", &[f64_ty.into()], Some(str_ty));

        // Closure boxing
        self.declare_extern_fn("ori_closure_box", &[i64_ty.into()], Some(ptr_ty));
    }

    /// Get a declared function by name, or None if not declared.
    pub fn get_declared_fn(&self, name: &str) -> Option<FunctionValue<'ll>> {
        self.scx.llmod.get_function(name)
    }

    /// Declare an external function with a pre-mangled name.
    ///
    /// This is used to declare imported functions from other modules.
    /// The caller is responsible for providing the correctly mangled name.
    ///
    /// # Arguments
    ///
    /// * `mangled_name` - The fully mangled symbol name (e.g., `_ori_helper$add`)
    /// * `param_types` - LLVM types for the parameters
    /// * `return_type` - LLVM return type, or `None` for void
    ///
    /// # Returns
    ///
    /// The function declaration with external linkage.
    pub fn declare_external_fn_mangled(
        &self,
        mangled_name: &str,
        param_types: &[BasicMetadataTypeEnum<'ll>],
        return_type: Option<BasicTypeEnum<'ll>>,
    ) -> FunctionValue<'ll> {
        // Check if already declared
        if let Some(func) = self.scx.llmod.get_function(mangled_name) {
            return func;
        }

        // Build function type
        let fn_type = match return_type {
            Some(ret) => self.scx.type_func(param_types, ret),
            None => self.scx.type_void_func(param_types),
        };

        // Add function with external linkage (resolved at link time)
        self.scx
            .llmod
            .add_function(mangled_name, fn_type, Some(Linkage::External))
    }

    /// Declare a global variable.
    ///
    /// Creates a global variable declaration that can be defined later.
    pub fn declare_global(
        &self,
        name: &str,
        ty: BasicTypeEnum<'ll>,
    ) -> inkwell::values::GlobalValue<'ll> {
        // Check if already declared
        if let Some(global) = self.scx.llmod.get_global(name) {
            return global;
        }

        self.scx.llmod.add_global(ty, None, name)
    }

    /// Define a global variable with an initializer.
    pub fn define_global(
        &self,
        name: &str,
        ty: BasicTypeEnum<'ll>,
        initializer: inkwell::values::BasicValueEnum<'ll>,
    ) -> inkwell::values::GlobalValue<'ll> {
        let global = self.declare_global(name, ty);
        global.set_initializer(&initializer);
        global
    }

    /// Declare a global string constant.
    ///
    /// Returns a pointer to the string data. The string is stored as a
    /// null-terminated constant in the data section.
    pub fn declare_global_string(
        &self,
        value: &str,
        name: &str,
    ) -> inkwell::values::GlobalValue<'ll> {
        let bytes = value.as_bytes();
        let array_type = self.scx.type_i8().array_type((bytes.len() + 1) as u32);

        // Create initializer with null terminator
        let mut init_values: Vec<inkwell::values::IntValue<'ll>> = bytes
            .iter()
            .map(|&b| self.scx.type_i8().const_int(u64::from(b), false))
            .collect();
        init_values.push(self.scx.type_i8().const_int(0, false)); // null terminator

        let initializer = self.scx.type_i8().const_array(&init_values);

        let global = self.scx.llmod.add_global(array_type, None, name);
        global.set_initializer(&initializer);
        global.set_constant(true);
        global.set_linkage(Linkage::Private);
        global
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkwell::context::Context;
    use ori_ir::StringInterner;

    #[test]
    fn test_declare_function() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let name = interner.intern("add");
        let func = cx.declare_fn(name, &[Idx::INT, Idx::INT], Idx::INT);

        // Verify function was created
        assert_eq!(func.get_name().to_str().unwrap(), "add");
        assert_eq!(func.count_params(), 2);

        // Second declaration should return same function
        let func2 = cx.declare_fn(name, &[Idx::INT, Idx::INT], Idx::INT);
        assert_eq!(func, func2);
    }

    #[test]
    fn test_declare_void_function() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let name = interner.intern("print_hello");
        let func = cx.declare_fn(name, &[], Idx::UNIT);

        assert_eq!(func.get_name().to_str().unwrap(), "print_hello");
        assert_eq!(func.count_params(), 0);
    }

    #[test]
    fn test_declare_runtime() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        cx.declare_runtime_functions();

        // Verify some runtime functions exist
        assert!(cx.get_declared_fn("ori_print").is_some());
        assert!(cx.get_declared_fn("ori_assert").is_some());
        assert!(cx.get_declared_fn("ori_str_concat").is_some());
    }

    #[test]
    fn test_declare_global() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let global = cx.declare_global("my_global", cx.scx.type_i64().into());
        assert_eq!(global.get_name().to_str().unwrap(), "my_global");

        // Define with initializer
        let initialized = cx.define_global(
            "my_init_global",
            cx.scx.type_i64().into(),
            cx.scx.type_i64().const_int(42, false).into(),
        );
        assert!(initialized.get_initializer().is_some());
    }

    #[test]
    fn test_declare_global_string() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let global = cx.declare_global_string("hello", "str_hello");
        assert_eq!(global.get_name().to_str().unwrap(), "str_hello");
        assert!(global.is_constant());
    }

    #[test]
    fn test_declare_external_fn_mangled() {
        let context = Context::create();
        let interner = StringInterner::new();
        let cx = CodegenCx::new(&context, &interner, "test");

        let i64_ty: BasicTypeEnum = cx.scx.type_i64().into();
        let func = cx.declare_external_fn_mangled(
            "_ori_helper$add",
            &[i64_ty.into(), i64_ty.into()],
            Some(i64_ty),
        );

        assert_eq!(func.get_name().to_str().unwrap(), "_ori_helper$add");
        assert_eq!(func.count_params(), 2);

        // Second call should return same function
        let func2 = cx.declare_external_fn_mangled(
            "_ori_helper$add",
            &[i64_ty.into(), i64_ty.into()],
            Some(i64_ty),
        );
        assert_eq!(func, func2);
    }
}
