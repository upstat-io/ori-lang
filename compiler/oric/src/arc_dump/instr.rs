//! Instruction and terminator formatting for the ARC IR dump.
//!
//! Each `ArcInstr` and `ArcTerminator` variant gets a single-line textual
//! representation following LLVM IR conventions.

use std::fmt::Write;

use ori_arc::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ArgOwnership, CtorKind, LitValue,
    PrimOp,
};
use ori_ir::StringInterner;
use ori_types::Pool;

use super::{fmt_strategy, fmt_var, fmt_var_typed};

/// Format a single ARC instruction.
///
/// Produces one line (no trailing newline). Value-producing instructions use
/// `%dst: type [repr] = ...` form. Side-effect instructions (RcInc/RcDec/Set)
/// omit the `=` prefix.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub fn fmt_instr(
    out: &mut String,
    instr: &ArcInstr,
    func: &ArcFunction,
    pool: &Pool,
    interner: &StringInterner,
) {
    match instr {
        ArcInstr::Let { dst, ty: _, value } => {
            write!(out, "{} = ", fmt_var_typed(func, *dst, pool, interner)).unwrap();
            fmt_value(out, value, interner);
        }

        ArcInstr::Apply {
            dst,
            func: callee,
            args,
            arg_ownership,
            ..
        } => {
            write!(
                out,
                "{} = Apply @{}",
                fmt_var_typed(func, *dst, pool, interner),
                interner.lookup(*callee)
            )
            .unwrap();
            fmt_args_with_ownership(out, args, arg_ownership, func);
        }

        ArcInstr::ApplyIndirect {
            dst, closure, args, ..
        } => {
            write!(
                out,
                "{} = ApplyIndirect {}",
                fmt_var_typed(func, *dst, pool, interner),
                fmt_var(func, *closure),
            )
            .unwrap();
            write!(out, "(").unwrap();
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                write!(out, "{}", fmt_var(func, *arg)).unwrap();
            }
            write!(out, ")").unwrap();
        }

        ArcInstr::PartialApply {
            dst,
            func: callee,
            args,
            ..
        } => {
            write!(
                out,
                "{} = PartialApply @{}",
                fmt_var_typed(func, *dst, pool, interner),
                interner.lookup(*callee),
            )
            .unwrap();
            fmt_args_simple(out, args, func);
        }

        ArcInstr::Project {
            dst, value, field, ..
        } => {
            write!(
                out,
                "{} = Project {}.{}",
                fmt_var_typed(func, *dst, pool, interner),
                fmt_var(func, *value),
                field,
            )
            .unwrap();
        }

        ArcInstr::Construct {
            dst, ctor, args, ..
        } => {
            write!(
                out,
                "{} = Construct {}",
                fmt_var_typed(func, *dst, pool, interner),
                fmt_ctor(ctor, interner),
            )
            .unwrap();
            fmt_args_simple(out, args, func);
        }

        // RC operations
        ArcInstr::RcInc {
            var,
            count,
            strategy,
        } => {
            write!(
                out,
                "RcInc {} [{}]",
                fmt_var(func, *var),
                fmt_strategy(*strategy),
            )
            .unwrap();
            if *count > 1 {
                write!(out, " x{count}").unwrap();
            }
        }

        ArcInstr::RcDec { var, strategy } => {
            write!(
                out,
                "RcDec {} [{}]",
                fmt_var(func, *var),
                fmt_strategy(*strategy),
            )
            .unwrap();
        }

        // Reuse operations
        ArcInstr::IsShared { dst, var } => {
            write!(
                out,
                "{} = IsShared {}",
                fmt_var(func, *dst),
                fmt_var(func, *var),
            )
            .unwrap();
        }

        ArcInstr::Set {
            base, field, value, ..
        } => {
            write!(
                out,
                "Set {}.{} = {}",
                fmt_var(func, *base),
                field,
                fmt_var(func, *value),
            )
            .unwrap();
        }

        ArcInstr::SetTag { base, tag } => {
            write!(out, "SetTag {}.tag = {tag}", fmt_var(func, *base)).unwrap();
        }

        ArcInstr::Reset { var, token } => {
            write!(
                out,
                "{} = Reset {}",
                fmt_var(func, *token),
                fmt_var(func, *var),
            )
            .unwrap();
        }

        ArcInstr::Reuse {
            token,
            dst,
            ctor,
            args,
            ..
        } => {
            write!(
                out,
                "{} = Reuse {} {}",
                fmt_var_typed(func, *dst, pool, interner),
                fmt_var(func, *token),
                fmt_ctor(ctor, interner),
            )
            .unwrap();
            fmt_args_simple(out, args, func);
        }

        ArcInstr::Select {
            dst,
            cond,
            true_val,
            false_val,
            ..
        } => {
            write!(
                out,
                "{} = Select {} ? {} : {}",
                fmt_var_typed(func, *dst, pool, interner),
                fmt_var(func, *cond),
                fmt_var(func, *true_val),
                fmt_var(func, *false_val),
            )
            .unwrap();
        }
    }
}

/// Format a block terminator.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub fn fmt_terminator(
    out: &mut String,
    term: &ArcTerminator,
    func: &ArcFunction,
    pool: &Pool,
    interner: &StringInterner,
) {
    match term {
        ArcTerminator::Return { value } => {
            write!(out, "Return {}", fmt_var(func, *value)).unwrap();
        }

        ArcTerminator::Jump { target, args } => {
            write!(out, "Jump bb{}", target.raw()).unwrap();
            if !args.is_empty() {
                fmt_args_simple(out, args, func);
            }
        }

        ArcTerminator::Branch {
            cond,
            then_block,
            else_block,
        } => {
            write!(
                out,
                "Branch {} ? bb{} : bb{}",
                fmt_var(func, *cond),
                then_block.raw(),
                else_block.raw(),
            )
            .unwrap();
        }

        ArcTerminator::Switch {
            scrutinee,
            cases,
            default,
        } => {
            write!(out, "Switch {} [", fmt_var(func, *scrutinee),).unwrap();
            for (i, (val, block)) in cases.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                write!(out, "{val} => bb{}", block.raw()).unwrap();
            }
            write!(out, ", default => bb{}]", default.raw()).unwrap();
        }

        ArcTerminator::Invoke {
            dst,
            func: callee,
            args,
            arg_ownership,
            normal,
            unwind,
            ..
        } => {
            write!(
                out,
                "{} = Invoke @{}",
                fmt_var_typed(func, *dst, pool, interner),
                interner.lookup(*callee),
            )
            .unwrap();
            fmt_args_with_ownership(out, args, arg_ownership, func);
            write!(out, " normal bb{} unwind bb{}", normal.raw(), unwind.raw()).unwrap();
        }

        ArcTerminator::Resume => {
            write!(out, "Resume").unwrap();
        }

        ArcTerminator::Unreachable => {
            write!(out, "Unreachable").unwrap();
        }
    }
}

// Private formatting helpers

/// Format a literal or variable value (rhs of a Let instruction).
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn fmt_value(out: &mut String, value: &ArcValue, interner: &StringInterner) {
    match value {
        ArcValue::Var(v) => write!(out, "%{}", v.raw()).unwrap(),
        ArcValue::Literal(lit) => fmt_literal(out, lit, interner),
        ArcValue::PrimOp { op, args } => {
            let op_str = match op {
                PrimOp::Binary(b) => b.as_symbol(),
                PrimOp::Unary(u) => match u {
                    ori_ir::UnaryOp::Neg => "-",
                    ori_ir::UnaryOp::Not => "!",
                    ori_ir::UnaryOp::BitNot => "~",
                    ori_ir::UnaryOp::Try => "?",
                },
            };
            if args.len() == 1 {
                write!(out, "{op_str}%{}", args[0].raw()).unwrap();
            } else if args.len() == 2 {
                write!(out, "%{} {op_str} %{}", args[0].raw(), args[1].raw()).unwrap();
            } else {
                write!(out, "PrimOp({op_str}").unwrap();
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(out, ", ").unwrap();
                    }
                    write!(out, "%{}", a.raw()).unwrap();
                }
                write!(out, ")").unwrap();
            }
        }
    }
}

/// Format a literal constant.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn fmt_literal(out: &mut String, lit: &LitValue, interner: &StringInterner) {
    match lit {
        LitValue::Int(n) => write!(out, "{n}").unwrap(),
        LitValue::Float(bits) => {
            let f = f64::from_bits(*bits);
            write!(out, "{f}").unwrap();
        }
        LitValue::Bool(b) => write!(out, "{b}").unwrap(),
        LitValue::String(name) => {
            let s = interner.lookup(*name);
            // Truncate long strings for readability
            if s.len() > 40 {
                write!(out, "\"{}...\"", &s[..37]).unwrap();
            } else {
                write!(out, "\"{s}\"").unwrap();
            }
        }
        LitValue::Char(c) => write!(out, "'{c}'").unwrap(),
        LitValue::Duration { value, unit } => {
            write!(out, "{value}{unit:?}").unwrap();
        }
        LitValue::Size { value, unit } => {
            write!(out, "{value}{unit:?}").unwrap();
        }
        LitValue::Unit => write!(out, "()").unwrap(),
    }
}

/// Format a constructor kind.
fn fmt_ctor(ctor: &CtorKind, interner: &StringInterner) -> String {
    match ctor {
        CtorKind::Struct(name) => format!("Struct({})", interner.lookup(*name)),
        CtorKind::EnumVariant { enum_name, variant } => {
            format!("Variant({}.{variant})", interner.lookup(*enum_name))
        }
        CtorKind::Tuple => "Tuple".to_string(),
        CtorKind::ListLiteral => "List".to_string(),
        CtorKind::MapLiteral => "Map".to_string(),
        CtorKind::SetLiteral => "Set".to_string(),
        CtorKind::Closure { func } => format!("Closure(@{})", interner.lookup(*func)),
    }
}

/// Format arguments as `(%0, %1, %2)`.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn fmt_args_simple(out: &mut String, args: &[ArcVarId], func: &ArcFunction) {
    write!(out, "(").unwrap();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            write!(out, ", ").unwrap();
        }
        write!(out, "{}", fmt_var(func, *arg)).unwrap();
    }
    write!(out, ")").unwrap();
}

/// Format arguments with per-argument ownership: `(%0 [own], %1 [borrow])`.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
fn fmt_args_with_ownership(
    out: &mut String,
    args: &[ArcVarId],
    arg_ownership: &[ArgOwnership],
    func: &ArcFunction,
) {
    write!(out, "(").unwrap();
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            write!(out, ", ").unwrap();
        }
        write!(out, "{}", fmt_var(func, *arg)).unwrap();
        if let Some(own) = arg_ownership.get(i) {
            let tag = match own {
                ArgOwnership::Owned => "own",
                ArgOwnership::Borrowed => "borrow",
            };
            write!(out, " [{tag}]").unwrap();
        }
    }
    write!(out, ")").unwrap();
}
