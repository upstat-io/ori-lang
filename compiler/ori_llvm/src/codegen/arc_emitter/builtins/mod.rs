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
//! # Migration
//!
//! During the Section 05 migration, submodules are converted one at a time
//! from empty `declare_builtins! {}` stubs to fully populated declarations.
//! The legacy dispatch (`legacy_dispatch()`) handles all types not yet
//! migrated. Once all submodules are migrated, the legacy dispatch is removed.

// declare_builtins! macro — MUST appear before submodule `mod` declarations
// for textual scoping (macro_rules! follow source order in Rust).

/// Declare builtin methods for a submodule.
///
/// Generates both a `dispatch()` function and a `REGISTERED` const from a
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

mod collections;
mod compound_traits;
mod compound_type_impls;
mod iterator;
mod iterator_consumers;
mod list_traits;
mod option_result;
pub(crate) mod prelude;
mod primitives;
mod traits;
mod trampolines;

pub(super) use traits::CmpPredicate;

use std::sync::LazyLock;

use ori_arc::ir::{ArcFunction, ArcVarId};
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use super::ArcIrEmitter;
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

/// Context passed to submodule `dispatch()` functions.
///
/// Carries all data any builtin handler might need. Handlers extract only
/// what they require. This "superset context" pattern avoids per-handler
/// signature variation — every handler sees the same interface.
pub(super) struct BuiltinCtx<'a> {
    /// Type name from `TypeInfo::builtin_type_name()` (e.g., `"int"`, `"Option"`).
    pub type_name: &'static str,
    /// Method name from the string interner (e.g., `"abs"`, `"is_some"`).
    pub method: &'a str,
    /// Resolved LLVM values for all arguments (receiver is `arg_vals[0]`).
    pub arg_vals: &'a [ValueId],
    /// Type pool index of the receiver (for parametric type queries).
    pub receiver_ty: Idx,
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
// NOTE: BuiltinTable and friends are test-only (called from tests.rs and
// #[cfg(test)] helpers). Uses #[allow(dead_code)] (not #[expect]) because the
// items are dead in non-test mode but alive in test mode — #[expect] would
// trigger unfulfilled-lint-expectation errors under `clippy --tests`.
// A future cleanup could move them under #[cfg(test)] instead.
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

// Dispatch entry point

impl<'scx: 'ctx, 'ctx> ArcIrEmitter<'_, 'scx, 'ctx, '_> {
    /// Try to emit inline IR for a builtin method call.
    ///
    /// Returns `Some(result_value)` if the method was handled, `None` if
    /// the caller should fall through to the runtime function lookup.
    ///
    /// # Dispatch order
    ///
    /// 1. Build [`BuiltinCtx`] (type name, method, resolved arg values)
    /// 2. Try submodule dispatch chain (declarative via `declare_builtins!`)
    /// 3. Fall through to generic clone for types without builtin names
    pub(super) fn try_emit_builtin_method(
        &mut self,
        callee: Name,
        args: &[ArcVarId],
        arc_func: &ArcFunction,
    ) -> Option<ValueId> {
        if args.is_empty() {
            return None;
        }

        let method_name = self.interner.lookup(callee);
        let receiver_ty = arc_func.var_type(args[0]);
        let type_info = self.type_info.get(receiver_ty);
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        // Types with builtin names dispatch through the declarative submodule chain
        if let Some(type_name) = type_info.builtin_type_name() {
            let ctx = BuiltinCtx {
                type_name,
                method: method_name,
                arg_vals: &arg_vals,
                receiver_ty,
                type_info: &type_info,
                arc_args: args,
                arc_func,
            };

            let result = primitives::dispatch(self, &ctx)
                .or_else(|| collections::dispatch(self, &ctx))
                .or_else(|| traits::dispatch(self, &ctx))
                .or_else(|| option_result::dispatch(self, &ctx))
                .or_else(|| iterator::dispatch(self, &ctx))
                .or_else(|| compound_traits::dispatch(self, &ctx));

            if result.is_some() {
                return result;
            }

            // Auto-iter promotion: collection.iter_method(...) → collection.iter().iter_method(...)
            // When a collection type (list, map, Set, str, range) has an unresolved method
            // that IS a known iterator method, emit .iter() implicitly and forward.
            if is_iterator_method(method_name) {
                if let Some(iter_val) = self.emit_auto_iter(&type_info, arg_vals[0], receiver_ty) {
                    let iter_info = TypeInfo::Iterator {
                        element: auto_iter_element_type(&type_info),
                    };
                    let mut iter_args = vec![iter_val];
                    iter_args.extend_from_slice(&arg_vals[1..]);
                    let iter_ctx = BuiltinCtx {
                        type_name: "Iterator",
                        method: method_name,
                        arg_vals: &iter_args,
                        receiver_ty,
                        type_info: &iter_info,
                        arc_args: args,
                        arc_func,
                    };
                    let iter_result = iterator::dispatch(self, &iter_ctx);
                    if iter_result.is_some() {
                        return iter_result;
                    }
                }
            }
        }

        // Types without builtin names: Unit, Struct, Enum, Function
        // Only clone is handled (identity for Unit, RC inc for heap types).
        if method_name == "clone" {
            return match &type_info {
                TypeInfo::Unit => Some(arg_vals[0]),
                TypeInfo::Struct { .. } | TypeInfo::Enum { .. } | TypeInfo::Function { .. } => {
                    self.emit_rc_inc_clone(arg_vals[0], receiver_ty)
                }
                _ => None,
            };
        }

        None
    }

    /// Emit slice-aware RC increment for a value.
    ///
    /// For List/Set: uses `ori_list_rc_inc(data, cap)` which handles
    /// seamless slices (where `data` is interior to another buffer).
    /// For other types: falls back to `ori_rc_inc(data)`.
    fn emit_slice_aware_rc_inc(&mut self, val: ValueId, ty: ori_types::Idx) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            ori_types::Tag::List | ori_types::Tag::Set => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, 1, "rc_inc.cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    self.call_list_rc_inc(dp, cap, 1);
                } else {
                    self.call_rc_inc_all(&[val], 1);
                }
            }
            _ => {
                let rc_inc = self.builder.runtime_fn("ori_rc_inc");
                let data_ptrs = self.extract_rc_data_ptrs(val, ty);
                for data_ptr in data_ptrs {
                    self.emit_rt_call(rc_inc, &[data_ptr], "");
                }
            }
        }
    }

    /// Emit RC increment + return receiver (clone for heap-backed types).
    ///
    /// Uses slice-aware `ori_list_rc_inc(data, cap)` for List/Set types.
    pub(crate) fn emit_rc_inc_clone(
        &mut self,
        val: ValueId,
        ty: ori_types::Idx,
    ) -> Option<ValueId> {
        self.emit_slice_aware_rc_inc(val, ty);
        Some(val)
    }

    /// Emit an implicit `.iter()` call for a collection type.
    ///
    /// The iterator takes ownership of one RC reference. Since the caller
    /// didn't account for this (no explicit `.iter()` in the ARC IR), we
    /// must `RcInc` the collection before creating the iterator.
    ///
    /// Returns the iterator pointer if the receiver is a collection that
    /// supports iteration, `None` otherwise.
    fn emit_auto_iter(
        &mut self,
        type_info: &TypeInfo,
        receiver: ValueId,
        receiver_ty: ori_types::Idx,
    ) -> Option<ValueId> {
        // RcInc the collection — the iterator will consume one reference.
        // For List/Set: use slice-aware ori_list_rc_inc(data, cap) which
        // handles seamless slices (where data is interior to another buffer).
        self.emit_slice_aware_rc_inc(receiver, receiver_ty);
        match type_info {
            TypeInfo::List { element } => self.emit_list_iter(receiver, receiver_ty, *element),
            TypeInfo::Set { element } => self.emit_set_iter(receiver, *element),
            TypeInfo::Map { key, value } => self.emit_map_iter(receiver, *key, *value),
            TypeInfo::Str => self.emit_str_iter(receiver),
            TypeInfo::Range => self.emit_range_iter(receiver),
            _ => None,
        }
    }
}

/// Known iterator method names for auto-iter promotion.
fn is_iterator_method(name: &str) -> bool {
    matches!(
        name,
        "fold"
            | "map"
            | "filter"
            | "any"
            | "all"
            | "count"
            | "find"
            | "for_each"
            | "collect"
            | "take"
            | "skip"
            | "chain"
            | "enumerate"
            | "zip"
            | "flat_map"
            | "flatten"
            | "join"
    )
}

/// Extract the element type from a collection's `TypeInfo` for iterator
/// dispatch after auto-iter promotion.
fn auto_iter_element_type(type_info: &TypeInfo) -> ori_types::Idx {
    match type_info {
        TypeInfo::List { element } | TypeInfo::Set { element } => *element,
        TypeInfo::Map { key, .. } => *key,
        _ => ori_types::Idx::INT, // fallback for str (char→int), range (int)
    }
}

/// NEVER CALLED. Exists solely so that Rust's exhaustive match checker
/// forces updates to this crate when a new `TypeTag` variant is added.
/// If you see a compile error pointing here, a new `TypeTag` was added
/// to `ori_registry` without updating this crate's builtin codegen.
fn _enforce_exhaustiveness(tag: ori_registry::TypeTag) {
    use ori_registry::TypeTag;
    #[expect(
        clippy::match_same_arms,
        reason = "exhaustiveness guard — each arm must be explicit"
    )]
    match tag {
        TypeTag::Int | TypeTag::Float | TypeTag::Bool | TypeTag::Char | TypeTag::Byte => {}
        TypeTag::Unit | TypeTag::Never => {}
        TypeTag::Duration | TypeTag::Size | TypeTag::Ordering => {}
        TypeTag::Str | TypeTag::Error => {}
        TypeTag::List | TypeTag::Map | TypeTag::Set | TypeTag::Range => {}
        TypeTag::Tuple | TypeTag::Option | TypeTag::Result | TypeTag::Channel => {}
        TypeTag::Function | TypeTag::Iterator | TypeTag::DoubleEndedIterator => {}
    }
}

#[cfg(test)]
mod tests;
