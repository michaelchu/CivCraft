//! Automated gameplay scenarios covering treaty behavior.

use super::*;

impl Simulation {
    pub(crate) fn city_buyback_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, point) in [(HUMAN, (8, 24)), (1, (12, 24)), (HUMAN, (10, 24))] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(owner, point.0, point.1);
        }
        for point in [(20, 10), (24, 40)] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(1, point.0, point.1);
        }
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let id = self.cities[2].id;
        self.transfer_traded_city(2, 1);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, 12, 24);
        let discount = self.city_trade_price(&self.cities[2]);
        let founder = self.city_founders.remove(&id);
        let full = self.city_trade_price(&self.cities[2]);
        self.city_founders.insert(id, HUMAN);
        let (Some(discount), Some(full)) = (discount, full) else {
            return false;
        };
        self.players[HUMAN].gold = discount;
        founder == Some(HUMAN)
            && discount == full / 2
            && discount > 0
            && self.apply(Command::BuyCity(id, discount))
            && self.cities[2].owner == HUMAN
            && self.players[HUMAN].gold == 0
            && self.city_founders.get(&id) == Some(&HUMAN)
    }

    pub(crate) fn city_trade_playtest(&mut self) -> bool {
        self.units.clear();
        let capital = (12, 24);
        let town = (10, 24);
        let field = (9, 24);
        for point in [capital, town, field] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
        }
        self.create_city(1, capital.0, capital.1);
        self.create_city(1, town.0, town.1);
        for point in [(20, 10), (24, 40)] {
            self.tile_mut(point.0, point.1).terrain = Terrain::Grassland;
            self.create_city(1, point.0, point.1);
        }
        self.tile_mut(3, 3).terrain = Terrain::Grassland;
        self.create_city(HUMAN, 3, 3);
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, capital.0, capital.1);
        self.tile_mut(town.0, town.1).known_by[HUMAN] = true;
        let id = self.cities[1].id;
        let capital_id = self.cities[0].id;
        self.cities[1].size = 3;
        self.cities[1].shields = 7;
        self.cities[1].food = 5;
        self.cities[1].buildings.push(Building::GRANARY);
        let guard = self.spawn_unit(1, UnitKind::Warrior, town.0, town.1);
        let worker = self.spawn_unit(1, UnitKind::Worker, field.0, field.1);
        self.units.last_mut().unwrap().home_city = Some(id);
        let rehome = self.spawn_unit(1, UnitKind::Warrior, capital.0, capital.1);
        self.units.last_mut().unwrap().home_city = Some(id);
        self.sign_pact(1, 2, Pact::Alliance);
        let visitor = self.spawn_unit(2, UnitKind::Warrior, town.0, town.1);
        let allied_visitor = self.spawn_unit(1, UnitKind::Warrior, field.0, field.1);
        let Some(price) = self.city_trade_price(&self.cities[1]) else {
            return false;
        };
        self.players[HUMAN].gold = price;
        let before = self.players[1].gold;
        let rejected = self.city_trade_price(&self.cities[0]).is_none()
            && !self.apply(Command::BuyCity(id, price - 1))
            && self.cities[1].owner == 1;
        let purchased = self.apply(Command::BuyCity(id, price));
        let city = &self.cities[1];
        let displaced = self.units.iter().any(|unit| {
            unit.id == visitor
                && unit.owner == 2
                && (unit.x, unit.y) != town
                && (unit.y - town.1).abs() <= 4
        });
        let allied_stayed = self
            .units
            .iter()
            .any(|unit| unit.id == allied_visitor && unit.owner == 1 && (unit.x, unit.y) == field);
        displaced
            && allied_stayed
            && rejected
            && purchased
            && self.players[HUMAN].gold == 0
            && self.players[1].gold == before + price
            && city.owner == HUMAN
            && city.size == 3
            && city.food == 5
            && city.shields == 7
            && city.has_building(Building::GRANARY)
            && city.domestic_citizens() == 0
            && self
                .units
                .iter()
                .any(|unit| unit.id == guard && unit.owner == HUMAN && unit.home_city.is_none())
            && self
                .units
                .iter()
                .any(|unit| unit.id == worker && unit.owner == HUMAN && unit.home_city == Some(id))
            && self.units.iter().any(|unit| {
                unit.id == rehome && unit.owner == 1 && unit.home_city == Some(capital_id)
            })
    }

    pub(crate) fn treaty_map_playtest(&mut self) -> bool {
        // Generated starting units can reveal the probe tiles during contact setup.
        self.units.clear();
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let land = (10, 24);
        let sea = (11, 24);
        let unexplored = (12, 24);
        for (point, terrain, known) in [
            (land, Terrain::Grassland, true),
            (sea, Terrain::Sea, true),
            (unexplored, Terrain::Sea, false),
        ] {
            let tile = self.tile_mut(point.0, point.1);
            tile.terrain = terrain;
            tile.known_by[1] = known;
            tile.known_by[HUMAN] = false;
            tile.visible = false;
            tile.seen = false;
        }
        self.create_city(HUMAN, 3, 3);
        // Meet away from the map probes so contact itself reveals none of
        // the tiles whose knowledge must be transferred by the treaty.
        self.tile_mut(22, 10).terrain = Terrain::Grassland;
        self.create_city(1, 22, 10);
        let envoy = offset_native_by_map(22, 10, 1, 0);
        self.tile_mut(envoy.0, envoy.1).terrain = Terrain::Grassland;
        self.spawn_unit(HUMAN, UnitKind::Diplomat, envoy.0, envoy.1);
        let price = self.map_trade_price(1, HUMAN, TreatyMap::Sea, Pact::Ceasefire);
        self.players[HUMAN].gold = price - 1;
        let refused = !self.apply(Command::BuyMap(1, TreatyMap::Sea))
            && !self.tile(sea.0, sea.1).seen
            && self.players[HUMAN].gold == price - 1;
        self.players[HUMAN].gold = price;
        let paid = self.apply(Command::BuyMap(1, TreatyMap::Sea))
            && self.players[HUMAN].gold == 0
            && self.tile(sea.0, sea.1).seen
            && !self.tile(land.0, land.1).seen;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let world = self.apply(Command::BuyMap(1, TreatyMap::World))
            && self.tile(land.0, land.1).seen
            && !self.tile(unexplored.0, unexplored.1).seen
            && !self.tile(sea.0, sea.1).visible
            && !self.tile(land.0, land.1).visible
            && self.vision_recipients(1) == [1];
        refused && paid && world
    }
}
