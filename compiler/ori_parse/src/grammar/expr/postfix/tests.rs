//! Postfix-expression parsing regression pins.

mod postfix_op_table_tests {
    //! Pins the single-source-of-truth link between `PostfixOp::from_tag`, the
    //! `apply_postfix_ops` dispatch, and the derived `POSTFIX_BITSET` — the
    //! postfix analogue of the `operators.rs` `OPER_TABLE` exhaustiveness pin.

    use super::super::{is_postfix_tag, PostfixOp, POSTFIX_BITSET};

    /// `start_tag` and `from_tag` are inverses for every variant: each op's
    /// start tag resolves back to that exact op.
    #[test]
    fn from_tag_roundtrips_every_variant() {
        for op in PostfixOp::ALL {
            assert_eq!(
                PostfixOp::from_tag(op.start_tag()),
                Some(op),
                "from_tag must invert start_tag for {op:?}",
            );
        }
    }

    /// Every variant's start tag is set in the derived bitset — the fast-exit
    /// gate cannot drift from the dispatch.
    #[test]
    fn bitset_covers_every_variant() {
        for op in PostfixOp::ALL {
            assert!(
                is_postfix_tag(op.start_tag()),
                "POSTFIX_BITSET must include the start tag of {op:?}",
            );
        }
    }

    /// The bitset has EXACTLY the variant start tags set — no extra bit, no
    /// missing bit. Counts set bits and compares to the variant count.
    #[test]
    fn bitset_has_exactly_variant_tags() {
        let set_bits: u32 = POSTFIX_BITSET.iter().map(|w| w.count_ones()).sum();
        assert_eq!(
            set_bits as usize,
            PostfixOp::ALL.len(),
            "POSTFIX_BITSET set-bit count must equal the PostfixOp variant count",
        );
    }
}

#[expect(clippy::expect_used, reason = "Tests use expect for brevity")]
mod method_call_turbofish_tests {
    //! Regression pins for method-call call-site turbofish parsing.
    //!
    //! A method-call turbofish (`recv.method<T>(args)`) parses to a
    //! `MethodCall` / `MethodCallNamed` node carrying the parsed type-args in the
    //! `method_call_type_args` side-table — NOT a `<`/`>` comparison chain.
    //! Negative pins lock the comparison reading for a method/field access followed
    //! by a genuine `<` / `>` comparison.
    //!
    //! Spec: grammar.ebnf call-site `type_args`; proposal
    //! `call-site-method-generics-grammar-alignment-proposal.md` (Disambiguation).

    use crate::parse;
    use ori_ir::{BinaryOp, ExprId, ExprKind, StringInterner};

    /// Parse a source module; assert no parse errors; return interner + output.
    fn parse_ok(source: &str) -> (StringInterner, crate::ParseOutput) {
        let interner = StringInterner::new();
        let tokens = ori_lexer::lex(source, &interner);
        let output = parse(&tokens, &interner);
        assert!(
            output.errors.is_empty(),
            "unexpected parse errors: {:?}",
            output.errors
        );
        (interner, output)
    }

    /// Find the method-call node (`MethodCall` / `MethodCallNamed`) whose method
    /// selector is `method`, returning its `ExprId` + recorded type-arg count.
    fn find_method_call(
        interner: &StringInterner,
        output: &crate::ParseOutput,
        method: &str,
    ) -> Option<(ExprId, usize)> {
        let want = interner.intern(method);
        let count = u32::try_from(output.arena.expr_count()).expect("expr count fits in u32");
        for i in 0..count {
            let id = ExprId::new(i);
            let (ExprKind::MethodCall { method, .. } | ExprKind::MethodCallNamed { method, .. }) =
                output.arena.get_expr(id).kind
            else {
                continue;
            };
            if method == want {
                return Some((id, output.arena.method_call_type_args(id).len()));
            }
        }
        None
    }

    /// Assert at least one `<`/`>`/`<=`/`>=` comparison node exists.
    fn assert_has_comparison(output: &crate::ParseOutput) {
        let mut saw = false;
        let count = u32::try_from(output.arena.expr_count()).expect("expr count fits in u32");
        for i in 0..count {
            if let ExprKind::Binary { op, .. } = output.arena.get_expr(ExprId::new(i)).kind {
                if matches!(
                    op,
                    BinaryOp::Lt | BinaryOp::Gt | BinaryOp::LtEq | BinaryOp::GtEq
                ) {
                    saw = true;
                }
            }
        }
        assert!(saw, "expected a comparison node, found none");
    }

    // Positive pins: method-call turbofish parses to a method call

    #[test]
    fn method_type_turbofish_parses_with_type_args() {
        // `b.pick<int>(item: 5)` — the filed-bug symptom shape (now fixed).
        let (it, output) = parse_ok(concat!(
            "type Boxer = { tag: int }\n",
            "impl Boxer { @pick<T> (self, item: T) -> T = item; }\n",
            "@main () -> int = {\n    let $b = Boxer { tag: 0 };\n    b.pick<int>(item: 5)\n}\n",
        ));
        let (_, argc) = find_method_call(&it, &output, "pick").expect("MethodCall for `pick`");
        assert_eq!(
            argc, 1,
            "one call-site type arg recorded on the method call"
        );
    }

    #[test]
    fn method_const_turbofish_parses_with_type_args() {
        // `b.cap<2>()` — const call-site arg (bare integer literal) on a method call;
        // a distinct disambiguation case from the type-turbofish pin above.
        let (it, output) = parse_ok(concat!(
            "type Box = { n: int }\n",
            "impl Box { @cap<$N: int> (self) -> int = N; }\n",
            "@main () -> int = {\n    let $b = Box { n: 0 };\n    b.cap<2>()\n}\n",
        ));
        let (_, argc) = find_method_call(&it, &output, "cap").expect("MethodCall for `cap`");
        assert_eq!(
            argc, 1,
            "one const call-site type arg recorded on the method call"
        );
    }

    // Negative pins: method/field then comparison stays comparison

    #[test]
    fn method_call_then_less_than_stays_comparison() {
        // `xs.len() < n` — method-call then `<` MUST parse as comparison.
        let (it, output) = parse_ok(
            "@main () -> bool = {\n    let $xs = [1, 2, 3];\n    let $n = 5;\n    xs.len() < n\n}\n",
        );
        assert_has_comparison(&output);
        let (_, argc) = find_method_call(&it, &output, "len").expect("MethodCall for `len`");
        assert_eq!(argc, 0, "the `len()` call carries no call-site type args");
    }

    #[test]
    fn field_access_then_greater_than_stays_comparison() {
        // `p.foo > y` — field access then `>` MUST parse as comparison.
        let (_it, output) = parse_ok(concat!(
            "type P = { foo: int }\n",
            "@main () -> bool = {\n    let $p = P { foo: 3 };\n    let $y = 5;\n    p.foo > y\n}\n",
        ));
        assert_has_comparison(&output);
    }

    // Primary-position type-path turbofish: `Type<args>.method()`

    #[test]
    fn type_path_turbofish_assoc_fn_parses() {
        // `Box<int>.new(v: 5)` — type-args on a primary-position type name,
        // committed because a `.` immediately follows the balanced `>`.
        let (it, output) = parse_ok(concat!(
            "type Box<T> = { value: T }\n",
            "impl<T> Box<T> { @new (v: T) -> Box<T> = Box { value: v }; }\n",
            "@main () -> int = {\n    let r: Box<int> = Box<int>.new(v: 5);\n    r.value\n}\n",
        ));
        find_method_call(&it, &output, "new").expect("MethodCall for `new`");
    }

    #[test]
    fn bare_type_path_assoc_fn_still_parses() {
        // Regression: the bare associated-function form keeps parsing.
        let (it, output) = parse_ok(concat!(
            "type Box<T> = { value: T }\n",
            "impl<T> Box<T> { @new (v: T) -> Box<T> = Box { value: v }; }\n",
            "@main () -> int = {\n    let r: Box<int> = Box.new(v: 9);\n    r.value\n}\n",
        ));
        find_method_call(&it, &output, "new").expect("MethodCall for `new`");
    }

    #[test]
    fn primary_ident_less_than_stays_comparison() {
        // `a < b` in primary position, no trailing `.` — stays comparison; the
        // speculative type-arg path must restore.
        let (_it, output) =
            parse_ok("@main () -> bool = {\n    let $a = 3;\n    let $b = 5;\n    a < b\n}\n");
        assert_has_comparison(&output);
    }
}
