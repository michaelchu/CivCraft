use super::*;

impl Playtest {
    pub(super) fn advance_naval_ai_ui(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
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
        if frame == 361 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (merchant, home, destination) = game.sim.ai_merchant_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let delivered = game.sim.merchant_playtest_route_exists(home, destination)
                && game.sim.units().iter().all(|unit| unit.id != merchant);
            self.results.push(("ai_merchant", delivered,
                format!("AI Freight delivered goods and established a route during End Turn: {delivered}.")));
        }
        if frame == 364 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_merchant_production", sim.ai_trade_production_gate_playtest(),
                "Civ2Civ3's disabled national trade suppresses the reference regular caravan production advisor, even with five cities and a foreign ally.".to_owned()));
        }
        if frame == 367 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (merchant, ferry, home, destination) = game.sim.ai_merchant_ferry_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let delivered = game.sim.merchant_playtest_route_exists(home, destination)
                && game.sim.units().iter().all(|unit| unit.id != merchant)
                && game.sim.units().iter().any(|unit| unit.id == ferry);
            self.results.push(("ai_merchant_ferry", delivered,
                format!("An AI Galleon ferried Freight between islands and established a trade route: {delivered}.")));
        }
        if frame == 370 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (merchant, ferry, home, destination) = game.sim.ai_ferry_dispatch_playtest_setup();
            let position = |game: &Game, id| {
                game.sim
                    .units()
                    .iter()
                    .find(|unit| unit.id == id)
                    .map(|unit| (unit.x, unit.y))
            };
            let waiting = position(game, merchant);
            let initial = position(game, ferry);
            game.perform_action(UiAction::EndTurn);
            let dispatched =
                position(game, ferry) != initial && position(game, merchant) == waiting;
            for _ in 0..4 {
                if game.sim.merchant_playtest_route_exists(home, destination) {
                    break;
                }
                game.perform_action(UiAction::EndTurn);
            }
            let delivered = game.sim.merchant_playtest_route_exists(home, destination);
            self.results.push(("ai_ferry_dispatch", dispatched && delivered,
                format!("Freight waited while a distant ferry sailed to pick it up: {dispatched}. The delivery established a route: {delivered}.")));
        }
        if frame == 373 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (_, ferry, home, destination) = game.sim.ai_inland_ferry_playtest_setup();
            for _ in 0..4 {
                game.perform_action(UiAction::EndTurn);
                if game.sim.merchant_playtest_route_exists(home, destination) {
                    break;
                }
            }
            let delivered = game.sim.merchant_playtest_route_exists(home, destination);
            let at_sea =
                game.sim.units().iter().any(|unit| {
                    unit.id == ferry && game.sim.tile(unit.x, unit.y).terrain.is_water()
                });
            self.results.push(("ai_inland_ferry", delivered && at_sea,
                format!("Freight established a route with an inland city after landing: {delivered}. Ferry stayed at sea: {at_sea}.")));
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
            let (theatre, crusade, obsolete) = game.sim.theatre_and_crusade_playtest();
            self.results.push(("theatre_and_crusade", theatre && crusade && obsolete,
                format!("Theatre respected local, empire and enduring specialist effects: {theatre}. Crusade reduced raw military unhappiness by one and removed this gold upkeep: {crusade}. Communism ended both effects: {obsolete}.")));
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
        if frame == 391 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (spy, city) = game.sim.poison_playtest_setup();
            game.perform_action(UiAction::SelectUnit(spy));
            game.perform_action(UiAction::SpyTargets(Some((city, 0))));
        }
        if frame == 393 && self.act_passed {
            let poison = buttons.iter().find_map(|(_, action)| match action {
                UiAction::PoisonCity(city) => Some((*action, *city)),
                _ => None,
            });
            let passed = poison.is_some_and(|(action, target)| {
                game.perform_action(action);
                game.sim
                    .cities()
                    .iter()
                    .any(|city| city.id == target && city.size == 1 && city.food == 5)
            });
            self.results.push((
                "spy_poison",
                passed,
                "The Spy target menu reduced the city to size one and preserved its food stock."
                    .to_owned(),
            ));
        }
        if frame == 396 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (spy, _) = game.sim.sabotage_unit_playtest_setup();
            game.perform_action(UiAction::SelectUnit(spy));
        }
        if frame == 398 && self.act_passed {
            let spy = game.selected_unit;
            let sabotage = buttons.iter().find_map(|(_, action)| match action {
                UiAction::SabotageUnit(target) => Some((*action, *target)),
                _ => None,
            });
            let passed = sabotage.is_some_and(|(action, target)| {
                game.perform_action(action);
                game.sim
                    .units()
                    .iter()
                    .any(|unit| unit.id == target && unit.hp == 1)
                    && game
                        .sim
                        .units()
                        .iter()
                        .any(|unit| Some(unit.id) == spy && unit.moves_left == 0)
                    && !game.sim.apply(Command::SabotageUnit(spy.unwrap(), target))
            });
            self.results.push(("spy_sabotage_unit", passed,
                "The Spy used the HUD action, halved a Warrior's three HP to one, and escaped with no moves left.".to_owned()));
        }
        if frame == 401 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (actor, _, _) = game.sim.incite_playtest_setup();
            game.perform_action(UiAction::SelectUnit(actor));
        }
        if frame == 403 && self.act_passed {
            let incite = buttons.iter().find_map(|(_, action)| match action {
                UiAction::InciteCity(city, price) => Some((*action, *city, *price)),
                _ => None,
            });
            let actor = game.selected_unit;
            let passed = incite.is_some_and(|(action, target, price)| {
                let gold = game.sim.players()[HUMAN].gold;
                game.perform_action(action);
                game.sim.players()[HUMAN].gold == gold - price
                    && game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY)
                    && game.sim.units().iter().all(|unit| Some(unit.id) != actor)
                    && game
                        .sim
                        .units()
                        .iter()
                        .any(|unit| unit.owner == HUMAN && unit.home_city == Some(target))
                    && game.sim.cities().iter().any(|city| {
                        city.id == target
                            && city.owner == HUMAN
                            && city.size == 1
                            && city.shields == 0
                            && city.food == 5
                    })
            });
            self.results.push(("incite_city", passed,
                "The Diplomat paid the quoted price, gained a city and its garrison, and stole Pottery. The revolt reduced population and cleared shields.".to_owned()));
        }
    }
}
