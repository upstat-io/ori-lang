//! Equality and hash thunk generation for LLVM codegen.
//!
//! Generates type-specialized comparison and hashing functions used by
//! map/set operations. Each element type gets a dedicated LLVM function
//! with signature `fn(*const u8, *const u8) -> i1` (equality) or
//! `fn(*const u8) -> i64` (hash).
//!
//! For strings, delegates to `ori_str_eq` / `ori_str_hash` directly.
//! For primitives, generates inline comparison/hash functions.

use ori_types::Idx;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

use super::super::super::ArcIrEmitter;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Get or generate an LLVM equality thunk for comparing elements of `elem_ty`.
    ///
    /// The thunk has signature `fn(*const u8, *const u8) -> bool` (i1). Each
    /// element type gets a specialized function that loads elements by their
    /// native LLVM type before comparing.
    ///
    /// For strings, returns a pointer to `ori_str_eq` directly (same signature).
    /// For primitives, generates an inline comparison function.
    ///
    /// Returns a function pointer `ValueId`, or `None` if the element type
    /// is not yet supported for equality comparison.
    pub(in crate::codegen::arc_emitter) fn get_or_create_eq_thunk(
        &mut self,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        // Check cache first
        if let Some(&func_id) = self.eq_thunk_cache.get(&elem_ty) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let elem_info = self.type_info.get(elem_ty);

        // String equality: `ori_str_eq` has signature fn(ptr, ptr) -> i1 — use directly
        if matches!(&elem_info, TypeInfo::Str) {
            let func_id = self.builder.runtime_fn("ori_str_eq");
            self.eq_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        let type_suffix = match &elem_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
            TypeInfo::Float => "float",
            TypeInfo::Bool => "bool",
            TypeInfo::Char => "char",
            TypeInfo::Byte => "byte",
            _ => return None,
        };

        let func_name = format!("_ori_eq_{type_suffix}");

        // Check if already declared in the module (shared across emitters)
        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);

        // If the function already has a body, just cache and return
        if self.builder.function_has_body(func_id) {
            self.eq_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Set up the function
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let a_ptr = self.builder.get_param(func_id, 0);
        let b_ptr = self.builder.get_param(func_id, 1);

        // Generate the equality comparison body
        let result = self.gen_primitive_eq(a_ptr, b_ptr, elem_ty, &elem_info);

        self.builder.ret(result);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.eq_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
    }

    /// Generate equality comparison body for primitive types.
    ///
    /// Loads values from pointers, compares with `icmp eq` (integers) or
    /// `fcmp oeq` (floats), and returns `i1`.
    ///
    fn gen_primitive_eq(
        &mut self,
        a_ptr: ValueId,
        b_ptr: ValueId,
        elem_ty: Idx,
        elem_info: &TypeInfo,
    ) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let a_val = self.builder.load(llvm_ty, a_ptr, "a");
        let b_val = self.builder.load(llvm_ty, b_ptr, "b");

        match elem_info {
            // Integer equality (signed: int, Duration, Size; unsigned: bool, char, byte)
            TypeInfo::Int
            | TypeInfo::Duration
            | TypeInfo::Size
            | TypeInfo::Bool
            | TypeInfo::Char
            | TypeInfo::Byte => self.builder.icmp_eq(a_val, b_val, "eq"),
            // Float equality (ordered — NaN != NaN)
            TypeInfo::Float => self.builder.fcmp_oeq(a_val, b_val, "eq"),
            _ => unreachable!("non-primitive passed to gen_primitive_eq"),
        }
    }

    /// Get or generate an LLVM hash thunk for hashing elements of `elem_ty`.
    ///
    /// The thunk has signature `fn(*const u8) -> i64`. Each element type gets
    /// a specialized function that loads the element by its native LLVM type
    /// before computing the hash.
    ///
    /// For strings, returns a pointer to `ori_str_hash` directly (same signature).
    /// For primitives, generates an inline hash function.
    ///
    /// Returns a function pointer `ValueId`, or `None` if the element type
    /// is not yet supported for hashing.
    pub(in crate::codegen::arc_emitter) fn get_or_create_hash_thunk(
        &mut self,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        // Check cache first
        if let Some(&func_id) = self.hash_thunk_cache.get(&elem_ty) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let elem_info = self.type_info.get(elem_ty);

        // String hash: `ori_str_hash` has signature fn(ptr) -> i64 — use directly
        if matches!(&elem_info, TypeInfo::Str) {
            let func_id = self.builder.runtime_fn("ori_str_hash");
            self.hash_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        let type_suffix = match &elem_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
            TypeInfo::Float => "float",
            TypeInfo::Bool => "bool",
            TypeInfo::Char => "char",
            TypeInfo::Byte => "byte",
            _ => return None,
        };

        let func_name = format!("_ori_hash_{type_suffix}");

        // Check if already declared in the module (shared across emitters)
        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty], i64_ty);

        // If the function already has a body, just cache and return
        if self.builder.function_has_body(func_id) {
            self.hash_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        // Save builder position
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        // Set up the function
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);

        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let a_ptr = self.builder.get_param(func_id, 0);

        // Generate the hash body
        let result = self.gen_primitive_hash(a_ptr, elem_ty, &elem_info);

        self.builder.ret(result);

        // Restore builder position
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }

        self.hash_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
    }

    /// Generate hash body for primitive types.
    ///
    /// Loads the value from the pointer and converts to i64:
    /// - `int`/`Duration`/`Size`: identity (already i64)
    /// - `bool`/`byte`: load i8, zero-extend to i64
    /// - `char`: load i32, zero-extend to i64
    /// - `float`: load f64, bitcast to i64
    fn gen_primitive_hash(
        &mut self,
        a_ptr: ValueId,
        elem_ty: Idx,
        elem_info: &TypeInfo,
    ) -> ValueId {
        let llvm_ty = self.resolve_type(elem_ty);
        let a_val = self.builder.load(llvm_ty, a_ptr, "a");
        let i64_ty = self.builder.i64_type();

        match elem_info {
            // i64 types — identity hash
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => a_val,
            // Sub-i64 types (i8 bool/byte, i32 char) — zero-extend to i64
            TypeInfo::Bool | TypeInfo::Byte | TypeInfo::Char => {
                self.builder.zext(a_val, i64_ty, "hash.zext")
            }
            // f64 — bitcast to i64
            TypeInfo::Float => self.builder.bitcast(a_val, i64_ty, "hash.bits"),
            _ => unreachable!("non-primitive passed to gen_primitive_hash"),
        }
    }
}
