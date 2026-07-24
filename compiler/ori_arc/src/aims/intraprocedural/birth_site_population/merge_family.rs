//! A merge family receives a birth site only when its external origins agree.

use crate::ir::ArcVarId;

use super::super::birth_site_partition::{BirthSiteId, BirthSitePartition, NodeIdx};
use super::whole_node;

/// Assign the unique external birth site to each sound merge family.
pub(super) fn resolve_merge_families(
    partition: &mut BirthSitePartition,
    merges: &[(ArcVarId, Vec<ArcVarId>)],
) -> bool {
    let mut leftover: Vec<usize> = Vec::new();
    for (index, (merge_var, args)) in merges.iter().enumerate() {
        let merge_node = whole_node(partition, *merge_var);
        let unadmitted = args.iter().any(|&arg| {
            let arg_node = whole_node(partition, arg);
            !partition.same_rep(arg_node, merge_node)
        });
        if unadmitted && partition.site(merge_node).is_none() {
            leftover.push(index);
        }
    }
    if leftover.is_empty() {
        return false;
    }

    let mut assigned = false;
    let mut visited: Vec<bool> = vec![false; leftover.len()];
    for start in 0..leftover.len() {
        if visited[start] {
            continue;
        }
        let mut family: Vec<usize> = vec![start];
        visited[start] = true;
        let mut cursor = 0;
        while cursor < family.len() {
            let (_, args) = &merges[leftover[family[cursor]]];
            for &arg in args {
                let arg_node = whole_node(partition, arg);
                for (slot, &other) in leftover.iter().enumerate() {
                    if visited[slot] {
                        continue;
                    }
                    let other_node = whole_node(partition, merges[other].0);
                    if partition.same_rep(arg_node, other_node) {
                        visited[slot] = true;
                        family.push(slot);
                    }
                }
            }
            cursor += 1;
        }

        let member_nodes: Vec<NodeIdx> = family
            .iter()
            .map(|&slot| whole_node(partition, merges[leftover[slot]].0))
            .collect();
        let mut external_site: Option<BirthSiteId> = None;
        let mut sound = true;
        let mut has_external = false;
        for &slot in &family {
            let (_, args) = &merges[leftover[slot]];
            for &arg in args {
                let arg_node = whole_node(partition, arg);
                let internal = member_nodes
                    .iter()
                    .any(|&member| partition.same_rep(arg_node, member));
                if internal {
                    continue;
                }
                has_external = true;
                match (partition.site(arg_node), external_site) {
                    (Some(site), None) => external_site = Some(site),
                    (Some(site), Some(seen)) if site == seen => {}
                    _ => {
                        sound = false;
                        break;
                    }
                }
            }
            if !sound {
                break;
            }
        }
        let (true, true, Some(site)) = (sound, has_external, external_site) else {
            continue;
        };
        for &member in &member_nodes {
            partition.set_site(member, site);
        }
        assigned = true;
        tracing::trace!(
            target: "ori_arc::aims::intraprocedural",
            family_size = family.len(),
            ?site,
            "SCC flow witness: assigned the family's single external birth site"
        );
    }
    assigned
}
