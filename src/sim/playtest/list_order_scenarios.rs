//! Automated gameplay scenarios covering list order behavior.

use super::*;

impl Simulation {
    pub(crate) fn native_list_order_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.sync_list_order();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
        }
        let p = (10, 24);
        let q = offset_native_by_map(p.0, p.1, 1, 0);
        let first = self.spawn_unit(HUMAN, UnitKind::Warrior, p.0, p.1);
        let second = self.spawn_unit(HUMAN, UnitKind::Warrior, q.0, q.1);
        let prepended = self
            .player_units_ordered(HUMAN)
            .map(|u| u.id)
            .collect::<Vec<_>>()
            == [second, first];
        self.enter_tile(first, q.0, q.1, MOVE_FRAGMENTS, None);
        let attacker = self.make_unit(0, 1, UnitKind::Warrior, q.0, q.1);
        let arrival = self
            .tile_units_ordered(q.0, q.1)
            .map(|u| u.id)
            .collect::<Vec<_>>()
            == [first, second]
            && self
                .player_units_ordered(HUMAN)
                .map(|u| u.id)
                .collect::<Vec<_>>()
                == [second, first]
            && self.best_defender_id(&attacker, q.0, q.1) == Some(first);
        self.units.clear();
        self.sync_list_order();
        self.create_city(HUMAN, p.0, p.1);
        let old_city = self.cities[0].id;
        self.create_city(1, 22, 40);
        let new_city = self.cities[1].id;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let old = self.spawn_unit(HUMAN, UnitKind::Warrior, p.0, p.1);
        self.units.last_mut().unwrap().home_city = Some(old_city);
        let recent = self.spawn_unit(1, UnitKind::Warrior, p.0, p.1);
        self.transfer_traded_city(0, 1);
        let ownership = self
            .player_units_ordered(1)
            .map(|u| u.id)
            .collect::<Vec<_>>()
            == [old, recent]
            && self
                .tile_units_ordered(p.0, p.1)
                .map(|u| u.id)
                .collect::<Vec<_>>()
                == [recent, old];
        let cities = self
            .player_cities_ordered(1)
            .map(|c| c.id)
            .collect::<Vec<_>>()
            == [old_city, new_city];
        (prepended && arrival, ownership, cities)
    }

    pub(crate) fn native_cargo_order_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.sync_list_order();
        let p = (10, 24);
        self.tile_mut(p.0, p.1).terrain = Terrain::Sea;
        let boat = self.spawn_unit(HUMAN, UnitKind::Carrier, p.0, p.1);
        let first = self.spawn_unit(HUMAN, UnitKind::Fighter, p.0, p.1);
        let second = self.spawn_unit(HUMAN, UnitKind::Fighter, p.0, p.1);
        let boarded = self.board_or_deboard(second) && self.board_or_deboard(first);
        let order = self
            .cargo_units_ordered(boat)
            .map(|unit| unit.id)
            .collect::<Vec<_>>()
            == [second, first];
        let reboarded = self.board_or_deboard(second) && self.board_or_deboard(second);
        boarded
            && order
            && reboarded
            && self
                .cargo_units_ordered(boat)
                .map(|unit| unit.id)
                .collect::<Vec<_>>()
                == [first, second]
            && self.valid_cargo_order()
    }

    pub(crate) fn native_city_activity_order_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.sync_list_order();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
        }
        self.create_city(HUMAN, 10, 24);
        self.create_city(HUMAN, 22, 40);
        let oldest = self.cities[0].id;
        let newest = self.cities[1].id;
        assert_eq!(
            self.player_cities_ordered(HUMAN)
                .map(|city| city.id)
                .collect::<Vec<_>>(),
            [newest, oldest]
        );
        for city in &mut self.cities {
            city.production = Production::Unit(UnitKind::Warrior);
            city.shields = city.production.cost();
        }
        let city_ids: Vec<_> = self
            .player_cities_ordered(HUMAN)
            .map(|city| city.id)
            .collect();
        // Each city consumes the C2C3 conversion roll before city activity
        // selection, even though these cities have no foreign residents.
        let mut reference_rng = crate::random::FreecivRng::new(2);
        for _ in &city_ids {
            reference_rng.draw(1000);
        }
        let expected_first_city = city_ids[reference_rng.draw(city_ids.len() as u32) as usize];
        self.set_random_seed(2);
        self.process_player_cities(HUMAN);
        let first_unit_home = self
            .units
            .iter()
            .min_by_key(|unit| unit.id)
            .and_then(|unit| unit.home_city);
        let warrior_count = self
            .units
            .iter()
            .filter(|unit| unit.kind == UnitKind::Warrior)
            .count();
        first_unit_home == Some(expected_first_city) && warrior_count == 2
    }
}
