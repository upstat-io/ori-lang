//! Unit tests for the body / terminator axes of `assert_no_unresolved_type_vars`.
//!
//! Extends the core 12-cell matrix in the parent module with coverage of the
//! walker extension to instruction operands (`blocks[*].body[*]`) and
//! terminator operands (`blocks[*].terminator`).
//!
//! Cells covered (body-walker matrix axis):
//! - Cell (m): `Tag::Var` in `ArcInstr::Construct.ty` — canonical
//!   type-nominating instruction.
//! - Cell (n): `Tag::Var` in `ArcInstr::Apply.ty` — representative direct-call
//!   variant from the exhaustive set
//!   `{Let, Apply, ApplyIndirect, PartialApply, Project, Construct, Reuse,
//!   CollectionReuse, Select}`.
//! - Cell (o): `Tag::Var` in `ArcInstr::Project.ty` — representative
//!   borrow/projection variant from the same set (distinct shape from Apply).
//! - Cell (p): direct call of `assert_no_unresolved_idx` on a synthetic
//!   `Tag::Var` `Idx` — pins the thin-helper path for non-`ArcFunction` call
//!   sites (derive synthesis, panic / iterator trampolines).
//!
//! Bonus cell:
//! - Cell (q): `Tag::Var` in `ArcTerminator::Invoke.ty` — terminator-operand
//!   coverage paralleling cell (m)'s instruction-operand shape.
//!
//! # Test Fixture Strategy
//!
//! Every test consumes helpers from `crate::test_helpers`. Block bodies and
//! terminators are constructed inline at the test site because the
//! instruction-operand and terminator-operand axes are local to the body of
//! the test (Test Fixture Strategy option (b)) — the helper covers the
//! `ArcFunction` skeleton; per-instruction customization stays in the test.

use std::collections::HashSet;

use ori_ir::StringInterner;
use ori_types::{Idx, Pool, Tag};

use super::super::{assert_no_unresolved_idx, assert_no_unresolved_type_vars};
use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, CtorKind};
use crate::test_helpers::{b, make_func, v};

/// Matrix cell (m) (body-walker): `ArcInstr::Construct.ty` carries `Tag::Var`;
/// `var_types[*]` + `params[*]` + `return_type` + `blocks[*].params[*].1`
/// fully resolved; empty exempt → `Err` with `var_id: ArcVarId(0)` (the
/// `dst` of the offending instruction per `validate.rs:178` exhaustive
/// `check_idx(*ty, *dst)` arm). Pins the canonical type-nominating
/// instruction; `Construct` always carries an `Idx` payload.
#[test]
fn test_tag_var_in_construct_ty_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let dst = v(0);
    let entry_block = ArcBlock {
        id: b(0),
        params: Vec::new(),
        body: vec![ArcInstr::Construct {
            dst,
            ty: leak,
            ctor: CtorKind::Tuple,
            args: Vec::new(),
        }],
        terminator: ArcTerminator::Unreachable,
    };
    let func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![entry_block],
        // Single SSA slot for `dst`; clean Idx isolates the violation to the
        // instruction operand.
        vec![Idx::INT],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var in Construct.ty must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(
        err.var_id, dst,
        "reported var_id matches Construct.dst (ArcVarId(0)) per exhaustive instruction-walk arm"
    );
}

/// Matrix cell (n) (body-walker): `ArcInstr::Apply.ty` carries `Tag::Var`; all
/// other type positions clean; empty exempt → `Err` with `var_id` matching
/// the Apply's `dst`. Pins a representative direct-call variant from the
/// exhaustive `Idx`-bearing instruction set; `Apply` is the most common
/// call-site shape.
#[test]
fn test_tag_var_in_apply_ty_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let dst = v(0);
    let callee = interner.intern("callee");
    let entry_block = ArcBlock {
        id: b(0),
        params: Vec::new(),
        body: vec![ArcInstr::Apply {
            dst,
            ty: leak,
            func: callee,
            args: Vec::new(),
            arg_ownership: Vec::new(),
            mono_instance_id: None,
        }],
        terminator: ArcTerminator::Unreachable,
    };
    let func = make_func(Vec::new(), Idx::UNIT, vec![entry_block], vec![Idx::INT]);

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var in Apply.ty must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(err.var_id, dst, "reported var_id matches Apply.dst");
}

/// Matrix cell (o) (body-walker): `ArcInstr::Project.ty` carries `Tag::Var`;
/// all other type positions clean; empty exempt → `Err` with `var_id`
/// matching the Project's `dst`. Pins a representative projection variant
/// from the exhaustive instruction set; `Project` shape (borrow with a
/// `value` operand + `field` index) differs structurally from `Apply`.
#[test]
fn test_tag_var_in_project_ty_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let dst = v(1);
    let source = v(0);
    // var_types[0] = source (clean Idx::INT); var_types[1] = dst (clean Idx::INT).
    // The only leak is on Project.ty so the walker must reach the body
    // after passing var_types / params / return / block-params clean.
    let entry_block = ArcBlock {
        id: b(0),
        params: Vec::new(),
        body: vec![ArcInstr::Project {
            dst,
            ty: leak,
            value: source,
            field: 0,
        }],
        terminator: ArcTerminator::Unreachable,
    };
    let func = make_func(
        Vec::new(),
        Idx::UNIT,
        vec![entry_block],
        vec![Idx::INT, Idx::INT],
    );

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var in Project.ty must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(err.var_id, dst, "reported var_id matches Project.dst");
}

/// Matrix cell (p) (body-walker thin-helper path): direct call of `assert_no_unresolved_idx` on
/// a synthetic `Tag::Var` `Idx` → `Err` with `var_id: ArcVarId::INVALID`
/// (the helper's sentinel — no owning SSA var) and `function` carrying the
/// caller-supplied site name. Pins the thin-helper path used by codegen
/// surfaces that hold a raw `type_idx` rather than a full `ArcFunction`
/// (derive synthesis, panic / iterator trampolines).
#[test]
fn test_assert_no_unresolved_idx_returns_err_on_tag_var() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let site = interner.intern("derive_site");

    let Err(err) = assert_no_unresolved_idx(&pool, leak, site) else {
        panic!("Tag::Var fed to assert_no_unresolved_idx must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(
        err.var_id,
        ArcVarId::INVALID,
        "thin helper reports INVALID sentinel — no owning SSA var at the call site"
    );
    assert_eq!(
        err.function, site,
        "thin helper carries the caller-supplied site name in `function`"
    );
}

/// Matrix cell (p) dual: direct call of `assert_no_unresolved_idx` on a
/// fully-resolved concrete primitive → `Ok(())`. Pins the positive face of
/// the thin-helper path so a future weakening that always returns `Err`
/// would still fail this test.
#[test]
fn test_assert_no_unresolved_idx_returns_ok_on_resolved_primitive() {
    let interner = StringInterner::new();
    let pool = Pool::new();
    let site = interner.intern("derive_site");

    let result = assert_no_unresolved_idx(&pool, Idx::INT, site);
    assert!(
        result.is_ok(),
        "fully-resolved concrete primitive must pass thin helper; got {result:?}"
    );
}

/// Matrix cell (q) (body-walker terminator axis, bonus): `ArcTerminator::Invoke.ty` carries
/// `Tag::Var`; all other type positions clean; empty exempt → `Err` with
/// `var_id` matching the terminator's `dst`. Pins the terminator-operand
/// axis of the walker extension. `Invoke` and `InvokeIndirect` are
/// the only `Idx`-bearing terminator variants today (per the exhaustive
/// arm at `validate.rs:193-194`). The walker still enforces forward-safety
/// on these carriers even when construction is post-2026 panic/effect
/// support.
#[test]
fn test_tag_var_in_invoke_terminator_ty_fails() {
    let interner = StringInterner::new();
    let mut pool = Pool::new();

    let leak = pool.fresh_var();
    let dst = v(0);
    let callee = interner.intern("invoke_callee");
    let entry_block = ArcBlock {
        id: b(0),
        params: Vec::new(),
        body: Vec::new(),
        // Invoke + normal/unwind targets reference unreachable sentinel
        // blocks because the walker does not traverse the CFG — it walks
        // every block's terminator regardless of reachability. Block IDs
        // are arbitrary `ArcBlockId` values and are not validated by this
        // walker (validation lives in `verify::check_block_connectivity`).
        terminator: ArcTerminator::Invoke {
            dst,
            ty: leak,
            func: callee,
            args: Vec::new(),
            arg_ownership: Vec::<ArgOwnership>::new(),
            mono_instance_id: None,
            normal: ArcBlockId::INVALID,
            unwind: ArcBlockId::INVALID,
        },
    };
    let func = make_func(Vec::new(), Idx::UNIT, vec![entry_block], vec![Idx::INT]);

    let exempt: HashSet<u32> = HashSet::new();
    let Err(err) = assert_no_unresolved_type_vars(&pool, &func, &interner, &exempt) else {
        panic!("Tag::Var in Invoke.ty must fail PC-2");
    };
    assert_eq!(err.tag, Tag::Var);
    assert_eq!(err.var_id, dst, "reported var_id matches Invoke.dst");
}
