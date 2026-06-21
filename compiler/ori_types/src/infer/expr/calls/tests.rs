use super::suggest_iterator_fix;
use crate::Tag;

// Regression:
// `suggest_iterator_fix` is the tag-specialized hint helper used by the
// flat_map closure-return diagnostic. Types whose values can be turned into
// an iterator with a method call (`.iter()`) get a concrete suggestion;
// other types get a generic message because there is no canonical wrapping.

#[test]
fn suggest_iterator_fix_for_list_recommends_iter() {
    let s = suggest_iterator_fix(Tag::List);
    assert!(
        s.message.contains(".iter()"),
        "List return should suggest `.iter()`, got: {}",
        s.message
    );
}

#[test]
fn suggest_iterator_fix_for_int_uses_generic_message() {
    let s = suggest_iterator_fix(Tag::Int);
    assert!(
        !s.message.contains(".iter()"),
        "int return should NOT suggest `.iter()` (no canonical wrap), got: {}",
        s.message
    );
    assert!(
        s.message.contains("flat_map"),
        "generic suggestion should name flat_map, got: {}",
        s.message
    );
}

#[test]
fn suggest_iterator_fix_for_struct_uses_generic_message() {
    // Tag::Struct represents user-defined struct types. Without knowing the
    // struct's API, no `.iter()` suggestion is appropriate.
    let s = suggest_iterator_fix(Tag::Struct);
    assert!(
        !s.message.contains(".iter()"),
        "struct return should NOT suggest `.iter()`, got: {}",
        s.message
    );
}

#[test]
fn suggest_iterator_fix_for_option_recommends_iter() {
    // Option is in the suggestion-friendly set —
    // `Option<T>::iter()` exists and yields a 0-or-1-element iterator.
    let s = suggest_iterator_fix(Tag::Option);
    assert!(
        s.message.contains(".iter()"),
        "Option return should suggest `.iter()`, got: {}",
        s.message
    );
}

// Sanity coverage for the remaining suggestion-friendly tags so the matrix
// is fully clamped (any future re-categorization breaks one of these).
#[test]
fn suggest_iterator_fix_full_friendly_set() {
    for friendly in [Tag::Set, Tag::Map, Tag::Str, Tag::Range] {
        let s = suggest_iterator_fix(friendly);
        assert!(
            s.message.contains(".iter()"),
            "tag {:?} should be in the suggestion-friendly set, got: {}",
            friendly,
            s.message
        );
    }
}

#[test]
fn suggest_iterator_fix_tuple_uses_generic_message() {
    // Tuple is intentionally NOT in the suggestion-friendly set —
    // tuples have no `.iter()` method.
    let s = suggest_iterator_fix(Tag::Tuple);
    assert!(
        !s.message.contains(".iter()"),
        "Tuple return should NOT suggest `.iter()`, got: {}",
        s.message
    );
}

// Tag::Map receiver projection in `unify_closure_param_with_iterator_elem`.
// The helper is the SSOT for receiver -> lambda-param propagation; these cells
// exercise it (and both dispatching call paths) directly with constructed types.

mod map_receiver_lambda_param {
    use super::super::closure_unify::{
        unify_closure_param_with_iterator_elem, unify_flat_map_constraints,
        unify_higher_order_constraints,
    };
    use crate::{Idx, Pool, Tag};
    use ori_ir::Span;

    #[test]
    fn test_lambda_param_from_map_receiver_propagates_kv_tuple() {
        let mut pool = Pool::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        let map_ty = engine.pool_mut().map(Idx::STR, Idx::INT);
        let param_var = engine.pool_mut().fresh_var();
        let ret_var = engine.pool_mut().fresh_var();
        let closure = engine.pool_mut().function1(param_var, ret_var);

        unify_closure_param_with_iterator_elem(&mut engine, closure, map_ty);

        let resolved = engine.resolve(param_var);
        let expected = engine.pool_mut().tuple(&[Idx::STR, Idx::INT]);
        assert_eq!(
            resolved, expected,
            "Map receiver must bind the closure param to the (K, V) tuple"
        );
    }

    #[test]
    fn test_lambda_param_from_map_via_higher_order_map_path() {
        // `unify_higher_order_constraints` "map" arm — the `.map(...)` call path.
        let interner = ori_ir::StringInterner::new();
        let mut pool = Pool::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        engine.set_interner(&interner);
        let map_ty = engine.pool_mut().map(Idx::STR, Idx::INT);
        let param_var = engine.pool_mut().fresh_var();
        let closure = engine.pool_mut().function1(param_var, Idx::STR);
        let elem_var = engine.pool_mut().fresh_var();
        let ret_iter = engine.pool_mut().iterator(elem_var);
        let method = engine.intern_name("map");

        unify_higher_order_constraints(
            &mut engine,
            method.unwrap_or_else(|| unreachable!("interner attached above")),
            ret_iter,
            map_ty,
            &[closure],
            &[Span::new(0, 0)],
            Span::new(0, 0),
        );

        let resolved = engine.resolve(param_var);
        let expected = engine.pool_mut().tuple(&[Idx::STR, Idx::INT]);
        assert_eq!(
            resolved, expected,
            ".map dispatch must bind the closure param to (K, V)"
        );
    }

    #[test]
    fn test_lambda_param_from_map_via_flat_map_path() {
        // `unify_flat_map_constraints` — the `.flat_map(...)` call path; the
        // shared helper cures BOTH SSOT call sites in one fix.
        let mut pool = Pool::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        let map_ty = engine.pool_mut().map(Idx::STR, Idx::INT);
        let param_var = engine.pool_mut().fresh_var();
        let inner_iter = engine.pool_mut().iterator(Idx::STR);
        let closure = engine.pool_mut().function1(param_var, inner_iter);
        let elem_var = engine.pool_mut().fresh_var();
        let ret_iter = engine.pool_mut().iterator(elem_var);

        unify_flat_map_constraints(
            &mut engine,
            ret_iter,
            map_ty,
            &[closure],
            &[Span::new(0, 0)],
            Span::new(0, 0),
        );

        let resolved = engine.resolve(param_var);
        let expected = engine.pool_mut().tuple(&[Idx::STR, Idx::INT]);
        assert_eq!(
            resolved, expected,
            ".flat_map dispatch must bind the closure param to (K, V)"
        );
        let resolved_elem = engine.resolve(elem_var);
        assert_eq!(
            resolved_elem,
            Idx::STR,
            "flat_map result elem must unify with the closure's inner iterator elem"
        );
    }

    #[test]
    fn test_lambda_param_from_iterator_receiver_unchanged_by_map_arm() {
        // Negative pin (Matrix Clamping): an Iterator<int>
        // receiver routes the `is_iterator()` branch; the param resolves to
        // int, NOT a synthetic tuple. Widening the Map arm to match
        // `Tag::Iterator` breaks this clamp.
        let mut pool = Pool::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        let iter_ty = engine.pool_mut().iterator(Idx::INT);
        assert_eq!(engine.pool().tag(iter_ty), Tag::Iterator);
        let param_var = engine.pool_mut().fresh_var();
        let closure = engine.pool_mut().function1(param_var, Idx::INT);

        unify_closure_param_with_iterator_elem(&mut engine, closure, iter_ty);

        assert_eq!(
            engine.resolve(param_var),
            Idx::INT,
            "iterator receiver must bind the closure param to the element type"
        );
    }
}

// fold/rfold receiver-tag parity via the shared `receiver_element_type`
// SSOT. The `fold`/`rfold` arm binds the closure's SECOND param (the element)
// to the receiver element. These cells pin the `Tag::Map` ((K, V) tuple) +
// `Tag::Str` (char) coverage that previously diverged from
// `unify_closure_param_with_iterator_elem`, plus a `Tag::List` regression cell
// and a non-element negative pin.
mod fold_rfold_receiver_lambda_param {
    use super::super::closure_unify::unify_higher_order_constraints;
    use crate::{Idx, Pool, Tag};
    use ori_ir::{Span, StringInterner};

    /// Drive the `fold`/`rfold` arm: closure is `(acc, elem) -> acc`; returns
    /// the post-unification resolved type of the `elem` (second) param.
    fn resolve_fold_elem_param(method_name: &str, receiver_ty_tag: ReceiverShape) -> ResolvedElem {
        let interner = StringInterner::new();
        let mut pool = Pool::new();
        let mut engine = crate::InferEngine::new(&mut pool);
        engine.set_interner(&interner);
        let receiver_ty = receiver_ty_tag.build(&mut engine);
        let acc_param = engine.pool_mut().fresh_var();
        let elem_param = engine.pool_mut().fresh_var();
        let acc_ret = engine.pool_mut().fresh_var();
        let closure = engine.pool_mut().function2(acc_param, elem_param, acc_ret);
        let ret_ty = engine.pool_mut().fresh_var();
        let method = engine
            .intern_name(method_name)
            .unwrap_or_else(|| unreachable!("interner attached above"));

        unify_higher_order_constraints(
            &mut engine,
            method,
            ret_ty,
            receiver_ty,
            &[Idx::INT, closure],
            &[Span::new(0, 0), Span::new(0, 0)],
            Span::new(0, 0),
        );

        let resolved = engine.resolve(elem_param);
        ResolvedElem {
            idx: resolved,
            tag: engine.pool().tag(resolved),
            kv_tuple: engine.pool_mut().tuple(&[Idx::STR, Idx::INT]),
        }
    }

    enum ReceiverShape {
        MapStrInt,
        Str,
        ListInt,
        NonElement,
    }

    impl ReceiverShape {
        fn build(self, engine: &mut crate::InferEngine<'_>) -> Idx {
            match self {
                ReceiverShape::MapStrInt => engine.pool_mut().map(Idx::STR, Idx::INT),
                ReceiverShape::Str => Idx::STR,
                ReceiverShape::ListInt => engine.pool_mut().list(Idx::INT),
                ReceiverShape::NonElement => Idx::INT,
            }
        }
    }

    struct ResolvedElem {
        idx: Idx,
        tag: Tag,
        kv_tuple: Idx,
    }

    #[test]
    fn test_fold_second_param_from_map_receiver_resolves_kv_tuple() {
        let r = resolve_fold_elem_param("fold", ReceiverShape::MapStrInt);
        assert_eq!(
            r.idx, r.kv_tuple,
            "fold on a Map receiver must bind the element param to the (K, V) tuple"
        );
    }

    #[test]
    fn test_rfold_second_param_from_map_receiver_resolves_kv_tuple() {
        let r = resolve_fold_elem_param("rfold", ReceiverShape::MapStrInt);
        assert_eq!(
            r.idx, r.kv_tuple,
            "rfold dispatches the same arm — Map element param must be (K, V)"
        );
    }

    #[test]
    fn test_fold_second_param_from_str_receiver_resolves_char() {
        let r = resolve_fold_elem_param("fold", ReceiverShape::Str);
        assert_eq!(
            r.idx,
            Idx::CHAR,
            "fold on a Str receiver must bind the element param to char"
        );
    }

    #[test]
    fn test_fold_second_param_from_list_receiver_resolves_elem() {
        // Regression cell: List coverage pre-dated the parity fix and must
        // survive the refactor to the shared `receiver_element_type` SSOT.
        let r = resolve_fold_elem_param("fold", ReceiverShape::ListInt);
        assert_eq!(
            r.idx,
            Idx::INT,
            "fold on a List<int> receiver must bind the element param to int"
        );
    }

    #[test]
    fn test_fold_second_param_from_non_element_receiver_stays_unresolved() {
        // Negative pin: a primitive `int` receiver has no element shape, so
        // `receiver_element_type` returns None and the element param stays an
        // unconstrained Var. Widening the SSOT to bind a non-collection
        // receiver breaks this clamp.
        let r = resolve_fold_elem_param("fold", ReceiverShape::NonElement);
        assert_eq!(
            r.tag,
            Tag::Var,
            "non-element receiver must leave the fold element param unresolved"
        );
    }
}
