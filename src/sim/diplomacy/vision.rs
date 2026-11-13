//! Shared vision and diplomatic map knowledge.

use super::*;

impl Simulation {
    pub(crate) fn gives_shared_vision(&self, giver: usize, receiver: usize) -> bool {
        if giver == receiver || giver >= self.players.len() || receiver >= self.players.len() {
            return false;
        }
        self.diplomacy
            .get(&Self::relation_key(giver, receiver))
            .is_some_and(|relation| relation.vision[usize::from(giver > receiver)])
    }

    pub(in crate::sim) fn cancel_shared_vision(&mut self, giver: usize, receiver: usize) -> bool {
        if !self.gives_shared_vision(giver, receiver) {
            return false;
        }
        let side = usize::from(giver > receiver);
        self.diplomacy
            .get_mut(&Self::relation_key(giver, receiver))
            .unwrap()
            .vision[side] = false;
        self.refresh_visibility();
        if giver == HUMAN {
            self.add_event(format!(
                "You no longer share vision with {}.",
                self.players[receiver].name
            ));
        }
        true
    }

    /// Freeciv's server/diplhand.c shares the requester's map with the
    /// counterpart when a new City clause is added and the counterpart is not
    /// currently seeing the city center.
    pub(in crate::sim) fn share_treaty_city_map(&mut self, other: usize, city_id: u32) -> bool {
        if other == HUMAN || other >= self.players.len() || !self.can_negotiate(other) {
            return false;
        }
        let receiver = other;
        let Some(city) = self.cities.iter().find(|city| city.id == city_id).cloned() else {
            return false;
        };
        if !self
            .treaty_trade_cities(city.owner, HUMAN)
            .contains(&city_id)
            && !self
                .treaty_trade_cities(city.owner, other)
                .contains(&city_id)
        {
            return false;
        }
        if self.has_main_sight_at(receiver, city.x, city.y) {
            return true;
        }

        // C2C3's init_city_radius_sq is 5. city_work_radius_sq adds any
        // City_Radius_Sq effect and applies the ruleset's legal bounds.
        // In Freeciv's ISO|HEX topology, map_vector_to_sq_distance uses the
        // squared real distance; the integer radius is therefore floor(sqrt).
        let radius_sq = self.city_work_radius_sq(&city).max(0);
        let radius = (radius_sq as f64).sqrt().floor() as i32;
        let coordinates = self.map_wrap.freeciv_square_radius(city.x, city.y, radius);
        let recipients = self.vision_recipients(receiver);
        let mut remembered = vec![vec![false; self.tiles.len()]; self.players.len()];

        for (x, y) in coordinates {
            let index = self.map_wrap.index(x, y);
            // handle_diplomacy_create_clause_req passes its packet sender
            // (the human player in this single-player campaign), not the
            // owner of the city clause, as the map source.
            if !self.tiles[index].known_by[HUMAN] {
                continue;
            }
            let requester_sees = self.has_main_sight_at(HUMAN, x, y);
            let observation = if requester_sees {
                self.live_map_observation(index)
            } else {
                self.route_memory[index]
                    .map(|route| MapObservation {
                        route,
                        owner: self.tiles[index].last_seen_owner,
                        ocean: route.is_water(),
                    })
                    .unwrap_or_else(|| self.live_map_observation(index))
            };

            for &recipient in &recipients {
                if recipient == BARBARIANS
                    || self.has_main_sight_at(recipient, x, y)
                    || (!requester_sees && self.tiles[index].known_by[recipient])
                {
                    continue;
                }
                self.tiles[index].known_by[recipient] = true;
                if recipient == HUMAN {
                    self.tiles[index].seen = true;
                    self.tiles[index].last_seen_owner = observation.owner;
                    self.route_memory[index] = Some(observation.route);
                } else {
                    self.ai_map_memory.insert((recipient, index), observation);
                }
                remembered[recipient][index] = true;
            }
        }

        // Store a bounded remembered view for the recipients. The boolean
        // mask contains only tiles inside this city's effective work radius.
        self.remember_render_map(&remembered);
        true
    }

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
                && (!self.unit_vision_layer_for(unit.kind).requires_detection()
                    || tile.stealth_visible))
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

    #[test]
    fn added_city_clause_shares_only_requester_known_tiles_in_city_radius() {
        let mut game = crate::sim::diplomacy::war_planning_game();
        let city = game
            .cities
            .iter()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .clone();
        game.cities
            .iter_mut()
            .find(|candidate| candidate.id == city.id)
            .unwrap()
            .buildings
            .retain(|&building| {
                building != crate::rules::Building::PALACE
                    && building != crate::rules::Building::ECCLESIASTICAL_PALACE
            });
        let radius = game.map_wrap.freeciv_square_radius(city.x, city.y, 2);
        let inside = *radius
            .iter()
            .find(|&&(x, y)| (x, y) != (city.x, city.y))
            .unwrap();
        let outside = game.map_wrap.offset(city.x, city.y, 3, 0).unwrap();
        assert!(!radius.contains(&outside));

        for tile in &mut game.tiles {
            tile.known_by.fill(false);
            tile.seen = false;
        }
        game.tile_mut(city.x, city.y).known_by[1] = true;
        game.tile_mut(city.x, city.y).known_by[HUMAN] = true;
        game.tile_mut(inside.0, inside.1).known_by[HUMAN] = true;
        game.tile_mut(outside.0, outside.1).known_by[HUMAN] = true;
        game.tile_mut(outside.0, outside.1).known_by[1] = false;

        assert!(game.share_treaty_city_map(1, city.id));
        assert!(game.tile(inside.0, inside.1).known_by[1]);
        assert!(!game.tile(outside.0, outside.1).known_by[1]);
    }

    #[test]
    fn vision_clause_and_withdrawal_are_independent_of_pact_type() {
        let mut game = crate::sim::diplomacy::war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Peace);
        let view = game.diplomacy_meeting_view(1, None).unwrap();
        assert!(view.sides.iter().all(|side| side.can_offer_vision));

        let mut offer = TreatyOffer::default();
        offer.terms[0] = Concessions::default().set_shared_vision(true);
        assert!(game.execute_offer(HUMAN, 1, &offer));
        assert!(game.gives_shared_vision(HUMAN, 1));
        let view = game.diplomacy_meeting_view(1, None).unwrap();
        assert!(!view.sides[0].can_offer_vision);
        assert!(view.sides[1].can_offer_vision);
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);

        assert!(game.apply(crate::sim::commands::Command::CancelSharedVision(1)));
        assert!(!game.gives_shared_vision(HUMAN, 1));
        assert!(game.diplomacy_meeting_view(1, None).unwrap().sides[0].can_offer_vision);
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(!game.apply(crate::sim::commands::Command::CancelSharedVision(1)));
    }
}
