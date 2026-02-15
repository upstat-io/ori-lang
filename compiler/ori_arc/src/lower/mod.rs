//! AST → ARC IR lowering pass.
//!
//! Converts the typed expression tree (implicit control flow) into basic-block
//! ARC IR (explicit control flow). This IR is the foundation for all ARC
//! analysis passes: borrow inference (06.2), RC insertion (07), RC elimination
//! (08), and constructor reuse (09).
//!
//! # Entry Point
//!
//! [`lower_function_can`] takes a canonical IR body and produces an [`ArcFunction`]
//! plus any lambda bodies as additional [`ArcFunction`]s.
//!
//! # Architecture
//!
//! - [`ArcIrBuilder`] — owns the in-progress function, provides block/var
//!   allocation and instruction emission.
//! - [`ArcLowerer`] (in `expr.rs`) — walks the expression tree and calls
//!   builder methods.
//! - [`ArcScope`] (in `scope.rs`) — tracks name→`ArcVarId` bindings with
//!   mutable variable tracking for SSA merge.

mod calls;
mod collections;
mod control_flow;
mod expr;
mod patterns;
pub(crate) mod scope;

use ori_ir::canon::{CanId, CanonResult};
use ori_ir::{Name, Span, StringInterner};
use ori_types::{Idx, Pool};

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind,
};
use crate::Ownership;

pub use self::expr::ArcLowerer;
pub use self::scope::ArcScope;

// Diagnostics

/// Problem encountered during ARC IR lowering.
///
/// These are collected during lowering and reported to the caller.
/// They do not abort lowering — the builder produces a best-effort
/// `ArcFunction` even when problems occur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArcProblem {
    /// An expression kind that is not yet supported for lowering.
    UnsupportedExpr { kind: &'static str, span: Span },
    /// A pattern kind that is not yet supported for lowering.
    UnsupportedPattern { kind: &'static str, span: Span },
    /// An internal error (invariant violation) during lowering.
    InternalError { message: String, span: Span },
}

// BlockBuilder

/// In-progress basic block being constructed.
struct BlockBuilder {
    id: ArcBlockId,
    params: Vec<(ArcVarId, Idx)>,
    body: Vec<ArcInstr>,
    spans: Vec<Option<Span>>,
    terminator: Option<ArcTerminator>,
}

impl BlockBuilder {
    fn new(id: ArcBlockId) -> Self {
        Self {
            id,
            params: Vec::new(),
            body: Vec::new(),
            spans: Vec::new(),
            terminator: None,
        }
    }
}

// ArcIrBuilder

/// Builder for an in-progress ARC IR function.
///
/// Owns block and variable state while the function is being lowered.
/// Consumed by [`finish`](ArcIrBuilder::finish) to produce the final
/// [`ArcFunction`].
///
/// # Design
///
/// Follows the same "position at a block, emit instructions, terminate"
/// pattern as LLVM's `IRBuilder`. The key difference is that ARC IR uses
/// block parameters instead of phi nodes for SSA merge.
pub struct ArcIrBuilder {
    blocks: Vec<BlockBuilder>,
    current_block: ArcBlockId,
    next_var: u32,
    var_types: Vec<Idx>,
}

impl Default for ArcIrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcIrBuilder {
    /// Create a builder with an entry block already allocated.
    pub fn new() -> Self {
        let entry = BlockBuilder::new(ArcBlockId::new(0));
        Self {
            blocks: vec![entry],
            current_block: ArcBlockId::new(0),
            next_var: 0,
            var_types: Vec::new(),
        }
    }

    // Block management

    /// Allocate a new empty block and return its ID.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "block indices never exceed u32"
    )]
    pub fn new_block(&mut self) -> ArcBlockId {
        let id = ArcBlockId::new(self.blocks.len() as u32);
        self.blocks.push(BlockBuilder::new(id));
        id
    }

    /// Set the current insertion point to the given block.
    pub fn position_at(&mut self, block: ArcBlockId) {
        debug_assert!(
            (block.index()) < self.blocks.len(),
            "ArcBlockId {} out of bounds (have {} blocks)",
            block.raw(),
            self.blocks.len(),
        );
        self.current_block = block;
    }

    /// Get the current block being built.
    #[inline]
    pub fn current_block(&self) -> ArcBlockId {
        self.current_block
    }

    /// Check whether the current block already has a terminator.
    #[inline]
    pub fn is_terminated(&self) -> bool {
        self.blocks[self.current_block.index()].terminator.is_some()
    }

    /// Get the entry block (always block 0).
    #[inline]
    pub fn entry_block(&self) -> ArcBlockId {
        ArcBlockId::new(0)
    }

    // Variable allocation

    /// Allocate a fresh variable with the given type.
    pub fn fresh_var(&mut self, ty: Idx) -> ArcVarId {
        let id = ArcVarId::new(self.next_var);
        self.next_var += 1;
        self.var_types.push(ty);
        id
    }

    /// Add a block parameter and return the variable bound to it.
    pub fn add_block_param(&mut self, block: ArcBlockId, ty: Idx) -> ArcVarId {
        let var = self.fresh_var(ty);
        self.blocks[block.index()].params.push((var, ty));
        var
    }

    // Instruction emission

    /// Emit a `Let` instruction binding a value to a fresh variable.
    pub fn emit_let(&mut self, ty: Idx, value: ArcValue, span: Option<Span>) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Let { dst, ty, value });
        block.spans.push(span);
        dst
    }

    /// Emit an `Apply` (direct function call) instruction.
    pub fn emit_apply(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Apply {
            dst,
            ty,
            func,
            args,
        });
        block.spans.push(span);
        dst
    }

    /// Emit an `ApplyIndirect` (closure call) instruction.
    pub fn emit_apply_indirect(
        &mut self,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::ApplyIndirect {
            dst,
            ty,
            closure,
            args,
        });
        block.spans.push(span);
        dst
    }

    /// Emit a `Construct` instruction.
    pub fn emit_construct(
        &mut self,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Construct {
            dst,
            ty,
            ctor,
            args,
        });
        block.spans.push(span);
        dst
    }

    /// Emit a `Project` (field access) instruction.
    pub fn emit_project(
        &mut self,
        ty: Idx,
        value: ArcVarId,
        field: u32,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let block = &mut self.blocks[self.current_block.index()];
        block.body.push(ArcInstr::Project {
            dst,
            ty,
            value,
            field,
        });
        block.spans.push(span);
        dst
    }

    // Invoke (call that may unwind)

    /// Emit an `Invoke` terminator for a function call that may unwind.
    ///
    /// Creates a normal continuation block and an unwind cleanup block.
    /// The current block is terminated with `Invoke`. The builder is
    /// positioned at the normal block on return. The unwind block is
    /// terminated with `Resume` (cleanup blocks will be filled in later
    /// by the RC insertion pass).
    ///
    /// Returns the `dst` variable holding the call result (defined at
    /// the normal block's entry).
    pub fn emit_invoke(
        &mut self,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        span: Option<Span>,
    ) -> ArcVarId {
        let dst = self.fresh_var(ty);
        let normal = self.new_block();
        let unwind = self.new_block();

        // Track the span on the invoking block (one span per instruction,
        // but Invoke is a terminator so we don't push to spans here —
        // terminators don't have span slots in the current design).
        let _ = span;

        self.terminate_invoke(dst, ty, func, args, normal, unwind);

        // Unwind block: initially just Resume. The RC insertion pass
        // (Phase 3C) will add cleanup RcDec instructions before Resume.
        self.position_at(unwind);
        self.terminate_resume();

        // Position at the normal continuation block for subsequent lowering.
        self.position_at(normal);
        dst
    }

    // Terminators

    /// Terminate with `Return`.
    pub fn terminate_return(&mut self, value: ArcVarId) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Return { value });
    }

    /// Terminate with unconditional `Jump`.
    pub fn terminate_jump(&mut self, target: ArcBlockId, args: Vec<ArcVarId>) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Jump { target, args });
    }

    /// Terminate with conditional `Branch`.
    pub fn terminate_branch(
        &mut self,
        cond: ArcVarId,
        then_block: ArcBlockId,
        else_block: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Branch {
            cond,
            then_block,
            else_block,
        });
    }

    /// Terminate with multi-way `Switch`.
    pub fn terminate_switch(
        &mut self,
        scrutinee: ArcVarId,
        cases: Vec<(u64, ArcBlockId)>,
        default: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        });
    }

    /// Terminate with `Invoke` (function call that may unwind).
    ///
    /// The `dst` variable is defined at the `normal` continuation block's
    /// entry, NOT in the current block. The `unwind` block receives control
    /// if the callee unwinds (panics).
    pub fn terminate_invoke(
        &mut self,
        dst: ArcVarId,
        ty: Idx,
        func: Name,
        args: Vec<ArcVarId>,
        normal: ArcBlockId,
        unwind: ArcBlockId,
    ) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Invoke {
            dst,
            ty,
            func,
            args,
            normal,
            unwind,
        });
    }

    /// Terminate with `Resume` (re-raise an unwinding panic).
    pub fn terminate_resume(&mut self) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Resume);
    }

    /// Terminate with `Unreachable`.
    pub fn terminate_unreachable(&mut self) {
        let block = &mut self.blocks[self.current_block.index()];
        debug_assert!(
            block.terminator.is_none(),
            "block {} already terminated",
            self.current_block.raw()
        );
        block.terminator = Some(ArcTerminator::Unreachable);
    }

    // Finalization

    /// Consume the builder and produce a finished [`ArcFunction`].
    ///
    /// Validates that every block has a terminator. Unterminated blocks
    /// get `Unreachable` as a fallback (with a tracing warning).
    pub fn finish(
        mut self,
        name: Name,
        params: Vec<ArcParam>,
        return_type: Idx,
        entry: ArcBlockId,
    ) -> ArcFunction {
        let mut blocks = Vec::with_capacity(self.blocks.len());
        let mut spans = Vec::with_capacity(self.blocks.len());

        for bb in &mut self.blocks {
            if bb.terminator.is_none() {
                tracing::warn!(
                    block = bb.id.raw(),
                    "unterminated block in ARC IR — adding Unreachable"
                );
                bb.terminator = Some(ArcTerminator::Unreachable);
            }

            let terminator = bb.terminator.take().unwrap_or(ArcTerminator::Unreachable);
            let body = std::mem::take(&mut bb.body);
            let block_spans = std::mem::take(&mut bb.spans);
            let block_params = std::mem::take(&mut bb.params);

            blocks.push(ArcBlock {
                id: bb.id,
                params: block_params,
                body,
                terminator,
            });
            spans.push(block_spans);
        }

        ArcFunction {
            name,
            params,
            return_type,
            blocks,
            entry,
            var_types: self.var_types,
            spans,
        }
    }
}

// Public entry point

/// Lower a typed function body from canonical IR into ARC IR.
///
/// This is the canonical-IR entry point, consuming `CanId` + `CanonResult`
/// instead of `ExprId` + `ExprArena`. Returns the lowered function plus
/// any lambda bodies encountered during lowering.
#[expect(
    clippy::too_many_arguments,
    reason = "public API entry point -- a config struct would add unnecessary complexity"
)]
pub fn lower_function_can(
    name: Name,
    params: &[(Name, Idx)],
    return_type: Idx,
    body: CanId,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    problems: &mut Vec<ArcProblem>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let mut builder = ArcIrBuilder::new();
    let mut scope = ArcScope::new();

    // Bind function parameters.
    let mut arc_params = Vec::with_capacity(params.len());
    for &(param_name, param_ty) in params {
        let var = builder.fresh_var(param_ty);
        scope.bind(param_name, var);
        arc_params.push(ArcParam {
            var,
            ty: param_ty,
            ownership: Ownership::Owned, // Refined by borrow inference (06.2).
        });
    }

    let entry = builder.entry_block();
    let mut lambdas = Vec::new();

    // Lower the body expression.
    let mut lowerer = ArcLowerer {
        builder: &mut builder,
        arena: &canon.arena,
        canon,
        interner,
        pool,
        scope,
        loop_ctx: None,
        problems,
        lambdas: &mut lambdas,
    };

    let result_var = lowerer.lower_expr(body);

    // Terminate the entry block (or current block) with Return.
    if !lowerer.builder.is_terminated() {
        lowerer.builder.terminate_return(result_var);
    }

    let func = builder.finish(name, arc_params, return_type, entry);
    (func, lambdas)
}

// Tests

#[cfg(test)]
mod tests;
