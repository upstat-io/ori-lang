use std::sync::Arc;

use ori_types::{Idx, Pool, TypeCheckResult};
use rustc_hash::FxHashMap;

pub(super) struct LoadedModules {
    pub(super) type_results: Vec<TypeCheckResult>,
    pub(super) canons: Vec<ori_ir::canon::SharedCanonResult>,
    pub(super) pools: Vec<Arc<Pool>>,
}

pub(super) fn load(
    db: &crate::db::CompilerDb,
    resolved: &crate::imports::ResolvedImports,
) -> LoadedModules {
    let modules = resolved.modules.iter().chain(resolved.prelude.as_ref());
    let mut type_results = Vec::new();
    let mut canons = Vec::new();
    let mut pools = Vec::new();

    for module in modules {
        let Some((typed, pool)) = crate::query::type_check_module(
            db,
            &module.parse_output,
            &module.module_path,
            module.source_file,
        ) else {
            type_results.push(TypeCheckResult::ok(ori_types::TypedModule::default()));
            canons.push(ori_ir::canon::SharedCanonResult::new(
                ori_ir::canon::CanonResult::empty(),
            ));
            pools.push(Arc::new(Pool::new()));
            continue;
        };
        let canon = crate::query::canonicalize_cached_by_path(
            db,
            &module.module_path,
            &module.parse_output,
            &typed,
            &pool,
        );
        type_results.push(typed);
        canons.push(canon);
        pools.push(pool);
    }

    LoadedModules {
        type_results,
        canons,
        pools,
    }
}

pub(super) fn remap_canons(
    canons: &[ori_ir::canon::SharedCanonResult],
    pools: &[Arc<Pool>],
    merged_pool: &mut Pool,
    caches: &mut [FxHashMap<Idx, Idx>],
    var_remaps: &mut [FxHashMap<u32, u32>],
) -> Vec<ori_ir::canon::CanonResult> {
    canons
        .iter()
        .enumerate()
        .map(|(module_index, shared)| {
            let source_pool = &pools[module_index];
            let cache = &mut caches[module_index];
            let var_remap = &mut var_remaps[module_index];
            let mut remapped = (**shared).clone();
            remapped.arena.remap_types(|type_id| {
                let source = Idx::from_raw(type_id.raw());
                let target = ori_types::re_intern_type_with_var_remap(
                    source_pool,
                    source,
                    merged_pool,
                    cache,
                    var_remap,
                );
                ori_ir::TypeId::from_raw(target.raw())
            });
            remapped
        })
        .collect()
}
