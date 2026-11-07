use super::*;

impl Simulation {
    pub(super) fn upgrade_target(&self, unit: &Unit) -> Option<UnitKind> {
        let target = self.available_unit_successor(unit.owner, unit.kind)?;
        if unit.kind == UnitKind::Trireme
            && !self.tile(unit.x, unit.y).terrain.is_water()
            && !self.cities.iter().any(|city| {
                (city.x, city.y) == (unit.x, unit.y) && self.naval_city_access(city, target)
            })
        {
            return None;
        }
        let cargo: Vec<_> = self
            .units
            .iter()
            .filter(|other| other.transport == Some(unit.id))
            .collect();
        if cargo.len() > target.transport_capacity()
            || cargo.iter().any(|other| !target.can_carry(other.kind))
        {
            return None;
        }
        if let Some(carrier) = unit.transport
            && !self
                .units
                .iter()
                .any(|other| other.id == carrier && other.kind.can_carry(target))
        {
            return None;
        }
        Some(target)
    }

    pub(crate) fn paid_upgrade(&self, unit: &Unit) -> Option<(UnitKind, i32)> {
        self.cities
            .iter()
            .find(|city| city.owner == unit.owner && (city.x, city.y) == (unit.x, unit.y))?;
        let target = self.upgrade_target(unit)?;
        let missing = target.stats().cost - percent_of(unit.kind.stats().cost, 50);
        let price = 2 * missing + missing * missing / 20;
        let discount = if self.has_tech(unit.owner, Tech::INVENTION) {
            50
        } else {
            100
        };
        Some((target, percent_of(price, discount)))
    }

    pub(super) fn upgrade_unit(&mut self, id: u32, target: UnitKind, quoted_price: i32) -> bool {
        let Some(index) = self.unit_index_by_id(id) else {
            return false;
        };
        let owner = self.units[index].owner;
        if self.paid_upgrade(&self.units[index]) != Some((target, quoted_price))
            || self.players[owner].gold < quoted_price
        {
            return false;
        }
        self.players[owner].gold -= quoted_price;
        self.transform_upgraded_unit(index, target, quoted_price);
        self.refresh_visibility();
        true
    }

    #[cfg(test)]
    pub(super) fn apply_workshop_upgrades(&mut self) {
        for owner in 0..self.players.len() {
            self.apply_workshop_upgrades_for(owner);
        }
    }

    pub(super) fn apply_workshop_upgrades_for(&mut self, owner: usize) {
        if !self.has_active_wonder(owner, Building::LEONARDOS_WORKSHOP) {
            return;
        }
        let candidates: Vec<_> = self
            .player_units_ordered(owner)
            .filter_map(|unit| self.upgrade_target(unit).map(|target| (unit.id, target)))
            .collect();
        if !candidates.is_empty() {
            let (id, target) = candidates[self.rand_below(candidates.len() as i32) as usize];
            let index = self.unit_index_by_id(id).unwrap();
            self.transform_upgraded_unit(index, target, 0);
        }
    }

    fn transform_upgraded_unit(&mut self, index: usize, target: UnitKind, price: i32) {
        let unit = &mut self.units[index];
        let original = unit.kind;
        let from = original.stats();
        let to = target.stats();
        unit.kind = target;
        unit.veteran = unit.veteran.saturating_sub(1);
        unit.hp = (unit.hp * to.hp / from.hp).max(1);
        unit.moves_left = unit.moves_left * to.moves / from.moves;
        if unit.owner == HUMAN {
            let message = format!(
                "{} upgraded to {} for {price} gold.",
                original.name(),
                target.name()
            );
            self.add_event(message);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paid_upgrades_scale_health_and_reduce_rank_and_require_own_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let id = game.upgrade_playtest_setup();
        let original = game.units.last().unwrap().clone();
        let cost = UnitKind::Musketeers.stats().cost - UnitKind::Warrior.stats().cost / 2;
        let price = (2 * cost + cost * cost / 20) / 2;
        assert_eq!(
            game.paid_upgrade(&original),
            Some((UnitKind::Musketeers, price))
        );
        assert!(!game.apply(Command::UpgradeUnit(id, UnitKind::Musketeers, price - 1)));
        let gold = game.players[HUMAN].gold;
        assert!(game.apply(Command::UpgradeUnit(id, UnitKind::Musketeers, price)));
        let unit = game.units.last().unwrap();
        assert_eq!(unit.veteran, 1);
        assert_eq!(unit.hp, UnitKind::Musketeers.stats().hp / 2);
        assert_eq!(game.players[HUMAN].gold, gold - price);
        let field = game.spawn_unit(HUMAN, UnitKind::Warrior, 11, 20);
        assert!(!game.apply(Command::UpgradeUnit(field, UnitKind::Musketeers, price)));
    }

    #[test]
    fn a_workshop_completed_in_city_activity_waits_until_next_restore_phase() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.hut = false;
        }
        game.create_city(HUMAN, 10, 24);
        game.players[HUMAN]
            .known
            .extend([Tech::INVENTION, Tech::EXPLOSIVES]);
        game.cities[0].production = Production::Building(Building::LEONARDOS_WORKSHOP);
        game.cities[0].shields = 1000;
        game.cities[0].food = 10;
        let workers: Vec<_> = [11, 12]
            .into_iter()
            .map(|x| game.spawn_unit(HUMAN, UnitKind::Worker, x, 24))
            .collect();
        game.restore_player_units(HUMAN);
        game.process_cities();
        assert!(game.cities[0].has_building(Building::LEONARDOS_WORKSHOP));
        assert!(
            game.units
                .iter()
                .filter(|unit| workers.contains(&unit.id))
                .all(|unit| unit.kind == UnitKind::Worker)
        );
        game.restore_player_units(HUMAN);
        assert_eq!(
            game.units
                .iter()
                .filter(|unit| workers.contains(&unit.id) && unit.kind == UnitKind::Engineer)
                .count(),
            1
        );
    }

    #[test]
    fn leonardo_upgrades_one_field_unit_for_free() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.create_city(HUMAN, 10, 20);
        game.cities[0].buildings.push(Building::LEONARDOS_WORKSHOP);
        game.players[HUMAN].known.push(Tech::EXPLOSIVES);
        for x in [11, 12] {
            game.spawn_unit(HUMAN, UnitKind::Worker, x, 20);
        }
        let gold = game.players[HUMAN].gold;
        game.apply_workshop_upgrades();
        assert_eq!(
            game.units
                .iter()
                .filter(|unit| unit.kind == UnitKind::Engineer)
                .count(),
            1
        );
        assert_eq!(game.players[HUMAN].gold, gold);
    }
}
