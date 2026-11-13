//! Technology exchange rules in diplomacy.

use super::research::campaign_research_settings;
use super::*;
use crate::rules::freeciv::ResearchRulesetSettings;

impl Simulation {
    /// Runs Freeciv's receiver-side transfer-loss roll for a technology treaty.
    ///
    /// `server/techtools.c::tech_transfer` always draws once for
    /// `techlost_recv`, even when the default chance is zero. The setting is
    /// an engine setting rather than a ruleset parameter, so the caller passes
    /// the campaign's resolved chance (currently Freeciv's default of zero).
    /// Keep the draw here so successful trades preserve the shared gameplay
    /// random stream.
    pub(super) fn technology_transfer_succeeds(&mut self, receiver_loss_chance: i32) -> bool {
        self.rand_below(100) >= receiver_loss_chance.clamp(0, 100)
    }

    pub(super) fn transfer_treaty_technology(
        &mut self,
        donor: usize,
        receiver: usize,
        tech: Tech,
    ) -> bool {
        self.transfer_treaty_technology_with_settings(
            donor,
            receiver,
            tech,
            *campaign_research_settings(),
        )
    }

    fn transfer_treaty_technology_with_settings(
        &mut self,
        donor: usize,
        receiver: usize,
        tech: Tech,
        settings: ResearchRulesetSettings,
    ) -> bool {
        if !self.has_tech(donor, tech)
            || !self.technology_trade_receivable(receiver, tech, settings.tech_trade_allow_holes)
        {
            return false;
        }
        if settings.tech_lost_donor_percent > 0
            && self.donor_can_lose_treaty_technology(donor, tech, settings)
            && self.rand_below(100) < settings.tech_lost_donor_percent
        {
            self.players[donor].known.retain(|&known| known != tech);
            // The treaty-transfer path calls research_tech_lost() directly;
            // only the upkeep-driven loss path clears suspended research.
            self.handle_government_tech_loss(donor, tech);
            if donor == HUMAN {
                self.add_event(format!(
                    "You lost {} while transferring the technology.",
                    tech.name()
                ));
            }
        }

        if !self.technology_transfer_succeeds(settings.tech_lost_recv_percent) {
            return false;
        }

        let was_researching = self.players[receiver].researching == tech;
        self.apply_research_penalty(receiver, tech, settings.diplomatic_bulb_cost_percent);
        self.discover(receiver, tech);
        if was_researching {
            let free_bulbs = self.players[receiver].research;
            let next = self.choose_research_target(receiver);
            self.choose_research_tech(receiver, next, free_bulbs);
        }
        true
    }

    fn donor_can_lose_treaty_technology(
        &self,
        donor: usize,
        tech: Tech,
        settings: ResearchRulesetSettings,
    ) -> bool {
        settings.tech_trade_loss_allow_holes
            || self.players[donor]
                .known
                .iter()
                .all(|&known| !known.civ3_prerequisites().unwrap_or(&[]).contains(&tech))
    }

    fn technology_gold_equivalent(&self, buyer: usize, tech: Tech, include_advisor: bool) -> i32 {
        // Civ2Civ3 disables trades that would leave holes in the tech tree.
        if !self.technology_trade_receivable(
            buyer,
            tech,
            campaign_research_settings().tech_trade_allow_holes,
        ) {
            return 0;
        }
        let bulbs: i32 = self
            .research_path(buyer, tech)
            .into_iter()
            .map(|required| self.target_cost(buyer, required))
            .sum();
        let want = if include_advisor {
            (self.advisor_tech_want(buyer, tech).max(0.0) / self.turn.max(1) as f32) as i32
        } else {
            0
        };
        // With no holes allowed, all eligible buyers already know the prerequisites.
        (3 * bulbs + want) / 2
    }

    pub(super) fn tech_sell_price(&self, seller: usize, buyer: usize, tech: Tech) -> Option<i32> {
        self.tech_sell_price_with_advisor(seller, buyer, tech, true)
    }

    pub(super) fn tech_sell_price_with_advisor(
        &self,
        seller: usize,
        buyer: usize,
        tech: Tech,
        include_advisor: bool,
    ) -> Option<i32> {
        self.tech_trade_quote(seller, buyer, tech, include_advisor)
            .and_then(|(price, danger)| (!danger).then_some(price))
    }

    pub(super) fn tech_trade_quote(
        &self,
        seller: usize,
        buyer: usize,
        tech: Tech,
        include_advisor: bool,
    ) -> Option<(i32, bool)> {
        let settings = campaign_research_settings();
        if !self.has_tech(seller, tech)
            || !self.technology_trade_receivable(buyer, tech, settings.tech_trade_allow_holes)
        {
            return None;
        }
        let mut price = self.technology_gold_equivalent(buyer, tech, include_advisor);
        let mut dangerous = false;
        if self.allied(seller, buyer) {
            price /= 2;
        }
        for third in 0..self.players.len() {
            if third == seller
                || third == buyer
                || third == BARBARIANS
                || !self.player_alive(third)
                || self.has_tech(third, tech)
                || !self.allied(buyer, third)
            {
                continue;
            }
            dangerous |= self.tech_trade_partner_is_dangerous(seller, third);
            if !self.allied(seller, third) {
                price += self.technology_gold_equivalent(third, tech, include_advisor) / 4;
            }
        }
        Some((price, dangerous))
    }

    fn technology_trade_receivable(&self, receiver: usize, tech: Tech, allow_holes: bool) -> bool {
        if !tech.is_civ3_advance() || self.has_tech(receiver, tech) {
            return false;
        }
        if allow_holes {
            // The preserved Civ III technology graph has no AR_ROOT
            // requirement class. C2C3 also has no root_req entries, so
            // Freeciv's allow-holes predicate accepts any unknown C3 advance.
            // Ordinary prerequisites still control research availability.
            true
        } else {
            self.tech_available(receiver, tech)
        }
    }

    pub(super) fn ai_treaty_clause_balance(
        &self,
        owner: usize,
        other: usize,
        give: &diplomacy::Concessions,
        receive: &diplomacy::Concessions,
        after: Pact,
    ) -> Option<(i64, bool)> {
        // Native daidiplomacy.c treats city clauses as more than gifts, even
        // when a city is offered for free. Its at-war filter rejects them
        // unless a peace clause moves the resulting state out of war.
        if after == Pact::War
            && (give.embassy()
                || receive.embassy()
                || !give.cities().is_empty()
                || !receive.cities().is_empty()
                // C2C3 uses Linear tech costs, so the default AI does not
                // classify advance clauses as harmless gifts while at war.
                || give.technology.is_some()
                || receive.technology.is_some())
        {
            return None;
        }
        let owner_city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count();
        if !give.cities().is_empty() && owner_city_count.saturating_sub(give.cities().len()) <= 2 {
            // daidiplomacy.c::dai_treaty_evaluate always keeps at least two
            // cities when the AI is the giver.
            return None;
        }
        if give.shared_vision() && (after != Pact::Alliance || !self.vision_safe(owner, other)) {
            return None;
        }
        let mut clauses = vec![-i64::from(give.gold), i64::from(receive.gold)];
        if give.embassy() {
            let love = self.diplomatic_love(owner, other);
            let turn = i64::from(self.turn);
            clauses.push(match after {
                Pact::Alliance => 0,
                Pact::Peace => -5 * turn,
                _ => (-50 * turn + i64::from(love)).min(-5 * turn),
            });
        }
        if receive.embassy() {
            // Civ2Civ3 uses Embassies leakage, with the default techleak=100.
            clauses.push(10);
        }
        for map in give.maps() {
            clauses.push(self.ai_treaty_map_clause_value(owner, other, map, after));
        }
        for &id in give.cities() {
            let city = self
                .cities
                .iter()
                .find(|city| city.id == id && city.owner == owner)?;
            clauses.push(-i64::from(self.city_clause_value(city, other)));
        }
        for &id in receive.cities() {
            let city = self
                .cities
                .iter()
                .find(|city| city.id == id && city.owner == other)?;
            clauses.push(i64::from(self.city_gold_worth(city)));
        }
        if let Some(tech) = give.technology {
            clauses.push(-i64::from(self.tech_sell_price(owner, other, tech)?));
        }
        if let Some(tech) = receive.technology {
            // Native receiving valuation ignores the giver's danger flag;
            // only an AI giver itself may refuse that concession.
            let (mut price, _) = self.tech_trade_quote(other, owner, tech, true)?;
            let player = &self.players[owner];
            let researched = player.known.len() as i32 + player.future_techs + 1;
            let minimum = (self.technology_upkeep(owner) / researched).max(1);
            if player.last_science < minimum {
                if player.research < 0 {
                    return None;
                }
                price /= 2;
            }
            clauses.push(i64::from(price));
        }
        Some((
            clauses.iter().sum(),
            clauses.iter().all(|&value| value >= 0),
        ))
    }

    fn ai_treaty_map_clause_value(
        &self,
        owner: usize,
        other: usize,
        map: TreatyMap,
        after: Pact,
    ) -> i64 {
        if after == Pact::Alliance {
            // The default AI treats both maps as useless to an ally.
            return 0;
        }
        let city_count = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count() as i64;
        let base_cost = match map {
            TreatyMap::Sea => 15 * city_count,
            TreatyMap::World => 40 * city_count.max(1) * if after == Pact::War { 2 } else { 1 },
        };
        let love = i64::from(self.diplomatic_love(owner, other));
        let love_limit = match map {
            TreatyMap::Sea => 7 * love,
            TreatyMap::World => 10 * love,
        };
        // Freeciv clips the negative map value with MIN(love * scale, -cost).
        love_limit.min(-base_cost)
    }

    pub(super) fn ai_accepts_treaty_exchange(
        &self,
        a: usize,
        b: usize,
        terms: &[diplomacy::Concessions; 2],
        after: Pact,
    ) -> bool {
        [(a, b), (b, a)]
            .into_iter()
            .enumerate()
            .all(|(side, (owner, other))| {
                !self.players[owner].ai
                    || self
                        .ai_treaty_clause_balance(
                            owner,
                            other,
                            &terms[side],
                            &terms[1 - side],
                            after,
                        )
                        .is_some_and(|(value, _)| value >= 0)
            })
    }
}

#[cfg(test)]
mod arithmetic_tests {
    use super::*;

    #[test]
    fn cached_adv_want_is_divided_as_float_before_c_integer_conversion() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[1].known.clear();
        sim.turn = 3;
        let mut wants = vec![0.0; Tech::all().count()];
        wants[Tech::POTTERY.0] = 67_108_864.0;
        sim.ai_tech_wants.insert(1, wants);
        let base = 3 * sim.target_cost(1, Tech::POTTERY);
        // Independent C: int tech_want = 67108864.f / 3;
        // gives 22369622 (float rounding), while early conversion followed
        // by integer division incorrectly gives 22369621.
        assert_eq!(
            sim.technology_gold_equivalent(1, Tech::POTTERY, true),
            (base + 22_369_622) / 2
        );
        assert_ne!(
            sim.technology_gold_equivalent(1, Tech::POTTERY, true),
            (base + 22_369_621) / 2
        );
        sim.ai_tech_wants.get_mut(&1).unwrap()[Tech::POTTERY.0] = -2.75;
        assert_eq!(
            sim.technology_gold_equivalent(1, Tech::POTTERY, true),
            base / 2
        );
    }

    #[test]
    fn missing_and_human_snapshots_have_no_recomputed_technology_premium() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[1].known.clear();
        sim.players[HUMAN].known.clear();
        sim.ai_tech_wants.clear();
        for owner in [HUMAN, 1] {
            assert_eq!(sim.advisor_tech_want(owner, Tech::POTTERY), 0.0);
            assert_eq!(
                sim.technology_gold_equivalent(owner, Tech::POTTERY, true),
                sim.technology_gold_equivalent(owner, Tech::POTTERY, false)
            );
        }
        let mut wants = vec![0.0; Tech::all().count()];
        wants[Tech::POTTERY.0] = 17.75;
        sim.ai_tech_wants.insert(1, wants);
        let cached = sim.technology_gold_equivalent(1, Tech::POTTERY, true);
        sim.players[1].gold += 1000;
        sim.players[1].research_goal = Tech::MONARCHY;
        sim.players[1].known.push(Tech::ALPHABET);
        assert_eq!(sim.advisor_tech_want(1, Tech::POTTERY), 17.75);
        assert_eq!(
            sim.technology_gold_equivalent(1, Tech::POTTERY, true),
            cached
        );
    }

    #[test]
    fn technology_transfer_consumes_receiver_roll_even_when_default_chance_is_zero() {
        let mut actual = Simulation::new(crate::mapgen::generate(42));
        let mut expected = actual.clone();

        assert!(actual.technology_transfer_succeeds(0));
        expected.rand_below(100);

        assert_eq!(actual.random_snapshot(), expected.random_snapshot());
    }

    #[test]
    fn technology_transfer_uses_the_configured_receiver_loss_chance() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let mut expected = sim.clone();
        let roll = expected.rand_below(100);

        assert_eq!(sim.technology_transfer_succeeds(100), roll >= 100);
        assert_eq!(sim.random_snapshot(), expected.random_snapshot());
    }

    #[test]
    fn accepted_technology_transfer_discovers_the_technology_and_consumes_receiver_draw() {
        let mut actual = Simulation::new(crate::mapgen::generate(42));
        let mut expected = actual.clone();
        actual.players[HUMAN].known.clear();
        expected.players[HUMAN].known.clear();
        actual.players[1].known = vec![Tech::ALPHABET];

        assert!(actual.transfer_treaty_technology(1, HUMAN, Tech::ALPHABET));
        expected.rand_below(100);

        assert!(actual.has_tech(HUMAN, Tech::ALPHABET));
        assert_eq!(actual.random_snapshot(), expected.random_snapshot());
    }

    #[test]
    fn allow_holes_ruleset_setting_allows_receiving_an_advance_without_its_c3_prerequisites() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        sim.players[1].known = vec![Tech::INVENTION];
        let mut settings = *campaign_research_settings();

        assert!(!sim.technology_trade_receivable(HUMAN, Tech::INVENTION, false));
        settings.tech_trade_allow_holes = true;
        assert!(sim.transfer_treaty_technology_with_settings(1, HUMAN, Tech::INVENTION, settings));
        assert!(sim.has_tech(HUMAN, Tech::INVENTION));
        assert!(!sim.tech_available(HUMAN, Tech::INVENTION));
    }

    #[test]
    fn receiver_transfer_loss_roll_prevents_discovery() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        sim.players[1].known = vec![Tech::ALPHABET];
        let mut settings = *campaign_research_settings();
        settings.tech_lost_recv_percent = 100;

        assert!(!sim.transfer_treaty_technology_with_settings(1, HUMAN, Tech::ALPHABET, settings));
        assert!(!sim.has_tech(HUMAN, Tech::ALPHABET));
    }

    #[test]
    fn donor_transfer_loss_is_skipped_when_it_would_open_a_technology_hole() {
        let mut actual = Simulation::new(crate::mapgen::generate(42));
        let mut expected = actual.clone();
        actual.players[HUMAN].known.clear();
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        actual.players[1].known = vec![Tech::ALPHABET, writing];
        expected.players[HUMAN].known.clear();
        let mut settings = *campaign_research_settings();
        settings.tech_lost_donor_percent = 100;

        assert!(actual.transfer_treaty_technology_with_settings(
            1,
            HUMAN,
            Tech::ALPHABET,
            settings
        ));
        expected.rand_below(100);

        assert!(actual.has_tech(1, Tech::ALPHABET));
        assert!(actual.has_tech(HUMAN, Tech::ALPHABET));
        assert_eq!(actual.random_snapshot(), expected.random_snapshot());
    }

    #[test]
    fn donor_transfer_loss_precedes_receiver_roll_and_can_remove_unneeded_tech() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        sim.players[1].known = vec![Tech::ALPHABET];
        let mut settings = *campaign_research_settings();
        settings.tech_lost_donor_percent = 100;

        assert!(sim.transfer_treaty_technology_with_settings(1, HUMAN, Tech::ALPHABET, settings));
        assert!(!sim.has_tech(1, Tech::ALPHABET));
        assert!(sim.has_tech(HUMAN, Tech::ALPHABET));
    }

    #[test]
    fn treaty_donor_loss_preserves_suspended_research_like_freeciv() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        sim.players[1].known = vec![Tech::ALPHABET];
        sim.players[1].researching = Tech::FEUDALISM;
        sim.players[1].researching_saved = Some(Tech::ALPHABET);
        sim.players[1].research_saved = 37;
        let mut settings = *campaign_research_settings();
        settings.tech_lost_donor_percent = 100;

        assert!(sim.transfer_treaty_technology_with_settings(1, HUMAN, Tech::ALPHABET, settings));
        assert!(!sim.has_tech(1, Tech::ALPHABET));
        assert_eq!(sim.players[1].researching_saved, Some(Tech::ALPHABET));
        assert_eq!(sim.players[1].research_saved, 37);
    }
}
