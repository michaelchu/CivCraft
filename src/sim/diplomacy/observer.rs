//! Diplomatic observation and evaluation of other players.

use super::*;

impl Simulation {
    // common/player.c:can_player_see_unit_at. known_by is remembered map
    // knowledge, not current sight; reconstruct the observer's live layers.
    pub(crate) fn unit_visible_to(&self, observer: usize, unit: &Unit) -> bool {
        if observer >= self.players.len() {
            return false;
        }
        if unit.transport.is_some() && !self.allied(observer, unit.owner) {
            return false;
        }
        if self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (unit.x, unit.y) && !self.allied(observer, city.owner))
        {
            return false;
        }
        self.unit_is_visible_from_sources(observer, unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observers() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (18, 24)), (2, (28, 24))] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
            game.create_city(owner, x, y);
        }
        game
    }

    #[test]
    fn remembered_tiles_do_not_reveal_units_and_foreign_cities_conceal_garrisons() {
        let mut game = observers();
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 26, 24);
        game.tile_mut(26, 24).known_by[1] = true;
        assert!(!game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        let id = game.spawn_unit(HUMAN, UnitKind::Warrior, 18, 24);
        assert!(game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        let city = game.cities.iter_mut().find(|city| city.owner == 1).unwrap();
        city.owner = 2;
        game.spawn_unit(1, UnitKind::Warrior, 17, 23);
        assert!(!game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
    }

    #[test]
    fn stealth_needs_detection_and_cargo_requires_an_alliance() {
        let mut game = observers();
        let id = game.spawn_unit(HUMAN, UnitKind::Submarine, 22, 24);
        assert!(!game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        game.spawn_unit(1, UnitKind::Destroyer, 21, 23);
        assert!(game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        let transport = game.spawn_unit(HUMAN, UnitKind::Transport, 22, 24);
        game.units
            .iter_mut()
            .find(|u| u.id == id)
            .unwrap()
            .transport = Some(transport);
        assert!(!game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert!(game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
    }

    #[test]
    fn source_city_concealment_precedes_even_own_unit_exemption() {
        let mut game = observers();
        let id = game.spawn_unit(1, UnitKind::Warrior, 8, 24);
        // common/player.c checks city visibility before its allied-unit return.
        assert!(!game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert!(game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
    }

    #[test]
    fn shared_vision_supplies_both_observer_layers() {
        let mut game = observers();
        let id = game.spawn_unit(HUMAN, UnitKind::Submarine, 28, 24);
        assert!(!game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
        game.sign_pact(1, 2, Pact::Alliance);
        let relation = game.diplomacy.get_mut(&(1, 2)).unwrap();
        relation.vision[1] = true;
        assert!(game.unit_visible_to(1, game.unit_by_id(id).unwrap()));
    }
}
