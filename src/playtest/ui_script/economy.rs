use super::*;

impl Playtest {
    pub(super) fn advance_economy_ui(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 302 && self.act_passed {
            let mut outcomes = [false; 2];
            for (index, nuclear) in [false, true].into_iter().enumerate() {
                *game = Game::new(crate::mapgen::generate(42));
                game.perform_action(UiAction::FoundCity);
                outcomes[index] = game.sim.run_disaster_playtest(nuclear);
            }
            self.results.push(("city_disasters", outcomes == [true, true],
                format!("Industrial accident reduced population and left pollution: {}. Nuclear accident reduced population and left fallout: {}.", outcomes[0], outcomes[1])));
        }
        if frame == 304 && self.act_passed {
            let mut sizes = [0; 2];
            for (index, protected) in [false, true].into_iter().enumerate() {
                *game = Game::new(crate::mapgen::generate(42));
                game.perform_action(UiAction::FoundCity);
                sizes[index] = game.sim.run_plague_playtest(protected);
            }
            self.results.push((
                "city_plague",
                sizes == [8, 9],
                format!(
                    "City size after the same plague roll without and with an Aqueduct: {sizes:?}."
                ),
            ));
        }
        if frame == 306 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let scenario = game.sim.merchant_playtest_setup();
            self.merchant = Some(scenario);
            game.perform_action(UiAction::SelectUnit(scenario.0));
        }
        if frame == 307
            && let Some((_, freight, _, destination)) = self.merchant
        {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::DeliverGoods(city) if *city == destination)
            }) {
                game.perform_action(*action);
            }
            game.perform_action(UiAction::SelectUnit(freight));
        }
        if frame == 308
            && let Some((caravan, freight, home, destination)) = self.merchant
        {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::HelpWonder(city) if *city == home))
            {
                game.perform_action(*action);
            }
            let established = game.sim.merchant_playtest_route_exists(home, destination)
                && game.sim.units().iter().all(|unit| unit.id != caravan);
            let contributed = game
                .sim
                .cities()
                .iter()
                .any(|city| city.id == home && city.shields == 50)
                && game.sim.units().iter().all(|unit| unit.id != freight);
            self.results.push(("merchant", established && contributed,
                format!("Caravan established a trade route: {established}. Freight added 50 shields to a wonder: {contributed}.")));
        }
        if frame == 310 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (warrior, merchant, home) = game.sim.merchant_capture_playtest_setup();
            game.perform_action(UiAction::SelectUnit(warrior));
            let capture = game.sim.apply(Command::Move(warrior, 1, 0));
            let transferred = game.sim.units().iter().all(|unit| unit.id != merchant)
                && game.sim.units().iter().any(|unit| {
                    unit.kind == UnitKind::Freight
                        && unit.owner == HUMAN
                        && unit.home_city == Some(home)
                        && unit.hp == 7
                });
            self.results.push(("merchant_capture", capture && transferred,
                format!("Warrior captured the unescorted Freight: {capture}. Freight kept its HP and gained the Warrior’s home city: {transferred}.")));
        }
        if frame == 312 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (warrior, _, city) = game.sim.migrants_playtest_setup();
            if game.sim.apply(Command::Move(warrior, 1, 0))
                && let Some(unit) = game
                    .sim
                    .units()
                    .iter()
                    .find(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Migrants)
            {
                let id = unit.id;
                self.migrants = Some((id, city));
                game.perform_action(UiAction::SelectUnit(id));
            }
        }
        if frame == 313 && self.act_passed {
            let joined = self.migrants.is_some_and(|(migrants, city)| {
                if let Some((_, action)) = buttons.iter().find(
                    |(_, action)| matches!(action, UiAction::JoinCity(target) if *target == city),
                ) {
                    game.perform_action(*action);
                }
                game.sim.units().iter().all(|unit| unit.id != migrants)
                    && game
                        .sim
                        .cities()
                        .iter()
                        .any(|target| target.id == city && target.size == 2)
            });
            self.results.push(("migrants", joined, format!("Captured Migrants joined the city through the action strip and added one citizen: {joined}.")));
        }
        if frame == 315 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let scenario = game.sim.airlift_playtest_setup();
            self.airlift = Some(scenario);
            game.perform_action(UiAction::SelectUnit(scenario.0));
        }
        if frame == 316
            && let Some((first, second, destination)) = self.airlift
        {
            if let Some((_, action)) = buttons.iter().find(
                |(_, action)| matches!(action, UiAction::Airlift(city) if *city == destination),
            ) {
                game.perform_action(*action);
            }
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.id == destination)
                .unwrap();
            let arrived = game.sim.units().iter().any(|unit| {
                unit.id == first && (unit.x, unit.y) == (city.x, city.y) && unit.moves_left == 0
            });
            let limited = !game.sim.apply(Command::Airlift(second, destination));
            self.results.push(("airlift", arrived && limited,
                format!("Warrior airlifted to a city without an Airport: {arrived}. Second departure from the same Airport was blocked: {limited}.")));
        }
        if frame == 318 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            game.perform_action(UiAction::CloseCity);
            game.sim.spaceship_playtest_setup();
        }
        if frame == 319
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleSpaceship))
        {
            game.perform_action(*action);
        }
        if frame == 320 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::LaunchSpaceship))
            {
                game.perform_action(*action);
            }
            let launched = game
                .sim
                .spaceship_status(HUMAN)
                .is_some_and(|status| status.starts_with("Spaceship arrives"));
            game.sim.spaceship_playtest_finish_flight();
            let won = game
                .sim
                .game_over()
                .is_some_and(|message| message.contains("Alpha Centauri"));
            self.results.push((
                "space_race",
                launched && won,
                format!(
                    "Spaceship launched through the HUD: {launched}. Arrival ended the game: {won}."
                ),
            ));
        }
        if frame == 321 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            game.perform_action(UiAction::CloseCity);
            game.sim.manual_spaceship_playtest_setup();
            game.perform_action(UiAction::ToggleSpaceship);
        }
        if frame == 322
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleManualAssembly))
        {
            game.perform_action(*action);
            game.sim.manual_spaceship_playtest_build_structural();
        }
        if frame == 323 && self.act_passed {
            let root = crate::sim::ShipPart::Structural(0);
            let child = crate::sim::ShipPart::Structural(1);
            let offered = |part| {
                buttons.iter().find(|(_, action)| matches!(action, UiAction::PlaceShipPart(candidate) if *candidate == part))
            };
            let blocked = offered(child).is_none();
            if let Some((_, action)) = offered(root) {
                game.perform_action(*action);
            }
            let (unplaced, structure, _, _, manual) = game.sim.ship_assembly();
            let installed = manual && structure[0] && unplaced[0] == 0;
            game.perform_action(UiAction::ToggleSpaceship);
            self.results.push(("manual_spaceship", blocked && installed,
                format!("Disconnected structural slot was unavailable: {blocked}. The assembly control installed the root slot: {installed}.")));
        }
        if frame == 325 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            game.perform_action(UiAction::CloseCity);
            game.sim.turn_limit_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let ranked = game
                .sim
                .game_over()
                .is_some_and(|message| message.starts_with("Turn limit reached"))
                && game
                    .sim
                    .score_standings()
                    .first()
                    .is_some_and(|(owner, _)| *owner == HUMAN);
            self.results.push(("score_endgame", ranked, format!("The final turn ended the game and ranked the highest-scoring surviving civilization first: {ranked}.")));
        }
        if frame == 328 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            game.perform_action(UiAction::CloseCity);
            let bonus = game.sim.achievement_playtest_setup();
            let mapped = game
                .sim
                .events()
                .iter()
                .any(|event| event.contains("Entire Map Known"));
            self.results.push(("achievements", mapped && bonus >= 20,
                format!("Apollo revealed the map and earned the mapping achievement: {mapped}. Score bonus from newly earned achievements: {bonus} points.")));
        }
        if frame == 331 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (unit, city) = game.sim.multicultural_playtest_setup();
            game.perform_action(UiAction::SelectUnit(unit));
            let captured = game.sim.apply(Command::Move(unit, 1, 0))
                && game.sim.cities().iter().any(|target| {
                    target.id == city
                        && target.owner == HUMAN
                        && target.size == 2
                        && target.domestic_citizens() == 0
                });
            game.perform_action(UiAction::EndTurn);
            let awarded = game
                .sim
                .events()
                .iter()
                .any(|event| event.contains("Multicultural"));
            self.results.push(("citizen_nationality", captured && awarded,
                format!("Captured city retained its two surviving foreign citizens: {captured}. Multicultural was awarded: {awarded}.")));
        }
        if frame == 334 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (unit, city) = game.sim.partisan_playtest_setup();
            game.perform_action(UiAction::SelectUnit(unit));
            let captured = game.sim.apply(Command::Move(unit, 1, 0))
                && game
                    .sim
                    .cities()
                    .iter()
                    .any(|target| target.id == city && target.owner == HUMAN);
            let resistance = game
                .sim
                .units()
                .iter()
                .filter(|unit| {
                    unit.owner == 1
                        && unit.kind == UnitKind::Partisan
                        && unit.state == sim::UnitState::Fortified
                })
                .count();
            self.results.push(("partisan_resistance", captured && resistance > 0,
                format!("City captured: {captured}. Fortified Partisans spawned for the former owner: {resistance}.")));
        }
        if frame == 337 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (before, after, history) = game.sim.amphitheater_playtest();
            self.results.push(("amphitheater", before && !after && history == 2,
                format!("Electricity let an Amphitheater end disorder in a size-eight city: {}. History per turn: {history}.", before && !after)));
        }
        if frame == 340 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (advance, host, rival) = game.sim.science_wonder_playtest();
            self.results.push(("science_wonders", advance && host == 28 && rival == 20,
                format!("Great Library granted a free advance: {advance}. Host and rival science output with Libraries: {host} and {rival}.")));
        }
        if frame == 343 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (hp, docked_moves, sea_moves, rank) = game.sim.naval_bonus_playtest();
            self.results.push(("naval_bonuses", (hp, docked_moves, sea_moves, rank) == (30, 42, 27, 1),
                format!("Port repaired the cruiser to {hp} HP and grants build rank {rank}. Repaired and damaged ships received {docked_moves} and {sea_moves} movement fragments.")));
        }
    }
}
