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
use crate::codegen::value_id::{FunctionId, ValueId};

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

        // User-defined Eq (manual `impl T: Eq` or `#derive(Eq)`): synthesize a
        // thunk that calls the user `@eq` impl (method name is "eq", NOT
        // "equals" — per ori_ir/derives/mod.rs + operators/mod.rs). Without this
        // a user-Eq map/set key fell through to `_ => return None` → null key_eq
        // → SIGSEGV.
        let eq_name = self.interner.intern("eq");
        if let Some((eq_fid, eq_abi)) = self.user_method(elem_ty, eq_name) {
            return self.gen_user_eq_thunk(elem_ty, eq_fid, eq_abi);
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

    /// Synthesize an `fn(ptr,ptr)->i1` equality thunk that calls the user `@eq`
    /// impl for a user-Eq map/set key type. Threads each operand by-value
    /// (`Direct`) or by-pointer (`Indirect`/`Reference`) per its ABI, mirroring
    /// `emit_user_drop_via_pointer`. Borrowed operands — no RC ops. Returns the
    /// thunk's function pointer, cached by `Idx`.
    fn gen_user_eq_thunk(
        &mut self,
        elem_ty: Idx,
        eq_fid: FunctionId,
        eq_abi: crate::codegen::abi::FunctionAbi,
    ) -> Option<ValueId> {
        use crate::codegen::abi::ParamPassing;
        let ptr_ty = self.builder.ptr_type();
        let bool_ty = self.builder.bool_type();
        let func_name = format!("_ori_eq_user${}", elem_ty.raw());
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], bool_ty);
        if self.builder.function_has_body(func_id) {
            self.eq_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        let a_ptr = self.builder.get_param(func_id, 0);
        let b_ptr = self.builder.get_param(func_id, 1);
        let resolved = self.pool.resolve_fully(elem_ty);
        let self_ty = self.resolve_type(resolved);
        // @eq (self, other: Self) -> bool: both operands share the self ABI.
        let pa = eq_abi
            .params
            .first()
            .map(|p| p.passing)
            .unwrap_or(ParamPassing::Reference);
        let pb = eq_abi.params.get(1).map(|p| p.passing).unwrap_or(pa);
        let a_arg = match pa {
            ParamPassing::Direct => self.builder.load(self_ty, a_ptr, "eq.a"),
            _ => a_ptr,
        };
        let b_arg = match pb {
            ParamPassing::Direct => self.builder.load(self_ty, b_ptr, "eq.b"),
            _ => b_ptr,
        };
        let result = self
            .builder
            .call(eq_fid, &[a_arg, b_arg], "eq.r")
            .unwrap_or_else(|| self.builder.const_bool(false));
        self.builder.ret(result);
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }
        self.eq_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
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

        // User-defined Hashable (manual `impl T: Hashable` or `#derive(Hashable)`):
        // synthesize a thunk that calls the user `@hash` impl. Without this a
        // user-Hashable map/set key fell through to `_ => return None` → null
        // key_hash → SIGSEGV in ori_map_literal_put / set ops / collect_set.
        let hash_name = self.interner.intern("hash");
        if let Some((hash_fid, hash_abi)) = self.user_method(elem_ty, hash_name) {
            return self.gen_user_hash_thunk(elem_ty, hash_fid, hash_abi);
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

    /// Synthesize an `fn(ptr)->i64` hash thunk that calls the user `@hash` impl
    /// for a user-Hashable map/set key type. Threads `self` by-value (`Direct`)
    /// or by-pointer (`Indirect`/`Reference`) per its ABI, mirroring
    /// `emit_user_drop_via_pointer`. Borrowed `self` — no RC ops. Returns the
    /// thunk's function pointer, cached by `Idx`.
    fn gen_user_hash_thunk(
        &mut self,
        elem_ty: Idx,
        hash_fid: FunctionId,
        hash_abi: crate::codegen::abi::FunctionAbi,
    ) -> Option<ValueId> {
        use crate::codegen::abi::ParamPassing;
        let ptr_ty = self.builder.ptr_type();
        let i64_ty = self.builder.i64_type();
        let func_name = format!("_ori_hash_user${}", elem_ty.raw());
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty], i64_ty);
        if self.builder.function_has_body(func_id) {
            self.hash_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();
        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);
        let key_ptr = self.builder.get_param(func_id, 0);
        let resolved = self.pool.resolve_fully(elem_ty);
        let self_ty = self.resolve_type(resolved);
        let pa = hash_abi
            .params
            .first()
            .map(|p| p.passing)
            .unwrap_or(ParamPassing::Reference);
        let arg = match pa {
            ParamPassing::Direct => self.builder.load(self_ty, key_ptr, "hash.self"),
            _ => key_ptr,
        };
        let result = self
            .builder
            .call(hash_fid, &[arg], "hash.r")
            .unwrap_or_else(|| self.builder.const_i64(0));
        self.builder.ret(result);
        self.builder.restore_position(saved_pos);
        if let Some(f) = saved_func {
            self.builder.set_current_function(f);
        }
        self.hash_thunk_cache.insert(elem_ty, func_id);
        Some(self.builder.get_function_ptr(func_id))
    }
}
