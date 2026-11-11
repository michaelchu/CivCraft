//! Shared vision and diplomatic map knowledge.

use super::*;

impl Simulation {
    pub(in crate::sim) fn live_map_observation(&self, index: usize) -> MapObservation {
        let (x, y) = self.map_wrap.position(index);
        let tile = &self.tiles[index];
        MapObservation {
            route: self.live_route_tile(x, y),
            owner: tile.owner,
            ocean: tile.terrain.is_water(),
        }
    }

    pub(in crate::sim) fn transfer_treaty_map(
        &mut self,
        giver: usize,
        receiver: usize,
        map: TreatyMap,
    ) {
        let recipients = self.vision_recipients(receiver);
        for &recipient in &recipients {
            self.transfer_render_map(giver, recipient, map);
        }
        for index in 0..self.tiles.len() {
            let tile = &self.tiles[index];
            if !tile.known_by[giver] {
                continue;
            }
            // Older saves and knowledge granted without direct sight may lack a snapshot.
            let observation = if giver == HUMAN && !tile.visible {
                self.route_memory[index].map(|route| MapObservation {
                    route,
                    owner: tile.last_seen_owner,
                    ocean: route.is_water(),
                })
            } else {
                self.ai_map_memory.get(&(giver, index)).copied()
            }
            .unwrap_or_else(|| self.live_map_observation(index));
            if map == TreatyMap::Sea && !observation.ocean {
                continue;
            }
            let tile = &mut self.tiles[index];
            for &recipient in &recipients {
                if !tile.known_by[recipient] && recipient != HUMAN && recipient != BARBARIANS {
                    self.ai_map_memory.insert((recipient, index), observation);
                }
                tile.known_by[recipient] = true;
            }
            if recipients.contains(&HUMAN) && !tile.seen {
                tile.seen = true;
                tile.last_seen_owner = observation.owner;
                self.route_memory[index] = Some(observation.route);
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

    pub(in crate::sim) fn vision_safe(&self, giver: usize, receiver: usize) -> bool {
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
                    .filter(|city| self.tiles[self.map_wrap.index(city.x, city.y)].visible)
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
    fn sea_map_transfers_the_sellers_last_observation() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.map_snapshot_playtest());
    }
}
