use super::*;

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

#[test]
fn test_new_env_is_empty() {
    let env = TypeEnv::new();
    assert!(env.lookup(name(1)).is_none());
    assert!(!env.is_bound_locally(name(1)));
}

#[test]
fn test_bind_and_lookup() {
    let mut env = TypeEnv::new();
    env.bind(name(1), Idx::INT);

    assert_eq!(env.lookup(name(1)), Some(Idx::INT));
    assert!(env.lookup(name(2)).is_none());
}

#[test]
fn test_child_scope_shadows_parent() {
    let mut parent = TypeEnv::new();
    parent.bind(name(1), Idx::INT);

    let mut child = parent.child();
    child.bind(name(1), Idx::BOOL);

    // Child sees shadowed value
    assert_eq!(child.lookup(name(1)), Some(Idx::BOOL));
    // Parent still has original
    assert_eq!(parent.lookup(name(1)), Some(Idx::INT));
}

#[test]
fn test_child_sees_parent_bindings() {
    let mut parent = TypeEnv::new();
    parent.bind(name(1), Idx::INT);

    let child = parent.child();

    // Child can see parent's bindings
    assert_eq!(child.lookup(name(1)), Some(Idx::INT));
}

#[test]
fn test_is_bound_locally() {
    let mut parent = TypeEnv::new();
    parent.bind(name(1), Idx::INT);

    let child = parent.child();

    // name(1) is in parent, not local to child
    assert!(!child.is_bound_locally(name(1)));
    assert!(parent.is_bound_locally(name(1)));
}

#[test]
fn test_names_iterator() {
    let mut parent = TypeEnv::new();
    parent.bind(name(1), Idx::INT);
    parent.bind(name(2), Idx::BOOL);

    let mut child = parent.child();
    child.bind(name(3), Idx::STR);

    let names: Vec<Name> = child.names().collect();

    assert!(names.contains(&name(1)));
    assert!(names.contains(&name(2)));
    assert!(names.contains(&name(3)));
    assert_eq!(names.len(), 3);
}

#[test]
fn test_local_count() {
    let mut env = TypeEnv::new();
    assert_eq!(env.local_count(), 0);

    env.bind(name(1), Idx::INT);
    assert_eq!(env.local_count(), 1);

    env.bind(name(2), Idx::BOOL);
    assert_eq!(env.local_count(), 2);
}

#[test]
fn test_parent() {
    let parent = TypeEnv::new();
    let child = parent.child();

    assert!(parent.parent().is_none());
    assert!(child.parent().is_some());
}

// ====================================================================
// Edit distance tests (uses crate::edit_distance from type_error/diff.rs)
// ====================================================================

use crate::edit_distance;

#[test]
fn test_edit_distance_identical() {
    assert_eq!(edit_distance("hello", "hello"), 0);
    assert_eq!(edit_distance("", ""), 0);
}

#[test]
fn test_edit_distance_empty() {
    assert_eq!(edit_distance("hello", ""), 5);
    assert_eq!(edit_distance("", "world"), 5);
}

#[test]
fn test_edit_distance_single_edit() {
    assert_eq!(edit_distance("abc", "adc"), 1); // substitution
    assert_eq!(edit_distance("abc", "abcd"), 1); // insertion
    assert_eq!(edit_distance("abcd", "abc"), 1); // deletion
}

#[test]
fn test_edit_distance_typos() {
    assert_eq!(edit_distance("lenght", "length"), 2); // transposition (2 edits in Levenshtein)
    assert_eq!(edit_distance("helo", "hello"), 1); // missing char
    assert_eq!(edit_distance("mpa", "map"), 2); // transposition
}

#[test]
fn test_default_threshold() {
    assert_eq!(default_threshold(0), 0);
    assert_eq!(default_threshold(1), 1);
    assert_eq!(default_threshold(2), 1);
    assert_eq!(default_threshold(3), 2);
    assert_eq!(default_threshold(5), 2);
    assert_eq!(default_threshold(6), 3);
    assert_eq!(default_threshold(10), 3);
}

// ====================================================================
// find_similar tests
// ====================================================================

/// Create a simple resolver mapping Name(raw) -> &str.
fn make_resolver<'a>(names: &'a [(u32, &'a str)]) -> impl Fn(Name) -> Option<&'a str> + 'a {
    move |n: Name| {
        names
            .iter()
            .find(|(id, _)| Name::from_raw(*id) == n)
            .map(|(_, s)| *s)
    }
}

#[test]
fn test_find_similar_basic_typo() {
    let mut env = TypeEnv::new();
    // Bind "length", "height", "width"
    env.bind(name(1), Idx::INT); // "length"
    env.bind(name(2), Idx::INT); // "height"
    env.bind(name(3), Idx::INT); // "width"

    let resolver = make_resolver(&[(1, "length"), (2, "height"), (3, "width"), (4, "lenght")]);

    // "lenght" (typo) should find "length"
    let similar = env.find_similar(name(4), 3, &resolver);
    assert!(!similar.is_empty(), "should find at least one suggestion");
    assert_eq!(similar[0], name(1), "best match should be 'length'");
}

#[test]
fn test_find_similar_no_match() {
    let mut env = TypeEnv::new();
    env.bind(name(1), Idx::INT); // "alpha"
    env.bind(name(2), Idx::INT); // "beta"

    let resolver = make_resolver(&[(1, "alpha"), (2, "beta"), (3, "xyz")]);

    let similar = env.find_similar(name(3), 3, &resolver);
    assert!(similar.is_empty(), "no similar names should be found");
}

#[test]
fn test_find_similar_empty_env() {
    let env = TypeEnv::new();
    let resolver = make_resolver(&[(1, "anything")]);

    let similar = env.find_similar(name(1), 3, &resolver);
    assert!(similar.is_empty());
}

#[test]
fn test_find_similar_respects_max_results() {
    let mut env = TypeEnv::new();
    env.bind(name(1), Idx::INT); // "abc"
    env.bind(name(2), Idx::INT); // "abd"
    env.bind(name(3), Idx::INT); // "abe"
    env.bind(name(4), Idx::INT); // "abf"

    let resolver = make_resolver(&[(1, "abc"), (2, "abd"), (3, "abe"), (4, "abf"), (5, "abx")]);

    let similar = env.find_similar(name(5), 2, &resolver);
    assert!(similar.len() <= 2, "should respect max_results limit");
}

#[test]
fn test_find_similar_searches_parent_scopes() {
    let mut parent = TypeEnv::new();
    parent.bind(name(1), Idx::INT); // "filter" in parent

    let mut child = parent.child();
    child.bind(name(2), Idx::INT); // "map" in child

    let resolver = make_resolver(&[(1, "filter"), (2, "map"), (3, "fiter")]);

    // "fiter" should find "filter" from parent scope
    let similar = child.find_similar(name(3), 3, &resolver);
    assert!(!similar.is_empty(), "should search parent scopes");
    assert_eq!(similar[0], name(1));
}

#[test]
fn test_find_similar_skips_target_name() {
    let mut env = TypeEnv::new();
    env.bind(name(1), Idx::INT); // "foo"

    let resolver = make_resolver(&[(1, "foo")]);

    // Looking up "foo" itself shouldn't suggest "foo" back
    let similar = env.find_similar(name(1), 3, &resolver);
    assert!(
        similar.is_empty(),
        "should not suggest the target name itself"
    );
}

#[test]
fn test_find_similar_sorted_by_distance() {
    let mut env = TypeEnv::new();
    env.bind(name(1), Idx::INT); // "abcde" (distance 2 from "abxyz")
    env.bind(name(2), Idx::INT); // "abcyz" (distance 1 from "abxyz")

    let resolver = make_resolver(&[(1, "abcde"), (2, "abcyz"), (3, "abxyz")]);

    let similar = env.find_similar(name(3), 3, &resolver);
    if similar.len() >= 2 {
        // Closer match should come first
        assert_eq!(similar[0], name(2), "closer match should be first");
    }
}

#[test]
fn test_find_similar_unresolvable_target() {
    let mut env = TypeEnv::new();
    env.bind(name(1), Idx::INT);

    // Resolver that can't resolve the target
    let resolver = |_: Name| -> Option<&str> { None };

    let similar = env.find_similar(name(99), 3, resolver);
    assert!(similar.is_empty());
}
