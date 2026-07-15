//! Exact source ownership for monomorphized executable callables.

use ori_ir::{Name, StringInterner};
use ori_repr::monomorphize::MonoFunction;
use rustc_hash::FxHashMap;

/// One checked mono inventory before any canonical body is lowered.
#[derive(Debug)]
pub(crate) struct MonoFunctionInventory {
    local_start: usize,
    all: Vec<MonoFunction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Producer {
    Local,
    Imported,
}

impl Producer {
    const fn description(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Imported => "imported",
        }
    }
}

/// A final mono identity lacks one unambiguous source body.
#[derive(Debug, thiserror::Error)]
pub(crate) enum MonoFunctionInventoryError {
    /// One producer repeated a final executable identity.
    #[error(
        "monomorphized callable `{callable}` has more than one {producer} source body because the {producer} producer emitted it more than once; normalize that producer inventory before executable realization. Run with `ORI_LOG=oric::realization::mono_inventory=debug` and report this compiler error"
    )]
    DuplicateProducer {
        callable: String,
        producer: &'static str,
    },
    /// Local and imported metadata disagree for one final identity.
    #[error(
        "monomorphized callable `{callable}` has conflicting local and imported {field}; executable realization cannot select one source body. Run with `ORI_LOG=oric::realization::mono_inventory=debug` and report this compiler error"
    )]
    ConflictingProducers {
        callable: String,
        field: &'static str,
    },
    /// Two real source namespaces claimed one final identity.
    #[error(
        "monomorphized callable `{callable}` has both local and imported source bodies; executable realization cannot choose one owner. Rename or remove the conflicting Ori declaration; if no source conflict is reported, run with `ORI_LOG=oric::realization::mono_inventory=debug` and report this compiler error"
    )]
    ConflictingSourceOwners { callable: String },
    /// Imported collector metadata arrived without its source module body.
    #[error(
        "monomorphized callable `{callable}` is marked imported but has no imported source body; executable realization will not lower it against the host module. Check that the defining module is included in the build, then run with `ORI_LOG=oric::realization::mono_inventory=debug` and report this compiler error"
    )]
    MissingImportedSource { callable: String },
}

impl MonoFunctionInventory {
    /// Bind every final mono identity to exactly one source namespace.
    ///
    /// Imported body ownership is authoritative when the general collector
    /// also observes the same imported specialization through its signature
    /// surface. The redundant metadata must be exactly compatible; only its
    /// dispatch ids are merged. Same-origin duplicates remain producer bugs.
    pub(crate) fn try_new(
        local: Vec<MonoFunction>,
        imported: impl IntoIterator<Item = MonoFunction>,
        interner: &StringInterner,
    ) -> Result<Self, MonoFunctionInventoryError> {
        let mut all = Vec::new();
        let mut owner_by_name: FxHashMap<Name, (usize, Producer)> = FxHashMap::default();

        for function in imported {
            let name = function.mangled_name;
            if let Some((_, producer)) = owner_by_name.get(&name).copied() {
                return Err(MonoFunctionInventoryError::DuplicateProducer {
                    callable: interner.lookup(name).to_owned(),
                    producer: producer.description(),
                });
            }
            tracing::debug!(
                target: "oric::realization::mono_inventory",
                callable = interner.lookup(name),
                "registered imported mono source body"
            );
            owner_by_name.insert(name, (all.len(), Producer::Imported));
            all.push(function);
        }

        let local_start = all.len();
        for function in local {
            let name = function.mangled_name;
            match owner_by_name.get(&name).copied() {
                Some((existing_index, Producer::Imported)) => {
                    if !function.is_imported {
                        return Err(MonoFunctionInventoryError::ConflictingSourceOwners {
                            callable: interner.lookup(name).to_owned(),
                        });
                    }
                    let existing = &mut all[existing_index];
                    validate_cross_producer_identity(existing, &function, interner)?;
                    merge_instance_ids(existing, &function);
                    tracing::debug!(
                        target: "oric::realization::mono_inventory",
                        callable = interner.lookup(name),
                        "bound redundant collector metadata to imported source body"
                    );
                }
                Some((_, Producer::Local)) => {
                    return Err(MonoFunctionInventoryError::DuplicateProducer {
                        callable: interner.lookup(name).to_owned(),
                        producer: Producer::Local.description(),
                    });
                }
                None => {
                    if function.is_imported {
                        return Err(MonoFunctionInventoryError::MissingImportedSource {
                            callable: interner.lookup(name).to_owned(),
                        });
                    }
                    tracing::debug!(
                        target: "oric::realization::mono_inventory",
                        callable = interner.lookup(name),
                        "registered local mono source body"
                    );
                    owner_by_name.insert(name, (all.len(), Producer::Local));
                    all.push(function);
                }
            }
        }

        Ok(Self { local_start, all })
    }

    /// Monomorphized bodies owned by the host module's canonical namespace.
    pub(crate) fn local_bodies(&self) -> &[MonoFunction] {
        &self.all[self.local_start..]
    }

    /// Every unique mono identity used for target rewriting and dispatch.
    pub(crate) fn all(&self) -> &[MonoFunction] {
        &self.all
    }

    /// Consume the checked inventory for later LLVM declaration/projection.
    pub(crate) fn into_all(self) -> Vec<MonoFunction> {
        self.all
    }
}

fn validate_cross_producer_identity(
    imported: &MonoFunction,
    local: &MonoFunction,
    interner: &StringInterner,
) -> Result<(), MonoFunctionInventoryError> {
    let differing_field = if imported.original_name != local.original_name {
        Some("source callable identity")
    } else if imported.origin != local.origin {
        Some("semantic body origin")
    } else if imported.sig != local.sig {
        Some("concrete signature")
    } else if imported.body_type_map != local.body_type_map {
        Some("body substitution map")
    } else if imported.receiver_type_name != local.receiver_type_name {
        Some("receiver identity")
    } else {
        None
    };

    if let Some(field) = differing_field {
        tracing::debug!(
            target: "oric::realization::mono_inventory",
            callable = interner.lookup(imported.mangled_name),
            field,
            imported = ?imported,
            local = ?local,
            "rejected conflicting mono producers"
        );
        return Err(MonoFunctionInventoryError::ConflictingProducers {
            callable: interner.lookup(imported.mangled_name).to_owned(),
            field,
        });
    }

    Ok(())
}

fn merge_instance_ids(survivor: &mut MonoFunction, redundant: &MonoFunction) {
    for &instance_id in &redundant.instance_ids {
        if !survivor.instance_ids.contains(&instance_id) {
            survivor.instance_ids.push(instance_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use ori_ir::canon::MonoInstanceId;
    use ori_ir::StringInterner;
    use ori_repr::monomorphize::MonoFunction;
    use ori_types::{FunctionSig, Idx};
    use rustc_hash::FxHashMap;

    use super::{MonoFunctionInventory, MonoFunctionInventoryError};

    #[track_caller]
    fn require_inventory(
        result: Result<MonoFunctionInventory, MonoFunctionInventoryError>,
        context: &str,
    ) -> MonoFunctionInventory {
        match result {
            Ok(inventory) => inventory,
            Err(error) => panic!("{context}: {error}"),
        }
    }

    #[track_caller]
    fn require_inventory_error(
        result: Result<MonoFunctionInventory, MonoFunctionInventoryError>,
        context: &str,
    ) -> MonoFunctionInventoryError {
        match result {
            Ok(inventory) => panic!("{context}: got {inventory:?}"),
            Err(error) => error,
        }
    }

    fn mono(
        interner: &StringInterner,
        callable: &str,
        return_type: Idx,
        instance_id: u32,
        is_imported: bool,
    ) -> MonoFunction {
        let name = interner.intern(callable);
        MonoFunction {
            mangled_name: name,
            original_name: interner.intern("identity"),
            origin: ori_repr::monomorphize::MonoFunctionOrigin::Source,
            sig: FunctionSig::simple(name, vec![Idx::INT], return_type),
            body_type_map: FxHashMap::default(),
            instance_ids: vec![MonoInstanceId::new(instance_id)],
            is_imported,
            receiver_type_name: None,
        }
    }

    #[test]
    fn imported_body_owns_compatible_cross_inventory_identity() {
        let interner = StringInterner::new();
        let local = mono(&interner, "identity$m$int", Idx::INT, 0, true);
        let imported = mono(&interner, "identity$m$int", Idx::INT, 1, true);

        let inventory = require_inventory(
            MonoFunctionInventory::try_new(vec![local], vec![imported], &interner),
            "compatible imported identity must own the source body",
        );

        assert!(
            inventory.local_bodies().is_empty(),
            "the host canon must not lower an imported specialization"
        );
        assert_eq!(inventory.all().len(), 1);
        assert!(inventory.all()[0].is_imported);
        assert_eq!(
            inventory.all()[0].instance_ids,
            vec![MonoInstanceId::new(1), MonoInstanceId::new(0)],
            "dispatch ids from both metadata paths remain bound to the survivor"
        );
    }

    #[test]
    fn local_only_identity_remains_a_local_body() {
        let interner = StringInterner::new();
        let local = mono(&interner, "identity$m$int", Idx::INT, 0, false);

        let inventory = require_inventory(
            MonoFunctionInventory::try_new(vec![local], Vec::new(), &interner),
            "one local identity is valid",
        );

        assert_eq!(inventory.local_bodies().len(), 1);
        assert_eq!(inventory.all().len(), 1);
        assert!(!inventory.all()[0].is_imported);
    }

    #[test]
    fn conflicting_cross_inventory_identity_fails_with_actionable_name() {
        let interner = StringInterner::new();
        let local = mono(&interner, "identity$m$int", Idx::INT, 0, true);
        let imported = mono(&interner, "identity$m$int", Idx::STR, 1, true);

        let error = require_inventory_error(
            MonoFunctionInventory::try_new(vec![local], vec![imported], &interner),
            "different concrete signatures cannot share an executable identity",
        )
        .to_string();

        assert!(error.contains("identity$m$int"));
        assert!(error.contains("concrete signature"));
        assert!(error.contains("ORI_LOG=oric::realization::mono_inventory=debug"));
    }

    #[test]
    fn duplicate_imported_producer_is_rejected_before_lowering() {
        let interner = StringInterner::new();
        let first = mono(&interner, "identity$m$int", Idx::INT, 0, true);
        let second = mono(&interner, "identity$m$int", Idx::INT, 1, true);

        let error = require_inventory_error(
            MonoFunctionInventory::try_new(Vec::new(), vec![first, second], &interner),
            "one imported source must own each final identity",
        )
        .to_string();

        assert!(error.contains("identity$m$int"));
        assert!(error.contains("imported producer emitted it more than once"));
    }

    #[test]
    fn duplicate_local_producer_is_rejected_before_lowering() {
        let interner = StringInterner::new();
        let first = mono(&interner, "identity$m$int", Idx::INT, 0, false);
        let second = mono(&interner, "identity$m$int", Idx::INT, 1, false);

        let error = require_inventory_error(
            MonoFunctionInventory::try_new(vec![first, second], Vec::new(), &interner),
            "one local source must own each final identity",
        )
        .to_string();

        assert!(error.contains("identity$m$int"));
        assert!(error.contains("local producer emitted it more than once"));
    }

    #[test]
    fn genuinely_local_body_cannot_collide_with_imported_source() {
        let interner = StringInterner::new();
        let local = mono(&interner, "identity$m$int", Idx::INT, 0, false);
        let imported = mono(&interner, "identity$m$int", Idx::INT, 1, true);

        let error = require_inventory_error(
            MonoFunctionInventory::try_new(vec![local], vec![imported], &interner),
            "two source namespaces cannot own one final identity",
        )
        .to_string();

        assert!(error.contains("identity$m$int"));
        assert!(error.contains("both local and imported source bodies"));
    }

    #[test]
    fn imported_collector_metadata_requires_an_imported_source_body() {
        let interner = StringInterner::new();
        let metadata_only = mono(&interner, "identity$m$int", Idx::INT, 0, true);

        let error = require_inventory_error(
            MonoFunctionInventory::try_new(vec![metadata_only], Vec::new(), &interner),
            "import metadata cannot be lowered against the host canon",
        )
        .to_string();

        assert!(error.contains("identity$m$int"));
        assert!(error.contains("has no imported source body"));
    }
}
