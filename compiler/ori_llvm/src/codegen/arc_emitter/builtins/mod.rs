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
    /// Emit parity-correct traceless defaults for the Traceable read accessors
    /// (`trace` / `has_trace` / `trace_entries`) on an `Error` struct or a
    /// Result/Option delegation receiver, plus `with_trace` identity on an
    /// `Error` struct. Returns `None` for any other method/receiver.
    ///
    /// AOT emits no `?`-hop trace injection, so a runtime `Error` carries no
    /// trace storage; these are the eval-traceless-parity answers. Invoked as an
    /// early intercept in `emit_apply` / `emit_invoke` ahead of `resolve_callee`
    /// — a `backend_required: false` Traceable method otherwise resolves to an
    /// unbacked `_ori_trace` mono decl with a mismatched ABI.
    pub(super) fn try_emit_traceless_traceable(
        &mut self,
        callee: Name,
        args: &[ArcVarId],
        arc_func: &ArcFunction,
        dst_ty: Idx,
    ) -> Option<ValueId> {
        if args.is_empty() {
            return None;
        }
        let method_name = self.interner.lookup(callee);
        if !matches!(
            method_name,
            "trace_entries" | "trace" | "has_trace" | "with_trace"
        ) {
            return None;
        }
        // The user-facing `Error` struct (`{ message: str }`, distinct from the
        // `Idx::ERROR` poison sentinel) has no `builtin_type_name`; key it on the
        // pool's SSOT Idx. Result/Option delegate has_trace/trace_entries/trace to
        // the inner Error (eval `dispatch_result_method`).
        let receiver_ty = arc_func.var_type(args[0]);
        let is_error_struct = self.pool.error_struct_idx().is_some_and(|e| {
            receiver_ty == e || self.pool.resolve_fully(receiver_ty) == self.pool.resolve_fully(e)
        });
        let is_traceless = is_error_struct
            || (matches!(method_name, "trace_entries" | "trace" | "has_trace")
                && self
                    .type_info
                    .get(receiver_ty)
                    .builtin_type_name()
                    .is_some_and(|n| n == "Option" || n == "Result"));
        if !is_traceless {
            return None;
        }
        match method_name {
            "trace_entries" => {
                let llvm = self.resolve_type(dst_ty);
                Some(self.builder.const_zero_ty(llvm))
            }
            "trace" => self.emit_literal_ori_str(""),
            "has_trace" => Some(self.builder.const_bool(false)),
            "with_trace" if is_error_struct => Some(self.var(args[0])),
            _ => None,
        }
    }

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
        dst_ty: Idx,
    ) -> Option<ValueId> {
        if args.is_empty() {
            return None;
        }

        let method_name = self.interner.lookup(callee);
        let receiver_ty = arc_func.var_type(args[0]);
        let type_info = self.type_info.get(receiver_ty);
        let arg_vals: Vec<ValueId> = args.iter().map(|a| self.var(*a)).collect();

        // Traceless Traceable accessors (Error-struct + Result/Option delegation).
        // SSOT helper, also invoked as an early intercept in `emit_apply` /
        // `emit_invoke` before callee resolution.
        if let Some(val) = self.try_emit_traceless_traceable(callee, args, arc_func, dst_ty) {
            return Some(val);
        }

        // Types with builtin names dispatch through the declarative submodule chain
        if let Some(type_name) = type_info.builtin_type_name() {
            let ctx = BuiltinCtx {
                type_name,
                method: method_name,
                arg_vals: &arg_vals,
                receiver_ty,
                dst_ty,
                type_info: &type_info,
                arc_args: args,
                arc_func,
            };

            let result = primitives::dispatch(self, &ctx)
                .or_else(|| collections::dispatch(self, &ctx))
                .or_else(|| traits::dispatch(self, &ctx))
                .or_else(|| option_result::dispatch(self, &ctx))
                .or_else(|| iterator::dispatch(self, &ctx))
                .or_else(|| compound_traits::dispatch(self, &ctx))
                .or_else(|| self.try_emit_dei_rekeyed(&ctx));

            if result.is_some() {
                return result;
            }

            // Auto-iter promotion: collection.iter_method(...) → collection.iter.iter_method(...)
            // When a collection type (list, map, Set, str, range) has an unresolved method
            // that IS a known iterator method, emit.iter implicitly and forward.
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
                        dst_ty,
                        type_info: &iter_info,
                        arc_args: args,
                        arc_func,
                    };
                    if let Some(iter_val) = iterator::dispatch(self, &iter_ctx) {
                        return Some(self.collect_auto_iter_result(iter_val, dst_ty));
                    }
                }
            }

            // Eval-parity unknown-method trap. The receiver is a builtin type
            // and every dispatch surface missed (submodule chain, auto-iter
            // promotion). The interpreter resolves builtin methods at dispatch
            // time and raises `no method '<name>' on type <type>` as a RUNTIME
            // panic — mirror that here instead of failing codegen, preserving
            // dual-execution parity for typechecked-but-undispatchable calls
            // (e.g. `str.updated(...)` — str implements Index, not IndexSet).
            // Gates:
            // - `is_callee_intercepted` (SSOT) — the call must be one no
            //   other resolution surface owns (not a declared function, not
            //   a runtime fn, not a monomorphized generic like a stdlib
            //   `assert_eq` whose first ARG happens to be a builtin type,
            //   not prelude/protocol). Mis-firing on those turns a
            //   missing-mono codegen error into a wrong runtime panic.
            // - Methods the registry DOES define on this receiver type are
            //   excluded — a missing dispatch arm for a real method is a
            //   codegen gap (keep the unresolved-function error), not an
            //   unknown method.
            // - Names the registry defines as an ASSOCIATED function on any
            //   type are excluded — associated calls (`Size.from_bytes`)
            //   carry no receiver, so `args[0]` is an ordinary argument and
            //   the receiver-type heuristic misattributes it.
            // Runtime/protocol names (`ori_*`, `__*`) are additionally
            // excluded: `is_callee_intercepted` classifies protocol builtins
            // (`ori_iter_drop`, `__iter_next`) as intercepted, but they
            // resolve via the protocol/runtime-fn fallbacks after this
            // returns None.
            if !method_name.starts_with("ori_")
                && !method_name.starts_with("__")
                && super::context::is_callee_intercepted(
                    method_name,
                    callee,
                    args,
                    arc_func,
                    self.ctx,
                    self.type_info,
                )
                && !registry_defines_method(type_name, method_name)
                && !registry_has_associated_fn(method_name)
            {
                return Some(self.emit_unknown_method_panic(type_name, method_name, dst_ty));
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

    /// Re-key a `DoubleEndedIterator` method dispatch onto an `"Iterator"`
    /// receiver.
    ///
    /// `rev` / `last` / `rfind` / `rfold` / `next_back` register under the
    /// `"DoubleEndedIterator"` key (registry `dei_only`), but every iterator
    /// handle realizes as `TypeInfo::Iterator` (`type_name` `"Iterator"`) at
    /// codegen — there is no DEI `TypeInfo` variant. The type checker proved the
    /// receiver is double-ended, so dispatch the same `BuiltinCtx` under
    /// `"DoubleEndedIterator"` to reach the existing `emit_iter_*` emitters.
    fn try_emit_dei_rekeyed(&mut self, base: &BuiltinCtx<'_>) -> Option<ValueId> {
        if base.type_name != "Iterator" || !ori_registry::is_dei_only(base.method) {
            return None;
        }
        let dei_ctx = BuiltinCtx {
            type_name: "DoubleEndedIterator",
            ..*base
        };
        iterator::dispatch(self, &dei_ctx)
    }

    /// Emit the eval-parity `no method '<name>' on type <type>` runtime panic.
    ///
    /// Emits an unconditional branch to a panic block (`ori_panic_cstr` +
    /// `unreachable`), then positions the builder at an unreachable
    /// continuation block and returns a zero value of the destination type so
    /// the caller's normal-block wiring stays well-formed. Mirrors the
    /// interpreter's unknown-method dispatch error (dual-execution parity).
    fn emit_unknown_method_panic(
        &mut self,
        type_name: &str,
        method_name: &str,
        dst_ty: Idx,
    ) -> ValueId {
        let panic_bb = self
            .builder
            .append_block(self.current_function, "no_method.panic");
        let cont_bb = self
            .builder
            .append_block(self.current_function, "no_method.cont");
        self.builder.br(panic_bb);

        self.builder.position_at_end(panic_bb);
        let msg = format!("no method '{method_name}' on type {type_name}");
        let msg_ptr = self.builder.build_global_string_ptr(&msg, "no_method.msg");
        let panic_fn = self.builder.runtime_fn("ori_panic_cstr");
        self.emit_rt_call(panic_fn, &[msg_ptr], "");
        self.builder.unreachable();

        // Unreachable continuation — keeps the caller's `br` to the normal
        // block well-formed; the zero value is never observed.
        self.builder.position_at_end(cont_bb);
        let dst_llvm_ty = self.resolve_type(dst_ty);
        self.builder.const_zero_ty(dst_llvm_ty)
    }

    /// Materialize an auto-iter-promoted iterator handle back into its
    /// declared result collection.
    ///
    /// Auto-iter promotion (`try_emit_builtin_method`) handles an eager
    /// collection method (`[T].filter`, `[T].map`, etc.) by implicitly
    /// `.iter()`-ing the receiver and dispatching the iterator adapter, which
    /// yields an opaque runtime iterator handle (`ptr`). But the method's
    /// result type per Spec: Annex C is the eager collection (`[T]` for list
    /// adapters), not an iterator. The handle's representation (opaque `ptr`)
    /// must agree with its type (a `{len,cap,data}` fat pointer) before any
    /// downstream `.len()` / indexing / RC op runs — otherwise codegen does
    /// `extract_value` on the opaque handle. So when `dst_ty` resolves to a
    /// collection, collect the iterator back into that collection.
    ///
    /// Consumer methods (`count → int`, `any → bool`, `fold → T`) have a
    /// non-collection `dst_ty`; the dispatch already produced the final value
    /// and this is the identity. Explicit `.iter()...collect()` chains keep an
    /// `Iterator`-typed `dst_ty` at the adapter step and likewise pass through.
    fn collect_auto_iter_result(&mut self, iter_val: ValueId, dst_ty: Idx) -> ValueId {
        let resolved = self.pool.resolve_fully(dst_ty);
        match self.pool.tag(resolved) {
            ori_types::Tag::List => {
                let elem_ty = self.pool.list_elem(resolved);
                self.emit_iter_collect(iter_val, elem_ty)
                    .unwrap_or(iter_val)
            }
            ori_types::Tag::Set => {
                let elem_ty = self.pool.set_elem(resolved);
                self.emit_iter_collect_set(iter_val, elem_ty)
                    .unwrap_or(iter_val)
            }
            // Non-collection result (`count`/`any`/`all`/`fold`/`find`
            // consumers → int/bool/T) or a genuinely iterator-typed result
            // (explicit `.iter()` chains) — the dispatch already produced the
            // correct value; no collect-back.
            _ => iter_val,
        }
    }

    /// Emit slice-aware RC increment for a value.
    ///
    /// For List/Set: uses `ori_list_rc_inc(data, cap)` which handles
    /// seamless slices (where `data` is interior to another buffer).
    /// For Str: uses `ori_str_rc_inc(data, cap)` which handles SSO,
    /// heap, and seamless slices from `str.split`.
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
            // Str: slice-aware RC inc via ori_str_rc_inc(data, cap).
            // Handles SSO, heap, and seamless slices from str.split.
            ori_types::Tag::Str => {
                if let Some(dp) = self.builder.extract_value(val, 2, "rc_inc.data") {
                    let cap = self
                        .builder
                        .extract_value(val, 1, "rc_inc.str_cap")
                        .unwrap_or_else(|| self.builder.const_i64(0));
                    self.call_str_rc_inc(dp, cap, 1);
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
    /// Uses slice-aware RC inc for List/Set and Str types.
    pub(crate) fn emit_rc_inc_clone(
        &mut self,
        val: ValueId,
        ty: ori_types::Idx,
    ) -> Option<ValueId> {
        self.emit_slice_aware_rc_inc(val, ty);
        Some(val)
    }

    /// Emit `str.into() : Error` — construct the user-facing
    /// `Error` struct `{ message: str }` from the receiver string.
    ///
    /// The receiver is borrowed (the caller retains its reference), so the
    /// constructed `Error` takes its own ref to the message (slice-aware
    /// RC inc) before the message str becomes the struct's field 0. Mirrors
    /// the evaluator's `Value::error(s)` (`ori_eval/methods/collections.rs`)
    /// so interp↔LLVM observable behavior agrees (message preserved).
    pub(crate) fn emit_str_into_error(
        &mut self,
        receiver_str: ValueId,
        str_ty: ori_types::Idx,
        error_ty: ori_types::Idx,
    ) -> Option<ValueId> {
        self.emit_slice_aware_rc_inc(receiver_str, str_ty);
        let llvm_ty = self.resolve_type(error_ty);
        Some(
            self.builder
                .build_struct(llvm_ty, &[receiver_str], "error.into"),
        )
    }

    /// Emit an implicit `.iter` call for a collection type.
    ///
    /// The iterator takes ownership of one RC reference. Since the caller
    /// didn't account for this (no explicit `.iter` in the ARC IR), we
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
        // For List/Set: slice-aware ori_list_rc_inc(data, cap).
        // For Str: slice-aware ori_str_rc_inc(data, cap).
        self.emit_slice_aware_rc_inc(receiver, receiver_ty);
        match type_info {
            // The slice-aware inc above gave the iterator its own ref → owns_data = true.
            TypeInfo::List { element } => {
                self.emit_list_iter(receiver, receiver_ty, *element, true)
            }
            TypeInfo::Set { element } => self.emit_set_iter(receiver, *element),
            TypeInfo::Map { key, value } => {
                self.emit_map_iter(receiver, *key, *value, receiver_ty, true)
            }
            TypeInfo::Str => self.emit_str_iter(receiver),
            TypeInfo::Range => self.emit_range_iter(receiver),
            _ => None,
        }
    }
}

/// Whether the registry defines `method` on the builtin type whose legacy
/// dispatch name is `type_name` (any method kind).
fn registry_defines_method(type_name: &str, method: &str) -> bool {
    ori_registry::BUILTIN_TYPES.iter().any(|td| {
        ori_registry::legacy_type_name(td.name) == type_name
            && td.methods.iter().any(|m| m.name == method)
    })
}

/// Whether ANY builtin type defines `method` as an ASSOCIATED function
/// (factory — no receiver; `args[0]` is an ordinary argument).
fn registry_has_associated_fn(method: &str) -> bool {
    ori_registry::BUILTIN_TYPES.iter().any(|td| {
        td.methods
            .iter()
            .any(|m| m.name == method && m.kind == ori_registry::MethodKind::Associated)
    })
}

use iterators_guard::{auto_iter_element_type, is_iterator_method};

#[cfg(test)]
mod tests;
