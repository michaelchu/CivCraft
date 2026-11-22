//! Government selection and government-dependent rules.
pub(crate) mod observation;

use super::*;
use crate::rules::engine_rules::RulesetData;
use crate::sim::action_rules::SimulationRequirementContext;
use std::sync::OnceLock;

mod advisor;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Government(pub(crate) usize);

impl serde::Serialize for Government {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(self.name())
        } else {
            serializer.serialize_u64(self.0 as u64)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Government {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            let name = String::deserialize(deserializer)?;
            Self::from_rule_name(&name)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown government {name:?}")))
        } else {
            usize::try_from(u64::deserialize(deserializer)?)
                .map(Self)
                .map_err(|_| serde::de::Error::custom("government index overflows usize"))
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct GovernmentAdvisorCache {
    pub(super) evaluated_turn: i32,
    pub(super) next_evaluation: i32,
    pub(super) scores: Vec<f64>,
}

#[allow(dead_code)]
impl Government {
    pub(crate) const ANARCHY: Self = Self(0);
    pub(crate) const DESPOTISM: Self = Self(1);
    pub(crate) const MONARCHY: Self = Self(2);
    pub(crate) const COMMUNISM: Self = Self(3);
    pub(crate) const REPUBLIC: Self = Self(4);
    pub(crate) const DEMOCRACY: Self = Self(5);
    pub(crate) const FASCISM: Self = Self(6);
    pub(crate) const FEUDALISM: Self = Self(7);

    pub(crate) fn count() -> usize {
        crate::rules::native::governments().governments.len()
    }

    pub(crate) const fn index(self) -> usize {
        self.0
    }

    pub(crate) fn all() -> impl Iterator<Item = Self> {
        (0..Self::count()).map(Self)
    }

    pub(crate) fn during_revolution() -> Self {
        Self::from_rule_name("anarchy").unwrap_or(Self::ANARCHY)
    }

    pub(crate) fn targets() -> &'static [Self] {
        static TARGETS: OnceLock<Vec<Government>> = OnceLock::new();
        TARGETS.get_or_init(|| {
            let anarchy = Self::during_revolution();
            Self::all()
                .filter(|government| *government != anarchy)
                .collect()
        })
    }

    pub(crate) fn from_rule_name(name: &str) -> Option<Self> {
        crate::rules::native::governments()
            .governments
            .iter()
            .position(|government| {
                government.government.eq_ignore_ascii_case(name)
                    || government.name.eq_ignore_ascii_case(name)
            })
            .map(Self)
    }

    pub(crate) fn rule_name(self) -> Option<&'static str> {
        self.native_record()
            .map(|government| government.government.as_str())
    }

    pub(crate) fn name(self) -> &'static str {
        self.native_record()
            .map(|government| government.name.as_str())
            .unwrap_or("Unknown Government")
    }

    pub(crate) fn requirement(self) -> Option<Tech> {
        self.native_record()?.technology
    }

    fn native_record(self) -> Option<&'static crate::rules::native::NativeGovernment> {
        crate::rules::native::governments().governments.get(self.0)
    }

    pub(crate) fn max_rate(self) -> i32 {
        self.base_effect(EffectKind::MaxRates)
    }

    pub(super) fn empire_size(self) -> i32 {
        self.base_effect(EffectKind::EmpireSizeBase)
    }

    pub(super) fn civil_war_chance(self) -> i32 {
        self.base_effect(EffectKind::CivilWarChance)
    }

    fn base_effect(self, kind: EffectKind) -> i32 {
        let name = self
            .rule_name()
            .expect("campaign government has a ruleset identity");
        i32::try_from(RulesetData::campaign_default().government_base_effect(name, kind))
            .expect("government base effect fits i32")
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
            self.city_output_after_bonus(city, shields, "Shield", government, granted),
            self.city_output_after_bonus(city, trade, "Trade", government, granted),
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

    #[cfg(any(test, feature = "scenarios"))]
    pub(super) fn set_scenario_government(&mut self, owner: usize, government: Government) {
        assert!(
            Government::targets().contains(&government)
                && self.government_requirements_met(owner, government),
            "scenario government {government:?} is unavailable to slot {owner}"
        );
        let player = &mut self.players[owner];
        player.government = government;
        player.revolution_target = None;
        player.revolution_turn = None;
        player.rates = rates_after_government_change(player.rates, government.max_rate());
        self.ai_government_cache.remove(&owner);
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
        let previous = self.players[owner].government;
        let player = &mut self.players[owner];
        player.government = temporary;
        player.revolution_target = Some(temporary);
        player.revolution_turn = Some(finish);
        // The caller assigns its pending war target AFTER this callback,
        // matching daidiplomacy.c's native ordering.
        self.trace_government_change(owner, previous, "revolution_started");
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
        let settings = crate::rules::engine_rules::ResearchRulesetSettings::CAMPAIGN;
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
            && settings.revolution_type == crate::rules::engine_rules::RevolutionType::RandomQuick
        {
            return false;
        }
        let temporary = Government::during_revolution();
        let finish = self.turn + turns;
        let previous = self.players[owner].government;
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
        self.trace_government_change(owner, previous, "revolution_started");
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
        let max_turns = (length - self.government_adoptions[target.index()] / change_speed).max(1);
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
            let previous = self.players[owner].government;
            {
                let player = &mut self.players[owner];
                if player.revolution_turn.is_some_and(|turn| turn <= self.turn)
                    && player
                        .revolution_target
                        .is_some_and(|target| target != Government::during_revolution())
                    && let Some(target) = player.revolution_target.take()
                {
                    player.government = target;
                    self.government_adoptions[target.index()] += 1;
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
            if previous != self.players[owner].government {
                self.trace_government_change(owner, previous, "adopted");
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
