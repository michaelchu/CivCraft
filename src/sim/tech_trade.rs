use super::*;

impl Simulation {
    fn technology_gold_equivalent(&self, buyer: usize, tech: Tech, include_advisor: bool) -> i32 {
        // Civ2Civ3 disables trades that would leave holes in the tech tree.
        if tech == Tech::FUTURE || !self.tech_available(buyer, tech) {
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
        if !self.has_tech(seller, tech) || tech == Tech::FUTURE || !self.tech_available(buyer, tech)
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

    pub(super) fn ai_treaty_clause_balance(
        &self,
        owner: usize,
        other: usize,
        give: diplomacy::Concessions,
        receive: diplomacy::Concessions,
        after: Pact,
    ) -> Option<(i64, bool)> {
        if (after == Pact::War
            && (give.city().is_some()
                || receive.city().is_some()
                || give.embassy()
                || receive.embassy()))
            || (give.city().is_some()
                && self
                    .cities
                    .iter()
                    .filter(|city| city.owner == owner)
                    .count()
                    <= 3)
        {
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
        if let Some(map) = give.map() {
            clauses.push(-i64::from(self.map_trade_price(owner, other, map, after)));
        }
        if let Some(id) = give.city() {
            let city = self
                .cities
                .iter()
                .find(|city| city.id == id && city.owner == owner)?;
            clauses.push(-i64::from(self.city_sale_value(city, other)));
        }
        if let Some(id) = receive.city() {
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
                        .ai_treaty_clause_balance(owner, other, terms[side], terms[1 - side], after)
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
}
