use super::*;

#[test]
fn edit_distance_empty() {
    assert_eq!(edit_distance("", ""), 0);
    assert_eq!(edit_distance("abc", ""), 3);
    assert_eq!(edit_distance("", "abc"), 3);
}

#[test]
fn edit_distance_same() {
    assert_eq!(edit_distance("hello", "hello"), 0);
    assert_eq!(edit_distance("test", "test"), 0);
}

#[test]
fn edit_distance_one_off() {
    assert_eq!(edit_distance("hello", "hallo"), 1); // substitution
    assert_eq!(edit_distance("hello", "hell"), 1); // deletion
    assert_eq!(edit_distance("hello", "helloo"), 1); // insertion
}

#[test]
fn edit_distance_multiple() {
    assert_eq!(edit_distance("kitten", "sitting"), 3);
    assert_eq!(edit_distance("saturday", "sunday"), 3);
}

#[test]
fn diff_int_float() {
    let pool = Pool::new();
    let problems = diff_types(&pool, Idx::INT, Idx::FLOAT);
    assert_eq!(problems.len(), 1);
    assert!(matches!(problems[0], TypeProblem::IntFloat { .. }));
}

#[test]
fn diff_same_type() {
    let pool = Pool::new();
    let problems = diff_types(&pool, Idx::INT, Idx::INT);
    assert!(problems.is_empty());
}

#[test]
fn diff_list_expected() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let problems = diff_types(&pool, list_int, Idx::INT);
    assert!(problems
        .iter()
        .any(|p| matches!(p, TypeProblem::ExpectedList { .. })));
}

#[test]
fn diff_needs_unwrap() {
    let mut pool = Pool::new();
    let option_int = pool.option(Idx::INT);
    let problems = diff_types(&pool, Idx::INT, option_int);
    assert!(problems
        .iter()
        .any(|p| matches!(p, TypeProblem::NeedsUnwrap { .. })));
}

#[test]
fn diff_function_arity() {
    let mut pool = Pool::new();
    let fn1 = pool.function(&[Idx::INT], Idx::BOOL);
    let fn2 = pool.function(&[Idx::INT, Idx::STR], Idx::BOOL);
    let problems = diff_types(&pool, fn1, fn2);
    assert!(problems
        .iter()
        .any(|p| matches!(p, TypeProblem::WrongArity { .. })));
}

#[test]
fn diff_string_to_number() {
    let pool = Pool::new();
    let problems = diff_types(&pool, Idx::INT, Idx::STR);
    assert!(problems
        .iter()
        .any(|p| matches!(p, TypeProblem::StringToNumber)));
}
