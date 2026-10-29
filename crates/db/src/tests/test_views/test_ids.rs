use super::*;

use crate::views::ids::{Local, derive_id};
use crate::views::is_derived;

#[test]
fn derived_ids_are_stable_and_marked() {
    let key = Uuid::new_v4();
    let a = derive_id("view", Some(&key), Local::Node(0));
    assert_eq!(a, derive_id("view", Some(&key), Local::Node(0)));
    assert!(is_derived(&a));
    assert!(!is_derived(&Uuid::new_v4()));
    assert!(!is_derived(&Uuid::nil()));
}

#[test]
fn derived_ids_differ_by_every_part() {
    let (k1, k2) = (Uuid::new_v4(), Uuid::new_v4());
    let (p1, p2) = (Pointee::from(k1), Pointee::from(k2));
    let ids = [
        derive_id("view", Some(&k1), Local::Node(0)),
        derive_id("view", Some(&k1), Local::Node(1)),
        derive_id("view", Some(&k2), Local::Node(0)),
        derive_id("other", Some(&k1), Local::Node(0)),
        derive_id("view", None, Local::Node(0)),
        derive_id("view", Some(&k1), Local::Hyperedge(0)),
        derive_id("view", Some(&k1), Local::Edge(&p1, &p2, 0)),
        derive_id("view", Some(&k1), Local::Edge(&p2, &p1, 0)),
        derive_id("view", Some(&k1), Local::Edge(&p1, &p2, 1)),
    ];
    let distinct: HashSet<_> = ids.iter().collect();
    assert_eq!(distinct.len(), ids.len());
}
