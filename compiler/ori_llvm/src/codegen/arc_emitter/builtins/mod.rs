//! Builtin method codegen for the ARC emitter.
//!
//! Intercepts method calls between the `method_functions` lookup and the
//! runtime fallback to generate inline LLVM IR for builtin methods like
//! `clone`, `length`, `iter`, `is_some`, etc.
//!
//! # Architecture
//!
//! Each submodule uses [`declare_builtins!`] to declare its `(type, method)`
//! registrations and dispatch in a single macro invocation — this guarantees
//! the registration list and match cascade can never drift.
//!
//! A [`BuiltinTable`] (lazily initialized singleton) aggregates all submodule
//! registrations for O(1) lookup and sync-test enumeration against
//! `ori_registry::BUILTIN_TYPES`.
//!
//! # Submodule dispatch
//!
//! Each submodule declares its builtin methods via `declare_builtins!`.
//! No legacy dispatch remains -- all types are covered by submodule
//! declarations.

// declare_builtins! macro — MUST appear before submodule `mod` declarations
// for textual scoping (macro_rules! follow source order in Rust).

/// Declare builtin methods for a submodule.
///
/// Generates both a `dispatch` function and a `REGISTERED` const from a
/// single list of entries, guaranteeing they can never drift apart.
///
/// # Generated items
///
/// - `pub(super) fn dispatch(emitter, ctx) -> Option<ValueId>` — match-based
///   handler dispatch on `(ctx.type_name, ctx.method)`
/// - `pub(super) const REGISTERED: &[BuiltinRegistration]` — enumerable list
///   of all `(type, method)` pairs
///
/// # Usage
///
/// ```ignore
/// declare_builtins! { emitter, ctx;
///     ("int", "abs") => emitter.emit_int_abs(ctx),
///     ("int", "clone") => Some(ctx.arg_vals[0]),
/// }
/// ```
///
/// The `emitter` and `ctx` identifiers are passed explicitly at the
/// invocation site so that handler body expressions share the same
/// macro hygiene context as the generated function parameters.
/// Each handler must evaluate to `Option<ValueId>`.
macro_rules! declare_builtins {
    ($emitter:ident, $ctx:ident; $( ($type_name:expr, $method:expr) => $body:expr ),* $(,)?) => {
        #[allow(dead_code, unused_variables, reason = "macro-generated; not all handlers use every field")]
        pub(super) fn dispatch<'scx: 'ctx, 'ctx>(
            $emitter: &mut $crate::codegen::arc_emitter::ArcIrEmitter<'_, 'scx, 'ctx, '_>,
            $ctx: &super::BuiltinCtx<'_>,
        ) -> Option<$crate::codegen::value_id::ValueId> {
            match ($ctx.type_name, $ctx.method) {
                $(($type_name, $method) => $body,)*
                _ => None,
            }
        }

        pub(super) const REGISTERED: &[super::BuiltinRegistration] = &[
            $(super::BuiltinRegistration {
                type_name: $type_name,
                method_name: $method,
            },)*
        ];
    };
}

mod associated;
mod collections;
mod compound_traits;
mod compound_type_impls;
mod debug_compound;
mod debug_helpers;
mod debug_map_set;
mod dispatch;
mod iterator;
mod iterator_consumers;
mod iterator_protocol;
mod iterators_guard;
mod list_traits;
mod option_result;
mod option_result_helpers;
mod option_result_monadic;
pub(crate) mod prelude;
mod primitives;
mod result_monadic;
mod structural_eq;
mod traceable;
mod traits;
mod trampolines;

pub(super) use traits::CmpPredicate;

use std::sync::LazyLock;

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

// BuiltinRegistration

/// Metadata for a single builtin method codegen handler.
///
/// Declared via [`declare_builtins!`] in each submodule's `REGISTERED` const.
/// Aggregated into [`BuiltinTable`] for O(1) lookup and sync testing.
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "test-only: REGISTERED arrays consumed by BuiltinTable sync tests"
)]
pub(crate) struct BuiltinRegistration {
    /// Type name matching `ori_registry` convention.
    /// Lowercase for primitives (`"int"`), `PascalCase` for named types (`"Option"`).
    pub type_name: &'static str,
    /// Method name (e.g., `"abs"`, `"is_some"`, `"iter"`).
    pub method_name: &'static str,
}

// BuiltinCtx

/// Context passed to submodule `dispatch` functions.
///
/// Carries all data any builtin handler might need. Handlers extract only
/// what they require. This "superset context" pattern avoids per-handler
/// signature variation — every handler sees the same interface.
pub(super) struct BuiltinCtx<'a> {
    /// Type name from `TypeInfo::builtin_type_name` (e.g., `"int"`, `"Option"`).
    pub type_name: &'static str,
    /// Method name from the string interner (e.g., `"abs"`, `"is_some"`).
    pub method: &'a str,
    /// Resolved LLVM values for all arguments (receiver is `arg_vals[0]`).
    pub arg_vals: &'a [ValueId],
    /// Type pool index of the receiver (for parametric type queries).
    pub receiver_ty: Idx,
    /// Type pool index of the destination variable (return type of the method).
    /// Used by result-type-directed builtins (e.g. `str.into() : Error`
    /// constructs the dst Error struct) + niche-aware codegen.
    pub dst_ty: Idx,
    /// Full type info (for extracting inner types, element types, etc.).
    pub type_info: &'a TypeInfo,
    /// Original ARC variable IDs (needed by iterator methods for `var_type` lookups).
    pub arc_args: &'a [ArcVarId],
    /// Enclosing ARC function (needed by iterator methods for variable metadata).
    pub arc_func: &'a ArcFunction,
}

// BuiltinTable

/// Compiled dispatch table for O(1) builtin method lookup.
///
/// Built once as a `LazyLock` singleton from all submodule `REGISTERED` arrays.
/// Uses a two-level map (`type_name` → `method_name` → registration) because
/// `HashMap<(&'static str, &'static str), _>` can't look up with non-static
/// `&str` keys (tuples don't implement `Borrow` transitively), while
/// `HashMap<&'static str, _>::get(&str)` works via `Borrow<str>`.
///
/// Used for:
/// - Early rejection in `try_emit_builtin_method` (skip dispatch for non-builtins)
/// - Enumeration in sync tests vs `ori_registry::BUILTIN_TYPES`
// BuiltinTable and friends are test-only (called from tests.rs and
// #[cfg(test)] helpers). Uses #[allow(dead_code)] (not #[expect]) because the
// items are dead in non-test mode but alive in test mode — #[expect] would
// trigger unfulfilled-lint-expectation errors under `clippy --tests`.
#[allow(dead_code, reason = "test-only: used by sync tests and test helpers")]
pub(crate) struct BuiltinTable {
    /// Two-level map: `type_name` → (`method_name` → registration).
    entries: FxHashMap<&'static str, FxHashMap<&'static str, &'static BuiltinRegistration>>,
}

#[allow(dead_code, reason = "test-only: used by sync tests and test helpers")]
impl BuiltinTable {
    /// Build the table from all submodule registrations.
    fn build() -> Self {
        let sources: &[&[BuiltinRegistration]] = &[
            primitives::REGISTERED,
            collections::REGISTERED,
            iterator::REGISTERED,
            option_result::REGISTERED,
            traits::REGISTERED,
            compound_traits::REGISTERED,
            trampolines::REGISTERED,
        ];

        let mut entries: FxHashMap<
            &'static str,
            FxHashMap<&'static str, &'static BuiltinRegistration>,
        > = FxHashMap::default();

        for source in sources {
            for reg in *source {
                let methods = entries.entry(reg.type_name).or_default();
                debug_assert!(
                    !methods.contains_key(reg.method_name),
                    "duplicate builtin registration: ({}, {})",
                    reg.type_name,
                    reg.method_name,
                );
                methods.insert(reg.method_name, reg);
            }
        }

        Self { entries }
    }

    /// Check whether a `(type_name, method_name)` pair is registered.
    pub(crate) fn has(&self, type_name: &str, method: &str) -> bool {
        self.entries
            .get(type_name)
            .is_some_and(|methods| methods.contains_key(method))
    }

    /// Look up the registration for a `(type_name, method_name)` pair.
    pub(crate) fn lookup(
        &self,
        type_name: &str,
        method: &str,
    ) -> Option<&'static BuiltinRegistration> {
        self.entries.get(type_name)?.get(method).copied()
    }

    /// All registered `(type_name, method_name)` pairs, sorted for deterministic comparison.
    pub(crate) fn all_registered(&self) -> Vec<(&'static str, &'static str)> {
        let mut pairs = Vec::new();
        for (&type_name, methods) in &self.entries {
            for &method_name in methods.keys() {
                pairs.push((type_name, method_name));
            }
        }
        pairs.sort_unstable();
        pairs
    }
}

/// Lazily initialized singleton builtin table.
///
/// Built once on first access from all submodule `REGISTERED` arrays.
/// Thread-safe via `LazyLock`. All data is `'static`.
#[allow(dead_code, reason = "test-only: used by sync tests and test helpers")]
static BUILTIN_TABLE: LazyLock<BuiltinTable> = LazyLock::new(BuiltinTable::build);

/// Access the global builtin table.
#[allow(dead_code, reason = "test-only: used by sync tests and test helpers")]
pub(crate) fn builtin_table() -> &'static BuiltinTable {
    &BUILTIN_TABLE
}

#[cfg(test)]
mod tests;
