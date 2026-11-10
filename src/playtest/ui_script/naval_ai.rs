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
            *game = Game::new(crate::mapgen::generate(42));
            let (ship, cargo) = game.sim.trireme_playtest_setup();
            game.perform_action(UiAction::SelectUnit(cargo));
            game.perform_action(UiAction::BoardOrDeboard);
            game.perform_action(UiAction::SelectUnit(ship));
            let sailed = game.sim.apply(Command::Move(ship, 1, 0));
            let boarded = game.sim.cargo_count(ship) == 1;
            let blocked = !game.sim.apply(Command::Move(ship, 1, 0));
            self.results.push(("trireme", sailed && boarded && blocked,
                format!("Trireme traveled along the river: {sailed}. Cargo stayed aboard: {boarded}. Deep ocean entry was blocked: {blocked}.")));
        }
        if frame == 349 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (production, upgrade, shore_attack) = game.sim.surface_warship_playtest();
            self.results.push(("surface_warships", production && upgrade && shore_attack,
                format!("Port production and obsolescence: {production}. Frigate can upgrade directly to Destroyer: {upgrade}. Battleship defeated a shore unit while staying at sea: {shore_attack}.")));
        }
        if frame == 352 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (submarine, missile, target, doubled) = game.sim.submarine_playtest_setup();
            game.perform_action(UiAction::SelectUnit(missile));
            game.perform_action(UiAction::BoardOrDeboard);
            let loaded = game.sim.cargo_count(submarine) == 1;
            let launched = game.sim.apply(Command::Move(missile, 1, 0))
                && game
                    .sim
                    .units()
                    .iter()
                    .all(|unit| unit.id != target && unit.id != missile);
            self.results.push(("submarine", loaded && launched && doubled,
                format!("Cruise Missile boarded the Submarine: {loaded}. Its launch destroyed the target: {launched}. Destroyer defense doubled against a Submarine: {doubled}.")));
        }
        if frame == 355 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (submarine, missile, target) = game.sim.ai_submarine_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let launched = game
                .sim
                .units()
                .iter()
                .all(|unit| unit.id != missile && unit.id != target)
                && game.sim.units().iter().any(|unit| unit.id == submarine);
            self.results.push(("ai_submarine", launched,
                format!("AI Submarine launched a Cruise Missile and destroyed the enemy Carrier: {launched}.")));
        }
        if frame == 358 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (target, hull, ammo) = game.sim.normal_hunter_production_playtest();
            for (name, passed) in [
                ("native_normal_hunter_worthwhile_target", target),
                ("native_normal_hunter_hull_production_gate", hull),
                ("native_normal_hunter_ammunition_production_gate", ammo),
            ] {
                self.results.push((name, passed, "Normal AI's H_TARGETS handicap suppressed dedicated hunter and ammunition advice despite a worthwhile existing target and eligible units; existing hunter tactics remain enabled.".to_owned()));
            }
        }
                                                if frame == 376 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (stacked, healed, retained) = game.sim.barracks_playtest();
            self.results.push(("barracks", stacked && healed && retained,
                format!("Barracks and Sun Tzu granted two ranks: {stacked}. Barracks fully healed a stationary unit: {healed}. Barracks training remained active after Gunpowder: {retained}.")));
        }
        if frame == 379 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (trade, luxury, obsolete) = game.sim.wonder_output_playtest();
            self.results.push(("wonder_output", trade && luxury && obsolete,
                format!("Colossus increased only trade-producing tiles: {trade}. Luxury bonuses respected city and empire scope and Marketplace: {luxury}. Automobile and Electricity disabled the relevant bonuses: {obsolete}.")));
        }
        if frame == 382 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let theatre = game.sim.theatre_output_playtest();
            self.results.push(("theatre_output", theatre,
                format!("Shakespeare's Theater trade effects followed local, empire, and specialist scope: {theatre}.")));
        }
        if frame == 385 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (active, ended, retained) = game.sim.obsolete_wonders_playtest();
            self.results.push(("obsolete_wonders", active && ended && retained,
                format!("Rival discoveries left wonders active: {active}. Owner discoveries ended movement, defense and science effects: {ended}. Buildings and accumulated history survived obsolescence: {retained}.")));
        }
        if frame == 388 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let passed = game.sim.combat_population_playtest();
            self.results.push(("combat_population", passed,
                "Checked heavy and light attackers, City Walls, empire-wide Great Wall protection, Machine Tools obsolescence, and size-one immunity.".to_owned()));
        }
                                                    }
}
