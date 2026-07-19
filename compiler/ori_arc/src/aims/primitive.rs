//! Typed primitive-operation resolution at the AIMS input seam.

use std::collections::BTreeMap;

use crate::ir::{ArcFunction, ArcValue, ArcVarId, PrimOp, PrimitiveFact, PrimitiveFacts};
use crate::verify::VerifyError;

type PrimitiveSites = BTreeMap<ArcVarId, (PrimOp, Vec<ArcVarId>)>;
use crate::ArcClassification;

/// Freeze exact primitive facts for every function in one analysis batch.
///
/// This is the shared population seam for source-produced and synthetic ARC
/// bodies. A function whose table is empty receives one typed fact for every
/// `PrimOp`. A non-empty table is immutable evidence and is only validated;
/// missing, extra, duplicate, or stale entries fail closed instead of being
/// repaired. Publication is transactional: if any function is invalid, no
/// pending table in the batch is installed. Every caller that constructs a
/// batch outside the normal lowering pipeline must pass through this seam
/// before analysis or artifact closure.
pub fn freeze_primitive_facts(
    functions: &mut [ArcFunction],
    classifier: &dyn ArcClassification,
) -> Result<(), Vec<VerifyError>> {
    let mut pending = Vec::new();
    let mut errors = Vec::new();
    for (index, function) in functions.iter().enumerate() {
        let (sites, mut function_errors) = collect_sites(function);
        if !function_errors.is_empty() {
            errors.append(&mut function_errors);
            continue;
        }
        if function.primitive_facts.is_empty() {
            match resolve_sites(function, classifier, &sites) {
                Ok(resolved) => pending.push((index, resolved)),
                Err(mut resolution_errors) => errors.append(&mut resolution_errors),
            }
        } else if let Err(mut validation_errors) =
            validate_frozen_facts(function, classifier, &sites)
        {
            errors.append(&mut validation_errors);
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    for (index, resolved) in pending {
        functions[index].primitive_facts = resolved;
    }
    Ok(())
}

/// Resolve the function's primitive facts once, or validate an already-frozen
/// table without repairing it.
///
/// Exact coverage is part of the ARC artifact contract: every `PrimOp` has one
/// fact keyed by its SSA destination, and no fact exists without a matching
/// instruction. A non-empty table is immutable evidence from an earlier AIMS
/// pass, so disagreement fails closed.
pub(crate) fn ensure_primitive_facts(
    func: &mut ArcFunction,
    classifier: &dyn ArcClassification,
) -> Result<(), Vec<VerifyError>> {
    let (sites, duplicate_errors) = collect_sites(func);
    if !duplicate_errors.is_empty() {
        return Err(duplicate_errors);
    }

    if func.primitive_facts.is_empty() {
        func.primitive_facts = resolve_sites(func, classifier, &sites)?;
        return Ok(());
    }

    validate_frozen_facts(func, classifier, &sites)
}

fn resolve_sites(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sites: &BTreeMap<ArcVarId, (PrimOp, Vec<ArcVarId>)>,
) -> Result<PrimitiveFacts, Vec<VerifyError>> {
    let mut resolved = PrimitiveFacts::default();
    let mut errors = Vec::new();
    for (&dst, &(op, ref args)) in sites {
        match resolve_site(func, classifier, dst, op, args) {
            Ok(fact) => {
                let previous = resolved.insert(dst, fact);
                debug_assert!(previous.is_none());
            }
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(resolved)
    } else {
        Err(errors)
    }
}

/// Validate exact primitive-fact coverage without resolving or repairing it.
///
/// The AIMS pipeline is the only policy owner that may populate the table.
/// Immutable artifact boundaries call this function to reject missing, extra,
/// duplicate, or stale evidence before any executor projects it.
pub fn validate_primitive_facts(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
) -> Result<(), Vec<VerifyError>> {
    let (sites, duplicate_errors) = collect_sites(func);
    if duplicate_errors.is_empty() {
        validate_frozen_facts(func, classifier, &sites)
    } else {
        Err(duplicate_errors)
    }
}

/// Retire facts whose keyed instructions were removed by a structural pass.
///
/// This is a mechanical identity projection, not semantic re-resolution: a
/// surviving site keeps its already-frozen fact byte-for-byte, while a removed
/// SSA definition can no longer own artifact evidence. Missing or changed facts
/// remain visible to the validation seam.
pub(crate) fn retire_removed_primitive_facts(func: &mut ArcFunction) {
    let (sites, _) = collect_sites(func);
    func.primitive_facts.retain(|dst| sites.contains_key(&dst));
}

fn collect_sites(func: &ArcFunction) -> (PrimitiveSites, Vec<VerifyError>) {
    let mut sites = BTreeMap::new();
    let mut errors = Vec::new();
    for block in &func.blocks {
        for instr in &block.body {
            let crate::ir::ArcInstr::Let {
                dst,
                value: ArcValue::PrimOp { op, args },
                ..
            } = instr
            else {
                continue;
            };
            if sites.insert(*dst, (*op, args.clone())).is_some() {
                errors.push(invalid(
                    *dst,
                    "multiple PrimOp instructions define one destination",
                ));
            }
        }
    }
    (sites, errors)
}

fn validate_frozen_facts(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    sites: &PrimitiveSites,
) -> Result<(), Vec<VerifyError>> {
    let mut errors = Vec::new();

    for (&dst, &(op, ref args)) in sites {
        let Some(frozen) = func.primitive_facts.get(dst) else {
            errors.push(invalid(dst, "missing frozen fact"));
            continue;
        };
        match resolve_site(func, classifier, dst, op, args) {
            Ok(expected) if frozen == expected && frozen.is_valid_for(args.len()) => {}
            Ok(_) => errors.push(invalid(dst, "frozen fact disagrees with typed registry")),
            Err(error) => errors.push(error),
        }
    }

    for (dst, _) in func.primitive_facts.iter() {
        if !sites.contains_key(&dst) {
            errors.push(invalid(dst, "frozen fact has no PrimOp instruction"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn resolve_site(
    func: &ArcFunction,
    classifier: &dyn ArcClassification,
    dst: ArcVarId,
    op: PrimOp,
    args: &[ArcVarId],
) -> Result<PrimitiveFact, VerifyError> {
    let expected_arity = match op {
        PrimOp::Binary(_) => 2,
        PrimOp::Unary(_) => 1,
    };
    if args.len() != expected_arity {
        return Err(invalid(dst, "primitive arity does not match operator"));
    }
    let Some(&receiver) = args.first() else {
        return Err(invalid(dst, "primitive has no typed receiver"));
    };
    let Some(&receiver_ty) = func.var_types.get(receiver.index()) else {
        return Err(invalid(dst, "primitive receiver variable is out of bounds"));
    };
    let strategy = match op {
        PrimOp::Binary(operation) => classifier.builtin_type_tag(receiver_ty).map_or_else(
            || {
                ori_types::user_structural_binary_strategy(operation)
                    .unwrap_or(ori_registry::OpStrategy::Unsupported)
            },
            |type_tag| ori_types::primitive_binary_strategy(type_tag, operation),
        ),
        PrimOp::Unary(operation) => classifier
            .builtin_type_tag(receiver_ty)
            .map_or(ori_registry::OpStrategy::Unsupported, |type_tag| {
                ori_types::primitive_unary_strategy(type_tag, operation)
            }),
    };
    PrimitiveFact::resolve(strategy, args.len())
        .ok_or_else(|| invalid(dst, "typed registry has no valid primitive descriptor"))
}

fn invalid(dst: ArcVarId, reason: &'static str) -> VerifyError {
    VerifyError::PrimitiveFactInvalid { dst, reason }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator};
    use ori_ir::{BinaryOp, Name};
    use ori_registry::{PrimitiveResultOwnership, RuntimeOperator};
    use ori_types::{Idx, Pool};

    fn list_concat_function(pool: &mut Pool) -> ArcFunction {
        let list_ty = pool.list(Idx::INT);
        ArcFunction {
            name: Name::from_raw(1),
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: list_ty,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            }],
            var_types: vec![list_ty, list_ty, list_ty],
            return_type: list_ty,
            ..ArcFunction::default()
        }
    }

    fn ordinary_loop_primitive_function() -> ArcFunction {
        ArcFunction {
            name: Name::from_raw(2),
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: ArcVarId::new(3),
                        ty: Idx::BOOL,
                        value: ArcValue::PrimOp {
                            op: PrimOp::Binary(BinaryOp::Eq),
                            args: vec![ArcVarId::new(1), ArcVarId::new(2)],
                        },
                    },
                    ArcInstr::Let {
                        dst: ArcVarId::new(5),
                        ty: Idx::INT,
                        value: ArcValue::PrimOp {
                            op: PrimOp::Binary(BinaryOp::Sub),
                            args: vec![ArcVarId::new(1), ArcVarId::new(4)],
                        },
                    },
                ],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(5),
                },
            }],
            var_types: vec![Idx::INT, Idx::INT, Idx::INT, Idx::BOOL, Idx::INT, Idx::INT],
            return_type: Idx::INT,
            ..ArcFunction::default()
        }
    }

    #[test]
    fn freezes_list_concat_descriptor_from_typed_registry() {
        let mut pool = Pool::new();
        let mut function = list_concat_function(&mut pool);
        let classifier = crate::ArcClassifier::new(&pool);

        let Ok(()) = ensure_primitive_facts(&mut function, &classifier) else {
            panic!("descriptor should resolve");
        };

        let Some(fact) = function.primitive_facts.get(ArcVarId::new(2)) else {
            panic!("expected a list-concat fact");
        };
        assert_eq!(
            fact.strategy,
            ori_registry::OpStrategy::RuntimeCall(RuntimeOperator::ListConcat)
        );
        assert!(matches!(
            fact.descriptor.result,
            PrimitiveResultOwnership::OwnedFromConsumedOrIndependent { .. }
        ));
    }

    #[test]
    fn batch_freeze_covers_every_ordinary_primitive_site() {
        let mut pool = Pool::new();
        let mut functions = vec![
            list_concat_function(&mut pool),
            ordinary_loop_primitive_function(),
        ];
        let classifier = crate::ArcClassifier::new(&pool);

        let Ok(()) = freeze_primitive_facts(&mut functions, &classifier) else {
            panic!("every primitive site should resolve");
        };

        assert_eq!(functions[0].primitive_facts.len(), 1);
        assert_eq!(functions[1].primitive_facts.len(), 2);
        assert!(functions[1].primitive_facts.get(ArcVarId::new(3)).is_some());
        assert!(functions[1].primitive_facts.get(ArcVarId::new(5)).is_some());
    }

    #[test]
    fn freezes_user_structural_equality_without_inventing_a_callable() {
        let mut pool = Pool::new();
        let type_name = Name::from_raw(40);
        let field_name = Name::from_raw(41);
        let user_type = pool.struct_type(type_name, &[(field_name, Idx::INT)]);
        let mut function = ArcFunction {
            name: Name::from_raw(42),
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::BOOL,
                    value: ArcValue::PrimOp {
                        op: PrimOp::Binary(BinaryOp::Eq),
                        args: vec![ArcVarId::new(0), ArcVarId::new(1)],
                    },
                }],
                terminator: ArcTerminator::Return {
                    value: ArcVarId::new(2),
                },
            }],
            var_types: vec![user_type, user_type, Idx::BOOL],
            return_type: Idx::BOOL,
            ..ArcFunction::default()
        };
        let classifier = crate::ArcClassifier::new(&pool);

        let Ok(()) = ensure_primitive_facts(&mut function, &classifier) else {
            panic!("user structural equality should resolve");
        };

        assert_eq!(
            function
                .primitive_facts
                .get(ArcVarId::new(2))
                .map(|fact| fact.strategy),
            Some(ori_registry::OpStrategy::StructuralEquality)
        );
    }

    #[test]
    fn batch_freeze_rejects_partial_evidence_without_repair() {
        let pool = Pool::new();
        let mut function = ordinary_loop_primitive_function();
        let Some(fact) = PrimitiveFact::resolve(ori_registry::OpStrategy::SignedInteger, 2) else {
            panic!("expected an integer descriptor");
        };
        assert!(function
            .primitive_facts
            .insert(ArcVarId::new(3), fact)
            .is_none());
        let mut functions = vec![function];
        let classifier = crate::ArcClassifier::new(&pool);

        let Err(errors) = freeze_primitive_facts(&mut functions, &classifier) else {
            panic!("partial evidence must fail closed");
        };

        assert!(errors.iter().any(|error| matches!(
            error,
            VerifyError::PrimitiveFactInvalid { dst, reason }
                if *dst == ArcVarId::new(5) && *reason == "missing frozen fact"
        )));
        assert_eq!(functions[0].primitive_facts.len(), 1);
        assert!(functions[0].primitive_facts.get(ArcVarId::new(5)).is_none());
    }

    #[test]
    fn rejected_batch_publishes_no_partial_primitive_tables() {
        let mut pool = Pool::new();
        let valid = list_concat_function(&mut pool);
        let mut invalid = ordinary_loop_primitive_function();
        let Some(frozen) = PrimitiveFact::resolve(ori_registry::OpStrategy::SignedInteger, 2)
        else {
            panic!("expected an integer descriptor");
        };
        assert!(invalid
            .primitive_facts
            .insert(ArcVarId::new(3), frozen)
            .is_none());
        let mut functions = vec![valid, invalid];
        let classifier = crate::ArcClassifier::new(&pool);

        let Err(_errors) = freeze_primitive_facts(&mut functions, &classifier) else {
            panic!("one invalid function must reject the whole batch");
        };

        assert!(
            functions[0].primitive_facts.is_empty(),
            "a rejected batch must not publish earlier pending tables"
        );
        assert_eq!(functions[1].primitive_facts.len(), 1);
    }

    #[test]
    fn frozen_table_disagreement_fails_closed() {
        let mut pool = Pool::new();
        let mut function = list_concat_function(&mut pool);
        let classifier = crate::ArcClassifier::new(&pool);
        let Some(scalar) = PrimitiveFact::resolve(ori_registry::OpStrategy::SignedInteger, 2)
        else {
            panic!("expected a scalar descriptor");
        };
        function.primitive_facts.insert(ArcVarId::new(2), scalar);

        let Err(errors) = ensure_primitive_facts(&mut function, &classifier) else {
            panic!("stale fact must be rejected");
        };
        assert!(matches!(
            errors.as_slice(),
            [VerifyError::PrimitiveFactInvalid { dst, .. }] if *dst == ArcVarId::new(2)
        ));
    }
}
