use super::*;

impl Playtest {
    pub(super) fn advance_combat_ui(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
        if frame == 252 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let home = game.sim.veteran_playtest_setup();
            game.perform_action(UiAction::EndTurn);
            let trained = game
                .sim
                .units()
                .iter()
                .find(|unit| {
                    unit.home_city == Some(home)
                        && unit.kind == UnitKind::Warrior
                        && unit.veteran == 1
                })
                .map(|unit| unit.id);
            if let Some(id) = trained {
                game.perform_action(UiAction::SelectUnit(id));
            }
            self.results.push(("veterancy", trained.is_some(),
                format!("A city with Barracks completed a veteran Warrior through the end-turn control: {}.", trained.is_some())));
        }
        if frame == 256 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let warrior = game.sim.upgrade_playtest_setup();
            game.perform_action(UiAction::SelectUnit(warrior));
        }
        if frame == 258 && self.act_passed {
            let gold = game.sim.players()[HUMAN].gold;
            let upgrade = buttons.iter().find_map(|(_, action)| match action {
                UiAction::UpgradeUnit(UnitKind::Musketeers, price) => Some((*action, *price)),
                _ => None,
            });
            let upgraded = upgrade.is_some_and(|(action, price)| {
                game.perform_action(action);
                game.sim.players()[HUMAN].gold == gold - price
                    && game.selected_unit.is_some_and(|id| {
                        game.sim.units().iter().any(|unit| {
                            unit.id == id && unit.kind == UnitKind::Musketeers && unit.veteran == 1
                        })
                    })
            });
            self.results.push(("unit_upgrade", upgraded,
                format!("Paid the displayed upgrade price and converted a hardened Warrior into veteran Musketeers: {upgraded}.")));
        }
        if frame == 261 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let engineer = game.sim.terrain_work_playtest_setup();
            game.perform_action(UiAction::SelectUnit(engineer));
        }
        if frame == 263 && self.act_passed {
            let action = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::TerrainWork(sim::WorkKind::Transform))
            });
            let promoted = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                game.sim.work_promotion_playtest_tick();
                game.selected_unit.is_some_and(|id| {
                    game.sim.units().iter().any(|unit| {
                        unit.id == id && unit.veteran == 1 && unit.work_turns_left() == 8
                    })
                })
            });
            self.results.push(("engineer_promotion", promoted,
                format!("Engineer promoted while transforming terrain and reduced the remaining estimate to eight turns: {promoted}.")));
        }
        if frame == 267 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let team = game.sim.cooperative_work_playtest_setup();
            self.work_team = Some(team);
            game.perform_action(UiAction::SelectUnit(team.0));
        }
        if frame == 269
            && let Some((_, second)) = self.work_team
        {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Irrigate))
            {
                game.perform_action(*action);
            }
            game.perform_action(UiAction::SelectUnit(second));
        }
        if frame == 271
            && let Some((first, second)) = self.work_team
        {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Irrigate))
            {
                game.perform_action(*action);
            }
            let site = game
                .sim
                .units()
                .iter()
                .find(|unit| unit.id == first)
                .filter(|unit| game.sim.project_turns_left(unit) == 3)
                .map(|unit| (unit.x, unit.y));
            let completed = site.is_some_and(|(x, y)| {
                (0..3).all(|_| game.sim.apply(Command::EndTurn))
                    && game.sim.tile(x, y).irrigation
                    && [first, second].into_iter().all(|id| {
                        game.sim
                            .units()
                            .iter()
                            .any(|unit| unit.id == id && unit.work_kind.is_none())
                    })
            });
            self.results.push(("cooperative_work", completed,
                format!("Two Workers completed the same five-turn irrigation project in three turns: {completed}.")));
        }
        if frame == 275 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (bomber, targets) = game.sim.bombard_playtest_setup();
            game.perform_action(UiAction::SelectUnit(bomber));
            let bombarded = game.sim.apply(Command::Move(bomber, 1, 0))
                && targets.into_iter().all(|id| {
                    game.sim
                        .units()
                        .iter()
                        .any(|unit| unit.id == id && unit.hp == 7)
                })
                && game
                    .sim
                    .units()
                    .iter()
                    .any(|unit| unit.id == bomber && unit.moves_left == 0 && unit.hp == 30);
            self.results.push(("bombardment", bombarded,
                format!("Bomber hit both ground targets without retaliation and spent its remaining movement: {bombarded}.")));
        }
        if frame == 279 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (fighter, bomber) = game.sim.fighter_playtest_setup();
            game.perform_action(UiAction::SelectUnit(fighter));
            let intercepted =
                game.sim.apply(Command::Move(fighter, 1, 0))
                    && game.sim.units().iter().all(|unit| unit.id != bomber)
                    && game.sim.units().iter().any(|unit| {
                        unit.id == fighter && unit.moves_left == 9 * sim::MOVE_FRAGMENTS
                    });
            self.results.push(("fighter_interception", intercepted,
                format!("Fighter attacked and destroyed an airborne Bomber while retaining its unused movement: {intercepted}.")));
        }
        if frame == 283 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (carrier, fighter) = game.sim.carrier_playtest_setup();
            game.perform_action(UiAction::SelectUnit(fighter));
            let landed =
                game.sim.apply(Command::Move(fighter, 1, 0)) && game.sim.cargo_count(carrier) == 1;
            let sailed = game.sim.apply(Command::Move(carrier, 1, 0));
            game.perform_action(UiAction::EndTurn);
            let fueled = game.sim.units().iter().any(|unit| {
                unit.id == fighter && unit.transport == Some(carrier) && unit.fuel == 2
            });
            game.perform_action(UiAction::SelectUnit(fighter));
            let launched =
                game.sim.apply(Command::Move(fighter, 1, 0)) && game.sim.cargo_count(carrier) == 0;
            self.results.push(("carrier_operations", landed && sailed && fueled && launched,
                format!("Fighter landed: {landed}. Carrier sailed: {sailed}. Fighter refueled: {fueled}. Fighter launched: {launched}.")));
        }
        if frame == 287 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let scenario = game.sim.stealth_aircraft_playtest_setup();
            self.stealth_aircraft = Some(scenario);
            game.perform_action(UiAction::SelectUnit(scenario.0));
        }
        if frame == 289
            && let Some((bomber, target)) = self.stealth_aircraft
        {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::UpgradeUnit(UnitKind::StealthBomber, _))
            }) {
                game.perform_action(*action);
            }
            let upgraded = game
                .sim
                .units()
                .iter()
                .any(|unit| unit.id == bomber && unit.kind == UnitKind::StealthBomber);
            let bombarded = upgraded
                && game.sim.apply(Command::Move(bomber, 1, 0))
                && game
                    .sim
                    .units()
                    .iter()
                    .any(|unit| unit.id == target && unit.hp == 6);
            self.results.push(("stealth_aircraft", upgraded && bombarded,
                format!("Upgraded to a Stealth Bomber through the unit control: {upgraded}. Its four bombardment strikes landed: {bombarded}.")));
        }
        if frame == 290 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (helicopter, passenger) = game.sim.helicopter_playtest_setup();
            game.perform_action(UiAction::SelectUnit(passenger));
            game.perform_action(UiAction::BoardOrDeboard);
            let boarded = game.sim.cargo_count(helicopter) == 1;
            game.perform_action(UiAction::SelectUnit(helicopter));
            let flew = game.sim.apply(Command::Move(helicopter, 1, 0));
            game.perform_action(UiAction::SelectUnit(passenger));
            game.perform_action(UiAction::BoardOrDeboard);
            let stayed_aboard = game.sim.cargo_count(helicopter) == 1;
            game.perform_action(UiAction::EndTurn);
            let took_damage = game
                .sim
                .units()
                .iter()
                .any(|unit| unit.id == helicopter && unit.hp == 27 && unit.fuel == 0);
            self.results.push(("helicopter", boarded && flew && stayed_aboard && took_damage,
                format!("Boarded in city: {boarded}. Flew: {flew}. Field unloading was blocked: {stayed_aboard}. Lost three HP and remained airborne: {took_damage}.")));
        }
        if frame == 292 && self.act_passed {
            let mut remaining_hp = [0; 2];
            for (index, protected) in [false, true].into_iter().enumerate() {
                *game = Game::new(crate::mapgen::generate(42));
                let (bomber, defender) = game.sim.sam_battery_playtest_setup(protected);
                game.perform_action(UiAction::SelectUnit(bomber));
                if game.sim.apply(Command::Move(bomber, 1, 0)) {
                    remaining_hp[index] = game
                        .sim
                        .units()
                        .iter()
                        .find(|unit| unit.id == defender)
                        .unwrap()
                        .hp;
                }
            }
            self.results.push(("sam_battery", remaining_hp == [17, 20],
                format!("Musketeer HP after three bombardment rounds without and with a SAM Battery: {remaining_hp:?}.")));
        }
        if frame == 294 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (helicopter, cruiser) = game.sim.aegis_playtest_setup();
            game.perform_action(UiAction::SelectUnit(helicopter));
            let attacked = game.sim.apply(Command::Move(helicopter, 1, 0));
            let repelled = game.sim.units().iter().all(|unit| unit.id != helicopter)
                && game.sim.units().iter().any(|unit| unit.id == cruiser);
            self.results.push((
                "aegis_cruiser",
                attacked && repelled,
                format!(
                    "Helicopter attacked at sea: {attacked}. AEGIS Cruiser repelled it: {repelled}."
                ),
            ));
        }
        if frame == 296 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (carrier, missile, target) = game.sim.cruise_missile_playtest_setup();
            game.perform_action(UiAction::SelectUnit(missile));
            game.perform_action(UiAction::BoardOrDeboard);
            let boarded = game.sim.cargo_count(carrier) == 1;
            let launched = game.sim.apply(Command::Move(missile, 1, 0));
            let resolved = game
                .sim
                .units()
                .iter()
                .all(|unit| unit.id != missile && unit.id != target)
                && game.sim.units().iter().any(|unit| unit.id == carrier);
            self.results.push(("cruise_missile", boarded && launched && resolved,
                format!("Missile boarded: {boarded}. Launched from carrier: {launched}. Target destroyed and missile expended: {resolved}.")));
        }
        if frame == 298 && self.act_passed {
            let mut outcomes = [false; 2];
            for (index, intercepted) in [false, true].into_iter().enumerate() {
                *game = Game::new(crate::mapgen::generate(42));
                let (missile, defender, city) = game.sim.nuclear_playtest_setup(intercepted);
                game.perform_action(UiAction::SelectUnit(missile));
                let launched = game.sim.apply(Command::Move(missile, 1, 0));
                outcomes[index] = launched
                    && game.sim.units().iter().all(|unit| unit.id != missile)
                    && game.sim.units().iter().any(|unit| unit.id == defender) == intercepted
                    && game.sim.cities().iter().any(|target| {
                        target.id == city && target.size == if intercepted { 4 } else { 2 }
                    });
            }
            self.results.push(("nuclear_strike", outcomes == [true, true],
                format!("Strike destroyed the garrison and halved the city's population: {}. Nearby SDI intercepted the other strike: {}.", outcomes[0], outcomes[1])));
        }
        if frame == 300 && self.act_passed {
            let mut outcomes = [false; 2];
            for (index, warming) in [true, false].into_iter().enumerate() {
                *game = Game::new(crate::mapgen::generate(42));
                game.perform_action(UiAction::FoundCity);
                game.sim.climate_playtest_setup(warming);
                game.perform_action(UiAction::EndTurn);
                let event = if warming {
                    "Global warming has occurred."
                } else {
                    "Nuclear winter has occurred."
                };
                outcomes[index] = game.sim.events().iter().any(|text| text == event)
                    && (0..crate::sim::topology::MAP_H).any(|y| {
                        (0..crate::sim::topology::MAP_W)
                            .any(|x| game.sim.tile(x, y).terrain == crate::sim::Terrain::Desert)
                    });
            }
            self.results.push((
                "climate_change",
                outcomes == [true, true],
                format!(
                    "Pollution triggered global warming: {}. Fallout triggered nuclear winter: {}.",
                    outcomes[0], outcomes[1]
                ),
            ));
        }
    }
}
