//! Bounded negotiations over existing gold, information and peace clauses.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    aid_minimum_trust: i32,
    aid_surplus_percent: i32,
    maximum_aid: i32,
    map_minimum_trust: i32,
    map_minimum_tiles: usize,
    map_tiles_per_gold: i64,
    map_seller_percent: i64,
    map_trait_bonus_percent: i64,
    maximum_map_value: i64,
    peace_income_horizon: i32,
    peace_visible_force_ratio: i64,
    peace_force_value: i64,
    maximum_peace_value: i64,
}
impl Policy {
    pub(super) fn validate(&self) {
        assert!((0..=10000).contains(&self.aid_minimum_trust));
        assert!((0..=10000).contains(&self.map_minimum_trust));
        assert!((1..=50).contains(&self.aid_surplus_percent));
        assert!((1..=10000).contains(&self.maximum_aid));
        assert!((1..=10000).contains(&self.map_minimum_tiles));
        for value in [
            self.map_tiles_per_gold,
            self.map_seller_percent,
            self.map_trait_bonus_percent,
            self.maximum_map_value,
            i64::from(self.peace_income_horizon),
            self.peace_visible_force_ratio,
            self.peace_force_value,
            self.maximum_peace_value,
        ] {
            assert!((1..=1000).contains(&value));
        }
        assert!(self.map_seller_percent < 100);
    }
}
fn policy() -> &'static Policy {
    super::trade_policy::negotiation_policy()
}

#[derive(Serialize)]
struct Decision {
    actor: usize,
    other: usize,
    reason: &'static str,
    actor_reserve: i32,
    funding_shortfall: i32,
    peace_value: [i64; 2],
    compensation: i64,
    selected: Option<TreatyOffer>,
}
impl trace::Payload for Decision {
    const KIND: &'static str = "diplomacy.negotiation_decision";
}

impl Simulation {
    /// The recipient's own funding assessment is shared when requesting aid.
    /// Human queues and funding preferences are never inferred.
    pub(in crate::sim) fn diplomatic_aid_allowance(&self, donor: usize, recipient: usize) -> i32 {
        let trust = if self.players[donor].ai {
            self.diplomatic_love(donor, recipient)
        } else {
            // The requesting AI's trust gates a request to a human, whose
            // willingness remains an explicit approval decision.
            self.diplomatic_love(recipient, donor)
        };
        if !self.players[recipient].ai
            || !self.allied(donor, recipient)
            || donor == recipient
            || !self.player_alive(donor)
            || !self.player_alive(recipient)
            || self.relationship_caution(donor, recipient)
            || self.ai_war_countdown(donor, recipient) >= 0
            || self.ai_war_countdown(recipient, donor) >= 0
            || trust * 10 < policy().aid_minimum_trust
        {
            return 0;
        }
        let reserve = if self.players[donor].ai {
            let strategy = self.strategic_priorities(donor);
            if strategy.economic_crisis
                || strategy.preparing
                || self.diplomatic_funding_shortfall(donor) > 0
            {
                return 0;
            }
            self.diplomatic_trade_reserve(donor)
                .max(self.diplomatic_funding_target(donor))
        } else {
            0
        };
        let surplus = i64::from(self.players[donor].gold.saturating_sub(reserve).max(0));
        let budget = surplus * i64::from(policy().aid_surplus_percent) / 100;
        self.diplomatic_funding_shortfall(recipient)
            .min(policy().maximum_aid)
            .min(budget as i32)
    }

    /// Use only information the giver can actually transfer. Old/repeated maps
    /// do not become an inexhaustible source of trade value.
    fn new_map_tiles(&self, giver: usize, receiver: usize, map: TreatyMap) -> usize {
        self.observed_map
            .tiles
            .range((giver, 0)..=(giver, usize::MAX))
            .filter(|((_, index), _)| {
                let tile = &self.tiles[*index];
                tile.known_by[giver]
                    && !tile.known_by[receiver]
                    && (map != TreatyMap::Sea || tile.terrain.is_water())
            })
            .count()
    }

    pub(in crate::sim) fn diplomatic_map_value(
        &self,
        owner: usize,
        other: usize,
        map: TreatyMap,
        giving: bool,
        after: Pact,
    ) -> Option<i64> {
        let (giver, receiver) = if giving {
            (owner, other)
        } else {
            (other, owner)
        };
        let tiles = self.new_map_tiles(giver, receiver, map);
        if tiles < policy().map_minimum_tiles
            || !matches!(after, Pact::Peace | Pact::Armistice | Pact::Alliance)
        {
            return if giving {
                Some(self.ai_treaty_map_clause_value(owner, other, map, after))
            } else {
                Some(0)
            };
        }
        let base = (tiles as i64 / policy().map_tiles_per_gold).min(policy().maximum_map_value);
        if giving {
            if self.players[owner].ai
                && (self.ai_war_countdown(owner, other) >= 0
                    || self.relationship_caution(owner, other)
                    || (!self.allied(owner, other)
                        && self.diplomatic_love(owner, other) * 10 < policy().map_minimum_trust)
                    || !self.vision_safe(owner, other)
                    || (!self.allied(owner, other)
                        && self.tech_trade_partner_is_dangerous(owner, other)))
            {
                return None;
            }
            return Some(if after == Pact::Alliance {
                0
            } else {
                -(base * policy().map_seller_percent / 100).max(1)
            });
        }
        let traits = self.ai_civilization(owner);
        let trait_bonus = if traits.is_some_and(|civ| civ.expansionist || civ.seafaring) {
            policy().map_trait_bonus_percent
        } else {
            0
        };
        let priority = if self.players[owner].ai {
            let strategy = self.strategic_priorities(owner);
            i64::from(strategy.economy.max(strategy.science))
        } else {
            100
        };
        Some((base * (priority + trait_bonus) / 100).min(policy().maximum_map_value))
    }

    /// Recovery is valuable under an actual deficit or observed military
    /// disadvantage. Hidden enemy armies and research do not enter this value.
    pub(super) fn diplomatic_peace_value(&self, owner: usize, other: usize) -> i64 {
        if !self.players[owner].ai
            || !matches!(
                self.contacted_pact(owner, other),
                Some(Pact::War | Pact::Ceasefire)
            )
            || (0..self.civilization_count)
                .any(|ally| self.benefits_from_ally_support(ally, owner, other))
        {
            return 0;
        }
        let strategy = self.strategic_priorities(owner);
        let own = self.diplomatic_military_force(owner, owner);
        let enemy = self.diplomatic_military_force(owner, other);
        let military =
            (enemy - own * policy().peace_visible_force_ratio).max(0) * policy().peace_force_value;
        let fiscal = if strategy.economic_crisis {
            i64::from(strategy.projected_income.saturating_neg().max(0))
                * i64::from(policy().peace_income_horizon)
        } else {
            0
        };
        let multi_front = if strategy.wars >= 2 {
            (strategy.wars - 1) as i64 * 80
        } else {
            0
        };
        ((fiscal + military + multi_front) * i64::from(strategy.economy + strategy.science)
            / i64::from(2 * strategy.military).max(1))
        .min(policy().maximum_peace_value)
    }

    fn need_decision(&self, owner: usize, other: usize) -> Decision {
        Decision {
            actor: owner,
            other,
            reason: "no_affordable_agreement",
            actor_reserve: self.diplomatic_trade_reserve(owner),
            funding_shortfall: 0,
            peace_value: [0; 2],
            compensation: 0,
            selected: None,
        }
    }

    fn recovery_negotiation(&self, owner: usize, other: usize) -> Option<Decision> {
        if !matches!(
            self.contacted_pact(owner, other),
            Some(Pact::War | Pact::Ceasefire | Pact::Alliance)
        ) {
            return None;
        }
        let mut decision = self.need_decision(owner, other);
        decision.peace_value = [
            self.diplomatic_peace_value(owner, other),
            self.diplomatic_peace_value(other, owner),
        ];
        if decision.peace_value[0] > 0 {
            let pact = self.next_pact(owner, other)?;
            if !matches!(pact, Pact::Ceasefire | Pact::Peace)
                || self.pact_refusal(owner, other, pact).is_some()
                || self.pact_refusal(other, owner, pact).is_some()
            {
                return None;
            }
            let mut offer = TreatyOffer {
                pact: Some(pact),
                ..Default::default()
            };
            // A reluctant counterpart can receive compensation, bounded by
            // the proposer's recovery benefit and spendable reserves.
            let compensation = if self.players[other].ai {
                (-self.pact_clause_value(other, owner, pact)).max(0)
            } else {
                0
            };
            decision.compensation = compensation;
            let spendable = self.players[owner]
                .gold
                .saturating_sub(decision.actor_reserve)
                .max(0);
            if compensation <= decision.peace_value[0] && compensation <= i64::from(spendable) {
                offer.terms[usize::from(owner > other)].gold = compensation as i32;
                if self.accepts_negotiated_offer(owner, other, &offer) {
                    decision.reason = "war_recovery";
                    decision.selected = Some(offer);
                }
            }
            return Some(decision);
        }
        if !self.allied(owner, other) {
            return None;
        }
        for recipient in [owner, other] {
            if !self.players[recipient].ai {
                continue;
            }
            let donor = if recipient == owner { other } else { owner };
            let shortfall = self.diplomatic_funding_shortfall(recipient);
            let amount = self.diplomatic_aid_allowance(donor, recipient);
            decision.funding_shortfall = decision.funding_shortfall.max(shortfall);
            if amount > 0 {
                let mut offer = TreatyOffer::default();
                offer.terms[usize::from(donor > recipient)].gold = amount;
                if self.accepts_negotiated_offer(owner, other, &offer) {
                    decision.reason = "ally_funding";
                    decision.selected = Some(offer);
                    return Some(decision);
                }
            }
        }
        (decision.funding_shortfall > 0).then_some(decision)
    }

    fn accepts_negotiated_offer(&self, a: usize, b: usize, offer: &TreatyOffer) -> bool {
        let key = Self::relation_key(a, b);
        let after = offer.pact.unwrap_or_else(|| self.pact(a, b));
        [(key.0, key.1), (key.1, key.0)]
            .into_iter()
            .enumerate()
            .all(|(side, (owner, other))| {
                !self.players[owner].ai
                    || (offer
                        .pact
                        .is_none_or(|pact| self.pact_refusal(owner, other, pact).is_none())
                        && self
                            .ai_treaty_clause_balance(
                                owner,
                                other,
                                &offer.terms[side],
                                &offer.terms[1 - side],
                                after,
                            )
                            .is_some_and(|(balance, _)| {
                                balance
                                    + offer.pact.map_or(0, |pact| {
                                        self.pact_clause_value(owner, other, pact)
                                    })
                                    >= 0
                            }))
            })
    }

    fn information_negotiation(&self, owner: usize, other: usize) -> Option<Decision> {
        if self.ai_war_countdown(owner, other) >= 0 || self.ai_war_countdown(other, owner) >= 0 {
            return None;
        }
        let mut decision = self.need_decision(owner, other);
        let side = usize::from(owner > other);
        let after = self.pact(owner, other);
        if self.allied(owner, other)
            && self.strategic_priorities(owner).wars > 0
            && self.new_map_tiles(other, owner, TreatyMap::World) >= policy().map_minimum_tiles
        {
            let offer = self.source_allied_offer(owner, other);
            if offer.bundled() && self.accepts_negotiated_offer(owner, other, &offer) {
                decision.reason = "allied_information";
                decision.selected = Some(offer);
                return Some(decision);
            }
        }
        for map in [TreatyMap::World, TreatyMap::Sea] {
            if self.new_map_tiles(owner, other, map) < policy().map_minimum_tiles
                || self.new_map_tiles(other, owner, map) < policy().map_minimum_tiles
            {
                continue;
            }
            let offer = TreatyOffer {
                terms: std::array::from_fn(|_| Concessions::default().set_map(Some(map))),
                ..Default::default()
            };
            if self.accepts_negotiated_offer(owner, other, &offer) {
                decision.reason = "map_exchange";
                decision.selected = Some(offer);
                return Some(decision);
            }
        }
        for giver in [other, owner] {
            let receiver = if giver == owner { other } else { owner };
            for map in [TreatyMap::World, TreatyMap::Sea] {
                if self.new_map_tiles(giver, receiver, map) < policy().map_minimum_tiles {
                    continue;
                }
                let Some(ask) = self.diplomatic_map_value(giver, receiver, map, true, after) else {
                    continue;
                };
                let ask = -ask;
                let Some(bid) = self.diplomatic_map_value(receiver, giver, map, false, after)
                else {
                    continue;
                };
                let reserve = if self.players[receiver].ai {
                    self.diplomatic_trade_reserve(receiver)
                } else {
                    0
                };
                let spendable = self.players[receiver].gold.saturating_sub(reserve).max(0);
                if ask > bid || ask > i64::from(spendable) {
                    continue;
                }
                let gold = if after == Pact::Alliance {
                    0
                } else {
                    ((ask + bid) / 2).min(i64::from(spendable))
                };
                let mut offer = TreatyOffer::default();
                let giver_side = if giver == owner { side } else { 1 - side };
                offer.terms[giver_side] = Concessions::default().set_map(Some(map));
                offer.terms[1 - giver_side].gold = gold as i32;
                if self.accepts_negotiated_offer(owner, other, &offer) {
                    decision.reason = "useful_map";
                    decision.selected = Some(offer);
                    return Some(decision);
                }
            }
        }
        None
    }

    pub(super) fn trace_recovery_offer(
        &mut self,
        owner: usize,
        other: usize,
        offer: &TreatyOffer,
    ) -> bool {
        if !offer
            .pact
            .is_some_and(|pact| matches!(pact, Pact::Ceasefire | Pact::Peace))
            || self.diplomatic_peace_value(owner, other) == 0
        {
            return false;
        }
        let mut decision = self.need_decision(owner, other);
        decision.reason = "war_recovery";
        decision.peace_value = [
            self.diplomatic_peace_value(owner, other),
            self.diplomatic_peace_value(other, owner),
        ];
        decision.selected = Some(offer.clone());
        self.trace.emit(
            self.turn,
            || {
                vec![
                    trace::Entity::Civilization(owner),
                    trace::Entity::Civilization(other),
                ]
            },
            || decision,
        );
        true
    }

    pub(super) fn advance_need_negotiation(
        &mut self,
        owner: usize,
        other: usize,
        information: bool,
    ) -> bool {
        let decision = if information {
            self.information_negotiation(owner, other)
        } else {
            self.recovery_negotiation(owner, other)
        };
        let Some(decision) = decision else {
            return false;
        };
        let selected = decision.selected.clone();
        let reason = decision.reason;
        self.trace.emit(
            self.turn,
            || {
                vec![
                    trace::Entity::Civilization(owner),
                    trace::Entity::Civilization(other),
                ]
            },
            || decision,
        );
        if let Some(offer) = selected {
            self.send_ai_negotiated_offer(owner, other, &offer, reason);
            true
        } else {
            false
        }
    }

    pub(super) fn send_ai_negotiated_offer(
        &mut self,
        owner: usize,
        other: usize,
        offer: &TreatyOffer,
        reason: &'static str,
    ) {
        self.trace_diplomacy(
            owner,
            other,
            "offer_proposed",
            reason,
            self.contacted_pact(owner, other),
            Some(offer),
        );
        let key = Self::relation_key(owner, other);
        if self.players[other].ai {
            self.execute_offer(key.0, key.1, offer);
        } else {
            self.diplomacy.get_mut(&key).unwrap().offer = Some(offer.clone());
            self.add_event(format!("{} proposes a treaty.", self.players[owner].name));
        }
    }
}

#[cfg(test)]
mod tests;
