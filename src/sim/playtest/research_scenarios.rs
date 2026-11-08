//! Automated gameplay scenarios covering research behavior.

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_RESEARCH_SAVE: AtomicU64 = AtomicU64::new(0);

impl Simulation {
    pub(crate) fn native_unset_research_goal_playtest(&self) -> (bool, bool, bool) {
        let mut sim = self.clone();
        sim.players[1].known.clear();
        sim.players[1].researching = Tech::BRONZE_WORKING;
        sim.players[1].research_goal = Tech::POTTERY;
        sim.players[1].research = 17;
        let cache = sim.ai_tech_wants.clone();
        sim.discover(1, Tech::POTTERY);
        let gift_clears = sim.players[1].research_goal == Tech::UNSET
            && sim.players[1].researching == Tech::BRONZE_WORKING
            && sim.players[1].research == 17
            && sim.ai_tech_wants == cache;

        sim.players[1].known.clear();
        sim.players[1].research_goal = Tech::GUNPOWDER;
        sim.ai_tech_wants
            .insert(1, vec![-1000.5; Tech::all().count()]);
        sim.select_ai_research(1);
        let negative_clears = sim.players[1].research_goal == Tech::UNSET;
        let known = sim.players[1].known.clone();
        let bulbs = sim.players[1].research;
        sim.discover(1, Tech::UNSET);
        let sentinel = Tech::UNSET.0 == Tech::FUTURE.0 + 1
            && !sim.tech_available(1, Tech::UNSET)
            && sim.research_path(1, Tech::UNSET).is_empty()
            && sim.target_cost(1, Tech::UNSET) == 0
            && sim.players[1].known == known
            && sim.players[1].research == bulbs
            && sim.research_target(1) != Tech::UNSET;
        (gift_clears, negative_clears, sentinel)
    }

    pub(crate) fn native_research_arithmetic_playtest(&self) -> (bool, bool, bool) {
        let paths = vec![
            vec![Tech(0)],
            vec![Tech(1)],
            vec![Tech(0), Tech(1), Tech(2)],
            vec![Tech(1), Tech(3)],
        ];
        // Golden vectors compiled from literal C compound assignments with
        // float wants and int values/goal_values (daitech.c123-153).
        let positive = Self::source_research_scores(&[1.75, -1.5, 7.75, -3.125], &paths)
            == (vec![3, 0, 7, -3], vec![3, 0, 3, -1]);
        let negative = Self::source_research_scores(&[-4.75, 2.5, -7.75, 3.125], &paths)
            == (vec![-6, 1, -7, 3], vec![-6, 1, -4, 2]);
        let precision = Self::source_research_scores(&[16_777_216.0, 0.0, 4.75, 0.75], &paths)
            == (vec![16_777_218, 1, 4, 0], vec![16_777_218, 1, 5_592_407, 0]);
        (positive, negative, precision)
    }

    pub(crate) fn ai_research_switching_playtest(&mut self) -> bool {
        self.units.clear();
        let home = (10, 24);
        for (x, y) in super::terrain_work::neighbors(home.0, home.1).chain([home]) {
            let tile = self.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.river = true;
            tile.road = true;
        }
        self.create_city(1, home.0, home.1);
        self.cities[0].size = 4;
        self.reconcile_city_workers();
        self.players[1].known.clear();
        // Advisor selection can prefer a deep advance which choose_tech will
        // reject. Bring that candidate to the frontier for the switching checks.
        let best = loop {
            self.ai_tech_wants.insert(1, self.advisor_tech_wants(1));
            let (candidate, _, _) = self.select_ai_research(1);
            if self.tech_available(1, candidate) {
                break candidate;
            }
            for tech in self.research_path(1, candidate) {
                if tech != candidate && !self.has_tech(1, tech) {
                    self.players[1].known.push(tech);
                }
            }
        };
        let Some(alternative) = Tech::all().find(|&tech| {
            if tech == best || !self.tech_available(1, tech) || self.target_cost(1, tech) <= 6 {
                return false;
            }
            self.players[1].researching = tech;
            let (_, want, current) = self.select_ai_research(1);
            want - current > 3
        }) else {
            return false;
        };
        self.players[1].researching = alternative;
        let near_completion = self.target_cost(1, alternative) - 1;
        self.players[1].research = near_completion;
        self.manage_ai_research(1);
        let held = self.players[1].researching == alternative
            && self.players[1].research == near_completion;
        self.players[1].research = 1;
        self.manage_ai_research(1);
        let switched = self.players[1].researching == best && self.players[1].research == 0;
        self.players[1].researching = alternative;
        self.players[1].research = 3;
        self.ai_free_bulbs.insert(1, 3);
        self.manage_ai_research(1);
        held && switched && self.players[1].researching == best && self.players[1].research == 3
    }

    pub(crate) fn unset_goal_hud_playtest_setup(&mut self) {
        self.players[HUMAN].research_goal = Tech::BRONZE_WORKING;
        self.players[HUMAN].researching = Tech::ALPHABET;
        self.discover(HUMAN, Tech::BRONZE_WORKING);
    }

    pub(crate) fn native_unset_goal_save_playtest(&self) -> bool {
        let mut game = self.clone();
        let human_goal = game.players[HUMAN].research_goal;
        let command_guard = !game.apply(Command::Research(Tech::UNSET))
            && game.players[HUMAN].research_goal == human_goal;
        game.players[1].research_goal = Tech::UNSET;
        let path = std::env::temp_dir().join(format!(
            "civcraft-unset-goal-{}-{}.save",
            std::process::id(),
            NEXT_RESEARCH_SAVE.fetch_add(1, Ordering::Relaxed)
        ));
        let roundtrip = game
            .save_campaign(&path)
            .and_then(|_| Self::load_campaign(&path))
            .is_ok_and(|loaded| {
                loaded.players[1].research_goal == Tech::UNSET
                    && !loaded.tech_available(1, Tech::UNSET)
                    && loaded.research_target(1) != Tech::UNSET
            });
        game.players[1].researching = Tech::UNSET;
        let invalid_target =
            game.save_campaign(&path).is_ok() && Self::load_campaign(&path).is_err();
        game.players[1].researching = Tech::ALPHABET;
        game.players[1].known.push(Tech::UNSET);
        let invalid_known =
            game.save_campaign(&path).is_ok() && Self::load_campaign(&path).is_err();
        let _ = std::fs::remove_file(path);
        command_guard && roundtrip && invalid_target && invalid_known
    }

    pub(crate) fn native_research_phase_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.ai_tech_wants.clear();
        let initial_zero = Tech::all().all(|tech| game.advisor_tech_want(1, tech) == 0.0);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1.75);
        game.players[1].known.push(Tech::ALPHABET);
        game.players[1].rates = [0, 100, 0];
        let retained = initial_zero && game.advisor_tech_want(1, Tech::ALPHABET) == 1.75;

        game.clear_ai_tech_wants(1);
        game.add_ai_tech_want(1, Tech::ALPHABET, 16_777_216.0);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1.0);
        game.add_ai_tech_want(1, Tech::ALPHABET, 1.0);
        let ordered = game.advisor_tech_want(1, Tech::ALPHABET) == 16_777_216.0;
        game.clear_ai_tech_wants(1);
        let cleared = game.advisor_tech_want(1, Tech::ALPHABET) == 0.0;

        game.units.clear();
        game.cities.clear();
        for p in [(10, 24), (18, 28), (26, 32)] {
            game.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
            game.create_city(1, p.0, p.1);
        }
        game.add_ai_tech_want(1, Tech::ALPHABET, 42.5);
        let rebel = game.players.len();
        game.split_civil_war(1);
        let copied = game.players.len() == rebel + 1
            && game.advisor_tech_want(rebel, Tech::ALPHABET) == 42.5;
        game.add_ai_tech_want(rebel, Tech::ALPHABET, 1.0);
        let independent = game.advisor_tech_want(1, Tech::ALPHABET) == 42.5;

        let path = std::env::temp_dir().join(format!(
            "civcraft-research-phase-{}-{}.save",
            std::process::id(),
            NEXT_RESEARCH_SAVE.fetch_add(1, Ordering::Relaxed)
        ));
        let loaded_zero = game
            .save_campaign(&path)
            .and_then(|_| Self::load_campaign(&path))
            .is_ok_and(|loaded| {
                loaded.ai_tech_wants.is_empty()
                    && Tech::all().all(|tech| loaded.advisor_tech_want(1, tech) == 0.0)
            });
        let _ = std::fs::remove_file(path);
        (
            retained,
            ordered && cleared,
            copied && independent,
            loaded_zero,
        )
    }
}
