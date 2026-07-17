//! Module-wide accumulation that preserves cross-body mono-instance indices.

use ori_ir::ExprId;

use crate::check::ModuleChecker;
use crate::MonoInstanceId;

impl ModuleChecker<'_> {
    /// Accumulate one body's mono instances and parallel dispatch entries.
    ///
    /// `dispatch_entries` index `instances` in body-local coordinates. They
    /// are re-anchored to the module-wide `mono_instances` table before both
    /// vectors are extended. Finalization later remaps these pre-dedup IDs
    /// across deduplication and sorting.
    pub(super) fn accumulate_mono_session(
        &mut self,
        instances: Vec<crate::MonoInstance>,
        mut dispatch_entries: Vec<(ExprId, MonoInstanceId)>,
    ) {
        let Ok(offset) = u32::try_from(self.mono_instances.len()) else {
            unreachable!("module mono-instance table exceeds MonoInstanceId capacity");
        };

        for (_, id) in &mut dispatch_entries {
            assert!(
                id.index() < instances.len(),
                "body-local MonoInstanceId {} is outside its {}-instance session",
                id.raw(),
                instances.len()
            );
            let Some(reanchored) = id.raw().checked_add(offset) else {
                unreachable!(
                    "module-wide MonoInstanceId overflow: local {} + offset {}",
                    id.raw(),
                    offset
                );
            };
            *id = MonoInstanceId::new(reanchored);
        }

        self.mono_instances.extend(instances);
        self.mono_dispatch_pre_dedup.extend(dispatch_entries);
    }
}

#[cfg(test)]
mod tests;
