//! Small typed helpers for constructing deterministic campaign fixtures.

use crate::{
    rules::{Building, Genus, Tech, civilizations},
    sim::{Base, Government, MAX_CIVILIZATIONS, Pact, Simulation, UnitKind},
    world::{GeneratedMap, GeneratedTile, Resource, Terrain, TerrainBase},
};

mod readiness;
pub(crate) use readiness::UnitReadiness;

/// The stable catalog ID and starting knowledge for a civilization slot.
pub(crate) struct CivPreset<'a> {
    pub id: &'a str,
    /// None uses the catalog defaults. Some(&[]) starts with no technologies.
    pub technologies: Option<&'a [Tech]>,
}

/// Terrain and infrastructure for one tile in the starting map.
pub(crate) struct TilePreset {
    pub terrain: Terrain,
    pub base_terrain: TerrainBase,
    pub resource: Option<Resource>,
    pub river: bool,
    pub road: bool,
    pub railroad: bool,
    pub base: Option<Base>,
}

impl TilePreset {
    pub(crate) fn new(terrain: Terrain, base_terrain: TerrainBase) -> Self {
        Self {
            terrain,
            base_terrain,
            resource: None,
            river: false,
            road: false,
            railroad: false,
            base: None,
        }
    }
}

pub(crate) struct WorldBuilder<'a> {
    game: &'a mut Simulation,
    river_edges_dirty: bool,
}

impl<'a> WorldBuilder<'a> {
    pub(crate) fn new(game: &'a mut Simulation) -> Self {
        Self {
            game,
            river_edges_dirty: false,
        }
    }

    /// Keep the seed and topology. Active slots match the requested order.
    pub(crate) fn ocean_campaign(mut map: GeneratedMap, civs: &[CivPreset<'_>]) -> Simulation {
        assert!(
            (1..=MAX_CIVILIZATIONS).contains(&civs.len()),
            "invalid scenario roster size"
        );
        let mut roster: Vec<(&civilizations::Civilization, &[Tech])> = Vec::new();
        for preset in civs {
            let civ = civilizations::by_id(preset.id)
                .unwrap_or_else(|| panic!("unknown scenario civilization {}", preset.id));
            assert!(
                !roster.iter().any(|(entry, _)| entry.id == civ.id),
                "duplicate scenario civilization {}",
                civ.id
            );
            let techs = preset.technologies.unwrap_or(&civ.starting_techs);
            for (index, tech) in techs.iter().enumerate() {
                assert!(
                    Tech::all().any(|candidate| candidate == *tech),
                    "invalid scenario technology {tech:?}"
                );
                assert!(
                    !techs[..index].contains(tech),
                    "duplicate scenario technology {tech:?}"
                );
            }
            roster.push((civ, techs));
        }
        // Keep inactive slots for the fixed barbarian slot and save layout.
        // Their empty knowledge keeps them out of global technology history.
        for civ in civilizations::all() {
            if roster.len() == MAX_CIVILIZATIONS {
                break;
            }
            if !roster.iter().any(|(entry, _)| entry.id == civ.id) {
                roster.push((civ, &[]));
            }
        }
        map.civilization_count = civs.len();
        map.starts.clear();
        map.tiles.fill(GeneratedTile {
            terrain: Terrain::Sea,
            base_terrain: TerrainBase::Sea,
            altitude: 0,
            hut: false,
            river: false,
            river_edges: 0,
            resource: None,
        });
        Simulation::new_with_roster(map, roster)
    }

    pub(crate) fn start_at(&mut self, turn: i32, year: i32) {
        assert!(
            (1..=super::score::TURN_LIMIT).contains(&turn),
            "invalid scenario start turn {turn}"
        );
        self.game.turn = turn;
        self.game.year = year;
    }

    pub(crate) fn grassland(&mut self, position: (i32, i32)) {
        let tile = self.game.tile_mut(position.0, position.1);
        self.river_edges_dirty |= tile.terrain.is_water();
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
    }

    /// Apply a starting tile preset before placing cities and units.
    pub(crate) fn tile(&mut self, position: (i32, i32), preset: TilePreset) {
        assert!(
            !preset.terrain.is_water()
                || !(preset.river || preset.road || preset.railroad || preset.base.is_some()),
            "land infrastructure requires land at {position:?}"
        );
        let tile = self.game.tile_mut(position.0, position.1);
        self.river_edges_dirty |=
            tile.river != preset.river || tile.terrain.is_water() != preset.terrain.is_water();
        tile.terrain = preset.terrain;
        tile.base_terrain = preset.base_terrain;
        tile.resource = preset.resource;
        tile.river = preset.river;
        tile.road = preset.road || preset.railroad;
        tile.railroad = preset.railroad;
        tile.base = preset.base;
    }

    pub(crate) fn mark_known(&mut self, owner: usize, position: (i32, i32)) {
        self.game
            .tile_mut(position.0, position.1)
            .known_by
            .set(owner, true);
    }

    pub(crate) fn city(&mut self, owner: usize, position: (i32, i32), size: i32) -> u32 {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario city owner {owner}"
        );
        assert!(size > 0, "scenario city size must be positive");
        assert!(
            !self.game.tile(position.0, position.1).terrain.is_water(),
            "scenario city must start on land"
        );
        assert!(
            self.game.city_at(position.0, position.1).is_none(),
            "scenario city tile already occupied"
        );
        self.game.create_city(owner, position.0, position.1);
        let city = self.game.cities.last_mut().expect("city was just created");
        city.size = size;
        city.specialists[2] = size;
        city.id
    }

    /// Install a regular city improvement already in place at the start.
    pub(crate) fn city_improvement(&mut self, city_id: u32, building: Building) {
        assert_eq!(
            building.data().genus,
            Genus::Improvement,
            "expected a regular city improvement"
        );
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city must exist");
        if let Some(reason) = self
            .game
            .building_blocker(&self.game.cities[index], building)
        {
            panic!(
                "cannot preset {} in city {city_id}: {reason}",
                building.data().name
            );
        }
        self.game.cities[index].buildings.push(building);
    }

    pub(crate) fn unit(&mut self, owner: usize, kind: UnitKind, position: (i32, i32)) -> u32 {
        assert!(
            owner < self.game.civilization_count || owner == crate::sim::BARBARIANS,
            "inactive scenario unit owner {owner}"
        );
        self.game.spawn_unit(owner, kind, position.0, position.1)
    }

    pub(crate) fn unit_experience_rank(&mut self, id: u32, rank: usize) {
        let unit = self.unit_mut(id);
        assert!(rank < crate::rules::native::experience().levels.len());
        unit.veteran = rank;
        unit.hp = unit.max_hit_points();
    }

    pub(crate) fn unit_hit_points(&mut self, id: u32, hit_points: i32) {
        let unit = self.unit_mut(id);
        assert!((1..=unit.max_hit_points()).contains(&hit_points));
        unit.hp = hit_points;
    }

    fn unit_mut(&mut self, id: u32) -> &mut crate::sim::Unit {
        let index = self
            .game
            .unit_index_by_id(id)
            .expect("fixture unit must exist");
        &mut self.game.units[index]
    }

    /// Seed the previous turn's position for AI policies that track movement.
    pub(crate) fn previous_unit_position(&mut self, id: u32, position: (i32, i32)) {
        assert!(
            self.game.unit_by_id(id).is_some(),
            "fixture unit must exist"
        );
        self.game.previous_naval_positions.insert(id, position);
    }

    /// Trust values are ordered by the two slots passed by the caller.
    pub(crate) fn diplomacy(
        &mut self,
        first: usize,
        second: usize,
        pact: Pact,
        first_contact: i32,
        trust: [i32; 2],
    ) {
        assert!(
            first != second
                && first < self.game.civilization_count
                && second < self.game.civilization_count,
            "diplomacy requires two active scenario civilizations"
        );
        assert!(
            (0..=self.game.turn).contains(&first_contact),
            "first contact cannot occur after the scenario starts"
        );
        self.game
            .set_scenario_diplomacy(first, second, pact, first_contact, trust);
    }

    /// Start under an unlocked government without scheduling a revolution.
    pub(crate) fn government(&mut self, owner: usize, government: Government) {
        assert!(
            owner < self.game.civilization_count,
            "inactive scenario government owner {owner}"
        );
        self.game.set_scenario_government(owner, government);
    }

    /// The holder has an embassy with the host, independent of the reverse direction.
    pub(crate) fn embassy(&mut self, holder: usize, host: usize) {
        self.game.grant_scenario_embassy(holder, host);
    }

    /// The receiver sees the tiles currently visible to the giver.
    pub(crate) fn shared_vision(&mut self, giver: usize, receiver: usize) {
        self.game.grant_scenario_vision(giver, receiver);
    }

    /// Establish a route between two existing city IDs without a delivery bonus.
    pub(crate) fn trade_route(&mut self, from: u32, to: u32, goods: &str) {
        assert!(
            self.game.establish_scenario_trade_route(from, to, goods),
            "cannot preset trade route {from} -> {to} for goods {goods:?}"
        );
    }

    /// Select a deterministic initial assignment of ordinary workable tiles.
    pub(crate) fn city_workers(&mut self, city_id: u32, count: usize) {
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        let city = &self.game.cities[index];
        let worked: Vec<_> = self
            .game
            .map_city_radius(city)
            .map(|(_, _, x, y)| (x, y))
            .filter(|&(x, y)| self.game.tile_can_support_worker(city, x, y))
            .take(count)
            .collect();
        assert_eq!(worked.len(), count, "fixture needs enough worker tiles");
        self.game.cities[index].worked_tiles = worked;
        self.game.cities[index].specialists = [0; 3];
        self.game.cities[index].food = 0;
    }

    /// Toggle all existing governors atomically, preserving citizen assignments.
    pub(crate) fn governors_active(&mut self, active: bool) {
        for governor in self.game.city_governors.values_mut() {
            governor.active = active;
        }
    }

    /// Preset the founding governor without recomputing citizen assignments.
    pub(crate) fn founding_governor(&mut self, city_id: u32) {
        assert!(self.game.city_by_id(city_id).is_some());
        self.game.city_governors.insert(
            city_id,
            crate::sim::CityGovernor {
                active: true,
                settings: crate::sim::GovernorSettings::FOUNDING,
            },
        );
    }

    pub(crate) fn treasury(&mut self, owner: usize, gold: i32) {
        assert!(owner < self.game.civilization_count);
        self.game.players[owner].gold = gold;
    }

    /// Change only identity while comparing policy preferences over the same world.
    pub(crate) fn civilization_identity(&mut self, owner: usize, id: &str) {
        assert!(civilizations::by_id(id).is_some());
        self.game.players[owner].civilization_id = Some(id.into());
    }

    pub(crate) fn research_goal(&mut self, owner: usize, goal: Tech) {
        self.game.players[owner].research_goal = goal;
    }

    /// Override installed improvements for a controlled economic stress fixture.
    pub(crate) fn city_buildings(&mut self, city_id: u32, buildings: &[Building]) {
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].buildings = buildings.to_vec();
    }

    /// Two living, peaceful AI nations with equal research wants and ample cash.
    /// This is setup only; policy evaluation and assertions belong to scenarios.
    pub(crate) fn strategy_pair(&mut self) -> (u32, Tech) {
        self.game.units.clear();
        self.game.cities.clear();
        self.game.diplomacy.clear();
        self.game.ai_war_plans.clear();
        for player in &mut self.game.players {
            player.ai = false;
        }
        for (owner, x, y) in [(1, 10, 24), (2, 18, 30)] {
            self.game.tile_mut(x, y).terrain = Terrain::Grassland;
            self.game.create_city(owner, x, y);
            let player = &mut self.game.players[owner];
            player.ai = true;
            player.known = vec![Tech::ALPHABET];
            player.researching = Tech::POTTERY;
            player.research_goal = Tech::UNSET;
            player.research = 0;
            player.last_science = 20;
            player.gold = 5000;
        }
        self.game.make_diplomatic_contact(1, 2);
        self.game.sign_pact(1, 2, Pact::Peace);
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        self.game.players[2].known.push(writing);
        self.game.clear_ai_tech_wants(1);
        self.game.add_ai_tech_want(1, Tech::POTTERY, 100.0);
        self.game.add_ai_tech_want(1, writing, 100.0);
        self.civilization_identity(1, "korea");
        (self.game.cities[0].id, writing)
    }

    /// Two AI nations with one technology available for a cash-only exchange.
    pub(crate) fn technology_trade_pair(&mut self) -> Tech {
        self.game.units.clear();
        self.game.cities.clear();
        self.game.diplomacy.clear();
        self.game.ai_war_plans.clear();
        for player in &mut self.game.players {
            player.ai = false;
        }
        let writing = Tech::all().find(|tech| tech.name() == "Writing").unwrap();
        for (owner, x, y) in [(1, 10, 24), (2, 18, 30)] {
            self.game.tile_mut(x, y).terrain = Terrain::Grassland;
            self.game.create_city(owner, x, y);
            let player = &mut self.game.players[owner];
            player.ai = true;
            player.known = vec![Tech::ALPHABET];
            player.researching = Tech::POTTERY;
            player.research_goal = Tech::UNSET;
            player.last_science = 20;
            player.research = 0;
        }
        self.game.players[2].known.push(writing);
        self.game.players[2].gold = 0;
        self.game.make_diplomatic_contact(1, 2);
        self.game.sign_pact(1, 2, Pact::Alliance);
        self.game.diplomacy.get_mut(&(1, 2)).unwrap().trust = [6000; 2];
        writing
    }

    pub(crate) fn known_technologies(&mut self, owner: usize, technologies: &[Tech]) {
        self.game.players[owner].known = technologies.to_vec();
    }

    /// Override the established pact and trust without running treaty actions.
    pub(crate) fn diplomatic_state(
        &mut self,
        first: usize,
        second: usize,
        pact: Pact,
        mut trust: [i32; 2],
    ) {
        if first > second {
            trust.swap(0, 1);
        }
        let relation = self
            .game
            .diplomacy
            .get_mut(&(first.min(second), first.max(second)))
            .expect("fixture relationship exists");
        relation.pact = pact;
        relation.trust = trust;
    }

    pub(crate) fn treaty_expiry(&mut self, first: usize, second: usize, until: i32) {
        self.game
            .diplomacy
            .get_mut(&(first.min(second), first.max(second)))
            .expect("fixture relationship exists")
            .until = until;
    }

    pub(crate) fn ai_control(&mut self, owner: usize, enabled: bool) {
        self.game.players[owner].ai = enabled;
    }

    /// Established roads, rivers and cities for comparing concrete funding needs.
    pub(crate) fn funding_campaign(&mut self) {
        self.game.units.clear();
        self.game.cities.clear();
        self.game.diplomacy.clear();
        self.game.ai_war_plans.clear();
        self.game.ai_purchase_reserve.clear();
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = true;
            tile.road = true;
            tile.hut = false;
        }
        for (owner, x, y) in [(1, 10, 24), (1, 18, 30), (2, 26, 24)] {
            self.game.create_city(owner, x, y);
        }
        self.game.turn += 1; // Rush eligibility requires an established city.
        self.game.players[1].ai = true;
        self.game.players[1].civilization_id = Some("korea".into());
        self.game.players[1].government = Government::Monarchy;
        self.game.players[1].known = vec![Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::MONARCHY];
        self.game.players[1].researching = Tech::POTTERY;
        self.game.players[1].research = 0;
        self.game.players[1].last_science = 0;
        self.game.players[1].gold = 5000;
        self.game.players[1].rates = [60, 40, 0];
        self.game.reconcile_city_workers();
        self.game.make_diplomatic_contact(1, 2);
        self.game.sign_pact(1, 2, Pact::Peace);
    }

    pub(crate) fn unit_home_city(&mut self, unit_id: u32, home: u32) {
        assert!(self.game.city_by_id(home).is_some());
        self.unit_mut(unit_id).home_city = Some(home);
    }

    pub(crate) fn production_stock(
        &mut self,
        city_id: u32,
        production: crate::sim::Production,
        shields: i32,
    ) {
        assert!(shields >= 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].production = production;
        self.game.cities[index].shields = shields;
    }

    pub(crate) fn tax_rates(&mut self, owner: usize, rates: [i32; 3]) {
        assert!(
            rates.iter().all(|rate| (0..=100).contains(rate)) && rates.iter().sum::<i32>() == 100
        );
        self.game.players[owner].rates = rates;
    }

    /// Force a government for a transition/upkeep fixture, without revolution timing.
    pub(crate) fn government_state(&mut self, owner: usize, government: Government) {
        self.game.players[owner].government = government;
    }

    /// Clear starting entities and relationships for an isolated action case.
    pub(crate) fn clear_campaign_entities(&mut self) {
        self.game.units.clear();
        self.game.cities.clear();
        self.game.diplomacy.clear();
        self.game.ai_war_plans.clear();
        for player in &mut self.game.players {
            player.ai = false;
        }
    }

    /// Place a city with the simulation's ordinary founding defaults.
    pub(crate) fn founding_city(&mut self, owner: usize, position: (i32, i32)) -> u32 {
        assert!(owner < self.game.civilization_count);
        self.game.tile_mut(position.0, position.1).terrain = Terrain::Grassland;
        self.game.create_city(owner, position.0, position.1);
        self.game.cities.last().expect("city was just created").id
    }

    /// Empty grassland for isolated unit and city actions; retain the RNG and roster.
    pub(crate) fn grassland_campaign(&mut self, clear_contamination: bool) {
        self.game.units.clear();
        self.game.cities.clear();
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            if clear_contamination {
                tile.pollution = false;
                tile.fallout = false;
            }
        }
    }

    pub(crate) fn unit_moved(&mut self, unit_id: u32, moved: bool) {
        self.unit_mut(unit_id).moved = moved;
    }

    pub(crate) fn tile_owner(&mut self, position: (i32, i32), owner: Option<usize>) {
        self.game.tile_mut(position.0, position.1).owner = owner;
    }

    pub(crate) fn immediate_war_plan(&mut self, owner: usize, target: usize) {
        self.game.ai_war_plans.insert(
            (owner, target),
            super::diplomacy::WarPlan {
                countdown: 0,
                reason: super::diplomacy::WarReason::Hatred,
            },
        );
    }

    pub(crate) fn research_project(&mut self, owner: usize, target: Tech, progress: i32) {
        assert!(progress >= 0);
        self.game.players[owner].researching = target;
        self.game.players[owner].research = progress;
    }

    pub(crate) fn research_progress(&mut self, owner: usize, progress: i32) {
        assert!(progress >= 0);
        self.game.players[owner].research = progress;
    }

    /// Fertile surface for deterministic production and expansion fixtures.
    pub(crate) fn fertile_tiles(&mut self, owner: Option<usize>, known_owner: Option<usize>) {
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.hut = false;
            tile.owner = owner;
            if let Some(owner) = known_owner {
                tile.known_by.set(owner, true);
            }
        }
    }

    pub(crate) fn city_population(
        &mut self,
        city_id: u32,
        size: i32,
        specialists: Option<[i32; 3]>,
    ) {
        assert!(size > 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].size = size;
        if let Some(specialists) = specialists {
            assert!(specialists.iter().all(|&n| n >= 0));
            self.game.cities[index].specialists = specialists;
        }
    }

    pub(crate) fn clear_units(&mut self) {
        self.game.units.clear();
    }

    pub(crate) fn clear_units_and_cities(&mut self) {
        self.game.units.clear();
        self.game.cities.clear();
    }

    pub(crate) fn terrain(&mut self, position: (i32, i32), terrain: Terrain) {
        self.game.tile_mut(position.0, position.1).terrain = terrain;
    }

    /// Clear all terrain modifiers for isolated combat comparisons.
    pub(crate) fn combat_world(&mut self) {
        self.clear_units_and_cities();
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.resource = None;
            tile.hut = false;
        }
    }

    pub(crate) fn legacy_combat_rank(&mut self, id: u32, rank: usize) {
        self.unit_mut(id).c2c3_veteran = rank;
    }

    pub(crate) fn legacy_random_seed(&mut self, seed: u64) {
        self.game.native_random = None;
        self.game.rng = seed;
    }

    pub(crate) fn clear_outcome(&mut self) {
        self.game.game_over = None;
    }

    pub(crate) fn add_technology(&mut self, owner: usize, technology: Tech) {
        self.game.players[owner].known.push(technology);
    }

    pub(crate) fn remove_hut(&mut self, position: (i32, i32)) {
        self.game.tile_mut(position.0, position.1).hut = false;
    }

    pub(crate) fn refresh_visibility(&mut self) {
        self.game.refresh_visibility();
    }

    /// Remove fixture entities without emitting gameplay loss events.
    pub(crate) fn remove_fixture_units(&mut self, ids: &[u32], kind: Option<UnitKind>) {
        self.game
            .units
            .retain(|unit| !ids.contains(&unit.id) && kind != Some(unit.kind));
    }

    /// Productive coastal home and a known overseas island, with no checks or AI steps.
    pub(crate) fn settlement_ferry_coast(&mut self) -> (u32, (i32, i32)) {
        self.game.units.clear();
        self.game.completed_wonders = Building::all()
            .filter(|building| building.data().genus == crate::rules::Genus::GreatWonder)
            .collect();
        for tile in &mut self.game.tiles {
            tile.terrain = Terrain::Sea;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let island = super::topology::offset_native_by_map(home.0, home.1, 6, 0);
        self.game.tile_mut(home.0, home.1).terrain = Terrain::Forest;
        self.game.tile_mut(island.0, island.1).terrain = Terrain::Grassland;
        let productive = super::topology::offset_native_by_map(island.0, island.1, 1, 0);
        let productive_tile = self.game.tile_mut(productive.0, productive.1);
        productive_tile.terrain = Terrain::Grassland;
        productive_tile.known_by.set(1, true);
        self.game.tile_mut(island.0, island.1).known_by.set(1, true);
        for (_, _, x, y) in self.game.map_wrap.city_radius(island.0, island.1) {
            let tile = self.game.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.river = true;
            tile.known_by.set(1, true);
        }
        for (_, _, x, y) in self.game.map_wrap.city_radius(home.0, home.1) {
            if (x, y) == home {
                continue;
            }
            let tile = self.game.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.river = true;
        }
        self.game.create_city(1, home.0, home.1);
        self.game.cities[0].size = 3;
        self.game.cities[0].food = 20;
        self.game.reconcile_city_workers();
        let city = self.game.cities[0].id;
        let worked = self.game.cities[0].worked_tiles.clone();
        for (_, _, x, y) in self.game.map_wrap.city_radius(home.0, home.1) {
            if (x, y) == home || worked.contains(&(x, y)) {
                continue;
            }
            let tile = self.game.tile_mut(x, y);
            tile.terrain = Terrain::Sea;
            tile.resource = None;
            tile.irrigation = false;
            tile.river = false;
        }
        self.game.reconcile_city_workers();
        self.game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        (city, island)
    }

    pub(crate) fn city_history(&mut self, city_id: u32, history: i32) {
        assert!(history >= 0);
        let index = self
            .game
            .city_index_by_id(city_id)
            .expect("fixture city exists");
        self.game.cities[index].history = history;
    }

    /// Make the assembled world ready for the runner's initial snapshot.
    pub(crate) fn finish(self) {
        if self.river_edges_dirty {
            crate::world::topology::rebuild_civ3_river_edges(
                self.game.map_wrap,
                &mut self.game.tiles,
            );
        }
        self.game.refresh_visibility();
        self.game.refresh_score_snapshot();
    }
}

pub(crate) fn capture_known_tiles(sim: &mut Simulation, owner: usize) {
    let mut observed = vec![vec![false; sim.tiles.len()]; owner + 1];
    for (index, tile) in sim.tiles.iter().enumerate() {
        observed[owner][index] = tile.known_by[owner];
    }
    sim.remember_render_map(&observed);
}

pub(crate) fn give_unit_full_movement(sim: &mut Simulation, id: u32, kind: UnitKind) {
    sim.units
        .iter_mut()
        .find(|unit| unit.id == id)
        .unwrap()
        .moves_left = kind.stats().moves * super::movement_rules::campaign_move_fragments();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_initialize_roster_knowledge_calendar_entities_and_diplomacy() {
        let mut game = WorldBuilder::ocean_campaign(
            crate::test_support::seeded_map(),
            &[
                CivPreset {
                    id: "england",
                    technologies: Some(&[Tech::COMBUSTION]),
                },
                CivPreset {
                    id: "germany",
                    technologies: Some(&[]),
                },
            ],
        );
        let mut world = WorldBuilder::new(&mut game);
        world.start_at(120, 1900);
        world.grassland((10, 24));
        let city = world.city(0, (10, 24), 3);
        let ship = world.unit(1, UnitKind::Battleship, (10, 25));
        world.diplomacy(1, 0, Pact::Peace, 42, [3000, 5000]);
        world.finish();

        assert_eq!(game.civilization_count, 2);
        assert_eq!(game.players[0].civilization_id.as_deref(), Some("england"));
        assert_eq!(game.players[1].civilization_id.as_deref(), Some("germany"));
        assert_eq!(game.players[0].known, [Tech::COMBUSTION]);
        assert!(game.players[1].known.is_empty());
        assert_eq!(game.global_advances, [Tech::COMBUSTION]);
        assert_ne!(game.players[0].researching, Tech::COMBUSTION);
        assert_eq!((game.turn, game.year), (120, 1900));
        assert_eq!(game.city_by_id(city).unwrap().founded_turn, 120);
        assert_eq!(game.nation_scores[0].score_population, 3);
        assert_eq!(game.unit_by_id(ship).unwrap().owner, 1);
        assert_eq!(game.pact(0, 1), Pact::Peace);
        assert_eq!(game.diplomacy[&(0, 1)].trust, [5000, 3000]);
    }
}
