//! Civil war and civilization split resolution.

use super::*;
use crate::rules::engine_rules::RulesetData;

impl Simulation {
    pub(super) fn capital_loss_triggers_civil_war(&mut self, owner: usize) -> bool {
        if owner == BARBARIANS || owner >= self.players.len() {
            return false;
        }

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
        let base_chance = player_context.effect(EffectKind::CivilWarChance);
        let city_bonus: i64 = self
            .player_cities_ordered(owner)
            .map(|city| {
                let luxury = self.city_trade(city, self.city_yields(city).2)[2];
                let (disorder, happy) = self.city_mood(city, luxury);
                let context = super::action_rules::SimulationRequirementContext::for_city_at(
                    self,
                    city,
                    Some((city.x, city.y)),
                )
                .with_disordered(disorder)
                .with_celebrating(city.was_happy && happy);
                context.effect(EffectKind::CivilWarCityBonus)
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
        let action_enabled = rules
            .action_enabled("Civil War", &player_context, &player_context)
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
        let settings: crate::rules::engine_rules::CivilWarRulesetSettings = rules
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
        let playable_nations = crate::rules::civilizations::all().len();
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
        if original.government != Government::ANARCHY {
            original.revolution_target = Some(original.government);
            original.revolution_turn = Some(self.turn + 1);
            original.government = Government::ANARCHY;
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
            government: Government::DESPOTISM,
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
        for tech in self.players[rebel]
            .known
            .iter()
            .copied()
            .filter(|t| t.is_civ3_advance())
            .chain(std::iter::repeat_n(
                Tech::FUTURE,
                self.players[rebel].future_techs.max(0) as usize,
            ))
        {
            self.trace.technology_changed(
                self.turn,
                rebel,
                tech,
                research_observation::TechnologyAction::Acquired,
                research_observation::TechnologySource::CivilWar,
                Some(owner),
            );
        }
        // daiplayer.c:dai_player_copy copies retained advisor wants during
        // player splits; they are nevertheless transient across save/load.
        let wants = self
            .ai_tech_wants
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| vec![0.0; Tech::storage_len()]);
        self.ai_tech_wants.insert(rebel, wants);
        self.spaceships.push(Spaceship::default());
        self.nation_scores.push(NationScore::default());
        self.transfer_map_observations(owner, rebel, TreatyMap::World, |_| true);

        let mut remaining = (candidates.len() / 2).max(1);
        let mut defectors = Vec::new();
        for (position, &index) in candidates.iter().enumerate() {
            if remaining >= candidates.len() - position
                || (remaining > 0 && self.rand_below(2) == 1)
            {
                self.trace.city_transferred(
                    self.turn,
                    &self.cities[index],
                    rebel,
                    lifecycle::TransferReason::CivilWar,
                );
                self.cities[index].transfer_citizens(rebel);
                self.list_order.city(&self.cities[index]);
                defectors.push(index);
                remaining -= 1;
            }
        }
        let capital = defectors[self.rand_below(defectors.len() as i32) as usize];
        self.cities[capital].buildings.push(Building::PALACE);
        let leaders = self.leaders_of(owner);
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
                self.trace.unit_transferred(
                    self.turn,
                    unit,
                    city.owner,
                    lifecycle::TransferReason::CivilWar,
                );
                unit.owner = city.owner;
            } else if rebel_home {
                self.trace.unit_transferred(
                    self.turn,
                    unit,
                    rebel,
                    lifecycle::TransferReason::CivilWar,
                );
                unit.owner = rebel;
            }
            self.list_order.unit(unit);
        }
        self.remove_leaders_that_changed_owner(&leaders);
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
                && super::action_rules::unit_type_flag(unit.kind, "GameLoss")
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
                self.remove_units_with_cargo_because(vec![id], lifecycle::UnitRemoval::CivilWar);
            }
        }
    }
}
