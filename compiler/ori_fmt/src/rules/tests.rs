//! Tests for breaking rules.

use super::*;

mod method_chain_tests {
    use super::*;

    #[test]
    fn method_chain_constants() {
        const { assert!(MethodChainRule::ALL_METHODS_BREAK) };
        assert_eq!(MethodChainRule::MIN_CHAIN_LENGTH, 2);
    }
}

mod short_body_tests {
    use super::*;

    #[test]
    fn short_body_threshold() {
        assert_eq!(ShortBodyRule::THRESHOLD, 20);
    }

    #[test]
    fn break_point_enum() {
        assert_ne!(BreakPoint::BeforeFor, BreakPoint::AfterYield);
        assert_ne!(BreakPoint::AfterYield, BreakPoint::NoBreak);
    }
}

mod boolean_break_tests {
    use super::*;

    #[test]
    fn or_threshold() {
        assert_eq!(BooleanBreakRule::OR_THRESHOLD, 3);
    }
}

mod chained_else_if_tests {
    use super::*;

    #[test]
    fn if_chain_is_simple() {
        let chain = IfChain {
            condition: ori_ir::ExprId::INVALID,
            then_branch: ori_ir::ExprId::INVALID,
            else_ifs: vec![],
            final_else: None,
        };
        assert!(chain.is_simple());
        assert_eq!(chain.branch_count(), 1);
    }

    #[test]
    fn if_chain_with_else() {
        let chain = IfChain {
            condition: ori_ir::ExprId::INVALID,
            then_branch: ori_ir::ExprId::INVALID,
            else_ifs: vec![],
            final_else: Some(ori_ir::ExprId::INVALID),
        };
        assert!(chain.is_simple()); // Still simple (no else-if)
        assert_eq!(chain.branch_count(), 2);
    }

    #[test]
    fn if_chain_with_else_if() {
        let chain = IfChain {
            condition: ori_ir::ExprId::INVALID,
            then_branch: ori_ir::ExprId::INVALID,
            else_ifs: vec![ElseIfBranch {
                condition: ori_ir::ExprId::INVALID,
                then_branch: ori_ir::ExprId::INVALID,
            }],
            final_else: Some(ori_ir::ExprId::INVALID),
        };
        assert!(!chain.is_simple());
        assert_eq!(chain.branch_count(), 3);
    }
}

mod nested_for_tests {
    use super::*;

    #[test]
    fn for_chain_single() {
        let chain = ForChain {
            levels: vec![ForLevel {
                binding: ori_ir::Name::EMPTY,
                iter: ori_ir::ExprId::INVALID,
                guard: None,
                is_yield: true,
            }],
            body: ori_ir::ExprId::INVALID,
        };
        assert!(chain.is_single());
        assert_eq!(chain.depth(), 1);
    }

    #[test]
    fn for_chain_nested() {
        let chain = ForChain {
            levels: vec![
                ForLevel {
                    binding: ori_ir::Name::EMPTY,
                    iter: ori_ir::ExprId::INVALID,
                    guard: None,
                    is_yield: true,
                },
                ForLevel {
                    binding: ori_ir::Name::EMPTY,
                    iter: ori_ir::ExprId::INVALID,
                    guard: None,
                    is_yield: true,
                },
            ],
            body: ori_ir::ExprId::INVALID,
        };
        assert!(!chain.is_single());
        assert_eq!(chain.depth(), 2);
    }
}

mod parentheses_tests {
    use super::*;

    #[test]
    fn paren_position_distinct() {
        assert_ne!(ParenPosition::Receiver, ParenPosition::CallTarget);
        assert_ne!(ParenPosition::CallTarget, ParenPosition::IteratorSource);
        assert_ne!(ParenPosition::IteratorSource, ParenPosition::BinaryOperand);
        assert_ne!(ParenPosition::BinaryOperand, ParenPosition::UnaryOperand);
    }
}

mod run_rule_tests {
    use super::*;
    use crate::packing::Packing;

    #[test]
    fn run_packing_top_level() {
        assert_eq!(RunRule::packing(true), Packing::AlwaysStacked);
    }

    #[test]
    fn run_packing_nested() {
        assert_eq!(RunRule::packing(false), Packing::FitOrOnePerLine);
    }

    #[test]
    fn run_context_function_body() {
        let ctx = RunContext::function_body();
        assert!(ctx.is_top_level());
        assert_eq!(ctx.depth, 0);
        assert!(ctx.is_function_body);
    }

    #[test]
    fn run_context_nested() {
        let ctx = RunContext::nested();
        assert!(!ctx.is_top_level());
        assert_eq!(ctx.depth, 1);
        assert!(!ctx.is_function_body);
    }

    #[test]
    fn run_context_enter_nested() {
        let ctx = RunContext::function_body();
        let nested = ctx.enter_nested();
        assert!(!nested.is_top_level());
        assert_eq!(nested.depth, 1);
    }
}

mod loop_rule_tests {
    // LoopRule tests require an arena, covered in integration tests
}
