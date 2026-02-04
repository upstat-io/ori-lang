//! Compilation context for expression compilation.
//!
//! Bundles the commonly-threaded parameters (`arena`, `expr_types`, `locals`, `loop_ctx`)
//! into a single struct, reducing parameter count from 5+ to 2 (ctx + function).
//!
//! # Design Note
//!
//! This module provides `CompileCtx` as an optional convenience struct. The main
//! `compile_expr` function and its helpers continue to use explicit parameters
//! following established codegen conventions (see `rustc_codegen_llvm`).
//!
//! Using explicit parameters has advantages:
//! - Clear about what each function needs
//! - Simpler lifetime handling without reborrowing
//! - Matches Rust compiler conventions
//!
//! `CompileCtx` is available for users who prefer bundled parameters.

use ori_ir::{ExprArena, TypeId};

use crate::builder::Locals;
use crate::LoopContext;

/// Compilation context passed through expression compilation.
///
/// Bundles arena, type information, local variables, and loop context
/// to reduce parameter threading from 5 params to 1.
///
/// # Lifetime Parameters
/// - `'a`: Lifetime of the arena and `expr_types` references
/// - `'ll`: LLVM context lifetime
pub struct CompileCtx<'a, 'll> {
    /// The expression arena containing all AST nodes.
    pub arena: &'a ExprArena,
    /// Type of each expression (indexed by `ExprId`).
    pub expr_types: &'a [TypeId],
    /// Local variable bindings (mutable for let bindings).
    pub locals: &'a mut Locals<'ll>,
    /// Current loop context for break/continue (if inside a loop).
    pub loop_ctx: Option<&'a LoopContext<'ll>>,
}

impl<'a, 'll> CompileCtx<'a, 'll> {
    /// Create a new compilation context.
    #[inline]
    pub fn new(
        arena: &'a ExprArena,
        expr_types: &'a [TypeId],
        locals: &'a mut Locals<'ll>,
        loop_ctx: Option<&'a LoopContext<'ll>>,
    ) -> Self {
        Self {
            arena,
            expr_types,
            locals,
            loop_ctx,
        }
    }

    /// Create a context without loop context (for initial compilation).
    #[inline]
    pub fn without_loop(
        arena: &'a ExprArena,
        expr_types: &'a [TypeId],
        locals: &'a mut Locals<'ll>,
    ) -> Self {
        Self {
            arena,
            expr_types,
            locals,
            loop_ctx: None,
        }
    }

    /// Reborrow the context with a different loop context.
    ///
    /// This is useful when entering/exiting loops.
    #[inline]
    pub fn with_loop_ctx<'b>(
        &'b mut self,
        loop_ctx: Option<&'b LoopContext<'ll>>,
    ) -> CompileCtx<'b, 'll>
    where
        'a: 'b,
    {
        CompileCtx {
            arena: self.arena,
            expr_types: self.expr_types,
            locals: self.locals,
            loop_ctx,
        }
    }

    /// Reborrow the context, keeping the same loop context.
    ///
    /// Use this when you need to pass the context to a nested call
    /// without consuming the original reference.
    #[inline]
    pub fn reborrow<'b>(&'b mut self) -> CompileCtx<'b, 'll>
    where
        'a: 'b,
    {
        CompileCtx {
            arena: self.arena,
            expr_types: self.expr_types,
            locals: self.locals,
            loop_ctx: self.loop_ctx,
        }
    }
}
