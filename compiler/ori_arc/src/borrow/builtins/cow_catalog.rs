//! Copy-on-write builtin ownership catalog.

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashSet;

use super::intern_name_set;

/// Legacy method names with consuming receiver semantics for collections.
///
/// Runtime-backed persistent List mutations are derived from their registry
/// [`ori_registry::MethodRuntime`] identity by
/// [`consuming_receiver_builtin_names`]. This table only owns COW methods that
/// do not yet carry such an identity, plus cross-type names such as `updated`.
///
/// The ARC pipeline must not emit an additional `RcDec` for the receiver
/// argument when calling these methods.
pub(super) const CONSUMING_RECEIVER_METHOD_NAMES: &[&str] = &[
    "add",
    "concat",
    "iter",
    "merge",
    "pop",
    "reverse",
    "sort",
    "sort_stable",
    "updated",
];

/// Legacy COW methods that consume both receiver and second argument.
pub(super) const CONSUMING_SECOND_ARG_METHOD_NAMES: &[&str] = &["add", "concat", "merge"];

/// Legacy COW methods that consume receiver and third argument.
pub(super) const CONSUMING_THIRD_ARG_METHOD_NAMES: &[&str] = &["updated"];

/// COW methods that consume only the receiver; non-receiver args are borrowed.
pub(super) const CONSUMING_RECEIVER_ONLY_METHOD_NAMES: &[&str] =
    &["difference", "insert", "intersection", "remove", "union"];

/// Collect interned names for COW list methods with consuming receiver semantics.
pub(crate) fn consuming_receiver_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    let mut names = intern_name_set(CONSUMING_RECEIVER_METHOD_NAMES, interner);
    names.extend(persistent_list_runtime_methods().map(|method| interner.intern(method.name)));
    names
}

/// Collect interned names for COW list methods that consume their second arg.
pub(crate) fn consuming_second_arg_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    let mut names = intern_name_set(CONSUMING_SECOND_ARG_METHOD_NAMES, interner);
    names.extend(
        persistent_list_runtime_methods()
            .filter(|method| {
                method
                    .params
                    .first()
                    .is_some_and(|param| param.ownership == ori_registry::Ownership::Owned)
            })
            .map(|method| interner.intern(method.name)),
    );
    names
}

/// Collect interned names for COW methods that consume their third arg.
pub(crate) fn consuming_third_arg_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    let mut names = intern_name_set(CONSUMING_THIRD_ARG_METHOD_NAMES, interner);
    names.extend(
        persistent_list_runtime_methods()
            .filter(|method| {
                method
                    .params
                    .get(1)
                    .is_some_and(|param| param.ownership == ori_registry::Ownership::Owned)
            })
            .map(|method| interner.intern(method.name)),
    );
    names
}

pub(crate) fn persistent_list_runtime_methods(
) -> impl Iterator<Item = &'static ori_registry::MethodDef> {
    ori_registry::methods_for(ori_registry::TypeTag::List)
        .iter()
        .filter(|method| {
            matches!(
                method.runtime,
                Some(
                    ori_registry::MethodRuntime::ListPush
                        | ori_registry::MethodRuntime::ListSet
                        | ori_registry::MethodRuntime::ListInsert
                        | ori_registry::MethodRuntime::ListRemove
                        | ori_registry::MethodRuntime::ListPrepend
                )
            )
        })
}

/// Collect interned names for COW methods that consume only the receiver.
pub(crate) fn consuming_receiver_only_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    intern_name_set(CONSUMING_RECEIVER_ONLY_METHOD_NAMES, interner)
}

/// Map/Set COW methods whose borrowed args are copied into the receiver buffer.
const COPY_IN_METHOD_NAMES: &[&str] = &["insert"];

/// Collect interned names for copy-in methods.
pub fn copy_in_builtin_names(interner: &StringInterner) -> FxHashSet<Name> {
    intern_name_set(COPY_IN_METHOD_NAMES, interner)
}

/// Collect interned names for every COW method.
pub fn all_cow_method_names(interner: &StringInterner) -> FxHashSet<Name> {
    let mut names = consuming_receiver_builtin_names(interner);
    names.extend(consuming_receiver_only_builtin_names(interner));
    names
}
