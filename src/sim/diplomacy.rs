use super::*;

mod offers;
mod vision;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreatyMap {
    World,
    Sea,
}

impl TreatyMap {
    fn flag(self) -> u64 {
        match self {
            Self::World => 2,
            Self::Sea => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Concessions {
    pub(crate) gold: i32,
    pub(crate) technology: Option<Tech>,
    // Low bits retain the vision and map flags. Higher bits store the city ID.
    access: u64,
}

impl Concessions {
    pub(crate) fn shared_vision(self) -> bool {
        self.access & 1 != 0
    }

    pub(crate) fn city(self) -> Option<u32> {
        let id = (self.access >> 3) as u32;
        (id != 0).then_some(id)
    }

    fn with_city(mut self, id: u32) -> Self {
        self.access = (self.access & 7) | (u64::from(id) << 3);
        self
    }

    fn valid_access(self) -> bool {
        self.access & 7 <= 5 && self.access >> 3 <= u64::from(u32::MAX)
    }

    pub(crate) fn map(self) -> Option<TreatyMap> {
        [TreatyMap::World, TreatyMap::Sea]
            .into_iter()
            .find(|map| self.access & map.flag() != 0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TreatyOffer {
    pub(crate) pact: Option<Pact>,
    // Terms are ordered by player ID.
    pub(crate) terms: [Concessions; 2],
}

impl TreatyOffer {
    pub(crate) fn bundled(self) -> bool {
        self.terms != [Concessions::default(); 2]
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Relation {
    pact: Pact,
    until: i32,
    first_contact: Option<i32>,
    cause_until: [i32; 2],
    trust: [i32; 2],
    offer: Option<TreatyOffer>,
    vision: [bool; 2],
    embassy: [bool; 2],
    offer_after: i32,
}

impl Simulation {
    pub(super) fn relation_key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

    fn has_real_embassy(&self, owner: usize, other: usize) -> bool {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_some_and(|relation| relation.embassy[usize::from(owner > other)])
    }

    pub(crate) fn has_embassy(&self, owner: usize, other: usize) -> bool {
        owner != other
            && other != BARBARIANS
            && (self.has_real_embassy(owner, other)
                || self.has_active_wonder(owner, Building::MARCO_POLOS_EMBASSY))
    }

    pub(super) fn clear_outgoing_embassies(&mut self, owner: usize) {
        for (&(a, b), relation) in &mut self.diplomacy {
            if owner == a {
                relation.embassy[0] = false;
            } else if owner == b {
                relation.embassy[1] = false;
            }
        }
    }

    pub(crate) fn can_establish_embassy(&self, unit: &Unit, city: &City) -> bool {
        (unit.kind.is_diplomatic() || unit.kind == UnitKind::Explorer)
            && unit.moves_left > 0
            && unit.transport.is_none()
            && city.owner != unit.owner
            && city.owner != BARBARIANS
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
            && !self.has_real_embassy(unit.owner, city.owner)
    }

    pub(super) fn establish_embassy(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == unit_id) else {
            return false;
        };
        let Some(city) = self.cities.iter().find(|city| city.id == city_id) else {
            return false;
        };
        if !self.can_establish_embassy(unit, city) {
            return false;
        }
        let (owner, other) = (unit.owner, city.owner);
        let spy = unit.kind == UnitKind::Spy;
        let message = format!("An embassy was established in {}.", city.name);
        let relation = self
            .diplomacy
            .entry(Self::relation_key(owner, other))
            .or_default();
        relation.embassy[usize::from(owner > other)] = true;
        relation.first_contact.get_or_insert(self.turn);
        if spy {
            self.units
                .iter_mut()
                .find(|unit| unit.id == unit_id)
                .unwrap()
                .moves_left -= 1;
        } else {
            self.units.retain(|unit| unit.id != unit_id);
        }
        if owner == HUMAN {
            self.add_event(message);
        }
        self.refresh_visibility();
        true
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

    pub(crate) fn next_pact(&self, a: usize, b: usize) -> Option<Pact> {
        match self.pact(a, b) {
            Pact::War => Some(Pact::Ceasefire),
            Pact::Ceasefire if self.pact_turns(a, b) <= 4 => Some(Pact::Peace),
            Pact::Peace => Some(Pact::Alliance),
            _ => None,
        }
    }

    pub(crate) fn has_senate(&self, owner: usize) -> bool {
        if self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY) {
            return false;
        }
        matches!(
            self.players[owner].government,
            Government::Federation | Government::Democracy
        ) || (self
            .cities
            .iter()
            .any(|city| city.has_building(Building::UNITED_NATIONS))
            && self
                .completed_wonders
                .iter()
                .any(|building| *building == Building::MANHATTAN_PROJECT)
            && !self.apollo_completed
            && self.players[owner]
                .known
                .iter()
                .any(|tech| *tech == Tech::NUCLEAR_FISSION))
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

    pub(super) fn player_alive(&self, owner: usize) -> bool {
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

    pub(super) fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
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
            relation.vision = [false; 2];
            self.resolve_split_stacks(owner, other);
            self.refresh_visibility();
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
        self.prune_trade_routes();
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
            let proposed = self.next_pact(a, b).filter(|&pact| {
                self.pact_refusal(b, a, pact).is_none()
                    && (a == HUMAN || self.pact_refusal(a, b, pact).is_none())
            });
            let relation = self.diplomacy.get(&(a, b)).unwrap();
            if relation.offer.is_some() || self.turn < relation.offer_after {
                continue;
            }
            let offer = self.build_offer(a, b, proposed);
            if offer.pact.is_none() && !offer.bundled() {
                continue;
            }
            let relation = self.diplomacy.get_mut(&(a, b)).unwrap();
            relation.offer_after = self.turn
                + match proposed {
                    Some(Pact::Ceasefire) => 9,
                    Some(Pact::Peace) => 12,
                    _ => 13,
                };
            if a == HUMAN {
                relation.offer = Some(offer);
                let what = if offer.bundled() {
                    "a treaty bundle".to_owned()
                } else {
                    proposed.unwrap().name().to_lowercase()
                };
                self.add_event(format!("{} offers {what}.", self.players[b].name));
            } else {
                self.execute_offer(a, b, offer);
            }
        }
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

impl Simulation {
    pub(crate) fn city_bundle_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (14, 24)), (HUMAN, (10, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.transfer_traded_city(2, 1);
        let point = (23, 36);
        let tile = self.tile_mut(point.0, point.1);
        tile.known_by[1] = true;
        tile.known_by[HUMAN] = false;
        tile.seen = false;
        let key = (HUMAN, 1);
        self.diplomacy.entry(key).or_default().pact = Pact::Peace;
        self.players[HUMAN].known.clear();
        self.players[1].known.clear();
        self.players[HUMAN].gold = 20_000;
        let offer = self.build_offer(HUMAN, 1, None);
        if offer.terms[1].city() != Some(self.cities[2].id)
            || offer.terms[1].map() != Some(TreatyMap::World)
        {
            return false;
        }

        self.cities[2].buildings.push(Building::PALACE);
        self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        let rejected = !self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 20_000
            && self.cities[2].owner == 1
            && !self.tile(point.0, point.1).seen;
        self.cities[2]
            .buildings
            .retain(|building| *building != Building::PALACE);
        self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        rejected
            && self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 20_000 - offer.terms[0].gold
            && self.cities[2].owner == HUMAN
            && self.tile(point.0, point.1).seen
    }

    pub(crate) fn ai_map_offer_playtest(&mut self) -> bool {
        self.create_city(HUMAN, 3, 3);
        self.create_city(1, 18, 32);
        let point = (11, 24);
        let tile = self.tile_mut(point.0, point.1);
        tile.terrain = Terrain::Ocean;
        tile.known_by[1] = true;
        tile.known_by[HUMAN] = false;
        tile.visible = false;
        tile.seen = false;
        let relation = self.diplomacy.entry((HUMAN, 1)).or_default();
        relation.pact = Pact::Peace;
        relation.trust = [0; 2];
        self.players[HUMAN].gold = 15;
        let offer = self.build_offer(HUMAN, 1, None);
        if offer.terms[1].map() != Some(TreatyMap::Sea) || offer.terms[0].gold != 15 {
            return false;
        }
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
        self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 0
            && self.treaty_offer(1).is_none()
            && self.tile(point.0, point.1).seen
            && !self.tile(point.0, point.1).visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directional_embassy_consumes_explorer_and_reduces_research_cost() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        game.create_city(1, x, y);
        let city = game.cities[0].id;
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let explorer = game.spawn_unit(HUMAN, UnitKind::Explorer, nx, ny);
        let diplomat = game.spawn_unit(HUMAN, UnitKind::Diplomat, nx, ny);
        game.players[1].known.push(Tech::POTTERY);
        assert_eq!(game.target_cost(HUMAN, Tech::POTTERY), Tech::POTTERY.cost());
        assert!(game.apply(Command::EstablishEmbassy(explorer, city)));
        assert!(game.has_embassy(HUMAN, 1));
        assert!(!game.has_embassy(1, HUMAN));
        assert!(!game.apply(Command::EstablishEmbassy(diplomat, city)));
        assert!(game.units.iter().all(|unit| unit.id != explorer));
        assert_eq!(
            game.target_cost(HUMAN, Tech::POTTERY),
            Tech::POTTERY.cost() / 2
        );
    }

    #[test]
    fn technology_price_discounts_allies_before_adding_leakage() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known = vec![Tech::POTTERY];
        game.players[HUMAN].known.clear();
        game.players[2].known.clear();
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(45));
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(22));
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        let other = game.diplomacy.entry((1, 2)).or_default();
        other.pact = Pact::Peace;
        other.trust[0] = 1000;
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(33));
        game.sign_pact(1, 2, Pact::Ceasefire);
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), None);
    }

    #[test]
    fn low_science_can_make_a_balanced_offer_unacceptable_to_the_ai() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.diplomacy_playtest_technologies(true);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        let offer = game.build_offer(HUMAN, 1, None);
        assert!(offer.terms.iter().all(|term| term.technology.is_some()));
        assert!(game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
        game.players[1].last_science = 0;
        assert!(!game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
        game.players[1].research = -1;
        assert!(!game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
    }

    #[test]
    fn stale_bundle_applies_no_gold_technology_pact_or_vision() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        game.diplomacy_playtest_technologies(true);
        let key = Simulation::relation_key(HUMAN, 1);
        let relation = game.diplomacy.entry(key).or_default();
        relation.pact = Pact::Peace;
        relation.trust = [3000; 2];
        let offer = game.build_offer(HUMAN, 1, Some(Pact::Alliance));
        assert!(offer.terms[0].gold > 0 && offer.terms[1].technology.is_some());
        game.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        game.players[HUMAN].gold = offer.terms[0].gold - 1;
        let balances = [game.players[HUMAN].gold, game.players[1].gold];
        assert!(!game.apply(Command::AcceptOffer(1, offer)));
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert_eq!([game.players[HUMAN].gold, game.players[1].gold], balances);
        assert!(!game.has_tech(HUMAN, offer.terms[1].technology.unwrap()));
        assert_eq!(game.vision_recipients(1), vec![1]);
    }

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
            .find(|tech| *tech == Tech::NUCLEAR_FISSION)
            .unwrap();
        game.players[HUMAN].known.push(fission);
        assert!(game.has_senate(HUMAN));
        game.apollo_completed = true;
        assert!(!game.has_senate(HUMAN));
    }
}
