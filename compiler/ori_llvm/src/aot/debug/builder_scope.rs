//! `DebugInfoBuilder` methods for function/scope/location/variable management and finalization.

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::debug_info::{
    AsDIScope, DICompositeType, DIExpression, DIFlags, DIFlagsConstants, DILexicalBlock,
    DILocalVariable, DILocation, DIScope, DISubprogram, DISubroutineType, DIType,
};
use inkwell::values::{AsValueRef, BasicValueEnum, FunctionValue, InstructionValue, PointerValue};

use super::builder::{DebugInfoBuilder, FieldInfo};
use super::config::DebugInfoError;

impl<'ctx> DebugInfoBuilder<'ctx> {
    // -- Function Debug Info --

    /// Create debug info for a function.
    ///
    /// # Arguments
    ///
    /// * `name` - Function name as it appears in source
    /// * `linkage_name` - Mangled name (or None to use `name`)
    /// * `line` - Line number where function is defined
    /// * `subroutine_type` - Function's type signature
    /// * `is_local` - Whether function has internal linkage
    /// * `is_definition` - Whether this is the function definition (not declaration)
    ///
    /// # Returns
    ///
    /// The `DISubprogram` to attach to the LLVM function.
    pub fn create_function(
        &self,
        name: &str,
        linkage_name: Option<&str>,
        line: u32,
        subroutine_type: DISubroutineType<'ctx>,
        is_local: bool,
        is_definition: bool,
    ) -> DISubprogram<'ctx> {
        self.inner.create_function(
            self.compile_unit.as_debug_info_scope(),
            name,
            linkage_name,
            self.file(),
            line,
            subroutine_type,
            is_local,
            is_definition,
            line, // scope_line = definition line
            DIFlags::ZERO,
            self.config.optimized,
        )
    }

    /// Create a simple function debug info entry.
    ///
    /// Convenience method that creates a void-returning function with no parameters.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the void type for the return type.
    pub fn create_simple_function(
        &self,
        name: &str,
        line: u32,
    ) -> Result<DISubprogram<'ctx>, DebugInfoError> {
        let void_type = self.void_type()?;
        let subroutine = self.create_subroutine_type(Some(void_type.as_type()), &[]);
        Ok(self.create_function(name, None, line, subroutine, false, true))
    }

    /// Attach debug info to a function value.
    pub fn attach_function(&self, func: FunctionValue<'ctx>, subprogram: DISubprogram<'ctx>) {
        func.set_subprogram(subprogram);
    }

    // -- Scope Management --

    /// Create a lexical block (scope) within a function or other scope.
    pub fn create_lexical_block(
        &self,
        scope: DIScope<'ctx>,
        line: u32,
        column: u32,
    ) -> DILexicalBlock<'ctx> {
        self.inner
            .create_lexical_block(scope, self.file(), line, column)
    }

    /// Push a scope onto the scope stack.
    pub fn push_scope(&self, scope: DIScope<'ctx>) {
        self.scope_stack.borrow_mut().push(scope);
    }

    /// Pop a scope from the scope stack.
    pub fn pop_scope(&self) -> Option<DIScope<'ctx>> {
        self.scope_stack.borrow_mut().pop()
    }

    /// Get the current scope (top of stack or compile unit).
    pub fn current_scope(&self) -> DIScope<'ctx> {
        self.scope_stack
            .borrow()
            .last()
            .copied()
            .unwrap_or_else(|| self.compile_unit.as_debug_info_scope())
    }

    // -- Location Setting --

    /// Set the current debug location for subsequent instructions.
    ///
    /// # Arguments
    ///
    /// * `builder` - The LLVM IR builder
    /// * `line` - Source line number (1-indexed)
    /// * `column` - Source column number (1-indexed)
    /// * `scope` - The debug scope for this location
    pub fn set_location(
        &self,
        builder: &Builder<'ctx>,
        line: u32,
        column: u32,
        scope: DIScope<'ctx>,
    ) {
        let loc = self.inner.create_debug_location(
            self.context,
            line,
            column,
            scope,
            None, // No inlined-at
        );
        builder.set_current_debug_location(loc);
    }

    /// Set debug location using the current scope from the stack.
    pub fn set_location_in_current_scope(&self, builder: &Builder<'ctx>, line: u32, column: u32) {
        self.set_location(builder, line, column, self.current_scope());
    }

    /// Clear the current debug location.
    pub fn clear_location(&self, builder: &Builder<'ctx>) {
        builder.unset_current_debug_location();
    }

    // -- Variable Debug Info --

    /// Create a debug info entry for a local (auto) variable.
    ///
    /// Used for `let` bindings and other locally-scoped variables.
    ///
    /// # Arguments
    ///
    /// * `scope` - The scope containing this variable
    /// * `name` - Variable name as it appears in source
    /// * `line` - Line number where variable is defined
    /// * `ty` - Debug type of the variable
    pub fn create_auto_variable(
        &self,
        scope: DIScope<'ctx>,
        name: &str,
        line: u32,
        ty: DIType<'ctx>,
    ) -> DILocalVariable<'ctx> {
        self.inner.create_auto_variable(
            scope,
            name,
            self.file(),
            line,
            ty,
            true, // always_preserve: keep even if optimized away
            DIFlags::ZERO,
            0, // align_in_bits: 0 lets LLVM use type's natural alignment
        )
    }

    /// Create a debug info entry for a function parameter variable.
    ///
    /// Parameter numbers are 1-indexed (first param = 1).
    ///
    /// # Arguments
    ///
    /// * `scope` - The function scope (`DISubprogram`)
    /// * `name` - Parameter name
    /// * `arg_no` - Parameter position (1-indexed)
    /// * `line` - Line number of the function definition
    /// * `ty` - Debug type of the parameter
    pub fn create_parameter_variable(
        &self,
        scope: DIScope<'ctx>,
        name: &str,
        arg_no: u32,
        line: u32,
        ty: DIType<'ctx>,
    ) -> DILocalVariable<'ctx> {
        self.inner.create_parameter_variable(
            scope,
            name,
            arg_no,
            self.file(),
            line,
            ty,
            true, // always_preserve
            DIFlags::ZERO,
        )
    }

    /// Create a debug location (line/column/scope).
    pub fn create_debug_location(
        &self,
        line: u32,
        column: u32,
        scope: DIScope<'ctx>,
    ) -> DILocation<'ctx> {
        self.inner
            .create_debug_location(self.context, line, column, scope, None)
    }

    /// Create an empty debug expression (no address transformations).
    pub fn create_expression(&self) -> DIExpression<'ctx> {
        self.inner.create_expression(Vec::new())
    }

    /// Emit a `llvm.dbg.declare` intrinsic for a mutable binding (alloca).
    ///
    /// Associates an alloca with a debug variable so debuggers can
    /// inspect the variable at its stack address.
    pub fn emit_dbg_declare(
        &self,
        alloca: PointerValue<'ctx>,
        var: DILocalVariable<'ctx>,
        loc: DILocation<'ctx>,
        block: BasicBlock<'ctx>,
    ) -> InstructionValue<'ctx> {
        let expr = self.create_expression();
        self.inner
            .insert_declare_at_end(alloca, Some(var), Some(expr), loc, block)
    }

    /// Emit a `llvm.dbg.value` intrinsic for an immutable binding (SSA value).
    ///
    /// Associates an SSA value with a debug variable so debuggers can
    /// inspect the variable's value.
    pub fn emit_dbg_value(
        &self,
        value: BasicValueEnum<'ctx>,
        var: DILocalVariable<'ctx>,
        loc: DILocation<'ctx>,
        insert_before: InstructionValue<'ctx>,
    ) -> InstructionValue<'ctx> {
        let expr = self.create_expression();
        self.inner
            .insert_dbg_value_before(value, var, Some(expr), loc, insert_before)
    }

    /// Emit a `llvm.dbg.value` intrinsic at the end of a basic block.
    ///
    /// Like [`emit_dbg_value`], but appends to the block instead of
    /// inserting before a specific instruction. Uses the LLVM C API
    /// `LLVMDIBuilderInsertDbgValueAtEnd` which inkwell doesn't wrap.
    pub fn emit_dbg_value_at_end(
        &self,
        value: BasicValueEnum<'ctx>,
        var: DILocalVariable<'ctx>,
        loc: DILocation<'ctx>,
        block: BasicBlock<'ctx>,
    ) {
        use llvm_sys::debuginfo::LLVMDIBuilderInsertDbgValueAtEnd;

        let expr = self.create_expression();
        unsafe {
            LLVMDIBuilderInsertDbgValueAtEnd(
                self.inner.as_mut_ptr(),
                value.as_value_ref(),
                var.as_mut_ptr(),
                expr.as_mut_ptr(),
                loc.as_mut_ptr(),
                block.as_mut_ptr(),
            );
        }
    }

    // -- Composite Type Cache --

    /// Cache a composite debug type by its type pool index.
    pub fn cache_composite_type(&self, idx: u32, ty: DIType<'ctx>) {
        self.type_cache.borrow_mut().composites.insert(idx, ty);
    }

    /// Look up a cached composite debug type.
    pub fn get_cached_composite(&self, idx: u32) -> Option<DIType<'ctx>> {
        self.type_cache.borrow().composites.get(&idx).copied()
    }

    // -- ARC-specific Types --

    /// Create debug info for an ARC heap allocation: `RC<T> = { strong_count: i64, data: T }`.
    ///
    /// This represents the heap layout of a reference-counted value.
    /// The 8-byte `strong_count` header precedes the actual data.
    ///
    /// # Errors
    ///
    /// Returns `DebugInfoError::BasicTypeCreation` if LLVM fails to create
    /// the underlying int type for `strong_count`.
    pub fn create_rc_heap_type(
        &self,
        inner_type: DIType<'ctx>,
        inner_name: &str,
        inner_size_bits: u64,
    ) -> Result<DICompositeType<'ctx>, DebugInfoError> {
        let int_ty = self.int_type()?.as_type();

        let fields = [
            FieldInfo {
                name: "strong_count",
                ty: int_ty,
                size_bits: 64,
                offset_bits: 0,
                line: 0,
            },
            FieldInfo {
                name: "data",
                ty: inner_type,
                size_bits: inner_size_bits,
                offset_bits: 64, // 8-byte header
                line: 0,
            },
        ];

        let total_size = 64 + inner_size_bits;
        let type_name = format!("RC<{inner_name}>");
        Ok(self.create_struct_type(&type_name, 0, total_size, 64, &fields))
    }

    // -- Finalization --

    /// Finalize the debug info.
    ///
    /// Must be called before emitting the module as object code.
    /// This resolves forward references and validates the debug info.
    pub fn finalize(&self) {
        self.inner.finalize();
    }
}
