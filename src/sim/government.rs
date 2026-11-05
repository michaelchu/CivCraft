use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Government {
    Anarchy,
    Tribal,
    Despotism,
    Monarchy,
    Communism,
    Fundamentalism,
    Federation,
    Republic,
    Democracy,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GovernmentAdvisorCache {
    pub(super) evaluated_turn: i32,
    pub(super) next_evaluation: i32,
    pub(super) scores: [f64; 9],
}

impl Government {
    pub(crate) const TARGETS: [Self; 8] = [
        Self::Tribal,
        Self::Despotism,
        Self::Monarchy,
        Self::Communism,
        Self::Fundamentalism,
        Self::Federation,
        Self::Republic,
        Self::Democracy,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Anarchy => "Anarchy",
            Self::Tribal => "Tribal",
            Self::Despotism => "Despotism",
            Self::Monarchy => "Monarchy",
            Self::Communism => "Communism",
            Self::Fundamentalism => "Fundamentalism",
            Self::Federation => "Federation",
            Self::Republic => "Republic",
            Self::Democracy => "Democracy",
        }
    }

    pub(crate) fn requirement(self) -> Option<Tech> {
        match self {
            Self::Anarchy | Self::Tribal | Self::Despotism => None,
            Self::Monarchy => Some(Tech::MONARCHY),
            Self::Communism => Some(Tech::COMMUNISM),
            Self::Fundamentalism => Some(Tech::THEOLOGY),
            Self::Federation => Some(Tech::ECONOMICS),
            Self::Republic => Some(Tech::THE_REPUBLIC),
            Self::Democracy => Some(Tech::DEMOCRACY),
        }
    }

    pub(crate) fn max_rate(self) -> i32 {
        match self {
            Self::Anarchy => 100,
            Self::Tribal | Self::Despotism => 60,
            Self::Monarchy | Self::Fundamentalism => 70,
            Self::Communism | Self::Republic => 80,
            Self::Federation | Self::Democracy => 90,
        }
    }

    // Shield cost per eligible unit converted to gold, then city allowances.
    pub(crate) fn unit_support(self) -> (i32, i32, i32) {
        match self {
            Self::Anarchy | Self::Tribal => (0, 2, 0),
            Self::Despotism => (1, 0, 2),
            Self::Monarchy => (1, 0, 3),
            Self::Communism => (0, 3, 0),
            Self::Fundamentalism | Self::Federation => (2, 0, 4),
            Self::Republic => (0, 1, 0),
            Self::Democracy => (2, 0, 2),
        }
    }

    pub(super) fn base_waste(self) -> (i32, i32) {
        match self {
            Self::Anarchy => (30, 0),
            Self::Tribal => (0, 30),
            Self::Despotism => (10, 20),
            Self::Monarchy => (20, 10),
            Self::Communism => (0, 30),
            Self::Fundamentalism => (15, 15),
            Self::Federation => (30, 0),
            Self::Republic => (5, 25),
            Self::Democracy => (25, 5),
        }
    }

    pub(super) fn empire_size(self) -> i32 {
        match self {
            Self::Anarchy => 0,
            Self::Tribal => 12,
            Self::Despotism => 10,
            Self::Monarchy => 14,
            Self::Communism => 28,
            Self::Fundamentalism => 20,
            Self::Federation => 24,
            Self::Republic => 16,
            Self::Democracy => 32,
        }
    }
}

impl Simulation {
    pub(super) fn government_output_waste(
        &self,
        city: &City,
        shields: i32,
        trade: i32,
        granted: Option<Tech>,
    ) -> (i32, i32) {
        self.government_output_waste_under(
            city,
            shields,
            trade,
            granted,
            self.players[city.owner].government,
        )
    }

    pub(super) fn government_output_waste_under(
        &self,
        city: &City,
        shields: i32,
        trade: i32,
        granted: Option<Tech>,
        government: Government,
    ) -> (i32, i32) {
        let player = &self.players[city.owner];
        let (shield_base, trade_base) = government.base_waste();
        let distance = if city.has_building(Building::PALACE)
            || city.has_building(Building::ECCLESIASTICAL_PALACE)
        {
            Some(0)
        } else {
            self.cities
                .iter()
                .filter(|center| {
                    center.owner == city.owner
                        && (center.has_building(Building::PALACE)
                            || center.has_building(Building::ECCLESIASTICAL_PALACE))
                })
                .map(|center| self.map_distance(city.x, city.y, center.x, center.y))
                .min()
        };
        let has_tech = |tech| granted == Some(tech) || player.known.contains(&tech);
        let shield_distance =
            if matches!(government, Government::Communism | Government::Federation) {
                0
            } else {
                1 + i32::from(!has_tech(Tech::TRADE))
            };
        let trade_distance = match government {
            Government::Communism => 0,
            Government::Federation => i32::from(!has_tech(Tech::THE_CORPORATION)),
            _ => 1 + i32::from(!has_tech(Tech::THE_CORPORATION)),
        };
        let relief = 50
            * (i32::from(city.has_building(Building::COURTHOUSE))
                + i32::from(
                    city.has_building(Building::PALACE)
                        || city.has_building(Building::ECCLESIASTICAL_PALACE),
                ));
        let net = |gross, base, per_tile| {
            if distance.is_none() && per_tile > 0 {
                return 0;
            }
            let waste = percent_of(gross, base + distance.unwrap_or(0) * per_tile);
            gross - (waste - percent_of(waste, relief)).clamp(0, gross)
        };
        // effect_barb2 subtracts200 from distance-based waste, not from
        // base corruption. city_waste ignores nonpositive distance effects.
        let barbarian_relief = 2 * i32::from(city.owner == BARBARIANS);
        (
            net(
                shields,
                shield_base,
                (shield_distance - barbarian_relief).max(0),
            ),
            net(
                trade,
                trade_base,
                (trade_distance - barbarian_relief).max(0),
            ),
        )
    }

    pub(crate) fn barbarian_output_waste_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        self.players[BARBARIANS].government = Government::Despotism;
        self.players[BARBARIANS].known.clear();
        self.create_city(BARBARIANS, 12, 24);
        let mut city = self.cities[0].clone();
        let no_center = !city.has_building(Building::PALACE);
        let base = self.government_output_waste(&city, 100, 100, None) == (90, 80);
        city.buildings.push(Building::COURTHOUSE);
        let courthouse = self.government_output_waste(&city, 100, 100, None) == (95, 90);
        city.buildings.clear();
        city.owner = 1;
        let ordinary =
            self.government_output_waste_under(&city, 100, 100, None, Government::Despotism)
                == (0, 0);
        no_center && base && courthouse && ordinary
    }

    pub(super) fn has_active_wonder(&self, owner: usize, building: Building) -> bool {
        self.cities.iter().any(|city| {
            city.owner == owner
                && city.has_building(building)
                && !self.building_is_obsolete(city, building, None)
        })
    }

    pub(super) fn handle_government_tech_loss(&mut self, owner: usize, lost: Tech) {
        let player = &self.players[owner];
        let government = player.revolution_target.unwrap_or(player.government);
        if government.requirement() != Some(lost)
            || self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY)
        {
            return;
        }
        let available: Vec<_> = Government::TARGETS
            .into_iter()
            .filter(|candidate| self.can_change_government(owner, *candidate))
            .collect();
        if !available.is_empty() {
            let target = available[self.rand_below(available.len() as i32) as usize];
            self.change_government(owner, target);
        }
    }

    fn government_available(&self, owner: usize, government: Government) -> bool {
        government
            .requirement()
            .is_none_or(|tech| self.has_tech(owner, tech))
            || self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY)
    }

    fn government_player_bonus(&self, owner: usize, government: Government) -> i32 {
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
                && (0..self.players.len())
                    .any(|player| self.has_tech(player, Tech::GUERILLA_WARFARE)),
        ) * 3;
        // Civ2Civ3 actions.ruleset has no Gov target requirement in any
        // enabler: action_immune_government returns false for every action.
        fanatics + partisans
    }

    fn government_advisor_add_city_value(&self, city: &City, total: &mut f32) {
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
    fn government_advisor_city_value(&self, city: &City) -> f64 {
        let mut value = 0.0_f32;
        self.government_advisor_add_city_value(city, &mut value);
        f64::from(value)
    }

    fn calculate_government_scores(&mut self, owner: usize, mut scores: [f64; 9]) -> [f64; 9] {
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

    fn refresh_government_advisor(&mut self, owner: usize) {
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

    fn government_goals(&self, owner: usize, scores: &[f64; 9]) -> (Government, Government, f64) {
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

    pub(super) fn choose_ai_government(&mut self, owner: usize) {
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

    pub(super) fn government_research_want(&mut self, owner: usize) -> Option<(Tech, i32)> {
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

    pub(super) fn ai_goal_government(&self, owner: usize) -> Government {
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

    pub(super) fn government_city_value(&self, city: &City) -> i32 {
        let forecast = self.city_forecast(city);
        let food = forecast.food_surplus(city.size, forecast.support.food);
        let shields = if forecast.disorder {
            0
        } else {
            forecast.shields - forecast.support.shields
        };
        let gold = forecast.gold - forecast.building_upkeep - forecast.support.gold;
        30 * food + 17 * shields + 18 * gold + 21 * forecast.science
    }

    pub(super) fn project_government_output(&self, owner: usize) -> i32 {
        let mut projected = self.clone();
        projected.arrange_government_workers(owner);
        projected
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .map(|city| projected.government_city_value(city))
            .sum()
    }

    pub(crate) fn can_change_government(&self, owner: usize, target: Government) -> bool {
        let player = &self.players[owner];
        target != Government::Anarchy
            && target != player.government
            && player.revolution_target != Some(target)
            && (player.revolution_turn != Some(self.turn)
                || self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY))
            && (target
                .requirement()
                .is_none_or(|tech| player.known.contains(&tech))
                || self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY))
    }

    pub(super) fn begin_ai_war_revolution(&mut self, owner: usize) -> bool {
        let player = &self.players[owner];
        if !player.ai
            || player.government == Government::Anarchy
            || player.revolution_turn == Some(self.turn)
                && !self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY)
        {
            return false;
        }
        // dai_go_to_war requests Anarchy itself, postponing selection of
        // the eventual government until the next government advisor pass.
        let finish = if self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY) {
            self.turn
        } else if let Some(finish) = player.revolution_turn.filter(|&finish| finish >= self.turn) {
            finish
        } else {
            let maximum = (5 - self.government_adoptions[Government::Anarchy as usize]).max(1);
            self.turn + 1 + self.rand_below(maximum)
        };
        let player = &mut self.players[owner];
        player.government = Government::Anarchy;
        player.revolution_target = Some(Government::Anarchy);
        player.revolution_turn = Some(finish);
        // The caller assigns its pending war target AFTER this callback,
        // matching daidiplomacy.c's native ordering.
        self.resume_ai_war_after_revolution(owner);
        true
    }

    pub(super) fn change_government(&mut self, owner: usize, target: Government) -> bool {
        if !self.can_change_government(owner, target) {
            return false;
        }
        let player = &self.players[owner];
        let finish = if self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY) {
            self.turn
        } else if player.government == Government::Anarchy {
            player.revolution_turn.unwrap()
        } else {
            // RANDQUICK with the reference default length (5) and six nations.
            let max_turns = (5 - self.government_adoptions[target as usize]).max(1);
            self.turn + 1 + self.rand_below(max_turns)
        };
        let player = &mut self.players[owner];
        player.government = Government::Anarchy;
        player.revolution_target = Some(target);
        player.revolution_turn = Some(finish);
        if owner == HUMAN && finish > self.turn {
            self.add_event(format!(
                "Revolution toward {}. Anarchy for {} turns.",
                target.name(),
                finish - self.turn
            ));
        }
        self.resume_ai_war_after_revolution(owner);
        self.finish_revolutions();
        true
    }

    pub(super) fn finish_revolutions(&mut self) {
        for owner in 0..self.players.len() {
            if owner == BARBARIANS {
                continue;
            }
            let player = &mut self.players[owner];
            if player.revolution_turn.is_some_and(|turn| turn <= self.turn)
                && player
                    .revolution_target
                    .is_some_and(|target| target != Government::Anarchy)
                && let Some(target) = player.revolution_target.take()
            {
                player.government = target;
                self.government_adoptions[target as usize] += 1;
                player.rates = capped_rates(player.rates, target.max_rate());
                if owner == HUMAN {
                    self.add_event(format!("{} is now your government.", target.name()));
                }
            }
        }
    }
}

fn capped_rates(mut rates: [i32; 3], cap: i32) -> [i32; 3] {
    let mut surplus = 0;
    for rate in &mut rates {
        surplus += (*rate - cap).max(0);
        *rate = (*rate).min(cap);
    }
    for rate in &mut rates {
        let added = surplus.min(cap - *rate);
        *rate += added;
        surplus -= added;
    }
    rates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civ2civ3_default_government_and_initial_support_are_despotism() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        // nations.ruleset31; nation.c:init_government_of_nation; includes
        // nonplayable Barbarian/Pirate nations without an explicit override.
        assert!(
            game.players
                .iter()
                .all(|player| player.government == Government::Despotism)
        );
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        for _ in 0..3 {
            let id = game.spawn_unit(1, UnitKind::Warrior, 12, 24);
            let home = game.cities[0].id;
            let unit = game.units.iter_mut().find(|unit| unit.id == id).unwrap();
            unit.home_city = Some(home);
            unit.home_order = id;
        }
        let support = game.city_unit_upkeep(&game.cities[0]);
        assert_eq!(support.shields, 0);
        assert_eq!(support.gold, 1); // Despotism converts shields to gold; two free.
    }

    #[test]
    fn barbarians_remove_distance_waste_but_retain_government_base_and_building_relief() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.barbarian_output_waste_playtest());
        let mut city = game.cities[0].clone();
        assert_eq!(
            game.government_output_waste_under(&city, 100, 100, None, Government::Tribal),
            (100, 70)
        );
        game.players[BARBARIANS]
            .known
            .extend([Tech::TRADE, Tech::THE_CORPORATION]);
        assert_eq!(
            game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
            (90, 80)
        );
        city.buildings.push(Building::PALACE);
        assert_eq!(
            game.government_output_waste_under(&city, 100, 100, None, Government::Despotism),
            (95, 90)
        );
    }

    #[test]
    fn fundamentalism_tithes_feed_actual_forecasts_and_candidate_valuation() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.players[1].government = Government::Fundamentalism;
        game.players[1].known = vec![Tech::MYSTICISM];
        game.players[1].rates = [0, 100, 0];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
        }
        game.create_city(1, 12, 24);
        let baseline = game.cities[0].clone();
        let mut temple = baseline.clone();
        temple.buildings.push(Building::TEMPLE);
        assert_eq!(
            game.city_forecast(&temple).gross_gold - game.city_forecast(&baseline).gross_gold,
            2
        );
        // Even a content size1 city receives both Temple/Mysticism tithes.
        assert_eq!(game.city_feelings(&temple, 0).1, 0);
        assert!(
            game.government_advisor_city_value(&temple)
                > game.government_advisor_city_value(&baseline)
        );
    }

    #[test]
    fn cached_government_scores_follow_reference_cadence_but_unlocks_apply_immediately() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_government_cache_playtest());
    }

    #[test]
    fn government_cache_interval_uses_city_count_with_five_and_twenty_turn_bounds() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.turn = 0;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
        }
        for index in 0..21 {
            game.create_city(1, 2 + (index % 6) * 5, 4 + (index / 6) * 12);
        }
        game.refresh_government_advisor(1);
        assert_eq!(game.ai_government_cache[&1].next_evaluation, 20);
        game.cities.truncate(6);
        game.turn = 19;
        game.refresh_government_advisor(1);
        assert_eq!(game.ai_government_cache[&1].next_evaluation, 20);
        game.turn = 20;
        game.refresh_government_advisor(1);
        assert_eq!(game.ai_government_cache[&1].next_evaluation, 26);
        game.cities.truncate(5);
        game.turn = 26;
        game.refresh_government_advisor(1);
        assert_eq!(game.ai_government_cache[&1].next_evaluation, 31);
    }

    #[test]
    fn government_player_bonus_uses_player_context_and_gates_partisan_techs() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known.clear();
        assert_eq!(game.government_player_bonus(1, Government::Communism), 0);
        assert_eq!(game.government_player_bonus(1, Government::Federation), 0);
        assert_eq!(game.government_player_bonus(1, Government::Democracy), 0);
        assert_eq!(
            game.government_player_bonus(1, Government::Fundamentalism),
            3
        );
        game.players[1]
            .known
            .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
        game.players[HUMAN].known.push(Tech::GUERILLA_WARFARE);
        assert_eq!(game.government_player_bonus(1, Government::Communism), 3);
        assert_eq!(game.government_player_bonus(1, Government::Democracy), 3);
        assert_eq!(
            game.government_player_bonus(BARBARIANS, Government::Communism),
            0
        );
    }

    #[test]
    fn government_source_effects_include_negative_requirements_and_veteran_class_counts() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.create_city(1, 10, 24);
        game.players[1].government = Government::Communism;
        let city = game.cities[0].clone();
        let base = game.advisor_city_output_exact(&city, None);
        // Source: Veteran_Build(3) minus excluded free-military effects(3+4).
        assert_eq!(game.government_advisor_city_value(&city) - base, -4.0);
        game.spawn_unit(1, UnitKind::Warrior, 10, 24);
        assert_eq!(
            game.government_advisor_city_value(&city) - game.advisor_city_output_exact(&city, None),
            -3.0
        );
    }

    #[test]
    fn anarchy_waives_normal_technology_upkeep() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.players[HUMAN].known = Tech::all().collect();
        let upkeep = game.technology_upkeep(HUMAN);
        assert!(upkeep > 0);
        let known = game.players[HUMAN].known.len();
        game.apply_research_upkeep(HUMAN, 0);
        assert_eq!(game.players[HUMAN].known.len(), known - 1);
        game.players[HUMAN].government = Government::Anarchy;
        assert_eq!(game.technology_upkeep(HUMAN), 0);
    }

    #[test]
    fn government_upkeep_applies_output_effects_and_gold_allowances() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        let city_id = game.cities[0].id;
        for kind in [UnitKind::Warrior; 5]
            .into_iter()
            .chain([UnitKind::Caravel])
        {
            let id = game.spawn_unit(HUMAN, kind, 10, 10);
            let unit = game.units.last_mut().unwrap();
            unit.home_city = Some(city_id);
            unit.home_order = id;
        }
        // Caravel lacks ShieldToGold, but does not bypass the government's
        // shield Upkeep_Pct effect (zero in the gold-support governments).
        for (government, shields, gold) in [
            (Government::Anarchy, 4, 0),
            (Government::Tribal, 4, 0),
            (Government::Despotism, 0, 3),
            (Government::Monarchy, 0, 2),
            (Government::Communism, 3, 0),
            (Government::Fundamentalism, 0, 6),
            (Government::Federation, 0, 6),
            (Government::Republic, 5, 0),
            (Government::Democracy, 0, 8),
        ] {
            game.players[HUMAN].government = government;
            let upkeep = game.city_unit_upkeep(&game.cities[0]);
            assert_eq!(
                (upkeep.food, upkeep.shields, upkeep.gold),
                (2, shields, gold),
                "{government:?}"
            );
        }
        let paid = game.city_gold_upkeep(&game.cities[0]);
        let first = paid[0].0;
        game.players[HUMAN].gold = -8;
        game.balance_unit_gold_upkeep(HUMAN, &paid);
        assert_eq!(game.players[HUMAN].gold, 0);
        assert!(!game.units.iter().any(|unit| unit.id == first));
        assert!(game.units.iter().any(|unit| unit.kind == UnitKind::Caravel));
    }

    #[test]
    fn revolution_respects_unlocks_anarchy_and_rate_caps() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[HUMAN].government = Government::Tribal;
        assert!(!game.apply(Command::Government(Government::Monarchy)));
        assert!(game.apply(Command::Government(Government::Despotism)));
        let finish = game.players[HUMAN].revolution_turn.unwrap();
        assert!((1..=5).contains(&(finish - game.turn)));
        assert_eq!(game.players[HUMAN].government, Government::Anarchy);
        assert!(!game.apply(Command::RaiseRate(1)));
        game.turn = finish;
        game.finish_revolutions();
        assert_eq!(game.players[HUMAN].government, Government::Despotism);
        assert!(!game.apply(Command::Government(Government::Tribal)));
    }
}
