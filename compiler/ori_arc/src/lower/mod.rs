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

mod builder;
mod calls;
mod collections;
mod constructs;
mod control_flow;
mod expr;
mod patterns;
pub(crate) mod scope;

use ori_ir::canon::{CanId, CanonResult};
use ori_ir::{Name, Span, StringInterner};
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashMap;

use crate::classify::ArcClassifier;
use crate::ir::{self, ArcFunction, ArcParam};
use crate::Ownership;

pub(crate) use self::builder::ArcIrBuilder;
pub(crate) use self::expr::ArcLowerer;
pub(crate) use self::scope::ArcScope;

// Variant constructor lookup

/// Maps variant name → `(enum_name, variant_index, field_count)`.
///
/// Built once per function from the [`Pool`]'s enum type data, then shared
/// by reference with the expression lowerer and any inner lambda lowerers.
pub(crate) type VariantCtors = FxHashMap<Name, (Name, u32, usize)>;

/// Scan the pool for all enum types and build a reverse lookup map
/// from variant name to its parent enum info.
#[expect(
    clippy::cast_possible_truncation,
    reason = "pool indices and variant counts never exceed u32"
)]
fn build_variant_ctors(pool: &Pool) -> VariantCtors {
    let mut map = VariantCtors::default();
    for raw in 0..pool.len() as u32 {
        let idx = Idx::from_raw(raw);
        if pool.tag(idx) == Tag::Enum {
            let enum_name = pool.enum_name(idx);
            for (vi, (vname, fields)) in pool.enum_variants(idx).into_iter().enumerate() {
                map.insert(vname, (enum_name, vi as u32, fields.len()));
            }
        }
    }
    map
}

// Diagnostics

/// Problem encountered during ARC IR lowering.
///
/// These are collected during lowering and reported to the caller.
/// They do not abort lowering — the builder produces a best-effort
/// `ArcFunction` even when problems occur.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArcProblem {
    /// A pattern kind that is not yet supported for lowering.
    UnsupportedPattern { kind: &'static str, span: Span },
    /// An internal error (invariant violation) during lowering.
    InternalError { message: String, span: Span },
    /// Function annotated `#fbip` has missed reuse opportunities.
    FbipViolation {
        func_name: String,
        missed_count: usize,
        achieved_count: usize,
        span: Span,
    },
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
#[expect(
    clippy::implicit_hasher,
    reason = "always called with FxHashMap internally"
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
    is_fbip: bool,
    type_subst: Option<&rustc_hash::FxHashMap<Idx, Idx>>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let fn_name = interner.lookup(name);
    tracing::debug!(
        name = fn_name,
        params = params.len(),
        "lower_function_can: enter"
    );

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
        tracing::trace!(
            param = interner.lookup(param_name),
            var = var.raw(),
            "lower_function_can: bind param"
        );
    }

    let entry = builder.entry_block();
    let mut lambdas = Vec::new();
    let variant_ctors = build_variant_ctors(pool);

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
        hash_length: None,
        block_let_names: rustc_hash::FxHashSet::default(),
        func_name: name,
        variant_ctors: &variant_ctors,
        type_subst,
        return_type,
    };

    let result_var = lowerer.lower_expr(body);

    // Terminate the entry block (or current block) with Return.
    if !lowerer.builder.is_terminated() {
        lowerer.builder.terminate_return(result_var);
    }

    let mut func = builder.finish(name, arc_params, return_type, entry, is_fbip);

    // Pre-populate value representations so every variable has a correct
    // repr from the moment it exists. `run_arc_pipeline` will re-compute
    // (same values) as a consistency check.
    let classifier = ArcClassifier::new(pool);
    func.var_reprs = ir::compute_var_reprs(&func, &classifier, pool);

    // Lambda bodies also get pre-populated reprs.
    for lambda in &mut lambdas {
        lambda.var_reprs = ir::compute_var_reprs(lambda, &classifier, pool);
    }

    tracing::debug!(
        name = fn_name,
        blocks = func.blocks.len(),
        vars = func.var_types.len(),
        lambdas = lambdas.len(),
        problems = problems.len(),
        "lower_function_can: done"
    );
    (func, lambdas)
}

// Tests

#[cfg(test)]
mod tests;
