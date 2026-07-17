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
pub mod burden;
pub mod burden_lookup;
mod calls;
pub(crate) mod collections;
mod constructs;
mod control_flow;
pub use control_flow::pool_type_store_size;
mod expr;
mod patterns;
pub(crate) mod scope;

pub use burden::{
    BorrowedFieldView, Burden, BurdenRef, OwnedFieldView, TransferRuleView, TypeRef,
    VariantBurdenView,
};
pub use burden_lookup::{idx_to_type_ref, lookup_burden, type_has_user_drop};

use ori_ir::canon::{CanId, CanonResult, MonoConstBinding};
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
fn build_variant_ctors(pool: &Pool) -> VariantCtors {
    let mut map = VariantCtors::default();
    for idx in pool.iter_indices() {
        if pool.tag(idx) == Tag::Enum {
            let enum_name = pool.enum_name(idx);
            for (vi, (vname, fields)) in pool.enum_variants(idx).into_iter().enumerate() {
                let Ok(variant_index) = u32::try_from(vi) else {
                    unreachable!("enum variant index exceeds the u32 ARC IR domain");
                };
                map.insert(vname, (enum_name, variant_index, fields.len()));
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
    /// Contract coherence violation: inferred contract disagrees with
    /// what the realization pipeline actually emitted (oracle check).
    /// Only reported under `ORI_VERIFY_ARC=1`.
    ContractCoherenceViolation {
        func_name: String,
        mismatches: Vec<crate::aims::verify::oracle::CoherenceMismatch>,
    },
}

// Public entry point

/// Canonical function coordinates consumed by ARC IR lowering.
#[derive(Clone, Copy)]
pub struct ArcLoweringInput<'a> {
    pub name: Name,
    pub params: &'a [(Name, Idx)],
    pub return_type: Idx,
    pub body: CanId,
    pub canon: &'a CanonResult,
    pub interner: &'a StringInterner,
    pub pool: &'a Pool,
    pub type_subst: Option<&'a FxHashMap<Idx, Idx>>,
    pub const_bindings: Option<&'a [MonoConstBinding]>,
    pub is_fbip: bool,
}

/// Lower a typed function body from canonical IR into ARC IR.
///
/// This is the canonical-IR entry point, consuming `CanId` + `CanonResult`
/// instead of `ExprId` + `ExprArena`. Returns the lowered function plus
/// any lambda bodies encountered during lowering.
pub fn lower_function_can(
    input: ArcLoweringInput<'_>,
    problems: &mut Vec<ArcProblem>,
) -> (ArcFunction, Vec<ArcFunction>) {
    let ArcLoweringInput {
        name,
        params,
        return_type,
        body,
        canon,
        interner,
        pool,
        type_subst,
        const_bindings,
        is_fbip,
    } = input;
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
        loop_ctx_stack: Vec::new(),
        problems,
        lambdas: &mut lambdas,
        hash_length: None,
        block_let_names: rustc_hash::FxHashSet::default(),
        func_name: name,
        variant_ctors: &variant_ctors,
        type_subst,
        const_bindings,
        return_type,
    };

    let result_var = lowerer.lower_expr(body);

    // Terminate the entry block (or current block) with Return.
    if !lowerer.builder.is_terminated() {
        lowerer.builder.terminate_return(result_var);
    }

    let mut func = builder.finish(name, arc_params, return_type, entry, is_fbip);

    // Every variable carries its representation before the ARC pipeline runs.
    // The pipeline recomputes the same values as a consistency check.
    let classifier = ArcClassifier::new(pool);
    let representations = ir::compute_var_reprs(&func, &classifier, pool);
    func.replace_variable_representations(representations);

    // Lambda bodies also get pre-populated reprs.
    for lambda in &mut lambdas {
        let representations = ir::compute_var_reprs(lambda, &classifier, pool);
        lambda.replace_variable_representations(representations);
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
pub(crate) mod test_utils;

#[cfg(test)]
mod tests;
