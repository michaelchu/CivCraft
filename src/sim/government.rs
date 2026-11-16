//! Government selection and government-dependent rules.

use super::*;
use crate::rules::freeciv::RulesetData;
use crate::sim::action_rules::SimulationRequirementContext;
use std::sync::OnceLock;

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
    Tribal,
    Fundamentalism,
    Federation,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GovernmentAdvisorCache {
    pub(super) evaluated_turn: i32,
    pub(super) next_evaluation: i32,
    pub(super) scores: [f64; 11],
}

impl Government {
    pub(crate) fn during_revolution() -> Self {
        Self::Anarchy
    }

    pub(crate) fn targets() -> &'static [Self] {
        static TARGETS: OnceLock<Vec<Government>> = OnceLock::new();
        TARGETS.get_or_init(|| {
            crate::rules::native::governments()
                .governments
                .iter()
                .filter(|government| !government.government.eq_ignore_ascii_case("anarchy"))
                .map(|government| {
                    Self::from_rule_name(&government.government).unwrap_or_else(|| {
                        panic!(
                            "selected government {:?} has no stable runtime identity",
                            government.government
                        )
                    })
                })
                .collect()
        })
    }

    fn from_rule_name(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "anarchy" => Self::Anarchy,
            "tribal" => Self::Tribal,
            "despotism" => Self::Despotism,
            "monarchy" => Self::Monarchy,
            "communism" => Self::Communism,
            "fundamentalism" => Self::Fundamentalism,
            "federation" => Self::Federation,
            "republic" => Self::Republic,
            "democracy" => Self::Democracy,
            "fascism" => Self::Fascism,
            "feudalism" => Self::Feudalism,
            _ => return None,
        })
    }

    pub(crate) fn rule_name(self) -> Option<&'static str> {
        Some(match self {
            Self::Anarchy => "Anarchy",
            Self::Tribal => "Tribal",
            Self::Despotism => "Despotism",
            Self::Monarchy => "Monarchy",
            Self::Communism => "Communism",
            Self::Fundamentalism => "Fundamentalism",
            Self::Federation => "Federation",
            Self::Republic => "Republic",
            Self::Democracy => "Democracy",
            Self::Fascism => "Fascism",
            Self::Feudalism => "Feudalism",
        })
    }

    pub(crate) fn name(self) -> &'static str {
        self.rule_name()
            .and_then(|name| crate::rules::native::governments().get(name))
            .map(|government| government.name.as_str())
            .unwrap_or_else(|| self.rule_name().unwrap_or("Unknown Government"))
    }

    pub(crate) fn requirement(self) -> Option<Tech> {
        let name = self.rule_name()?;
        let technology = crate::rules::native::governments()
            .get(name)
            .map(|government| government.required_technology.as_str())
            .filter(|name| !name.is_empty())?;
        Tech::all().find(|tech| tech.name().eq_ignore_ascii_case(technology))
    }

    fn native_record(self) -> Option<&'static crate::rules::native::NativeGovernment> {
        self.rule_name()
            .and_then(|name| crate::rules::native::governments().get(name))
    }

    pub(crate) fn max_rate(self) -> i32 {
        self.ruleset_effect_value(EffectKind::MaxRates)
            .expect("selected government has a native Max_Rates value") as i32
    }

    pub(super) fn empire_size(self) -> i32 {
        self.ruleset_effect_value(EffectKind::EmpireSizeBase)
            .expect("selected government has a native Empire_Size_Base value") as i32
    }

    pub(crate) fn ruleset_effect_value(self, kind: EffectKind) -> Option<i64> {
        let record = self.native_record()?;
        Some(match kind {
            EffectKind::CivilWarChance => record.civil_war_chance.into(),
            EffectKind::EmpireSizeBase => record.empire_size_base.into(),
            EffectKind::MaxRates => record.max_rates.into(),
            _ => return None,
        })
    }

    pub(crate) fn has_flag(self, flag: &str) -> bool {
        self.native_record().is_some_and(|record| {
            record
                .flags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(flag))
        })
    }
}

impl Simulation {
    pub(crate) fn can_mobilize(&self, owner: usize) -> bool {
        owner < self.civilization_count
            && !self.players[owner].mobilized
            && self.has_tech(owner, Tech::NATIONALISM)
    }

    pub(super) fn mobilize(&mut self, owner: usize) -> bool {
        if !self.can_mobilize(owner) {
            return false;
        }
        self.players[owner].mobilized = true;
        self.add_event(format!("{} mobilizes for war.", self.players[owner].name));
        true
    }

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

    #[cfg(test)]
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
        (
            self.c2c3_city_output_after_bonus(city, shields, "Shield", government, granted),
            self.c2c3_city_output_after_bonus(city, trade, "Trade", government, granted),
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
        let current = player.government;
        let pending = player.revolution_target;
        if !self.government_requirements_met(owner, current) {
            if let Some(target) = self.random_available_government(owner) {
                self.change_government(owner, target);
            }
        } else if let Some(pending) = pending
            && pending != Government::during_revolution()
            && !self.government_requirements_met(owner, pending)
            && let Some(target) = self.random_available_government(owner)
        {
            self.players[owner].revolution_target = Some(target);
        }
        if owner == HUMAN
            && (current.requirement() == Some(lost)
                || pending.is_some_and(|government| government.requirement() == Some(lost)))
        {
            self.add_event(format!(
                "The lost technology {} changed your government options.",
                lost.name()
            ));
        }
    }

    fn random_available_government(&mut self, owner: usize) -> Option<Government> {
        let available: Vec<_> = Government::targets()
            .iter()
            .copied()
            .filter(|&candidate| self.can_change_government(owner, candidate))
            .collect();
        (!available.is_empty()).then(|| available[self.rand_below(available.len() as i32) as usize])
    }

    pub(crate) fn can_change_government(&self, owner: usize, target: Government) -> bool {
        target != self.players[owner].government && self.government_requirements_met(owner, target)
    }

    fn government_requirements_met(&self, owner: usize, target: Government) -> bool {
        if target.native_record().is_none() {
            return false;
        }
        if self.player_effect(owner, EffectKind::AnyGovernment) > 0 {
            return true;
        }
        target
            .requirement()
            .is_none_or(|technology| self.has_tech(owner, technology))
    }

    pub(super) fn begin_ai_war_revolution(&mut self, owner: usize) -> bool {
        let player = &self.players[owner];
        let temporary = Government::during_revolution();
        if !player.ai
            || player.government == temporary
            || player.revolution_turn == Some(self.turn)
                && self.player_effect(owner, EffectKind::NoAnarchy) <= 0
        {
            return false;
        }
        // dai_go_to_war requests the ruleset's during-revolution government.
        // Default Freeciv AI has no revolution handicap, so this request
        // enters Anarchy without rolling the human RANDQUICK duration.
        let finish = player
            .revolution_turn
            .filter(|&finish| finish >= self.turn)
            .unwrap_or(self.turn);
        let player = &mut self.players[owner];
        player.government = temporary;
        player.revolution_target = Some(temporary);
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
        self.start_government_revolution(owner, target)
    }

    fn start_government_revolution(&mut self, owner: usize, target: Government) -> bool {
        self.invalidate_stable_city_advice_for_owner(owner);
        let settings = RulesetData::campaign_default()
            .research_settings()
            .expect("valid C2C3 revolution settings");
        let no_anarchy = self.player_effect(owner, EffectKind::NoAnarchy) > 0;
        let already_revolting = self.players[owner].revolution_turn;
        let mut anarchy = !no_anarchy;
        let turns = if let Some(finish) = already_revolting {
            finish - self.turn
        } else if self.players[owner].ai || no_anarchy {
            // Freeciv's normal AI has no H_REVOLUTION handicap, so it skips
            // anarchy; No_Anarchy effects provide the same ruleset path.
            anarchy = false;
            0
        } else {
            self.random_quick_revolution_length(target, settings.revolution_length)
        };
        if anarchy
            && turns <= 0
            && self.players[owner].government != Government::during_revolution()
            && settings.revolution_type == crate::rules::freeciv::RevolutionType::RandomQuick
        {
            return false;
        }
        let temporary = Government::during_revolution();
        let finish = self.turn + turns;
        let player = &mut self.players[owner];
        player.government = temporary;
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
        true
    }

    pub(in crate::sim) fn update_city_disorder_revolution(&mut self, city_id: u32) {
        let Some(index) = self.city_index_by_id(city_id) else {
            return;
        };
        let city = self.cities[index].clone();
        let government = self.players[city.owner].government;
        let disorder = self.city_forecast(&city).disorder;
        let turns = self.city_rule_effect(
            &city,
            EffectKind::RevolutionUnhappiness,
            None,
            None,
            None,
            false,
            government,
            None,
        );
        let anarchy = if disorder {
            self.cities[index].anarchy.saturating_add(1)
        } else {
            0
        };
        self.cities[index].anarchy = anarchy;
        if turns > 0 && anarchy > turns && self.government_requirements_met(city.owner, government)
        {
            // cityturn.c calls handle_player_change_government() for the
            // current government, putting the country through a revolution
            // back to the same government unless No_Anarchy applies.
            self.start_government_revolution(city.owner, government);
        }
    }

    fn random_quick_revolution_length(&mut self, target: Government, length: i32) -> i32 {
        if length <= 1 {
            return 1;
        }
        // common/player.c::player_count counts used slots, including the
        // campaign's allocated barbarian player slot.
        let player_count = self.civilization_count as i32 + 1;
        let change_speed = ((player_count / 2) / (length - 1)).max(1);
        let max_turns = (length - self.government_adoptions[target as usize] / change_speed).max(1);
        self.rand_below(max_turns) + 1
    }

    pub(super) fn finish_revolutions(&mut self) {
        // city_refresh_for_player() has no cached-output equivalent here:
        // city forecasts read government and rates directly. Freeciv's worker
        // auto-arrangement is conditional on a city-radius tile-count change,
        // which the selected C2C3 governments do not cause.
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.player_alive(owner) {
                continue;
            }
            {
                let player = &mut self.players[owner];
                if player.revolution_turn.is_some_and(|turn| turn <= self.turn)
                    && player
                        .revolution_target
                        .is_some_and(|target| target != Government::during_revolution())
                    && let Some(target) = player.revolution_target.take()
                {
                    player.government = target;
                    self.government_adoptions[target as usize] += 1;
                    if owner == HUMAN {
                        player.rates =
                            rates_after_government_change(player.rates, target.max_rate());
                        self.add_event(format!("{} is now your government.", target.name()));
                    }
                } else if player.government != Government::during_revolution()
                    && player.revolution_turn.is_some_and(|turn| turn < self.turn)
                {
                    player.revolution_turn = None;
                }
            }
        }
    }
}

fn rates_after_government_change(rates: [i32; 3], cap: i32) -> [i32; 3] {
    let [science, tax, luxury] = rates;
    if [science, tax, luxury].into_iter().all(|rate| rate <= cap) {
        return rates;
    }
    // Freeciv keeps luxury unchanged, then assigns as much as possible to
    // science and tax under the new maximum government rate.
    let science = (100 - luxury).min(cap);
    let tax = (100 - luxury - science).min(cap);
    let luxury = 100 - science - tax;
    [science, tax, luxury]
}

#[cfg(test)]
#[path = "government/tests.rs"]
mod tests;
