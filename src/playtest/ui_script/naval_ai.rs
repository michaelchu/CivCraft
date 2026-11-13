//! Implements naval ai behavior within the ui script subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_naval_ai_ui(
        &mut self,
        frame: u32,
        game: &mut Game,
        _buttons: &[(Rect, UiAction)],
    ) {
        if frame == 346 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (ship, cargo) = game.sim.playtest_harness().trireme_playtest_setup();
            game.perform_action(UiAction::SelectUnit(cargo));
            game.perform_action(UiAction::BoardOrDeboard);
            game.perform_action(UiAction::SelectUnit(ship));
            let sailed = game.sim.playtest_harness().apply(Command::Move(ship, 1, 0));
            let boarded = game.sim.playtest_state().cargo_count(ship) == 1;
            let blocked = !game.sim.playtest_harness().apply(Command::Move(ship, 1, 0));
            self.results.push(("trireme", sailed && boarded && blocked,
                format!("Trireme traveled along the river: {sailed}. Cargo stayed aboard: {boarded}. Deep ocean entry was blocked: {blocked}.")));
        }
        if frame == 349 && self.act_passed {
            super::reset_founded_city_scenario(game);
            let (production, upgrade, shore_attack) =
                game.sim.playtest_harness().surface_warship_playtest();
            self.results.push(("surface_warships", production && upgrade && shore_attack,
                format!("Civ III ship requirements and upgrade chain: {production}. Frigate has no catalog upgrade and Ironclad upgrades to Destroyer: {upgrade}. Battleship defeated a shore unit while staying at sea: {shore_attack}.")));
        }
        if frame == 352 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (submarine, missile, _target, doubled) =
                game.sim.playtest_harness().submarine_playtest_setup();
            let allowed = game
                .sim
                .playtest_harness()
                .apply(Command::BoardOrDeboard(missile))
                && game.sim.playtest_state().cargo_count(submarine) == 1;
            self.results.push(("submarine", allowed && doubled,
                format!("C2C3 Submarine accepted Cruise Missile cargo: {allowed}. Destroyer defense doubled against a Submarine: {doubled}.")));
        }
        if frame == 355 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (submarine, missile, target) =
                game.sim.playtest_harness().ai_submarine_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let launched = game
                .sim
                .playtest_state()
                .units()
                .iter()
                .all(|unit| unit.id != missile && unit.id != target)
                && game
                    .sim
                    .playtest_state()
                    .units()
                    .iter()
                    .any(|unit| unit.id == submarine);
            self.results.push(("ai_submarine", launched,
                format!("AI Submarine launched a Cruise Missile and destroyed the enemy Carrier: {launched}.")));
        }
        if frame == 358 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (target, hull, ammo) = game
                .sim
                .playtest_harness()
                .normal_hunter_production_playtest();
            for (name, passed) in [
                ("native_normal_hunter_worthwhile_target", target),
                ("native_normal_hunter_hull_production_gate", hull),
                ("native_normal_hunter_ammunition_production_gate", ammo),
            ] {
                self.results.push((name, passed, "Normal AI's H_TARGETS handicap suppressed dedicated hunter and ammunition advice despite a worthwhile existing target and eligible units; existing hunter tactics remain enabled.".to_owned()));
            }
        }
        if frame == 376 && self.act_passed {
            super::reset_founded_city_scenario(game);
            let (stacked, healed, retained) = game.sim.playtest_harness().barracks_playtest();
            self.results.push(("barracks", stacked && healed && retained,
                format!("Barracks and Sun Tzu granted two ranks: {stacked}. Barracks fully healed a stationary unit: {healed}. Barracks training remained active after Gunpowder: {retained}.")));
        }
        if frame == 379 && self.act_passed {
            super::reset_founded_city_scenario(game);
            let (trade, luxury, colossus_obsolete, gardens_obsolete) =
                game.sim.playtest_harness().wonder_output_playtest();
            self.results.push(("wonder_output", trade && luxury && colossus_obsolete && gardens_obsolete,
                format!("Colossus increased only trade-producing tiles: {trade}. Luxury bonuses respected city and empire scope and Marketplace: {luxury}. C2C3 Automobile (Civ III Motorized Transportation) obsoleted Colossus: {colossus_obsolete}. Steam Power obsoleted Hanging Gardens: {gardens_obsolete}.")));
        }
        if frame == 382 && self.act_passed {
            super::reset_founded_city_scenario(game);
            let theatre = game.sim.playtest_harness().theatre_output_playtest();
            self.results.push(("theatre_output", theatre,
                format!("Shakespeare's Theater trade effects followed local, empire, and specialist scope: {theatre}.")));
        }
        if frame == 385 && self.act_passed {
            super::reset_founded_city_scenario(game);
            let (active, ended, retained) = game.sim.playtest_harness().obsolete_wonders_playtest();
            self.results.push(("obsolete_wonders", active && ended && retained,
                format!("Rival technologies left the owner's wonders active: {active}. Owner technologies obsolete the selected wonders and suppress movement and science effects: {ended}. Wonder history accumulated while buildings remained: {retained}.")));
        }
        if frame == 388 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let passed = game.sim.playtest_harness().combat_population_playtest();
            self.results.push(("combat_population", passed,
                "Checked heavy and light attackers, City Walls, empire-wide Great Wall protection, Machine Tools obsolescence, and size-one immunity.".to_owned()));
        }
    }
}
