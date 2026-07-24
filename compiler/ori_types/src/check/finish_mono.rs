//! Monomorphized-instance deduplication and dispatch remapping.

use ori_ir::Name;

use crate::Idx;

type MonoIdentityKey = (
    Name,
    Vec<crate::GenericArg>,
    Vec<Idx>,
    Vec<crate::GenericArg>,
    Vec<crate::GenericArg>,
    Vec<Idx>,
    Option<Idx>,
    Option<crate::MethodProducer>,
);

/// Deduplicate instances, sort survivors, and remap pre-dedup dispatch IDs.
pub(super) fn dedup_and_remap_mono_instances(
    mut mono_instances: Vec<crate::MonoInstance>,
    mono_dispatch_pre_dedup: Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
) -> (
    Vec<crate::MonoInstance>,
    Vec<(ori_ir::ExprId, crate::MonoInstanceId)>,
) {
    let pre_dedup_len = mono_instances.len();
    let mut seen: rustc_hash::FxHashMap<MonoIdentityKey, u32> = rustc_hash::FxHashMap::default();
    let mut deduped = Vec::with_capacity(pre_dedup_len);
    let mut old_to_dedup = Vec::with_capacity(pre_dedup_len);
    for instance in mono_instances.drain(..) {
        let key: MonoIdentityKey = (
            instance.fn_name,
            instance.generic_args.clone(),
            instance.capability_args.clone(),
            instance.impl_args.clone(),
            instance.method_args.clone(),
            instance.concrete_param_types.clone(),
            instance.receiver_type,
            instance.method_producer.clone(),
        );
        if let Some(&existing) = seen.get(&key) {
            old_to_dedup.push(existing);
        } else {
            let Ok(new_index) = u32::try_from(deduped.len()) else {
                unreachable!("monomorphized instance table exceeded MonoInstanceId capacity");
            };
            seen.insert(key, new_index);
            deduped.push(instance);
            old_to_dedup.push(new_index);
        }
    }

    let dedup_len = deduped.len();
    let mut indexed: Vec<(u32, crate::MonoInstance)> = deduped
        .into_iter()
        .enumerate()
        .map(|(index, instance)| {
            let Ok(index) = u32::try_from(index) else {
                unreachable!("monomorphized instance table exceeded MonoInstanceId capacity");
            };
            (index, instance)
        })
        .collect();
    indexed.sort_by_key(|(_, instance)| instance.fn_name);

    let mut dedup_to_sorted = vec![0; dedup_len];
    for (sorted_position, (dedup_position, _)) in indexed.iter().enumerate() {
        let Ok(sorted_position) = u32::try_from(sorted_position) else {
            unreachable!("sorted mono-instance table exceeded MonoInstanceId capacity");
        };
        dedup_to_sorted[*dedup_position as usize] = sorted_position;
    }
    let mono_instances = indexed.into_iter().map(|(_, instance)| instance).collect();

    let mono_dispatch_map = mono_dispatch_pre_dedup
        .into_iter()
        .map(|(expression, id)| {
            let dedup_index = old_to_dedup[id.index()];
            let final_index = dedup_to_sorted[dedup_index as usize];
            (expression, crate::MonoInstanceId::new(final_index))
        })
        .collect();

    (mono_instances, mono_dispatch_map)
}
