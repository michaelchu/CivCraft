use super::*;

#[test]
fn baseline_clones_preserve_rng_and_normal_campaign_results() {
    let initial = Simulation::new(crate::mapgen::generate(42));
    let original = bincode::serialize(&initial).unwrap();
    let mut a = initial.clone();
    let mut b = initial.clone();
    let founder = initial
        .units
        .iter()
        .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
        .unwrap()
        .id;
    assert!(a.apply(Command::FoundCity(founder)));
    assert!(b.apply(Command::FoundCity(founder)));
    assert!(a.apply(Command::EndTurn));
    assert!(b.apply(Command::EndTurn));
    assert_eq!(
        bincode::serialize(&a).unwrap(),
        bincode::serialize(&b).unwrap()
    );
    for _ in 0..32 {
        assert_eq!(a.rand_below(1_000_000), b.rand_below(1_000_000));
    }
    assert_eq!(bincode::serialize(&initial).unwrap(), original);
}
