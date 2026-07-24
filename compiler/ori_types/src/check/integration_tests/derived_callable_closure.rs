//! Closed callable targets required transitively by structural builtin methods.

use super::support::*;

#[test]
fn list_compare_publishes_each_generic_derived_element_target() {
    let result = check_source(
        r"
#derive(Eq, Comparable) type Pair<A, B> = { a: A, b: B }

@cmp_int_str (xs: [Pair<int, str>], ys: [Pair<int, str>]) -> Ordering =
    xs.compare(other: ys);

@cmp_str_int (xs: [Pair<str, int>], ys: [Pair<str, int>]) -> Ordering =
    xs.compare(other: ys);
",
    );
    assert!(
        !result.has_errors(),
        "structural compare program must type-check: {:?}",
        result.error_kinds()
    );

    let accepted = result
        .result
        .typed
        .accepted_derives
        .iter()
        .find(|accepted| accepted.trait_kind == ori_ir::DerivedTrait::Comparable)
        .expect("Comparable derive must be accepted");
    let producer = crate::MethodProducer::Derived(accepted.id);
    let all_instances: Vec<_> = result
        .result
        .typed
        .mono_instances
        .iter()
        .filter(|instance| instance.method_producer.as_ref() == Some(&producer))
        .collect();

    for args in [[Idx::INT, Idx::STR], [Idx::STR, Idx::INT]] {
        let receiver = result
            .find_applied("Pair", &args)
            .expect("concrete Pair instantiation must exist");
        let instances: Vec<_> = result
            .result
            .typed
            .mono_instances
            .iter()
            .filter(|instance| {
                instance.method_producer.as_ref() == Some(&producer)
                    && instance.receiver_type == Some(receiver)
            })
            .collect();

        assert_eq!(
            instances.len(),
            1,
            "List.compare must publish one exact derived element target for {args:?}: {instances:?}; all Comparable instances: {all_instances:?}; plans: {:?}",
            result.result.typed.derived_call_plans,
        );
        assert_eq!(
            instances[0].impl_args,
            args.into_iter()
                .map(crate::GenericArg::Type)
                .collect::<Vec<_>>()
        );
        assert!(
            result
                .result
                .typed
                .derived_call_plans
                .iter()
                .any(|plan| plan.derived == accepted.id && plan.binder_substitutions == args),
            "the exact derived element body must have a frozen call plan for {args:?}"
        );
    }
}
