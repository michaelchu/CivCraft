//! Implements advisor behavior within the government subsystem.

use super::*;

impl super::Simulation {
    pub(super) fn government_available(&self, owner: usize, government: Government) -> bool {
        government
            .requirement()
            .is_none_or(|tech| self.has_tech(owner, tech))
            || self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY)
    }

    pub(super) fn government_player_bonus(&self, owner: usize, government: Government) -> i32 {
        // advdata.c:adv_gov_player_bonus_want uses a player-only CERTAIN
        // effect query. The ruleset's Veteran_Build effects require unit
        // flags, and Output_Inc_Tile requires an output/tile context, so
        // neither supplies a player-only bonus. Civ2Civ3 has no Rapture_Grow.
        let fanatics = i32::from(government == Government::Fundamentalism) * 3;
        let partisans = i32::from(
            owner != BARBARIANS
                && matches!(government, Government::Communism | Government::Democracy)
                && self.has_tech(owner, Tech::COMMUNISM)
                && self.has_tech(owner, Tech::GUNPOWDER)
                && (0..self.players.len()).any(|player| self.has_tech(player, Tech::NATIONALISM)),
        ) * 3;
        // Civ2Civ3 actions.ruleset has no Gov target requirement in any
        // enabler: action_immune_government returns false for every action.
        fanatics + partisans
    }

    pub(super) fn government_advisor_add_city_value(&self, city: &City, total: &mut f32) {
        let government = self.players[city.owner].government;
        let forecast = self.city_forecast(city);
        let (_, unhappy, _) = self.city_feelings(city, forecast.luxury);
        let (_, earlier_unhappy, _) = self.city_feelings_before_military(city, forecast.luxury);
        let content =
            |amount: i32| amount.min(unhappy.min(earlier_unhappy) + city.specialists[2]) * 35;
        let supported = self
            .units
            .iter()
            .filter(|unit| unit.owner == city.owner && unit.home_city == Some(city.id))
            .count() as i32;
        let military_content = |amount: i32| unhappy * amount * supported * 2 + (amount + 2).max(1);
        // adv_want is float. Each source += rounds separately, including
        // the city output and each effect in effects.ruleset iteration order.
        *total += self.advisor_city_output_exact(city, None) as f32;
        if government != Government::Republic {
            *total -= military_content(1) as f32;
        }
        if matches!(
            government,
            Government::Communism | Government::Monarchy | Government::Republic
        ) {
            *total -= military_content(2) as f32;
        }
        if !matches!(government, Government::Communism | Government::Monarchy) {
            *total -= military_content(3) as f32;
        }
        if government == Government::Democracy
            && self.has_active_wonder(city.owner, Building::WOMENS_SUFFRAGE)
        {
            *total += military_content(-1) as f32;
        }
        if government == Government::Tribal {
            *total += content(1) as f32;
            let land = self
                .units
                .iter()
                .filter(|unit| {
                    unit.owner == city.owner && !unit.kind.is_aircraft() && !unit.kind.is_naval()
                })
                .count() as i32;
            *total += (2 + land) as f32;
        }
        if matches!(government, Government::Communism | Government::Federation) {
            *total += (3 + self
                .units
                .iter()
                .filter(|unit| unit.owner == city.owner)
                .count()) as f32;
        }
        if government == Government::Communism
            && city.has_building(Building::CATHEDRAL)
            && self.has_tech(city.owner, Tech::THEOLOGY)
        {
            *total -= content(1) as f32;
        }
        if government == Government::Democracy
            && city.has_building(Building::POLICE_STATION)
            && !self.has_active_wonder(city.owner, Building::WOMENS_SUFFRAGE)
        {
            *total += military_content(1) as f32;
        }
        if government == Government::Communism
            && city.has_building(Building::TEMPLE)
            && self.has_tech(city.owner, Tech::MYSTICISM)
        {
            *total -= content(1) as f32;
        }
    }

    #[cfg(test)]
    pub(super) fn government_advisor_city_value(&self, city: &City) -> f64 {
        let mut value = 0.0_f32;
        self.government_advisor_add_city_value(city, &mut value);
        f64::from(value)
    }

    pub(super) fn calculate_government_scores(
        &mut self,
        owner: usize,
        mut scores: [f64; 9],
    ) -> [f64; 9] {
        let original = self.players[owner].government;
        let rates = self.players[owner].rates;
        for government in Government::TARGETS {
            let better = match government {
                Government::Tribal => Some(Government::Republic),
                Government::Despotism => Some(Government::Monarchy),
                _ => None,
            };
            if better.is_some_and(|better| self.government_available(owner, better)) {
                continue;
            }
            self.players[owner].government = government;
            self.players[owner].rates =
                capped_rates(self.players[owner].rates, government.max_rate());
            self.arrange_government_workers(owner);
            let mut value = 0.0_f32;
            for city in self.player_cities_ordered(owner) {
                self.government_advisor_add_city_value(city, &mut value);
            }
            let mut bonus = self.government_player_bonus(owner, government);
            let revolution_unhappiness =
                if matches!(government, Government::Republic | Government::Democracy)
                    || self
                        .cities
                        .iter()
                        .any(|city| city.has_building(Building::UNITED_NATIONS))
                {
                    2
                } else {
                    0
                };
            if revolution_unhappiness > 0 {
                bonus -= 6 / revolution_unhappiness;
            }
            value += value * bonus as f32 / 100.0;
            // advdata/daidata apply at least one amortization turn for every
            // technology requirement, even if already known or bypassed.
            let distance = government.requirement().map_or(0, |tech| {
                self.research_path(owner, tech).len().max(1) as i32
            });
            scores[government as usize] =
                f64::from((f64::from(value) * (23.0_f64 / 24.0).powi(distance)) as f32);
        }
        self.players[owner].government = original;
        self.players[owner].rates = rates;
        self.arrange_government_workers(owner);
        scores
    }

    pub(super) fn refresh_government_advisor(&mut self, owner: usize) {
        if self.ai_government_cache.get(&owner).is_some_and(|cache| {
            self.turn >= cache.evaluated_turn && self.turn < cache.next_evaluation
        }) {
            return;
        }
        let old = self
            .ai_government_cache
            .get(&owner)
            .map_or([0.0; 9], |cache| cache.scores);
        let scores = self.calculate_government_scores(owner, old);
        let cities = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .count() as i32;
        self.ai_government_cache.insert(
            owner,
            GovernmentAdvisorCache {
                evaluated_turn: self.turn,
                next_evaluation: self.turn + cities.clamp(5, 20),
                scores,
            },
        );
    }

    pub(super) fn government_goals(
        &self,
        owner: usize,
        scores: &[f64; 9],
    ) -> (Government, Government, f64) {
        let current = self.players[owner].government;
        let (mut goal, mut available) = (current, current);
        let (mut goal_value, mut available_value) = (0.0, 0.0);
        for government in Government::TARGETS {
            let value = scores[government as usize];
            if value > goal_value {
                goal = government;
                goal_value = value;
            }
            if value > available_value && self.government_available(owner, government) {
                available = government;
                available_value = value;
            }
        }
        (goal, available, goal_value - available_value)
    }

    pub(in crate::sim) fn choose_ai_government(&mut self, owner: usize) {
        self.refresh_government_advisor(owner);
        if self.players[owner]
            .revolution_target
            .is_some_and(|target| target != Government::Anarchy)
            || !self.cities.iter().any(|city| city.owner == owner)
        {
            return;
        }
        let (_, government, _) =
            self.government_goals(owner, &self.ai_government_cache[&owner].scores);
        if government != self.players[owner].government {
            self.change_government(owner, government);
        }
    }

    pub(in crate::sim) fn government_research_want(&mut self, owner: usize) -> Option<(Tech, i32)> {
        if owner == BARBARIANS
            || !self.players[owner].ai
            || !self.cities.iter().any(|city| city.owner == owner)
        {
            return None;
        }
        self.refresh_government_advisor(owner);
        let (goal, _, value) =
            self.government_goals(owner, &self.ai_government_cache[&owner].scores);
        let required = goal.requirement()?;
        if value <= 0.0 || self.has_tech(owner, required) {
            return None;
        }
        Some((
            required,
            (value as i32).max(100)
                + if self.players[owner].government == Government::Tribal {
                    25 * self.turn
                } else {
                    0
                },
        ))
    }

    pub(in crate::sim) fn ai_goal_government(&self, owner: usize) -> Government {
        if let Some(cache) = self
            .ai_government_cache
            .get(&owner)
            .filter(|cache| self.turn >= cache.evaluated_turn && self.turn < cache.next_evaluation)
        {
            return self.government_goals(owner, &cache.scores).0;
        }
        let mut scratch = self.clone();
        scratch.refresh_government_advisor(owner);
        scratch
            .government_goals(owner, &scratch.ai_government_cache[&owner].scores)
            .0
    }

    pub(crate) fn ai_government_cache_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.ai_government_cache.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.road = true;
            tile.river = true;
        }
        self.create_city(1, 5, 8);
        self.cities[0].size = 4;
        self.reconcile_city_workers();
        self.players[1].government = Government::Tribal;
        self.players[1].known = self.research_path(1, Tech::MONARCHY);
        self.players[1].known.retain(|&tech| tech != Tech::MONARCHY);
        self.players[1].rates = [60, 40, 0];
        self.turn = 0;
        self.refresh_government_advisor(1);
        let before = self.ai_government_cache[&1].clone();
        let wanted = self
            .government_research_want(1)
            .is_some_and(|(goal, _)| goal == Tech::MONARCHY);
        self.players[1].known.push(Tech::MONARCHY);
        let available = self
            .government_goals(1, &self.ai_government_cache[&1].scores)
            .1
            == Government::Monarchy
            && self.government_research_want(1).is_none()
            && self.ai_government_cache[&1].evaluated_turn == 0;
        self.cities[0].size = 8;
        self.reconcile_city_workers();
        self.turn = before.next_evaluation - 1;
        self.refresh_government_advisor(1);
        let held = self.ai_government_cache[&1].scores == before.scores;
        self.turn += 1;
        self.refresh_government_advisor(1);
        let recomputed = self.ai_government_cache[&1].evaluated_turn == self.turn
            && self.ai_government_cache[&1].scores != before.scores;
        wanted
            && available
            && held
            && recomputed
            && before.next_evaluation == 5
            && self.players[1].government == Government::Tribal
            && self.players[1].rates == [60, 40, 0]
    }
}
