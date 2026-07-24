//! Assignment-target parsing tests.
//!
//! Covers the `AssignTarget` chain shape for `x[i] = v` / `x.f = v` style LHS,
//! the zero-step guard (bare `x = v` stays `Assign { target: Ident }`),
//! invalid-target rejection (`E1020`), and shared-identity of chain step
//! `ExprId`s between a compound assignment's read-copy and write-target.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test assertions use unwrap/expect for clarity"
)]

use crate::{parse, ParseOutput};
use ori_ir::{AccessStep, BinaryOp, ExprId, ExprKind, StringInterner};

fn parse_source(source: &str) -> ParseOutput {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    parse(&tokens, &interner)
}

/// Parse `@test () -> void = <expr>;` and return the function-body `ExprKind`.
fn body_kind(result: &ParseOutput) -> ExprKind {
    let func = &result.module.functions[0];
    result.arena.get_expr(func.body).kind
}

/// Assert a successful parse whose body is `Assign { target: AssignTarget { ... } }`,
/// returning `(root, steps)` as `(ExprId, Vec<AccessStep>)`.
fn expect_assign_target(result: &ParseOutput) -> (ExprId, Vec<AccessStep>) {
    assert!(!result.has_errors(), "parse errors: {:?}", result.errors);
    let ExprKind::Assign { target, .. } = body_kind(result) else {
        panic!("expected Assign, got {:?}", body_kind(result));
    };
    let ExprKind::AssignTarget { root, steps } = result.arena.get_expr(target).kind else {
        panic!(
            "expected AssignTarget target, got {:?}",
            result.arena.get_expr(target).kind
        );
    };
    (root, result.arena.get_access_steps(steps).to_vec())
}

// Positive shapes: 14 shipped operators over single-step LHS

/// All 14 shipped assignment operators (`=` plus the 13 shipped compound ops;
/// `**=` is excluded — the `**` power operator is spec-defined but unshipped). Each is exercised on an `x[i]` LHS.
const SHIPPED_ASSIGN_OPS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "@=", "&=", "|=", "^=", "<<=", ">>=", "&&=", "||=",
];

#[test]
fn assign_target_index_lhs_all_shipped_ops_builds_assign_target() {
    let mut count = 0;
    for op in SHIPPED_ASSIGN_OPS {
        let src = format!("@test () -> void = x[i] {op} v;");
        let result = parse_source(&src);
        let (root, steps) = expect_assign_target(&result);
        assert!(
            matches!(result.arena.get_expr(root).kind, ExprKind::Ident(_)),
            "op {op}: root should be Ident"
        );
        assert_eq!(steps.len(), 1, "op {op}: one index step");
        assert!(
            matches!(steps[0], AccessStep::Index(_)),
            "op {op}: step should be Index"
        );
        count += 1;
    }
    assert_eq!(count, SHIPPED_ASSIGN_OPS.len());
}

#[test]
fn assign_target_field_lhs_all_shipped_ops_builds_assign_target() {
    let mut count = 0;
    for op in SHIPPED_ASSIGN_OPS {
        let src = format!("@test () -> void = x.f {op} v;");
        let result = parse_source(&src);
        let (root, steps) = expect_assign_target(&result);
        assert!(matches!(
            result.arena.get_expr(root).kind,
            ExprKind::Ident(_)
        ));
        assert_eq!(steps.len(), 1, "op {op}: one field step");
        assert!(
            matches!(steps[0], AccessStep::Field(_)),
            "op {op}: step should be Field"
        );
        count += 1;
    }
    assert_eq!(count, SHIPPED_ASSIGN_OPS.len());
}

// Positive multi-step shapes (root-to-leaf step ordering)

#[test]
fn assign_target_field_then_index_orders_steps_root_to_leaf() {
    let result = parse_source("@test () -> void = x.f[i] = v;");
    let (root, steps) = expect_assign_target(&result);
    assert!(matches!(
        result.arena.get_expr(root).kind,
        ExprKind::Ident(_)
    ));
    assert_eq!(steps.len(), 2);
    // Root-to-leaf: `.f` then `[i]`.
    assert!(
        matches!(steps[0], AccessStep::Field(_)),
        "first step is `.f`"
    );
    assert!(
        matches!(steps[1], AccessStep::Index(_)),
        "second step is `[i]`"
    );
}

#[test]
fn assign_target_index_then_field_orders_steps_root_to_leaf() {
    let result = parse_source("@test () -> void = x[i].f = v;");
    let (_root, steps) = expect_assign_target(&result);
    assert_eq!(steps.len(), 2);
    // Root-to-leaf: `[i]` then `.f`.
    assert!(
        matches!(steps[0], AccessStep::Index(_)),
        "first step is `[i]`"
    );
    assert!(
        matches!(steps[1], AccessStep::Field(_)),
        "second step is `.f`"
    );
}

#[test]
fn assign_target_deep_chain_orders_all_steps_root_to_leaf() {
    let result = parse_source("@test () -> void = x.f.g[i].h = v;");
    let (_root, steps) = expect_assign_target(&result);
    assert_eq!(steps.len(), 4);
    assert!(matches!(steps[0], AccessStep::Field(_)), "step 0 = `.f`");
    assert!(matches!(steps[1], AccessStep::Field(_)), "step 1 = `.g`");
    assert!(matches!(steps[2], AccessStep::Index(_)), "step 2 = `[i]`");
    assert!(matches!(steps[3], AccessStep::Field(_)), "step 3 = `.h`");
}

// Zero-step guard: bare identifier stays Assign { target: Ident }

#[test]
fn assign_bare_ident_keeps_plain_ident_target() {
    let result = parse_source("@test () -> void = x = v;");
    assert!(!result.has_errors(), "parse errors: {:?}", result.errors);
    let ExprKind::Assign { target, .. } = body_kind(&result) else {
        panic!("expected Assign, got {:?}", body_kind(&result));
    };
    assert!(
        matches!(result.arena.get_expr(target).kind, ExprKind::Ident(_)),
        "bare `x = v` target must stay Ident, got {:?}",
        result.arena.get_expr(target).kind
    );
}

#[test]
fn compound_assign_bare_ident_keeps_plain_ident_target() {
    let result = parse_source("@test () -> void = x += v;");
    assert!(!result.has_errors(), "parse errors: {:?}", result.errors);
    let ExprKind::Assign { target, value } = body_kind(&result) else {
        panic!("expected Assign, got {:?}", body_kind(&result));
    };
    assert!(
        matches!(result.arena.get_expr(target).kind, ExprKind::Ident(_)),
        "bare `x += v` target must stay Ident"
    );
    // Value is the desugared `x + v` binary.
    assert!(matches!(
        result.arena.get_expr(value).kind,
        ExprKind::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

// Compound assignment shape: value is Binary over the read-copy chain

#[test]
fn compound_assign_index_lhs_value_is_binary_over_chain() {
    let result = parse_source("@test () -> void = x[i] += v;");
    assert!(!result.has_errors(), "parse errors: {:?}", result.errors);
    let ExprKind::Assign { target, value } = body_kind(&result) else {
        panic!("expected Assign");
    };
    // Write-target is an AssignTarget chain.
    assert!(matches!(
        result.arena.get_expr(target).kind,
        ExprKind::AssignTarget { .. }
    ));
    // Value is `x[i] + v` — read-copy chain is the binary's left (an Index node).
    let ExprKind::Binary {
        op: BinaryOp::Add,
        left,
        ..
    } = result.arena.get_expr(value).kind
    else {
        panic!("expected Binary Add value");
    };
    assert!(matches!(
        result.arena.get_expr(left).kind,
        ExprKind::Index { .. }
    ));
}

// Shared-identity: write-target step ExprId == read-copy step ExprId

#[test]
fn compound_assign_shares_chain_step_expr_ids_between_read_and_write() {
    // `state.items[i] += 1`: the write-target's Index step ExprId must equal the
    // ExprId of the corresponding index in the read-copy chain (binary left).
    let result = parse_source("@test () -> void = state.items[i] += 1;");
    assert!(!result.has_errors(), "parse errors: {:?}", result.errors);
    let ExprKind::Assign { target, value } = body_kind(&result) else {
        panic!("expected Assign");
    };

    // Write-target: AssignTarget with steps [Field(items), Index(i)].
    let ExprKind::AssignTarget { steps, .. } = result.arena.get_expr(target).kind else {
        panic!("expected AssignTarget write-target");
    };
    let write_steps = result.arena.get_access_steps(steps).to_vec();
    let AccessStep::Index(write_index_id) = write_steps[1] else {
        panic!("expected Index as second write step");
    };

    // Read-copy: binary left is the top Index node `state.items[i]`.
    let ExprKind::Binary { left, .. } = result.arena.get_expr(value).kind else {
        panic!("expected Binary value");
    };
    let ExprKind::Index {
        index: read_index_id,
        ..
    } = result.arena.get_expr(left).kind
    else {
        panic!("expected Index read-copy");
    };

    assert_eq!(
        write_index_id, read_index_id,
        "write-target index step ExprId must equal read-copy index ExprId (shared chain identity)"
    );
}

// Negatives: invalid assignment targets rejected with E1020

/// Assert the source fails to parse with an `E1020` invalid-assignment-target error.
fn expect_e1020(src: &str) {
    let result = parse_source(src);
    assert!(
        result
            .errors
            .iter()
            .any(|e| e.code() == ori_diagnostic::ErrorCode::E1020),
        "expected E1020 for `{src}`, got errors: {:?}",
        result.errors
    );
}

#[test]
fn assign_call_root_rejected() {
    expect_e1020("@test () -> void = f()[0] = x;");
}

#[test]
fn assign_paren_binary_root_rejected() {
    expect_e1020("@test () -> void = (a + b)[0] = x;");
}

#[test]
fn assign_try_in_chain_rejected() {
    expect_e1020("@test () -> void = a?.b = x;");
}

#[test]
fn assign_method_call_target_rejected() {
    expect_e1020("@test () -> void = a.m() = x;");
}

#[test]
fn assign_method_call_then_index_rejected() {
    expect_e1020("@test () -> void = a.m()[0] = x;");
}

#[test]
fn assign_cast_root_rejected() {
    expect_e1020("@test () -> void = (a as int).f = x;");
}
