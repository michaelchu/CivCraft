//! Bounded, shared proposal and acceptance policy using visible pressure.
use super::*;

impl Simulation {
    pub(in crate::sim::diplomacy) fn ai_accepts_vassalage(
        &self,
        a: usize,
        b: usize,
        terms: VassalTerms,
    ) -> bool {
        let vassal = if terms.master == a { b } else { a };
        let p = policy();
        let own = self.diplomatic_military_force(vassal, vassal).max(1);
        let protector = self.diplomatic_military_force(vassal, terms.master);
        let willing = match terms.kind {
            VassalKind::Capitulation => {
                protector >= own * p.submission_force_ratio
                    && self.diplomatic_peace_value(vassal, terms.master) >= p.minimum_peace_value
            }
            VassalKind::Voluntary => {
                self.diplomatic_love(vassal, terms.master) * 10 >= p.voluntary_minimum_trust
                    && protector >= own * p.submission_force_ratio
                    && (0..self.civilization_count).any(|enemy| {
                        self.contacted_pact(vassal, enemy) == Some(Pact::War)
                            && self.diplomatic_peace_value(vassal, enemy) >= p.minimum_peace_value
                    })
            }
        };
        if !willing || !self.players[terms.master].ai {
            return willing;
        }
        if self.strategic_priorities(terms.master).economic_crisis {
            return false;
        }
        let new_pressure: i64 = (0..self.civilization_count)
            .filter(|&enemy| {
                enemy != terms.master
                    && enemy != vassal
                    && self.contacted_pact(vassal, enemy) == Some(Pact::War)
                    && self.contacted_pact(terms.master, enemy) != Some(Pact::War)
                    && terms.kind == VassalKind::Voluntary
            })
            .map(|enemy| self.diplomatic_military_force(terms.master, enemy))
            .sum();
        new_pressure * 100
            <= self
                .diplomatic_military_force(terms.master, terms.master)
                .max(1)
                * p.maximum_protection_pressure_percent
    }

    pub(in crate::sim::diplomacy) fn advance_vassal_offers_for(&mut self, vassal: usize) {
        if vassal >= self.civilization_count
            || !self.players[vassal].ai
            || self.vassal_master(vassal).is_some()
            || !self.player_alive(vassal)
        {
            return;
        }
        for master in 0..self.civilization_count {
            let key = Self::relation_key(master, vassal);
            if master == vassal
                || !self.diplomatic_meeting_possible(vassal, master)
                || self.diplomacy.get(&key).is_some_and(|r| r.offer.is_some())
                || self
                    .vassalage
                    .offer_after
                    .get(&key)
                    .is_some_and(|&t| t > self.turn)
            {
                continue;
            }
            let terms = VassalTerms {
                master,
                kind: if self.at_war(vassal, master) {
                    VassalKind::Capitulation
                } else {
                    VassalKind::Voluntary
                },
            };
            if self.plan_vassalage(master, vassal, terms).is_err()
                || !self.ai_accepts_vassalage(master, vassal, terms)
            {
                continue;
            }
            self.vassalage
                .offer_after
                .insert(key, self.turn + policy().negotiation_cooldown_turns);
            let offer = TreatyOffer {
                vassalage: Some(terms),
                ..TreatyOffer::default()
            };
            if self.players[master].ai {
                self.execute_offer(master, vassal, &offer);
            } else {
                self.diplomacy.get_mut(&key).expect("contact exists").offer = Some(offer.clone());
                self.trace_diplomacy(
                    vassal,
                    master,
                    "offer_created",
                    "vassalage",
                    None,
                    Some(&offer),
                );
                self.add_event(format!(
                    "{} offers {}.",
                    self.players[vassal].name,
                    terms.kind.name()
                ));
            }
            break;
        }
    }
}
