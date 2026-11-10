//! Government selection and government-dependent rules.

use super::*;

mod advisor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Government {
    Anarchy,
    Despotism,
    Monarchy,
    Communism,
    Fascism,
    Feudalism,
    Republic,
    Democracy,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GovernmentAdvisorCache {
    pub(super) evaluated_turn: i32,
    pub(super) next_evaluation: i32,
    pub(super) scores: [f64; 8],
}

impl Government {
    pub(crate) const TARGETS: [Self; 7] = [
        Self::Despotism,
        Self::Monarchy,
        Self::Communism,
        Self::Fascism,
        Self::Feudalism,
        Self::Republic,
        Self::Democracy,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Anarchy => "Anarchy",
            Self::Despotism => "Despotism",
            Self::Monarchy => "Monarchy",
            Self::Communism => "Communism",
            Self::Fascism => "Fascism",
            Self::Feudalism => "Feudalism",
            Self::Republic => "Republic",
            Self::Democracy => "Democracy",
        }
    }

    pub(crate) fn requirement(self) -> Option<Tech> {
        match self {
            Self::Anarchy | Self::Despotism => None,
            Self::Monarchy => Some(Tech::MONARCHY),
            Self::Communism => Some(Tech::COMMUNISM),
            Self::Fascism => Some(Tech::FASCISM),
            Self::Feudalism => Some(Tech::FEUDALISM),
            Self::Republic => Some(Tech::THE_REPUBLIC),
            Self::Democracy => Some(Tech::DEMOCRACY),
        }
    }

    pub(crate) fn max_rate(self) -> i32 {
        match self {
            Self::Anarchy => 100,
            Self::Despotism | Self::Feudalism => 60,
            Self::Monarchy | Self::Fascism => 70,
            Self::Communism | Self::Republic => 80,
            Self::Democracy => 90,
        }
    }

    /// Conquests free unit slots for town/city/metropolis and cost per excess unit.
    /// The rules are kept in the ruleset TSV so a rebalance does not touch logic.
    pub(crate) fn unit_support(self) -> Option<UnitSupportRule> {
        let key = match self {
            Self::Anarchy => "anarchy",
            Self::Despotism => "despotism",
            Self::Monarchy => "monarchy",
            Self::Communism => "communism",
            Self::Fascism => "fascism",
            Self::Feudalism => "feudalism",
            Self::Republic => "republic",
            Self::Democracy => "democracy",
        };
        unit_support_rules().iter().find(|rule| rule.government == key).copied()
    }

    pub(super) fn base_waste(self) -> (i32, i32) {
        match self {
            Self::Anarchy => (30, 0),
            Self::Despotism => (10, 20),
            Self::Monarchy => (20, 10),
            Self::Communism => (0, 30),
            Self::Fascism => (15, 15),
            Self::Feudalism => (20, 10),
            Self::Republic => (5, 25),
            Self::Democracy => (25, 5),
        }
    }

    pub(super) fn empire_size(self) -> i32 {
        match self {
            Self::Anarchy => 0,
            Self::Despotism => 10,
            Self::Monarchy => 14,
            Self::Communism => 28,
            Self::Fascism => 20,
            Self::Feudalism => 14,
            Self::Republic => 16,
            Self::Democracy => 32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UnitSupportRule {
    government: &'static str,
    pub(crate) town: i32,
    pub(crate) city: i32,
    pub(crate) metropolis: i32,
    pub(crate) cost_per_unit: i32,
}

fn unit_support_rules() -> &'static [UnitSupportRule] {
    use std::sync::OnceLock;
    static RULES: OnceLock<Vec<UnitSupportRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        include_str!("../../data/rulesets/civ3_conquests/government_unit_support.tsv")
            .lines()
            .skip(1)
            .map(|line| {
                let mut fields = line.split('\t');
                let government = fields.next().expect("support row government");
                let number = |value: Option<&str>| value.expect("support value").parse().expect("numeric support value");
                let town = number(fields.next());
                let city = number(fields.next());
                let metropolis = number(fields.next());
                let cost_per_unit = number(fields.next());
                assert!(fields.next().is_none(), "unexpected support table columns");
                UnitSupportRule { government, town, city, metropolis, cost_per_unit }
            })
            .collect()
    })
}

impl Simulation {
    pub(super) fn government_center_distance(&self, city: &City) -> Option<i32> {
        if city.has_building(Building::PALACE)
            || city.has_building(Building::ECCLESIASTICAL_PALACE)
            || city.has_building(Building::FORBIDDEN_PALACE)
        {
            Some(0)
        } else {
            self.cities
                .iter()
                .filter(|center| {
                    center.owner == city.owner
                        && (center.has_building(Building::PALACE)
                            || center.has_building(Building::ECCLESIASTICAL_PALACE)
                            || center.has_building(Building::FORBIDDEN_PALACE))
                })
                .map(|center| self.map_distance(city.x, city.y, center.x, center.y))
                .min()
        }
    }

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
        let (shield_base, trade_base) = government.base_waste();
        let distance = self.government_center_distance(city);
        let has_tech = |tech| granted == Some(tech) || self.has_tech(city.owner, tech);
        let shield_distance =
            if matches!(government, Government::Communism | Government::Feudalism) {
                0
            } else {
                1 + i32::from(!has_tech(Tech::ECONOMICS))
            };
        let trade_distance = match government {
            Government::Communism => 0,
            Government::Feudalism => i32::from(!has_tech(Tech::THE_CORPORATION)),
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
#[path = "government/tests.rs"]
mod tests;
