//! For-yield list comprehension lowering.
//!
//! `for x in iter yield body` produces a `List` by evaluating `body` for each
//! element and collecting results. Three strategies based on iterable type:
//!
//! - **Option**: 0-or-1 element list via `Construct(ListLiteral, [])` or `[body_val]`.
//! - **Range**: Iterator-based loop with `ori_list_new` + `ori_list_push`.
//! - **Iterator/Collection**: Same as range — convert to iterator, loop, push.

use ori_ir::canon::{CanBindingPatternId, CanId};
use ori_types::{Idx, Tag};

use crate::ir::{ArcValue, ArcVarId, CtorKind, LitValue, PrimOp};

use super::super::expr::ArcLowerer;

impl ArcLowerer<'_> {
    /// Dispatch for-yield to the appropriate strategy based on iterable type.
    #[expect(
        clippy::too_many_arguments,
        reason = "lowering params are all semantically distinct"
    )]
    pub(crate) fn lower_for_yield(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        iter_ty: Idx,
        tag: Tag,
        guard: CanId,
        body: CanId,
        result_ty: Idx,
    ) -> ArcVarId {
        tracing::debug!(
            pattern = ?pattern,
            ?tag,
            has_guard = guard.is_valid(),
            "for_yield: enter"
        );
        if tag == Tag::Option {
            let elem_ty = self.pool.option_inner(iter_ty);
            self.lower_for_yield_option(pattern, iter_val, elem_ty, guard, body, result_ty)
        } else {
            // Range, Iterator, List, Set, Str, Map — all go through iterator loop.
            let (iter_handle, elem_ty) = self.prepare_iterator(iter_val, iter_ty, tag);
            self.lower_for_yield_iterator(pattern, iter_handle, elem_ty, guard, body, result_ty)
        }
    }

    /// Prepare an iterator handle from any iterable type.
    ///
    /// Returns `(iterator_ptr_var, element_type)`.
    fn prepare_iterator(&mut self, iter_val: ArcVarId, iter_ty: Idx, tag: Tag) -> (ArcVarId, Idx) {
        if tag == Tag::Range {
            // Range → convert to iterator via .iter()
            let iter_name = self.interner.intern("iter");
            let iter_handle = self
                .builder
                .emit_apply(Idx::INT, iter_name, vec![iter_val], None);
            (iter_handle, Idx::INT)
        } else if tag.is_iterator() {
            let elem_ty = self.pool.iterator_elem(iter_ty);
            (iter_val, elem_ty)
        } else {
            // List, Set, Str, Map → .iter()
            let elem_ty = self.extract_yield_elem_type(tag, iter_ty);
            let iter_name = self.interner.intern("iter");
            // Use INT for iterator handle — it's an opaque ptr, no RC.
            let iter_handle = self
                .builder
                .emit_apply(Idx::INT, iter_name, vec![iter_val], None);
            (iter_handle, elem_ty)
        }
    }

    /// Extract element type for yield expressions.
    fn extract_yield_elem_type(&self, tag: Tag, iter_ty: Idx) -> Idx {
        match tag {
            Tag::List => self.pool.list_elem(iter_ty),
            Tag::Set => self.pool.set_elem(iter_ty),
            Tag::Str => Idx::CHAR,
            Tag::Map => {
                let key_ty = self.pool.map_key(iter_ty);
                let val_ty = self.pool.map_value(iter_ty);
                self.pool.find_tuple(&[key_ty, val_ty]).unwrap_or(Idx::INT)
            }
            _ => Idx::INT,
        }
    }

    /// Lower `for x in <option> yield body` — produce 0-or-1 element list.
    ///
    /// Option yield doesn't need a loop or dynamic list building. It's
    /// a conditional: if Some, construct `[body_val]`; if None, construct `[]`.
    fn lower_for_yield_option(
        &mut self,
        pattern: CanBindingPatternId,
        option_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
        result_ty: Idx,
    ) -> ArcVarId {
        let some_block = self.builder.new_block();
        let none_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Exit takes the result list as a block parameter.
        let result_param = self.builder.add_block_param(exit_block, result_ty);

        // Check tag: project field 0. ARC convention: Some=0, None=1.
        let tag = self.builder.emit_project(Idx::INT, option_val, 0, None);
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let is_some = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::Eq),
                args: vec![tag, zero],
            },
            None,
        );
        self.builder
            .terminate_branch(is_some, some_block, none_block);

        // None path: empty list.
        self.builder.position_at(none_block);
        let empty_list =
            self.builder
                .emit_construct(result_ty, CtorKind::ListLiteral, vec![], None);
        self.builder.terminate_jump(exit_block, vec![empty_list]);

        // Some path: extract element, optionally check guard, evaluate body.
        self.builder.position_at(some_block);
        let elem = self.builder.emit_project(elem_ty, option_val, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        if guard.is_valid() {
            let body_block = self.builder.new_block();
            let guard_val = self.lower_expr(guard);

            // Guard skip → empty list (element filtered out).
            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            self.builder.position_at(guard_skip);
            let skip_empty =
                self.builder
                    .emit_construct(result_ty, CtorKind::ListLiteral, vec![], None);
            self.builder.terminate_jump(exit_block, vec![skip_empty]);

            self.builder.position_at(body_block);
        }

        let body_val = self.lower_expr(body);
        let one_list =
            self.builder
                .emit_construct(result_ty, CtorKind::ListLiteral, vec![body_val], None);

        if !self.builder.is_terminated() {
            self.builder.terminate_jump(exit_block, vec![one_list]);
        }

        // Exit: the result list from whichever path.
        self.builder.position_at(exit_block);
        result_param
    }

    /// Lower `for x in <iterator> yield body` — dynamic list building.
    ///
    /// Uses `ori_list_new` to create a heap-allocated growable list,
    /// `ori_list_push` to append each body result, and `ori_list_take`
    /// to extract the final list struct and free the wrapper.
    ///
    /// ```text
    /// entry: list_ptr = ori_list_new(8, elem_size)
    ///        jump → header
    /// header: next = __iter_next(iter)
    ///         tag = project(next, 0)
    ///         has_more = (tag != 0)
    ///         branch(has_more, body, exit)
    /// body: elem = project(next, 1)
    ///       body_val = lower(body)
    ///       ori_list_push(list_ptr, body_val, elem_size)
    ///       jump → header
    /// exit: result = ori_list_take(list_ptr)
    /// ```
    fn lower_for_yield_iterator(
        &mut self,
        pattern: CanBindingPatternId,
        iter_val: ArcVarId,
        elem_ty: Idx,
        guard: CanId,
        body: CanId,
        result_ty: Idx,
    ) -> ArcVarId {
        let header_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Determine the body result element type from the list result type.
        // result_ty is `List<T>` — extract T for the element size.
        let body_elem_ty = if self.pool.tag(result_ty) == Tag::List {
            self.pool.list_elem(result_ty)
        } else {
            elem_ty // fallback: use iterator element type
        };

        // Allocate growable list: ori_list_new(initial_cap=8, elem_size)
        let list_new = self.interner.intern("ori_list_new");
        let eight = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(8)), None);
        let body_elem_size = self.compute_elem_size(body_elem_ty);
        let elem_size_var = self.builder.emit_let(
            Idx::INT,
            ArcValue::Literal(LitValue::Int(body_elem_size)),
            None,
        );
        // ori_list_new returns ptr (opaque); use INT type in ARC IR (scalar, no RC).
        let list_ptr =
            self.builder
                .emit_apply(Idx::INT, list_new, vec![eight, elem_size_var], None);

        self.builder.terminate_jump(header_block, vec![]);

        // Header: call __iter_next(iter, elem_ty_marker) → {tag, element}
        self.builder.position_at(header_block);
        let iter_next = self
            .interner
            .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::IterNext.name());
        // Use INT result type to suppress ARC RC on the wrapper struct.
        // Pass elem_ty marker so the LLVM emitter can size the scratch buffer.
        let elem_ty_marker =
            self.builder
                .emit_let(elem_ty, ArcValue::Literal(LitValue::Int(0)), None);
        let next_result =
            self.builder
                .emit_apply(Idx::INT, iter_next, vec![iter_val, elem_ty_marker], None);

        let tag = self.builder.emit_project(Idx::INT, next_result, 0, None);
        let zero = self
            .builder
            .emit_let(Idx::INT, ArcValue::Literal(LitValue::Int(0)), None);
        let has_more = self.builder.emit_let(
            Idx::BOOL,
            ArcValue::PrimOp {
                op: PrimOp::Binary(ori_ir::BinaryOp::NotEq),
                args: vec![tag, zero],
            },
            None,
        );

        if guard.is_valid() {
            let guarded_block = self.builder.new_block();
            self.builder
                .terminate_branch(has_more, guarded_block, exit_block);

            self.builder.position_at(guarded_block);
            let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
            self.bind_for_pattern(pattern, elem, elem_ty);
            let guard_val = self.lower_expr(guard);

            let guard_skip = self.builder.new_block();
            self.builder
                .terminate_branch(guard_val, body_block, guard_skip);

            // Guard skip: jump back to header without pushing.
            self.builder.position_at(guard_skip);
            self.builder.terminate_jump(header_block, vec![]);
        } else {
            self.builder
                .terminate_branch(has_more, body_block, exit_block);
        }

        // Body: extract element, evaluate body, push to list.
        self.builder.position_at(body_block);
        let elem = self.builder.emit_project(elem_ty, next_result, 1, None);
        self.bind_for_pattern(pattern, elem, elem_ty);

        let body_val = self.lower_expr(body);

        // ori_list_push(list_ptr, body_val, elem_size)
        let list_push = self.interner.intern("ori_list_push");
        self.builder.emit_apply(
            Idx::UNIT,
            list_push,
            vec![list_ptr, body_val, elem_size_var],
            None,
        );

        if !self.builder.is_terminated() {
            self.builder.terminate_jump(header_block, vec![]);
        }

        // Exit: drop the iterator handle, then extract the final list.
        self.builder.position_at(exit_block);
        let iter_drop = self.interner.intern("ori_iter_drop");
        self.builder
            .emit_apply(Idx::UNIT, iter_drop, vec![iter_val], None);
        let list_take = self.interner.intern("ori_list_take");
        self.builder
            .emit_apply(result_ty, list_take, vec![list_ptr], None)
    }

    /// Compute element size in bytes for a given type.
    ///
    /// Used to pass `elem_size` to `ori_list_new` and `ori_list_push`.
    /// Must match `TypeLayoutResolver::type_store_size()` in `ori_llvm`
    /// (sum of field sizes, no alignment padding).
    fn compute_elem_size(&self, elem_ty: Idx) -> i64 {
        Self::type_store_size(elem_ty, self.pool, 0)
    }

    fn type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
        pool_type_store_size(ty, pool, depth)
    }
}

/// Recursive store-size computation from Pool type information.
///
/// TODO(type_strategy_registry/section-11): Extract shared type layout logic to `ori_ir`.
/// This function duplicates `ori_llvm::codegen::type_info::TypeLayoutResolver::type_store_size()`.
///
/// Sums field sizes without alignment padding. This must stay in sync with
/// `TypeLayoutResolver::type_store_size()` in `ori_llvm` — both compute the
/// same logical size for every type, just at different abstraction levels
/// (Pool indices here vs LLVM `BasicTypeEnum` there).
///
/// See `compiler/ori_llvm/src/codegen/type_info/mod.rs`.
pub(crate) fn pool_type_store_size(ty: Idx, pool: &ori_types::Pool, depth: u32) -> i64 {
    if depth > 16 {
        return 8; // Prevent infinite recursion on recursive types
    }
    // Resolve Named/Applied/Alias types to their underlying structural type.
    let ty = pool.resolve_fully(ty);
    let tag = pool.tag(ty);
    match tag {
        Tag::Bool | Tag::Byte => 1,
        Tag::Char => 4,
        Tag::Unit => 0,
        Tag::Str | Tag::List | Tag::Set | Tag::Map => 24, // {i64, i64, ptr}
        Tag::Struct => pool
            .struct_fields(ty)
            .iter()
            .map(|(_, field_ty)| pool_type_store_size(*field_ty, pool, depth + 1))
            .sum(),
        Tag::Tuple => pool
            .tuple_elems(ty)
            .iter()
            .map(|&field_ty| pool_type_store_size(field_ty, pool, depth + 1))
            .sum(),
        Tag::Option => {
            // Option<T> = {i64 tag, T payload}
            8 + pool_type_store_size(pool.option_inner(ty), pool, depth + 1)
        }
        Tag::Result => {
            // Result<T, E> = {i64 tag, max(T, E) payload}
            let ok_ty = pool.result_ok(ty);
            let err_ty = pool.result_err(ty);
            let ok_size = pool_type_store_size(ok_ty, pool, depth + 1);
            let err_size = pool_type_store_size(err_ty, pool, depth + 1);
            8 + ok_size.max(err_size)
        }
        _ => 8, // Int, Float, Duration, Size, pointer-sized default
    }
}
