//! Memory operations (alloca, load, store, GEP) for `IrBuilder`.

use inkwell::types::{BasicTypeEnum, StructType};
use inkwell::values::IntValue;

use super::IrBuilder;
use crate::codegen::value_id::{FunctionId, LLVMTypeId, ValueId};

impl<'ctx> IrBuilder<'_, 'ctx> {
    /// Build a stack allocation (alloca) at the current position.
    pub fn alloca(&mut self, ty: LLVMTypeId, name: &str) -> ValueId {
        let llvm_ty = self.arena.get_type(ty);
        let ptr = self.builder.build_alloca(llvm_ty, name).expect("alloca");
        self.arena.push_value(ptr.into())
    }

    /// Build an alloca at the function entry block.
    ///
    /// Placing allocas in the entry block is required for LLVM's `mem2reg`
    /// pass to promote them to SSA registers. This saves the current position,
    /// inserts at the entry block start, then restores.
    pub fn create_entry_alloca(
        &mut self,
        function: FunctionId,
        name: &str,
        ty: LLVMTypeId,
    ) -> ValueId {
        let func_val = self.arena.get_function(function);
        let llvm_ty = self.arena.get_type(ty);

        let entry = func_val
            .get_first_basic_block()
            .expect("function has entry block");

        // Save current position.
        let saved_block = self.current_block;

        // Position at entry block start.
        if let Some(first_instr) = entry.get_first_instruction() {
            self.builder.position_before(&first_instr);
        } else {
            self.builder.position_at_end(entry);
        }

        let ptr = self.builder.build_alloca(llvm_ty, name).expect("alloca");

        // Restore position.
        if let Some(block_id) = saved_block {
            let bb = self.arena.get_block(block_id);
            self.builder.position_at_end(bb);
        }

        self.arena.push_value(ptr.into())
    }

    /// Build a load from a pointer.
    ///
    /// Struct types are automatically decomposed into per-field GEP + load +
    /// `insert_value` sequences. This prevents creating large aggregate SSA values
    /// that LLVM's `FastISel` mishandles in JIT at O0 (stack corruption for structs
    /// >16 bytes). See llvm.md §8. Recursive: nested structs decompose further.
    ///
    /// Defensive: if `ptr` is not a pointer value, records a codegen error
    /// and returns a zero constant instead of panicking.
    pub fn load(&mut self, ty: LLVMTypeId, ptr: ValueId, name: &str) -> ValueId {
        let llvm_ty = self.arena.get_type(ty);

        // Auto-decompose struct loads to avoid FastISel aggregate spill issues.
        // Per-field GEP+load+insert_value matches Clang -O0 output.
        if let BasicTypeEnum::StructType(st) = llvm_ty {
            if st.count_fields() > 0 {
                return self.load_struct_per_field(ty, st, ptr, name);
            }
        }

        // Non-struct (or empty struct): direct load.
        let raw = self.arena.get_value(ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "load from non-pointer — returning zero");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_load(llvm_ty, raw.into_pointer_value(), name)
            .expect("load");
        self.arena.push_value(v)
    }

    /// Load a struct from a pointer using per-field GEP + load + `insert_value`.
    ///
    /// This avoids creating a single large aggregate SSA value, which LLVM's
    /// `FastISel` mishandles for structs exceeding register capacity (>16 bytes).
    /// Each field load is small enough for `FastISel` to handle correctly.
    ///
    /// Recursive: if a field is itself a struct, the recursive `load()` call
    /// will decompose it further — no aggregate SSA value is ever created.
    fn load_struct_per_field(
        &mut self,
        struct_ty_id: LLVMTypeId,
        st: StructType<'ctx>,
        ptr: ValueId,
        name: &str,
    ) -> ValueId {
        let raw = self.arena.get_value(ptr);
        if !raw.is_pointer_value() {
            tracing::error!(
                val_type = ?raw.get_type(),
                "load_struct from non-pointer — returning zero"
            );
            self.record_codegen_error();
            return self.const_zero(BasicTypeEnum::StructType(st));
        }

        let num_fields = st.count_fields();
        let mut agg = self.const_zero(BasicTypeEnum::StructType(st));

        for f in 0..num_fields {
            let field_ty = st.get_field_type_at_index(f).expect("field index in range");
            let field_ty_id = self.register_type(field_ty);
            let field_ptr = self.struct_gep(struct_ty_id, ptr, f, &format!("{name}.f{f}.ptr"));
            // Recursive: if field is a struct, it will decompose further.
            let field_val = self.load(field_ty_id, field_ptr, &format!("{name}.f{f}"));
            agg = self.insert_value(agg, field_val, f, &format!("{name}.s{f}"));
        }

        agg
    }

    /// Build a store to a pointer.
    ///
    /// Defensive: if `ptr` is not a pointer value, records a codegen error
    /// and skips the store instead of panicking.
    pub fn store(&mut self, val: ValueId, ptr: ValueId) {
        let v = self.arena.get_value(val);
        let p = self.arena.get_value(ptr);
        if !p.is_pointer_value() {
            tracing::error!(val_type = ?p.get_type(), "store to non-pointer — skipping");
            self.record_codegen_error();
            return;
        }
        self.builder
            .build_store(p.into_pointer_value(), v)
            .expect("store");
    }

    /// Build a GEP (get element pointer) with arbitrary indices.
    ///
    /// # Safety
    /// Caller must ensure indices are valid for the pointee type.
    #[allow(
        unsafe_code,
        reason = "LLVM C API requires unsafe for build_in_bounds_gep"
    )]
    pub fn gep(
        &mut self,
        pointee_ty: LLVMTypeId,
        ptr: ValueId,
        indices: &[ValueId],
        name: &str,
    ) -> ValueId {
        let llvm_ty = self.arena.get_type(pointee_ty);
        let raw_ptr = self.arena.get_value(ptr);
        if !raw_ptr.is_pointer_value() {
            tracing::error!(val_type = ?raw_ptr.get_type(), "gep on non-pointer — returning null");
            self.record_codegen_error();
            return self.const_null_ptr();
        }
        let mut idx_vals: Vec<IntValue<'ctx>> = Vec::with_capacity(indices.len());
        for &id in indices {
            let raw = self.arena.get_value(id);
            if !raw.is_int_value() {
                tracing::error!(val_type = ?raw.get_type(), "gep index is not int — returning null");
                self.record_codegen_error();
                return self.const_null_ptr();
            }
            idx_vals.push(raw.into_int_value());
        }
        // SAFETY: Caller ensures indices are valid for the pointee type.
        let v = unsafe {
            self.builder
                .build_in_bounds_gep(llvm_ty, raw_ptr.into_pointer_value(), &idx_vals, name)
                .expect("gep")
        };
        self.arena.push_value(v.into())
    }

    /// Build a struct GEP (field access by index).
    ///
    /// Defensive: if the type is not a struct or the value is not a pointer,
    /// returns a null pointer instead of panicking.
    pub fn struct_gep(
        &mut self,
        struct_ty: LLVMTypeId,
        ptr: ValueId,
        index: u32,
        name: &str,
    ) -> ValueId {
        let raw_ty = self.arena.get_type(struct_ty);
        let BasicTypeEnum::StructType(struct_t) = raw_ty else {
            tracing::error!(?raw_ty, "struct_gep on non-struct type");
            self.record_codegen_error();
            return self.const_null_ptr();
        };
        let raw_val = self.arena.get_value(ptr);
        if !raw_val.is_pointer_value() {
            tracing::error!(?raw_val, "struct_gep on non-pointer value");
            self.record_codegen_error();
            return self.const_null_ptr();
        }
        let v = self
            .builder
            .build_struct_gep(struct_t, raw_val.into_pointer_value(), index, name)
            .expect("struct_gep");
        self.arena.push_value(v.into())
    }
}
