use super::*;

impl Simulation {
    pub(super) fn transfer_treaty_map(&mut self, giver: usize, receiver: usize, map: TreatyMap) {
        let recipients = self.vision_recipients(receiver);
        for index in 0..self.tiles.len() {
            let tile = &self.tiles[index];
            if !tile.known_by[giver] || (map == TreatyMap::Sea && !tile.terrain.is_water()) {
                continue;
            }
            let human_learns = recipients.contains(&HUMAN) && !tile.seen;
            let observed = human_learns.then(|| {
                let (x, y) = Self::xy(index);
                self.live_route_tile(x, y)
            });
            let tile = &mut self.tiles[index];
            for &recipient in &recipients {
                tile.known_by[recipient] = true;
            }
            if let Some(observed) = observed {
                tile.seen = true;
                tile.last_seen_owner = tile.owner;
                self.route_memory[index] = Some(observed);
            }
        }
    }

    pub(crate) fn unit_visible_to_human(&self, unit: &Unit) -> bool {
        let tile = self.tile(unit.x, unit.y);
        if !tile.visible
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (unit.x, unit.y) && !self.allied(HUMAN, city.owner))
        {
            return false;
        }
        self.allied(HUMAN, unit.owner)
            || (unit.transport.is_none()
                && (!unit.kind.uses_stealth_vision() || tile.stealth_visible))
    }

    pub(super) fn vision_safe(&self, giver: usize, receiver: usize) -> bool {
        self.vision_recipients(receiver).into_iter().all(|third| {
            third == giver
                || third == receiver
                || self.allied(giver, third)
                || !self
                    .diplomacy
                    .contains_key(&Self::relation_key(giver, third))
        })
    }

    pub(in crate::sim) fn vision_recipients(&self, giver: usize) -> Vec<usize> {
        let mut recipients = vec![giver];
        let mut next = 0;
        while let Some(&source) = recipients.get(next) {
            for (&(a, b), relation) in &self.diplomacy {
                let target = if a == source && relation.vision[0] {
                    b
                } else if b == source && relation.vision[1] {
                    a
                } else {
                    continue;
                };
                if !recipients.contains(&target) {
                    recipients.push(target);
                }
            }
            next += 1;
        }
        recipients
    }

    pub(in crate::sim) fn remember_visible_rivals(&mut self) {
        let visible_owners: Vec<_> = self
            .units
            .iter()
            .filter(|unit| self.unit_visible_to_human(unit))
            .map(|unit| unit.owner)
            .chain(
                self.cities
                    .iter()
                    .filter(|city| self.tiles[Self::idx(city.x, city.y)].visible)
                    .map(|city| city.owner),
            )
            .collect();
        for owner in visible_owners {
            if owner != HUMAN && owner != BARBARIANS {
                self.diplomacy
                    .entry(Self::relation_key(HUMAN, owner))
                    .or_default()
                    .first_contact
                    .get_or_insert(self.turn);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stealth_units_require_close_detection_and_foreign_city_garrisons_stay_hidden() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (explorer, spy) = game.stealth_playtest_setup();
        let target = game
            .units
            .iter()
            .find(|unit| unit.id == spy)
            .unwrap()
            .clone();
        assert!(game.tile(target.x, target.y).visible);
        assert!(!game.unit_visible_to_human(&target));
        assert!(!game.can_negotiate(1));
        assert!(game.apply(Command::Move(explorer, 1, 0)));
        assert!(game.unit_visible_to_human(&target));
        assert!(game.can_negotiate(1));
        game.create_city(1, target.x, target.y);
        game.refresh_visibility();
        assert!(!game.unit_visible_to_human(&target));
    }
}
