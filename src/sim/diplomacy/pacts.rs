//! Treaty and pact state transitions.

use super::*;

impl Simulation {
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
            Government::Feudalism | Government::Democracy
        ) || (self
            .cities
            .iter()
            .any(|city| city.has_building(Building::UNITED_NATIONS))
            && self
                .completed_wonders
                .contains(&Building::MANHATTAN_PROJECT)
            && !self.apollo_completed
            && self.players[owner].known.contains(&Tech::NUCLEAR_FISSION))
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

    pub(super) fn pact_refusal(
        &self,
        receiver: usize,
        proposer: usize,
        proposed: Pact,
    ) -> Option<&'static str> {
        if self.players[receiver].ai
            && proposed != Pact::Ceasefire
            && self.ai_war_countdown(receiver, proposer) != -1
        {
            return Some("We are preparing for war or remain committed to the current war.");
        }
        if self.next_pact(receiver, proposer) != Some(proposed)
            && !(proposed == Pact::Alliance && self.pact(receiver, proposer) != Pact::Alliance)
        {
            return Some("We must honor the current treaty before negotiating the next one.");
        }
        if proposed != Pact::Ceasefire {
            let live_enemy = (0..self.players.len()).any(|third| {
                third != receiver
                    && third != proposer
                    && third != BARBARIANS
                    && self.player_alive(third)
                    && self.allied(proposer, third)
                    && self.diplomatic_at_war(receiver, third)
            });
            if live_enemy {
                return Some("Your ties to an enemy make this treaty impossible.");
            }
            if self
                .diplomatic_intel(receiver, proposer)
                .allied_with_enemy
                .is_some()
            {
                return Some("Keep your distance from our enemy for a while first.");
            }
            if proposed == Pact::Alliance
                && (0..self.players.len()).any(|third| {
                    third != receiver
                        && third != proposer
                        && third != BARBARIANS
                        && self.player_alive(third)
                        && self.allied(receiver, third)
                        && self.diplomatic_at_war(proposer, third)
                })
            {
                return Some("You are at war with our ally.");
            }
        }
        let empty = Relation::default();
        let relation = self
            .diplomacy
            .get(&Self::relation_key(receiver, proposer))
            .unwrap_or(&empty);
        if relation.first_contact.is_none() && !self.diplomatic_meeting_possible(receiver, proposer)
        {
            return Some("We have not made contact.");
        }
        if proposed == Pact::Alliance && relation.trust[usize::from(receiver > proposer)] < 1000 {
            return Some("We simply do not trust you with an alliance yet.");
        }
        None
    }

    pub(super) fn pact_clause_value(&self, owner: usize, other: usize, proposed: Pact) -> i64 {
        let empty = Relation::default();
        let relation = self
            .diplomacy
            .get(&Self::relation_key(owner, other))
            .unwrap_or(&empty);
        let love = relation.trust[usize::from(owner > other)] / 10;
        let missing = match proposed {
            Pact::Peace => love - 125,
            Pact::Alliance => love - 250,
            Pact::Ceasefire if self.turn - relation.first_contact.unwrap_or(0) < 15 => {
                return 0;
            }
            Pact::Ceasefire => love,
            _ => return 0,
        };
        // daidiplomacy.c:greed converts missing love into negative gold value.
        let deficit = i64::from(missing.min(0));
        let mut value = -(deficit * deficit / 10);
        if proposed == Pact::Alliance
            && !matches!(
                self.pact(owner, other),
                Pact::Peace | Pact::Armistice | Pact::Alliance
            )
        {
            let peace_deficit = i64::from((love - 125).min(0));
            value -= peace_deficit * peace_deficit / 10;
        }
        let shared_ally = self
            .diplomatic_intel(owner, other)
            .allied_with_ally
            .is_some();
        if shared_ally {
            value /= 2;
        }
        value
    }

    pub(in crate::sim) fn player_alive(&self, owner: usize) -> bool {
        self.cities.iter().any(|city| city.owner == owner)
            || self.units.iter().any(|unit| unit.owner == owner)
    }

    pub(in crate::sim) fn negotiate(&mut self, other: usize, proposed: Pact) -> bool {
        if !self.can_negotiate(other) {
            return false;
        }
        if let Some(reason) = self.pact_refusal(other, HUMAN, proposed) {
            self.add_event(format!("{} declined. {reason}", self.players[other].name));
            return false;
        }
        let offer = TreatyOffer {
            pact: Some(proposed),
            ..TreatyOffer::default()
        };
        if !self.execute_offer(HUMAN, other, offer) {
            self.add_event(format!(
                "{} requires compensation for that treaty.",
                self.players[other].name
            ));
            return false;
        }
        true
    }

    pub(in crate::sim) fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
        let ends_war =
            matches!(self.pact(a, b), Pact::War | Pact::Ceasefire) && proposed == Pact::Peace;
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
        if ends_war {
            for city in &mut self.cities {
                let nationality = if city.owner == a {
                    Some(b)
                } else if city.owner == b {
                    Some(a)
                } else {
                    None
                };
                if let Some(nationality) = nationality {
                    city.resisting_citizens
                        .retain(|&(owner, _)| owner != nationality);
                }
            }
        }
        // Native diplomacy has a permanent state independently of physical
        // contact; a treaty through an embassy can change NO_CONTACT without
        // calling first_contact. Its untouched first_contact_turn is zero.
        relation.first_contact.get_or_insert(0);
        for key in [(a, b), (b, a)] {
            if key.0 == BARBARIANS || key.1 == BARBARIANS {
                continue;
            }
            let memory = self
                .ai_diplomatic_memory
                .entry(key)
                .or_insert_with(|| DiplomaticMemory::for_pair(key.0, key.1));
            if Self::pact_rank(proposed) > Self::pact_rank(memory.strongest_pact) {
                memory.strongest_pact = proposed;
            }
        }
        self.ai_war_plans.remove(&(a, b));
        self.ai_war_plans.remove(&(b, a));
        if proposed == Pact::Peace {
            let mut returned_to_normalcy = Vec::new();
            for owner in [a, b] {
                if let Some(player) = self.players.get_mut(owner)
                    && player.mobilized
                {
                    player.mobilized = false;
                    returned_to_normalcy.push(player.name.clone());
                }
            }
            for name in returned_to_normalcy {
                self.add_event(format!("{name} returns to normalcy."));
            }
        }
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

    pub(in crate::sim) fn cancel_pact(&mut self, owner: usize, other: usize) -> bool {
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
        if pact != Pact::Alliance {
            self.record_war_incident(owner, other, pact);
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
        self.add_event(format!(
            "{} ended the {} with {}.",
            self.players[owner].name,
            pact.name().to_lowercase(),
            self.players[other].name
        ));
        true
    }
}
