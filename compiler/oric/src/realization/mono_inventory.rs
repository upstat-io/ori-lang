//! Exact source ownership for monomorphized executable callables.

use ori_ir::{Name, StringInterner};
use ori_repr::monomorphize::MonoFunction;
use rustc_hash::FxHashMap;

/// One checked mono inventory before any canonical body is lowered.
#[derive(Debug)]
pub(crate) struct MonoFunctionInventory {
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

        Ok(Self { all })
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
    let differing_field = if imported.identity.original_name() != local.identity.original_name() {
        Some("source callable identity")
    } else if imported.origin != local.origin {
        Some("semantic body origin")
    } else if imported.identity.method_producer() != local.identity.method_producer() {
        Some("method producer identity")
    } else if imported.identity.method_args() != local.identity.method_args() {
        Some("method generic arguments")
    } else if imported.identity.const_bindings() != local.identity.const_bindings() {
        Some("method const bindings")
    } else if imported.sig != local.sig {
        Some("concrete signature")
    } else if imported.body_type_map != local.body_type_map {
        Some("body substitution map")
    } else if imported.identity.receiver_type() != local.identity.receiver_type() {
        Some("concrete receiver identity")
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
    for &instance_id in redundant.identity.instance_ids() {
        if !survivor.identity.instance_ids().contains(&instance_id) {
            survivor.identity.push_instance_id(instance_id);
        }
    }
}

#[cfg(test)]
mod tests;
