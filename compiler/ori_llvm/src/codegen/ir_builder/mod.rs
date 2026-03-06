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
//! | Category | Module |
//! |----------|--------|
//! | Constants | `constants` |
//! | Memory | `memory` |
//! | Arithmetic | `arithmetic` |
//! | Checked arithmetic | `checked_ops` |
//! | Comparisons | `comparisons` |
//! | Conversions | `conversions` |
//! | Control flow | `control_flow` |
//! | Aggregates | `aggregates` |
//! | Calls | `calls` |
//! | Invoke / EH | `invoke` |
//! | Attributes | `attributes` |
//! | Phi / Types / Blocks | `phi_types_blocks` |

mod aggregates;
mod arithmetic;
mod attributes;
mod calls;
mod checked_ops;
mod comparisons;
mod constants;
mod control_flow;
mod conversions;
mod invoke;
mod memory;
mod phi_types_blocks;
pub(crate) mod seh;

use std::cell::{Cell, RefCell};

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder as InkwellBuilder;
use inkwell::targets::TargetData;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, CallSiteValue, FunctionValue};
use rustc_hash::FxHashMap;

use crate::context::SimpleCx;

use super::eh_model::EhModel;
use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueArena, ValueId};

/// Whether the LLVM module is being compiled for JIT or AOT execution.
///
/// Used to guard `runtime_fn()` / `try_runtime_fn()` against using
/// AOT-only runtime functions in JIT mode. This prevents silent symbol
/// resolution failures at MCJIT relocation time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompilationMode {
    Jit,
    Aot,
}

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
    pub(super) builder: InkwellBuilder<'ctx>,
    /// Shared LLVM context for type creation.
    pub(super) scx: &'scx SimpleCx<'ctx>,
    /// Arena storing all LLVM values behind IDs.
    pub(super) arena: ValueArena<'ctx>,
    /// Currently-active function (set by `set_current_function`).
    pub(super) current_function: Option<FunctionId>,
    /// Currently-active basic block (tracked for save/restore).
    pub(super) current_block: Option<BlockId>,
    /// Count of type-mismatch errors during IR construction.
    ///
    /// Incremented by defensive fallback methods (e.g., `build_struct` on non-struct,
    /// `icmp_impl` on non-int). When > 0, the generated IR is malformed and must
    /// NOT be passed to LLVM's JIT — doing so causes heap corruption (SIGABRT).
    /// The evaluator checks this after compilation to bail out early.
    pub(super) codegen_errors: Cell<u32>,
    /// Descriptive messages for each codegen error (for diagnostics).
    pub(super) codegen_error_descriptions: RefCell<Vec<String>>,
    /// Cache: runtime function name → already-declared `FunctionId`.
    ///
    /// Avoids redundant LLVM module lookups and arena pushes for
    /// runtime functions. Populated on first use by `runtime_fn()`.
    runtime_cache: FxHashMap<&'static str, FunctionId>,
    /// Last emitted `call` or `invoke` instruction.
    ///
    /// Stored after each `call()` / `invoke()` to allow adding per-call-site
    /// attributes (e.g., `noalias` on a specific parameter for `StaticUnique`
    /// COW operations). Updated on every call; `None` before any call.
    pub(super) last_call_site: Option<CallSiteValue<'ctx>>,
    /// Whether this builder is compiling for JIT or AOT.
    mode: CompilationMode,
    /// Exception handling model (Itanium vs SEH).
    eh_model: EhModel,
    /// Cached `TargetData` for ABI alignment queries.
    ///
    /// Created lazily from the module's `DataLayout` string on first use.
    /// Used by `load()` and `store()` to set correct alignment on every
    /// memory instruction, ensuring correctness on strict-alignment
    /// architectures (ARM, RISC-V).
    target_data: Option<TargetData>,
    /// Cache: string content → already-emitted global string pointer.
    ///
    /// Deduplicates identical string constants (e.g., overflow panic messages)
    /// so each unique string is emitted as a single LLVM global per module.
    global_strings: FxHashMap<String, ValueId>,
    /// CSE cache for checked arithmetic intrinsics.
    ///
    /// Maps `(intrinsic_name, lhs, rhs)` to the already-computed result
    /// `ValueId`. Operands are normalized via [`checked_ops::CseOperand`]
    /// so that identical constants match regardless of `ValueId`.
    /// Scoped per ARC block — cleared at each ARC-block-boundary
    /// transition via [`Self::clear_cse_cache`]. NOT cleared by internal
    /// `position_at_end` calls within `emit_checked_binop` (which creates
    /// panic/continue blocks as part of a single logical operation).
    cse_cache: FxHashMap<
        (
            &'static str,
            checked_ops::CseOperand,
            checked_ops::CseOperand,
        ),
        ValueId,
    >,
}

impl<'scx, 'ctx> IrBuilder<'scx, 'ctx> {
    /// Create a new `IrBuilder` for AOT compilation (default).
    pub fn new(scx: &'scx SimpleCx<'ctx>) -> Self {
        Self::with_mode(scx, CompilationMode::Aot)
    }

    /// Create a new `IrBuilder` for JIT compilation.
    ///
    /// Runtime function calls are guarded: only functions marked
    /// `jit_allowed: true` in `RT_FUNCTIONS` can be used.
    pub fn new_jit(scx: &'scx SimpleCx<'ctx>) -> Self {
        Self::with_mode(scx, CompilationMode::Jit)
    }

    /// Create a new `IrBuilder` for AOT compilation with an explicit EH model.
    ///
    /// Used by the AOT pipeline where the target triple is known. The EH model
    /// determines whether Itanium (`landingpad`/`resume`) or SEH
    /// (`catchswitch`/`catchpad`/`cleanuppad`) instructions are emitted.
    pub fn new_aot(scx: &'scx SimpleCx<'ctx>, eh_model: EhModel) -> Self {
        Self::with_mode_and_eh(scx, CompilationMode::Aot, eh_model)
    }

    fn with_mode(scx: &'scx SimpleCx<'ctx>, mode: CompilationMode) -> Self {
        Self::with_mode_and_eh(scx, mode, EhModel::Itanium)
    }

    fn with_mode_and_eh(
        scx: &'scx SimpleCx<'ctx>,
        mode: CompilationMode,
        eh_model: EhModel,
    ) -> Self {
        let builder = scx.llcx.create_builder();
        Self {
            builder,
            scx,
            arena: ValueArena::new(),
            current_function: None,
            current_block: None,
            codegen_errors: Cell::new(0),
            codegen_error_descriptions: RefCell::new(Vec::new()),
            runtime_cache: FxHashMap::default(),
            last_call_site: None,
            mode,
            eh_model,
            target_data: None,
            global_strings: FxHashMap::default(),
            cse_cache: FxHashMap::default(),
        }
    }

    /// Access the underlying `SimpleCx` for direct LLVM context operations.
    #[inline]
    pub fn scx(&self) -> &'scx SimpleCx<'ctx> {
        self.scx
    }

    /// The exception handling model for this builder.
    #[inline]
    pub fn eh_model(&self) -> EhModel {
        self.eh_model
    }

    /// Query the ABI alignment (in bytes) for an LLVM type.
    ///
    /// Uses the module's `DataLayout` to determine the correct alignment
    /// for the active target. Returns the **ABI alignment** (not preferred),
    /// which is the minimum alignment guaranteed by the target — safe for
    /// any load/store. Falls back to 8 if no `DataLayout` is available.
    ///
    /// The `TargetData` is created lazily on first call and cached.
    pub fn abi_alignment(&mut self, ty: LLVMTypeId) -> u32 {
        // Lazily initialize TargetData from the module's DataLayout string.
        if self.target_data.is_none() {
            let dl = self.scx.llmod.get_data_layout();
            let dl_str = dl.as_str().to_string_lossy();
            if dl_str.is_empty() {
                // JIT path: module may not have DataLayout set.
                // Use x86-64 default (the only supported JIT host).
                self.target_data = Some(TargetData::create(
                    "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
                ));
            } else {
                self.target_data = Some(TargetData::create(&dl_str));
            }
        }
        let td = self
            .target_data
            .as_ref()
            .expect("target_data initialized above");
        let llvm_ty = self.arena.get_type(ty);
        td.get_abi_alignment(&llvm_ty)
    }

    /// Record a type-mismatch error during IR construction.
    ///
    /// Called by defensive fallback methods when they detect a type mismatch
    /// that would normally cause a panic. The generated IR is malformed and
    /// must not be JIT-compiled.
    pub(crate) fn record_codegen_error(&self) {
        self.codegen_errors.set(self.codegen_errors.get() + 1);
    }

    /// Record a codegen error with a descriptive message for diagnostics.
    pub(crate) fn record_codegen_error_with_msg(&self, msg: impl Into<String>) {
        self.codegen_errors.set(self.codegen_errors.get() + 1);
        self.codegen_error_descriptions
            .borrow_mut()
            .push(msg.into());
    }

    /// Descriptive messages for codegen errors (if any were recorded with messages).
    pub fn codegen_error_descriptions(&self) -> Vec<String> {
        self.codegen_error_descriptions.borrow().clone()
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
    /// Used by `ArcIrEmitter` to bail out early and avoid
    /// cascading type mismatches that corrupt LLVM's internal state.
    pub fn has_codegen_errors(&self) -> bool {
        self.codegen_errors.get() > 0
    }

    /// Clear the CSE cache for checked arithmetic intrinsics.
    ///
    /// Called by `ArcIrEmitter` at each ARC-block-boundary transition.
    /// NOT called from internal `position_at_end` calls within
    /// `emit_checked_binop`, which creates panic/continue blocks as part
    /// of a single logical operation.
    #[inline]
    pub fn clear_cse_cache(&mut self) {
        self.cse_cache.clear();
    }

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

    /// Check if a value is a single-word type (fits in one i64 slot).
    ///
    /// Returns `true` for `i64`, `i32`, `i16`, `i8`, `i1`, `f64`, `ptr` —
    /// all types that occupy at most 8 bytes. Returns `false` for structs,
    /// arrays, and other multi-word aggregates.
    ///
    /// Used by variant construction to determine if the `insertvalue` chain
    /// optimization is safe: only single-word fields can be inserted into
    /// the `[M x i64]` payload array by value.
    pub fn is_single_word(&self, id: ValueId) -> bool {
        let val = self.arena.get_value(id);
        matches!(
            val,
            BasicValueEnum::IntValue(_)
                | BasicValueEnum::FloatValue(_)
                | BasicValueEnum::PointerValue(_)
        )
    }

    /// Check if an LLVM type is a single-slot scalar (fits in one i64 payload slot).
    ///
    /// Returns `true` for integer, float, and pointer types. Returns `false`
    /// for struct, array, and vector types (multi-word aggregates).
    ///
    /// Used by enum payload extraction to decide between the `extractvalue`
    /// path (scalar fields) and the alloca+GEP+load path (multi-word fields).
    pub fn is_single_slot_type(&self, ty: LLVMTypeId) -> bool {
        let raw = self.arena.get_type(ty);
        matches!(
            raw,
            BasicTypeEnum::IntType(_) | BasicTypeEnum::FloatType(_) | BasicTypeEnum::PointerType(_)
        )
    }

    /// Check if a value is a struct (SSA aggregate), not a pointer or scalar.
    pub fn is_struct_value(&self, val: ValueId) -> bool {
        matches!(self.arena.get_value(val), BasicValueEnum::StructValue(_))
    }

    /// Reinterpret raw `i64` bits as the target type.
    ///
    /// Enum payloads store all fields as `i64` in a `[N x i64]` array.
    /// After `extractvalue`, we get the raw `i64` and need to reinterpret
    /// the bits as the actual field type:
    /// - `i64 → i64`: identity (no-op)
    /// - `i64 → double`: `bitcast`
    /// - `i64 → i1/i8/i32`: `trunc`
    /// - `i64 → ptr`: `inttoptr`
    pub fn reinterpret_from_i64(
        &mut self,
        val: ValueId,
        target_ty: LLVMTypeId,
        name: &str,
    ) -> ValueId {
        let raw = self.arena.get_type(target_ty);
        match raw {
            BasicTypeEnum::IntType(it) => {
                if it.get_bit_width() == 64 {
                    val
                } else {
                    self.trunc(val, target_ty, name)
                }
            }
            BasicTypeEnum::FloatType(_) => self.bitcast(val, target_ty, name),
            BasicTypeEnum::PointerType(_) => self.int_to_ptr(val, name),
            _ => {
                tracing::error!(
                    ?raw,
                    "reinterpret_from_i64: unexpected non-scalar target type"
                );
                self.record_codegen_error();
                val
            }
        }
    }

    /// Check if a struct type's field at the given index is an array type.
    ///
    /// Used by variant construction to detect enum payload layout:
    /// - `{ i64, [M x i64] }` (user-defined enums) has an array at field 1
    /// - `{ i64, T }` (Option/Result) has a direct type at field 1
    ///
    /// Returns `false` if the type is not a struct or the field index is
    /// out of bounds.
    pub fn is_struct_field_array(&self, ty: LLVMTypeId, field_index: u32) -> bool {
        let llvm_ty = self.arena.get_type(ty);
        let BasicTypeEnum::StructType(st) = llvm_ty else {
            return false;
        };
        if field_index >= st.count_fields() {
            return false;
        }
        matches!(
            st.get_field_type_at_index(field_index),
            Some(BasicTypeEnum::ArrayType(_))
        )
    }

    /// Check if a value's LLVM type matches a struct field's type at the given index.
    ///
    /// Used by variant construction to determine if `insertvalue` is type-safe
    /// for Option/Result layouts where the payload slot type may differ from
    /// the value being stored (e.g., `Ok(42)` stores i64 into a `{ i64, i64, ptr }` slot).
    pub fn value_type_matches_struct_field(
        &self,
        struct_ty: LLVMTypeId,
        field_index: u32,
        val: ValueId,
    ) -> bool {
        let llvm_ty = self.arena.get_type(struct_ty);
        let BasicTypeEnum::StructType(st) = llvm_ty else {
            return false;
        };
        let Some(field_ty) = st.get_field_type_at_index(field_index) else {
            return false;
        };
        let val_ty = self.arena.get_value(val).get_type();
        field_ty == val_ty
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

    /// Get or lazily declare a runtime function by name.
    ///
    /// On first call for a given name, declares the function in the LLVM
    /// module with the correct signature and attributes (from `RT_FUNCTIONS`).
    /// Subsequent calls return the cached `FunctionId`.
    ///
    /// # Panics
    ///
    /// Panics if `name` is not a known runtime function, or if used in JIT
    /// mode with a function not marked `jit_allowed` in `RT_FUNCTIONS`.
    pub fn runtime_fn(&mut self, name: &'static str) -> FunctionId {
        if self.mode == CompilationMode::Jit {
            assert!(
                super::runtime_decl::runtime_functions::is_jit_allowed(name),
                "runtime function `{name}` used in JIT mode but not marked \
                 jit_allowed in RT_FUNCTIONS"
            );
        }
        if let Some(&id) = self.runtime_cache.get(name) {
            return id;
        }
        let id = super::runtime_decl::declare_single(self, name);
        self.runtime_cache.insert(name, id);
        id
    }

    /// Try to get or lazily declare a runtime function by name.
    ///
    /// Unlike [`runtime_fn`](Self::runtime_fn), accepts `&str` (not just
    /// `&'static str`) and returns `None` for unknown names instead of
    /// panicking. Used by `ArcIrEmitter`'s fallback resolution path where
    /// the callee name comes from ARC IR and may or may not be a runtime
    /// function.
    ///
    /// In JIT mode, panics if the resolved function is not `jit_allowed`.
    pub fn try_runtime_fn(&mut self, name: &str) -> Option<FunctionId> {
        if let Some(&id) = self.runtime_cache.get(name) {
            return Some(id);
        }
        let (static_name, id) = super::runtime_decl::try_declare_single(self, name)?;
        if self.mode == CompilationMode::Jit {
            assert!(
                super::runtime_decl::runtime_functions::is_jit_allowed(static_name),
                "runtime function `{static_name}` used in JIT mode but not marked \
                 jit_allowed in RT_FUNCTIONS"
            );
        }
        self.runtime_cache.insert(static_name, id);
        Some(id)
    }
}

#[cfg(test)]
#[allow(
    clippy::approx_constant,
    clippy::doc_markdown,
    reason = "test code — approximate constants are intentional, doc style relaxed"
)]
mod tests;
