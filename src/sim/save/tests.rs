//! Regression tests for the current campaign save format.

use super::*;

fn temporary_save(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "civcraft-{name}-{}-{}.save",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn current_campaign_round_trip_preserves_gameplay_and_topology_state() {
    let path = temporary_save("save-round-trip");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.create_city(HUMAN, 10, 20);
    game.set_random_seed(42);
    game.rand_below(1000);
    game.save_campaign(&path).unwrap();

    let loaded = Simulation::load_campaign(&path).unwrap();
    fs::remove_file(path).unwrap();
    assert_eq!(loaded.map_wrap, MapWrap::X);
    assert_eq!(loaded.random_snapshot(), game.random_snapshot());
    assert_eq!(loaded.turn, game.turn);
    assert_eq!(
        bincode::serialize(&loaded.cities).unwrap(),
        bincode::serialize(&game.cities).unwrap()
    );
    assert_eq!(
        bincode::serialize(&loaded.tiles).unwrap(),
        bincode::serialize(&game.tiles).unwrap()
    );
}

#[test]
fn old_save_headers_and_corrupt_current_payloads_are_rejected() {
    let path = temporary_save("save-invalid");
    fs::write(&path, b"CIVCRAFT\0\x11previous payload").unwrap();
    assert!(Simulation::load_campaign(&path).is_err());

    fs::write(&path, [HEADER, b"invalid payload"].concat()).unwrap();
    let error = match Simulation::load_campaign(&path) {
        Ok(_) => panic!("corrupt current payload was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    fs::remove_file(path).unwrap();
}

#[test]
fn invalid_campaign_state_is_rejected_on_load() {
    let path = temporary_save("save-validation");
    let mut game = Simulation::new(crate::mapgen::generate(42));
    game.climate_current = [game.map_wrap.tile_count() as i32 + 1, 0];
    game.save_campaign(&path).unwrap();
    assert!(Simulation::load_campaign(&path).is_err());
    fs::remove_file(path).unwrap();
}
