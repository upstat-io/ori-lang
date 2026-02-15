use super::*;

#[test]
fn primitive_item() {
    let item = Item::primitive(Tag::Int);
    assert_eq!(item.tag, Tag::Int);
    assert_eq!(item.data, 0);
}

#[test]
fn simple_container_item() {
    let item = Item::simple_container(Tag::List, Idx::INT);
    assert_eq!(item.tag, Tag::List);
    assert_eq!(item.child(), Idx::INT);
}

#[test]
fn extra_item() {
    let item = Item::with_extra(Tag::Function, 100);
    assert_eq!(item.tag, Tag::Function);
    assert_eq!(item.extra_idx(), 100);
}

#[test]
fn var_item() {
    let item = Item::var(Tag::Var, 42);
    assert_eq!(item.tag, Tag::Var);
    assert_eq!(item.var_id(), 42);
}
