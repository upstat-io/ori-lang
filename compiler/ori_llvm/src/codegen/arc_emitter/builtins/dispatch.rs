//! Builtin-method dispatch entry point.
//!
//! Holds the `ArcIrEmitter` surface that routes method calls through the
//! declarative builtin registry.

use ori_arc::ir::{ArcFunction, ArcVarId, ArgOwnership};
use ori_ir::Name;
use ori_types::Idx;

use super::iterators_guard::{auto_iter_element_type, is_iterator_method};
use super::{
    collections, compound_traits, iterator, option_result, primitives, traits, BuiltinCtx,
};
use crate::codegen::arc_emitter::ArcIrEmitter;
use crate::codegen::type_info::TypeInfo;
use crate::codegen::value_id::ValueId;

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
    pub(in crate::codegen::arc_emitter) fn try_emit_builtin_method(
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
                        // `collect` is itself a terminal consumer that already
                        // produces the eager collection value directly
                        // (`emit_iter_collect`/`emit_iter_collect_set` return
                        // the final `{len,cap,data}` value, not an opaque
                        // iterator handle). Its own `dst_ty` legitimately
                        // resolves to `List`/`Set`, which would otherwise
                        // false-trigger `collect_auto_iter_result`'s
                        // tag-based collect-back and re-collect the already
                        // collected value as though it were a raw iterator
                        // pointer. Every other auto-iter-promoted method
                        // (adapters, and non-collection consumers like
                        // `count`/`any`/`fold`) still needs the collect-back.
                        if method_name == "collect" {
                            return Some(iter_val);
                        }
                        return self.collect_auto_iter_result(iter_val, dst_ty);
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
                && crate::codegen::arc_emitter::context::is_callee_intercepted(
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
    /// the caller's normal-block wiring stays well-formed. Both execution
    /// engines report the same unknown-method failure.
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
    /// `.len()`, indexing, or RC operation runs — otherwise codegen does
    /// `extract_value` on the opaque handle. So when `dst_ty` resolves to a
    /// collection, collect the iterator back into that collection.
    ///
    /// Consumer methods (`count → int`, `any → bool`, `fold → T`) have a
    /// non-collection `dst_ty`; the dispatch already produced the final value
    /// and this is the identity. Explicit `.iter()...collect()` chains keep an
    /// `Iterator`-typed `dst_ty` at the adapter step and likewise pass through.
    fn collect_auto_iter_result(&mut self, iter_val: ValueId, dst_ty: Idx) -> Option<ValueId> {
        let resolved = self.pool.resolve_fully(dst_ty);
        match self.pool.tag(resolved) {
            ori_types::Tag::List => {
                let elem_ty = self.pool.list_elem(resolved);
                self.emit_iter_collect(iter_val, elem_ty)
            }
            ori_types::Tag::Set => {
                let elem_ty = self.pool.set_elem(resolved);
                self.emit_iter_collect_set(iter_val, elem_ty)
            }
            // Non-collection result (`count`/`any`/`all`/`fold`/`find`
            // consumers → int/bool/T) or a genuinely iterator-typed result
            // (explicit `.iter()` chains) — the dispatch already produced the
            // correct value; no collect-back.
            _ => Some(iter_val),
        }
    }

    /// Emit slice-aware RC increment for a value.
    ///
    /// For List/Set: uses `ori_list_rc_inc(data, cap)` which handles
    /// seamless slices (where `data` is interior to another buffer).
    /// For Str: uses `ori_str_rc_inc(data, cap)` which handles SSO,
    /// heap, and seamless slices from `str.split`.
    /// For other types: falls back to `ori_rc_inc(data)`.
    pub(super) fn emit_slice_aware_rc_inc(&mut self, val: ValueId, ty: ori_types::Idx) {
        let resolved = self.pool.resolve_fully(ty);
        let tag = self.pool.tag(resolved);
        match tag {
            ori_types::Tag::List | ori_types::Tag::Set => {
                let Some(data) = self.builder.extract_value(val, 2, "rc_inc.data") else {
                    return;
                };
                let Some(capacity) = self.builder.extract_value(val, 1, "rc_inc.cap") else {
                    return;
                };
                self.call_list_rc_inc(data, capacity, 1);
            }
            // Str: slice-aware RC inc via ori_str_rc_inc(data, cap).
            // Handles SSO, heap, and seamless slices from str.split.
            ori_types::Tag::Str => {
                let Some(data) = self.builder.extract_value(val, 2, "rc_inc.data") else {
                    return;
                };
                let Some(capacity) = self.builder.extract_value(val, 1, "rc_inc.str_cap") else {
                    return;
                };
                self.call_str_rc_inc(data, capacity, 1);
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
    /// RC inc) before the message str becomes the struct's field 0. Both
    /// execution engines preserve the message.
    pub(crate) fn emit_str_into_error(
        &mut self,
        receiver_str: ValueId,
        str_ty: ori_types::Idx,
        error_ty: ori_types::Idx,
    ) -> Option<ValueId> {
        self.emit_slice_aware_rc_inc(receiver_str, str_ty);
        let llvm_ty = self.resolve_type(error_ty);
        // Build the full 2-field Error `{ message, trace }`; zero-init `trace`
        // to an empty list via the canonical `const_zero_ty` empty-value
        // primitive so every Error-construction site shares one init path.
        let list_llvm = self.resolve_type(ori_types::Idx::STR);
        let empty_trace = self.builder.const_zero_ty(list_llvm);
        Some(
            self.builder
                .build_struct(llvm_ty, &[receiver_str, empty_trace], "error.into"),
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
            // The slice-aware increment gives the iterator its own reference.
            TypeInfo::List { element } => {
                self.emit_list_iter(receiver, receiver_ty, *element, ArgOwnership::Owned)
            }
            // The slice-aware increment gives the iterator ownership of the set buffer.
            TypeInfo::Set { element } => {
                self.emit_set_iter(receiver, *element, ArgOwnership::Owned)
            }
            TypeInfo::Map { key, value } => {
                self.emit_map_iter(receiver, *key, *value, receiver_ty, ArgOwnership::Owned)
            }
            TypeInfo::Str => self.emit_str_iter(receiver),
            TypeInfo::Range => self.emit_range_iter(receiver),
            _ => None,
        }
    }
}

/// Whether the registry defines `method` on the builtin type whose lowercase
/// dispatch name is `type_name` (any method kind).
fn registry_defines_method(type_name: &str, method: &str) -> bool {
    ori_registry::BUILTIN_TYPES.iter().any(|td| {
        ori_registry::lowercase_type_name(td.name) == type_name
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
