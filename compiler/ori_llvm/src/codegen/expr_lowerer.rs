//! Expression lowering coordinator for V2 codegen.
//!
//! `ExprLowerer` owns the lowering context (scope, loop state, function ID)
//! and dispatches each `ExprKind` variant to a focused `lower_*` method
//! implemented in separate files. This replaces the monolithic 387-line
//! `Builder::compile_expr` match with independently testable modules.
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
//!   └── lower_constructs.rs   — FunctionSeq, FunctionExp, SelfRef, Await, …
//! ```

use std::cell::Cell;

use ori_ir::{ExprArena, ExprId, ExprKind, Name, Span, StringInterner};
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

/// Coordinates expression lowering from AST (`ExprKind`) to LLVM IR (`ValueId`).
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
    /// The parsed expression tree.
    pub(crate) arena: &'a ExprArena,
    /// Parallel array: `expr_types[expr_id.index()] == Idx` (type of each expr).
    pub(crate) expr_types: &'a [Idx],
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
        arena: &'a ExprArena,
        expr_types: &'a [Idx],
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
            arena,
            expr_types,
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

    /// Get the type index for an expression.
    pub(crate) fn expr_type(&self, id: ExprId) -> Idx {
        self.expr_types
            .get(id.index())
            .copied()
            .unwrap_or(Idx::NONE)
    }

    /// Resolve a `Name` to a string slice via the interner.
    pub(crate) fn resolve_name(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    // -----------------------------------------------------------------------
    // Main dispatch
    // -----------------------------------------------------------------------

    /// Lower an expression to LLVM IR, returning a `ValueId`.
    ///
    /// Returns `None` for expressions that produce no value (e.g., `Unit`,
    /// `Error`, void-returning calls, terminated control flow).
    ///
    /// Every `ExprKind` variant is listed explicitly — no catch-all — so
    /// adding a new variant to the AST causes a compile error here.
    #[allow(clippy::too_many_lines)] // Exhaustive dispatch over all ExprKind variants
    pub fn lower(&mut self, id: ExprId) -> Option<ValueId> {
        if !id.is_valid() {
            return None;
        }

        let expr = self.arena.get_expr(id);

        // Set debug location for this expression so all emitted
        // instructions are tagged with the correct source position.
        if let Some(dc) = self.debug_context {
            if expr.span != Span::DUMMY {
                dc.set_location_from_offset_in_current_scope(
                    self.builder.inkwell_builder(),
                    expr.span.start,
                );
            }
        }

        match &expr.kind {
            // --- Literals & identifiers (lower_literals.rs) ---
            ExprKind::Int(n) => Some(self.lower_int(*n)),
            ExprKind::Float(bits) => Some(self.lower_float(*bits)),
            ExprKind::Bool(b) => Some(self.lower_bool(*b)),
            ExprKind::Char(c) => Some(self.lower_char(*c)),
            ExprKind::String(name) | ExprKind::TemplateFull(name) => self.lower_string(*name),
            ExprKind::Duration { value, unit } => Some(self.lower_duration(*value, *unit)),
            ExprKind::Size { value, unit } => Some(self.lower_size(*value, *unit)),
            ExprKind::Unit => Some(self.lower_unit()),
            ExprKind::Ident(name) => self.lower_ident(*name, id),
            ExprKind::Const(name) => self.lower_const(*name, id),
            ExprKind::FunctionRef(name) => self.lower_function_ref(*name),
            ExprKind::HashLength => self.lower_hash_length(),
            ExprKind::TemplateLiteral { head, parts } => self.lower_template_literal(*head, *parts),

            // --- Operators (lower_operators.rs) ---
            ExprKind::Binary { op, left, right } => self.lower_binary(*op, *left, *right, id),
            ExprKind::Unary { op, operand } => self.lower_unary(*op, *operand, id),
            ExprKind::Cast {
                expr: inner,
                fallible,
                ..
            } => self.lower_cast(*inner, *fallible, id),

            // --- Control flow (lower_control_flow.rs) ---
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if(*cond, *then_branch, *else_branch, id),
            ExprKind::Block { stmts, result } => self.lower_block(*stmts, *result),
            ExprKind::Let {
                pattern,
                init,
                mutable,
                ..
            } => self.lower_let(*pattern, *init, *mutable),
            ExprKind::Loop { body } => self.lower_loop(*body, id),
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => self.lower_for(*binding, *iter, *guard, *body, *is_yield, id),
            ExprKind::Break(value) => self.lower_break(*value),
            ExprKind::Continue(value) => self.lower_continue(*value),
            ExprKind::Assign { target, value } => self.lower_assign(*target, *value),
            ExprKind::Match { scrutinee, arms } => self.lower_match(*scrutinee, *arms, id),

            // --- Error handling (lower_error_handling.rs) ---
            ExprKind::Ok(inner) => self.lower_ok(*inner, id),
            ExprKind::Err(inner) => self.lower_err(*inner, id),
            ExprKind::Some(inner) => self.lower_some(*inner, id),
            ExprKind::None => self.lower_none(id),
            ExprKind::Try(inner) => self.lower_try(*inner, id),

            // --- Collections (lower_collections.rs) ---
            ExprKind::Tuple(range) => self.lower_tuple(*range, id),
            ExprKind::Struct { name, fields } => self.lower_struct(*name, *fields, id),
            ExprKind::StructWithSpread { name, fields } => {
                self.lower_struct_with_spread(*name, *fields, id)
            }
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => self.lower_range(*start, *end, *step, *inclusive),
            ExprKind::Field { receiver, field } => self.lower_field(*receiver, *field),
            ExprKind::Index { receiver, index } => self.lower_index(*receiver, *index),
            ExprKind::List(range) => self.lower_list(*range, id),
            ExprKind::ListWithSpread(elements) => self.lower_list_with_spread(*elements),
            ExprKind::Map(entries) => self.lower_map(*entries, id),
            ExprKind::MapWithSpread(elements) => self.lower_map_with_spread(*elements),

            // --- Calls (lower_calls.rs) ---
            ExprKind::Call { func, args } => self.lower_call(*func, *args),
            ExprKind::CallNamed { func, args } => self.lower_call_named(*func, *args),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(*receiver, *method, *args),
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.lower_method_call_named(*receiver, *method, *args),
            ExprKind::Lambda {
                params,
                ret_ty: _,
                body,
            } => self.lower_lambda(*params, *body, id),

            // --- Constructs (lower_constructs.rs) ---
            ExprKind::FunctionSeq(seq_id) => self.lower_function_seq(*seq_id, id),
            ExprKind::FunctionExp(exp_id) => self.lower_function_exp(*exp_id, id),
            ExprKind::SelfRef => self.lower_self_ref(),
            ExprKind::Await(inner) => self.lower_await(*inner),
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => self.lower_with_capability(*capability, *provider, *body),

            // --- Error placeholder (should not reach codegen) ---
            ExprKind::Error => {
                tracing::warn!("ExprKind::Error reached codegen");
                None
            }
        }
    }
}
