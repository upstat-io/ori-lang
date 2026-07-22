use ori_arc::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, LitValue};
use ori_arc::{ArcBlockId, ArcVarId};

use super::loop_carried_narrowing_exclusions;

#[test]
fn loop_carried_values_and_dependents_keep_canonical_width() {
    let initial = ArcVarId::new(0);
    let induction = ArcVarId::new(1);
    let dependent = ArcVarId::new(2);
    let unrelated = ArcVarId::new(3);
    let function = ArcFunction {
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: vec![],
                body: vec![
                    ArcInstr::Let {
                        dst: initial,
                        ty: ori_types::Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(0)),
                    },
                    ArcInstr::Let {
                        dst: unrelated,
                        ty: ori_types::Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(7)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![initial],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: vec![(induction, ori_types::Idx::INT)],
                body: vec![ArcInstr::Let {
                    dst: dependent,
                    ty: ori_types::Idx::INT,
                    value: ArcValue::Var(induction),
                }],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(1),
                    args: vec![dependent],
                },
            },
        ],
        entry: ArcBlockId::new(0),
        var_types: vec![ori_types::Idx::INT; 4],
        spans: vec![vec![None; 2], vec![None]],
        ..Default::default()
    };

    let excluded = loop_carried_narrowing_exclusions(&function);
    assert!(excluded.contains(&induction));
    assert!(excluded.contains(&dependent));
    assert!(!excluded.contains(&unrelated));
}
