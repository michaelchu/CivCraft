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
        let player = &self.players[city.owner];
        let government = player.government;
        let (shield_base, trade_base) = government.base_waste();
        let distance = self
            .cities
            .iter()
            .filter(|center| {
                center.owner == city.owner
                    && (center.has_building(Building::PALACE)
                        || center.has_building(Building::ECCLESIASTICAL_PALACE))
            })
            .map(|center| self.map_distance(city.x, city.y, center.x, center.y))
            .min();
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
        if city.owner == BARBARIANS {
            return (shields, trade);
        }
        (
            net(shields, shield_base, shield_distance),
            net(trade, trade_base, trade_distance),
        )
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

    pub(super) fn choose_ai_government(&mut self, owner: usize) {
        if self.players[owner].revolution_target.is_some()
            || !self.cities.iter().any(|city| city.owner == owner)
        {
            return;
        }
        let original = self.players[owner].government;
        let best = self
            .government_projections(owner, false)
            .into_iter()
            .max_by(|(a, _, left), (b, _, right)| {
                left.total_cmp(right)
                    .then_with(|| (*a == original).cmp(&(*b == original)))
            });
        if let Some((government, _, _)) = best
            && government != original
        {
            self.change_government(owner, government);
        }
    }

    pub(super) fn government_research_want(&mut self, owner: usize) -> Option<(Tech, i32)> {
        if owner == BARBARIANS
            || !self.players[owner].ai
            || self.players[owner].revolution_target.is_some()
            || !self.cities.iter().any(|city| city.owner == owner)
        {
            return None;
        }
        let projections = self.government_projections(owner, true);
        let available = projections
            .iter()
            .filter(|(_, required, _)| required.is_none())
            .map(|&(_, _, score)| score)
            .reduce(f64::max)?;
        let (_, required, score) = projections
            .into_iter()
            .filter(|(_, required, score)| required.is_some() && *score > available)
            .max_by(|(_, _, a), (_, _, b)| a.total_cmp(b))?;
        let initial_bonus = if self.players[owner].government == Government::Tribal {
            25 * self.turn
        } else {
            0
        };
        Some((
            required?,
            ((score - available) as i32).max(100) + initial_bonus,
        ))
    }

    fn government_projections(
        &mut self,
        owner: usize,
        include_locked: bool,
    ) -> Vec<(Government, Option<Tech>, f64)> {
        let original = self.players[owner].government;
        let original_rates = self.players[owner].rates;
        let immediate = self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY);
        let candidates: Vec<_> = Government::TARGETS
            .into_iter()
            .filter_map(|government| {
                let superseded = match government {
                    Government::Tribal => {
                        self.can_change_government(owner, Government::Republic)
                            || original == Government::Republic
                    }
                    Government::Despotism => {
                        self.can_change_government(owner, Government::Monarchy)
                            || original == Government::Monarchy
                    }
                    _ => false,
                };
                if superseded {
                    return None;
                }
                if government == original || self.can_change_government(owner, government) {
                    return Some((government, None, 0));
                }
                let required = government.requirement()?;
                if !include_locked || self.has_tech(owner, required) {
                    return None;
                }
                Some((
                    government,
                    Some(required),
                    self.research_path(owner, required).len() as i32,
                ))
            })
            .collect();
        let mut projections = Vec::new();
        for (government, required, unknown) in candidates {
            self.players[owner].government = government;
            self.players[owner].rates = capped_rates(original_rates, government.max_rate());
            let output = f64::from(self.project_government_output(owner));
            let delay = if immediate || government == original {
                0.0
            } else {
                f64::from(1 + (5 - self.government_adoptions[government as usize]).max(1)) / 2.0
            } + f64::from(unknown);
            let score = output - output.abs() * (1.0 - (23.0_f64 / 24.0).powf(delay));
            projections.push((government, required, score));
        }
        self.players[owner].government = original;
        self.players[owner].rates = original_rates;
        projections
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
        use std::collections::HashSet;

        let mut occupied: HashSet<_> = self
            .cities
            .iter()
            .flat_map(|city| city.worked_tiles.iter().copied())
            .collect();
        let mut output = 0;
        for city in self.cities.iter().filter(|city| city.owner == owner) {
            let mut projected = city.clone();
            let mut value = self.government_city_value(&projected);
            let candidates: Vec<_> = Self::city_radius(city)
                .map(|(_, _, x, y)| (x, y))
                .filter(|&(x, y)| self.tile_can_support_worker(city, x, y))
                .collect();
            for worker in 0..projected.worked_tiles.len() {
                let original = projected.worked_tiles[worker];
                occupied.remove(&original);
                let mut best = original;
                for &tile in &candidates {
                    if tile == original || occupied.contains(&tile) {
                        continue;
                    }
                    projected.worked_tiles[worker] = tile;
                    let score = self.government_city_value(&projected);
                    if score > value {
                        best = tile;
                        value = score;
                    }
                }
                projected.worked_tiles[worker] = best;
                occupied.insert(best);
            }
            output += value;
        }
        output
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
    fn government_upkeep_keeps_caravels_on_shields_and_applies_gold_allowances() {
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
        for (government, shields, gold) in [
            (Government::Anarchy, 4, 0),
            (Government::Tribal, 4, 0),
            (Government::Despotism, 1, 3),
            (Government::Monarchy, 1, 2),
            (Government::Communism, 3, 0),
            (Government::Fundamentalism, 1, 6),
            (Government::Federation, 1, 6),
            (Government::Republic, 5, 0),
            (Government::Democracy, 1, 8),
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
