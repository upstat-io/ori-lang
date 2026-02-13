//! ID-based LLVM instruction builder for V2 codegen.
//!
//! `IrBuilder` wraps inkwell's `Builder`, stores all LLVM values in a
//! `ValueArena`, and exposes only opaque ID types to callers. This
//! hides the `'ctx` lifetime from the codegen pipeline.
//!
//! # Design
//!
//! - Callers see `ValueId`, `LLVMTypeId`, `BlockId`, `FunctionId` — all `Copy`.
//! - The arena lives inside `IrBuilder`, so the `'ctx` lifetime is contained.
//! - All methods take `&mut self` because arena mutations require `&mut`.
//! - Debug assertions catch type mismatches (e.g., adding float + int) at zero
//!   cost in release builds.
//!
//! # Method Organization
//!
//! | Category | Methods |
//! |----------|---------|
//! | Constants | `const_i8`, `const_i32`, `const_i64`, `const_f64`, `const_bool`, ... |
//! | Memory | `alloca`, `create_entry_alloca`, `load`, `store`, `gep`, `struct_gep` |
//! | Arithmetic | `add`, `sub`, `mul`, `sdiv`, `srem`, `neg`, `fadd`, ... |
//! | Comparisons | `icmp_eq`, `icmp_slt`, `fcmp_oeq`, ... |
//! | Conversions | `bitcast`, `trunc`, `sext`, `zext`, `si_to_fp`, ... |
//! | Control flow | `br`, `cond_br`, `switch`, `select`, `ret`, `ret_void`, `unreachable` |
//! | Aggregates | `extract_value`, `insert_value`, `build_struct` |
//! | Calls | `call`, `call_tail`, `call_indirect`, `invoke`, `invoke_indirect` |
//! | EH | `landingpad`, `resume`, `set_personality` |
//! | Phi nodes | `phi`, `phi_from_incoming` |
//! | Types | `register_type`, `bool_type`, `i8_type`, `i32_type`, ... |
//! | Blocks | `append_block`, `position_at_end`, `current_block`, ... |
//! | Functions | `declare_function`, `get_or_declare_function`, ... |

use std::cell::Cell;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder as InkwellBuilder;
use inkwell::module::Linkage;
use inkwell::types::{AnyType, BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, IntValue};
use inkwell::{FloatPredicate, IntPredicate};

use crate::context::SimpleCx;

use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueArena, ValueId};

// ---------------------------------------------------------------------------
// IrBuilder
// ---------------------------------------------------------------------------

/// ID-based LLVM IR builder.
///
/// All LLVM values are stored in an internal arena; callers only handle
/// opaque `ValueId` / `BlockId` / etc. The `'ctx` lifetime is contained
/// inside this struct — it never leaks to callers.
///
/// Two lifetimes:
/// - `'ctx`: The LLVM context lifetime (from `Context::create()`).
/// - `'scx`: The borrow lifetime of the `SimpleCx` reference.
///
/// These are separate to avoid drop-checker issues where `IrBuilder`
/// and `SimpleCx` are local variables in the same scope.
pub struct IrBuilder<'scx, 'ctx> {
    /// The underlying inkwell builder.
    builder: InkwellBuilder<'ctx>,
    /// Shared LLVM context for type creation.
    scx: &'scx SimpleCx<'ctx>,
    /// Arena storing all LLVM values behind IDs.
    arena: ValueArena<'ctx>,
    /// Currently-active function (set by `set_current_function`).
    current_function: Option<FunctionId>,
    /// Currently-active basic block (tracked for save/restore).
    current_block: Option<BlockId>,
    /// Count of type-mismatch errors during IR construction.
    ///
    /// Incremented by defensive fallback methods (e.g., `build_struct` on non-struct,
    /// `icmp_impl` on non-int). When > 0, the generated IR is malformed and must
    /// NOT be passed to LLVM's JIT — doing so causes heap corruption (SIGABRT).
    /// The evaluator checks this after compilation to bail out early.
    codegen_errors: Cell<u32>,
}

impl<'scx, 'ctx> IrBuilder<'scx, 'ctx> {
    /// Create a new `IrBuilder`.
    pub fn new(scx: &'scx SimpleCx<'ctx>) -> Self {
        let builder = scx.llcx.create_builder();
        Self {
            builder,
            scx,
            arena: ValueArena::new(),
            current_function: None,
            current_block: None,
            codegen_errors: Cell::new(0),
        }
    }

    /// Access the underlying `SimpleCx` for direct LLVM context operations.
    #[inline]
    pub fn scx(&self) -> &'scx SimpleCx<'ctx> {
        self.scx
    }

    /// Record a type-mismatch error during IR construction.
    ///
    /// Called by defensive fallback methods when they detect a type mismatch
    /// that would normally cause a panic. The generated IR is malformed and
    /// must not be JIT-compiled.
    pub(crate) fn record_codegen_error(&self) {
        self.codegen_errors.set(self.codegen_errors.get() + 1);
    }

    /// Number of type-mismatch errors recorded during IR construction.
    ///
    /// If > 0, the module's IR is malformed and must not be passed to
    /// LLVM's JIT engine. The evaluator should return an error instead.
    pub fn codegen_error_count(&self) -> u32 {
        self.codegen_errors.get()
    }

    /// Whether any codegen errors have been recorded.
    ///
    /// Used by `ExprLowerer::lower()` to bail out early and avoid
    /// cascading type mismatches that corrupt LLVM's internal state.
    pub fn has_codegen_errors(&self) -> bool {
        self.codegen_errors.get() > 0
    }

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    /// Create an i8 constant.
    #[inline]
    pub fn const_i8(&mut self, val: i8) -> ValueId {
        let v = self.scx.type_i8().const_int(val as u64, val < 0);
        self.arena.push_value(v.into())
    }

    /// Create an i32 constant.
    #[inline]
    pub fn const_i32(&mut self, val: i32) -> ValueId {
        let v = self.scx.type_i32().const_int(val as u64, val < 0);
        self.arena.push_value(v.into())
    }

    /// Create an i64 constant.
    #[inline]
    pub fn const_i64(&mut self, val: i64) -> ValueId {
        let v = self.scx.type_i64().const_int(val as u64, val < 0);
        self.arena.push_value(v.into())
    }

    /// Create an f64 constant.
    #[inline]
    pub fn const_f64(&mut self, val: f64) -> ValueId {
        let v = self.scx.type_f64().const_float(val);
        self.arena.push_value(v.into())
    }

    /// Create an i1 (boolean) constant.
    #[inline]
    pub fn const_bool(&mut self, val: bool) -> ValueId {
        let v = self.scx.type_i1().const_int(u64::from(val), false);
        self.arena.push_value(v.into())
    }

    /// Create a null pointer constant.
    #[inline]
    pub fn const_null_ptr(&mut self) -> ValueId {
        let v = self.scx.type_ptr().const_null();
        self.arena.push_value(v.into())
    }

    /// Create a zero/null constant of any LLVM basic type.
    ///
    /// Used for zero-initializing Option/Result payloads when the inner
    /// type is not i64 (e.g., `option[bool]` needs an `i1 0` payload,
    /// `option[str]` needs a `{i64 0, ptr null}` payload).
    pub fn const_zero(&mut self, ty: BasicTypeEnum<'ctx>) -> ValueId {
        let v: BasicValueEnum<'ctx> = match ty {
            BasicTypeEnum::IntType(t) => t.const_int(0, false).into(),
            BasicTypeEnum::FloatType(t) => t.const_float(0.0).into(),
            BasicTypeEnum::StructType(t) => t.const_zero().into(),
            BasicTypeEnum::PointerType(t) => t.const_null().into(),
            BasicTypeEnum::ArrayType(t) => t.const_zero().into(),
            BasicTypeEnum::VectorType(t) => t.const_zero().into(),
            BasicTypeEnum::ScalableVectorType(_) => {
                // Scalable vectors don't support const_zero; fall back to i64.
                self.scx.type_i64().const_int(0, false).into()
            }
        };
        self.arena.push_value(v)
    }

    /// Create a constant string value (non-null-terminated byte array).
    pub fn const_string(&mut self, s: &[u8]) -> ValueId {
        let v = self.scx.llcx.const_string(s, false);
        self.arena.push_value(v.into())
    }

    /// Create a global null-terminated string and return a pointer to it.
    pub fn build_global_string_ptr(&mut self, value: &str, name: &str) -> ValueId {
        let v = self
            .builder
            .build_global_string_ptr(value, name)
            .expect("build_global_string_ptr")
            .as_pointer_value();
        self.arena.push_value(v.into())
    }

    // -----------------------------------------------------------------------
    // Memory
    // -----------------------------------------------------------------------

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
    /// Defensive: if `ptr` is not a pointer value, records a codegen error
    /// and returns a zero constant instead of panicking.
    pub fn load(&mut self, ty: LLVMTypeId, ptr: ValueId, name: &str) -> ValueId {
        let llvm_ty = self.arena.get_type(ty);
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

    // -----------------------------------------------------------------------
    // Signed arithmetic
    // -----------------------------------------------------------------------

    /// Build integer addition.
    pub fn add(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "add on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_add(l.into_int_value(), r.into_int_value(), name)
            .expect("add");
        self.arena.push_value(v.into())
    }

    /// Build integer subtraction.
    pub fn sub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "sub on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_sub(l.into_int_value(), r.into_int_value(), name)
            .expect("sub");
        self.arena.push_value(v.into())
    }

    /// Build integer multiplication.
    pub fn mul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "mul on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_mul(l.into_int_value(), r.into_int_value(), name)
            .expect("mul");
        self.arena.push_value(v.into())
    }

    /// Build signed integer division.
    pub fn sdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "sdiv on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_signed_div(l.into_int_value(), r.into_int_value(), name)
            .expect("sdiv");
        self.arena.push_value(v.into())
    }

    /// Build signed integer remainder.
    pub fn srem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "srem on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_signed_rem(l.into_int_value(), r.into_int_value(), name)
            .expect("srem");
        self.arena.push_value(v.into())
    }

    /// Build integer negation.
    pub fn neg(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "neg on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_neg(v.into_int_value(), name)
            .expect("neg");
        self.arena.push_value(result.into())
    }

    // -----------------------------------------------------------------------
    // Unsigned arithmetic
    // -----------------------------------------------------------------------

    /// Build unsigned integer division.
    pub fn udiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "udiv on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_unsigned_div(l.into_int_value(), r.into_int_value(), name)
            .expect("udiv");
        self.arena.push_value(v.into())
    }

    /// Build unsigned integer remainder.
    pub fn urem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "urem on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_int_unsigned_rem(l.into_int_value(), r.into_int_value(), name)
            .expect("urem");
        self.arena.push_value(v.into())
    }

    /// Build logical right shift (zero-extending).
    pub fn lshr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "lshr on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_right_shift(l.into_int_value(), r.into_int_value(), false, name)
            .expect("lshr");
        self.arena.push_value(v.into())
    }

    // -----------------------------------------------------------------------
    // Float arithmetic
    // -----------------------------------------------------------------------

    /// Build floating-point addition.
    pub fn fadd(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fadd on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_add(l.into_float_value(), r.into_float_value(), name)
            .expect("fadd");
        self.arena.push_value(v.into())
    }

    /// Build floating-point subtraction.
    pub fn fsub(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fsub on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_sub(l.into_float_value(), r.into_float_value(), name)
            .expect("fsub");
        self.arena.push_value(v.into())
    }

    /// Build floating-point multiplication.
    pub fn fmul(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fmul on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_mul(l.into_float_value(), r.into_float_value(), name)
            .expect("fmul");
        self.arena.push_value(v.into())
    }

    /// Build floating-point division.
    pub fn fdiv(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "fdiv on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_div(l.into_float_value(), r.into_float_value(), name)
            .expect("fdiv");
        self.arena.push_value(v.into())
    }

    /// Build floating-point remainder.
    pub fn frem(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "frem on non-float operands");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let v = self
            .builder
            .build_float_rem(l.into_float_value(), r.into_float_value(), name)
            .expect("frem");
        self.arena.push_value(v.into())
    }

    /// Build floating-point negation.
    pub fn fneg(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fneg on non-float operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_float_neg(v.into_float_value(), name)
            .expect("fneg");
        self.arena.push_value(result.into())
    }

    // -----------------------------------------------------------------------
    // Bitwise operations
    // -----------------------------------------------------------------------

    /// Build bitwise AND.
    pub fn and(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "and on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_and(l.into_int_value(), r.into_int_value(), name)
            .expect("and");
        self.arena.push_value(v.into())
    }

    /// Build bitwise OR.
    pub fn or(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "or on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_or(l.into_int_value(), r.into_int_value(), name)
            .expect("or");
        self.arena.push_value(v.into())
    }

    /// Build bitwise XOR.
    pub fn xor(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "xor on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_xor(l.into_int_value(), r.into_int_value(), name)
            .expect("xor");
        self.arena.push_value(v.into())
    }

    /// Build bitwise NOT (complement).
    pub fn not(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "not on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_not(v.into_int_value(), name)
            .expect("not");
        self.arena.push_value(result.into())
    }

    /// Build left shift.
    pub fn shl(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "shl on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_left_shift(l.into_int_value(), r.into_int_value(), name)
            .expect("shl");
        self.arena.push_value(v.into())
    }

    /// Build arithmetic right shift (sign-extending).
    pub fn ashr(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(lhs_type = ?l.get_type(), rhs_type = ?r.get_type(), "ashr on non-int operands");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let v = self
            .builder
            .build_right_shift(l.into_int_value(), r.into_int_value(), true, name)
            .expect("ashr");
        self.arena.push_value(v.into())
    }

    // -----------------------------------------------------------------------
    // Integer comparisons
    // -----------------------------------------------------------------------

    /// Generic integer comparison.
    ///
    /// Defensive: if either operand is not an integer, returns `false` (i1 0)
    /// instead of panicking. This prevents process-killing crashes when type
    /// mismatches reach codegen (e.g., comparing str values with `icmp`).
    fn icmp_impl(&mut self, pred: IntPredicate, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_int_value() || !r.is_int_value() {
            tracing::error!(
                lhs_type = ?l.get_type(),
                rhs_type = ?r.get_type(),
                "icmp on non-int operands — returning false"
            );
            self.record_codegen_error();
            return self.const_bool(false);
        }
        let v = self
            .builder
            .build_int_compare(pred, l.into_int_value(), r.into_int_value(), name)
            .expect("icmp");
        self.arena.push_value(v.into())
    }

    /// Integer equal.
    pub fn icmp_eq(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::EQ, lhs, rhs, name)
    }

    /// Integer not equal.
    pub fn icmp_ne(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::NE, lhs, rhs, name)
    }

    /// Signed less than.
    pub fn icmp_slt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SLT, lhs, rhs, name)
    }

    /// Signed greater than.
    pub fn icmp_sgt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SGT, lhs, rhs, name)
    }

    /// Signed less than or equal.
    pub fn icmp_sle(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SLE, lhs, rhs, name)
    }

    /// Signed greater than or equal.
    pub fn icmp_sge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::SGE, lhs, rhs, name)
    }

    /// Unsigned less than.
    pub fn icmp_ult(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::ULT, lhs, rhs, name)
    }

    /// Unsigned greater than.
    pub fn icmp_ugt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::UGT, lhs, rhs, name)
    }

    /// Unsigned less than or equal.
    pub fn icmp_ule(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::ULE, lhs, rhs, name)
    }

    /// Unsigned greater than or equal.
    pub fn icmp_uge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.icmp_impl(IntPredicate::UGE, lhs, rhs, name)
    }

    // -----------------------------------------------------------------------
    // Float comparisons
    // -----------------------------------------------------------------------

    /// Generic float comparison.
    ///
    /// Defensive: if either operand is not a float, returns `false` (i1 0)
    /// instead of panicking. Prevents crashes from type mismatches.
    fn fcmp_impl(
        &mut self,
        pred: FloatPredicate,
        lhs: ValueId,
        rhs: ValueId,
        name: &str,
    ) -> ValueId {
        let l = self.arena.get_value(lhs);
        let r = self.arena.get_value(rhs);
        if !l.is_float_value() || !r.is_float_value() {
            tracing::error!(
                lhs_type = ?l.get_type(),
                rhs_type = ?r.get_type(),
                "fcmp on non-float operands — returning false"
            );
            self.record_codegen_error();
            return self.const_bool(false);
        }
        let v = self
            .builder
            .build_float_compare(pred, l.into_float_value(), r.into_float_value(), name)
            .expect("fcmp");
        self.arena.push_value(v.into())
    }

    /// Ordered equal.
    pub fn fcmp_oeq(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OEQ, lhs, rhs, name)
    }

    /// Ordered less than.
    pub fn fcmp_olt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OLT, lhs, rhs, name)
    }

    /// Ordered greater than.
    pub fn fcmp_ogt(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OGT, lhs, rhs, name)
    }

    /// Ordered less than or equal.
    pub fn fcmp_ole(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OLE, lhs, rhs, name)
    }

    /// Ordered greater than or equal.
    pub fn fcmp_oge(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::OGE, lhs, rhs, name)
    }

    /// Ordered not equal (false if either is NaN).
    pub fn fcmp_one(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::ONE, lhs, rhs, name)
    }

    /// Unordered not equal (true if either is NaN or values differ).
    /// This is the correct IEEE 754 `!=` — NaN != NaN returns true.
    pub fn fcmp_une(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::UNE, lhs, rhs, name)
    }

    /// Ordered (both non-NaN).
    pub fn fcmp_ord(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::ORD, lhs, rhs, name)
    }

    /// Unordered (either NaN).
    pub fn fcmp_uno(&mut self, lhs: ValueId, rhs: ValueId, name: &str) -> ValueId {
        self.fcmp_impl(FloatPredicate::UNO, lhs, rhs, name)
    }

    // -----------------------------------------------------------------------
    // Conversions
    // -----------------------------------------------------------------------

    /// Build a bitcast.
    pub fn bitcast(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty);
        let result = self
            .builder
            .build_bit_cast(v, target, name)
            .expect("bitcast");
        self.arena.push_value(result)
    }

    /// Build integer truncation (to a smaller integer type).
    pub fn trunc(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "trunc on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_truncate(v.into_int_value(), target, name)
            .expect("trunc");
        self.arena.push_value(result.into())
    }

    /// Build sign extension (to a larger integer type).
    pub fn sext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "sext on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_s_extend(v.into_int_value(), target, name)
            .expect("sext");
        self.arena.push_value(result.into())
    }

    /// Build zero extension (to a larger integer type).
    pub fn zext(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "zext on non-int operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_int_z_extend(v.into_int_value(), target, name)
            .expect("zext");
        self.arena.push_value(result.into())
    }

    /// Build signed integer to floating-point conversion.
    pub fn si_to_fp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_float_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "si_to_fp on non-int operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_signed_int_to_float(v.into_int_value(), target, name)
            .expect("si_to_fp");
        self.arena.push_value(result.into())
    }

    /// Build floating-point to signed integer conversion.
    pub fn fp_to_si(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fp_to_si on non-float operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_float_to_signed_int(v.into_float_value(), target, name)
            .expect("fp_to_si");
        self.arena.push_value(result.into())
    }

    /// Build unsigned integer to floating-point conversion.
    pub fn uitofp(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_float_type();
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "uitofp on non-int operand");
            self.record_codegen_error();
            return self.const_f64(0.0);
        }
        let result = self
            .builder
            .build_unsigned_int_to_float(v.into_int_value(), target, name)
            .expect("uitofp");
        self.arena.push_value(result.into())
    }

    /// Build floating-point to unsigned integer conversion.
    pub fn fptoui(&mut self, val: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        let target = self.arena.get_type(ty).into_int_type();
        if !v.is_float_value() {
            tracing::error!(val_type = ?v.get_type(), "fptoui on non-float operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_float_to_unsigned_int(v.into_float_value(), target, name)
            .expect("fptoui");
        self.arena.push_value(result.into())
    }

    /// Build pointer-to-integer conversion.
    pub fn ptr_to_int(&mut self, ptr: ValueId, ty: LLVMTypeId, name: &str) -> ValueId {
        let p = self.arena.get_value(ptr);
        let target = self.arena.get_type(ty).into_int_type();
        if !p.is_pointer_value() {
            tracing::error!(val_type = ?p.get_type(), "ptr_to_int on non-pointer operand");
            self.record_codegen_error();
            return self.const_i64(0);
        }
        let result = self
            .builder
            .build_ptr_to_int(p.into_pointer_value(), target, name)
            .expect("ptr_to_int");
        self.arena.push_value(result.into())
    }

    /// Build integer-to-pointer conversion.
    pub fn int_to_ptr(&mut self, val: ValueId, name: &str) -> ValueId {
        let v = self.arena.get_value(val);
        if !v.is_int_value() {
            tracing::error!(val_type = ?v.get_type(), "int_to_ptr on non-int operand");
            self.record_codegen_error();
            return self.const_null_ptr();
        }
        let result = self
            .builder
            .build_int_to_ptr(v.into_int_value(), self.scx.type_ptr(), name)
            .expect("int_to_ptr");
        self.arena.push_value(result.into())
    }

    // -----------------------------------------------------------------------
    // Control flow
    // -----------------------------------------------------------------------

    /// Build an unconditional branch.
    pub fn br(&mut self, dest: BlockId) {
        let bb = self.arena.get_block(dest);
        self.builder
            .build_unconditional_branch(bb)
            .expect("build_br");
    }

    /// Build a conditional branch.
    ///
    /// Defensive: if `cond` is not an i1/int value, falls back to an
    /// unconditional branch to the else block instead of panicking.
    pub fn cond_br(&mut self, cond: ValueId, then_bb: BlockId, else_bb: BlockId) {
        let raw = self.arena.get_value(cond);
        if !raw.is_int_value() {
            tracing::error!(val_type = ?raw.get_type(), "cond_br on non-int — branching to else");
            self.record_codegen_error();
            self.br(else_bb);
            return;
        }
        let then_block = self.arena.get_block(then_bb);
        let else_block = self.arena.get_block(else_bb);
        self.builder
            .build_conditional_branch(raw.into_int_value(), then_block, else_block)
            .expect("build_cond_br");
    }

    /// Build a switch instruction.
    ///
    /// Defensive: if the scrutinee or any case value is not an int, falls
    /// back to a branch to the default block instead of panicking.
    pub fn switch(&mut self, val: ValueId, default: BlockId, cases: &[(ValueId, BlockId)]) {
        let raw = self.arena.get_value(val);
        if !raw.is_int_value() {
            tracing::error!(val_type = ?raw.get_type(), "switch on non-int — branching to default");
            self.record_codegen_error();
            self.br(default);
            return;
        }
        let default_bb = self.arena.get_block(default);
        let mut resolved: Vec<(IntValue<'ctx>, BasicBlock<'ctx>)> = Vec::with_capacity(cases.len());
        for &(case_val, case_bb) in cases {
            let case_raw = self.arena.get_value(case_val);
            if !case_raw.is_int_value() {
                tracing::error!(val_type = ?case_raw.get_type(), "switch case is non-int — branching to default");
                self.record_codegen_error();
                self.br(default);
                return;
            }
            resolved.push((case_raw.into_int_value(), self.arena.get_block(case_bb)));
        }
        let switch = self
            .builder
            .build_switch(raw.into_int_value(), default_bb, &resolved)
            .expect("build_switch");
        let _ = switch;
    }

    /// Build a select (ternary) instruction.
    ///
    /// Defensive: if `cond` is not an i1/int, returns the else value
    /// instead of panicking.
    pub fn select(
        &mut self,
        cond: ValueId,
        then_val: ValueId,
        else_val: ValueId,
        name: &str,
    ) -> ValueId {
        let raw = self.arena.get_value(cond);
        if !raw.is_int_value() {
            tracing::error!(val_type = ?raw.get_type(), "select on non-int cond — returning else");
            self.record_codegen_error();
            return else_val;
        }
        let t = self.arena.get_value(then_val);
        let e = self.arena.get_value(else_val);
        let v = self
            .builder
            .build_select(raw.into_int_value(), t, e, name)
            .expect("select");
        self.arena.push_value(v)
    }

    /// Build a return with a value.
    pub fn ret(&mut self, val: ValueId) {
        let v = self.arena.get_value(val);
        self.builder.build_return(Some(&v)).expect("build_return");
    }

    /// Build a void return.
    pub fn ret_void(&mut self) {
        self.builder.build_return(None).expect("build_return");
    }

    /// Build an unreachable terminator.
    pub fn unreachable(&mut self) {
        self.builder.build_unreachable().expect("build_unreachable");
    }

    // -----------------------------------------------------------------------
    // Aggregates
    // -----------------------------------------------------------------------

    /// Extract a value from an aggregate (struct/array) by index.
    pub fn extract_value(&mut self, agg: ValueId, index: u32, name: &str) -> Option<ValueId> {
        let raw = self.arena.get_value(agg);
        let BasicValueEnum::StructValue(v) = raw else {
            tracing::error!(?raw, index, "extract_value on non-struct value");
            self.record_codegen_error();
            return None;
        };
        self.builder
            .build_extract_value(v, index, name)
            .ok()
            .map(|result| self.arena.push_value(result))
    }

    /// Insert a value into an aggregate at the given index.
    pub fn insert_value(&mut self, agg: ValueId, val: ValueId, index: u32, name: &str) -> ValueId {
        let raw_agg = self.arena.get_value(agg);
        let BasicValueEnum::StructValue(a) = raw_agg else {
            tracing::error!(?raw_agg, index, "insert_value on non-struct value");
            self.record_codegen_error();
            return agg; // Return unchanged aggregate
        };
        let v = self.arena.get_value(val);
        let result = self
            .builder
            .build_insert_value(a, v, index, name)
            .expect("insert_value");
        match result {
            inkwell::values::AggregateValueEnum::StructValue(sv) => {
                self.arena.push_value(sv.into())
            }
            inkwell::values::AggregateValueEnum::ArrayValue(av) => self.arena.push_value(av.into()),
        }
    }

    /// Build a struct from values by successive `insert_value`.
    pub fn build_struct(&mut self, ty: LLVMTypeId, values: &[ValueId], name: &str) -> ValueId {
        let raw_ty = self.arena.get_type(ty);

        // Defensive: verify this is actually a struct type
        let BasicTypeEnum::StructType(struct_ty) = raw_ty else {
            tracing::error!(
                ?raw_ty,
                "build_struct called with non-struct type — falling back"
            );
            self.record_codegen_error();
            return values.first().copied().unwrap_or_else(|| self.const_i64(0));
        };

        let mut result = struct_ty.get_undef();
        for (i, &val_id) in values.iter().enumerate() {
            let v = self.arena.get_value(val_id);
            let Some(agg) = self
                .builder
                .build_insert_value(result, v, i as u32, &format!("{name}.{i}"))
                .ok()
            else {
                tracing::error!(
                    index = i,
                    num_fields = struct_ty.count_fields(),
                    "build_struct: insert_value failed (index out of bounds?)"
                );
                self.record_codegen_error();
                return self.arena.push_value(struct_ty.get_undef().into());
            };
            match agg {
                inkwell::values::AggregateValueEnum::StructValue(sv) => result = sv,
                inkwell::values::AggregateValueEnum::ArrayValue(_) => {
                    tracing::error!(index = i, "build_struct insert_value returned array");
                    self.record_codegen_error();
                    return self.arena.push_value(struct_ty.get_undef().into());
                }
            }
        }
        self.arena.push_value(result.into())
    }

    // -----------------------------------------------------------------------
    // Calls
    // -----------------------------------------------------------------------

    /// Build a direct function call.
    ///
    /// Returns `None` for void-returning functions.
    pub fn call(&mut self, callee: FunctionId, args: &[ValueId], name: &str) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();
        let call_val = self
            .builder
            .build_call(func, &arg_vals, name)
            .expect("call");
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build a direct function call marked as a tail call.
    ///
    /// Sets the `tail` attribute on the call instruction, which tells LLVM
    /// that this call is in tail position. Combined with `fastcc`, LLVM will
    /// perform tail call optimization (reusing the caller's stack frame).
    ///
    /// Returns `None` for void-returning functions.
    pub fn call_tail(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();
        let call_val = self
            .builder
            .build_call(func, &arg_vals, name)
            .expect("call_tail");
        call_val.set_tail_call(true);
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect call through a function pointer.
    ///
    /// `return_type` is the function's return type; `param_types` are the
    /// parameter types. These are used to construct the LLVM function type
    /// needed for the indirect call.
    ///
    /// Returns `None` for void-returning functions.
    pub fn call_indirect(
        &mut self,
        return_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
        name: &str,
    ) -> Option<ValueId> {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "call_indirect on non-pointer");
            self.record_codegen_error();
            return None;
        }
        let ptr = raw.into_pointer_value();
        let arg_vals: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> = args
            .iter()
            .map(|&id| self.arena.get_value(id).into())
            .collect();

        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let func_ty = ret_ty.fn_type(&param_tys, false);

        let call_val = self
            .builder
            .build_indirect_call(func_ty, ptr, &arg_vals, name)
            .expect("call_indirect");
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    // -----------------------------------------------------------------------
    // Exception handling (invoke / landingpad / resume)
    // -----------------------------------------------------------------------

    /// Build a direct invoke (call that may unwind).
    ///
    /// On normal return, execution continues at `then_block`.
    /// On unwind (exception), execution continues at `catch_block`.
    ///
    /// Returns `None` for void-returning functions, `Some(ValueId)` otherwise.
    /// The result value is only valid in `then_block`.
    pub fn invoke(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self.arena.get_function(callee);
        let arg_vals: Vec<BasicValueEnum<'ctx>> =
            args.iter().map(|&id| self.arena.get_value(id)).collect();
        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);
        let call_val = self
            .builder
            .build_invoke(func, &arg_vals, then_bb, catch_bb, name)
            .expect("invoke");
        // inkwell's build_invoke does not automatically copy the calling
        // convention from the callee (unlike build_call). Without this,
        // fastcc callees get invoked with the default ccc, causing SIGSEGV.
        call_val.set_call_convention(func.get_call_conventions());
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build an indirect invoke through a function pointer.
    ///
    /// Like [`invoke`], but the callee is a function pointer with an
    /// explicit type signature.
    pub fn invoke_indirect(
        &mut self,
        return_type: LLVMTypeId,
        param_types: &[LLVMTypeId],
        fn_ptr: ValueId,
        args: &[ValueId],
        then_block: BlockId,
        catch_block: BlockId,
        name: &str,
    ) -> Option<ValueId> {
        let raw = self.arena.get_value(fn_ptr);
        if !raw.is_pointer_value() {
            tracing::error!(val_type = ?raw.get_type(), "invoke_indirect on non-pointer");
            self.record_codegen_error();
            return None;
        }
        let ptr = raw.into_pointer_value();
        let arg_vals: Vec<BasicValueEnum<'ctx>> =
            args.iter().map(|&id| self.arena.get_value(id)).collect();

        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let func_ty = ret_ty.fn_type(&param_tys, false);

        let then_bb = self.arena.get_block(then_block);
        let catch_bb = self.arena.get_block(catch_block);
        let call_val = self
            .builder
            .build_indirect_invoke(func_ty, ptr, &arg_vals, then_bb, catch_bb, name)
            .expect("invoke_indirect");
        call_val
            .try_as_basic_value()
            .basic()
            .map(|v| self.arena.push_value(v))
    }

    /// Build a `landingpad` instruction for exception handling cleanup.
    ///
    /// `personality` is the personality function (typically `__gxx_personality_v0`
    /// for C++/Rust Itanium EH ABI). `is_cleanup` should be `true` for cleanup
    /// pads that don't catch specific exceptions.
    ///
    /// Returns the landing pad value (an `{ i8*, i32 }` struct) as a `ValueId`.
    pub fn landingpad(&mut self, personality: FunctionId, is_cleanup: bool, name: &str) -> ValueId {
        let personality_fn = self.arena.get_function(personality);

        // Landing pad type is { ptr, i32 } (Itanium ABI convention).
        let i8_ptr_ty = self.scx.ptr_type;
        let i32_ty = self.scx.llcx.i32_type();
        let lp_ty = self
            .scx
            .llcx
            .struct_type(&[i8_ptr_ty.into(), i32_ty.into()], false);

        let lp_val = self
            .builder
            .build_landing_pad(lp_ty, personality_fn, &[], is_cleanup, name)
            .expect("landingpad");
        self.arena.push_value(lp_val)
    }

    /// Build a `resume` instruction to re-raise an exception.
    ///
    /// `value` must be the result of a `landingpad` instruction.
    /// This terminates the current basic block.
    pub fn resume(&mut self, value: ValueId) {
        let v = self.arena.get_value(value);
        self.builder.build_resume(v).expect("resume");
    }

    /// Set the personality function on an LLVM function.
    ///
    /// Required for any function containing `invoke`/`landingpad`.
    /// Typically `__gxx_personality_v0` (Itanium EH ABI on Linux/macOS).
    pub fn set_personality(&mut self, func: FunctionId, personality: FunctionId) {
        let func_val = self.arena.get_function(func);
        let personality_fn = self.arena.get_function(personality);
        func_val.set_personality_function(personality_fn);
    }

    // -----------------------------------------------------------------------
    // Phi nodes
    // -----------------------------------------------------------------------

    /// Build an empty phi node.
    ///
    /// The caller must add incoming values afterwards using the returned
    /// `ValueId`. Note: the underlying inkwell `PhiValue` is stored as a
    /// `BasicValueEnum` (via `as_basic_value()`).
    pub fn phi(&mut self, ty: LLVMTypeId, name: &str) -> ValueId {
        let llvm_ty = self.arena.get_type(ty);
        let phi = self.builder.build_phi(llvm_ty, name).expect("phi");
        self.arena.push_value(phi.as_basic_value())
    }

    /// Add incoming values to a phi node.
    ///
    /// The `phi` parameter must be a `ValueId` returned by `self.phi()`.
    /// We reconstruct the `PhiValue` from the stored LLVM value ref.
    pub fn add_phi_incoming(&mut self, phi: ValueId, incoming: &[(ValueId, BlockId)]) {
        use inkwell::values::AsValueRef;

        let phi_val = self.arena.get_value(phi);

        // SAFETY: `phi_val` was created by `build_phi` and stored via
        // `as_basic_value()`. The underlying LLVMValueRef is still a phi.
        let raw_phi = unsafe { inkwell::values::PhiValue::new(phi_val.as_value_ref()) };

        // Collect values and blocks into owned Vecs so we can borrow them.
        let vals: Vec<BasicValueEnum<'ctx>> = incoming
            .iter()
            .map(|&(v, _)| self.arena.get_value(v))
            .collect();
        let blocks: Vec<BasicBlock<'ctx>> = incoming
            .iter()
            .map(|&(_, b)| self.arena.get_block(b))
            .collect();

        // Build the &[(&dyn BasicValue, BasicBlock)] slice that inkwell expects.
        let refs: Vec<(&dyn BasicValue<'ctx>, BasicBlock<'ctx>)> = vals
            .iter()
            .zip(blocks.iter())
            .map(|(v, &b)| (v as &dyn BasicValue<'ctx>, b))
            .collect();
        raw_phi.add_incoming(&refs);
    }

    /// Build a phi from a list of incoming (value, block) pairs.
    ///
    /// Optimizations:
    /// - 0 incoming → returns `None`
    /// - 1 incoming → returns the value directly (no phi needed)
    /// - 2+ incoming → creates a real phi node
    pub fn phi_from_incoming(
        &mut self,
        ty: LLVMTypeId,
        incoming: &[(ValueId, BlockId)],
        name: &str,
    ) -> Option<ValueId> {
        match incoming.len() {
            0 => None,
            1 => Some(incoming[0].0),
            _ => {
                let phi_id = self.phi(ty, name);
                self.add_phi_incoming(phi_id, incoming);
                Some(phi_id)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Type registration
    // -----------------------------------------------------------------------

    /// Register an LLVM type in the arena.
    pub fn register_type(&mut self, ty: BasicTypeEnum<'ctx>) -> LLVMTypeId {
        self.arena.push_type(ty)
    }

    /// Register and return the `i1` (bool) type ID.
    #[inline]
    pub fn bool_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i1().into())
    }

    /// Register and return the `i8` type ID.
    #[inline]
    pub fn i8_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i8().into())
    }

    /// Register and return the `i32` type ID.
    #[inline]
    pub fn i32_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i32().into())
    }

    /// Register and return the `i64` type ID.
    #[inline]
    pub fn i64_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i64().into())
    }

    /// Register and return the `f64` type ID.
    #[inline]
    pub fn f64_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_f64().into())
    }

    /// Register and return the opaque pointer type ID.
    #[inline]
    pub fn ptr_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_ptr().into())
    }

    /// Register and return the unit type ID (i64, matching Ori convention).
    #[inline]
    pub fn unit_type(&mut self) -> LLVMTypeId {
        self.arena.push_type(self.scx.type_i64().into())
    }

    /// Register and return the closure fat-pointer type `{ ptr, ptr }`.
    ///
    /// All function-typed values use this two-pointer representation:
    /// field 0 = function pointer, field 1 = environment pointer (null if
    /// no captures).
    pub fn closure_type(&mut self) -> LLVMTypeId {
        let struct_ty = self.scx.type_struct(
            &[self.scx.type_ptr().into(), self.scx.type_ptr().into()],
            false,
        );
        self.arena.push_type(struct_ty.into())
    }

    // -----------------------------------------------------------------------
    // Block management
    // -----------------------------------------------------------------------

    /// Append a new basic block to a function.
    pub fn append_block(&mut self, function: FunctionId, name: &str) -> BlockId {
        let func = self.arena.get_function(function);
        let bb = self.scx.llcx.append_basic_block(func, name);
        self.arena.push_block(bb)
    }

    /// Position the builder at the end of a basic block.
    pub fn position_at_end(&mut self, block: BlockId) {
        let bb = self.arena.get_block(block);
        self.builder.position_at_end(bb);
        self.current_block = Some(block);
    }

    /// Get the current basic block ID (if any).
    #[inline]
    pub fn current_block(&self) -> Option<BlockId> {
        self.current_block
    }

    /// Check if the current block is already terminated.
    pub fn current_block_terminated(&self) -> bool {
        self.current_block
            .is_some_and(|id| self.arena.get_block(id).get_terminator().is_some())
    }

    // -----------------------------------------------------------------------
    // Position management
    // -----------------------------------------------------------------------

    /// Save the current builder position, returning the block ID.
    ///
    /// Call `restore_position` with the returned ID to restore.
    /// This uses the manual save/restore pattern to avoid borrow checker
    /// friction with RAII guards and `&mut self`.
    #[inline]
    pub fn save_position(&self) -> Option<BlockId> {
        self.current_block
    }

    /// Restore builder position to a previously saved block.
    pub fn restore_position(&mut self, saved: Option<BlockId>) {
        if let Some(block_id) = saved {
            let bb = self.arena.get_block(block_id);
            self.builder.position_at_end(bb);
            self.current_block = Some(block_id);
        }
    }

    // -----------------------------------------------------------------------
    // Function management
    // -----------------------------------------------------------------------

    /// Set the currently-active function.
    pub fn set_current_function(&mut self, func: FunctionId) {
        self.current_function = Some(func);
    }

    /// Get the currently-active function ID.
    #[inline]
    pub fn current_function(&self) -> Option<FunctionId> {
        self.current_function
    }

    /// Get the inkwell `FunctionValue` for the currently-active function.
    pub fn current_function_value(&self) -> Option<FunctionValue<'ctx>> {
        self.current_function.map(|id| self.arena.get_function(id))
    }

    /// Get the inkwell `FunctionValue` for any function ID.
    pub fn get_function_value(&self, id: FunctionId) -> FunctionValue<'ctx> {
        self.arena.get_function(id)
    }

    /// Get a function parameter as a `ValueId`.
    ///
    /// `param_index` is the LLVM-level parameter index (0-based, includes
    /// hidden sret parameter if present).
    pub fn get_param(&mut self, func: FunctionId, param_index: u32) -> ValueId {
        let func_val = self.arena.get_function(func);
        let Some(param) = func_val.get_nth_param(param_index) else {
            tracing::error!(
                func = %func_val.get_name().to_string_lossy(),
                param_index,
                param_count = func_val.count_params(),
                "parameter index out of bounds — returning zero"
            );
            self.record_codegen_error();
            return self.const_i64(0);
        };
        self.arena.push_value(param)
    }

    /// Set the debug name of a value.
    pub fn set_value_name(&self, val: ValueId, name: &str) {
        let v = self.arena.get_value(val);
        v.set_name(name);
    }

    /// Check if a specific block has a terminator instruction.
    pub fn block_has_terminator(&self, block: BlockId) -> bool {
        self.arena.get_block(block).get_terminator().is_some()
    }

    /// Declare a function in the LLVM module.
    pub fn declare_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
        return_type: LLVMTypeId,
    ) -> FunctionId {
        let ret_ty = self.arena.get_type(return_type);
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let fn_type = ret_ty.fn_type(&param_tys, false);
        let func = self.scx.llmod.add_function(name, fn_type, None);
        self.arena.push_function(func)
    }

    /// Declare a void-returning function in the LLVM module.
    pub fn declare_void_function(&mut self, name: &str, param_types: &[LLVMTypeId]) -> FunctionId {
        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();
        let fn_type = self.scx.type_void_func(&param_tys);
        let func = self.scx.llmod.add_function(name, fn_type, None);
        self.arena.push_function(func)
    }

    /// Declare an external function with `External` linkage.
    ///
    /// Used for runtime library functions (`ori_print`, `ori_panic`, etc.)
    /// and imported functions from other modules. Supports void return
    /// (pass `None` for `return_type`).
    pub fn declare_extern_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
        return_type: Option<LLVMTypeId>,
    ) -> FunctionId {
        // Reuse existing declaration if present
        if let Some(func) = self.scx.llmod.get_function(name) {
            return self.arena.push_function(func);
        }

        let param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = param_types
            .iter()
            .map(|&id| self.arena.get_type(id).into())
            .collect();

        let fn_type = match return_type {
            Some(ret_id) => {
                let ret_ty = self.arena.get_type(ret_id);
                ret_ty.fn_type(&param_tys, false)
            }
            None => self.scx.type_void_func(&param_tys),
        };

        let func = self
            .scx
            .llmod
            .add_function(name, fn_type, Some(Linkage::External));
        self.arena.push_function(func)
    }

    // -----------------------------------------------------------------------
    // Calling conventions
    // -----------------------------------------------------------------------

    /// Set the calling convention on a function.
    ///
    /// Convention IDs: 0 = C, 8 = fastcc. See LLVM CallingConv.h.
    pub fn set_calling_convention(&mut self, func: FunctionId, conv: u32) {
        let f = self.arena.get_function(func);
        f.set_call_conventions(conv);
    }

    /// Set `fastcc` calling convention on a function.
    ///
    /// Internal Ori functions use `fastcc` for better optimization (tail calls,
    /// non-standard register allocation).
    pub fn set_fastcc(&mut self, func: FunctionId) {
        self.set_calling_convention(func, 8); // LLVM FastCC = 8
    }

    /// Set C calling convention on a function.
    ///
    /// Used for `@main`, extern functions, and runtime library calls.
    pub fn set_ccc(&mut self, func: FunctionId) {
        self.set_calling_convention(func, 0); // LLVM CCC = 0
    }

    // -----------------------------------------------------------------------
    // Function attributes
    // -----------------------------------------------------------------------

    /// Add the `nounwind` attribute to a function.
    ///
    /// Declares the function will not unwind (no exceptions). Enables LLVM
    /// to optimize exception handling paths around calls to this function.
    pub fn add_nounwind_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("nounwind");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noinline` attribute to a function.
    ///
    /// Prevents LLVM from inlining this function. Used for cold paths like
    /// specialized drop functions and panic handlers.
    pub fn add_noinline_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noinline");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `cold` attribute to a function.
    ///
    /// Hints that this function is rarely called. LLVM uses this to:
    /// - Move cold code out of hot code layout
    /// - Reduce inlining priority
    /// - Optimize branch prediction away from cold paths
    pub fn add_cold_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("cold");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `noalias` attribute to a function's return value.
    ///
    /// Guarantees the returned pointer does not alias any other pointer
    /// visible to the caller. Used for allocation functions like `ori_rc_alloc`
    /// where the returned pointer is a fresh heap allocation.
    pub fn add_noalias_return_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("noalias");
        let attr = self.scx.llcx.create_enum_attribute(kind, 0);
        f.add_attribute(AttributeLoc::Return, attr);
    }

    /// Add the `memory(argmem: readwrite)` attribute to a function.
    ///
    /// Declares the function only reads/writes memory reachable from its pointer
    /// arguments (no global memory access, no inaccessible memory). This is
    /// critical for ARC runtime functions (`ori_rc_inc`, `ori_rc_dec`) which
    /// modify refcount at `ptr - 8` but don't access any other memory.
    ///
    /// # LLVM `MemoryEffects` Encoding
    ///
    /// The `memory` attribute uses a bitfield encoding from `ModRef.h`:
    /// - Bits \[1:0\]: `DefaultMem` access (None=0, Ref=1, Mod=2, ModRef=3)
    /// - Bits \[3:2\]: `ArgMem` access
    /// - Bits \[5:4\]: `InaccessibleMem` access
    ///
    /// `memory(argmem: readwrite)` = DefaultMem:None | ArgMem:ModRef | InaccessibleMem:None
    /// = 0 | (3 << 2) | (0 << 4) = 12
    pub fn add_memory_argmem_readwrite_attribute(&mut self, func: FunctionId) {
        let f = self.arena.get_function(func);
        let kind = Attribute::get_named_enum_kind_id("memory");
        // MemoryEffects encoding: argmem: readwrite (ModRef=3 at ArgMem position bits [3:2])
        let attr = self.scx.llcx.create_enum_attribute(kind, 12);
        f.add_attribute(AttributeLoc::Function, attr);
    }

    /// Add the `sret(T)` attribute to a function parameter.
    ///
    /// Marks the parameter as a hidden struct return pointer. LLVM uses
    /// this to optimize the return path and generate correct ABI code.
    pub fn add_sret_attribute(
        &mut self,
        func: FunctionId,
        param_index: u32,
        pointee_type: LLVMTypeId,
    ) {
        let f = self.arena.get_function(func);
        let ty = self.arena.get_type(pointee_type);
        let sret_kind = Attribute::get_named_enum_kind_id("sret");
        let sret_attr = self
            .scx
            .llcx
            .create_type_attribute(sret_kind, ty.as_any_type_enum());
        f.add_attribute(AttributeLoc::Param(param_index), sret_attr);
    }

    /// Add the `noalias` attribute to a function parameter.
    ///
    /// Guarantees the parameter pointer does not alias any other pointer
    /// visible to the callee. Required on sret parameters by the x86-64 ABI.
    pub fn add_noalias_attribute(&mut self, func: FunctionId, param_index: u32) {
        let f = self.arena.get_function(func);
        let noalias_kind = Attribute::get_named_enum_kind_id("noalias");
        let noalias_attr = self.scx.llcx.create_enum_attribute(noalias_kind, 0);
        f.add_attribute(AttributeLoc::Param(param_index), noalias_attr);
    }

    /// Add the `byval(T)` attribute to a function parameter.
    ///
    /// Indicates the parameter is passed by value on the stack. The callee
    /// receives a copy; modifications don't affect the caller's data.
    pub fn add_byval_attribute(
        &mut self,
        func: FunctionId,
        param_index: u32,
        pointee_type: LLVMTypeId,
    ) {
        let f = self.arena.get_function(func);
        let ty = self.arena.get_type(pointee_type);
        let byval_kind = Attribute::get_named_enum_kind_id("byval");
        let byval_attr = self
            .scx
            .llcx
            .create_type_attribute(byval_kind, ty.as_any_type_enum());
        f.add_attribute(AttributeLoc::Param(param_index), byval_attr);
    }

    // -----------------------------------------------------------------------
    // sret call helper
    // -----------------------------------------------------------------------

    /// Build a call to an sret function, hiding the ABI complexity.
    ///
    /// For functions using the sret convention:
    /// 1. Allocates stack space for the return value
    /// 2. Prepends the sret pointer as the first argument
    /// 3. Calls the void function
    /// 4. Loads the result from the sret pointer
    ///
    /// Returns the loaded result value, making sret transparent to callers.
    pub fn call_with_sret(
        &mut self,
        callee: FunctionId,
        args: &[ValueId],
        sret_type: LLVMTypeId,
        name: &str,
    ) -> Option<ValueId> {
        let func = self
            .current_function
            .expect("call_with_sret requires active function");

        // Allocate stack space at entry block for the return value
        let sret_ptr = self.create_entry_alloca(func, &format!("{name}.sret"), sret_type);

        // Prepend sret pointer to args
        let mut full_args = Vec::with_capacity(args.len() + 1);
        full_args.push(sret_ptr);
        full_args.extend_from_slice(args);

        // Call the void function (sret functions always return void)
        self.call(callee, &full_args, "");

        // Load the result from the sret pointer
        let result = self.load(sret_type, sret_ptr, name);
        Some(result)
    }

    /// Get or declare a function by name.
    ///
    /// If the function already exists in the module, registers it in the
    /// arena and returns its ID. Otherwise declares a new function.
    pub fn get_or_declare_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
        return_type: LLVMTypeId,
    ) -> FunctionId {
        if let Some(func) = self.scx.llmod.get_function(name) {
            self.arena.push_function(func)
        } else {
            self.declare_function(name, param_types, return_type)
        }
    }

    /// Get or declare a void-returning function by name.
    ///
    /// If the function already exists in the module, registers it in the
    /// arena and returns its ID. Otherwise declares a new void function.
    pub fn get_or_declare_void_function(
        &mut self,
        name: &str,
        param_types: &[LLVMTypeId],
    ) -> FunctionId {
        if let Some(func) = self.scx.llmod.get_function(name) {
            self.arena.push_function(func)
        } else {
            self.declare_void_function(name, param_types)
        }
    }

    /// Get a function's address as a pointer `ValueId`.
    ///
    /// Used for passing function pointers to runtime calls (e.g., registering
    /// the panic handler trampoline).
    pub fn get_function_ptr(&mut self, func: FunctionId) -> ValueId {
        let func_val = self.arena.get_function(func);
        let ptr_val = func_val.as_global_value().as_pointer_value();
        self.arena.push_value(ptr_val.into())
    }

    // -----------------------------------------------------------------------
    // Raw value access (for interop with existing code)
    // -----------------------------------------------------------------------

    /// Access the underlying inkwell `Builder` for direct LLVM operations.
    ///
    /// Needed by `DebugContext` to set debug locations and emit debug
    /// intrinsics (`insert_declare_at_end`, `insert_dbg_value_before`).
    pub fn inkwell_builder(&self) -> &InkwellBuilder<'ctx> {
        &self.builder
    }

    /// Get the raw `BasicValueEnum` for a `ValueId`.
    ///
    /// Use sparingly — this is for interop with code that hasn't been
    /// migrated to IDs yet.
    pub fn raw_value(&self, id: ValueId) -> BasicValueEnum<'ctx> {
        self.arena.get_value(id)
    }

    /// Get the raw `BasicTypeEnum` for an `LLVMTypeId`.
    pub fn raw_type(&self, id: LLVMTypeId) -> BasicTypeEnum<'ctx> {
        self.arena.get_type(id)
    }

    /// Get the raw `BasicBlock` for a `BlockId`.
    pub fn raw_block(&self, id: BlockId) -> BasicBlock<'ctx> {
        self.arena.get_block(id)
    }

    /// Intern a raw `BasicValueEnum` into the arena, returning a `ValueId`.
    pub fn intern_value(&mut self, val: BasicValueEnum<'ctx>) -> ValueId {
        self.arena.push_value(val)
    }

    /// Intern a raw `BasicBlock` into the arena, returning a `BlockId`.
    pub fn intern_block(&mut self, bb: BasicBlock<'ctx>) -> BlockId {
        self.arena.push_block(bb)
    }

    /// Intern a raw `FunctionValue` into the arena, returning a `FunctionId`.
    pub fn intern_function(&mut self, func: FunctionValue<'ctx>) -> FunctionId {
        self.arena.push_function(func)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::approx_constant,
    clippy::doc_markdown,
    reason = "test code — approximate constants are intentional, doc style relaxed"
)]
mod tests {
    use super::*;
    use inkwell::context::Context;

    /// Helper: create a `SimpleCx` for testing.
    fn test_scx(ctx: &Context) -> SimpleCx<'_> {
        SimpleCx::new(ctx, "ir_builder_test")
    }

    /// Helper: set up an `IrBuilder` with a function and entry block.
    fn setup_builder(irb: &mut IrBuilder<'_, '_>) {
        let i64_ty = irb.i64_type();
        let func = irb.declare_function("test_fn", &[], i64_ty);
        let entry = irb.append_block(func, "entry");
        irb.set_current_function(func);
        irb.position_at_end(entry);
    }

    // -- Constant creation --

    #[test]
    fn const_i64_roundtrip() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let id = irb.const_i64(42);
        let val = irb.raw_value(id);
        assert!(val.is_int_value());
        assert_eq!(val.into_int_value().get_zero_extended_constant(), Some(42));
        drop(irb);
    }

    #[test]
    fn const_f64_roundtrip() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let id = irb.const_f64(3.14);
        let val = irb.raw_value(id);
        assert!(val.is_float_value());
        drop(irb);
    }

    #[test]
    fn const_bool_roundtrip() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let t = irb.const_bool(true);
        let f = irb.const_bool(false);
        assert_eq!(
            irb.raw_value(t)
                .into_int_value()
                .get_zero_extended_constant(),
            Some(1)
        );
        assert_eq!(
            irb.raw_value(f)
                .into_int_value()
                .get_zero_extended_constant(),
            Some(0)
        );
        drop(irb);
    }

    // -- Arithmetic --

    #[test]
    fn integer_arithmetic() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let a = irb.const_i64(10);
        let b = irb.const_i64(3);

        let sum = irb.add(a, b, "sum");
        let diff = irb.sub(a, b, "diff");
        let prod = irb.mul(a, b, "prod");
        let quot = irb.sdiv(a, b, "quot");
        let rem = irb.srem(a, b, "rem");
        let n = irb.neg(a, "neg");

        assert_ne!(sum, diff);
        assert_ne!(prod, quot);
        assert!(irb.raw_value(sum).is_int_value());
        assert!(irb.raw_value(rem).is_int_value());
        assert!(irb.raw_value(n).is_int_value());
        drop(irb);
    }

    #[test]
    fn float_arithmetic() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let a = irb.const_f64(2.5);
        let b = irb.const_f64(1.5);

        let sum = irb.fadd(a, b, "fsum");
        let diff = irb.fsub(a, b, "fdiff");
        let prod = irb.fmul(a, b, "fprod");
        let quot = irb.fdiv(a, b, "fquot");
        let rem = irb.frem(a, b, "frem");
        let n = irb.fneg(a, "fneg");

        assert!(irb.raw_value(sum).is_float_value());
        assert!(irb.raw_value(diff).is_float_value());
        assert!(irb.raw_value(prod).is_float_value());
        assert!(irb.raw_value(quot).is_float_value());
        assert!(irb.raw_value(rem).is_float_value());
        assert!(irb.raw_value(n).is_float_value());
        drop(irb);
    }

    // -- Memory --

    #[test]
    fn alloca_load_store_roundtrip() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let i64_ty = irb.i64_type();
        let ptr = irb.alloca(i64_ty, "x");
        let val = irb.const_i64(99);
        irb.store(val, ptr);
        let loaded = irb.load(i64_ty, ptr, "x_loaded");

        assert!(irb.raw_value(ptr).is_pointer_value());
        assert!(irb.raw_value(loaded).is_int_value());
        drop(irb);
    }

    #[test]
    fn create_entry_alloca_inserts_at_entry() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("entry_test", &[], i64_ty);
        let _entry = irb.append_block(func, "entry");
        let second = irb.append_block(func, "second");
        irb.set_current_function(func);

        // Position in second block.
        irb.position_at_end(second);
        let saved = irb.current_block();
        assert_eq!(saved, Some(second));

        // Create entry alloca — should insert in entry, then restore to second.
        let ptr = irb.create_entry_alloca(func, "entry_var", i64_ty);
        assert!(irb.raw_value(ptr).is_pointer_value());
        assert_eq!(irb.current_block(), Some(second));
        drop(irb);
    }

    // -- Block management --

    #[test]
    fn block_creation_and_positioning() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("block_test", &[], i64_ty);
        let bb1 = irb.append_block(func, "bb1");
        let bb2 = irb.append_block(func, "bb2");

        assert_ne!(bb1, bb2);

        irb.position_at_end(bb1);
        assert_eq!(irb.current_block(), Some(bb1));

        irb.position_at_end(bb2);
        assert_eq!(irb.current_block(), Some(bb2));
        drop(irb);
    }

    #[test]
    fn current_block_terminated() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        assert!(!irb.current_block_terminated());

        let val = irb.const_i64(0);
        irb.ret(val);

        assert!(irb.current_block_terminated());
        drop(irb);
    }

    // -- Phi nodes --

    #[test]
    fn phi_from_incoming_zero() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let i64_ty = irb.i64_type();
        let result = irb.phi_from_incoming(i64_ty, &[], "empty");
        assert!(result.is_none());
        drop(irb);
    }

    #[test]
    fn phi_from_incoming_single() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let i64_ty = irb.i64_type();
        let val = irb.const_i64(42);
        let current = irb.current_block().unwrap();

        let result = irb.phi_from_incoming(i64_ty, &[(val, current)], "single");
        assert_eq!(result, Some(val));
        drop(irb);
    }

    #[test]
    fn phi_from_incoming_multiple() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("phi_test", &[], i64_ty);
        let bb1 = irb.append_block(func, "bb1");
        let bb2 = irb.append_block(func, "bb2");
        let merge = irb.append_block(func, "merge");
        irb.set_current_function(func);

        irb.position_at_end(bb1);
        let v1 = irb.const_i64(1);
        irb.br(merge);

        irb.position_at_end(bb2);
        let v2 = irb.const_i64(2);
        irb.br(merge);

        irb.position_at_end(merge);
        let result = irb.phi_from_incoming(i64_ty, &[(v1, bb1), (v2, bb2)], "merged");
        assert!(result.is_some());
        let phi_id = result.unwrap();
        assert_ne!(phi_id, v1);
        assert_ne!(phi_id, v2);
        drop(irb);
    }

    // -- Position save/restore --

    #[test]
    fn position_save_restore() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("pos_test", &[], i64_ty);
        let bb1 = irb.append_block(func, "bb1");
        let bb2 = irb.append_block(func, "bb2");

        irb.position_at_end(bb1);
        let saved = irb.save_position();
        assert_eq!(saved, Some(bb1));

        irb.position_at_end(bb2);
        assert_eq!(irb.current_block(), Some(bb2));

        irb.restore_position(saved);
        assert_eq!(irb.current_block(), Some(bb1));
        drop(irb);
    }

    // -- Type registration --

    #[test]
    fn type_registration() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let bool_ty = irb.bool_type();
        let i8_ty = irb.i8_type();
        let i32_ty = irb.i32_type();
        let i64_ty = irb.i64_type();
        let f64_ty = irb.f64_type();
        let ptr_ty = irb.ptr_type();
        let unit_ty = irb.unit_type();

        assert_ne!(bool_ty, f64_ty);
        assert_ne!(i8_ty, i64_ty);

        assert_eq!(irb.raw_type(bool_ty), scx.type_i1().into());
        assert_eq!(irb.raw_type(i8_ty), scx.type_i8().into());
        assert_eq!(irb.raw_type(i32_ty), scx.type_i32().into());
        assert_eq!(irb.raw_type(i64_ty), scx.type_i64().into());
        assert_eq!(irb.raw_type(f64_ty), scx.type_f64().into());
        assert_eq!(irb.raw_type(ptr_ty), scx.type_ptr().into());
        assert_eq!(irb.raw_type(unit_ty), scx.type_i64().into());
        drop(irb);
    }

    // -- Select instruction --

    #[test]
    fn select_instruction() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let cond = irb.const_bool(true);
        let then_val = irb.const_i64(1);
        let else_val = irb.const_i64(2);

        let result = irb.select(cond, then_val, else_val, "sel");
        assert!(irb.raw_value(result).is_int_value());
        drop(irb);
    }

    // -- Function management --

    #[test]
    fn declare_and_get_function() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("my_func", &[i64_ty, i64_ty], i64_ty);

        let val = irb.get_function_value(func);
        assert_eq!(val.get_name().to_str().unwrap(), "my_func");
        assert_eq!(val.count_params(), 2);
        drop(irb);
    }

    #[test]
    fn get_or_declare_function_idempotent() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let f1 = irb.get_or_declare_function("idempotent_fn", &[i64_ty], i64_ty);
        let f2 = irb.get_or_declare_function("idempotent_fn", &[i64_ty], i64_ty);

        assert_eq!(irb.get_function_value(f1), irb.get_function_value(f2));
        drop(irb);
    }

    // -- Comparisons --

    #[test]
    fn integer_comparisons() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let a = irb.const_i64(5);
        let b = irb.const_i64(10);

        let eq = irb.icmp_eq(a, b, "eq");
        let ne = irb.icmp_ne(a, b, "ne");
        let slt = irb.icmp_slt(a, b, "slt");
        let sgt = irb.icmp_sgt(a, b, "sgt");

        assert!(irb.raw_value(eq).is_int_value());
        assert!(irb.raw_value(ne).is_int_value());
        assert!(irb.raw_value(slt).is_int_value());
        assert!(irb.raw_value(sgt).is_int_value());
        drop(irb);
    }

    #[test]
    fn float_comparisons() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let a = irb.const_f64(1.0);
        let b = irb.const_f64(2.0);

        let oeq = irb.fcmp_oeq(a, b, "oeq");
        let olt = irb.fcmp_olt(a, b, "olt");
        let ogt = irb.fcmp_ogt(a, b, "ogt");

        assert!(irb.raw_value(oeq).is_int_value());
        assert!(irb.raw_value(olt).is_int_value());
        assert!(irb.raw_value(ogt).is_int_value());
        drop(irb);
    }

    // -- Conversions --

    #[test]
    fn integer_conversions() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);
        setup_builder(&mut irb);

        let i32_ty = irb.i32_type();
        let i64_ty = irb.i64_type();

        let val32 = irb.const_i32(42);
        let extended = irb.sext(val32, i64_ty, "sext");
        assert!(irb.raw_value(extended).is_int_value());

        let val64 = irb.const_i64(42);
        let truncated = irb.trunc(val64, i32_ty, "trunc");
        assert!(irb.raw_value(truncated).is_int_value());

        let zexted = irb.zext(val32, i64_ty, "zext");
        assert!(irb.raw_value(zexted).is_int_value());
        drop(irb);
    }

    // -- Intern helpers --

    #[test]
    fn intern_raw_values() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let raw_val: BasicValueEnum = scx.type_i64().const_int(77, false).into();
        let id = irb.intern_value(raw_val);
        assert_eq!(irb.raw_value(id), raw_val);
        drop(irb);
    }

    // -- Void function declaration --

    #[test]
    fn declare_void_function() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_void_function("void_fn", &[i64_ty]);
        let val = irb.get_function_value(func);

        assert_eq!(val.get_name().to_str().unwrap(), "void_fn");
        assert_eq!(val.count_params(), 1);
        // Void return type → function returns void
        assert!(val.get_type().get_return_type().is_none());
        drop(irb);
    }

    // -- Calling conventions --

    #[test]
    fn set_fastcc_and_ccc() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func_fast = irb.declare_function("fast_fn", &[], i64_ty);
        irb.set_fastcc(func_fast);

        let func_c = irb.declare_function("c_fn", &[], i64_ty);
        irb.set_ccc(func_c);

        // Verify conventions were set (8 = fastcc, 0 = ccc)
        assert_eq!(irb.get_function_value(func_fast).get_call_conventions(), 8);
        assert_eq!(irb.get_function_value(func_c).get_call_conventions(), 0);
        drop(irb);
    }

    // -- sret attribute --

    #[test]
    fn sret_attribute_applied() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let ptr_ty = irb.ptr_type();
        let i64_ty = irb.i64_type();
        let struct_ty = irb.register_type(
            scx.type_struct(
                &[
                    scx.type_i64().into(),
                    scx.type_i64().into(),
                    scx.type_ptr().into(),
                ],
                false,
            )
            .into(),
        );

        // Declare void function with ptr param (the sret pointer)
        let func = irb.declare_void_function("sret_fn", &[ptr_ty, i64_ty]);
        irb.add_sret_attribute(func, 0, struct_ty);
        irb.add_noalias_attribute(func, 0);

        // Verify function has correct shape
        let val = irb.get_function_value(func);
        assert_eq!(val.count_params(), 2);
        assert!(val.get_type().get_return_type().is_none());
        drop(irb);
    }

    // -- declare_extern_function --

    #[test]
    fn declare_extern_function_basic() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let ptr_ty = irb.ptr_type();
        let func = irb.declare_extern_function("ori_print", &[ptr_ty], None);
        let val = irb.get_function_value(func);

        assert_eq!(val.get_name().to_str().unwrap(), "ori_print");
        assert_eq!(val.count_params(), 1);
        assert!(val.get_type().get_return_type().is_none());
        drop(irb);
    }

    #[test]
    fn declare_extern_function_with_return() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let ptr_ty = irb.ptr_type();
        let func = irb.declare_extern_function("ori_list_len", &[ptr_ty], Some(i64_ty));
        let val = irb.get_function_value(func);

        assert_eq!(val.get_name().to_str().unwrap(), "ori_list_len");
        assert!(val.get_type().get_return_type().is_some());
        drop(irb);
    }

    #[test]
    fn declare_extern_function_idempotent() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let ptr_ty = irb.ptr_type();
        let f1 = irb.declare_extern_function("ori_print", &[ptr_ty], None);
        let f2 = irb.declare_extern_function("ori_print", &[ptr_ty], None);

        assert_eq!(irb.get_function_value(f1), irb.get_function_value(f2));
        drop(irb);
    }

    // -- Tail calls --

    #[test]
    fn call_tail_marks_instruction() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();

        // Declare a fastcc function that calls itself
        let func = irb.declare_function("recursive_fn", &[i64_ty], i64_ty);
        irb.set_fastcc(func);
        let entry = irb.append_block(func, "entry");
        irb.set_current_function(func);
        irb.position_at_end(entry);

        // Build a tail call to itself
        let arg = irb.const_i64(1);
        let result = irb.call_tail(func, &[arg], "recurse");
        assert!(result.is_some());

        irb.ret(result.unwrap());

        // Verify the IR contains "tail call"
        let ir = scx.llmod.print_to_string().to_string();
        assert!(
            ir.contains("tail call"),
            "Expected 'tail call' in IR, got:\n{ir}"
        );
        drop(irb);
    }

    #[test]
    fn call_without_tail_has_no_tail_attribute() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("normal_fn", &[i64_ty], i64_ty);
        let entry = irb.append_block(func, "entry");
        irb.set_current_function(func);
        irb.position_at_end(entry);

        // Build a regular (non-tail) call
        let arg = irb.const_i64(1);
        let result = irb.call(func, &[arg], "normal");
        assert!(result.is_some());

        irb.ret(result.unwrap());

        // Verify the IR does NOT contain "tail call"
        let ir = scx.llmod.print_to_string().to_string();
        assert!(
            !ir.contains("tail call"),
            "Expected no 'tail call' in IR, got:\n{ir}"
        );
        drop(irb);
    }

    // -- call_with_sret --

    #[test]
    fn call_with_sret_creates_alloca_and_load() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        // Set up: declare a caller function and position in it
        let i64_ty = irb.i64_type();
        let caller = irb.declare_function("caller", &[], i64_ty);
        let entry = irb.append_block(caller, "entry");
        irb.set_current_function(caller);
        irb.position_at_end(entry);

        // Declare an sret callee: void fn(ptr sret, i64)
        let struct_ty = irb.register_type(
            scx.type_struct(
                &[
                    scx.type_i64().into(),
                    scx.type_i64().into(),
                    scx.type_ptr().into(),
                ],
                false,
            )
            .into(),
        );
        let ptr_ty = irb.ptr_type();
        let callee = irb.declare_void_function("sret_callee", &[ptr_ty, i64_ty]);

        // Call with sret
        let arg = irb.const_i64(42);
        let result = irb.call_with_sret(callee, &[arg], struct_ty, "result");

        // Result should be a value (loaded from sret alloca)
        assert!(result.is_some());
        drop(irb);
    }

    // -- Exception handling --

    /// Helper: set up a function with entry, then, and catch blocks for invoke tests.
    fn setup_invoke_blocks(irb: &mut IrBuilder<'_, '_>) -> (FunctionId, BlockId, BlockId, BlockId) {
        let i64_ty = irb.i64_type();
        let func = irb.declare_function("invoke_test_fn", &[i64_ty], i64_ty);
        let entry = irb.append_block(func, "entry");
        let then_block = irb.append_block(func, "then");
        let catch_block = irb.append_block(func, "catch");
        irb.set_current_function(func);
        irb.position_at_end(entry);
        (func, entry, then_block, catch_block)
    }

    #[test]
    fn invoke_produces_invoke_instruction() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let (func, _entry, then_block, catch_block) = setup_invoke_blocks(&mut irb);

        let arg = irb.const_i64(42);
        let result = irb.invoke(func, &[arg], then_block, catch_block, "inv_result");
        assert!(result.is_some());

        // The invoke terminates the entry block.
        assert!(irb.current_block_terminated());

        let ir = scx.llmod.print_to_string().to_string();
        assert!(ir.contains("invoke"), "Expected 'invoke' in IR, got:\n{ir}");
        drop(irb);
    }

    #[test]
    fn invoke_void_returns_none() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let caller = irb.declare_function("invoke_void_caller", &[], i64_ty);
        let entry = irb.append_block(caller, "entry");
        let then_block = irb.append_block(caller, "then");
        let catch_block = irb.append_block(caller, "catch");
        irb.set_current_function(caller);
        irb.position_at_end(entry);

        // Declare a void callee.
        let ptr_ty = irb.ptr_type();
        let void_fn = irb.declare_extern_function("void_callee", &[ptr_ty], None);

        let arg = irb.const_i64(0);
        let ptr_val = irb.int_to_ptr(arg, "as_ptr");
        let result = irb.invoke(void_fn, &[ptr_val], then_block, catch_block, "");
        assert!(result.is_none(), "void invoke should return None");
        drop(irb);
    }

    #[test]
    fn landingpad_produces_struct_value() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let (func, _entry, then_block, catch_block) = setup_invoke_blocks(&mut irb);

        // Declare personality function.
        let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);
        irb.set_personality(func, personality);

        // Invoke in entry, then landingpad in catch.
        let arg = irb.const_i64(1);
        irb.invoke(func, &[arg], then_block, catch_block, "inv");

        irb.position_at_end(catch_block);
        let lp = irb.landingpad(personality, true, "lp");

        // The landing pad value is a struct { ptr, i32 }.
        let lp_val = irb.raw_value(lp);
        assert!(lp_val.is_struct_value());

        let ir = scx.llmod.print_to_string().to_string();
        assert!(
            ir.contains("landingpad"),
            "Expected 'landingpad' in IR, got:\n{ir}"
        );
        drop(irb);
    }

    #[test]
    fn resume_terminates_block() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let (func, _entry, then_block, catch_block) = setup_invoke_blocks(&mut irb);

        let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);
        irb.set_personality(func, personality);

        let arg = irb.const_i64(1);
        irb.invoke(func, &[arg], then_block, catch_block, "inv");

        // Build landingpad + resume in catch block.
        irb.position_at_end(catch_block);
        let lp = irb.landingpad(personality, true, "lp");
        assert!(!irb.current_block_terminated());
        irb.resume(lp);
        assert!(irb.current_block_terminated());

        let ir = scx.llmod.print_to_string().to_string();
        assert!(ir.contains("resume"), "Expected 'resume' in IR, got:\n{ir}");
        drop(irb);
    }

    #[test]
    fn full_invoke_landingpad_resume_flow() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();

        // Declare personality.
        let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);

        // Declare a callee that might throw.
        let callee = irb.declare_function("may_throw", &[i64_ty], i64_ty);

        // Build the caller function.
        let caller = irb.declare_function("caller", &[i64_ty], i64_ty);
        irb.set_personality(caller, personality);
        let entry = irb.append_block(caller, "entry");
        let normal = irb.append_block(caller, "normal");
        let unwind = irb.append_block(caller, "unwind");
        irb.set_current_function(caller);

        // Entry: invoke callee → normal or unwind.
        irb.position_at_end(entry);
        let arg = irb.const_i64(42);
        let result = irb.invoke(callee, &[arg], normal, unwind, "result");
        assert!(result.is_some());

        // Normal: return the invoke result.
        irb.position_at_end(normal);
        irb.ret(result.unwrap());

        // Unwind: landingpad + resume.
        irb.position_at_end(unwind);
        let lp = irb.landingpad(personality, true, "lp");
        irb.resume(lp);

        // Verify the complete EH flow in the IR.
        let ir = scx.llmod.print_to_string().to_string();
        assert!(ir.contains("invoke"), "Missing invoke in IR:\n{ir}");
        assert!(ir.contains("landingpad"), "Missing landingpad in IR:\n{ir}");
        assert!(ir.contains("resume"), "Missing resume in IR:\n{ir}");
        assert!(ir.contains("cleanup"), "Missing cleanup flag in IR:\n{ir}");
        assert!(
            ir.contains("to label %normal unwind label %unwind"),
            "Missing invoke branch targets in IR:\n{ir}"
        );
        drop(irb);
    }

    #[test]
    fn set_personality_on_function() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let func = irb.declare_function("personality_test", &[], i64_ty);
        let personality = irb.declare_extern_function("__gxx_personality_v0", &[], None);
        irb.set_personality(func, personality);

        let ir = scx.llmod.print_to_string().to_string();
        assert!(
            ir.contains("personality"),
            "Expected 'personality' in IR, got:\n{ir}"
        );
        drop(irb);
    }

    #[test]
    fn invoke_indirect_produces_invoke() {
        let ctx = Context::create();
        let scx = test_scx(&ctx);
        let mut irb = IrBuilder::new(&scx);

        let i64_ty = irb.i64_type();
        let caller = irb.declare_function("indirect_invoke_test", &[], i64_ty);
        let entry = irb.append_block(caller, "entry");
        let then_block = irb.append_block(caller, "then");
        let catch_block = irb.append_block(caller, "catch");
        irb.set_current_function(caller);
        irb.position_at_end(entry);

        // Get a function pointer to invoke indirectly.
        let target = irb.declare_function("target_fn", &[i64_ty], i64_ty);
        let fn_ptr = irb.get_function_ptr(target);

        let arg = irb.const_i64(7);
        let result = irb.invoke_indirect(
            i64_ty,
            &[i64_ty],
            fn_ptr,
            &[arg],
            then_block,
            catch_block,
            "indirect_inv",
        );
        assert!(result.is_some());
        assert!(irb.current_block_terminated());

        let ir = scx.llmod.print_to_string().to_string();
        assert!(ir.contains("invoke"), "Expected 'invoke' in IR, got:\n{ir}");
        drop(irb);
    }
}
