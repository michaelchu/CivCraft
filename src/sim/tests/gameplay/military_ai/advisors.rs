//! Unit checks and fixtures for private simulation behavior.

use super::super::*;

impl Simulation {
    pub(crate) fn offensive_research_test(&self) -> (bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        game.ai_ground_targets.clear();
        game.ai_guard_targets.clear();
        game.ai_unit_guards.clear();
        game.unit_goto_tiles.clear();
        game.ai_ferries = Default::default();
        game.players[1].known = vec![Tech::BRONZE_WORKING, Tech::THE_WHEEL];
        game.players[1].government = Government::DESPOTISM;
        game.players[1].rates = [40, 60, 0];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.mine = false;
            tile.irrigation = false;
            tile.resource = None;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(true);
        }
        let home = (10, 24);
        let enemy = offset_native_by_map(home.0, home.1, 3, 0);
        game.create_city(1, home.0, home.1);
        game.create_city(HUMAN, enemy.0, enemy.1);
        game.cities[1].size = 8;
        game.cities[0].buildings.push(Building::BARRACKS);
        game.spawn_unit(1, UnitKind::Spearman, home.0, home.1);
        game.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
        game.sign_pact(1, HUMAN, Pact::War);
        let city = game.cities[0].clone();
        game.tile_mut(enemy.0, enemy.1).known_by.set(1, false);
        let unknown = game.offensive_advice(&city);
        let hidden = unknown.0.is_none() && unknown.1.is_empty();
        game.tile_mut(enemy.0, enemy.1).known_by.set(1, true);
        game.players[1].known.push(Tech::IRON_WORKING);
        let learned = game.offensive_advice(&city);
        let known = learned
            .1
            .iter()
            .all(|(tech, _)| *tech != Tech::IRON_WORKING);
        (hidden, known)
    }

    pub(crate) fn counter_defender_research_test(&self) -> (bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        game.players[1].known.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
            tile.owner = None;
        }
        game.create_city(1, 10, 24);
        game.sign_pact(1, HUMAN, Pact::War);
        let quiet = game
            .counter_defender_tech_wants(&game.cities[0])
            .into_iter()
            .all(|want| want == 0.0);
        let target = offset_native_by_map(10, 24, 1, 0);
        game.spawn_unit(HUMAN, UnitKind::Warrior, target.0, target.1);
        let request = game.counter_defender_request(&game.cities[0], UnitKind::Warrior, 101);
        let first = request == Some((Tech::BRONZE_WORKING, 101));
        let threats = game.counter_defender_tech_wants(&game.cities[0]);
        let expected =
            game.advisor_threat_vulnerability(&game.cities[0], game.units.last().unwrap());
        let feedback = threats[Tech::BRONZE_WORKING.0] == expected as f32 && expected > 0;
        (quiet, first, feedback)
    }

    pub(crate) fn native_defensive_effects_test(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.base = None;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
        }
        self.create_city(1, 10, 24);
        self.cities[0].buildings.retain(|&b| b != Building::PALACE);
        let value =
            |sim: &Self, building| sim.defensive_building_effect_want(&sim.cities[0], building);
        let quiet = value(self, Building::CITY_WALLS) == 3.0
            && value(self, Building::COASTAL_DEFENSE) == 6.0
            && value(self, Building::SAM_BATTERY) == 6.0;
        // is_capital includes the secondary Capital_City value1 as well as
        // the primary Palace value2; it is not is_primary_capital.
        self.cities[0].buildings.push(Building::FORBIDDEN_PALACE);
        let quiet = quiet
            && value(self, Building::CITY_WALLS) == 11.0
            && value(self, Building::COASTAL_DEFENSE) == 22.0;
        self.cities[0]
            .buildings
            .retain(|&b| b != Building::FORBIDDEN_PALACE);
        let remote = offset_native_by_map(10, 24, 4, 0);
        self.spawn_unit(HUMAN, UnitKind::Marine, remote.0, remote.1);
        let unit_only = value(self, Building::CITY_WALLS) == 3.0;
        self.create_city(HUMAN, remote.0, remote.1);
        let cities = unit_only
            && value(self, Building::CITY_WALLS) == 83.0
            && value(self, Building::SAM_BATTERY) == 166.0;
        self.cities.retain(|city| city.owner == 1);
        self.units.clear();
        let coast = offset_native_by_map(10, 24, 1, 0);
        self.tile_mut(coast.0, coast.1).terrain = Terrain::Sea;
        self.spawn_unit(HUMAN, UnitKind::Battleship, coast.0, coast.1);
        let ocean = value(self, Building::COASTAL_DEFENSE) == 38.0
            && value(self, Building::SAM_BATTERY) == 38.0;
        self.spawn_unit(HUMAN, UnitKind::Transport, coast.0, coast.1);
        let rows = ocean
            && value(self, Building::CITY_WALLS) == 9.0
            && value(self, Building::COASTAL_DEFENSE) == 40.0
            && value(self, Building::SAM_BATTERY) == 50.0;
        self.units.clear();
        self.tile_mut(coast.0, coast.1).terrain = Terrain::Grassland;
        self.players[1].known.clear();
        self.players[1].government = Government::DESPOTISM;
        self.spawn_unit(HUMAN, UnitKind::Warrior, coast.0, coast.1);
        // Native static defender side effect: current Warrior rating.16,
        // future Spearman484/225, wallvalue90, danger211, Bronze30/4=7.
        // ((484/225)*9*211/27)*10/(.16*9) ~=1050.6584.
        let want = self.production_defender_tech_want(&self.cities[0], Tech::BRONZE_WORKING);
        let technology = want > 0.0 && self.advisor_tech_wants(1)[Tech::BRONZE_WORKING.0] >= want;
        (quiet, cities, rows, technology)
    }

    pub(crate) fn native_city_defense_advisor_test(&mut self) -> (bool, bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
        }
        self.players[1].known = vec![Tech::BRONZE_WORKING];
        self.players[1].government = Government::DESPOTISM;
        self.create_city(1, 10, 24);
        let quiet = self.native_city_defense_choice(&self.cities[0]).0.is_none();
        let near = offset_native_by_map(10, 24, 1, 0);
        self.spawn_unit(HUMAN, UnitKind::Warrior, near.0, near.1);
        let first = self
            .native_city_defense_choice(&self.cities[0])
            .0
            .is_some_and(|choice| choice.want > 0.0);
        self.spawn_unit(1, UnitKind::Spearman, 10, 24);
        // Danger rating2 => total4 => qdanger16; fortified Spearman
        // defense rating10 => defense100, giving a native16 percent bid.
        let defended_assessment = self.advisor_guard_city_assessment(&self.cities[0]);
        let defended = defended_assessment.danger > 0
            && defended_assessment.defense > 0
            && self.native_city_defense_choice(&self.cities[0]).0.is_some();
        self.units.clear();
        self.cities[0].size = 3;
        self.cities[0].specialists[2] = 1;
        let police = self
            .native_city_defense_choice(&self.cities[0])
            .0
            .is_some_and(|choice| choice.want > 0.0);
        (quiet, first, defended, police)
    }

    pub(crate) fn ai_growth_expansion_test(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Plains;
            tile.resource = Some(Resource::Wheat);
            tile.irrigation = true;
            tile.river = false;
            tile.hut = false;
        }
        self.create_city(1, 10, 24);
        self.cities[0].size = 6;
        self.cities[0].buildings.extend([
            Building::TEMPLE,
            Building::CATHEDRAL,
            Building::COLOSSEUM,
        ]);
        self.spawn_unit(1, UnitKind::Warrior, 10, 24);
        let id = self.cities[0].id;
        self.players[1]
            .known
            .extend([Tech::POTTERY, Tech::CONSTRUCTION]);
        self.reconcile_city_workers();
        let city = self.cities[0].clone();
        let size_limit = self.city_size_limit(&city);
        let mut with_aqueduct = city.clone();
        with_aqueduct.buildings.push(Building::AQUEDUCT);
        let aqueduct_extends_city_limit =
            size_limit == 6 && self.city_size_limit(&with_aqueduct) == 12;
        let aqueduct_available = self.building_blocker(&city, Building::AQUEDUCT).is_none();
        let expansion_available = self.settlement_production_choice(&self.cities[0]).is_some();
        // Source documents the building effects, not a required ranking among
        // the Aqueduct, Wonders, and expansion units. Check the rule and that
        // the advisor's chosen production is applied without fixing its rank.
        let advisor_choice = self.choose_ai_production(1, id);
        self.prepare_ai_city_defenses(1);
        let selected = self.cities[0].production == advisor_choice;
        aqueduct_extends_city_limit && aqueduct_available && expansion_available && selected
    }
}
