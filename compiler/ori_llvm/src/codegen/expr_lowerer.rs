//! Expression lowering coordinator for V2 codegen.
//!
//! `ExprLowerer` owns the lowering context (scope, loop state, function ID)
//! and dispatches each `CanExpr` variant to a focused `lower_*` method
//! implemented in separate files.
//!
//! # Architecture
//!
//! ```text
//! ExprLowerer
//!   ├── lower_literals.rs     — Int, Float, Bool, String, Ident, …
//!   ├── lower_operators.rs    — Binary, Unary, Cast
//!   ├── lower_control_flow.rs — If, Loop, For, Block, Break, Continue, …
//!   ├── lower_error_handling.rs — Ok, Err, Some, None, Try
//!   ├── lower_collections.rs  — List, Map, Tuple, Struct, Range, Field, Index
//!   ├── lower_calls.rs        — Call, MethodCall, Lambda
//!   └── lower_constructs.rs   — FunctionExp, SelfRef, Await, …
//! ```

use std::cell::Cell;

use ori_ir::canon::{CanExpr, CanId, CanonResult};
use ori_ir::{Name, Span, StringInterner};
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

use crate::aot::debug::DebugContext;

use super::abi::FunctionAbi;
use super::ir_builder::IrBuilder;
use super::scope::Scope;
use super::type_info::{TypeInfoStore, TypeLayoutResolver};
use super::value_id::{BlockId, FunctionId, LLVMTypeId, ValueId};

// ---------------------------------------------------------------------------
// LoopContext
// ---------------------------------------------------------------------------

/// Active loop state for break/continue lowering.
///
/// The deferred-phi pattern: break values are collected as `(ValueId, BlockId)`
/// pairs during lowering. After the loop body is complete, a single phi node
/// merges all break values at the exit block.
pub(crate) struct LoopContext {
    /// Block to branch to on `break`.
    pub exit_block: BlockId,
    /// Block to branch to on `continue`.
    ///
    /// For `for`-loops this is the **latch** block (increment + back-edge),
    /// not the header. This ensures the loop variable is updated before
    /// the next iteration check.
    pub continue_block: BlockId,
    /// Accumulated `(value, source_block)` pairs from `break expr`.
    pub break_values: Vec<(ValueId, BlockId)>,
}

// ---------------------------------------------------------------------------
// ExprLowerer
// ---------------------------------------------------------------------------

/// Coordinates expression lowering from canonical IR (`CanExpr`) to LLVM IR (`ValueId`).
///
/// Three lifetimes:
/// - `'a`: borrow duration of the lowerer's dependencies
/// - `'scx`: `SimpleCx` borrow (LLVM module + type shortcuts)
/// - `'ctx`: LLVM context lifetime (from `Context::create()`)
pub struct ExprLowerer<'a, 'scx, 'ctx, 'tcx> {
    /// ID-based LLVM instruction builder.
    pub(crate) builder: &'a mut IrBuilder<'scx, 'ctx>,
    /// Type info cache (`Idx` → `TypeInfo`).
    pub(crate) type_info: &'a TypeInfoStore<'tcx>,
    /// Recursive type layout resolver (`Idx` → `BasicTypeEnum`).
    ///
    /// Unlike `TypeInfo::storage_type()` which returns placeholder types
    /// for compound types (all-i64 fields), this resolver recursively
    /// resolves inner types with cycle detection and caching.
    pub(crate) type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
    /// Current lexical scope (owned; swapped via `mem::replace` for blocks).
    pub(crate) scope: Scope,
    /// Canonical IR — arena, constants, decision trees.
    pub(crate) canon: &'a CanonResult,
    /// String interner for `Name` → `&str` resolution.
    pub(crate) interner: &'a StringInterner,
    /// Type pool for structural queries (used by future lowering extensions).
    #[allow(dead_code)]
    pub(crate) pool: &'a Pool,
    /// The LLVM function currently being compiled.
    pub(crate) current_function: FunctionId,
    /// Declared functions: `Name` → (`FunctionId`, ABI). Used by call lowering to
    /// determine sret vs direct return and calling convention.
    pub(crate) functions: &'a FxHashMap<Name, (FunctionId, FunctionAbi)>,
    /// Type-qualified method map: `(type_name, method_name)` → (`FunctionId`, ABI).
    ///
    /// Enables same-name methods on different types (e.g., `Point.distance` vs
    /// `Line.distance`). Checked before `functions` in method call dispatch.
    pub(crate) method_functions: &'a FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
    /// Maps receiver type `Idx` → type `Name` for method dispatch resolution.
    pub(crate) type_idx_to_name: &'a FxHashMap<Idx, Name>,
    /// Active loop context for break/continue (None outside loops).
    pub(crate) loop_ctx: Option<LoopContext>,
    /// Module-wide lambda counter for unique lambda function names.
    ///
    /// Shared via `&Cell<u32>` so that nested lambdas (which create new
    /// `ExprLowerer` contexts internally) still get unique names. Owned
    /// by `FunctionCompiler`, passed by reference here.
    pub(crate) lambda_counter: &'a Cell<u32>,
    /// Module path for name mangling (e.g., "", "math").
    pub(crate) module_path: &'a str,
    /// Debug info context (None for JIT, Some for AOT with debug info enabled).
    pub(crate) debug_context: Option<&'a DebugContext<'ctx>>,
}

impl<'a, 'scx: 'ctx, 'ctx, 'tcx> ExprLowerer<'a, 'scx, 'ctx, 'tcx> {
    /// Create a new expression lowerer.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        builder: &'a mut IrBuilder<'scx, 'ctx>,
        type_info: &'a TypeInfoStore<'tcx>,
        type_resolver: &'a TypeLayoutResolver<'a, 'scx, 'ctx>,
        scope: Scope,
        canon: &'a CanonResult,
        interner: &'a StringInterner,
        pool: &'a Pool,
        current_function: FunctionId,
        functions: &'a FxHashMap<Name, (FunctionId, FunctionAbi)>,
        method_functions: &'a FxHashMap<(Name, Name), (FunctionId, FunctionAbi)>,
        type_idx_to_name: &'a FxHashMap<Idx, Name>,
        lambda_counter: &'a Cell<u32>,
        module_path: &'a str,
        debug_context: Option<&'a DebugContext<'ctx>>,
    ) -> Self {
        Self {
            builder,
            type_info,
            type_resolver,
            scope,
            canon,
            interner,
            pool,
            current_function,
            functions,
            method_functions,
            type_idx_to_name,
            loop_ctx: None,
            lambda_counter,
            module_path,
            debug_context,
        }
    }

    /// Resolve an `Idx` to an `LLVMTypeId` via the `TypeLayoutResolver`.
    ///
    /// Uses recursive resolution with cycle detection and caching,
    /// producing properly typed LLVM structs (not placeholder all-i64).
    pub(crate) fn resolve_type(&mut self, idx: Idx) -> LLVMTypeId {
        let llvm_ty = self.type_resolver.resolve(idx);
        self.builder.register_type(llvm_ty)
    }

    /// Get the type index for a canonical expression.
    pub(crate) fn expr_type(&self, id: CanId) -> Idx {
        if !id.is_valid() {
            return Idx::NONE;
        }
        Idx::from_raw(self.canon.arena.ty(id).raw())
    }

    /// Resolve a `Name` to a string slice via the interner.
    pub(crate) fn resolve_name(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    // -----------------------------------------------------------------------
    // Main dispatch
    // -----------------------------------------------------------------------

    /// Lower a canonical expression to LLVM IR, returning a `ValueId`.
    ///
    /// Returns `None` for expressions that produce no value (e.g., `Unit`,
    /// `Error`, void-returning calls, terminated control flow).
    ///
    /// Every `CanExpr` variant is listed explicitly — no catch-all — so
    /// adding a new variant to the canonical IR causes a compile error here.
    #[allow(clippy::too_many_lines)] // Exhaustive dispatch over all CanExpr variants
    pub fn lower(&mut self, id: CanId) -> Option<ValueId> {
        if !id.is_valid() {
            return None;
        }

        let kind = *self.canon.arena.kind(id);
        let span = self.canon.arena.span(id);

        // Set debug location for this expression so all emitted
        // instructions are tagged with the correct source position.
        if let Some(dc) = self.debug_context {
            if span != Span::DUMMY {
                dc.set_location_from_offset_in_current_scope(
                    self.builder.inkwell_builder(),
                    span.start,
                );
            }
        }

        match kind {
            // --- Literals & identifiers (lower_literals.rs) ---
            CanExpr::Int(n) => Some(self.lower_int_typed(n, id)),
            CanExpr::Float(bits) => Some(self.lower_float(bits)),
            CanExpr::Bool(b) => Some(self.lower_bool(b)),
            CanExpr::Char(c) => Some(self.lower_char(c)),
            CanExpr::Str(name) => self.lower_string(name),
            CanExpr::Duration { value, unit } => Some(self.lower_duration(value, unit)),
            CanExpr::Size { value, unit } => Some(self.lower_size(value, unit)),
            CanExpr::Unit | CanExpr::HashLength => Some(self.lower_unit()),
            CanExpr::Ident(name) | CanExpr::TypeRef(name) => self.lower_ident(name, id),
            CanExpr::Const(name) => self.lower_const(name, id),
            CanExpr::FunctionRef(name) => self.lower_function_ref(name),
            CanExpr::Constant(const_id) => self.lower_constant(const_id, id),

            // --- Operators (lower_operators.rs) ---
            CanExpr::Binary { op, left, right } => self.lower_binary(op, left, right, id),
            CanExpr::Unary { op, operand } => self.lower_unary(op, operand, id),
            CanExpr::Cast {
                expr: inner,
                fallible,
                ..
            } => self.lower_cast(inner, fallible, id),

            // --- Control flow (lower_control_flow.rs) ---
            CanExpr::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if(cond, then_branch, else_branch, id),
            CanExpr::Block { stmts, result } => self.lower_block(stmts, result),
            CanExpr::Let {
                pattern,
                init,
                mutable,
            } => self.lower_let(pattern, init, mutable),
            CanExpr::Loop { body } => self.lower_loop(body, id),
            CanExpr::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => self.lower_for(binding, iter, guard, body, is_yield, id),
            CanExpr::Break(value) => self.lower_break(value),
            CanExpr::Continue(value) => self.lower_continue(value),
            CanExpr::Assign { target, value } => self.lower_assign(target, value),
            CanExpr::Match {
                scrutinee,
                decision_tree,
                arms,
            } => self.lower_match(scrutinee, decision_tree, arms, id),

            // --- Error handling (lower_error_handling.rs) ---
            CanExpr::Ok(inner) => self.lower_ok(inner, id),
            CanExpr::Err(inner) => self.lower_err(inner, id),
            CanExpr::Some(inner) => self.lower_some(inner, id),
            CanExpr::None => self.lower_none(id),
            CanExpr::Try(inner) => self.lower_try(inner, id),

            // --- Collections (lower_collections.rs) ---
            CanExpr::Tuple(range) => self.lower_tuple(range, id),
            CanExpr::Struct { name, fields } => self.lower_struct(name, fields, id),
            CanExpr::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range(start, end, step, inclusive),
            CanExpr::Field { receiver, field } => self.lower_field(receiver, field),
            CanExpr::Index { receiver, index } => self.lower_index(receiver, index),
            CanExpr::List(range) => self.lower_list(range, id),
            CanExpr::Map(entries) => self.lower_map(entries, id),

            // --- Calls (lower_calls.rs) ---
            CanExpr::Call { func, args } => self.lower_call(func, args),
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(receiver, method, args),
            CanExpr::Lambda { params, body } => self.lower_lambda(params, body, id),

            // --- Constructs (lower_constructs.rs) ---
            CanExpr::FunctionExp { kind, props } => self.lower_function_exp(kind, props, id),
            CanExpr::SelfRef => self.lower_self_ref(),
            CanExpr::Await(inner) => self.lower_await(inner),
            CanExpr::WithCapability {
                capability,
                provider,
                body,
            } => self.lower_with_capability(capability, provider, body),

            // --- Error placeholder (should not reach codegen) ---
            CanExpr::Error => {
                tracing::warn!("CanExpr::Error reached codegen");
                None
            }
        }
    }
}
