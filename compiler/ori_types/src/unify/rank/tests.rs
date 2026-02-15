use super::*;

#[test]
fn rank_constants() {
    assert_eq!(Rank::TOP.raw(), 0);
    assert_eq!(Rank::IMPORT.raw(), 1);
    assert_eq!(Rank::FIRST.raw(), 2);
}

#[test]
fn rank_ordering() {
    assert!(Rank::TOP < Rank::IMPORT);
    assert!(Rank::IMPORT < Rank::FIRST);
    assert!(Rank::FIRST < Rank::MAX);
}

#[test]
fn rank_next_prev() {
    let r = Rank::FIRST;
    assert_eq!(r.next().raw(), 3);
    assert_eq!(r.prev().raw(), 1);
    assert_eq!(r.prev().prev().raw(), 0);

    // Saturates at TOP
    assert_eq!(Rank::TOP.prev(), Rank::TOP);

    // Saturates at MAX
    assert_eq!(Rank::MAX.next(), Rank::MAX);
}

#[test]
fn can_generalize_at() {
    let r3 = Rank::from_raw(3);
    let r5 = Rank::from_raw(5);

    // Variable at rank 5 can be generalized at rank 3, 4, or 5
    assert!(r5.can_generalize_at(Rank::from_raw(3)));
    assert!(r5.can_generalize_at(Rank::from_raw(4)));
    assert!(r5.can_generalize_at(Rank::from_raw(5)));

    // Variable at rank 3 cannot be generalized at rank 5
    assert!(!r3.can_generalize_at(Rank::from_raw(5)));
}

#[test]
fn is_generalized() {
    assert!(Rank::TOP.is_generalized());
    assert!(!Rank::IMPORT.is_generalized());
    assert!(!Rank::FIRST.is_generalized());
}

#[test]
fn display() {
    assert_eq!(format!("{}", Rank::TOP), "TOP");
    assert_eq!(format!("{}", Rank::IMPORT), "IMPORT");
    assert_eq!(format!("{}", Rank::FIRST), "FIRST");
    assert_eq!(format!("{}", Rank::from_raw(7)), "R7");
}
