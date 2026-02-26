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
//! | Comparisons | `comparisons` |
//! | Conversions | `conversions` |
//! | Control flow | `control_flow` |
//! | Aggregates | `aggregates` |
//! | Calls | `calls` |
//! | Phi / Types / Blocks | `phi_types_blocks` |

mod aggregates;
mod arithmetic;
mod calls;
mod comparisons;
mod constants;
mod control_flow;
mod conversions;
mod memory;
mod phi_types_blocks;

use std::cell::{Cell, RefCell};

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder as InkwellBuilder;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{BasicValueEnum, FunctionValue};
use rustc_hash::FxHashMap;

use crate::context::SimpleCx;

use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueArena, ValueId};

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
            codegen_error_descriptions: RefCell::new(Vec::new()),
            runtime_cache: FxHashMap::default(),
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

    /// Get or lazily declare a runtime function by name.
    ///
    /// On first call for a given name, declares the function in the LLVM
    /// module with the correct signature and attributes (from `RT_FUNCTIONS`).
    /// Subsequent calls return the cached `FunctionId`.
    ///
    /// # Panics
    ///
    /// Panics if `name` is not a known runtime function.
    pub fn runtime_fn(&mut self, name: &'static str) -> FunctionId {
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
    pub fn try_runtime_fn(&mut self, name: &str) -> Option<FunctionId> {
        if let Some(&id) = self.runtime_cache.get(name) {
            return Some(id);
        }
        let (static_name, id) = super::runtime_decl::try_declare_single(self, name)?;
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
