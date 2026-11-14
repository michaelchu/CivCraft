//! Civil war and civilization split resolution.

use super::*;
use crate::rules::freeciv::{RulesetData, action_enabled, effect_value};

// Retained for the existing Civ III uprising path, which is scheduled for
// the P12 world-events migration. The capital-capture path below uses C2C3
// effects directly and does not call this compatibility helper.
impl Government {
    pub(super) fn civil_war_chance(self) -> i32 {
        if let Some(value) = self.ruleset_effect_value("Civil_War_Chance") {
            return i32::try_from(value).expect("government civil-war chance fits i32");
        }
        match self {
            Government::Fascism => 60,
            Government::Feudalism => 20,
            _ => unreachable!("selected government has a ruleset civil-war effect"),
        }
    }
}

impl Simulation {
    pub(super) fn capital_loss_triggers_civil_war(&mut self, owner: usize) -> bool {
        if owner == BARBARIANS || owner >= self.players.len() {
            return false;
        }

        let rules = RulesetData::campaign_default();
        let normal_player_count = self.players.len().saturating_sub(1);
        if normal_player_count >= PLAYER_SLOTS - 12 || !self.civil_war_possible(owner, true, true) {
            return false;
        }

        // plrhand.c::civil_war_triggered consumes the trigger draw before
        // evaluating either the government effect or city-status effects.
        let dice = self.rand_below(100);
        let player_context =
            super::action_rules::SimulationRequirementContext::for_player_at(self, owner, None)
                .with_government(self.players[owner].government);
        let base_chance = effect_value(&rules.effects, "Civil_War_Chance", &player_context)
            .expect("campaign C2C3 Civil_War_Chance requirements are supported");
        let city_bonus: i64 = self
            .player_cities_ordered(owner)
            .cloned()
            .map(|city| {
                let luxury = self.city_trade(&city, self.city_yields(&city).2)[2];
                let (disorder, happy) = self.city_mood(&city, luxury);
                let context = super::action_rules::SimulationRequirementContext::for_city_at(
                    self,
                    &city,
                    Some((city.x, city.y)),
                )
                .with_disordered(disorder)
                .with_celebrating(city.was_happy && happy);
                effect_value(&rules.effects, "Civil_War_City_Bonus", &context)
                    .expect("campaign C2C3 Civil_War_City_Bonus requirements are supported")
            })
            .sum();
        i64::from(dice) < base_chance - city_bonus
    }

    fn civil_war_possible(
        &self,
        owner: usize,
        conquering_city: bool,
        honour_server_option: bool,
    ) -> bool {
        let rules = RulesetData::campaign_default();
        let player_context =
            super::action_rules::SimulationRequirementContext::for_player_at(self, owner, None)
                .with_action("Civil War")
                .with_relation_target(Some(owner));
        let action_enabled = action_enabled(
            &rules.action_enablers,
            "Civil War",
            &player_context,
            &player_context,
        )
        .expect("campaign C2C3 Civil War action requirements are supported");
        if !action_enabled {
            return false;
        }

        let city_count = self.player_cities_ordered(owner).count();
        let remaining = city_count.saturating_sub(if conquering_city { 1 } else { 0 });
        if remaining < 2 {
            return false;
        }
        if !honour_server_option {
            return true;
        }
        let settings: crate::rules::freeciv::CivilWarRulesetSettings = rules
            .civil_war_settings()
            .expect("selected C2C3 civil-war settings are valid");
        settings.city_threshold < 1000 && city_count >= settings.city_threshold
    }

    pub(super) fn split_civil_war(&mut self, owner: usize) {
        let rules = RulesetData::campaign_default();
        let settings = rules
            .civil_war_settings()
            .expect("selected C2C3 civil-war settings are valid");
        let normal_player_count = self.players.len().saturating_sub(1);
        let playable_nations = rules
            .source_content
            .iter()
            .filter(|record| record.kind == crate::rules::freeciv::ContentKind::Nation)
            .count();
        if normal_player_count >= settings.max_players
            || normal_player_count >= playable_nations
            || normal_player_count >= PLAYER_SLOTS - 12
        {
            return;
        }
        let candidates: Vec<_> = self
            .player_cities_ordered(owner)
            .filter(|city| !city.buildings.contains(&Building::PALACE))
            .filter(|city| !self.city_hosts_game_loss_unit(city, owner))
            .filter_map(|city| {
                self.cities
                    .iter()
                    .position(|candidate| candidate.id == city.id)
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        self.clear_outgoing_embassies(owner);
        let rebel = self.players.len();
        let original = &mut self.players[owner];
        let gold = original.gold / 2;
        original.gold -= gold;
        original.research = 0;
        if original.government != Government::Anarchy {
            original.revolution_target = Some(original.government);
            original.revolution_turn = Some(self.turn + 1);
            original.government = Government::Anarchy;
        }
        const REBEL_NAMES: [&str; 5] = [
            "Free Cities",
            "Separatists",
            "Breakaway League",
            "Rebel Coalition",
            "Independent Union",
        ];
        let rebel_player = Player {
            name: REBEL_NAMES[(rebel - PLAYER_COUNT - 1) % REBEL_NAMES.len()].to_owned(),
            civilization_id: None,
            gold,
            rates: [60, 40, 0],
            government: Government::Despotism,
            history: 0,
            revolution_target: None,
            revolution_turn: None,
            mobilized: false,
            known: original.known.clone(),
            researching: original.researching,
            researching_saved: original.researching_saved,
            research_saved: original.research_saved,
            research_goal: original.research_goal,
            future_techs: original.future_techs,
            research: 0,
            last_science: 0,
            ai: true,
        };
        self.players.push(rebel_player);
        // daiplayer.c:dai_player_copy copies retained advisor wants during
        // player splits; they are nevertheless transient across save/load.
        let wants = self
            .ai_tech_wants
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| vec![0.0; Tech::all().count()]);
        self.ai_tech_wants.insert(rebel, wants);
        self.spaceships.push(Spaceship::default());
        self.nation_scores.push(NationScore::default());
        for tile in &mut self.tiles {
            tile.known_by[rebel] = tile.known_by[owner];
        }

        let mut remaining = (candidates.len() / 2).max(1);
        let mut defectors = Vec::new();
        for (position, &index) in candidates.iter().enumerate() {
            if remaining >= candidates.len() - position
                || (remaining > 0 && self.rand_below(2) == 1)
            {
                self.cities[index].transfer_citizens(rebel);
                self.list_order.city(&self.cities[index]);
                defectors.push(index);
                remaining -= 1;
            }
        }
        let capital = defectors[self.rand_below(defectors.len() as i32) as usize];
        self.cities[capital].buildings.push(Building::PALACE);
        let unit_ids: Vec<_> = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect();
        for id in unit_ids {
            let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) else {
                continue;
            };
            let rebel_home = self
                .cities
                .iter()
                .any(|city| Some(city.id) == unit.home_city && city.owner == rebel);
            if let Some(city) = self.cities.iter().find(|city| {
                matches!(city.owner, id if id == owner || id == rebel)
                    && (city.x, city.y) == (unit.x, unit.y)
            }) {
                if unit.home_city.is_some() && (city.owner != owner || rebel_home) {
                    unit.home_city = Some(city.id);
                }
                unit.owner = city.owner;
            } else if rebel_home {
                unit.owner = rebel;
            }
            self.list_order.unit(unit);
        }
        self.resolve_split_stacks(owner, rebel);
        self.rebuild_city_ownership();
        self.refresh_visibility();
        self.add_event(format!(
            "Civil war partitions {}. {} now hold {} cities.",
            self.players[owner].name,
            self.players[rebel].name,
            defectors.len()
        ));
    }

    fn city_hosts_game_loss_unit(&self, city: &City, owner: usize) -> bool {
        self.units.iter().any(|unit| {
            unit.owner == owner
                && (unit.x, unit.y) == (city.x, city.y)
                && super::action_rules::freeciv_unit_type_flag(unit.kind, "GameLoss")
                    .expect("campaign C2C3 GameLoss flags are explicitly mapped")
        })
    }

    pub(super) fn resolve_split_stacks(&mut self, original: usize, rebel: usize) {
        let mut relocations = Vec::new();
        for unit in &self.units {
            if unit.owner != rebel && unit.owner != original {
                continue;
            }
            let wrong_transport = unit.transport.is_some_and(|id| {
                self.units
                    .iter()
                    .any(|boat| boat.id == id && boat.owner != unit.owner)
            });
            let mixed = self.units.iter().any(|other| {
                other.owner != unit.owner
                    && (other.owner == original || other.owner == rebel)
                    && (other.x, other.y) == (unit.x, unit.y)
            });
            let foreign_city = self.cities.iter().any(|city| {
                (city.x, city.y) == (unit.x, unit.y) && !self.allied(city.owner, unit.owner)
            });
            if !wrong_transport && !mixed && !foreign_city {
                continue;
            }
            if !wrong_transport && unit.transport.is_some() {
                continue;
            }
            let city = self
                .cities
                .iter()
                .filter(|city| city.owner == unit.owner)
                .filter(|city| !unit.kind.is_naval() || self.naval_city_access(city, unit.kind))
                .min_by_key(|city| self.map_distance(unit.x, unit.y, city.x, city.y));
            let rival_distance = self
                .cities
                .iter()
                .filter(|city| {
                    (city.owner == original || city.owner == rebel) && city.owner != unit.owner
                })
                .map(|city| self.map_distance(unit.x, unit.y, city.x, city.y))
                .min()
                .unwrap_or(i32::MAX);
            let distance = city.map_or(i32::MAX, |city| {
                self.map_distance(unit.x, unit.y, city.x, city.y)
            });
            if wrong_transport
                || foreign_city
                || distance < rival_distance
                || (distance == rival_distance && unit.owner == rebel)
            {
                relocations.push((unit.id, city.map(|city| (city.x, city.y))));
            }
        }
        for (id, destination) in relocations {
            if let Some((x, y)) = destination {
                if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
                    (unit.x, unit.y) = (x, y);
                    unit.transport = None;
                    unit.moves_left = 0;
                }
                self.move_cargo(id, x, y);
            } else {
                self.remove_units_with_cargo(vec![id]);
            }
        }
    }
}
