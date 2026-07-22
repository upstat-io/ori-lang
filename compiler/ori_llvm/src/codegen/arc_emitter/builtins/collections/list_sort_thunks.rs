//! Sort/compare thunk generators for list sorting.
//!
//! Generates per-element-type LLVM comparison functions used by the COW sort
//! runtime (`ori_list_sort_cow`, `ori_list_sort_stable_cow`).
//!
//! Each thunk has signature `fn(*const u8, *const u8) -> i32` and returns
//! -1 (less), 0 (equal), or 1 (greater).

use super::super::super::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::{BlockId, FunctionId, ValueId};
use ori_types::Idx;

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    fn begin_compare_thunk(
        &mut self,
        func_id: FunctionId,
    ) -> (Option<BlockId>, Option<FunctionId>, ValueId, ValueId) {
        let saved_pos = self.builder.save_position();
        let saved_func = self.builder.current_function();

        self.builder.set_ccc(func_id);
        self.builder.add_nounwind_attribute(func_id);
        let entry = self.builder.append_block(func_id, "entry");
        self.builder.position_at_end(entry);
        self.builder.set_current_function(func_id);

        let a_ptr = self.builder.get_param(func_id, 0);
        let b_ptr = self.builder.get_param(func_id, 1);
        (saved_pos, saved_func, a_ptr, b_ptr)
    }

    fn finish_compare_thunk(
        &mut self,
        func_id: FunctionId,
        result: ValueId,
        saved_pos: Option<BlockId>,
        saved_func: Option<FunctionId>,
    ) -> ValueId {
        self.builder.ret(result);
        self.builder.restore_position(saved_pos);
        if let Some(function) = saved_func {
            self.builder.set_current_function(function);
        }
        self.builder.get_function_ptr(func_id)
    }

    fn emit_compare_result(&mut self, less: ValueId, greater: ValueId) -> ValueId {
        let negative = self.builder.const_i32(-1);
        let equal = self.builder.const_i32(0);
        let positive = self.builder.const_i32(1);
        let greater_or_equal = self.builder.select(greater, positive, equal, "gt_or_eq");
        self.builder.select(less, negative, greater_or_equal, "cmp")
    }

    /// Get or generate an LLVM comparison thunk for sorting elements of `elem_ty`.
    ///
    /// The thunk has signature `fn(*const u8, *const u8) -> i32` and returns
    /// -1 (less), 0 (equal), or 1 (greater). Each element type gets a
    /// specialized function that loads elements by their native LLVM type
    /// before comparing.
    ///
    /// Returns a function pointer `ValueId`, or `None` if the element type
    /// is not yet supported for sorting.
    pub(in crate::codegen::arc_emitter::builtins::collections) fn get_or_create_compare_thunk(
        &mut self,
        elem_ty: Idx,
    ) -> Option<ValueId> {
        // Check cache first
        if let Some(&func_id) = self.compare_thunk_cache.get(&elem_ty) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let elem_info = self.type_info.get(elem_ty);
        let type_suffix = match &elem_info {
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => "int",
            TypeInfo::Float => "float",
            TypeInfo::Bool => "bool",
            TypeInfo::Char => "char",
            TypeInfo::Byte => "byte",
            TypeInfo::Str => "str",
            _ => return None,
        };

        let func_name = format!("_ori_cmp_{type_suffix}");

        // Check if already declared in the module (shared across emitters)
        let ptr_ty = self.builder.ptr_type();
        let i32_ty = self.builder.i32_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], i32_ty);

        // If the function already has a body, just cache and return
        if self.builder.function_has_body(func_id) {
            self.compare_thunk_cache.insert(elem_ty, func_id);
            return Some(self.builder.get_function_ptr(func_id));
        }

        let (saved_pos, saved_func, a_ptr, b_ptr) = self.begin_compare_thunk(func_id);

        // Generate the comparison body based on element type
        let result = match &elem_info {
            TypeInfo::Str => self.gen_str_compare(a_ptr, b_ptr),
            _ => self.gen_primitive_compare(a_ptr, b_ptr, elem_ty, &elem_info),
        };

        self.compare_thunk_cache.insert(elem_ty, func_id);
        Some(self.finish_compare_thunk(func_id, result, saved_pos, saved_func))
    }

    pub(super) fn gen_primitive_compare(
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
            // Signed integer comparison (int, Duration, Size)
            TypeInfo::Int | TypeInfo::Duration | TypeInfo::Size => {
                let lt = self.builder.icmp_slt(a_val, b_val, "lt");
                let gt = self.builder.icmp_sgt(a_val, b_val, "gt");
                self.emit_compare_result(lt, gt)
            }
            // Float comparison (ordered)
            TypeInfo::Float => {
                let lt = self.builder.fcmp_olt(a_val, b_val, "lt");
                let gt = self.builder.fcmp_ogt(a_val, b_val, "gt");
                self.emit_compare_result(lt, gt)
            }
            // Unsigned comparison (bool, char, byte) — zext to i32 first
            TypeInfo::Bool | TypeInfo::Char | TypeInfo::Byte => {
                let i32_ty = self.builder.i32_type();
                let a_ext = self.builder.zext(a_val, i32_ty, "a.ext");
                let b_ext = self.builder.zext(b_val, i32_ty, "b.ext");
                let lt = self.builder.icmp_ult(a_ext, b_ext, "lt");
                let gt = self.builder.icmp_ugt(a_ext, b_ext, "gt");
                self.emit_compare_result(lt, gt)
            }
            TypeInfo::Unit
            | TypeInfo::Never
            | TypeInfo::Str
            | TypeInfo::Ordering
            | TypeInfo::List { .. }
            | TypeInfo::Map { .. }
            | TypeInfo::Set { .. }
            | TypeInfo::Tuple { .. }
            | TypeInfo::Option { .. }
            | TypeInfo::Result { .. }
            | TypeInfo::Range
            | TypeInfo::Struct { .. }
            | TypeInfo::Enum { .. }
            | TypeInfo::Iterator { .. }
            | TypeInfo::Channel { .. }
            | TypeInfo::Function { .. }
            | TypeInfo::Error => {
                unreachable!("non-primitive passed to gen_primitive_compare")
            }
        }
    }

    /// Get or generate a narrowed compare thunk for sorting.
    ///
    /// Like `get_or_create_compare_thunk`, but loads elements at the narrowed
    /// width (i8/i16/i32) and sign-extends to i64 before comparing. Used for
    /// sort operations on narrowed `[int]` lists where the buffer elements are
    /// stored at sub-i64 widths.
    ///
    /// Returns `None` if int elements are not narrowed (caller should use the
    /// canonical compare thunk instead).
    pub(in crate::codegen::arc_emitter::builtins::collections) fn get_or_create_narrowed_compare_thunk(
        &mut self,
        list_ty: Idx,
    ) -> Option<ValueId> {
        let width = self.narrowed_collection_element_width(list_ty)?;

        let width_suffix = match width {
            ori_repr::IntWidth::I8 => "i8",
            ori_repr::IntWidth::I16 => "i16",
            ori_repr::IntWidth::I32 => "i32",
            ori_repr::IntWidth::I64 => return None,
        };
        let func_name = format!("_ori_cmp_int_narrow_{width_suffix}");

        let ptr_ty = self.builder.ptr_type();
        let i32_ty = self.builder.i32_type();
        let func_id = self
            .builder
            .get_or_declare_function(&func_name, &[ptr_ty, ptr_ty], i32_ty);

        if self.builder.function_has_body(func_id) {
            return Some(self.builder.get_function_ptr(func_id));
        }

        let (saved_pos, saved_func, a_ptr, b_ptr) = self.begin_compare_thunk(func_id);

        // Load at narrowed width, sign-extend to i64 for comparison.
        let narrow_ty = self.llvm_type_for_int_width(width);
        let i64_ty = self.builder.i64_type();
        let a_raw = self.builder.load(narrow_ty, a_ptr, "a");
        let a_val = self.builder.sext(a_raw, i64_ty, "a.sext");
        let b_raw = self.builder.load(narrow_ty, b_ptr, "b");
        let b_val = self.builder.sext(b_raw, i64_ty, "b.sext");

        let lt = self.builder.icmp_slt(a_val, b_val, "lt");
        let gt = self.builder.icmp_sgt(a_val, b_val, "gt");
        let result = self.emit_compare_result(lt, gt);
        Some(self.finish_compare_thunk(func_id, result, saved_pos, saved_func))
    }

    /// Generate comparison body for strings.
    ///
    /// Calls `ori_str_compare(a, b) -> i8` (returns Ori Ordering: 0=Less,
    /// 1=Equal, 2=Greater) and converts to i32 (-1/0/1) via `result - 1`.
    pub(super) fn gen_str_compare(&mut self, a_ptr: ValueId, b_ptr: ValueId) -> ValueId {
        let cmp_fn = self.builder.runtime_fn("ori_str_compare");
        let Some(ord) = self.builder.call(cmp_fn, &[a_ptr, b_ptr], "ord") else {
            panic!("ori_str_compare must return an Ordering tag");
        };

        // Convert Ordering (0,1,2) to sort convention (-1,0,1): subtract 1
        let one = self.builder.const_i8(1);
        let shifted = self.builder.sub(ord, one, "shifted");

        // Sign-extend i8 → i32
        let i32_ty = self.builder.i32_type();
        self.builder.sext(shifted, i32_ty, "cmp")
    }
}
