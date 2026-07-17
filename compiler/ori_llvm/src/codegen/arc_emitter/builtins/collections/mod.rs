//! Collection type builtin methods.
//!
//! Handles `length`/`len`, `is_empty`, `concat`, `iter` for List, Str, Map, Set, Range.

mod collection_fields;
mod hash_thunks;
mod list_builtins;
mod list_cow;
mod list_dispatch;
mod list_field_access;
mod list_sort_thunks;
mod map_builtins;
mod map_dispatch;
mod map_mutations;
mod range_dispatch;
mod set_builtins;
mod set_dispatch;
mod string_builtins;
mod string_dispatch;

use crate::codegen::value_id::ValueId;

use super::super::ArcIrEmitter;
use super::{BuiltinCtx, BuiltinRegistration};

pub(super) fn dispatch<'scx: 'ctx, 'ctx>(
    emitter: &mut ArcIrEmitter<'_, 'scx, 'ctx, '_>,
    ctx: &BuiltinCtx<'_>,
) -> Option<ValueId> {
    string_dispatch::dispatch(emitter, ctx)
        .or_else(|| list_dispatch::dispatch(emitter, ctx))
        .or_else(|| map_dispatch::dispatch(emitter, ctx))
        .or_else(|| set_dispatch::dispatch(emitter, ctx))
        .or_else(|| range_dispatch::dispatch(emitter, ctx))
}

pub(super) const REGISTRATION_GROUPS: &[&[BuiltinRegistration]] = &[
    string_dispatch::REGISTERED,
    list_dispatch::REGISTERED,
    map_dispatch::REGISTERED,
    set_dispatch::REGISTERED,
    range_dispatch::REGISTERED,
];
