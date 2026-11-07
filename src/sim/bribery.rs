use super::*;

impl Simulation {
    pub(super) fn ai_bribe_nearby(&mut self, actor: &Unit) -> bool {
        let expenses: i32 = self
            .cities
            .iter()
            .filter(|city| city.owner == actor.owner)
            .map(|city| self.building_upkeep(city) + self.city_unit_upkeep(city).gold)
            .sum();
        let reserve = self.ai_gold_reserve(actor.owner);
        let treasury = self.players[actor.owner].gold;
        let origin = (actor.x, actor.y);
        let mut best = None;
        for target in self.units.iter().filter(|target| {
            self.at_war(actor.owner, target.owner)
                && self.tile(target.x, target.y).known_by[actor.owner]
                && self.map_distance(actor.x, actor.y, target.x, target.y)
                    <= actor.moves_left / 2 + 1
        }) {
            for (x, y) in self.map_wrap.neighbors(target.x, target.y) {
                if self.tile(x, y).terrain.is_water()
                    || self.units.iter().any(|unit| {
                        (unit.x, unit.y) == (x, y) && self.at_war(actor.owner, unit.owner)
                    })
                    || self.cities.iter().any(|city| {
                        (city.x, city.y) == (x, y) && !self.allied(actor.owner, city.owner)
                    })
                {
                    continue;
                }
                let path = self.path_for_unit(actor, x, y);
                if (x, y) != origin && path.is_empty() {
                    continue;
                }
                let mut projected = actor.clone();
                for &(dx, dy) in &path {
                    let Some(next) = self.map_wrap.offset(projected.x, projected.y, dx, dy) else {
                        projected.moves_left = 0;
                        break;
                    };
                    let Some(cost) = self.movement_cost_for(&projected, next.0, next.1) else {
                        projected.moves_left = 0;
                        break;
                    };
                    projected.moves_left -= cost;
                    (projected.x, projected.y) = next;
                }
                let Some(price) = self.bribe_price(&projected, target) else {
                    continue;
                };
                let approach_cost = actor.moves_left - projected.moves_left;
                let contact_cost = approach_cost
                    + self
                        .movement_cost_for(&projected, target.x, target.y)
                        .unwrap_or(MOVE_FRAGMENTS)
                        .min(projected.moves_left);
                let stats = target.kind.stats();
                let threat =
                    stats.attack > stats.defense && self.unit_move_rate(target) > contact_cost;
                let keep = if threat { expenses } else { reserve };
                let priority = (contact_cost, target.id);
                if treasury - price >= keep
                    && best
                        .as_ref()
                        .is_none_or(|(current, _, _)| priority < *current)
                {
                    best = Some((priority, price, path));
                }
            }
        }
        let Some(((_, target), price, path)) = best else {
            return false;
        };
        let moved = self.follow_path(actor.id, path);
        self.bribe_unit(actor.id, target, price) || moved
    }

    pub(crate) fn bribe_price(&self, actor: &Unit, target: &Unit) -> Option<i32> {
        if !actor.kind.is_diplomatic()
            || target.kind.is_unbribable()
            || actor.moves_left < 1
            || self.tile(actor.x, actor.y).terrain.is_water()
            || self.allied(actor.owner, target.owner)
            || self.map_distance(actor.x, actor.y, target.x, target.y) > 1
            || target.transport.is_some()
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (target.x, target.y))
            || self
                .units
                .iter()
                .any(|unit| unit.id != target.id && (unit.x, unit.y) == (target.x, target.y))
            || !actor.home_city.is_some_and(|home| {
                self.cities
                    .iter()
                    .any(|city| city.id == home && city.owner == actor.owner)
            })
        {
            return None;
        }
        let capital_distance = self
            .cities
            .iter()
            .filter(|city| city.owner == target.owner && city.has_building(Building::PALACE))
            .map(|city| self.map_distance(city.x, city.y, target.x, target.y))
            .min()
            .unwrap_or(32)
            .min(32);
        let stats = target.kind.stats();
        let base = (750 + self.players[target.owner].gold.max(0)) / (capital_distance + 2);
        let cost = i64::from(base) * i64::from(stats.cost) / 10;
        let cost = cost * i64::from(target.veteran_power()) / 100;
        Some((cost * i64::from(stats.hp + target.hp) / i64::from(2 * stats.hp)) as i32)
    }

    pub(super) fn transfer_unit(&mut self, victim: &Unit, owner: usize, home: Option<u32>) {
        // Native places the gained unit before wiping the old one, so the
        // old owner is still present for ownership-contact refresh.
        self.refresh_diplomatic_contact_at(owner, victim.x, victim.y);
        let birth = self.merchant_birth_turns.get(&victim.id).copied();
        self.units.retain(|unit| unit.id != victim.id);
        self.discard_unit_ai(victim.id);
        let id = self.spawn_unit(owner, victim.kind, victim.x, victim.y);
        if let Some(born) = birth {
            self.merchant_birth_turns.insert(id, born);
        }
        let gained = self.units.last_mut().unwrap();
        gained.hp = victim.hp;
        gained.nationality = victim.nationality;
        gained.fuel = victim.fuel;
        gained.veteran = victim.veteran;
        gained.moves_left = victim.moves_left;
        gained.home_city = home;
        gained.home_order = id;
    }

    pub(super) fn bribe_unit(&mut self, actor: u32, target: u32, offered_price: i32) -> bool {
        let Some(diplomat) = self.unit_by_id(actor).cloned() else {
            return false;
        };
        let Some(victim) = self.unit_by_id(target).cloned() else {
            return false;
        };
        let Some(price) = self.bribe_price(&diplomat, &victim) else {
            return false;
        };
        if price != offered_price || self.players[diplomat.owner].gold < price {
            return false;
        }

        self.players[diplomat.owner].gold -= price;
        self.transfer_unit(&victim, diplomat.owner, diplomat.home_city);

        if !self.at_war(diplomat.owner, victim.owner) {
            self.record_provocation(victim.owner, diplomat.owner);
            self.record_diplomatic_incident(
                diplomat.owner,
                Some(victim.owner),
                diplomacy::DiplomaticIncident::UnitLoss,
                false,
            );
        }
        let (dx, dy) = self
            .map_wrap
            .vector(diplomat.x, diplomat.y, victim.x, victim.y);
        if self.try_move_unit(actor, dx, dy) == MoveResult::Blocked
            && let Some(unit) = self.units.iter_mut().find(|unit| unit.id == actor)
        {
            unit.moves_left = 0;
        }
        if diplomat.owner == HUMAN {
            self.add_event(format!(
                "Your {} bribed a {} for {price} gold.",
                diplomat.kind.name(),
                victim.kind.name()
            ));
        }
        self.promote_unit(actor, 100);
        self.refresh_visibility();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bribery_checks_price_and_isolation_and_rehomes_the_unit() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (actor, target) = game.bribe_playtest_setup();
        let original = game
            .units
            .iter()
            .find(|unit| unit.id == target)
            .unwrap()
            .clone();
        game.units.last_mut().unwrap().hp = 5;
        game.players[1].gold = 100;
        let bribe = |price| Command::BribeUnit(actor, target, price);
        let price = (750 + 100) / 34 * 3 / 4;
        assert!(!game.apply(bribe(price + 1)));
        let stack = game.spawn_unit(1, UnitKind::Warrior, original.x, original.y);
        assert!(!game.apply(bribe(price)));
        game.units.retain(|unit| unit.id != stack);
        let gold = game.players[HUMAN].gold;
        assert!(game.apply(bribe(price)));
        assert_eq!(game.players[HUMAN].gold, gold - price);
        let gained = game.units.last().unwrap();
        assert_eq!(gained.owner, HUMAN);
        assert_eq!(gained.hp, 5);
        assert_eq!(gained.home_city, Some(game.cities[0].id));
        let diplomat = game.unit_by_id(actor).unwrap();
        assert_eq!((diplomat.x, diplomat.y), (original.x, original.y));
    }
}
