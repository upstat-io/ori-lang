//! Pattern and type formatting for AST phase dumps.
//!
//! Contains inline formatters for binding patterns, match patterns,
//! and parsed type annotations.

use std::fmt::Write;

use ori_ir::ast::{BindingPattern, MatchPattern, Mutability};
use ori_ir::{ExprArena, Name, StringInterner};

use super::expr::dump_expr_inline;

/// Dump a binding pattern inline.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(crate) fn dump_binding_pattern(
    out: &mut String,
    pattern: &BindingPattern,
    interner: &StringInterner,
) {
    match pattern {
        BindingPattern::Name { name, mutable } => {
            let prefix = match mutable {
                Mutability::Immutable => "$",
                Mutability::Mutable => "",
            };
            write!(out, "{prefix}{}", interner.lookup(*name)).unwrap();
        }
        BindingPattern::Tuple(pats) => {
            write!(out, "(").unwrap();
            for (i, p) in pats.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                dump_binding_pattern(out, p, interner);
            }
            write!(out, ")").unwrap();
        }
        BindingPattern::Struct { fields, .. } => {
            write!(out, "{{ ").unwrap();
            for (i, f) in fields.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                let fname = interner.lookup(f.name);
                if let Some(ref pat) = f.pattern {
                    write!(out, "{fname}: ").unwrap();
                    dump_binding_pattern(out, pat, interner);
                } else {
                    write!(out, "{fname}").unwrap();
                }
            }
            write!(out, " }}").unwrap();
        }
        BindingPattern::List { elements, rest, .. } => {
            write!(out, "[").unwrap();
            for (i, p) in elements.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                dump_binding_pattern(out, p, interner);
            }
            if let Some((name, _)) = rest {
                if !elements.is_empty() {
                    write!(out, ", ").unwrap();
                }
                write!(out, "...{}", interner.lookup(*name)).unwrap();
            }
            write!(out, "]").unwrap();
        }
        BindingPattern::Wildcard => write!(out, "_").unwrap(),
    }
}

/// Dump a match pattern inline.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(crate) fn dump_match_pattern(
    out: &mut String,
    pattern: &MatchPattern,
    arena: &ExprArena,
    interner: &StringInterner,
) {
    match pattern {
        MatchPattern::Wildcard => write!(out, "_").unwrap(),
        MatchPattern::Binding(name) => write!(out, "{}", interner.lookup(*name)).unwrap(),
        MatchPattern::Literal(id) => dump_expr_inline(out, *id, arena, interner),
        MatchPattern::Variant { name, inner } => {
            write!(out, "{}", interner.lookup(*name)).unwrap();
            let pats = arena.get_match_pattern_list(*inner);
            if !pats.is_empty() {
                write!(out, "(").unwrap();
                for (i, pid) in pats.iter().enumerate() {
                    if i > 0 {
                        write!(out, ", ").unwrap();
                    }
                    dump_match_pattern(out, arena.get_match_pattern(*pid), arena, interner);
                }
                write!(out, ")").unwrap();
            }
        }
        MatchPattern::Tuple(pats) => {
            write!(out, "(").unwrap();
            for (i, pid) in arena.get_match_pattern_list(*pats).iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                dump_match_pattern(out, arena.get_match_pattern(*pid), arena, interner);
            }
            write!(out, ")").unwrap();
        }
        MatchPattern::Or(pats) => {
            for (i, pid) in arena.get_match_pattern_list(*pats).iter().enumerate() {
                if i > 0 {
                    write!(out, " | ").unwrap();
                }
                dump_match_pattern(out, arena.get_match_pattern(*pid), arena, interner);
            }
        }
        MatchPattern::Struct { fields, rest, .. } => {
            write!(out, "{{ ").unwrap();
            for (i, (name, sub)) in fields.iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                write!(out, "{}", interner.lookup(*name)).unwrap();
                if let Some(pid) = sub {
                    write!(out, ": ").unwrap();
                    dump_match_pattern(out, arena.get_match_pattern(*pid), arena, interner);
                }
            }
            if *rest {
                write!(out, ", ..").unwrap();
            }
            write!(out, " }}").unwrap();
        }
        MatchPattern::List { elements, rest } => {
            write!(out, "[").unwrap();
            for (i, pid) in arena.get_match_pattern_list(*elements).iter().enumerate() {
                if i > 0 {
                    write!(out, ", ").unwrap();
                }
                dump_match_pattern(out, arena.get_match_pattern(*pid), arena, interner);
            }
            if let Some(name) = rest {
                write!(out, ", ...{}", interner.lookup(*name)).unwrap();
            }
            write!(out, "]").unwrap();
        }
        MatchPattern::Range {
            start,
            end,
            inclusive,
        } => {
            if let Some(s) = start {
                dump_expr_inline(out, *s, arena, interner);
            }
            if *inclusive {
                write!(out, "..=").unwrap();
            } else {
                write!(out, "..").unwrap();
            }
            if let Some(e) = end {
                dump_expr_inline(out, *e, arena, interner);
            }
        }
        MatchPattern::At { name, pattern } => {
            write!(out, "{} @ ", interner.lookup(*name)).unwrap();
            dump_match_pattern(out, arena.get_match_pattern(*pattern), arena, interner);
        }
    }
}

/// Format a parsed type for display.
pub(crate) fn format_parsed_type(
    ty: &ori_ir::ParsedType,
    arena: &ExprArena,
    interner: &StringInterner,
) -> String {
    use ori_ir::ParsedType;
    match ty {
        ParsedType::Primitive(tid) => format!("{tid:?}"),
        ParsedType::Named { name, type_args } => {
            let n = interner.lookup(*name);
            if type_args.is_empty() {
                n.to_string()
            } else {
                let args: Vec<String> = arena
                    .get_parsed_type_list(*type_args)
                    .iter()
                    .map(|tid| format_parsed_type(arena.get_parsed_type(*tid), arena, interner))
                    .collect();
                format!("{n}<{}>", args.join(", "))
            }
        }
        ParsedType::List(elem) => {
            format!(
                "[{}]",
                format_parsed_type(arena.get_parsed_type(*elem), arena, interner)
            )
        }
        ParsedType::FixedList { elem, .. } => {
            format!(
                "[{}, max ...]",
                format_parsed_type(arena.get_parsed_type(*elem), arena, interner)
            )
        }
        ParsedType::Map { key, value } => {
            let k = format_parsed_type(arena.get_parsed_type(*key), arena, interner);
            let v = format_parsed_type(arena.get_parsed_type(*value), arena, interner);
            format!("{{{k}: {v}}}")
        }
        ParsedType::Tuple(elements) => {
            let elems: Vec<String> = arena
                .get_parsed_type_list(*elements)
                .iter()
                .map(|tid| format_parsed_type(arena.get_parsed_type(*tid), arena, interner))
                .collect();
            format!("({})", elems.join(", "))
        }
        ParsedType::Function { params, ret } => {
            let ps: Vec<String> = arena
                .get_parsed_type_list(*params)
                .iter()
                .map(|tid| format_parsed_type(arena.get_parsed_type(*tid), arena, interner))
                .collect();
            let r = format_parsed_type(arena.get_parsed_type(*ret), arena, interner);
            format!("({}) -> {r}", ps.join(", "))
        }
        ParsedType::Infer => "_".to_string(),
        ParsedType::SelfType => "Self".to_string(),
        ParsedType::AssociatedType {
            base, assoc_name, ..
        } => {
            let base_str = format_parsed_type(arena.get_parsed_type(*base), arena, interner);
            let name = interner.lookup(*assoc_name);
            format!("{base_str}.{name}")
        }
        ParsedType::ConstExpr(_) => "$const".to_string(),
        ParsedType::TraitBounds(bounds) => {
            let bound_strs: Vec<String> = arena
                .get_parsed_type_list(*bounds)
                .iter()
                .map(|tid| format_parsed_type(arena.get_parsed_type(*tid), arena, interner))
                .collect();
            bound_strs.join(" + ")
        }
    }
}

/// Format a label name (empty → "", non-empty → ":name").
pub(crate) fn format_label(label: Name, interner: &StringInterner) -> String {
    if label == Name::EMPTY {
        String::new()
    } else {
        format!(":{}", interner.lookup(label))
    }
}
