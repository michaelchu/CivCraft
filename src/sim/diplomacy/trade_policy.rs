//! Proactive, trait-aware economic diplomacy. Treaty legality stays in offers.
use super::*;

mod scenarios;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema_version: u32,
    cadence_turns: i32,
    technologies_per_side: usize,
    seller_percent: i64,
    commercial_margin_percent: i64,
    liquidity_discount_percent: i64,
    scientific_bonus_percent: i64,
    trait_bonus_percent: i64,
    research_goal_bonus_percent: i64,
    military_need_bonus_percent: i64,
    military_minimum_trust: i32,
    military_minimum_power: i32,
    victory_technologies: Vec<String>,
    interests: BTreeMap<String, Vec<String>>,
    relationships: super::relationships::Policy,
}

#[cfg(test)]
mod tests;

fn policy() -> &'static Policy {
    static POLICY: OnceLock<Policy> = OnceLock::new();
    POLICY.get_or_init(|| {
        let policy: Policy = toml::from_str(include_str!(
            "../../../data/rulesets/civcraft/content/diplomacy.toml"
        ))
        .expect("valid diplomacy policy");
        assert_eq!(policy.schema_version, 1);
        policy.relationships.validate();
        assert!(policy.cadence_turns > 0 && (1..=16).contains(&policy.technologies_per_side));
        assert!((1..=100).contains(&policy.seller_percent));
        assert_eq!(
            policy.interests.len(),
            8,
            "all Conquests traits need trade interests"
        );
        assert!(policy.military_minimum_power > 0);
        for (name, technologies) in &policy.interests {
            assert!(matches!(
                name.as_str(),
                "agricultural"
                    | "commercial"
                    | "expansionist"
                    | "industrious"
                    | "militaristic"
                    | "religious"
                    | "scientific"
                    | "seafaring"
            ));
            for technology in technologies {
                assert!(
                    Tech::all().any(|tech| tech.name() == technology),
                    "unknown policy technology {technology}"
                );
            }
        }
        for technology in &policy.victory_technologies {
            assert!(
                Tech::all().any(|tech| tech.name() == technology),
                "unknown victory technology {technology}"
            );
        }
        policy
    })
}

#[derive(Clone, Serialize)]
struct Choice {
    offer: TreatyOffer,
    actor_gain: i64,
    partner_gain: i64,
}

#[derive(Serialize)]
struct Decision {
    actor: usize,
    other: usize,
    reason: &'static str,
    candidates: usize,
    rejected: BTreeMap<&'static str, usize>,
    reserve: i32,
    selected: Option<Choice>,
}
impl trace::Payload for Decision {
    const KIND: &'static str = "diplomacy.trade_decision";
}

#[derive(Clone, Copy)]
struct Asset {
    tech: Tech,
    ask: i64,
    bid: i64,
}

impl Simulation {
    /// Public traits for human estimates; AI pressure is always the owner's own.
    pub(in crate::sim) fn strategic_technology_percent(
        &self,
        owner: usize,
        tech: Tech,
        strategy: Option<&ai_strategy::StrategicPriorities>,
    ) -> i32 {
        let rules = policy();
        let mut percent = 100;
        if let Some(civ) = self.ai_civilization(owner) {
            if civ.scientific {
                percent += rules.scientific_bonus_percent as i32;
            }
            for (name, technologies) in &rules.interests {
                if ai_traits::trait_enabled(civ, name)
                    && technologies.iter().any(|name| name == tech.name())
                {
                    percent += rules.trait_bonus_percent as i32;
                }
            }
        }
        if let Some(strategy) = strategy {
            let interested = |category: &str| {
                rules.interests[category]
                    .iter()
                    .any(|name| name == tech.name())
            };
            let mut weight = 100;
            if interested("commercial") || interested("agricultural") || interested("industrious") {
                weight = weight.max(strategy.economy);
            }
            if interested("scientific")
                || (strategy.space_program
                    && rules
                        .victory_technologies
                        .iter()
                        .any(|name| name == tech.name()))
            {
                weight = weight.max(strategy.science);
            }
            if self.military_technology(tech) {
                weight = weight.max(strategy.military);
            }
            percent += (weight - 100) / 2;
            if self.military_technology(tech) && strategy.wars > 0 {
                percent += rules.military_need_bonus_percent as i32;
            }
        }
        percent
    }

    pub(in crate::sim) fn diplomatic_trade_reserve(&self, owner: usize) -> i32 {
        self.strategic_priorities(owner).gold_reserve
    }

    fn trade_spendable(&self, owner: usize) -> i64 {
        let reserve = if self.players[owner].ai {
            self.diplomatic_trade_reserve(owner)
        } else {
            0
        };
        i64::from(self.players[owner].gold.saturating_sub(reserve).max(0))
    }

    fn military_technology(&self, tech: Tech) -> bool {
        UnitKind::ALL_NATIVE.iter().any(|&kind| {
            kind.required_tech() == Some(tech)
                && (kind.stats().attack >= policy().military_minimum_power
                    || kind.stats().defense >= policy().military_minimum_power
                    || kind.conquests_rule().bombard_strength > 0)
        })
    }

    /// The same reservation values are used by proposal selection and acceptance.
    pub(in crate::sim) fn diplomatic_technology_value(
        &self,
        owner: usize,
        other: usize,
        tech: Tech,
        giving: bool,
    ) -> Result<i64, &'static str> {
        self.technology_value_with_reserve(
            owner,
            other,
            tech,
            giving,
            self.diplomatic_trade_reserve(owner),
            self.players[owner]
                .ai
                .then(|| self.strategic_priorities(owner))
                .as_ref(),
        )
    }

    fn technology_value_with_reserve(
        &self,
        owner: usize,
        other: usize,
        tech: Tech,
        giving: bool,
        reserve: i32,
        strategy: Option<&ai_strategy::StrategicPriorities>,
    ) -> Result<i64, &'static str> {
        if self.players[owner].ai && self.ai_war_countdown(owner, other) >= 0 {
            return Err("planned_hostility");
        }
        let (seller, buyer) = if giving {
            (owner, other)
        } else {
            (other, owner)
        };
        let (base, dangerous) = self
            .tech_trade_quote(seller, buyer, tech, true)
            .ok_or("not_tradable")?;
        let base = i64::from(base.max(1));
        let rules = policy();
        let civ = self.ai_civilization(owner);
        if giving {
            if (self.military_technology(tech) && self.relationship_caution(owner, other))
                || dangerous
                || (self.military_technology(tech)
                    && !self.allied(owner, other)
                    && (self.tech_trade_partner_is_dangerous(owner, other)
                        || self.diplomatic_love(owner, other) * 10 < rules.military_minimum_trust))
            {
                return Err("military_security");
            }
            if !self.allied(owner, other)
                && self.ai_space_race_leader() == Some(other)
                && rules
                    .victory_technologies
                    .iter()
                    .any(|name| name == tech.name())
            {
                return Err("victory_security");
            }
            let commercial = civ.is_some_and(|civ| civ.commercial);
            let percent = rules.seller_percent
                + if commercial {
                    rules.commercial_margin_percent
                } else {
                    0
                };
            // Liquidity pressure favors selling, but does not justify dumping
            // technology for nothing or exporting military advantages.
            let percent = if self.players[owner].ai && self.players[owner].gold < reserve {
                percent
                    .saturating_sub(rules.liquidity_discount_percent)
                    .max(1)
            } else {
                percent
            };
            let percent =
                (percent - i64::from(self.relationship_trade_adjustment(owner, other))).max(1);
            return Ok((base * percent / 100).max(1));
        }
        let mut percent = self.strategic_technology_percent(owner, tech, strategy) as i64
            + i64::from(self.relationship_trade_adjustment(owner, other));
        let player = &self.players[owner];
        // An AI evaluates its own priorities privately. Human willingness is
        // only estimated from the market and public traits, never their queue.
        if player.ai
            && (self
                .shared_research_path(owner, player.research_goal)
                .contains(&tech)
                || player.researching == tech)
        {
            percent += rules.research_goal_bonus_percent;
        }
        let mut value = base * percent / 100;
        if player.ai && player.researching == tech {
            let cost = self.target_cost(owner, tech).max(1);
            let remaining = cost.saturating_sub(player.research.max(0)).max(0);
            value = value * i64::from(remaining) / i64::from(cost);
        }
        let researched = player.known.len() as i32 + player.future_techs + 1;
        let minimum = (self.technology_upkeep(owner) / researched).max(1);
        if player.ai && player.last_science < minimum {
            if player.research < 0 {
                return Err("research_deficit");
            }
            value /= 2;
        }
        Ok(value.max(0))
    }

    fn trade_assets(
        &self,
        seller: usize,
        buyer: usize,
        rejected: &mut BTreeMap<&'static str, usize>,
    ) -> Vec<Asset> {
        let mut assets = Vec::new();
        let seller_reserve = self.diplomatic_trade_reserve(seller);
        let buyer_reserve = self.diplomatic_trade_reserve(buyer);
        let seller_strategy = self.players[seller]
            .ai
            .then(|| self.strategic_priorities(seller));
        let buyer_strategy = self.players[buyer]
            .ai
            .then(|| self.strategic_priorities(buyer));
        for tech in
            Tech::all().filter(|&tech| self.has_tech(seller, tech) && !self.has_tech(buyer, tech))
        {
            let values = self
                .technology_value_with_reserve(
                    seller,
                    buyer,
                    tech,
                    true,
                    seller_reserve,
                    seller_strategy.as_ref(),
                )
                .and_then(|ask| {
                    self.technology_value_with_reserve(
                        buyer,
                        seller,
                        tech,
                        false,
                        buyer_reserve,
                        buyer_strategy.as_ref(),
                    )
                    .map(|bid| Asset { tech, ask, bid })
                });
            match values {
                Ok(asset) => assets.push(asset),
                Err(reason) => *rejected.entry(reason).or_default() += 1,
            }
        }
        assets.sort_by_key(|asset| (std::cmp::Reverse(asset.bid), asset.tech.0));
        assets.truncate(policy().technologies_per_side);
        assets
    }

    fn choose_technology_trade(&self, actor: usize, other: usize) -> Decision {
        let mut decision = Decision {
            actor,
            other,
            reason: "no_viable_trade",
            candidates: 0,
            rejected: BTreeMap::new(),
            reserve: self.diplomatic_trade_reserve(actor),
            selected: None,
        };
        let incoming = self.trade_assets(other, actor, &mut decision.rejected);
        let outgoing = self.trade_assets(actor, other, &mut decision.rejected);
        let side = usize::from(actor > other);
        let actor_spendable = self.trade_spendable(actor);
        let partner_spendable = self.trade_spendable(other);
        let mut consider = |received: Option<Asset>, given: Option<Asset>| {
            decision.candidates += 1;
            let own_surplus =
                received.map_or(0, |asset| asset.bid) - given.map_or(0, |asset| asset.ask);
            let other_surplus =
                given.map_or(0, |asset| asset.bid) - received.map_or(0, |asset| asset.ask);
            let lower = (-other_surplus).max(-partner_spendable);
            let upper = own_surplus.min(actor_spendable);
            if lower > upper {
                let reason = if -other_surplus <= own_surplus {
                    "treasury_reserve"
                } else {
                    "valuation_gap"
                };
                *decision.rejected.entry(reason).or_default() += 1;
                return;
            }
            let gold = (lower + upper) / 2;
            let own_gain = own_surplus - gold;
            let other_gain = other_surplus + gold;
            if own_gain <= 0 || other_gain <= 0 {
                *decision.rejected.entry("no_mutual_gain").or_default() += 1;
                return;
            }
            let mut offer = TreatyOffer::default();
            offer.terms[side].technology = given.map(|asset| asset.tech);
            offer.terms[1 - side].technology = received.map(|asset| asset.tech);
            offer.terms[side].gold = gold.max(0).min(i64::from(i32::MAX)) as i32;
            offer.terms[1 - side].gold = (-gold).max(0).min(i64::from(i32::MAX)) as i32;
            let score = own_gain.min(other_gain) + (own_gain + other_gain) / 4;
            let better = decision.selected.as_ref().is_none_or(|old| {
                score
                    > old.actor_gain.min(old.partner_gain) + (old.actor_gain + old.partner_gain) / 4
            });
            if better
                && self.ai_accepts_treaty_exchange(
                    actor.min(other),
                    actor.max(other),
                    &offer.terms,
                    self.pact(actor, other),
                )
            {
                decision.selected = Some(Choice {
                    offer,
                    actor_gain: own_gain,
                    partner_gain: other_gain,
                });
                decision.reason = "mutual_strategic_gain";
            }
        };
        for &asset in &incoming {
            consider(Some(asset), None);
        }
        for &asset in &outgoing {
            consider(None, Some(asset));
        }
        for &received in &incoming {
            for &given in &outgoing {
                consider(Some(received), Some(given));
            }
        }
        decision
    }

    pub(super) fn advance_proactive_trades_for(&mut self, owner: usize) {
        if owner >= self.civilization_count || !self.players[owner].ai || !self.player_alive(owner)
        {
            return;
        }
        for other in 0..self.civilization_count {
            if owner == other
                || !self.player_alive(other)
                || self.player_has_no_diplomacy(owner)
                || self.player_has_no_diplomacy(other)
                || !self.diplomatic_meeting_possible(owner, other)
                || self
                    .contacted_pact(owner, other)
                    .is_none_or(|pact| pact == Pact::War)
            {
                continue;
            }
            let key = Self::relation_key(owner, other);
            let relation = &self.diplomacy[&key];
            if relation.offer_after > self.turn || relation.offer.is_some() {
                continue;
            }
            self.diplomacy.get_mut(&key).unwrap().offer_after =
                self.turn.saturating_add(policy().cadence_turns);
            let decision = self.choose_technology_trade(owner, other);
            let selected = decision.selected.clone();
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
            if let Some(choice) = selected {
                self.trace_diplomacy(
                    owner,
                    other,
                    "offer_proposed",
                    "proactive_trade",
                    self.contacted_pact(owner, other),
                    Some(&choice.offer),
                );
                if self.players[other].ai {
                    self.execute_offer(key.0, key.1, &choice.offer);
                } else {
                    self.diplomacy.get_mut(&key).unwrap().offer = Some(choice.offer);
                    self.add_event(format!(
                        "{} offers a technology trade.",
                        self.players[owner].name
                    ));
                }
            }
        }
    }
}

pub(super) fn relationship_policy() -> &'static super::relationships::Policy {
    &policy().relationships
}
