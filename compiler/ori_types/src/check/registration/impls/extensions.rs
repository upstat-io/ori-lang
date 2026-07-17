//! Builtin extension-method indexing.

use ori_ir::Name;
use rustc_hash::{FxHashMap, FxHashSet};

use super::super::type_resolution::resolve_parsed_type_simple;
use crate::ModuleChecker;

/// Build the builtin-type extension-method index from `module.extends` and
/// install it on the checker. Only extensions whose target resolves to a builtin
/// `TypeTag` are indexed; user-type extensions defer through normal dispatch
/// (a user `Named`/`Applied` receiver is never reject-eligible). Consulted by
/// `emit_unknown_method` so an `extend <builtin> { @m }` method is not
/// false-rejected as unknown (TR-9 dispatch stays target-only — the evaluator
/// owns the actual call).
pub fn register_builtin_extensions(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    let arena = checker.arena();
    let mut index: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>> = FxHashMap::default();
    for ext in &module.extends {
        let target_idx = resolve_parsed_type_simple(checker, &ext.target_ty, arena);
        let tag = checker.pool().tag(target_idx);
        let Some(type_tag) = crate::infer::tag_to_type_tag(tag) else {
            continue;
        };
        let methods = index.entry(type_tag).or_default();
        for m in &ext.methods {
            methods.insert(m.name);
        }
    }
    checker.set_builtin_extensions(index);
}
