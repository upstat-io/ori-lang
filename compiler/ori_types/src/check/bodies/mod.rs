//! Function body type checking passes.
//!
//! This module implements Passes 2-5 of module type checking (`typeck.md` CK-1):
//! - Pass 2: Function bodies (`functions::check_function_bodies`)
//! - Pass 3: Test bodies (`functions::check_test_bodies`)
//! - Pass 4: Impl method bodies (`impls::check_impl_bodies`)
//! - Pass 5: Def impl (default implementation) method bodies (`impls::check_def_impl_bodies`)
//!
//! # Architecture
//!
//! Each function body is checked in a child environment that:
//! 1. Inherits from the frozen base environment (contains all function signatures)
//! 2. Has parameter bindings added
//! 3. Has function scope context set (`current_function`, capabilities)
//!
//! ```text
//! Base Environment (frozen after Pass 1)
//!     │
//!     ├─ child for function foo
//!     │   ├─ param: x -> int
//!     │   └─ param: y -> str
//!     │
//!     └─ child for function bar
//!         └─ param: n -> int
//! ```
//!
//! # File layout
//!
//! `bodies/mod.rs` is a thin dispatch hub. Body-checking logic lives in the
//! submodule that owns each pass — `functions.rs` (Passes 2–3) and `impls.rs`
//! (Passes 4–5). Each public pass-dispatch function has exactly one canonical
//! home; `mod.rs` re-exports via `pub use` so `check::bodies::check_function_bodies`
//! and siblings continue to resolve without changing import paths.

pub mod functions;
pub mod impls;

pub use functions::{check_function_bodies, check_test_bodies};
pub use impls::{check_def_impl_bodies, check_impl_bodies};

use ori_ir::ExprId;
use rustc_hash::FxHashMap;

use crate::check::validators::validate_body_types;
use crate::check::ModuleChecker;
use crate::output::FunctionSig;
use crate::{ExprIndex, Idx, TypeCheckError};

/// Shared PC-2 contract enforcement for every body-checking pass.
///
/// After inference and the end-of-body defaulting pass both complete, walks
/// `expr_types` and `sig` (`param_types` + `return_type`) for any surviving
/// unbound [`Tag::Var`] and emits `E2005` per position. Errors are
/// accumulated via `checker.push_error` alongside normal inference errors.
///
/// `ExprIndex` → `u32` conversion is lossless by construction (see
/// `infer/mod.rs` `store_type` call sites, which cap `ExprIndex` at
/// `u32::MAX`). Falls back to `Span::DUMMY` on the impossible overflow path
/// rather than indexing the arena with `ExprId::INVALID`.
///
/// Consumers: `check_function` / `check_test` in `functions.rs` (§03.1 /
/// §03.2) and `check_impl_method` / `check_def_impl_method` in `impls.rs`
/// (§03.3 / §03.4). Lives in `mod.rs` so both submodules can call it per
/// `impl-hygiene.md §Algorithmic DRY` — the four body passes share the
/// identical validation skeleton.
pub(super) fn run_validator(
    checker: &mut ModuleChecker<'_>,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    sig: &FunctionSig,
    sig_span: ori_ir::Span,
) {
    let validation_errors: Vec<TypeCheckError> = {
        // Scope the immutable borrows (pool, arena) so the subsequent
        // mutable borrow (push_error) can proceed without conflict.
        let arena = checker.arena();
        let pool = checker.pool();
        let mut errs: Vec<TypeCheckError> = Vec::new();
        validate_body_types(
            pool,
            expr_types,
            sig,
            sig_span,
            &sig.scheme_var_ids,
            &|expr_index| {
                u32::try_from(expr_index)
                    .map(|raw| arena.get_expr(ExprId::new(raw)).span)
                    .unwrap_or(ori_ir::Span::DUMMY)
            },
            &mut errs,
        );
        errs
    };
    for err in validation_errors {
        checker.push_error(err);
    }
}

#[cfg(test)]
mod tests;
