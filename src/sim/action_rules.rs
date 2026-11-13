//! Runtime adapters for Freeciv action enablers and shared rule requirements.
//!
//! The action records and enablers come from the selected C2C3 ruleset. Civ III
//! `UnitKind` identities remain authoritative: this module maps their source
//! capabilities and classes into the Freeciv universals needed by the action
//! rules, without creating or substituting Freeciv unit identities.

use super::{
    Base, Contamination, Pact, PillageTarget, Simulation, Tile, Unit, UnitKind, UnitState, WorkKind,
};
use crate::rules::freeciv::{
    ActionEvaluationError, ContentKind, EffectContext, Requirement, RequirementContext,
    RequirementEvaluationError, RequirementRange, RequirementValue, RuleRecord, RulesetData,
    action_enabled, effect_value, requirements_met,
};
use crate::rules::{Building, Genus, Tech};
use crate::world::{Resource, Terrain};
use std::sync::OnceLock;

/// A reusable view of simulation state for shared Freeciv requirement
/// evaluation. `tile` is the position at which a prospective action or effect
/// is evaluated, which may differ from the unit's current position for a
/// projected movement or transport disembark.
pub(crate) struct SimulationRequirementContext<'a> {
    simulation: &'a Simulation,
    ruleset: &'static RulesetData,
    owner: usize,
    unit: Option<&'a Unit>,
    city: Option<&'a super::City>,
    tile: Option<(i32, i32)>,
    tile_override: Option<((i32, i32), Tile)>,
    city_tile_center_override: Option<((i32, i32), bool)>,
    building_override: Option<Building>,
    relation_target: Option<usize>,
    output_type: Option<String>,
    specialist: Option<String>,
    action: Option<String>,
    government: Option<super::Government>,
    granted_tech: Option<Tech>,
    celebrating: Option<bool>,
    disordered: Option<bool>,
    unit_type_flag_record: Option<&'static crate::rules::freeciv::ContentRecord>,
    unbound_content_is_absent: bool,
}

impl<'a> SimulationRequirementContext<'a> {
    pub(crate) fn for_unit(simulation: &'a Simulation, unit: &'a Unit) -> Self {
        Self::for_unit_at(simulation, unit, (unit.x, unit.y))
    }

    pub(crate) fn for_unit_at(
        simulation: &'a Simulation,
        unit: &'a Unit,
        tile: (i32, i32),
    ) -> Self {
        Self {
            simulation,
            ruleset: RulesetData::campaign_default(),
            owner: unit.owner,
            unit: Some(unit),
            city: None,
            tile: Some(tile),
            tile_override: None,
            city_tile_center_override: None,
            building_override: None,
            relation_target: None,
            output_type: None,
            specialist: None,
            action: None,
            government: None,
            granted_tech: None,
            celebrating: None,
            disordered: None,
            unit_type_flag_record: None,
            unbound_content_is_absent: false,
        }
    }

    pub(crate) fn for_player_at(
        simulation: &'a Simulation,
        owner: usize,
        tile: Option<(i32, i32)>,
    ) -> Self {
        Self {
            simulation,
            ruleset: RulesetData::campaign_default(),
            owner,
            unit: None,
            city: None,
            tile,
            tile_override: None,
            city_tile_center_override: None,
            building_override: None,
            relation_target: None,
            output_type: None,
            specialist: None,
            action: None,
            government: None,
            granted_tech: None,
            celebrating: None,
            disordered: None,
            unit_type_flag_record: None,
            unbound_content_is_absent: false,
        }
    }

    pub(crate) fn for_city_at(
        simulation: &'a Simulation,
        city: &'a super::City,
        tile: Option<(i32, i32)>,
    ) -> Self {
        Self {
            simulation,
            ruleset: RulesetData::campaign_default(),
            owner: city.owner,
            unit: None,
            city: Some(city),
            tile,
            tile_override: None,
            city_tile_center_override: None,
            building_override: None,
            relation_target: None,
            output_type: None,
            specialist: None,
            action: None,
            government: None,
            granted_tech: None,
            celebrating: None,
            disordered: None,
            unit_type_flag_record: None,
            unbound_content_is_absent: true,
        }
    }

    /// Build a target-city context evaluated on behalf of a different player.
    /// Freeciv uses this for effects such as `Inspire_Partisans`, where the
    /// transferred city supplies local requirements but the defeated owner
    /// supplies player requirements.
    pub(crate) fn for_player_in_city(
        simulation: &'a Simulation,
        owner: usize,
        city: &'a super::City,
    ) -> Self {
        let mut context = Self::for_city_at(simulation, city, Some((city.x, city.y)));
        context.owner = owner;
        context
    }

    pub(crate) fn with_output_type(mut self, output_type: &str) -> Self {
        self.output_type = Some(output_type.to_owned());
        self
    }

    pub(crate) fn with_unit_type_flag_record(
        mut self,
        record: &'static crate::rules::freeciv::ContentRecord,
    ) -> Self {
        self.unit_type_flag_record = Some(record);
        self
    }

    pub(crate) fn with_building_override(mut self, building: Option<Building>) -> Self {
        self.building_override = building;
        self
    }

    pub(crate) fn with_unit(mut self, unit: &'a Unit) -> Self {
        self.owner = unit.owner;
        self.unit = Some(unit);
        self
    }

    /// Attach an actor unit while preserving the target-owner scope of a city
    /// or player context. Effects such as Unit_No_Lose_Pop evaluate both the
    /// target city's buildings and the attacking unit's class/type.
    pub(crate) fn with_unit_preserving_owner(mut self, unit: &'a Unit) -> Self {
        self.unit = Some(unit);
        self
    }

    pub(crate) fn with_specialist(mut self, specialist: &str) -> Self {
        self.specialist = Some(specialist.to_owned());
        self
    }

    pub(crate) fn with_action(mut self, action: &str) -> Self {
        self.action = Some(action.to_owned());
        self
    }

    pub(crate) fn with_government(mut self, government: super::Government) -> Self {
        self.government = Some(government);
        self
    }

    pub(crate) fn with_granted_tech(mut self, tech: Option<Tech>) -> Self {
        self.granted_tech = tech;
        self
    }

    pub(crate) fn with_celebrating(mut self, celebrating: bool) -> Self {
        self.celebrating = Some(celebrating);
        self
    }

    pub(crate) fn with_disordered(mut self, disordered: bool) -> Self {
        self.disordered = Some(disordered);
        self
    }

    pub(crate) fn with_relation_target(mut self, owner: Option<usize>) -> Self {
        self.relation_target = owner;
        self
    }

    fn at_tile(&self, tile: (i32, i32)) -> Self {
        Self {
            simulation: self.simulation,
            ruleset: self.ruleset,
            owner: self.owner,
            unit: self.unit,
            city: self.city,
            tile: Some(tile),
            tile_override: self.tile_override.clone(),
            city_tile_center_override: self.city_tile_center_override,
            building_override: self.building_override,
            relation_target: self.relation_target,
            output_type: self.output_type.clone(),
            specialist: self.specialist.clone(),
            action: self.action.clone(),
            government: self.government,
            granted_tech: self.granted_tech,
            celebrating: self.celebrating,
            disordered: self.disordered,
            unit_type_flag_record: self.unit_type_flag_record,
            unbound_content_is_absent: self.unbound_content_is_absent,
        }
    }

    fn with_tile_override(mut self, tile: (i32, i32), state: &Tile) -> Self {
        self.tile_override = Some((tile, state.clone()));
        self
    }

    fn with_city_tile_center_override(mut self, tile: (i32, i32), is_city_center: bool) -> Self {
        self.city_tile_center_override = Some((tile, is_city_center));
        self
    }

    fn tile_state(&self, tile: (i32, i32)) -> &Tile {
        self.tile_override
            .as_ref()
            .filter(|(position, _)| *position == tile)
            .map_or_else(|| self.simulation.tile(tile.0, tile.1), |(_, state)| state)
    }

    fn extra_requirement(name: &str) -> Requirement {
        Requirement {
            kind: "Extra".to_owned(),
            name: RequirementValue::String(name.to_owned()),
            range: RequirementRange::Tile,
            present: true,
            survives: false,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: "terrain.ruleset".into(),
                section: format!("extra_{name}"),
                key: "reqs".to_owned(),
                line: 0,
            },
        }
    }

    fn unsupported(requirement: &Requirement) -> RequirementEvaluationError {
        RequirementEvaluationError::Unsupported {
            kind: requirement.kind.clone(),
            name: requirement.name.clone(),
        }
    }

    fn string_name<'b>(
        requirement: &'b Requirement,
    ) -> Result<&'b str, RequirementEvaluationError> {
        match &requirement.name {
            RequirementValue::String(name) => Ok(name),
            _ => Err(Self::unsupported(requirement)),
        }
    }

    fn terrain_record(
        &self,
        tile: (i32, i32),
    ) -> Result<&crate::rules::freeciv::ContentRecord, RequirementEvaluationError> {
        let terrain = self.tile_state(tile).terrain;
        let terrain_name = super::movement_rules::MovementRules::campaign()
            .terrain_data(terrain)
            .map_err(|_| RequirementEvaluationError::Unsupported {
                kind: "Terrain mapping".to_owned(),
                name: RequirementValue::String(terrain.name().to_owned()),
            })?
            .rule_name;
        self.ruleset
            .content
            .iter()
            .find(|record| {
                record.kind == ContentKind::Terrain
                    && record.rule_name.eq_ignore_ascii_case(&terrain_name)
            })
            .ok_or_else(|| RequirementEvaluationError::Unsupported {
                kind: "Terrain mapping".to_owned(),
                name: RequirementValue::String(terrain_name),
            })
    }

    fn tile_positions(
        &self,
        requirement: &Requirement,
    ) -> Result<Vec<(i32, i32)>, RequirementEvaluationError> {
        let tile = self.tile.ok_or_else(|| Self::unsupported(requirement))?;
        match requirement.range {
            RequirementRange::Tile => Ok(vec![tile]),
            RequirementRange::Adjacent => Ok(self
                .simulation
                .map_wrap
                .civ3_neighbors(tile.0, tile.1)
                .collect()),
            // Freeciv's CAdjacent range uses map_distance <= 1, so it includes
            // the target tile as well as its topology-adjacent neighbors.
            RequirementRange::CAdjacent => Ok(std::iter::once(tile)
                .chain(self.simulation.map_wrap.civ3_neighbors(tile.0, tile.1))
                .collect()),
            _ => Err(Self::unsupported(requirement)),
        }
    }

    fn terrain_field_matches(
        &self,
        requirement: &Requirement,
        field: &str,
        expected: &str,
    ) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        if !name.eq_ignore_ascii_case(expected) {
            return Err(Self::unsupported(requirement));
        }
        let tile = self.tile.ok_or_else(|| Self::unsupported(requirement))?;
        let record = self.terrain_record(tile)?;
        match field {
            "class" => Ok(record
                .string_field(field)
                .map_err(|_| Self::unsupported(requirement))?
                .is_some_and(|value| value.eq_ignore_ascii_case(name))),
            "flags" => Ok(record
                .string_list_field(field)
                .map_err(|_| Self::unsupported(requirement))?
                .is_some_and(|flags| flags.iter().any(|flag| flag.eq_ignore_ascii_case(name)))),
            _ => unreachable!("field helper only handles named terrain fields"),
        }
    }

    fn terrain_alter(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let field = match name.to_ascii_lowercase().as_str() {
            "canroad" => "road_time",
            "canmine" => "mining_time",
            "canirrigate" => "irrigation_time",
            "canbase" => "base_time",
            "cancultivate" => "cultivate_time",
            "canplant" => "plant_time",
            _ => return Err(Self::unsupported(requirement)),
        };
        let tile = self.tile.ok_or_else(|| Self::unsupported(requirement))?;
        let record = self.terrain_record(tile)?;
        let time = record
            .integer_field(field)
            .map_err(|_| Self::unsupported(requirement))?
            .ok_or_else(|| Self::unsupported(requirement))?;
        Ok(time > 0)
    }

    fn unit_class_rule_name(
        unit: UnitKind,
        requirement: &Requirement,
    ) -> Result<String, RequirementEvaluationError> {
        freeciv_unit_class_rule_name(unit).ok_or_else(|| Self::unsupported(requirement))
    }

    fn unit_class_flag(
        &self,
        unit: UnitKind,
        flag: &str,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let class_name = Self::unit_class_rule_name(unit, requirement)?;
        let record = self
            .ruleset
            .rule_records
            .iter()
            .find(|record| {
                record.namespace.eq_ignore_ascii_case("unit_class")
                    && record.rule_name.eq_ignore_ascii_case(&class_name)
            })
            .ok_or_else(|| Self::unsupported(requirement))?;
        let flags = record
            .string_list_field("flags")
            .map_err(|_| Self::unsupported(requirement))?
            .ok_or_else(|| Self::unsupported(requirement))?;
        Ok(flags
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(flag)))
    }

    fn tech_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let record = self
            .ruleset
            .content
            .iter()
            .find(|record| {
                record.kind == ContentKind::Technology
                    && (record.rule_name.eq_ignore_ascii_case(name)
                        || record
                            .display_name
                            .as_deref()
                            .is_some_and(|display| display.eq_ignore_ascii_case(name)))
            })
            .ok_or_else(|| Self::unsupported(requirement))?;
        let freeciv_name = record.display_name.as_deref().unwrap_or(&record.rule_name);
        let direct = Tech::all().find(|tech| tech.name().eq_ignore_ascii_case(freeciv_name));
        let tech = if let Some(tech) = direct {
            tech
        } else if let super::content_adapter::FreecivTechBinding::Bound(tech) =
            super::content_adapter::bind_freeciv_technology(self.ruleset, freeciv_name)
                .map_err(|_| Self::unsupported(requirement))?
        {
            tech
        } else {
            if self.unbound_content_is_absent {
                return Ok(false);
            }
            // A Freeciv-only technology has no Civ III player-state identity.
            return Err(Self::unsupported(requirement));
        };
        match requirement.range {
            RequirementRange::Player => {
                Ok(self.granted_tech == Some(tech) || self.simulation.has_tech(self.owner, tech))
            }
            RequirementRange::World if requirement.survives => {
                // Freeciv 9ff184b6 common/requirements.c::is_tech_req_active
                // resolves surviving World advances from game.info.global_advances,
                // a monotonic record that remains set after a player loses the tech.
                Ok(self.simulation.global_advances.contains(&tech))
            }
            RequirementRange::World => {
                // Freeciv 9ff184b6 common/requirements.c::is_tech_req_active
                // checks ordinary World advances with players_iterate_alive;
                // knowledge on the context player alone is insufficient.
                Ok((0..self.simulation.players.len()).any(|player| {
                    self.simulation.player_alive(player)
                        && (self.simulation.has_tech(player, tech)
                            || (player == self.owner && self.granted_tech == Some(tech)))
                }))
            }
            _ => Err(Self::unsupported(requirement)),
        }
    }

    fn building_present(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let record = self
            .ruleset
            .content
            .iter()
            .find(|record| {
                record.kind == ContentKind::Building
                    && (record.rule_name.eq_ignore_ascii_case(name)
                        || record
                            .display_name
                            .as_deref()
                            .is_some_and(|display| display.eq_ignore_ascii_case(name)))
            })
            .ok_or_else(|| Self::unsupported(requirement))?;
        let Some(building) = self.runtime_building_for_record(record) else {
            return if self.unbound_content_is_absent {
                Ok(false)
            } else {
                Err(Self::unsupported(requirement))
            };
        };

        let local_city = self
            .city
            .or_else(|| self.tile.and_then(|(x, y)| self.simulation.city_at(x, y)));
        let hypothetical_here = self.building_override == Some(building)
            && local_city.is_some_and(|city| city.owner == self.owner);
        match requirement.range {
            RequirementRange::City => Ok(hypothetical_here
                || local_city.is_some_and(|city| city.buildings.contains(&building))),
            RequirementRange::Player => {
                if hypothetical_here {
                    return Ok(true);
                }
                if matches!(
                    building.data().genus,
                    Genus::GreatWonder | Genus::SmallWonder
                ) {
                    Ok(self.simulation.has_active_wonder(self.owner, building))
                } else {
                    Ok(self
                        .simulation
                        .cities
                        .iter()
                        .any(|city| city.owner == self.owner && city.buildings.contains(&building)))
                }
            }
            RequirementRange::World => Ok(hypothetical_here
                || self.simulation.cities.iter().any(|city| {
                    city.buildings.contains(&building)
                        && (!matches!(
                            building.data().genus,
                            Genus::GreatWonder | Genus::SmallWonder
                        ) || self.simulation.has_active_wonder(city.owner, building))
                })),
            _ => Err(Self::unsupported(requirement)),
        }
    }

    fn runtime_building_for_record(
        &self,
        record: &crate::rules::freeciv::ContentRecord,
    ) -> Option<Building> {
        Building::all().find(|building| {
            building.data().name.eq_ignore_ascii_case(&record.rule_name)
                || record
                    .display_name
                    .as_deref()
                    .is_some_and(|display| building.data().name.eq_ignore_ascii_case(display))
                || c2c3_building_name_aliases(building.data().name)
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(&record.rule_name))
        })
    }

    fn building_flag_present(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let flag = Self::string_name(requirement)?;
        let mut records = Vec::new();
        for record in self
            .ruleset
            .content
            .iter()
            .filter(|record| record.kind == ContentKind::Building)
        {
            let flags = record
                .string_list_field("flags")
                .map_err(|_| Self::unsupported(requirement))?;
            if flags.is_some_and(|flags| {
                flags
                    .iter()
                    .any(|candidate| candidate.eq_ignore_ascii_case(flag))
            }) {
                records.push(record);
            }
        }
        if records.is_empty() {
            return Err(Self::unsupported(requirement));
        }

        let mut mapped = Vec::new();
        for record in records {
            if let Some(building) = self.runtime_building_for_record(record) {
                mapped.push(building);
            } else if !self.unbound_content_is_absent {
                return Err(Self::unsupported(requirement));
            }
        }

        let local_city = self
            .city
            .or_else(|| self.tile.and_then(|(x, y)| self.simulation.city_at(x, y)));
        let building_is_owned = |building: Building, city: &super::City| {
            city.buildings.contains(&building)
                && (!matches!(
                    building.data().genus,
                    Genus::GreatWonder | Genus::SmallWonder
                ) || self.simulation.has_active_wonder(city.owner, building))
        };
        let in_city = |building: Building, city: &super::City| {
            (self.building_override == Some(building) && city.owner == self.owner)
                || building_is_owned(building, city)
        };

        match requirement.range {
            RequirementRange::City => Ok(local_city.is_some_and(|city| {
                mapped
                    .iter()
                    .copied()
                    .any(|building| in_city(building, city))
            })),
            RequirementRange::Player => Ok(self.simulation.cities.iter().any(|city| {
                city.owner == self.owner
                    && mapped
                        .iter()
                        .copied()
                        .any(|building| in_city(building, city))
            })),
            RequirementRange::World => Ok(self.simulation.cities.iter().any(|city| {
                mapped
                    .iter()
                    .copied()
                    .any(|building| in_city(building, city))
            })),
            _ => Err(Self::unsupported(requirement)),
        }
    }

    fn building_genus_present(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let expected = [
            (Genus::Improvement, "Improvement"),
            (Genus::SmallWonder, "SmallWonder"),
            (Genus::GreatWonder, "GreatWonder"),
            (Genus::Special, "Special"),
            (Genus::Convert, "Convert"),
        ]
        .into_iter()
        .find_map(|(genus, candidate)| name.eq_ignore_ascii_case(candidate).then_some(genus))
        .ok_or_else(|| Self::unsupported(requirement))?;
        let city = self
            .city
            .or_else(|| self.tile.and_then(|(x, y)| self.simulation.city_at(x, y)));
        Ok(city.is_some_and(|city| {
            matches!(city.production, super::Production::Building(building)
                if building.data().genus == expected)
        }))
    }

    fn extra_record(&self, name: &str) -> Option<&RuleRecord> {
        self.ruleset.rule_records.iter().find(|record| {
            record.namespace.eq_ignore_ascii_case("extra")
                && (record.rule_name.eq_ignore_ascii_case(name)
                    || record
                        .display_name
                        .as_deref()
                        .is_some_and(|display| display.eq_ignore_ascii_case(name)))
        })
    }

    fn extra_present(
        &self,
        name: &str,
        tile: (i32, i32),
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let Some(_record) = self.extra_record(name) else {
            return Err(Self::unsupported(requirement));
        };
        let state = self.tile_state(tile);
        Ok(match name.to_ascii_lowercase().as_str() {
            "irrigation" => state.irrigation,
            "farmland" => state.farmland,
            "mine" => state.mine,
            "oil well" => state.oil_well,
            "oil platform" => state.oil_platform,
            "road" => state.road,
            "railroad" => state.railroad,
            "river" => state.river,
            "pollution" => state.pollution,
            "fallout" => state.fallout,
            "fort" => {
                state.base == Some(Base::Fort)
                    || state.base.and_then(Base::underlying_after_removal) == Some(Base::Fort)
            }
            "fortress" => state.base == Some(Base::Fortress),
            "airstrip" => {
                state.base == Some(Base::Airstrip)
                    || state.base.and_then(Base::underlying_after_removal) == Some(Base::Airstrip)
            }
            "airbase" => state.base == Some(Base::Airbase),
            "minor tribe village" | "ruins" => state.hut,
            // CivCraft's campaign tile model has no Buoy extra or source
            // identity, so the selected C2C3 conflict is absent in current
            // runtime state rather than an unresolved extra.
            "buoy" => false,
            "oasis" => state.active_resource() == Some(Resource::Oasis),
            "oil" => state.active_resource() == Some(Resource::Oil),
            "iron" => state.active_resource() == Some(Resource::Iron),
            "coal" => state.active_resource() == Some(Resource::Coal),
            "fish" => state.active_resource() == Some(Resource::Fish),
            "whales" => state.active_resource() == Some(Resource::Whales),
            "wheat" => state.active_resource() == Some(Resource::Wheat),
            "icy ivory" => state.active_resource() == Some(Resource::Ivory),
            "furs" => state.active_resource() == Some(Resource::Furs),
            "gems" => state.active_resource() == Some(Resource::Gems),
            "silk" => state.active_resource() == Some(Resource::Silk),
            "spice" => state.active_resource() == Some(Resource::Spice),
            "wine" => state.active_resource() == Some(Resource::Wine),
            "ivory" => state.active_resource() == Some(Resource::Ivory),
            "game" => state.active_resource() == Some(Resource::Game),
            "fruit" => state.active_resource() == Some(Resource::TropicalFruit),
            "buffalo" | "cattle" => state.active_resource() == Some(Resource::Cattle),
            _ if self.unbound_content_is_absent => false,
            _ => return Err(Self::unsupported(requirement)),
        })
    }

    fn player_has_technology_flag(
        &self,
        flag: &str,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let mut found_flag = false;
        for record in self
            .ruleset
            .content
            .iter()
            .filter(|record| record.kind == ContentKind::Technology)
        {
            let flags = record
                .string_list_field("flags")
                .map_err(|_| Self::unsupported(requirement))?
                .unwrap_or_default();
            if !flags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(flag))
            {
                continue;
            }
            found_flag = true;
            let name = record.display_name.as_deref().unwrap_or(&record.rule_name);
            // Prefer an exact Civ III catalog identity before consulting
            // crosswalk aliases. Bridge Building is present in both catalogs,
            // but its normalized rule identifier is not handled by the
            // generic alias table.
            let direct = Tech::all().find(|tech| tech.name().eq_ignore_ascii_case(name));
            let tech = if let Some(tech) = direct {
                tech
            } else {
                match super::content_adapter::bind_freeciv_technology(self.ruleset, name)
                    .map_err(|_| Self::unsupported(requirement))?
                {
                    super::content_adapter::FreecivTechBinding::Bound(tech) => tech,
                    super::content_adapter::FreecivTechBinding::MissingFromCiv3Catalog => {
                        return Err(Self::unsupported(requirement));
                    }
                }
            };
            if self.simulation.has_tech(self.owner, tech) {
                return Ok(true);
            }
        }
        if !found_flag {
            return Err(Self::unsupported(requirement));
        }
        Ok(false)
    }

    fn build_extra_requirements_met(
        &self,
        extra_name: &str,
        tile: (i32, i32),
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let Some(extra) = self.extra_record(extra_name) else {
            return Err(Self::unsupported(requirement));
        };
        if self.extra_present(extra_name, tile, requirement)? {
            return Ok(false);
        }
        if !requirements_met(&extra.requirements, &self.at_tile(tile))? {
            return Ok(false);
        }

        // Roads and railroads declare River in `bridged_over`. Freeciv checks
        // the player's technology flag, not Civ III Engineering. The selected
        // ruleset's tech binding resolves which Civ III tech grants that flag.
        let bridged_over = extra
            .string_list_field("bridged_over")
            .map_err(|_| Self::unsupported(requirement))?
            .unwrap_or_default();
        for bridged_extra in bridged_over {
            if self.extra_present(&bridged_extra, tile, requirement)?
                && !self.player_has_technology_flag("Bridge", requirement)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn extra_flag_present(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let flag = Self::string_name(requirement)?;
        let positions = self.tile_positions(requirement)?;
        let mut known_flag = false;
        for record in self
            .ruleset
            .rule_records
            .iter()
            .filter(|record| record.namespace.eq_ignore_ascii_case("extra"))
        {
            let flags = record
                .string_list_field("flags")
                .map_err(|_| Self::unsupported(requirement))?;
            if !flags
                .unwrap_or_default()
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(flag))
            {
                continue;
            }
            known_flag = true;
            for tile in &positions {
                if self.extra_present(&record.rule_name, *tile, requirement)? {
                    return Ok(true);
                }
            }
        }
        if !known_flag {
            return Err(Self::unsupported(requirement));
        }
        Ok(false)
    }

    fn unit_state(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let Some(unit) = self.unit else {
            return Ok(false);
        };
        let tile = self.tile.ok_or_else(|| Self::unsupported(requirement))?;
        let state = self.tile_state(tile);
        let native = super::movement_rules::MovementRules::campaign()
            .tile_native_to_class(
                state.terrain,
                state.road,
                state.railroad,
                state.river,
                state.base,
                super::movement_rules::MovementRules::campaign()
                    .unit_class_for_kind(unit.kind)
                    .map_err(|_| Self::unsupported(requirement))?,
            )
            .map_err(|_| Self::unsupported(requirement))?;
        Ok(match name.to_ascii_lowercase().as_str() {
            "transported" => unit.transport.is_some(),
            "transporting" => self
                .simulation
                .units
                .iter()
                .any(|cargo| cargo.transport == Some(unit.id)),
            "onnativetile" => native,
            "onlivabletile" => native || self.simulation.city_at(tile.0, tile.1).is_some(),
            "hashomecity" => unit.home_city.is_some(),
            "movedthisturn" => unit.moved,
            _ => return Err(Self::unsupported(requirement)),
        })
    }

    fn diplomatic_relation(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let Some(other) = self.relation_target.or_else(|| {
            self.tile
                .and_then(|position| self.tile_state(position).owner)
        }) else {
            return Err(Self::unsupported(requirement));
        };
        self.diplomatic_relation_between(self.owner, other, name, requirement)
    }

    fn diplomatic_relation_tile_other(
        &self,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let name = Self::string_name(requirement)?;
        let Some(tile) = self.tile else {
            return Err(Self::unsupported(requirement));
        };
        let Some(tile_owner) = self.tile_state(tile).owner else {
            return Ok(false);
        };
        let Some(other) = self.relation_target else {
            return Ok(false);
        };
        self.diplomatic_relation_between(tile_owner, other, name, requirement)
    }

    fn diplomatic_relation_between(
        &self,
        owner: usize,
        other: usize,
        name: &str,
        requirement: &Requirement,
    ) -> Result<bool, RequirementEvaluationError> {
        let same = owner == other;
        let key = Simulation::relation_key(owner, other);
        // Contact timer entries are retained after their current contact
        // expires, so entry presence records that the civilizations have met
        // at least once. If diplomacy begins pruning those entries, it needs
        // a separate persistent first-contact query for this universal.
        let met = same
            || self.simulation.diplomatic_contact_until.contains_key(&key)
            || self.simulation.has_embassy(owner, other)
            || self.simulation.has_embassy(other, owner)
            || self.simulation.pact(owner, other) != Pact::War;
        let pact = if same {
            Pact::Alliance
        } else {
            self.simulation.pact(owner, other)
        };
        Ok(match name.to_ascii_lowercase().as_str() {
            "foreign" => !same,
            "domestic" => same,
            "never met" => !met,
            // Freeciv DiplRel=War asks for the diplomatic state itself; the
            // `Never Met` relation is a separate universal. Players may be at
            // war before the campaign has recorded a contact event.
            "war" => pact == Pact::War,
            "cease-fire" | "ceasefire" => pact == Pact::Ceasefire,
            "armistice" => pact == Pact::Armistice,
            "peace" => pact == Pact::Peace,
            "alliance" => pact == Pact::Alliance,
            "has real embassy" => self.simulation.has_embassy(owner, other),
            // The current single-player model has no team assignment state;
            // each player is its own Freeciv team. This is centralized here
            // so future explicit teams can replace the policy for every
            // shared requirement/effect/action consumer.
            "team" => same,
            _ => return Err(Self::unsupported(requirement)),
        })
    }
}

impl RequirementContext for SimulationRequirementContext<'_> {
    fn is_present(&self, requirement: &Requirement) -> Result<bool, RequirementEvaluationError> {
        let unsupported = || Self::unsupported(requirement);
        let kind = requirement.kind.to_ascii_lowercase();
        match kind.as_str() {
            "action" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                Ok(self
                    .action
                    .as_deref()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(name)))
            }
            "building"
                if matches!(
                    requirement.range,
                    RequirementRange::City | RequirementRange::Player | RequirementRange::World
                ) =>
            {
                self.building_present(requirement)
            }
            "buildingflag"
                if matches!(
                    requirement.range,
                    RequirementRange::City | RequirementRange::Player | RequirementRange::World
                ) =>
            {
                self.building_flag_present(requirement)
            }
            "buildinggenus" if requirement.range == RequirementRange::Local => {
                self.building_genus_present(requirement)
            }
            "tech"
                if matches!(
                    requirement.range,
                    RequirementRange::Player | RequirementRange::World
                ) =>
            {
                self.tech_present(requirement)
            }
            "gov" if requirement.range == RequirementRange::Player => {
                let name = Self::string_name(requirement)?;
                let government = self
                    .government
                    .unwrap_or(self.simulation.players[self.owner].government);
                Ok(c2c3_government_rule_name(government)
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(name)))
            }
            "govflag" if requirement.range == RequirementRange::Player => {
                let name = Self::string_name(requirement)?;
                let government = self
                    .government
                    .unwrap_or(self.simulation.players[self.owner].government);
                let Some(rule_name) = government.rule_name() else {
                    return Ok(false);
                };
                let catalog = self
                    .ruleset
                    .government_catalog()
                    .map_err(|_| unsupported())?;
                Ok(catalog.get(rule_name).is_some_and(|record| {
                    record
                        .flags
                        .iter()
                        .any(|flag| flag.eq_ignore_ascii_case(name))
                }))
            }
            "outputtype" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                Ok(self
                    .output_type
                    .as_deref()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(name)))
            }
            "specialist" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                Ok(self
                    .specialist
                    .as_deref()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(name)))
            }
            "minsize" if requirement.range == RequirementRange::City => {
                let city = self
                    .city
                    .or_else(|| self.tile.and_then(|(x, y)| self.simulation.city_at(x, y)));
                let Some(city) = city else {
                    return Ok(false);
                };
                let minimum = match &requirement.name {
                    RequirementValue::String(value) => value.parse::<i32>().ok(),
                    RequirementValue::Integer(value) => i32::try_from(*value).ok(),
                    _ => None,
                }
                .ok_or_else(unsupported)?;
                Ok(city.size >= minimum)
            }
            "citystatus" if requirement.range == RequirementRange::City => {
                let name = Self::string_name(requirement)?;
                match name.to_ascii_lowercase().as_str() {
                    "celebration" => Ok(self.celebrating.unwrap_or(false)),
                    "disorder" => Ok(self.disordered.unwrap_or(false)),
                    _ => Err(unsupported()),
                }
            }
            "citytile" if requirement.range == RequirementRange::Tile => {
                let name = Self::string_name(requirement)?;
                let tile = self.tile.ok_or_else(unsupported)?;
                match name.to_ascii_lowercase().as_str() {
                    "center" => {
                        if let Some((_, is_city_center)) = self
                            .city_tile_center_override
                            .filter(|(position, _)| *position == tile)
                        {
                            Ok(is_city_center)
                        } else {
                            let city = self
                                .city
                                .or_else(|| self.simulation.city_at(tile.0, tile.1));
                            Ok(city.is_some_and(|city| (city.x, city.y) == tile))
                        }
                    }
                    // common/requirements.c::is_citytile_req_active defines
                    // CITYT_CLAIMED as tile ownership being present, including
                    // the negated/unclaimed form used by C2C3 paradrop.
                    "claimed" => Ok(self.tile_state(tile).owner.is_some()),
                    _ => Err(unsupported()),
                }
            }
            "ai" if requirement.range == RequirementRange::Player => {
                let name = Self::string_name(requirement)?;
                match name.to_ascii_lowercase().as_str() {
                    // CivCraft has no difficulty tier; the campaign uses the
                    // normal rules and does not enable these Freeciv cheats.
                    "cheating" | "restricted" => Ok(false),
                    _ => Err(unsupported()),
                }
            }
            "unittype" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                if !name.eq_ignore_ascii_case("Nuclear") {
                    return Err(unsupported());
                }
                // C2C3's single Nuclear type corresponds to both Civ III
                // nuclear identities; no other roster unit is inferred from
                // similarly named missiles or unit flags.
                Ok(self.unit.is_some_and(|unit| unit.kind.is_nuclear_weapon()))
            }
            "unitclass" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                let Some(unit) = self.unit else {
                    return Ok(false);
                };
                Ok(Self::unit_class_rule_name(unit.kind, requirement)?.eq_ignore_ascii_case(name))
            }
            "unitclassflag" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                let Some(unit) = self.unit else {
                    return Ok(false);
                };
                self.unit_class_flag(unit.kind, name, requirement)
            }
            "unittypeflag" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                let Some(unit) = self.unit else {
                    return Ok(false);
                };
                if let Some(record) = self.unit_type_flag_record {
                    let flags = record
                        .string_list_field("flags")
                        .map_err(|_| unsupported())?
                        .unwrap_or_default();
                    Ok(flags.iter().any(|flag| flag.eq_ignore_ascii_case(name)))
                } else {
                    freeciv_unit_type_flag(unit.kind, name).ok_or_else(unsupported)
                }
            }
            "terrain" if requirement.range == RequirementRange::Tile => {
                let name = Self::string_name(requirement)?;
                let tile = self.tile.ok_or_else(unsupported)?;
                let terrain_name = super::movement_rules::MovementRules::campaign()
                    .terrain_data(self.tile_state(tile).terrain)
                    .map_err(|_| unsupported())?
                    .rule_name;
                Ok(terrain_name.eq_ignore_ascii_case(name))
            }
            "terrainclass"
                if matches!(
                    requirement.range,
                    RequirementRange::Tile
                        | RequirementRange::Adjacent
                        | RequirementRange::CAdjacent
                ) =>
            {
                let name = Self::string_name(requirement)?;
                let positions = self.tile_positions(requirement)?;
                for tile in positions {
                    let record = self.terrain_record(tile)?;
                    if record
                        .string_field("class")
                        .map_err(|_| unsupported())?
                        .is_some_and(|class| class.eq_ignore_ascii_case(name))
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            "terrainflag" if requirement.range == RequirementRange::Tile => {
                self.terrain_field_matches(requirement, "flags", Self::string_name(requirement)?)
            }
            "terrainalter" if requirement.range == RequirementRange::Tile => {
                self.terrain_alter(requirement)
            }
            "nationgroup" if requirement.range == RequirementRange::Player => {
                let name = Self::string_name(requirement)?;
                if !name.eq_ignore_ascii_case("Barbarian") {
                    return Err(unsupported());
                }
                Ok(self.owner == super::BARBARIANS)
            }
            "nation" if requirement.range == RequirementRange::Player => {
                let name = Self::string_name(requirement)?;
                if !name.eq_ignore_ascii_case("Animal Kingdom") {
                    return Err(unsupported());
                }
                // The campaign uses the fixed Civ III roster and has no
                // Animal Kingdom nation. Its barbarian player is distinct
                // from Freeciv's Animal Kingdom nation as well.
                Ok(false)
            }
            "extra" if requirement.range == RequirementRange::Tile => {
                let name = Self::string_name(requirement)?;
                let tile = self.tile.ok_or_else(unsupported)?;
                self.extra_present(name, tile, requirement)
            }
            "extraflag"
                if matches!(
                    requirement.range,
                    RequirementRange::Tile
                        | RequirementRange::Adjacent
                        | RequirementRange::CAdjacent
                ) =>
            {
                self.extra_flag_present(requirement)
            }
            "activity" if requirement.range == RequirementRange::Local => {
                let name = Self::string_name(requirement)?;
                let Some(unit) = self.unit else {
                    return Ok(false);
                };
                match name.to_ascii_lowercase().as_str() {
                    "fortified" => Ok(unit.state == UnitState::Fortified),
                    _ => Err(unsupported()),
                }
            }
            "unitstate" if requirement.range == RequirementRange::Local => {
                self.unit_state(requirement)
            }
            "minmovefrags" if requirement.range == RequirementRange::Local => {
                let Some(unit) = self.unit else {
                    return Ok(false);
                };
                let threshold = match &requirement.name {
                    RequirementValue::String(value) => value.parse::<i32>().ok(),
                    RequirementValue::Integer(value) => i32::try_from(*value).ok(),
                    _ => None,
                }
                .ok_or_else(unsupported)?;
                Ok(unit.moves_left >= threshold)
            }
            "maxunitsontile" if requirement.range == RequirementRange::Tile => {
                let tile = self.tile.ok_or_else(unsupported)?;
                let maximum = match &requirement.name {
                    RequirementValue::String(value) => value.parse::<usize>().ok(),
                    RequirementValue::Integer(value) => usize::try_from(*value).ok(),
                    _ => None,
                }
                .ok_or_else(unsupported)?;
                let count = self
                    .simulation
                    .units
                    .iter()
                    .filter(|unit| (unit.x, unit.y) == tile)
                    .count();
                // common/requirements.c::is_maxtotalunitsontile_req_active
                // compares the total tile-unit count with this maximum.
                Ok(count <= maximum)
            }
            "diplrel" if requirement.range == RequirementRange::Local => {
                self.diplomatic_relation(requirement)
            }
            "diplreltileother" if requirement.range == RequirementRange::Local => {
                self.diplomatic_relation_tile_other(requirement)
            }
            _ => Err(unsupported()),
        }
    }
}

impl EffectContext for SimulationRequirementContext<'_> {
    fn multiplier_percent(&self, _multiplier: &str) -> Option<i64> {
        // The selected C2C3 Move_Bonus effects used by movement have no
        // multiplier field. Leave unknown effect multipliers unresolved so
        // future consumers must add their rule-specific player factor.
        None
    }
}

fn c2c3_government_rule_name(government: super::Government) -> Option<&'static str> {
    Some(match government {
        super::Government::Anarchy => "Anarchy",
        super::Government::Despotism => "Despotism",
        super::Government::Monarchy => "Monarchy",
        super::Government::Communism => "Communism",
        super::Government::Tribal => "Tribal",
        super::Government::Fundamentalism => "Fundamentalism",
        super::Government::Federation => "Federation",
        super::Government::Republic => "Republic",
        super::Government::Democracy => "Democracy",
        // Fascism and Feudalism are Civ III governments without a direct
        // identity in the selected C2C3 government catalog. P8 replaces these
        // runtime identities; do not infer Fundamentalism or Federation here.
        super::Government::Fascism | super::Government::Feudalism => return None,
    })
}

fn c2c3_building_name_aliases(runtime_name: &str) -> &'static [&'static str] {
    match runtime_name {
        "Harbor" => &["Harbour"],
        "Shakespeare's Theater" => &["Shakespeare's Theatre"],
        _ => &[],
    }
}

impl Simulation {
    fn report_action_evaluation_error(action: &str, error: &ActionEvaluationError) {
        // Unsupported rule requirements fail closed in release builds. In
        // debug/test runs, surface the exact action and requirement where it
        // was encountered instead of silently treating it as unavailable.
        debug_assert!(false, "could not evaluate C2C3 action {action}: {error:?}");
    }

    pub(crate) fn action_available_fail_closed(
        &self,
        actor_id: u32,
        action: &str,
        target_xy: (i32, i32),
    ) -> bool {
        match self.action_available(actor_id, action, target_xy) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(action, &error);
                false
            }
        }
    }

    pub(crate) fn action_available_for_unit_fail_closed(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
    ) -> bool {
        match self.action_available_for_unit(actor, action, target_xy) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(action, &error);
                false
            }
        }
    }

    /// Evaluate an action from the player's remembered target tile. When a
    /// tile snapshot is supplied, its owner is authoritative even when it is
    /// `None`; consulting the live tile in that case would leak unseen state.
    pub(crate) fn action_available_for_unit_with_target_state_fail_closed(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
        target_state: &Tile,
    ) -> bool {
        match self.action_available_for_unit_with_tile(actor, action, target_xy, Some(target_state))
        {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(action, &error);
                false
            }
        }
    }

    /// Evaluate an action using the player's remembered tile and city-site
    /// state. Both snapshots are authoritative for target requirements, so
    /// hidden live ownership or city presence cannot affect action selection.
    pub(crate) fn action_available_for_unit_with_known_target_state_fail_closed(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
        target_state: &Tile,
        target_is_city_center: bool,
    ) -> bool {
        match self.action_available_for_unit_with_context(
            actor,
            action,
            target_xy,
            (actor.x, actor.y),
            Some(target_state),
            None,
            None,
            None,
            Some((target_xy, target_is_city_center)),
        ) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(action, &error);
                false
            }
        }
    }

    /// Evaluate a Freeciv action's ruleset enablers at a target tile. This is
    /// the enabler layer only; movement geometry, target-extra presence,
    /// project conflicts and action-result handlers remain separate checks.
    pub(crate) fn action_available(
        &self,
        actor_id: u32,
        action: &str,
        target_xy: (i32, i32),
    ) -> Result<bool, ActionEvaluationError> {
        let Some(actor) = self.unit_by_id(actor_id) else {
            return Ok(false);
        };
        self.action_available_for_unit(actor, action, target_xy)
    }

    /// Borrowed-unit form used by rule evaluators that already hold a stable
    /// Civ III actor record.
    pub(crate) fn action_available_for_unit(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
    ) -> Result<bool, ActionEvaluationError> {
        self.action_available_for_unit_with_tile(actor, action, target_xy, None)
    }

    /// Evaluate an action against a specific unit in the target stack. This
    /// keeps the actor and target requirement contexts shared while allowing
    /// callers such as bombardment to check each actual defender.
    pub(crate) fn action_available_for_unit_against_unit(
        &self,
        actor: &Unit,
        action: &str,
        target: &Unit,
    ) -> Result<bool, ActionEvaluationError> {
        self.action_available_for_unit_with_context(
            actor,
            action,
            (target.x, target.y),
            (actor.x, actor.y),
            None,
            None,
            Some(target),
            None,
            None,
        )
    }

    pub(crate) fn action_available_for_unit_against_unit_fail_closed(
        &self,
        actor: &Unit,
        action: &str,
        target: &Unit,
    ) -> bool {
        match self.action_available_for_unit_against_unit(actor, action, target) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(action, &error);
                false
            }
        }
    }

    /// Evaluate an action against one specific city target. The target city
    /// supplies the diplomatic owner and city-range requirements even when
    /// the tile ownership cache has not yet been refreshed.
    pub(crate) fn action_available_for_unit_against_city(
        &self,
        actor: &Unit,
        action: &str,
        target: &super::City,
    ) -> Result<bool, ActionEvaluationError> {
        self.action_available_for_unit_with_context(
            actor,
            action,
            (target.x, target.y),
            (actor.x, actor.y),
            None,
            None,
            None,
            Some(target),
            None,
        )
    }

    /// Evaluate an action for a Civ III actor using the selected Freeciv
    /// unit record that supplies its ruleset flags. Automatic upkeep actions
    /// use this path because their identities come from the explicit
    /// C2C3 economics crosswalk rather than the movement/action identity map.
    pub(crate) fn action_available_for_unit_with_record(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
        unit_record: &'static crate::rules::freeciv::ContentRecord,
        target_city: Option<&super::City>,
    ) -> Result<bool, ActionEvaluationError> {
        self.action_available_for_unit_with_context(
            actor,
            action,
            target_xy,
            (actor.x, actor.y),
            None,
            Some(unit_record),
            None,
            target_city,
            None,
        )
    }

    fn action_available_for_unit_with_tile(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
        target_state: Option<&Tile>,
    ) -> Result<bool, ActionEvaluationError> {
        self.action_available_for_unit_with_actor_tile(
            actor,
            action,
            target_xy,
            (actor.x, actor.y),
            target_state,
        )
    }

    fn action_available_for_unit_with_actor_tile(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
        actor_tile_xy: (i32, i32),
        target_state: Option<&Tile>,
    ) -> Result<bool, ActionEvaluationError> {
        self.action_available_for_unit_with_context(
            actor,
            action,
            target_xy,
            actor_tile_xy,
            target_state,
            None,
            None,
            None,
            None,
        )
    }

    fn action_available_for_unit_with_context(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
        actor_tile_xy: (i32, i32),
        target_state: Option<&Tile>,
        unit_record: Option<&'static crate::rules::freeciv::ContentRecord>,
        target_unit_override: Option<&Unit>,
        target_city: Option<&super::City>,
        city_tile_center_override: Option<((i32, i32), bool)>,
    ) -> Result<bool, ActionEvaluationError> {
        if target_unit_override.is_some_and(|unit| (unit.x, unit.y) != target_xy) {
            return Ok(false);
        }
        let target_unit =
            target_unit_override.or_else(|| self.action_target_unit(actor, action, target_xy));
        let target_city = target_city.filter(|city| (city.x, city.y) == target_xy);
        let target_owner = target_unit
            .map(|unit| unit.owner)
            .or_else(|| target_city.map(|city| city.owner))
            .or_else(|| {
                target_state.map_or_else(
                    || self.tile(target_xy.0, target_xy.1).owner,
                    |state| state.owner,
                )
            });
        let mut actor_context =
            SimulationRequirementContext::for_unit_at(self, actor, actor_tile_xy)
                .with_relation_target(Some(target_owner.unwrap_or(actor.owner)))
                .with_action(action);
        if let Some(record) = unit_record {
            actor_context = actor_context.with_unit_type_flag_record(record);
        }
        if let Some(state) = target_state
            && actor_tile_xy == target_xy
        {
            actor_context = actor_context.with_tile_override(target_xy, state);
        }
        if let Some((position, is_city_center)) = city_tile_center_override
            && actor_tile_xy == position
        {
            actor_context = actor_context.with_city_tile_center_override(position, is_city_center);
        }
        let target_context = if let Some(unit) = target_unit {
            SimulationRequirementContext::for_unit_at(self, unit, target_xy)
        } else if let Some(city) = target_city {
            SimulationRequirementContext::for_city_at(self, city, Some(target_xy))
        } else {
            SimulationRequirementContext::for_player_at(
                self,
                target_owner.unwrap_or(actor.owner),
                Some(target_xy),
            )
        }
        .with_relation_target(Some(actor.owner))
        .with_action(action);
        let target_context = if let Some(state) = target_state {
            target_context.with_tile_override(target_xy, state)
        } else {
            target_context
        };
        let target_context = if let Some((position, is_city_center)) = city_tile_center_override {
            target_context.with_city_tile_center_override(position, is_city_center)
        } else {
            target_context
        };
        Ok(action_enabled(
            &RulesetData::campaign_default().action_enablers,
            action,
            &actor_context,
            &target_context,
        )?)
    }

    /// Select the C2C3 disembark enabler for movement. The cargo and carrier
    /// have already moved together to the destination by the time Freeciv
    /// evaluates the disembark action, so actor-local requirements use that
    /// destination tile rather than the cargo's pre-move source tile.
    pub(crate) fn transport_disembark_action_for_unit(
        &self,
        actor: &Unit,
        destination_xy: (i32, i32),
    ) -> Result<Option<&'static str>, ActionEvaluationError> {
        for action in ["Transport Disembark", "Transport Disembark 2"] {
            if self.action_available_for_unit_with_actor_tile(
                actor,
                action,
                destination_xy,
                destination_xy,
                None,
            )? {
                return Ok(Some(action));
            }
        }
        Ok(None)
    }

    pub(crate) fn transport_disembark_action_for_unit_fail_closed(
        &self,
        actor: &Unit,
        destination_xy: (i32, i32),
    ) -> Option<&'static str> {
        match self.transport_disembark_action_for_unit(actor, destination_xy) {
            Ok(action) => action,
            Err(error) => {
                Self::report_action_evaluation_error("Transport Disembark", &error);
                None
            }
        }
    }

    /// Freeciv action name associated with a CivCraft worker-order identity.
    /// C2C3 uses `Build Road` for both road classes, `Build Irrigation` for the
    /// irrigation/farmland activity family, and `Build Mine` for mine/oil
    /// activity variants.
    pub(crate) fn freeciv_action_for_work(kind: WorkKind) -> &'static str {
        match kind {
            WorkKind::Road | WorkKind::Railroad => "Build Road",
            WorkKind::Irrigation | WorkKind::Farmland => "Build Irrigation",
            WorkKind::Mine | WorkKind::OilWell | WorkKind::OilPlatform => "Build Mine",
            WorkKind::Cultivate => "Cultivate",
            WorkKind::Plant => "Plant",
            WorkKind::BuildBase(_) => "Build Base",
            WorkKind::Pillage(_) => "Pillage",
            WorkKind::Clean(_) => "Clean",
        }
    }

    /// Evaluate action enablers and the selected build extra's own ruleset
    /// requirements for worker orders that create an extra. Existing terrain,
    /// project and target-conflict checks are still the caller's responsibility.
    pub(crate) fn work_action_available(
        &self,
        actor_id: u32,
        kind: WorkKind,
        target_xy: (i32, i32),
    ) -> Result<bool, ActionEvaluationError> {
        let Some(actor) = self.unit_by_id(actor_id) else {
            return Ok(false);
        };
        self.work_action_available_for_unit(actor, kind, target_xy)
    }

    pub(crate) fn work_action_available_for_unit(
        &self,
        actor: &Unit,
        kind: WorkKind,
        target_xy: (i32, i32),
    ) -> Result<bool, ActionEvaluationError> {
        self.work_action_available_for_unit_with_tile(actor, kind, target_xy, None)
    }

    pub(crate) fn work_action_available_for_unit_with_tile(
        &self,
        actor: &Unit,
        kind: WorkKind,
        target_xy: (i32, i32),
        target_state: Option<&Tile>,
    ) -> Result<bool, ActionEvaluationError> {
        if kind == WorkKind::Clean(Contamination::Craters) {
            // Craters are a Civ III-only state. The selected C2C3 ruleset has
            // no corresponding extra, clean requirement, or removal time.
            return Ok(false);
        }
        let action = Self::freeciv_action_for_work(kind);
        if !self.action_available_for_unit_with_tile(actor, action, target_xy, target_state)? {
            return Ok(false);
        }
        let Some(extra_name) = build_extra_for_work(kind) else {
            return Ok(true);
        };
        let context = SimulationRequirementContext::for_unit_at(self, actor, target_xy);
        let context = if let Some(state) = target_state {
            context.with_tile_override(target_xy, state)
        } else {
            context
        };
        let requirement = SimulationRequirementContext::extra_requirement(extra_name);
        Ok(context.build_extra_requirements_met(extra_name, target_xy, &requirement)?)
    }

    pub(crate) fn work_action_available_for_unit_with_tile_fail_closed(
        &self,
        actor: &Unit,
        kind: WorkKind,
        target_xy: (i32, i32),
        target_state: Option<&Tile>,
    ) -> bool {
        match self.work_action_available_for_unit_with_tile(actor, kind, target_xy, target_state) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(Self::freeciv_action_for_work(kind), &error);
                false
            }
        }
    }

    /// C2C3 carrier compatibility for the Civ III identities currently bound
    /// to selected unit records. Unbound passenger identities use the explicit
    /// no-record ledger default (false), never Civ III cargo metadata.
    pub(crate) fn ruleset_transport_accepts(
        &self,
        carrier: UnitKind,
        passenger: UnitKind,
    ) -> Result<bool, ActionEvaluationError> {
        let Some(record) = self.transport_unit_record(carrier)? else {
            return Ok(false);
        };
        if self.ruleset_transport_capacity(carrier)? == 0 {
            return Ok(false);
        }
        let cargo_classes = record
            .string_list_field("cargo")
            .map_err(|_| Self::unsupported_transport_unit(carrier))?
            .ok_or_else(|| Self::unsupported_transport_unit(carrier))?;
        let class = self.ruleset_unit_class_rule_name(passenger)?;
        Ok(cargo_classes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&class)))
    }

    /// Freeciv's transport type-pair check rejects reciprocal cargo links:
    /// the cargo type must not itself transport the carrier's class.
    pub(crate) fn ruleset_transport_pair_allowed(
        &self,
        carrier: UnitKind,
        passenger: UnitKind,
    ) -> Result<bool, ActionEvaluationError> {
        if !self.ruleset_transport_accepts(carrier, passenger)? {
            return Ok(false);
        }
        Ok(!self.ruleset_transport_accepts(passenger, carrier)?)
    }

    /// Reject a load that would make a carrier chain contain the cargo unit.
    /// CivCraft stores one parent id per unit and can check this cycle shape;
    /// Freeciv's broader type-level recursion is handled separately above.
    pub(crate) fn transport_would_create_cycle(&self, passenger_id: u32, carrier_id: u32) -> bool {
        let mut current = Some(carrier_id);
        let mut remaining = self.units.len().saturating_add(1);
        while let Some(id) = current {
            if id == passenger_id || remaining == 0 {
                return true;
            }
            remaining -= 1;
            current = self.unit_by_id(id).and_then(|unit| unit.transport);
        }
        false
    }

    /// Whether the C2C3 selected carrier record has a free slot. This is kept
    /// separate from cargo-class compatibility to support path planning.
    pub(crate) fn ruleset_transport_has_space(
        &self,
        carrier: UnitKind,
        cargo_count: usize,
    ) -> Result<bool, ActionEvaluationError> {
        Ok(self.ruleset_transport_capacity(carrier)? > cargo_count)
    }

    pub(crate) fn ruleset_transport_capacity(
        &self,
        carrier: UnitKind,
    ) -> Result<usize, ActionEvaluationError> {
        let Some(record) = self.transport_unit_record(carrier)? else {
            return Ok(0);
        };
        let capacity = record
            .integer_field("transport_cap")
            .map_err(|_| Self::unsupported_transport_unit(carrier))?
            .ok_or_else(|| Self::unsupported_transport_unit(carrier))?;
        usize::try_from(capacity).map_err(|_| Self::unsupported_transport_unit(carrier))
    }

    /// Ruleset field-load exception for a passenger type. The named class is
    /// the carrier's C2C3 class; an unbound passenger uses an explicit empty
    /// `embarks`/`disembarks` default recorded by the no-type-record ledger.
    pub(crate) fn ruleset_unit_can_embark(
        &self,
        passenger: UnitKind,
        carrier: UnitKind,
    ) -> Result<bool, ActionEvaluationError> {
        self.ruleset_unit_allows_transport_class(passenger, carrier, "embarks")
    }

    pub(crate) fn ruleset_unit_can_disembark(
        &self,
        passenger: UnitKind,
        carrier: UnitKind,
    ) -> Result<bool, ActionEvaluationError> {
        self.ruleset_unit_allows_transport_class(passenger, carrier, "disembarks")
    }

    pub(crate) fn ruleset_unit_can_embark_fail_closed(
        &self,
        passenger: UnitKind,
        carrier: UnitKind,
    ) -> bool {
        match self.ruleset_unit_can_embark(passenger, carrier) {
            Ok(allows) => allows,
            Err(error) => {
                Self::report_unexpected_transport_error("Transport Embark", &error);
                false
            }
        }
    }

    pub(crate) fn ruleset_unit_can_disembark_fail_closed(
        &self,
        passenger: UnitKind,
        carrier: UnitKind,
    ) -> bool {
        match self.ruleset_unit_can_disembark(passenger, carrier) {
            Ok(allows) => allows,
            Err(error) => {
                Self::report_unexpected_transport_error("Transport Disembark", &error);
                false
            }
        }
    }

    fn ruleset_unit_allows_transport_class(
        &self,
        passenger: UnitKind,
        carrier: UnitKind,
        field: &str,
    ) -> Result<bool, ActionEvaluationError> {
        let Some(record) = self.direct_unit_record(passenger)? else {
            return Ok(false);
        };
        let carrier_class = self.ruleset_unit_class_rule_name(carrier)?;
        let classes = record
            .string_list_field(field)
            .map_err(|_| Self::unsupported_transport_field(passenger, field))?
            .unwrap_or_default();
        Ok(classes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&carrier_class)))
    }

    /// Resolve the selected C2C3 unit class for transport data. A direct
    /// identity record supplies its own `class`; identities without such a
    /// record use the shared Civ III-to-C2C3 class adapter explicitly.
    pub(crate) fn ruleset_unit_class_rule_name(
        &self,
        unit: UnitKind,
    ) -> Result<String, ActionEvaluationError> {
        freeciv_unit_class_rule_name(unit).ok_or_else(|| Self::unsupported_transport_class(unit))
    }

    fn transport_unit_record(
        &self,
        unit: UnitKind,
    ) -> Result<Option<&'static crate::rules::freeciv::ContentRecord>, ActionEvaluationError> {
        let Some(id) = unit.conquests_id() else {
            return Err(Self::unsupported_transport_unit(unit));
        };
        if C2C3_TRANSPORT_RECORD_GAPS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(id))
        {
            return Err(Self::unsupported_transport_unit(unit));
        }
        let record_name = C2C3_TRANSPORT_UNIT_RECORDS
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(id))
            .map(|(_, section)| *section);
        let record = if let Some(section) = record_name {
            RulesetData::campaign_default()
                .content
                .iter()
                .find(|record| {
                    record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(section)
                })
        } else {
            self.direct_unit_record(unit)?
        };
        if record.is_none()
            && !C2C3_DEFAULT_NO_TYPE_FLAGS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(id))
            && !C2C3_TRANSPORT_RECORD_GAPS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(id))
        {
            return Err(Self::unsupported_transport_unit(unit));
        }
        Ok(record)
    }

    fn direct_unit_record(
        &self,
        unit: UnitKind,
    ) -> Result<Option<&'static crate::rules::freeciv::ContentRecord>, ActionEvaluationError> {
        let Some(id) = unit.conquests_id() else {
            return Err(Self::unsupported_transport_unit(unit));
        };
        if let Some(record) = direct_c2c3_unit_record(unit) {
            return Ok(Some(record));
        }
        if C2C3_DEFAULT_NO_TYPE_FLAGS
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(id))
        {
            return Ok(None);
        }
        Err(Self::unsupported_transport_unit(unit))
    }

    fn unsupported_transport_unit(unit: UnitKind) -> ActionEvaluationError {
        Self::unsupported_transport_mapping("Transport unit record", unit)
    }

    fn unsupported_transport_class(unit: UnitKind) -> ActionEvaluationError {
        Self::unsupported_transport_mapping("Transport unit class", unit)
    }

    fn unsupported_transport_field(unit: UnitKind, field: &str) -> ActionEvaluationError {
        ActionEvaluationError::Requirement(RequirementEvaluationError::Unsupported {
            kind: format!("Transport unit field {field}"),
            name: RequirementValue::String(unit.conquests_id().unwrap_or(unit.name()).to_owned()),
        })
    }

    fn unsupported_transport_mapping(kind: &str, unit: UnitKind) -> ActionEvaluationError {
        ActionEvaluationError::Requirement(RequirementEvaluationError::Unsupported {
            kind: kind.to_owned(),
            name: RequirementValue::String(unit.conquests_id().unwrap_or(unit.name()).to_owned()),
        })
    }

    fn is_expected_transport_record_gap(error: &ActionEvaluationError) -> bool {
        let ActionEvaluationError::Requirement(RequirementEvaluationError::Unsupported {
            kind,
            name: RequirementValue::String(name),
        }) = error
        else {
            return false;
        };
        kind.starts_with("Transport unit")
            && C2C3_TRANSPORT_RECORD_GAPS
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(name))
    }

    fn report_unexpected_transport_error(action: &str, error: &ActionEvaluationError) {
        if !Self::is_expected_transport_record_gap(error) {
            Self::report_action_evaluation_error(action, error);
        }
    }

    pub(crate) fn ruleset_transport_pair_allowed_fail_closed(
        &self,
        carrier: UnitKind,
        passenger: UnitKind,
    ) -> bool {
        match self.ruleset_transport_pair_allowed(carrier, passenger) {
            Ok(allowed) => allowed,
            Err(error) => {
                Self::report_unexpected_transport_error("Transport", &error);
                false
            }
        }
    }

    pub(crate) fn ruleset_transport_has_space_fail_closed(
        &self,
        carrier: UnitKind,
        cargo_count: usize,
    ) -> bool {
        match self.ruleset_transport_has_space(carrier, cargo_count) {
            Ok(has_space) => has_space,
            Err(error) => {
                Self::report_unexpected_transport_error("Transport", &error);
                false
            }
        }
    }

    pub(crate) fn work_action_available_fail_closed(
        &self,
        actor_id: u32,
        kind: WorkKind,
        target_xy: (i32, i32),
    ) -> bool {
        match self.work_action_available(actor_id, kind, target_xy) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(Self::freeciv_action_for_work(kind), &error);
                false
            }
        }
    }

    pub(crate) fn work_action_available_for_unit_fail_closed(
        &self,
        actor: &Unit,
        kind: WorkKind,
        target_xy: (i32, i32),
    ) -> bool {
        match self.work_action_available_for_unit(actor, kind, target_xy) {
            Ok(available) => available,
            Err(error) => {
                Self::report_action_evaluation_error(Self::freeciv_action_for_work(kind), &error);
                false
            }
        }
    }

    /// Freeciv's `terrain_extra_build_time()` uses an extra's positive
    /// `build_time` when present; otherwise it multiplies the terrain's
    /// activity time by the extra's `build_time_factor`.
    pub(crate) fn work_activity_time(
        &self,
        kind: WorkKind,
        terrain: Terrain,
    ) -> Result<i32, ActionEvaluationError> {
        if let WorkKind::Clean(target) = kind {
            let extra_name = match target {
                Contamination::Pollution => "Pollution",
                Contamination::Fallout => "Fallout",
                // C2C3 has no crater extra/removal-time setting. Keep this
                // Civ III-only terrain state unavailable to the Freeciv
                // clean-time adapter instead of assigning a guessed duration.
                Contamination::Craters => {
                    let requirement = SimulationRequirementContext::extra_requirement("Craters");
                    return Err(SimulationRequirementContext::unsupported(&requirement).into());
                }
            };
            return super::movement_rules::MovementRules::campaign()
                .terrain_extra_removal_time(terrain, extra_name)
                .map_err(|_| {
                    SimulationRequirementContext::unsupported(
                        &SimulationRequirementContext::extra_requirement(extra_name),
                    )
                    .into()
                });
        }
        if let WorkKind::Pillage(target) = kind {
            return super::movement_rules::MovementRules::campaign()
                .terrain_pillage_time(terrain, freeciv_pillage_extra(target))
                .map_err(|_| {
                    SimulationRequirementContext::unsupported(
                        &SimulationRequirementContext::extra_requirement(freeciv_pillage_extra(
                            target,
                        )),
                    )
                    .into()
                });
        }
        if matches!(kind, WorkKind::Cultivate | WorkKind::Plant) {
            let times = super::movement_rules::MovementRules::campaign()
                .terrain_activity_times(terrain)
                .map_err(|_| {
                    SimulationRequirementContext::unsupported(
                        &SimulationRequirementContext::extra_requirement(kind.name()),
                    )
                })?;
            return Ok(if kind == WorkKind::Cultivate {
                times.cultivate
            } else {
                times.plant
            });
        }
        let Some(extra_name) = build_extra_for_work(kind) else {
            let requirement = SimulationRequirementContext::extra_requirement(kind.name());
            return Err(SimulationRequirementContext::unsupported(&requirement).into());
        };
        let requirement = SimulationRequirementContext::extra_requirement(extra_name);
        let extra = RulesetData::campaign_default()
            .rule_records
            .iter()
            .find(|record| {
                record.namespace.eq_ignore_ascii_case("extra")
                    && (record.rule_name.eq_ignore_ascii_case(extra_name)
                        || record
                            .display_name
                            .as_deref()
                            .is_some_and(|display| display.eq_ignore_ascii_case(extra_name)))
            })
            .ok_or_else(|| SimulationRequirementContext::unsupported(&requirement))?;
        let build_time = extra
            .integer_field("build_time")
            .map_err(|_| SimulationRequirementContext::unsupported(&requirement))?
            .unwrap_or(0);
        if build_time > 0 {
            return i32::try_from(build_time)
                .map_err(|_| SimulationRequirementContext::unsupported(&requirement).into());
        }
        let factor = extra
            .integer_field("build_time_factor")
            .map_err(|_| SimulationRequirementContext::unsupported(&requirement))?
            .unwrap_or(1);
        if factor <= 0 {
            return Err(SimulationRequirementContext::unsupported(&requirement).into());
        }
        let times = super::movement_rules::MovementRules::campaign()
            .terrain_activity_times(terrain)
            .map_err(|_| SimulationRequirementContext::unsupported(&requirement))?;
        let terrain_time = match kind {
            WorkKind::Road | WorkKind::Railroad => times.road,
            WorkKind::Irrigation | WorkKind::Farmland => times.irrigation,
            WorkKind::Mine | WorkKind::OilWell | WorkKind::OilPlatform => times.mining,
            WorkKind::BuildBase(_) => times.base,
            WorkKind::Cultivate => times.cultivate,
            WorkKind::Plant => times.plant,
            WorkKind::Pillage(_) | WorkKind::Clean(_) => unreachable!("handled above"),
        };
        let result = i64::from(terrain_time) * factor;
        i32::try_from(result)
            .map_err(|_| SimulationRequirementContext::unsupported(&requirement).into())
    }

    pub(crate) fn work_activity_time_fail_closed(&self, kind: WorkKind, terrain: Terrain) -> i32 {
        match self.work_activity_time(kind, terrain) {
            Ok(time) => time,
            Err(error) => {
                Self::report_action_evaluation_error(Self::freeciv_action_for_work(kind), &error);
                0
            }
        }
    }

    fn action_target_unit(
        &self,
        actor: &Unit,
        action: &str,
        target_xy: (i32, i32),
    ) -> Option<&Unit> {
        match action.to_ascii_lowercase().as_str() {
            "transport embark" | "transport board" => self
                .units
                .iter()
                .find(|unit| (unit.x, unit.y) == target_xy && self.can_board_ship(actor, unit)),
            "transport disembark" | "transport disembark 2" | "transport deboard" => {
                actor.transport.and_then(|carrier| self.unit_by_id(carrier))
            }
            "transport unload" => self
                .units
                .iter()
                .find(|unit| (unit.x, unit.y) == target_xy && unit.transport == Some(actor.id)),
            "unit move" => self
                .units
                .iter()
                .find(|unit| unit.id != actor.id && (unit.x, unit.y) == target_xy),
            _ => None,
        }
    }
}

fn build_extra_for_work(kind: WorkKind) -> Option<&'static str> {
    match kind {
        WorkKind::Road => Some("Road"),
        WorkKind::Railroad => Some("Railroad"),
        WorkKind::Irrigation => Some("Irrigation"),
        WorkKind::Farmland => Some("Farmland"),
        WorkKind::Mine => Some("Mine"),
        WorkKind::OilWell => Some("Oil Well"),
        WorkKind::OilPlatform => Some("Oil Platform"),
        WorkKind::BuildBase(Base::Fort) => Some("Fort"),
        WorkKind::BuildBase(Base::Fortress) => Some("Fortress"),
        WorkKind::BuildBase(Base::Airstrip) => Some("Airstrip"),
        WorkKind::BuildBase(Base::Airbase) => Some("Airbase"),
        WorkKind::Cultivate | WorkKind::Plant | WorkKind::Pillage(_) | WorkKind::Clean(_) => None,
    }
}

/// Apply a selected C2C3 extra as `tile_extra_apply()` does: remove the
/// extras listed as conflicts, then add the requested extra. The campaign
/// tile model treats the unrepresented Buoy extra as absent.
pub(crate) fn apply_freeciv_work_extra(tile: &mut Tile, kind: WorkKind) -> bool {
    let Some(extra_name) = build_extra_for_work(kind) else {
        return false;
    };
    let Some(extra) = RulesetData::campaign_default()
        .rule_records
        .iter()
        .find(|record| {
            record.namespace.eq_ignore_ascii_case("extra")
                && (record.rule_name.eq_ignore_ascii_case(extra_name)
                    || record
                        .display_name
                        .as_deref()
                        .is_some_and(|display| display.eq_ignore_ascii_case(extra_name)))
        })
    else {
        return false;
    };
    let Ok(conflicts) = extra.string_list_field("conflicts") else {
        return false;
    };
    for conflict in conflicts.unwrap_or_default() {
        if !clear_conflicting_tile_extra(tile, &conflict) {
            return false;
        }
    }
    set_work_tile_extra(tile, kind)
}

fn clear_conflicting_tile_extra(tile: &mut Tile, name: &str) -> bool {
    match name.to_ascii_lowercase().as_str() {
        "irrigation" => tile.irrigation = false,
        "farmland" => tile.farmland = false,
        "mine" => tile.mine = false,
        "oil well" => tile.oil_well = false,
        "oil platform" => tile.oil_platform = false,
        "road" => tile.road = false,
        "railroad" => tile.railroad = false,
        "fort" => {
            if tile.base == Some(Base::Fort) {
                tile.base = None;
            }
        }
        "fortress" => {
            if tile.base == Some(Base::Fortress) {
                tile.base = None;
            }
        }
        "airstrip" => {
            if tile.base == Some(Base::Airstrip) {
                tile.base = None;
            }
        }
        "airbase" => {
            if tile.base == Some(Base::Airbase) {
                tile.base = None;
            }
        }
        // CivCraft has no Buoy tile state or creation action. It cannot be
        // present in this runtime, so applying Oil Platform needs no removal.
        "buoy" => {}
        _ => return false,
    }
    true
}

fn set_work_tile_extra(tile: &mut Tile, kind: WorkKind) -> bool {
    match kind {
        WorkKind::Road => tile.road = true,
        WorkKind::Railroad => tile.railroad = true,
        WorkKind::Irrigation => tile.irrigation = true,
        WorkKind::Farmland => tile.farmland = true,
        WorkKind::Mine => tile.mine = true,
        WorkKind::OilWell => tile.oil_well = true,
        WorkKind::OilPlatform => tile.oil_platform = true,
        WorkKind::BuildBase(base) => tile.base = Some(base),
        WorkKind::Cultivate | WorkKind::Plant | WorkKind::Pillage(_) | WorkKind::Clean(_) => {
            return false;
        }
    }
    true
}

fn freeciv_pillage_extra(target: PillageTarget) -> &'static str {
    target.name()
}

/// Direct same-identity Civ III-to-C2C3 unit records used for unit-type
/// universals. Sight-only aliases are deliberately not reused here.
const DIRECT_C2C3_UNIT_RECORDS: &[(&str, &str)] = &[
    ("settler", "unit_settlers"),
    ("worker", "unit_worker"),
    ("explorer", "unit_explorer"),
    ("warrior", "unit_warriors"),
    ("archer", "unit_archers"),
    ("chariot", "unit_chariot"),
    ("catapult", "unit_catapult"),
    ("marine", "unit_marines"),
    ("knight", "unit_knights"),
    ("cannon", "unit_cannon"),
    ("bomber", "unit_bomber"),
    ("fighter", "unit_fighter"),
    ("stealth_fighter", "unit_stealth_fighter"),
    ("stealth_bomber", "unit_stealth_bomber"),
    ("helicopter", "unit_helicopter"),
    ("caravel", "unit_caravel"),
    ("galleon", "unit_galleon"),
    ("transport", "unit_transport"),
    ("carrier", "unit_carrier"),
    ("frigate", "unit_frigate"),
    ("ironclad", "unit_ironclad"),
    ("destroyer", "unit_destroyer"),
    ("cruiser", "unit_cruiser"),
    ("battleship", "unit_battleship"),
    ("submarine", "unit_submarine"),
    ("aegis_cruiser", "unit_aegis_cruiser"),
    ("cruise_missile", "unit_cruise_missile"),
    ("paratrooper", "unit_paratroopers"),
    ("cavalry", "unit_cavalry"),
    ("artillery", "unit_artillery"),
    ("crusader", "unit_crusaders"),
    ("leader", "unit_leader"),
    ("musketeer", "unit_musketeers"),
];

/// Explicit gameplay-role alias for a Civ III Galley, whose C2C3 analogue is
/// the early Trireme carrier. Keep this separate from direct identity matches.
const C2C3_UNIT_RECORD_ALIASES: &[(&str, &str)] = &[("galley", "unit_trireme")];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitEconomicsBindingKind {
    Direct,
    RoleAlias,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum UnitEconomicsRecordBinding {
    Mapped {
        record: &'static crate::rules::freeciv::ContentRecord,
        kind: UnitEconomicsBindingKind,
    },
    Unsupported {
        source_id: &'static str,
        basis: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
struct UnitEconomicsRule {
    civ3_unit_id: &'static str,
    c2c3_record: &'static str,
    kind: Option<UnitEconomicsBindingKind>,
    basis: &'static str,
}

fn unit_economics_rules() -> &'static [UnitEconomicsRule] {
    static RULES: OnceLock<Vec<UnitEconomicsRule>> = OnceLock::new();
    RULES.get_or_init(|| {
        const SOURCE: &str =
            include_str!("../../data/rulesets/civcraft_civ2civ3/unit_economics.tsv");
        let mut rules = Vec::new();
        let mut header_seen = false;
        for (line_index, line) in SOURCE.lines().enumerate() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            if !header_seen {
                assert_eq!(
                    line.split('\t').collect::<Vec<_>>(),
                    ["civ3_unit_id", "c2c3_record", "mapping_kind", "basis"],
                    "invalid selected unit economics mapping header"
                );
                header_seen = true;
                continue;
            }
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(
                fields.len(),
                4,
                "invalid unit economics mapping row {}",
                line_index + 1
            );
            let kind = match fields[2] {
                "direct" => Some(UnitEconomicsBindingKind::Direct),
                "alias" => Some(UnitEconomicsBindingKind::RoleAlias),
                "unsupported" => None,
                other => panic!("unknown unit economics mapping kind {other:?}"),
            };
            if kind.is_none() {
                assert_eq!(fields[1], "-", "unsupported unit mapping has a target");
            } else {
                assert_ne!(fields[1], "-", "mapped unit record is missing");
            }
            rules.push(UnitEconomicsRule {
                civ3_unit_id: fields[0],
                c2c3_record: fields[1],
                kind,
                basis: fields[3],
            });
        }
        assert!(
            header_seen,
            "missing selected unit economics mapping header"
        );
        assert_eq!(
            rules.len(),
            92,
            "Civ III Conquests unit economics map count"
        );
        for (index, rule) in rules.iter().enumerate() {
            assert!(
                !rules[..index].iter().any(|previous| previous
                    .civ3_unit_id
                    .eq_ignore_ascii_case(rule.civ3_unit_id)),
                "duplicate unit economics mapping for {:?}",
                rule.civ3_unit_id
            );
        }
        rules
    })
}

/// Resolve the C2C3 unit record used only for upkeep and supported-unit
/// happiness. Unsupported rows are retained as decisions in the TSV and are
/// returned with their recorded basis rather than falling back to Civ III.
pub(crate) fn freeciv_unit_economics_record(unit: UnitKind) -> UnitEconomicsRecordBinding {
    let Some(source_id) = unit.conquests_id() else {
        return UnitEconomicsRecordBinding::Unsupported {
            source_id: unit.name(),
            basis: "Unit has no Civ III Conquests source identity.",
        };
    };
    let Some(rule) = unit_economics_rules()
        .iter()
        .find(|rule| rule.civ3_unit_id.eq_ignore_ascii_case(source_id))
    else {
        return UnitEconomicsRecordBinding::Unsupported {
            source_id,
            basis: "No row in the selected CivCraft unit economics crosswalk.",
        };
    };
    let Some(kind) = rule.kind else {
        return UnitEconomicsRecordBinding::Unsupported {
            source_id,
            basis: rule.basis,
        };
    };
    let record = RulesetData::campaign_default()
        .content
        .iter()
        .find(|record| {
            record.kind == ContentKind::Unit
                && record.section.eq_ignore_ascii_case(rule.c2c3_record)
        })
        .unwrap_or_else(|| panic!("missing selected C2C3 unit record {:?}", rule.c2c3_record));
    UnitEconomicsRecordBinding::Mapped { record, kind }
}

/// C2C3 records that define transport capacity and cargo compatibility.
/// Galley maps to the early Trireme role; Nuclear Submarine maps to the
/// selected Submarine role because C2C3 has no separate nuclear submarine.
/// This adapter is separate from the direct unit-type-flag table, where these
/// aliases are intentionally not treated as the same identity.
const C2C3_TRANSPORT_UNIT_RECORDS: &[(&str, &str)] = &[
    ("helicopter", "unit_helicopter"),
    ("caravel", "unit_caravel"),
    ("galleon", "unit_galleon"),
    ("submarine", "unit_submarine"),
    ("carrier", "unit_carrier"),
    ("transport", "unit_transport"),
    ("nuclear_submarine", "unit_submarine"),
];

/// Civ III identities that have transport capacity in the source catalog but
/// no C2C3 transport role. They fail closed instead of inheriting Civ III
/// capacity or cargo classes.
const C2C3_TRANSPORT_RECORD_GAPS: &[&str] = &["army", "dromon", "carrack"];

/// Civ III identities without a direct C2C3 unit-record match explicitly use
/// Freeciv's no-type-flag default. Similar names and sight aliases do not
/// establish a gameplay crosswalk. Keep this list exhaustive with the native
/// Conquests roster test below so new units cannot silently inherit flags.
const C2C3_DEFAULT_NO_TYPE_FLAGS: &[&str] = &[
    "horseman",
    "musketman",
    "pikeman",
    "rifleman",
    "tank",
    "mech_infantry",
    "longbowman",
    "infantry",
    "modern_armor",
    "privateer",
    "jet_fighter",
    "nuclear_submarine",
    "icbm",
    "tactical_nuke",
    "scout",
    "guerilla",
    "war_elephant",
    "modern_paratrooper",
    "spearman",
    "swordsman",
    "radar_artillery",
    "army",
    "jaguar_warrior",
    "bowman",
    "hoplite",
    "impi",
    "legionary",
    "immortals",
    "war_chariot",
    "rider",
    "mounted_warrior",
    "samurai",
    "cossack",
    "panzer",
    "man_o_war",
    "f_15",
    "keshik",
    "conquistador",
    "berserk",
    "sipahi",
    "gallic_swordsman",
    "ansar_warrior",
    "numidian_mercenary",
    "hwach_a",
    "medieval_infantry",
    "enkidu_warrior",
    "three_man_chariot",
    "carrack",
    "swiss_mercenary",
    "trebuchet",
    "chasqui_scout",
    "javelin_thrower",
    "dromon",
    "ancient_cavalry",
    "curragh",
    "tow_infantry",
    "flak",
    "mobile_sam",
];

/// Unit-class role mappings for retained Civ III military units without an
/// exact C2C3 unit record. Civ III's SmallLand category includes combat units;
/// Freeciv's Small Land class is reserved for non-occupying units, so these
/// military roles map to the selected C2C3 Land class without borrowing any
/// unit-type flags or stats.
const C2C3_UNIT_CLASS_ALIASES: &[(&str, &str)] = &[
    ("modern_paratrooper", "Land"),
    ("spearman", "Land"),
    ("swordsman", "Land"),
    ("pikeman", "Land"),
    ("longbowman", "Land"),
    ("musketman", "Land"),
    ("rifleman", "Land"),
    ("infantry", "Land"),
    ("jaguar_warrior", "Land"),
    ("bowman", "Land"),
    ("hoplite", "Land"),
    ("impi", "Land"),
    ("legionary", "Land"),
    ("immortals", "Land"),
    ("musketeer", "Land"),
    ("samurai", "Land"),
    ("berserk", "Land"),
    ("gallic_swordsman", "Land"),
    ("numidian_mercenary", "Land"),
    ("medieval_infantry", "Land"),
    ("guerilla", "Land"),
    ("enkidu_warrior", "Land"),
    ("swiss_mercenary", "Land"),
    ("chasqui_scout", "Land"),
    ("javelin_thrower", "Land"),
    ("tow_infantry", "Land"),
];

/// Resolve a C2C3 unit class while retaining the Civ III runtime identity.
/// Direct and role-mapped units use the selected unit record's class; only
/// identities in the explicit no-type-record ledger use the Civ III class
/// crosswalk.
pub(crate) fn freeciv_unit_class_rule_name(unit: UnitKind) -> Option<String> {
    if let Some(record) = direct_c2c3_unit_record(unit) {
        return record
            .string_field("class")
            .ok()
            .flatten()
            .map(str::to_owned);
    }
    let id = unit.conquests_id()?;
    if let Some((_, class)) = C2C3_UNIT_CLASS_ALIASES
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(id))
    {
        return Some((*class).to_owned());
    }
    if C2C3_DEFAULT_NO_TYPE_FLAGS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(id))
    {
        return super::movement_rules::MovementRules::campaign()
            .unit_class_movement_data(unit.unit_class())
            .ok()
            .map(|class| class.rule_name);
    }
    None
}

/// Return the selected C2C3 unit section bound to a Civ III runtime identity.
/// Units on the explicit no-type-record ledger have no selected unit-type
/// combat-bonus table.
pub(crate) fn freeciv_unit_type_section(unit: UnitKind) -> Option<String> {
    direct_c2c3_unit_record(unit).map(|record| record.section.clone())
}

fn direct_c2c3_unit_record(
    unit: UnitKind,
) -> Option<&'static crate::rules::freeciv::ContentRecord> {
    let id = unit.conquests_id()?;
    let section = DIRECT_C2C3_UNIT_RECORDS
        .iter()
        .find(|(candidate, _)| candidate.eq_ignore_ascii_case(id))
        .or_else(|| {
            C2C3_UNIT_RECORD_ALIASES
                .iter()
                .find(|(candidate, _)| candidate.eq_ignore_ascii_case(id))
        })
        .map(|(_, section)| *section)?;
    RulesetData::campaign_default()
        .content
        .iter()
        .find(|record| {
            record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case(section)
        })
}

/// Resolve a C2C3 per-unit flag through explicit identity bindings. Returns
/// `Some(false)` for a roster ID in the audited no-type-flag ledger and
/// `None` when the ID or selected unit record has no declared binding.
pub(crate) fn freeciv_unit_type_flag(unit: UnitKind, flag: &str) -> Option<bool> {
    if flag.eq_ignore_ascii_case("Nuclear") && unit.is_nuclear_weapon() {
        let rules = RulesetData::campaign_default();
        let record = rules.content.iter().find(|record| {
            record.kind == ContentKind::Unit && record.section.eq_ignore_ascii_case("unit_nuclear")
        })?;
        let flags = record.string_list_field("flags").ok().flatten()?;
        return Some(
            flags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case("Nuclear")),
        );
    }
    let conquests_id = unit.conquests_id()?;
    if let Some(record) = direct_c2c3_unit_record(unit) {
        let flags = record.string_list_field("flags").ok().flatten()?;
        Some(
            flags
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(flag)),
        )
    } else if C2C3_DEFAULT_NO_TYPE_FLAGS
        .iter()
        .any(|id| id.eq_ignore_ascii_case(conquests_id))
    {
        Some(false)
    } else {
        None
    }
}

/// Resolve whether a selected C2C3 unit record can attack a defender's class.
/// Freeciv targets every reachable unit class by default and only requires an
/// explicit `targets` entry for classes marked `Unreachable`.
pub(crate) fn freeciv_unit_can_target_class(
    attacker: UnitKind,
    defender: UnitKind,
) -> Option<bool> {
    // Requiring both mappings keeps Civ III catalog identities explicit. Units
    // without a direct type record use their already-audited C2C3 class alias
    // and the engine's default target behavior.
    freeciv_unit_class_rule_name(attacker)?;
    let target_class = freeciv_unit_class_rule_name(defender)?;
    let unreachable = freeciv_unit_class_flag(defender, "Unreachable")?;
    let explicitly_targeted = if let Some(record) = direct_c2c3_unit_record(attacker) {
        record
            .string_list_field("targets")
            .ok()?
            .unwrap_or_default()
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&target_class))
    } else {
        false
    };
    Some(explicitly_targeted || !unreachable)
}

/// Resolve one selected C2C3 class flag through the Civ III unit-class
/// crosswalk. Callers receive `None` when the runtime identity has no explicit
/// class mapping or the selected ruleset omits the mapped record.
pub(crate) fn freeciv_unit_class_flag(unit: UnitKind, flag: &str) -> Option<bool> {
    let class_name = freeciv_unit_class_rule_name(unit)?;
    let rules = RulesetData::campaign_default();
    let class = rules.rule_records.iter().find(|record| {
        record.namespace.eq_ignore_ascii_case("unit_class")
            && record.rule_name.eq_ignore_ascii_case(&class_name)
    })?;
    let flags = class.string_list_field("flags").ok()??;
    Some(
        flags
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(flag)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapgen;
    use crate::rules::freeciv::ActionEvaluationError;
    use crate::world::Terrain;

    fn flat_game() -> Simulation {
        let mut game = Simulation::new(mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.river = false;
            tile.resource = None;
            tile.owner = None;
        }
        game
    }

    #[test]
    fn citytile_claimed_uses_tile_ownership_and_supports_negation() {
        let mut game = flat_game();
        let position = (10, 20);
        let requirement = |present| Requirement {
            kind: "CityTile".to_owned(),
            name: RequirementValue::String("Claimed".to_owned()),
            range: RequirementRange::Tile,
            present,
            survives: false,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: std::path::PathBuf::new(),
                section: "fixture".to_owned(),
                key: "target_reqs".to_owned(),
                line: 1,
            },
        };

        let unclaimed =
            SimulationRequirementContext::for_player_at(&game, crate::sim::HUMAN, Some(position));
        assert!(!unclaimed.is_present(&requirement(true)).unwrap());
        assert!(requirements_met(&[requirement(false)], &unclaimed).unwrap());

        game.tile_mut(position.0, position.1).owner = Some(crate::sim::HUMAN);
        let claimed =
            SimulationRequirementContext::for_player_at(&game, crate::sim::HUMAN, Some(position));
        assert!(claimed.is_present(&requirement(true)).unwrap());
        assert!(!requirements_met(&[requirement(false)], &claimed).unwrap());
    }

    #[test]
    fn action_enabler_requires_both_worker_actor_and_buildable_target() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        let warrior = game.spawn_unit(crate::sim::HUMAN, UnitKind::Warrior, x, y);

        assert!(game.action_available(worker, "Build Road", (x, y)).unwrap());
        assert!(
            !game
                .action_available(warrior, "Build Road", (x, y))
                .unwrap()
        );

        game.tile_mut(x, y).terrain = Terrain::Lake;
        assert!(!game.action_available(worker, "Build Road", (x, y)).unwrap());
    }

    #[test]
    fn irrigation_enablers_are_alternatives_but_keep_each_branch_conjunctive() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        game.players[crate::sim::HUMAN]
            .known
            .push(Tech::ELECTRICITY);
        assert!(
            game.action_available(worker, "Build Irrigation", (x, y))
                .unwrap()
        );

        game.players[crate::sim::HUMAN]
            .known
            .retain(|tech| *tech != Tech::ELECTRICITY);
        let water = game.map_wrap.civ3_neighbor(x, y, 0).unwrap();
        game.tile_mut(water.0, water.1).terrain = Terrain::Sea;
        assert!(
            game.action_available(worker, "Build Irrigation", (x, y))
                .unwrap()
        );

        game.tile_mut(water.0, water.1).terrain = Terrain::Grassland;
        assert!(
            !game
                .action_available(worker, "Build Irrigation", (x, y))
                .unwrap()
        );
    }

    #[test]
    fn fortify_enabler_checks_class_flag_type_flag_activity_and_tile_flag() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let warrior = game.spawn_unit(crate::sim::HUMAN, UnitKind::Warrior, x, y);
        assert!(game.action_available(warrior, "Fortify", (x, y)).unwrap());

        game.units[0].state = UnitState::Fortified;
        assert!(!game.action_available(warrior, "Fortify", (x, y)).unwrap());
        game.units[0].state = UnitState::Idle;
        game.tile_mut(x, y).terrain = Terrain::Sea;
        assert!(!game.action_available(warrior, "Fortify", (x, y)).unwrap());
    }

    #[test]
    fn movement_enabler_checks_move_fragments_and_transport_state() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        game.units[0].moves_left = 1;
        assert!(
            game.action_available(worker, "Unit Move", (x + 1, y))
                .unwrap()
        );
        game.units[0].moves_left = 0;
        assert!(
            !game
                .action_available(worker, "Unit Move", (x + 1, y))
                .unwrap()
        );
        game.units[0].moves_left = 1;
        game.units[0].transport = Some(999);
        assert!(
            !game
                .action_available(worker, "Unit Move", (x + 1, y))
                .unwrap()
        );
    }

    #[test]
    fn transport_enablers_use_target_relationship_and_destination_nativity() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let cargo = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        let ship = game.spawn_unit(crate::sim::HUMAN, UnitKind::Transport, x + 1, y);
        game.units[0].moves_left = 1;

        assert!(
            game.action_available(cargo, "Transport Embark", (x + 1, y))
                .unwrap()
        );

        game.units[0].transport = Some(ship);
        assert!(
            game.action_available(cargo, "Transport Disembark", (x, y))
                .unwrap()
        );
        game.tile_mut(x, y).terrain = Terrain::Sea;
        assert!(
            !game
                .action_available(cargo, "Transport Disembark", (x, y))
                .unwrap()
        );
    }

    #[test]
    fn disembark_movement_selects_local_variant_at_destination_tile() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Sea;
        game.tile_mut(x + 1, y).terrain = Terrain::Grassland;
        let cargo = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        let ship = game.spawn_unit(crate::sim::HUMAN, UnitKind::Transport, x, y);
        game.units[0].moves_left = 1;
        game.units[0].transport = Some(ship);

        let cargo_unit = game.unit_by_id(cargo).unwrap().clone();
        // The generic action API evaluates Local against the unit's current
        // tile. Movement's disembark path instead evaluates both alternatives
        // after the carrier and cargo have arrived at the destination.
        assert!(
            !game
                .action_available(cargo, "Transport Disembark", (x + 1, y))
                .unwrap()
        );
        assert_eq!(
            game.transport_disembark_action_for_unit(&cargo_unit, (x + 1, y))
                .unwrap(),
            Some("Transport Disembark")
        );

        game.tile_mut(x + 1, y).terrain = Terrain::Sea;
        assert_eq!(
            game.transport_disembark_action_for_unit(&cargo_unit, (x + 1, y))
                .unwrap(),
            Some("Transport Disembark 2")
        );
    }

    #[test]
    fn work_order_uses_extra_requirements_after_action_enabler() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        game.tile_mut(x, y).irrigation = true;
        game.players[crate::sim::HUMAN]
            .known
            .push(Tech::REFRIGERATION);
        assert!(
            game.work_action_available(worker, WorkKind::Farmland, (x, y))
                .unwrap()
        );

        game.players[crate::sim::HUMAN]
            .known
            .retain(|tech| *tech != Tech::REFRIGERATION);
        assert!(
            !game
                .work_action_available(worker, WorkKind::Farmland, (x, y))
                .unwrap()
        );
    }

    #[test]
    fn extra_conflicts_and_bridged_over_rules_control_build_orders() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        game.tile_mut(x, y).river = true;

        assert!(
            !game
                .work_action_available(worker, WorkKind::Road, (x, y))
                .unwrap()
        );
        game.players[crate::sim::HUMAN]
            .known
            .push(Tech::ENGINEERING);
        assert!(
            game.work_action_available(worker, WorkKind::Road, (x, y))
                .unwrap()
        );
        game.players[crate::sim::HUMAN]
            .known
            .retain(|tech| *tech != Tech::ENGINEERING);
        game.players[crate::sim::HUMAN]
            .known
            .push(Tech::BRIDGE_BUILDING);
        assert!(
            game.work_action_available(worker, WorkKind::Road, (x, y))
                .unwrap()
        );

        game.tile_mut(x, y).river = false;
        game.tile_mut(x, y).mine = true;
        game.players[crate::sim::HUMAN]
            .known
            .push(Tech::ELECTRICITY);
        assert!(
            game.work_action_available(worker, WorkKind::Irrigation, (x, y))
                .unwrap()
        );
        let mut projected = game.tile(x, y).clone();
        assert!(apply_freeciv_work_extra(
            &mut projected,
            WorkKind::Irrigation
        ));
        assert!(projected.irrigation);
        assert!(!projected.mine);
    }

    #[test]
    fn oil_platform_uses_ruleset_enabler_without_a_civ3_resource_gate() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        game.tile_mut(x, y).terrain = Terrain::Ocean;
        game.tile_mut(x, y).resource = None;
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);
        game.players[crate::sim::HUMAN]
            .known
            .push(Tech::MINIATURIZATION);

        assert!(
            game.work_action_available(worker, WorkKind::OilPlatform, (x, y))
                .unwrap()
        );
        assert_eq!(
            game.work_activity_time(WorkKind::OilPlatform, Terrain::Ocean)
                .unwrap(),
            10
        );
    }

    #[test]
    fn player_building_and_nuclear_unit_requirements_resolve_from_runtime_identity() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let mut city = super::super::City::new(1, crate::sim::HUMAN, x, y, "Test".to_owned(), 0);
        city.buildings.push(Building::LIGHTHOUSE);
        game.cities.push(city);
        let icbm = game.spawn_unit(crate::sim::HUMAN, UnitKind::ICBM, x, y);
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, x, y);

        let requirement = |kind: &str, name: &str, range| Requirement {
            kind: kind.to_owned(),
            name: RequirementValue::String(name.to_owned()),
            range,
            present: true,
            survives: false,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: "effects.ruleset".into(),
                section: "test".to_owned(),
                key: "reqs".to_owned(),
                line: 1,
            },
        };
        let player_context =
            SimulationRequirementContext::for_player_at(&game, crate::sim::HUMAN, Some((x, y)));
        assert!(
            player_context
                .is_present(&requirement(
                    "Building",
                    "Lighthouse",
                    RequirementRange::Player
                ))
                .unwrap()
        );
        assert!(
            !player_context
                .is_present(&requirement(
                    "Building",
                    "Magellan's Expedition",
                    RequirementRange::Player,
                ))
                .unwrap()
        );

        let icbm_context =
            SimulationRequirementContext::for_unit(&game, game.unit_by_id(icbm).unwrap());
        let nuclear = requirement("UnitType", "Nuclear", RequirementRange::Local);
        assert!(icbm_context.is_present(&nuclear).unwrap());
        let worker_context =
            SimulationRequirementContext::for_unit(&game, game.unit_by_id(worker).unwrap());
        assert!(!worker_context.is_present(&nuclear).unwrap());
        assert!(matches!(
            worker_context.is_present(&requirement(
                "UnitType",
                "Cruise Missile",
                RequirementRange::Local,
            )),
            Err(RequirementEvaluationError::Unsupported { .. })
        ));
    }

    #[test]
    fn local_team_relation_uses_the_campaigns_single_player_team_mapping() {
        let game = flat_game();
        let context = || {
            SimulationRequirementContext::for_player_at(&game, crate::sim::HUMAN, Some((10, 20)))
        };
        let requirement = |name: &str| Requirement {
            kind: "DiplRel".to_owned(),
            name: RequirementValue::String(name.to_owned()),
            range: RequirementRange::Local,
            present: true,
            survives: false,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: "effects.ruleset".into(),
                section: "test_team_relation".to_owned(),
                key: "reqs".to_owned(),
                line: 1,
            },
        };

        assert!(
            context()
                .with_relation_target(Some(crate::sim::HUMAN))
                .is_present(&requirement("Team"))
                .unwrap()
        );
        assert!(
            !context()
                .with_relation_target(Some(1))
                .is_present(&requirement("Team"))
                .unwrap()
        );
        assert!(
            context()
                .with_relation_target(Some(1))
                .is_present(&requirement("Foreign"))
                .unwrap()
        );
    }

    #[test]
    fn shared_effect_evaluator_resolves_sdi_nuke_proof_target_and_actor_contexts() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let mut city = super::super::City::new(1, 1, x, y, "SDI City".to_owned(), 0);
        city.buildings.push(Building::SDI_DEFENSE);
        game.cities.push(city);
        let rules = RulesetData::campaign_default();

        let city_context = SimulationRequirementContext::for_city_at(
            &game,
            game.cities.last().unwrap(),
            Some((x, y)),
        )
        .with_relation_target(Some(crate::sim::HUMAN));
        assert_eq!(
            effect_value(&rules.effects, "Nuke_Proof", &city_context).unwrap(),
            100
        );

        let same_player_context = SimulationRequirementContext::for_city_at(
            &game,
            game.cities.last().unwrap(),
            Some((x, y)),
        )
        .with_relation_target(Some(1));
        assert_eq!(
            effect_value(&rules.effects, "Nuke_Proof", &same_player_context).unwrap(),
            0
        );

        game.cities.last_mut().unwrap().buildings.clear();
        let no_sdi_context = SimulationRequirementContext::for_city_at(
            &game,
            game.cities.last().unwrap(),
            Some((x, y)),
        )
        .with_relation_target(Some(crate::sim::HUMAN));
        assert_eq!(
            effect_value(&rules.effects, "Nuke_Proof", &no_sdi_context).unwrap(),
            0
        );
    }

    #[test]
    fn borrowed_target_action_context_uses_specific_enemy_on_an_unowned_tile() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let target_xy = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();
        game.sign_pact(crate::sim::HUMAN, 1, Pact::War);
        game.tile_mut(target_xy.0, target_xy.1).owner = None;
        let actor_id = game.spawn_unit(crate::sim::HUMAN, UnitKind::Bomber, x, y);
        let target_id = game.spawn_unit(1, UnitKind::Warrior, target_xy.0, target_xy.1);
        let actor = game.unit_by_id(actor_id).unwrap().clone();
        let target = game.unit_by_id(target_id).unwrap().clone();

        assert!(
            game.action_available_for_unit_against_unit(&actor, "Bombard", &target)
                .unwrap()
        );
        assert!(
            !game
                .action_available_for_unit(&actor, "Bombard", target_xy)
                .unwrap()
        );
    }

    #[test]
    fn retained_nuclear_units_map_the_selected_nuclear_flag_explicitly() {
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::ICBM, "Nuclear"),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::TacticalNuke, "Nuclear"),
            Some(true)
        );
        assert_ne!(
            freeciv_unit_type_flag(UnitKind::CruiseMissile, "Nuclear"),
            Some(true)
        );
    }

    #[test]
    fn borrowed_city_action_context_uses_city_owner_when_tile_owner_is_unset() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        let target_xy = game.map_wrap.civ3_offset(x, y, 2, 0).unwrap();
        game.sign_pact(crate::sim::HUMAN, 1, Pact::War);
        let actor_id = game.spawn_unit(crate::sim::HUMAN, UnitKind::ICBM, x, y);
        game.create_city(1, target_xy.0, target_xy.1);
        game.tile_mut(target_xy.0, target_xy.1).owner = None;
        let actor = game.unit_by_id(actor_id).unwrap().clone();
        let city = game.city_at(target_xy.0, target_xy.1).unwrap();

        assert!(
            !game
                .action_available_for_unit(&actor, "Nuke City", target_xy)
                .unwrap()
        );
        assert!(
            game.action_available_for_unit_against_city(&actor, "Nuke City", city)
                .unwrap()
        );
    }

    #[test]
    fn world_tech_requirement_uses_selected_catalog_binding_and_living_players() {
        let mut game = flat_game();
        game.players[crate::sim::HUMAN].known.clear();
        game.players[1].known.clear();
        game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, 10, 20);
        game.spawn_unit(1, UnitKind::Warrior, 12, 20);
        game.players[1].ai = false;
        // C2C3 calls this advance "Superconductors"; the Civ III catalog
        // identity is singular, so resolution must go through the selected
        // Freeciv content record and its explicit source binding.
        let tech = Tech::SUPERCONDUCTORS;
        game.global_advances.retain(|&known| known != tech);
        game.discover(1, tech);

        let requirement = |range, survives| Requirement {
            kind: "Tech".to_owned(),
            name: RequirementValue::String("Superconductors".to_owned()),
            range,
            present: true,
            survives,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: "effects.ruleset".into(),
                section: "test_world_tech".to_owned(),
                key: "reqs".to_owned(),
                line: 1,
            },
        };

        let player_context =
            SimulationRequirementContext::for_player_at(&game, crate::sim::HUMAN, None);
        assert!(
            !player_context
                .is_present(&requirement(RequirementRange::Player, false))
                .unwrap()
        );
        assert!(
            player_context
                .is_present(&requirement(RequirementRange::World, false))
                .unwrap()
        );
        assert!(
            player_context
                .is_present(&requirement(RequirementRange::World, true))
                .unwrap()
        );

        // Force one actual tech loss while the rival remains alive. Ordinary
        // World scope stops matching, while survives=TRUE stays in global history.
        game.players[1].researching = Tech::ALPHABET;
        game.players[1].research = 0;
        game.apply_research_upkeep(1, -10_000);
        assert!(!game.has_tech(1, tech));
        assert!(game.global_advances.contains(&tech));

        let player_context =
            SimulationRequirementContext::for_player_at(&game, crate::sim::HUMAN, None);
        assert!(
            !player_context
                .is_present(&requirement(RequirementRange::World, false))
                .unwrap()
        );
        assert!(
            player_context
                .is_present(&requirement(RequirementRange::World, true))
                .unwrap()
        );
    }

    #[test]
    fn unsupported_requirement_is_reported_instead_of_becoming_false() {
        let mut game = flat_game();
        let worker_id = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, 10, 20);
        let worker = game.unit_by_id(worker_id).unwrap();
        let context = SimulationRequirementContext::for_unit(&game, worker);
        let requirement = Requirement {
            kind: "MadeUpUniversal".to_owned(),
            name: RequirementValue::String("Something".to_owned()),
            range: RequirementRange::Local,
            present: true,
            survives: false,
            quiet: false,
            source: crate::rules::freeciv::RuleSourceLocation {
                file: "actions.ruleset".into(),
                section: "test".to_owned(),
                key: "actor_reqs".to_owned(),
                line: 1,
            },
        };
        assert!(matches!(
            context.is_present(&requirement),
            Err(RequirementEvaluationError::Unsupported { .. })
        ));
        assert!(!matches!(
            game.action_available(worker_id, "Not an action", (10, 20)),
            Ok(_)
        ));
        assert!(matches!(
            game.action_available(worker_id, "Not an action", (10, 20)),
            Err(ActionEvaluationError::UnknownAction(_))
        ));
    }

    #[test]
    fn freeciv_unit_type_flags_use_direct_records_and_explicit_default_ledger() {
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Worker, "Workers"),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Worker, "HasNoZOC"),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Worker, "IgZOC"),
            Some(false)
        );
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Settler, "Workers"),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Warrior, "HasNoZOC"),
            Some(false)
        );
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Galley, "HasNoZOC"),
            Some(true)
        );
        // The pinned C2C3 Trireme record has no CoastStrict flag; the Galley
        // alias still reads that selected record and does not infer the flag
        // from Civ III's coastal movement behavior.
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Galley, "CoastStrict"),
            Some(false)
        );
        // Similar names are not an explicit unit identity match. Civ III's
        // source flags do not leak into this Freeciv default.
        assert_eq!(
            freeciv_unit_type_flag(UnitKind::Guerilla, "IgZOC"),
            Some(false)
        );

        let all_ids = UnitKind::ALL_NATIVE
            .into_iter()
            .map(|kind| kind.conquests_id().expect("native source unit ID"))
            .collect::<Vec<_>>();
        for id in &all_ids {
            let direct = DIRECT_C2C3_UNIT_RECORDS
                .iter()
                .any(|(mapped, _)| mapped.eq_ignore_ascii_case(id));
            let alias = C2C3_UNIT_RECORD_ALIASES
                .iter()
                .any(|(mapped, _)| mapped.eq_ignore_ascii_case(id));
            let fallback = C2C3_DEFAULT_NO_TYPE_FLAGS
                .iter()
                .any(|mapped| mapped.eq_ignore_ascii_case(id));
            assert_eq!(
                usize::from(direct) + usize::from(alias) + usize::from(fallback),
                1,
                "unit ID {id} must have one flag binding"
            );
        }
        for (id, _) in DIRECT_C2C3_UNIT_RECORDS {
            let unit = UnitKind::ALL_NATIVE
                .into_iter()
                .find(|kind| kind.conquests_id() == Some(*id))
                .expect("direct identity belongs to Civ III roster");
            assert!(
                freeciv_unit_type_flag(unit, "HasNoZOC").is_some(),
                "direct C2C3 unit record resolves for {id}"
            );
        }
        for (id, _) in C2C3_UNIT_RECORD_ALIASES {
            let unit = UnitKind::ALL_NATIVE
                .into_iter()
                .find(|kind| kind.conquests_id() == Some(*id))
                .expect("aliased source unit ID belongs to Civ III roster");
            assert!(
                freeciv_unit_type_flag(unit, "HasNoZOC").is_some(),
                "C2C3 role alias resolves its unit record for {id}"
            );
        }
        for id in C2C3_DEFAULT_NO_TYPE_FLAGS {
            let unit = UnitKind::ALL_NATIVE
                .into_iter()
                .find(|kind| kind.conquests_id() == Some(*id))
                .expect("default identity belongs to Civ III roster");
            assert_eq!(
                freeciv_unit_type_flag(unit, "HasNoZOC"),
                Some(false),
                "{id}"
            );
            assert_eq!(freeciv_unit_type_flag(unit, "IgZOC"), Some(false), "{id}");
        }
        for (id, class) in C2C3_UNIT_CLASS_ALIASES {
            let unit = UnitKind::ALL_NATIVE
                .into_iter()
                .find(|kind| kind.conquests_id() == Some(*id))
                .expect("role-mapped source identity belongs to Civ III roster");
            assert_eq!(
                freeciv_unit_class_rule_name(unit).as_deref(),
                Some(*class),
                "{id}"
            );
        }
        assert_eq!(
            all_ids.len(),
            DIRECT_C2C3_UNIT_RECORDS.len()
                + C2C3_UNIT_RECORD_ALIASES.len()
                + C2C3_DEFAULT_NO_TYPE_FLAGS.len()
        );
    }

    #[test]
    fn combat_target_classes_follow_selected_c2c3_rules() {
        assert_eq!(
            freeciv_unit_can_target_class(UnitKind::Warrior, UnitKind::Worker),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_can_target_class(UnitKind::Warrior, UnitKind::Fighter),
            Some(false)
        );
        assert_eq!(
            freeciv_unit_can_target_class(UnitKind::Fighter, UnitKind::Fighter),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_can_target_class(UnitKind::Fighter, UnitKind::Warrior),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_class_flag(UnitKind::Fighter, "Unreachable"),
            Some(true)
        );
        assert_eq!(
            freeciv_unit_class_flag(UnitKind::Warrior, "AttackNonNative"),
            Some(false)
        );
    }

    #[test]
    fn projected_railroad_checks_the_ruleset_extra_requirement() {
        let mut game = flat_game();
        let position = (10, 20);
        let worker_id =
            game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, position.0, position.1);
        let worker = game.unit_by_id(worker_id).unwrap().clone();
        game.players[crate::sim::HUMAN].known.push(Tech::RAILROAD);

        assert!(
            !game
                .work_action_available_for_unit(&worker, WorkKind::Railroad, position)
                .unwrap()
        );
        let mut projected_road = game.tile(position.0, position.1).clone();
        projected_road.road = true;
        assert!(
            game.work_action_available_for_unit_with_tile(
                &worker,
                WorkKind::Railroad,
                position,
                Some(&projected_road),
            )
            .unwrap()
        );

        game.players[crate::sim::HUMAN]
            .known
            .retain(|tech| *tech != Tech::RAILROAD);
        assert!(
            !game
                .work_action_available_for_unit_with_tile(
                    &worker,
                    WorkKind::Railroad,
                    position,
                    Some(&projected_road),
                )
                .unwrap()
        );
    }

    #[test]
    fn transport_capacities_and_cargo_classes_come_from_c2c3_records() {
        let game = flat_game();
        assert_eq!(
            game.ruleset_transport_capacity(UnitKind::Transport)
                .unwrap(),
            8
        );
        assert!(
            game.ruleset_transport_has_space(UnitKind::Transport, 7)
                .unwrap()
        );
        assert!(
            !game
                .ruleset_transport_has_space(UnitKind::Transport, 8)
                .unwrap()
        );
        assert!(
            game.ruleset_transport_accepts(UnitKind::Galleon, UnitKind::Cannon)
                .unwrap()
        );
        assert!(
            !game
                .ruleset_transport_accepts(UnitKind::Caravel, UnitKind::Cannon)
                .unwrap()
        );
        assert!(
            game.ruleset_transport_pair_allowed(UnitKind::Carrier, UnitKind::Helicopter)
                .unwrap()
        );
        assert!(
            game.ruleset_unit_can_embark(UnitKind::Marine, UnitKind::Helicopter)
                .unwrap()
        );
        assert!(
            game.ruleset_unit_can_disembark(UnitKind::Marine, UnitKind::Helicopter)
                .unwrap()
        );
        assert!(
            !game
                .ruleset_unit_can_embark(UnitKind::Guerilla, UnitKind::Helicopter)
                .unwrap()
        );
        assert_eq!(
            game.ruleset_transport_capacity(UnitKind::NuclearSubmarine)
                .unwrap(),
            8
        );
        assert!(
            game.ruleset_transport_pair_allowed(UnitKind::NuclearSubmarine, UnitKind::TacticalNuke)
                .unwrap()
        );

        let transport_gaps: Vec<_> = UnitKind::ALL_NATIVE
            .iter()
            .copied()
            .filter(|kind| {
                kind.conquests_id().is_some_and(|id| {
                    C2C3_TRANSPORT_RECORD_GAPS
                        .iter()
                        .any(|gap| gap.eq_ignore_ascii_case(id))
                })
            })
            .collect();
        assert_eq!(transport_gaps.len(), C2C3_TRANSPORT_RECORD_GAPS.len());
        for gap in transport_gaps {
            // Expected catalog gaps are ordinary unavailable cases in the
            // runtime wrappers, while direct accessors above preserve the
            // explicit Unsupported result for diagnostics.
            assert!(!game.ruleset_transport_has_space_fail_closed(gap, 0));
            assert!(!game.ruleset_transport_pair_allowed_fail_closed(gap, UnitKind::Warrior));
            assert!(!game.ruleset_transport_pair_allowed_fail_closed(UnitKind::Transport, gap));
        }
    }

    #[test]
    fn upkeep_actions_use_mapped_unit_flags_and_target_city_production() {
        let mut game = flat_game();
        let (x, y) = (10, 20);
        game.create_city(crate::sim::HUMAN, x, y);
        game.cities[0].production = super::super::Production::Building(Building::PYRAMIDS);
        let caravan_record = RulesetData::campaign_default()
            .content
            .iter()
            .find(|record| {
                record.kind == ContentKind::Unit
                    && record.section.eq_ignore_ascii_case("unit_caravan")
            })
            .unwrap();
        let warrior_id = game.spawn_unit(crate::sim::HUMAN, UnitKind::Warrior, x, y);
        let warrior = game.unit_by_id(warrior_id).unwrap().clone();

        assert!(
            game.action_available_for_unit_with_record(
                &warrior,
                "Help Wonder",
                (x, y),
                caravan_record,
                Some(&game.cities[0]),
            )
            .unwrap()
        );
        assert!(
            game.action_available_for_unit_with_record(
                &warrior,
                "Disband Unit Recover",
                (x, y),
                caravan_record,
                Some(&game.cities[0]),
            )
            .unwrap()
        );
        assert!(
            game.action_available_for_unit_with_record(
                &warrior,
                "Disband Unit",
                (x, y),
                caravan_record,
                None,
            )
            .unwrap()
        );

        game.cities[0].production = super::super::Production::Building(Building::TEMPLE);
        assert!(
            !game
                .action_available_for_unit_with_record(
                    &warrior,
                    "Help Wonder",
                    (x, y),
                    caravan_record,
                    Some(&game.cities[0]),
                )
                .unwrap()
        );
    }

    #[test]
    fn action_requirement_context_applies_recycling_shield_value_effect() {
        let mut game = flat_game();
        let id = game.spawn_unit(crate::sim::HUMAN, UnitKind::Warrior, 10, 20);
        let unit = game.unit_by_id(id).unwrap();
        let context =
            SimulationRequirementContext::for_unit(&game, unit).with_action("Disband Unit Recover");
        assert_eq!(
            crate::rules::freeciv::effect_value(
                &RulesetData::campaign_default().effects,
                "Unit_Shield_Value_Pct",
                &context,
            )
            .unwrap(),
            -50
        );
    }

    #[test]
    fn transport_cycle_detection_checks_the_existing_parent_chain() {
        let mut game = flat_game();
        let passenger = game.spawn_unit(crate::sim::HUMAN, UnitKind::Transport, 10, 20);
        let carrier = game.spawn_unit(crate::sim::HUMAN, UnitKind::Transport, 10, 20);
        game.units
            .iter_mut()
            .find(|unit| unit.id == carrier)
            .unwrap()
            .transport = Some(passenger);

        assert!(game.transport_would_create_cycle(passenger, carrier));
        assert!(!game.transport_would_create_cycle(carrier, passenger));
    }

    #[test]
    fn clean_and_pillage_durations_use_selected_c2c3_terrain_data() {
        let game = flat_game();
        assert_eq!(
            game.work_activity_time(WorkKind::Clean(Contamination::Pollution), Terrain::Desert)
                .unwrap(),
            3
        );
        assert_eq!(
            game.work_activity_time(WorkKind::Pillage(PillageTarget::Road), Terrain::Grassland,)
                .unwrap(),
            1
        );
        assert!(matches!(
            game.work_activity_time(WorkKind::Clean(Contamination::Craters), Terrain::Grassland),
            Err(ActionEvaluationError::Requirement(
                crate::rules::freeciv::RequirementEvaluationError::Unsupported { .. }
            ))
        ));
    }

    #[test]
    fn civ_three_crater_cleaning_is_unavailable_without_a_c2c3_extra() {
        let mut game = flat_game();
        let worker = game.spawn_unit(crate::sim::HUMAN, UnitKind::Worker, 10, 20);
        game.tile_mut(10, 20).craters = true;

        assert!(!game.work_action_available_fail_closed(
            worker,
            WorkKind::Clean(Contamination::Craters),
            (10, 20)
        ));
    }

    #[test]
    fn civ_three_terrain_crosswalk_keeps_shallow_and_deep_water_distinct() {
        let rules = super::super::movement_rules::MovementRules::campaign();
        assert_eq!(
            rules.terrain_data(Terrain::Coast).unwrap().rule_name,
            "Lake"
        );
        assert_eq!(rules.terrain_data(Terrain::Sea).unwrap().rule_name, "Ocean");
        assert_eq!(
            rules.terrain_data(Terrain::Ocean).unwrap().rule_name,
            "Deep Ocean"
        );
        assert!(rules.terrain_data(Terrain::Volcano).is_err());
    }
}
