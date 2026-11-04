use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Pact {
    #[default]
    War,
    Ceasefire,
    Armistice,
    Peace,
    Alliance,
}

impl Pact {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::War => "War",
            Self::Ceasefire => "Ceasefire",
            Self::Armistice => "Armistice",
            Self::Peace => "Peace",
            Self::Alliance => "Alliance",
        }
    }
}

#[derive(Default)]
pub(super) struct Relation {
    pact: Pact,
    until: i32,
    first_contact: Option<i32>,
    cause_until: [i32; 2],
    trust: [i32; 2],
    offer: Option<Pact>,
    offer_after: i32,
}

impl Simulation {
    fn relation_key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

    pub(crate) fn pact(&self, a: usize, b: usize) -> Pact {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(Pact::War, |relation| relation.pact)
    }

    pub(crate) fn at_war(&self, a: usize, b: usize) -> bool {
        a != b && self.pact(a, b) == Pact::War
    }

    pub(crate) fn allied(&self, a: usize, b: usize) -> bool {
        a == b || self.pact(a, b) == Pact::Alliance
    }

    pub(crate) fn pact_turns(&self, a: usize, b: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(0, |relation| (relation.until - self.turn).max(0))
    }

    pub(crate) fn pact_offer(&self, other: usize) -> Option<Pact> {
        self.diplomacy
            .get(&Self::relation_key(HUMAN, other))
            .and_then(|relation| relation.offer)
    }

    pub(crate) fn next_pact(&self, a: usize, b: usize) -> Option<Pact> {
        match self.pact(a, b) {
            Pact::War => Some(Pact::Ceasefire),
            Pact::Ceasefire if self.pact_turns(a, b) <= 4 => Some(Pact::Peace),
            Pact::Peace => Some(Pact::Alliance),
            _ => None,
        }
    }

    pub(crate) fn has_senate(&self, owner: usize) -> bool {
        if self.owns_wonder(owner, "Statue of Liberty") {
            return false;
        }
        matches!(
            self.players[owner].government,
            Government::Federation | Government::Democracy
        ) || (self
            .cities
            .iter()
            .any(|city| city.has_building("United Nations"))
            && self
                .completed_wonders
                .iter()
                .any(|building| building.data().name == "Manhattan Project")
            && !self.apollo_completed
            && self.players[owner]
                .known
                .iter()
                .any(|tech| tech.name() == "Nuclear Fission"))
    }

    pub(crate) fn senate_blocks_cancellation(&self, owner: usize, other: usize) -> bool {
        self.has_senate(owner)
            && self
                .diplomacy
                .get(&Self::relation_key(owner, other))
                .is_none_or(|relation| {
                    relation.cause_until[usize::from(owner > other)] <= self.turn
                })
    }

    pub(crate) fn can_negotiate(&self, other: usize) -> bool {
        other != HUMAN
            && other != BARBARIANS
            && other < self.players.len()
            && (self.cities.iter().any(|city| city.owner == other)
                || self.units.iter().any(|unit| unit.owner == other))
            && (self
                .diplomacy
                .contains_key(&Self::relation_key(HUMAN, other))
                || self
                    .cities
                    .iter()
                    .any(|city| city.owner == other && self.tile(city.x, city.y).visible)
                || self
                    .units
                    .iter()
                    .any(|unit| unit.owner == other && self.tile(unit.x, unit.y).visible))
    }

    fn pact_refusal(
        &self,
        receiver: usize,
        proposer: usize,
        proposed: Pact,
    ) -> Option<&'static str> {
        if self.next_pact(receiver, proposer) != Some(proposed) {
            return Some("We must honor the current treaty before negotiating the next one.");
        }
        if proposed != Pact::Ceasefire
            && (0..self.players.len()).any(|third| {
                third != receiver
                    && third != proposer
                    && third != BARBARIANS
                    && self.player_alive(third)
                    && ((self.allied(proposer, third) && self.at_war(receiver, third))
                        || (proposed == Pact::Alliance
                            && self.allied(receiver, third)
                            && self.at_war(proposer, third)))
            })
        {
            return Some("Your ties to an enemy make this treaty impossible.");
        }
        let Some(relation) = self.diplomacy.get(&Self::relation_key(receiver, proposer)) else {
            return Some("We have not made contact.");
        };
        let trust = relation.trust[usize::from(receiver > proposer)];
        let threshold = match proposed {
            Pact::Ceasefire if self.turn - relation.first_contact.unwrap_or(self.turn) < 15 => {
                return None;
            }
            Pact::Ceasefire => 0,
            Pact::Peace => 1250,
            Pact::Alliance => 2500,
            _ => return Some("That treaty is unavailable."),
        };
        (trust < threshold).then_some("We do not trust you enough for that treaty.")
    }

    fn player_alive(&self, owner: usize) -> bool {
        self.cities.iter().any(|city| city.owner == owner)
            || self.units.iter().any(|unit| unit.owner == owner)
    }

    pub(super) fn negotiate(&mut self, other: usize, proposed: Pact) -> bool {
        if !self.can_negotiate(other) {
            return false;
        }
        let key = Self::relation_key(HUMAN, other);
        self.diplomacy
            .entry(key)
            .or_default()
            .first_contact
            .get_or_insert(self.turn);
        if let Some(reason) = self.pact_refusal(other, HUMAN, proposed) {
            self.add_event(format!("{} declined. {reason}", self.players[other].name));
            return false;
        }
        if self.next_pact(HUMAN, other) != Some(proposed) {
            return false;
        }
        self.sign_pact(HUMAN, other, proposed);
        true
    }

    pub(super) fn decline_offer(&mut self, other: usize) -> bool {
        let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(HUMAN, other)) else {
            return false;
        };
        relation.offer.take().is_some()
    }

    fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
        let relation = self.diplomacy.entry(Self::relation_key(a, b)).or_default();
        relation.pact = if proposed == Pact::Peace {
            Pact::Armistice
        } else {
            proposed
        };
        relation.until = if matches!(proposed, Pact::Peace | Pact::Ceasefire) {
            self.turn + 16
        } else {
            0
        };
        relation.offer = None;
        if a == HUMAN || b == HUMAN {
            let other = if a == HUMAN { b } else { a };
            let detail = if proposed == Pact::Peace {
                " Withdraw from their territory during the 16-turn armistice."
            } else {
                ""
            };
            self.add_event(format!(
                "{} agreed to {}.{detail}",
                self.players[other].name,
                proposed.name().to_lowercase()
            ));
        }
    }

    pub(super) fn cancel_pact(&mut self, owner: usize, other: usize) -> bool {
        let pact = self.pact(owner, other);
        if pact == Pact::War || !self.player_alive(other) {
            return false;
        }
        if self.senate_blocks_cancellation(owner, other) {
            if owner == HUMAN {
                self.add_event(format!(
                    "The Senate refuses to break the {} with {}.",
                    pact.name().to_lowercase(),
                    self.players[other].name
                ));
            }
            return false;
        }
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(owner, other))
            .unwrap();
        relation.pact = if pact == Pact::Alliance {
            Pact::Armistice
        } else {
            Pact::War
        };
        relation.until = if pact == Pact::Alliance {
            self.turn + 16
        } else {
            0
        };
        relation.offer = None;
        relation.cause_until[usize::from(owner > other)] = 0;
        if pact == Pact::Alliance {
            self.resolve_split_stacks(owner, other);
        } else {
            self.record_provocation(other, owner);
            let mutual_allies: Vec<_> = (0..self.players.len())
                .filter(|&ally| {
                    ally != owner
                        && ally != other
                        && self.player_alive(ally)
                        && self.allied(ally, owner)
                        && self.allied(ally, other)
                })
                .collect();
            for ally in mutual_allies {
                self.record_provocation(ally, owner);
                self.cancel_pact(ally, owner);
            }
        }
        self.add_event(format!(
            "{} ended the {} with {}.",
            self.players[owner].name,
            pact.name().to_lowercase(),
            self.players[other].name
        ));
        true
    }

    pub(super) fn record_provocation(&mut self, victim: usize, offender: usize) {
        if victim != offender && victim != BARBARIANS && offender != BARBARIANS {
            let relation = self
                .diplomacy
                .entry(Self::relation_key(victim, offender))
                .or_default();
            relation.first_contact.get_or_insert(self.turn);
            relation.cause_until[usize::from(victim > offender)] = self.turn + 2;
        }
    }

    pub(super) fn remember_visible_rivals(&mut self) {
        for owner in self
            .units
            .iter()
            .filter(|unit| self.tiles[Self::idx(unit.x, unit.y)].visible)
            .map(|unit| unit.owner)
            .chain(
                self.cities
                    .iter()
                    .filter(|city| self.tiles[Self::idx(city.x, city.y)].visible)
                    .map(|city| city.owner),
            )
        {
            if owner != HUMAN && owner != BARBARIANS {
                self.diplomacy
                    .entry(Self::relation_key(HUMAN, owner))
                    .or_default()
                    .first_contact
                    .get_or_insert(self.turn);
            }
        }
    }

    pub(super) fn advance_diplomacy(&mut self) {
        let mut transitions = Vec::new();
        for (&(a, b), relation) in &mut self.diplomacy {
            if relation.until > 0 && relation.until <= self.turn {
                relation.pact = if relation.pact == Pact::Armistice {
                    Pact::Peace
                } else {
                    Pact::War
                };
                relation.until = 0;
                relation.offer = None;
                transitions.push((a, b, relation.pact));
            }
        }
        for (a, b, pact) in transitions {
            if pact == Pact::Peace {
                let expelled: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == a && self.tile(unit.x, unit.y).owner == Some(b)
                            || unit.owner == b && self.tile(unit.x, unit.y).owner == Some(a)
                    })
                    .map(|unit| unit.id)
                    .collect();
                self.units.retain(|unit| {
                    !expelled.contains(&unit.id)
                        && unit.transport.is_none_or(|id| !expelled.contains(&id))
                });
            }
            if a == HUMAN || b == HUMAN {
                let other = if a == HUMAN { b } else { a };
                self.add_event(format!(
                    "{} with {}.",
                    if pact == Pact::Peace {
                        "Peace entered into force"
                    } else {
                        "Ceasefire expired"
                    },
                    self.players[other].name
                ));
            }
        }

        // Contact between AI nations comes from units encountering foreign borders
        // or meeting units and cities within one tile.
        let positions: Vec<_> = self
            .units
            .iter()
            .map(|unit| (unit.owner, unit.x, unit.y))
            .chain(self.cities.iter().map(|city| (city.owner, city.x, city.y)))
            .collect();
        for (index, &(a, x, y)) in positions.iter().enumerate() {
            if a == HUMAN || a == BARBARIANS {
                continue;
            }
            for &(b, bx, by) in &positions[index + 1..] {
                if b != a && b != HUMAN && b != BARBARIANS && self.map_distance(x, y, bx, by) <= 1 {
                    self.diplomacy
                        .entry(Self::relation_key(a, b))
                        .or_default()
                        .first_contact
                        .get_or_insert(self.turn);
                }
            }
        }
        let pairs: Vec<_> = self.diplomacy.keys().copied().collect();
        for (a, b) in pairs {
            if !self.player_alive(a) || !self.player_alive(b) {
                continue;
            }
            let pact = self.pact(a, b);
            let enemies = (0..self.players.len())
                .filter(|&third| {
                    third != a
                        && third != b
                        && third != BARBARIANS
                        && self.player_alive(third)
                        && self.at_war(a, third)
                        && self.at_war(b, third)
                })
                .count() as i32;
            let relation = self.diplomacy.get_mut(&(a, b)).unwrap();
            for (index, trust) in relation.trust.iter_mut().enumerate() {
                let change = if pact == Pact::War {
                    -150
                } else if relation.cause_until[index] > self.turn {
                    -1800
                } else {
                    150 + 100 * i32::from(pact == Pact::Alliance) + 75 * enemies
                };
                let updated = *trust + change;
                *trust = (updated - updated * 4 / 100).clamp(-10_000, 10_000);
            }
            let Some(proposed) = self.next_pact(a, b) else {
                continue;
            };
            if a == HUMAN {
                if self.pact_refusal(b, a, proposed).is_some() {
                    self.diplomacy.get_mut(&(a, b)).unwrap().offer = None;
                    continue;
                }
                let relation = self.diplomacy.get_mut(&(a, b)).unwrap();
                if relation.offer.is_none() && self.turn >= relation.offer_after {
                    relation.offer = Some(proposed);
                    relation.offer_after = self.turn
                        + match proposed {
                            Pact::Ceasefire => 9,
                            Pact::Peace => 12,
                            _ => 13,
                        };
                    self.add_event(format!(
                        "{} offers {}.",
                        self.players[b].name,
                        proposed.name().to_lowercase()
                    ));
                }
            } else if self.pact_refusal(a, b, proposed).is_none()
                && self.pact_refusal(b, a, proposed).is_none()
            {
                self.sign_pact(a, b, proposed);
            }
        }
    }

    // The desktop self-test advances treaty timers with the armies held in place.
    pub(crate) fn diplomacy_playtest_tick(&mut self, turns: i32) {
        for _ in 0..turns {
            self.turn += 1;
            self.advance_diplomacy();
        }
    }

    pub(crate) fn diplomacy_playtest_unit(&mut self, rival: usize) -> Option<(u32, i32, i32)> {
        let city = self.cities.iter().find(|city| city.owner == rival)?;
        let (x, y) = (city.x, city.y);
        let (start_x, start_y) = offset_native_by_map(x, y, -1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(start_x, start_y).terrain = Terrain::Grassland;
        let id = self.spawn_unit(HUMAN, UnitKind::Warrior, start_x, start_y);
        Some((id, x, y))
    }

    pub(super) fn treaty_blocks_move(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let blocked = |owner| !self.allied(unit.owner, owner) && !self.at_war(unit.owner, owner);
        let occupied = self
            .units
            .iter()
            .any(|other| (other.x, other.y) == (x, y) && blocked(other.owner))
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && blocked(city.owner));
        occupied
            || self
                .tile(x, y)
                .owner
                .is_some_and(|owner| self.pact(unit.owner, owner) == Pact::Peace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_armistice_disbands_units_that_fail_to_withdraw() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy_playtest_tick(15);
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(game.units.iter().any(|unit| unit.id == soldier));
        game.diplomacy_playtest_tick(1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert!(!game.units.iter().any(|unit| unit.id == soldier));
    }

    #[test]
    fn breaking_an_alliance_removes_visitors_from_former_allied_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert!(game.apply(Command::CancelPact(1)));
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == soldier && (unit.x, unit.y) != (20, 20))
        );
    }

    #[test]
    fn senate_enforces_treaties_unless_provoked_or_waived() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.players[HUMAN].government = Government::Federation;
        assert!(!game.apply(Command::CancelPact(1)));
        game.record_provocation(HUMAN, 1);
        assert!(game.apply(Command::CancelPact(1)));
        assert!(game.at_war(HUMAN, 1));

        let find = |name| {
            Building::all()
                .find(|building| building.data().name == name)
                .unwrap()
        };
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.cities[0].buildings.push(find("Statue of Liberty"));
        assert!(game.apply(Command::CancelPact(1)));

        game.cities[0]
            .buildings
            .retain(|building| building.data().name != "Statue of Liberty");
        game.players[HUMAN].government = Government::Tribal;
        game.cities[0].buildings.push(find("United Nations"));
        game.completed_wonders.push(find("Manhattan Project"));
        let fission = Tech::all()
            .find(|tech| tech.name() == "Nuclear Fission")
            .unwrap();
        game.players[HUMAN].known.push(fission);
        assert!(game.has_senate(HUMAN));
        game.apollo_completed = true;
        assert!(!game.has_senate(HUMAN));
    }
}
