mod ai;
mod buildings;
mod cities;
mod civil_war;
mod commands;
mod diplomacy;
mod economy;
mod government;
pub(crate) use government::Government;
mod movement;
mod research;
pub(crate) use commands::Command;
use model::research_cost;
mod model;
pub(crate) mod topology;
mod upkeep;
mod villages;

use crate::rules::{Building, Tech};
pub(crate) use model::*;
use std::collections::VecDeque;
use topology::*;

pub(crate) const PLAYER_COUNT: usize = 6;
pub(crate) const HUMAN: usize = 0;
pub(crate) const PLAYER_SLOTS: usize = 512;
pub(crate) const BARBARIANS: usize = PLAYER_COUNT;
const PERCENT_SCALE: i32 = 100;
const CITY_MIN_DISTANCE: i32 = 3;
pub(crate) const MOVE_FRAGMENTS: i32 = 6;
const POWER_FACTOR: i32 = 10;
const BASE_TECH_COST: i32 = 10;
const MIN_TECH_COST: i32 = 10;
const TECH_COST_PERCENT: i32 = 300;
const SCIENCEBOX_PERCENT: i32 = 100;
const FOODBOX_PERCENT: i32 = 100;
pub(crate) const CITIZEN_FOOD_COST: i32 = 2;
const GRANARY_FOOD_INITIAL: [i32; 8] = [20, 20, 20, 20, 20, 30, 30, 40];
const GRANARY_FOOD_INCREMENT: i32 = 0;
const CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_DEFEND_BONUS: i32 = 50;
const LARGE_CITY_SIZE: i32 = 9;
const FORTIFY_DEFENSE_BONUS: i32 = 50;
const CITY_MIN_HP_PERCENT: i32 = 33;
const BASE_HP_REGEN_2: i32 = 10;
const FORTIFIED_HP_REGEN_2: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MoveResult {
    Moved,
    Fought,
    Blocked,
}

pub(crate) struct Simulation {
    pub(crate) world_seed: u64,
    tiles: Vec<Tile>,
    units: Vec<Unit>,
    cities: Vec<City>,
    completed_wonders: Vec<Building>,
    spaceship_parts: Vec<[usize; 3]>,
    diplomacy: std::collections::BTreeMap<(usize, usize), diplomacy::Relation>,
    apollo_completed: bool,
    players: Vec<Player>,
    government_adoptions: [i32; 9],
    turn: i32,
    year: i32,
    next_unit_id: u32,
    next_city_id: u32,
    rng: u64,
    events: Vec<String>,
    game_over: Option<String>,
}

fn percent_of(value: i32, percent: i32) -> i32 {
    value * percent / PERCENT_SCALE
}

fn apply_percent_bonus(value: i32, bonus: i32) -> i32 {
    percent_of(value, PERCENT_SCALE + bonus)
}

// common/combat.c:win_chance. This is the probability that the attacker
// scores every required hit before the defender does, including unequal HP
// and firepower. The native defender-selection path uses this exact result.
fn combat_win_chance(
    attack_strength: i32,
    attacker_hp: i32,
    attacker_firepower: i32,
    defense_strength: i32,
    defender_hp: i32,
    defender_firepower: i32,
) -> f64 {
    let attacker_rounds_to_lose = (attacker_hp + defender_firepower - 1) / defender_firepower;
    let defender_rounds_to_lose = (defender_hp + attacker_firepower - 1) / attacker_firepower;
    let total_strength = attack_strength + defense_strength;
    let attacker_round_loss = if total_strength == 0 {
        0.5
    } else {
        defense_strength as f64 / total_strength as f64
    };
    let defender_round_loss = 1.0 - attacker_round_loss;

    let mut binomial_term = defender_round_loss.powi(defender_rounds_to_lose.saturating_sub(1));
    let mut accumulated = binomial_term;
    for lost_rounds in 1..attacker_rounds_to_lose {
        let series_length = lost_rounds + defender_rounds_to_lose - 1;
        binomial_term *= series_length as f64;
        binomial_term /= lost_rounds as f64;
        binomial_term *= attacker_round_loss;
        accumulated += binomial_term;
    }
    (accumulated * defender_round_loss).clamp(0.0, 1.0)
}

impl Simulation {
    pub(crate) fn units(&self) -> &[Unit] {
        &self.units
    }
    pub(crate) fn cities(&self) -> &[City] {
        &self.cities
    }
    pub(crate) fn players(&self) -> &[Player] {
        &self.players
    }
    pub(crate) fn events(&self) -> &[String] {
        &self.events
    }
    pub(crate) fn turn(&self) -> i32 {
        self.turn
    }
    pub(crate) fn year(&self) -> i32 {
        self.year
    }
    pub(crate) fn game_over(&self) -> Option<&str> {
        self.game_over.as_deref()
    }

    pub(crate) fn new(map: crate::mapgen::GeneratedMap) -> Self {
        let crate::mapgen::GeneratedMap {
            seed,
            tiles,
            starts,
        } = map;

        let names = [
            "Crimson League",
            "Golden Compact",
            "Azure Assembly",
            "Violet Court",
            "Amber Union",
            "Rose Confederacy",
        ];
        let free_techs = [
            Tech::BRONZE_WORKING,
            Tech::ALPHABET,
            Tech::HORSEBACK_RIDING,
            Tech::POTTERY,
            Tech::WARRIOR_CODE,
            Tech::BRONZE_WORKING,
        ];
        let research_targets = [
            Tech::WARRIOR_CODE,
            Tech::BRONZE_WORKING,
            Tech::POTTERY,
            Tech::HORSEBACK_RIDING,
            Tech::ALPHABET,
            Tech::WARRIOR_CODE,
        ];
        let mut players = Vec::new();
        for i in 0..PLAYER_COUNT {
            players.push(Player {
                name: names[i],
                gold: 50,
                rates: [60, 40, 0],
                government: Government::Tribal,
                revolution_target: None,
                revolution_turn: None,
                known: vec![free_techs[i]],
                researching: research_targets[i],
                research_goal: [
                    research_targets[i],
                    Tech::IRON_WORKING,
                    Tech::THE_WHEEL,
                    Tech::MATHEMATICS,
                    Tech::IRON_WORKING,
                    Tech::THE_WHEEL,
                ][i],
                future_techs: 0,
                research: 0,
                ai: i != HUMAN,
            });
        }

        players.push(Player {
            name: "Barbarians",
            gold: 0,
            rates: [60, 40, 0],
            government: Government::Tribal,
            revolution_target: None,
            revolution_turn: None,
            known: Vec::new(),
            researching: Tech::FUTURE,
            research_goal: Tech::FUTURE,
            future_techs: 0,
            research: 0,
            ai: true,
        });

        let mut units = Vec::new();
        let mut next_unit_id = 1;
        for owner in 0..PLAYER_COUNT {
            for kind in [
                UnitKind::Settlers,
                UnitKind::Worker,
                UnitKind::Explorer,
                UnitKind::Diplomat,
            ] {
                let stats = kind.stats();
                units.push(Unit {
                    id: next_unit_id,
                    owner,
                    kind,
                    transport: None,
                    home_city: None,
                    home_order: 0,
                    x: starts[owner].0,
                    y: starts[owner].1,
                    hp: stats.hp,
                    moves_left: stats.moves * MOVE_FRAGMENTS,
                    state: UnitState::Idle,
                    moved: false,
                    automated: false,
                    work_kind: None,
                    work_remaining: 0,
                });
                next_unit_id += 1;
            }
        }

        let mut game = Self {
            world_seed: seed,
            tiles,
            units,
            cities: Vec::new(),
            completed_wonders: Vec::new(),
            spaceship_parts: vec![[0; 3]; players.len()],
            diplomacy: Default::default(),
            apollo_completed: false,
            players,
            government_adoptions: [0; 9],
            turn: 1,
            year: -4000,
            next_unit_id,
            next_city_id: 1,
            rng: 0x9e37_79b9_7f4a_7c15,
            events: vec![
                "4000 BCE - Your people are ready to settle.".to_owned(),
                "Choose the Settlers and press B to found a city.".to_owned(),
                "Explore, research, grow, and unite the world.".to_owned(),
            ],
            game_over: None,
        };
        for owner in 1..PLAYER_COUNT {
            game.players[owner].researching = game.research_target(owner);
        }
        game.refresh_visibility();
        game
    }

    fn idx(x: i32, y: i32) -> usize {
        let wx = x.rem_euclid(MAP_W);
        let wy = y.rem_euclid(MAP_H);
        (wy * MAP_W + wx) as usize
    }

    fn xy(index: usize) -> (i32, i32) {
        ((index as i32) % MAP_W, (index as i32) / MAP_W)
    }

    pub(crate) fn tile(&self, x: i32, y: i32) -> &Tile {
        &self.tiles[Self::idx(x, y)]
    }

    fn tile_mut(&mut self, x: i32, y: i32) -> &mut Tile {
        let idx = Self::idx(x, y);
        &mut self.tiles[idx]
    }

    pub(crate) fn add_event(&mut self, text: impl Into<String>) {
        self.events.push(text.into());
        if self.events.len() > 10 {
            self.events.remove(0);
        }
    }

    fn rand_below(&mut self, max: i32) -> i32 {
        if max <= 1 {
            return 0;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % max as u64) as i32
    }

    pub(crate) fn unit_available(&self, owner: usize, kind: UnitKind) -> bool {
        match kind {
            UnitKind::Phalanx => self.has_tech(owner, Tech::BRONZE_WORKING),
            UnitKind::Archer => self.has_tech(owner, Tech::WARRIOR_CODE),
            UnitKind::Legion => self.has_tech(owner, Tech::IRON_WORKING),
            UnitKind::Horseman => self.has_tech(owner, Tech::HORSEBACK_RIDING),
            UnitKind::Chariot => self.has_tech(owner, Tech::THE_WHEEL),
            UnitKind::Catapult => self.has_tech(owner, Tech::MATHEMATICS),
            UnitKind::Diplomat => self.has_tech(owner, Tech::ALPHABET),
            UnitKind::Explorer => false,
            _ => true,
        }
    }

    fn refresh_visibility(&mut self) {
        for tile in &mut self.tiles {
            tile.visible = false;
        }
        let revealers: Vec<(usize, i32, i32, i32)> = self
            .units
            .iter()
            .map(|u| (u.owner, u.x, u.y, u.kind.stats().vision_sq))
            .chain(self.cities.iter().map(|c| (c.owner, c.x, c.y, 8)))
            .collect();
        for (owner, x, y, radius_sq) in revealers {
            let radius = (radius_sq as f32).sqrt().ceil() as i32;
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    let distance = isohex_vector_distance(dx, dy);
                    if distance * distance <= radius_sq {
                        let (visible_x, visible_y) = offset_native_by_map(x, y, dx, dy);
                        let idx = Self::idx(visible_x, visible_y);
                        if owner != BARBARIANS {
                            self.tiles[idx].known_by[owner] = true;
                        }
                        if owner == HUMAN {
                            self.tiles[idx].visible = true;
                            self.tiles[idx].seen = true;
                        }
                    }
                }
            }
        }
        self.remember_visible_rivals();
    }

    pub(crate) fn map_distance(&self, ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
        let (dx, dy) = wrapped_map_vector(ax, ay, bx, by);
        isohex_vector_distance(dx, dy)
    }

    fn spawn_unit(&mut self, owner: usize, kind: UnitKind, x: i32, y: i32) -> u32 {
        let id = self.next_unit_id;
        self.next_unit_id += 1;
        let stats = kind.stats();
        self.units.push(Unit {
            id,
            owner,
            kind,
            transport: None,
            home_city: None,
            home_order: 0,
            x,
            y,
            hp: stats.hp,
            moves_left: stats.moves * MOVE_FRAGMENTS,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_remaining: 0,
        });
        id
    }

    fn advance_calendar(&mut self) {
        let step = if self.year < -1000 {
            50
        } else if self.year < 0 {
            25
        } else if self.year < 1000 {
            20
        } else if self.year < 1500 {
            10
        } else if self.year < 1750 {
            5
        } else if self.year < 1900 {
            2
        } else {
            1
        };
        self.year += step;
        if self.year == 0 {
            self.year = 1;
        }
    }

    fn end_turn(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        let automated: Vec<u32> = self
            .units
            .iter()
            .filter(|u| u.owner == HUMAN && u.automated)
            .map(|u| u.id)
            .collect();
        for id in automated {
            self.run_automated_unit(id);
        }
        for owner in 1..self.players.len() {
            if owner == BARBARIANS {
                continue;
            }
            let upgrade = match self.players[owner].government {
                Government::Tribal => Some(Government::Republic),
                Government::Despotism => Some(Government::Monarchy),
                _ => None,
            };
            if let Some(target) = upgrade {
                self.change_government(owner, target);
            }
            self.run_ai_player(owner);
            self.choose_ai_rates(owner);
        }
        self.run_ai_player(BARBARIANS);
        self.process_cities();
        self.update_activities_and_restore();
        self.turn += 1;
        self.advance_diplomacy();
        self.finish_revolutions();
        self.advance_calendar();
        self.refresh_visibility();
        self.add_event(format!(
            "Turn {} begins - {}.",
            self.turn,
            year_label(self.year)
        ));
        self.check_game_over();
    }

    fn check_game_over(&mut self) {
        let human_survives = self.cities.iter().any(|c| c.owner == HUMAN)
            || self
                .units
                .iter()
                .any(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers);
        if !human_survives {
            self.game_over = Some("Your civilization has passed into history.".to_owned());
            return;
        }
        let rival_survives =
            self.cities
                .iter()
                .any(|c| c.owner != HUMAN && c.owner != BARBARIANS)
                || self.units.iter().any(|u| {
                    u.owner != HUMAN && u.owner != BARBARIANS && u.kind == UnitKind::Settlers
                });
        if !rival_survives {
            self.game_over = Some("The world is united beneath your banner.".to_owned());
        }
    }
}

pub(crate) fn year_label(year: i32) -> String {
    if year < 0 {
        format!("{} BCE", -year)
    } else {
        format!("{} CE", year)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn railroad_research_upgrades_centers_and_enables_worker_tracks() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let worker = game
            .units
            .iter()
            .find(|u| u.owner == HUMAN && u.kind == UnitKind::Worker)
            .unwrap()
            .clone();
        let (x, y) = (worker.x, worker.y);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        game.tile_mut(nx, ny).terrain = Terrain::Grassland;
        game.create_city(HUMAN, nx, ny);
        let tile = game.tile_mut(x, y);
        tile.terrain = Terrain::Hills;
        tile.road = true;
        tile.river = false;
        tile.mine = true;
        let initial = tile.yields();
        assert!(!game.apply(Command::Work(worker.id, WorkKind::Railroad)));
        game.discover(HUMAN, Tech::RAILROAD);
        assert!(game.tile(nx, ny).railroad);
        assert!(game.apply(Command::Work(worker.id, WorkKind::Railroad)));
        for _ in 0..2 {
            game.update_activities_and_restore();
            assert!(!game.tile(x, y).railroad);
        }
        game.update_activities_and_restore();
        assert!(game.tile(x, y).railroad);
        assert_eq!(
            game.tile(x, y).yields(),
            (initial.0, initial.1 + initial.1 / 2, initial.2)
        );
        assert_eq!(game.movement_cost_for(&worker, nx, ny), Some(1));
    }

    use super::*;

    #[test]
    fn native_and_map_coordinates_round_trip() {
        for native_y in 0..MAP_H {
            for native_x in 0..MAP_W {
                let (map_x, map_y) = native_to_map(native_x, native_y);
                assert_eq!(map_to_native(map_x, map_y), (native_x, native_y));
            }
        }
    }

    #[test]
    fn isohex_neighbors_and_wrapping_match_freeciv_distance() {
        assert!(!is_isohex_step(1, -1));
        assert!(!is_isohex_step(-1, 1));
        assert!(!is_isohex_step(2, 0));

        for &(start_x, start_y) in &[
            (0, 0),
            (MAP_W - 1, MAP_H - 1),
            crate::mapgen::generate(42).starts[0],
        ] {
            let mut neighbors = Vec::new();
            for (dx, dy) in ISOHEX_DIRS {
                let neighbor = offset_native_by_map(start_x, start_y, dx, dy);
                assert_eq!(
                    isohex_vector_distance(
                        wrapped_map_vector(start_x, start_y, neighbor.0, neighbor.1).0,
                        wrapped_map_vector(start_x, start_y, neighbor.0, neighbor.1).1,
                    ),
                    1
                );
                neighbors.push(neighbor);
            }
            neighbors.sort_unstable();
            neighbors.dedup();
            assert_eq!(neighbors.len(), 6);
        }
    }

    #[test]
    fn generated_world_uses_reference_dimensions_and_start_positions() {
        assert_eq!(MAP_TARGET_TILES, 2_000);
        assert_eq!(MAP_LINEAR_SCALE, 16);
        assert_eq!((MAP_W, MAP_H), (32, 64));
        let first = crate::mapgen::generate(42);
        let terrain = |map: &crate::mapgen::GeneratedMap| {
            map.tiles
                .iter()
                .map(|tile| (tile.terrain, tile.river, tile.resource))
                .collect::<Vec<_>>()
        };
        let replay = crate::mapgen::generate(42);
        assert_eq!(terrain(&first), terrain(&replay));
        assert_eq!(first.starts, replay.starts);
        assert_ne!(terrain(&first), terrain(&crate::mapgen::generate(12345)));

        for seed in 0..32 {
            let map = crate::mapgen::generate(seed);
            let starts = map.starts;
            let game = Simulation::new(map);
            assert_eq!(game.tiles.len(), (MAP_W * MAP_H) as usize);
            for (i, &(x, y)) in starts.iter().enumerate() {
                assert!(game.can_found_at(x, y));
                assert!(starts[..i].iter().all(|&(other_x, other_y)| {
                    game.map_distance(x, y, other_x, other_y) >= CITY_MIN_DISTANCE
                }));
            }
        }
    }

    #[test]
    fn wrapped_path_uses_one_legal_hex_step() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
        }
        let unit_id = game.units[0].id;
        game.units[0].x = 0;
        game.units[0].y = 0;
        let (target_x, target_y) = offset_native_by_map(0, 0, -1, -1);
        assert_eq!(game.path_to(unit_id, target_x, target_y), vec![(-1, -1)]);
        assert_eq!(game.map_distance(0, 0, target_x, target_y), 1);
    }

    #[test]
    fn healing_uses_city_minimum_and_fortified_regeneration() {
        let game = Simulation::new(crate::mapgen::generate(42));
        let mut unit = game
            .units
            .iter()
            .find(|unit| unit.kind == UnitKind::Warrior)
            .cloned()
            .unwrap_or_else(|| Unit {
                id: 999,
                owner: HUMAN,
                kind: UnitKind::Warrior,
                transport: None,
                home_city: None,
                home_order: 0,
                x: crate::mapgen::generate(42).starts[0].0,
                y: crate::mapgen::generate(42).starts[0].1,
                hp: 1,
                moves_left: 0,
                state: UnitState::Idle,
                moved: false,
                automated: false,
                work_kind: None,
                work_remaining: 0,
            });
        unit.hp = 1;
        unit.moved = false;
        unit.state = UnitState::Idle;
        assert_eq!(Simulation::hitpoint_gain(&unit, false), 1);
        assert_eq!(Simulation::hitpoint_gain(&unit, true), 4);
        unit.state = UnitState::Fortified;
        assert_eq!(Simulation::hitpoint_gain(&unit, true), 5);
        unit.moved = true;
        assert_eq!(Simulation::hitpoint_gain(&unit, true), 0);

        // The compiled hp_gain_coord performs integer max-HP / 10 before
        // applying HP_Regen_2. Settlers have 20 maximum HP, so they gain 2
        // in the field and 6 minimum-city HP plus 2 regeneration in a city.
        unit.kind = UnitKind::Settlers;
        unit.hp = 1;
        unit.state = UnitState::Idle;
        unit.moved = false;
        assert_eq!(Simulation::hitpoint_gain(&unit, false), 2);
        assert_eq!(Simulation::hitpoint_gain(&unit, true), 8);
    }

    #[test]
    fn defense_stages_and_research_costs_match_civ2civ3() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = crate::mapgen::generate(42).starts[0];
        game.tile_mut(x, y).terrain = Terrain::Forest;
        let mut defender = Unit {
            id: 999,
            owner: HUMAN,
            kind: UnitKind::Warrior,
            transport: None,
            home_city: None,
            home_order: 0,
            x,
            y,
            hp: 10,
            moves_left: MOVE_FRAGMENTS,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_remaining: 0,
        };

        // Forest first: 10 * 125% = 12. Field fortification is a later
        // stage: 12 * 150% = 18.
        assert_eq!(game.defense_power(&defender, x, y), 12);
        defender.state = UnitState::Fortified;
        assert_eq!(game.defense_power(&defender, x, y), 18);

        // A city adds a 50% Defend_Bonus and gives can-fortify ground units
        // an automatic 50% Fortify_Defense_Bonus. Integer stages are
        // 12 * 150% = 18, then 18 * 150% = 27.
        defender.state = UnitState::Idle;
        game.cities.push(City {
            id: 999,
            owner: HUMAN,
            x,
            y,
            name: "Test".to_owned(),
            size: 1,
            food: 0,
            shields: 0,
            production: Production::Unit(UnitKind::Warrior),
            production_before_switch: None,
            buildings: Vec::new(),
            specialists: [0; 3],
            was_happy: false,
        });
        assert_eq!(game.defense_power(&defender, x, y), 27);

        // Linear precalculation is 10 bulbs per recursive requirement; the
        // ruleset's unconditional 300% Tech_Cost_Pct effect is then applied.
        assert_eq!(Tech::ALPHABET.cost(), 30);
        assert_eq!(Tech::IRON_WORKING.cost(), 90);
    }

    #[test]
    fn combat_odds_and_defender_selection_match_native_rules() {
        let even_fight = combat_win_chance(10, 10, 1, 10, 10, 1);
        assert!((even_fight - 0.5).abs() < 1.0e-12);

        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        let attacker = Unit {
            id: 1,
            owner: HUMAN,
            kind: UnitKind::Warrior,
            transport: None,
            home_city: None,
            home_order: 0,
            x: 0,
            y: 0,
            hp: 10,
            moves_left: MOVE_FRAGMENTS,
            state: UnitState::Idle,
            moved: false,
            automated: false,
            work_kind: None,
            work_remaining: 0,
        };
        let mut fragile_defender = attacker.clone();
        fragile_defender.id = 2;
        fragile_defender.owner = 1;
        fragile_defender.x = 1;
        fragile_defender.hp = 1;
        let mut healthy_defender = fragile_defender.clone();
        healthy_defender.id = 3;
        healthy_defender.hp = 10;
        game.units = vec![attacker.clone(), fragile_defender, healthy_defender];
        game.tile_mut(1, 0).terrain = Terrain::Grassland;

        assert_eq!(game.best_defender_id(&attacker, 1, 0), Some(3));
    }

    #[test]
    fn granary_formula_uses_ruleset_initial_values_and_foodbox() {
        assert_eq!(Simulation::growth_box(0), 0);
        assert_eq!(Simulation::growth_box(1), 20);
        assert_eq!(Simulation::growth_box(6), 30);
        assert_eq!(Simulation::growth_box(8), 40);
        assert_eq!(Simulation::growth_box(20), 40);
    }

    #[test]
    fn famine_destroys_size_one_city() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let settler = game
            .units
            .iter()
            .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Settlers)
            .map(|unit| unit.id)
            .expect("human settler");
        assert!(game.found_city(settler));
        game.cities[0].food = -100;
        game.process_cities();
        assert!(game.cities.iter().all(|city| city.owner != HUMAN));
    }

    #[test]
    fn irrigation_needs_water_and_conflicts_with_mines() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        let (x, y) = crate::mapgen::generate(42).starts[0];
        game.tile_mut(x, y).terrain = Terrain::Plains;
        game.tile_mut(x, y).river = false;
        for (dx, dy) in ISOHEX_DIRS {
            let (neighbor_x, neighbor_y) = offset_native_by_map(x, y, dx, dy);
            let neighbor = game.tile_mut(neighbor_x, neighbor_y);
            neighbor.terrain = Terrain::Grassland;
            neighbor.irrigation = false;
            neighbor.river = false;
        }
        assert!(!game.can_irrigate_at(x, y));
        let (water_x, water_y) = offset_native_by_map(x, y, ISOHEX_DIRS[0].0, ISOHEX_DIRS[0].1);
        game.tile_mut(water_x, water_y).terrain = Terrain::Ocean;
        assert!(game.can_irrigate_at(x, y));

        let worker_index = game
            .units
            .iter()
            .position(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Worker)
            .expect("human worker");
        game.units[worker_index].x = x;
        game.units[worker_index].y = y;
        game.units[worker_index].work_kind = Some(WorkKind::Irrigation);
        game.units[worker_index].work_remaining = 1;
        game.tile_mut(x, y).mine = true;
        game.update_activities_and_restore();
        assert!(game.tile(x, y).irrigation);
        assert!(!game.tile(x, y).mine);

        game.units[worker_index].work_kind = Some(WorkKind::Mine);
        game.units[worker_index].work_remaining = 1;
        game.update_activities_and_restore();
        assert!(game.tile(x, y).mine);
        assert!(!game.tile(x, y).irrigation);
    }
}
