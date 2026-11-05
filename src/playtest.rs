use crate::{
    client::{Game, UiAction},
    ensure_parent,
    rules::Building,
    sim::{self, Command, HUMAN, Production, UnitKind, WorklistEdit, topology::ISOHEX_DIRS},
};
use macroquad::prelude::Rect;
use std::fs;

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub(crate) struct Playtest {
    path: String,
    move_passed: bool,
    move_detail: String,
    act_passed: bool,
    act_detail: String,
    diplomacy_probe: Option<(u32, i32, i32)>,
    vision_probe: Option<(i32, i32)>,
    stealth_aircraft: Option<(u32, u32)>,
    merchant: Option<(u32, u32, u32, u32)>,
    migrants: Option<(u32, u32)>,
    airlift: Option<(u32, u32, u32)>,
    work_team: Option<(u32, u32)>,
    pillage_probe: Option<(i32, i32)>,
    transport_probe: Option<(u32, u32, (i32, i32))>,
    results: Vec<(&'static str, bool, String)>,
    previous_passed: bool,
    detail_offset: usize,
}

impl Playtest {
    pub(crate) fn new(path: String) -> Self {
        Self {
            path,
            move_passed: false,
            move_detail: "No scripted move completed".to_owned(),
            act_passed: false,
            act_detail: "No scripted action completed".to_owned(),
            diplomacy_probe: None,
            vision_probe: None,
            transport_probe: None,
            pillage_probe: None,
            work_team: None,
            stealth_aircraft: None,
            merchant: None,
            migrants: None,
            airlift: None,
            results: Vec::new(),
            previous_passed: true,
            detail_offset: 0,
        }
    }

    pub(crate) fn advance(&mut self, frame: u32, game: &mut Game, buttons: &[(Rect, UiAction)]) {
        if frame == 12 {
            if let Some(explorer) = game
                .sim
                .units()
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Explorer)
                .cloned()
            {
                for (dx, dy) in ISOHEX_DIRS {
                    if game.sim.apply(Command::Move(explorer.id, dx, dy)) {
                        if let Some(after) = game.sim.units().iter().find(|u| u.id == explorer.id) {
                            let distance = game
                                .sim
                                .map_distance(explorer.x, explorer.y, after.x, after.y);
                            self.move_passed = distance > 0;
                            self.move_detail = format!(
                                "Explorer moved {distance} tile from {},{} to {},{} using terrain movement costs",
                                explorer.x, explorer.y, after.x, after.y
                            );
                        }
                        break;
                    }
                }
            }
        }

        if frame == 30 {
            if let Some(settler_id) = game
                .sim
                .units()
                .iter()
                .find(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers)
                .map(|u| u.id)
            {
                let cities_before = game.sim.cities().len();
                game.selected_unit = Some(settler_id);
                game.perform_action(UiAction::FoundCity);
                let cities_after = game.sim.cities().len();
                self.act_passed = cities_after == cities_before + 1;
                self.act_detail = if self.act_passed {
                    "Settlers founded a city, claimed its workable tiles, and opened city production"
                        .to_owned()
                } else {
                    "Settlers could not found a city at the scripted starting tile".to_owned()
                };
            }
        }

        if frame == 34
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CityTiles))
        {
            game.perform_action(*action);
        }
        if frame == 36
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::WorkCityTile(..)))
        {
            game.perform_action(*action);
        }
        if frame == 38 && self.act_passed {
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let reassigned = city.worked_tiles.is_empty() && city.specialists[2] == 1;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Specialist(_, 2, -1)))
            {
                game.perform_action(*action);
            }
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let restored = city.worked_tiles.len() == 1 && city.specialists[2] == 0;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CityTiles))
            {
                game.perform_action(*action);
            }
            self.results.push(("city_tiles", reassigned && restored,
                format!("Worked tile reassigned to entertainer: {reassigned}. Tile labor restored: {restored}.")));
        }

        if frame == 45 && self.act_passed {
            if let Some((_, rate_button)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::RaiseRate(1)))
            {
                game.perform_action(*rate_button);
            }
            let rate_changed = game.sim.players()[HUMAN].rates == [50, 50, 0];
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let trade = game.sim.city_yields(city).2;
            let [science, gold, _] = game.sim.city_trade(city, trade);
            let player = &game.sim.players()[HUMAN];
            let expected_gold = player.gold + gold - game.sim.building_upkeep(city);
            let expected_research = player.research + science;
            game.sim.apply(Command::EndTurn);
            let player = &game.sim.players()[HUMAN];
            self.act_passed = rate_changed
                && player.gold == expected_gold
                && player.research == expected_research;
            self.act_detail.push_str(&format!(
                ". Used the tax control and ended the turn. Treasury {} (expected {expected_gold}), research {} (expected {expected_research}).",
                player.gold, player.research
            ));
        }

        if frame == 55 && self.act_passed {
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let city_id = city.id;
            let before = game.sim.city_yields(city);
            if let Some((_, action)) = buttons.iter().find(
                |(_, action)| matches!(action, UiAction::Specialist(id, 0, 1) if *id == city_id),
            ) {
                game.perform_action(*action);
            }
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.id == city_id)
                .unwrap();
            let after = game.sim.city_yields(city);
            let science = game.sim.city_trade(city, 0)[0];
            self.act_passed = city.specialists == [1, 0, 0] && before != after && science == 2;
            self.act_detail.push_str(&format!(
                " Scientist replaced a tile worker and provided {science} science."
            ));
        }

        if frame == 65 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Specialist(_, 0, -1)))
            {
                game.perform_action(*action);
            }
            let mut verified = false;
            for _ in 0..12 {
                let Some(city) = game.sim.cities().iter().find(|city| city.owner == HUMAN) else {
                    break;
                };
                let id = city.id;
                let support = game.sim.city_unit_upkeep(city);
                if support.shields == 1 {
                    let (_, gross, trade) = game.sim.city_yields(city);
                    let luxury = game.sim.city_trade(city, trade)[2];
                    let expected = (city.shields + gross - 1) % city.production.cost();
                    let disorder = game.sim.city_in_disorder(city, luxury);
                    game.sim.apply(Command::EndTurn);
                    verified = !disorder
                        && game
                            .sim
                            .cities()
                            .iter()
                            .any(|city| city.id == id && city.shields == expected);
                    break;
                }
                game.sim.apply(Command::EndTurn);
            }
            self.act_passed = verified;
            self.act_detail.push_str(&format!(
                " Third supported unit charged one shield on the next turn: {verified}."
            ));
        }

        if frame == 78 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleGovernment))
            {
                game.perform_action(*action);
            }
        }
        if frame == 83 && self.act_passed {
            use sim::Government;
            let locked = !buttons.iter().any(|(_, action)| {
                matches!(action, UiAction::ChangeGovernment(Government::Monarchy))
            });
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::ChangeGovernment(Government::Despotism))
            }) {
                game.perform_action(*action);
            }
            let player = &game.sim.players()[HUMAN];
            self.act_passed = locked
                && player.government == Government::Anarchy
                && player.revolution_target == Some(Government::Despotism);
            self.act_detail.push_str(&format!(
                " Government panel enforced the tech lock and started Anarchy: {}.",
                self.act_passed
            ));
        }
        if frame == 90 && self.act_passed {
            use sim::Government;
            let finish = game.sim.players()[HUMAN].revolution_turn.unwrap();
            game.perform_action(UiAction::ToggleGovernment);
            while game.sim.turn() < finish && game.sim.apply(Command::EndTurn) {}
            let city = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .unwrap();
            let upkeep = game.sim.city_unit_upkeep(city);
            let [_, gold, luxury] = game.sim.city_trade(city, game.sim.city_yields(city).2);
            let revenue = if game.sim.city_in_disorder(city, luxury) {
                0
            } else {
                gold
            };
            let treasury = game.sim.players()[HUMAN].gold + revenue
                - game.sim.building_upkeep(city)
                - upkeep.gold;
            let adopted = game.sim.players()[HUMAN].government == Government::Despotism;
            game.sim.apply(Command::EndTurn);
            self.act_passed = adopted
                && upkeep.shields == 0
                && upkeep.gold > 0
                && game.sim.players()[HUMAN].gold == treasury;
            self.act_detail.push_str(&format!(
                " Despotism charged {} gold and {} shields in unit upkeep: {}.",
                upkeep.gold, upkeep.shields, self.act_passed
            ));
        }

        if frame == 98 && self.act_passed {
            game.selected_unit = game.sim.civil_war_playtest();
            self.act_passed = game.selected_unit.is_some();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleDiplomacy))
            {
                game.perform_action(*action);
            }
        }
        if frame == 101 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Negotiate(1, sim::Pact::Ceasefire)))
            {
                game.perform_action(*action);
            }
            self.act_passed = game.sim.pact_turns(HUMAN, 1) == 16;
        }
        if frame == 105 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            self.act_passed = game.sim.pact_turns(HUMAN, 1) == 16
                && !game
                    .sim
                    .apply(Command::Move(game.selected_unit.unwrap(), 1, 0));
            self.act_detail.push_str(&format!(
                " Senate veto preserved the ceasefire and blocked the attack: {}.",
                self.act_passed
            ));
        }
        if frame == 115 && self.act_passed {
            game.sim.apply(Command::Government(sim::Government::Tribal));
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            game.perform_action(UiAction::ToggleDiplomacy);
            let rebel = game.sim.players().len();
            let moved = game
                .sim
                .apply(Command::Move(game.selected_unit.unwrap(), 1, 0));
            let cities = game
                .sim
                .cities()
                .iter()
                .filter(|city| city.owner == rebel)
                .count();
            self.act_passed = moved
                && cities == 4
                && game.sim.players()[1].government == sim::Government::Anarchy;
            if self.act_passed {
                game.sim.apply(Command::EndTurn);
                self.act_passed = game.sim.players()[1].government == sim::Government::Tribal
                    && game.sim.cities().iter().any(|city| city.owner == rebel);
            }
            self.act_detail.push_str(&format!(" Revolution lifted the veto and capital capture split off {cities} rebel cities: {}.", self.act_passed));
        }

        if frame == 122 && self.act_passed {
            game.perform_action(UiAction::ToggleDiplomacy);
        }
        if frame == 125 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Negotiate(1, sim::Pact::Ceasefire)))
            {
                game.perform_action(*action);
            }
            let refused_early_peace = !game.sim.apply(Command::Negotiate(1, sim::Pact::Peace));
            self.act_passed =
                refused_early_peace && game.sim.pact(HUMAN, 1) == sim::Pact::Ceasefire;
            game.sim.diplomacy_playtest_technologies(false);
            game.sim.diplomacy_playtest_tick(12);
        }
        if frame == 128 && self.act_passed {
            let offered = game.sim.pact_offer(1) == Some(sim::Pact::Peace);
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Negotiate(1, sim::Pact::Peace)))
            {
                game.perform_action(*action);
            }
            self.act_passed = offered && game.sim.pact(HUMAN, 1) == sim::Pact::Armistice;
            game.sim.diplomacy_playtest_technologies(true);
            game.sim.diplomacy_playtest_tick(16);
            self.diplomacy_probe = game.sim.diplomacy_playtest_unit(1);
            self.act_passed &= game.sim.pact(HUMAN, 1) == sim::Pact::Peace
                && self
                    .diplomacy_probe
                    .is_some_and(|(unit, _, _)| !game.sim.apply(Command::Move(unit, 1, 0)));
            self.act_detail.push_str(&format!(" AI refused early peace, then offered it after the ceasefire held. Armistice matured and peace blocked entry: {}.", self.act_passed));
        }
        if frame == 132 && self.act_passed {
            self.vision_probe = game.sim.diplomacy_playtest_hidden_city(1);
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
            {
                game.perform_action(*action);
            }
        }
        if frame == 136 && self.act_passed {
            let human_gold = game.sim.players()[HUMAN].gold;
            let ai_gold = game.sim.players()[1].gold;
            let offer = game.sim.treaty_offer(1).unwrap_or_default();
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(*action);
            }
            let (unit, x, y) = self.diplomacy_probe.unwrap();
            let entered = game.sim.apply(Command::Move(unit, 1, 0));
            let stayed_allied = game
                .sim
                .cities()
                .iter()
                .any(|city| (city.x, city.y, city.owner) == (x, y, 1));
            let gold = offer.terms[0].gold;
            let exchanged = gold > 0
                && game.sim.players()[HUMAN].gold == human_gold - gold
                && game.sim.players()[1].gold == ai_gold + gold
                && game.sim.players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Writing")
                && game.sim.players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Pottery");
            let revealed = self
                .vision_probe
                .is_some_and(|(x, y)| game.sim.tile(x, y).visible);
            self.act_passed = entered
                && stayed_allied
                && game.sim.pact(HUMAN, 1) == sim::Pact::Alliance
                && exchanged
                && revealed;
            self.act_detail.push_str(&format!(" Reviewed and accepted an alliance bundle with Pottery, Writing, {gold} gold and live vision: {}.", self.act_passed));
        }
        if frame == 138 && self.act_passed {
            game.sim.diplomacy_playtest_allied_exchange();
        }
        if frame == 141 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ReviewOffer(1, _)))
            {
                game.perform_action(*action);
            }
        }
        if frame == 144 && self.act_passed {
            let offer = game.sim.treaty_offer(1).unwrap_or_default();
            let gold = game.sim.players()[HUMAN].gold;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::AcceptOffer(1, _)))
            {
                game.perform_action(*action);
            }
            let paid = offer.terms[1].gold;
            self.act_passed = offer.pact.is_none()
                && paid > 23
                && game.sim.players()[HUMAN].gold == gold + paid
                && game.sim.players()[HUMAN]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Warrior Code")
                && game.sim.players()[1]
                    .known
                    .iter()
                    .any(|tech| tech.name() == "Currency");
            self.act_detail.push_str(&format!(" Allied AI paid Warrior Code plus {paid} gold for Currency, above the 23-gold base difference because it values Marketplace income: {}.", self.act_passed));
        }
        if frame == 148 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::CancelPact(1)))
            {
                game.perform_action(*action);
            }
            self.act_passed = self.vision_probe.is_some_and(|(x, y)| {
                let tile = game.sim.tile(x, y);
                tile.seen && !tile.visible
            });
            self.act_detail.push_str(&format!(
                " Ending the alliance removed live vision and retained map knowledge: {}.",
                self.act_passed
            ));
            game.perform_action(UiAction::ToggleDiplomacy);
        }
        if frame == 150 && self.act_passed {
            let probe = game.sim.transport_playtest_setup();
            game.perform_action(UiAction::SelectUnit(probe.1));
            self.transport_probe = Some(probe);
        }
        if frame == 152
            && let Some((ship, passenger, shore)) = self.transport_probe
        {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::BoardOrDeboard))
            {
                game.perform_action(*action);
            }
            let boarded = game.sim.cargo_count(ship) == 1;
            let sailed = game.sim.apply(Command::Move(ship, 1, 0));
            let deboard_blocked = !game.sim.apply(Command::BoardOrDeboard(passenger));
            let landed = game.sim.apply(Command::Move(passenger, 1, 0))
                && game.sim.units().iter().any(|unit| {
                    unit.id == passenger && (unit.x, unit.y) == shore && unit.transport.is_none()
                });
            self.results.push(("transport", boarded && sailed && deboard_blocked && landed,
                format!("Boarded in port: {boarded}. Sailed with cargo: {sailed}. Blocked unloading at sea: {deboard_blocked}. Landed on shore: {landed}.")));
        }
        if frame == 155 && self.act_passed {
            let (x, y) = game.sim.production_playtest_setup();
            game.select_at(x, y);
        }
        if frame == 157 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::RushProduction(_)))
            {
                game.perform_action(*action);
            }
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ProductionPage(1)))
            {
                game.perform_action(*action);
            }
        }
        if frame == 159 && self.act_passed {
            let rushed = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == HUMAN)
                .is_some_and(|city| {
                    city.bought_production && city.shields == UnitKind::Warrior.stats().cost
                })
                && game.sim.players()[HUMAN].gold == 50;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::SellBuilding(_, Building::GRANARY)))
            {
                game.perform_action(*action);
            }
            let sold = game.sim.players()[HUMAN].gold == 50 + Building::GRANARY.data().cost
                && game
                    .sim
                    .cities()
                    .iter()
                    .find(|city| city.owner == HUMAN)
                    .is_some_and(|city| !city.buildings.contains(&Building::GRANARY));
            self.results.push((
                "city_transactions",
                rushed && sold,
                format!("Rushed Warrior for 50 gold: {rushed}. Sold Granary: {sold}."),
            ));
        }
        if frame == 160 && self.act_passed {
            game.perform_action(UiAction::ProductionPage(0));
        }
        if frame == 162 && self.act_passed {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::EditWorklist(
                        _,
                        WorklistEdit::Append(Production::Unit(UnitKind::Worker))
                    )
                )
            }) {
                game.perform_action(*action);
            }
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::WorklistPage(Some(0))))
            {
                game.perform_action(*action);
            }
        }
        if frame == 164 && self.act_passed {
            let visible = buttons.iter().any(|(_, action)| {
                matches!(action, UiAction::EditWorklist(_, WorklistEdit::Remove(0)))
            });
            let turn_ended = game.sim.apply(Command::EndTurn);
            let advanced = turn_ended
                && game
                    .sim
                    .cities()
                    .iter()
                    .find(|city| city.owner == HUMAN)
                    .is_some_and(|city| {
                        city.production == Production::Unit(UnitKind::Worker)
                            && city.worklist.is_empty()
                    });
            self.results.push(("worklist", visible && advanced,
                format!("Queued Worker in city UI: {visible}. Advanced to Worker after production: {advanced}.")));
        }
        if frame == 166 && self.act_passed {
            let worker = game.sim.farmland_playtest_setup();
            game.perform_action(UiAction::SelectUnit(worker));
        }
        if frame == 168 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Irrigate))
            {
                game.perform_action(*action);
            }
            let work = game
                .selected_unit
                .and_then(|id| game.sim.units().iter().find(|unit| unit.id == id))
                .filter(|unit| unit.work_kind == Some(sim::WorkKind::Farmland))
                .map(|unit| (unit.x, unit.y, unit.work_turns_left()));
            let completed = work.is_some_and(|(x, y, turns)| {
                (0..turns).all(|_| game.sim.apply(Command::EndTurn)) && game.sim.tile(x, y).farmland
            });
            self.results.push(("farmland", completed, format!("Started Farmland using the worker control and completed it over end turns: {completed}.")));
        }
        if frame == 172 && self.act_passed {
            let engineer = game.sim.terrain_work_playtest_setup();
            game.perform_action(UiAction::SelectUnit(engineer));
        }
        if frame == 174 && self.act_passed {
            if let Some((_, action)) = buttons.iter().find(|(_, action)| {
                matches!(action, UiAction::TerrainWork(sim::WorkKind::Transform))
            }) {
                game.perform_action(*action);
            }
            let location = game
                .selected_unit
                .and_then(|id| game.sim.units().iter().find(|unit| unit.id == id))
                .filter(|unit| {
                    unit.work_kind == Some(sim::WorkKind::Transform) && unit.work_turns_left() == 12
                })
                .map(|unit| (unit.x, unit.y));
            let completed = location.is_some_and(|(x, y)| {
                (0..12).all(|_| game.sim.apply(Command::EndTurn))
                    && game.sim.tile(x, y).terrain == sim::Terrain::Plains
            });
            self.results.push(("terrain_work", completed, format!("Used the Engineer's Transform control to turn Tundra into Plains within its 12-turn estimate: {completed}.")));
        }
        if frame == 177 && self.act_passed {
            let (warrior, tile) = game.sim.pillage_playtest_setup();
            self.pillage_probe = Some(tile);
            game.perform_action(UiAction::SelectUnit(warrior));
        }
        if frame == 179
            && let Some((x, y)) = self.pillage_probe
        {
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Pillage(sim::PillageTarget::Railroad))
                )
            });
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                game.sim.tile(x, y).railroad
                    && game.sim.apply(Command::EndTurn)
                    && !game.sim.tile(x, y).railroad
                    && game.sim.tile(x, y).road
            });
            self.results.push(("pillage", completed, format!("Pillaged the Railroad through the unit control and preserved its Road: {completed}.")));
        }
        if frame == 182 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let engineer = game.sim.base_playtest_setup();
            game.perform_action(UiAction::SelectUnit(engineer));
        }
        if frame == 184 || frame == 187 {
            let target = if frame == 184 {
                sim::Base::Airstrip
            } else {
                sim::Base::Airbase
            };
            let position = game
                .selected_unit
                .and_then(|id| game.sim.units().iter().find(|unit| unit.id == id))
                .map(|unit| (unit.x, unit.y));
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action, UiAction::TerrainWork(sim::WorkKind::BuildBase(base)) if *base == target
                )
            });
            let mut started = false;
            let completed = position.zip(action).is_some_and(|((x, y), (_, action))| {
                game.perform_action(*action);
                started = game.selected_unit.is_some_and(|id| {
                    game.sim.units().iter().any(|unit| {
                        unit.id == id && unit.work_kind == Some(sim::WorkKind::BuildBase(target))
                    })
                });
                started
                    && game.sim.apply(Command::EndTurn)
                    && game.sim.tile(x, y).base == Some(target)
            });
            self.results.push((
                if frame == 184 { "airstrip" } else { "airbase" },
                completed,
                format!(
                    "Built {} through the Engineer control: {completed}. Control available: {}. Started: {started}.",
                    target.name(), action.is_some()
                ),
            ));
        }
        if frame == 190 && self.act_passed {
            let engineer = game.sim.cleanup_playtest_setup();
            game.perform_action(UiAction::SelectUnit(engineer));
        }
        if frame == 192 && self.act_passed {
            let position = game
                .selected_unit
                .and_then(|id| game.sim.units().iter().find(|unit| unit.id == id))
                .map(|unit| (unit.x, unit.y));
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Clean(sim::Contamination::Fallout))
                )
            });
            let completed = position.zip(action).is_some_and(|((x, y), (_, action))| {
                game.perform_action(*action);
                (0..2).all(|_| game.sim.apply(Command::EndTurn))
                    && !game.sim.tile(x, y).fallout
                    && game.sim.tile(x, y).pollution
            });
            self.results.push(("cleanup", completed,
                format!("Cleaned Fallout through the Engineer control while leaving Pollution intact: {completed}.")));
        }
        if frame == 195 && self.act_passed {
            let ship = game.sim.naval_cleanup_playtest_setup();
            game.perform_action(UiAction::SelectUnit(ship));
        }
        if frame == 197 && self.act_passed {
            let position = game
                .selected_unit
                .and_then(|id| game.sim.units().iter().find(|unit| unit.id == id))
                .map(|unit| (unit.x, unit.y));
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TerrainWork(sim::WorkKind::Clean(sim::Contamination::Pollution))
                )
            });
            let completed = position.zip(action).is_some_and(|((x, y), (_, action))| {
                game.perform_action(*action);
                (0..3).all(|_| game.sim.apply(Command::EndTurn)) && !game.sim.tile(x, y).pollution
            });
            self.results.push((
                "naval_cleanup",
                completed,
                format!(
                    "Transport cleaned pollution at sea through its unit control: {completed}."
                ),
            ));
        }
        if frame == 200 && self.act_passed {
            let ship = game.sim.oil_platform_playtest_setup();
            game.perform_action(UiAction::SelectUnit(ship));
        }
        if frame == 202 && self.act_passed {
            let action = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::Mine));
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                let site = game
                    .selected_unit
                    .and_then(|id| game.sim.units().iter().find(|unit| unit.id == id))
                    .filter(|unit| {
                        unit.work_kind == Some(sim::WorkKind::OilPlatform)
                            && unit.work_turns_left() == 2
                    })
                    .map(|unit| (unit.x, unit.y));
                site.is_some_and(|(x, y)| {
                    (0..2).all(|_| game.sim.apply(Command::EndTurn))
                        && game.sim.tile(x, y).oil_platform
                })
            });
            self.results.push(("oil_platform", completed,
                format!("Transport built a deep-ocean Oil Platform in two turns through its unit control: {completed}.")));
        }
        if frame == 205 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (warrior, diplomat, destination) = game.sim.zone_control_playtest_setup();
            game.perform_action(UiAction::SelectUnit(warrior));
            let blocked = !game.sim.apply(Command::Move(warrior, 1, 0));
            game.perform_action(UiAction::SelectUnit(diplomat));
            let bypassed = game.sim.apply(Command::Move(diplomat, 1, 0))
                && game
                    .sim
                    .units()
                    .iter()
                    .any(|unit| unit.id == diplomat && (unit.x, unit.y) == destination);
            self.results.push((
                "zone_control",
                blocked && bypassed,
                format!(
                    "Foreign zone blocked a Warrior: {blocked}. Diplomat bypassed it: {bypassed}."
                ),
            ));
        }
        if frame == 209 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.embassy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 211 && self.act_passed {
            let diplomat = game.selected_unit;
            let action = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::EstablishEmbassy(_)));
            if let Some((_, action)) = action {
                game.perform_action(*action);
            }
            let established = game.sim.has_embassy(HUMAN, 1) && !game.sim.has_embassy(1, HUMAN);
            let became_ambassador =
                diplomat.is_some_and(|id| game.sim.units().iter().all(|unit| unit.id != id));
            game.perform_action(UiAction::ToggleDiplomacy);
            self.results.push(("embassy", established && became_ambassador,
                format!("Established a one-way embassy through the Diplomat control: {established}. Diplomat became ambassador: {became_ambassador}.")));
        }
        if frame == 214 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.embassy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 216 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::InvestigateCity(_)))
            {
                game.perform_action(*action);
            }
        }
        if frame == 218 && self.act_passed {
            let showed_report = buttons
                .iter()
                .any(|(_, action)| matches!(action, UiAction::CloseInvestigation));
            let retained_unit = game.selected_unit.is_some_and(|id| {
                game.sim.units().iter().any(|unit| {
                    unit.id == id
                        && unit.moves_left
                            == sim::MOVE_FRAGMENTS * UnitKind::Diplomat.stats().moves - 1
                })
            });
            game.perform_action(UiAction::CloseInvestigation);
            self.results.push(("investigation", showed_report && retained_unit,
                format!("Opened the investigation report: {showed_report}. Diplomat survived and spent one movement fragment: {retained_unit}.")));
        }
        if frame == 221 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (diplomat, _) = game.sim.bribe_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 223 && self.act_passed {
            let gold = game.sim.players()[HUMAN].gold;
            let action = buttons.iter().find_map(|(_, action)| match action {
                UiAction::BribeUnit(target, price) => Some((*action, *target, *price)),
                _ => None,
            });
            let transferred = action.is_some_and(|(action, id, price)| {
                let Some(target) = game.sim.units().iter().find(|unit| unit.id == id).cloned()
                else {
                    return false;
                };
                game.perform_action(action);
                game.sim.players()[HUMAN].gold == gold - price
                    && game.sim.units().iter().all(|unit| unit.id != id)
                    && game.sim.units().iter().any(|unit| {
                        unit.owner == HUMAN
                            && unit.kind == target.kind
                            && (unit.x, unit.y) == (target.x, target.y)
                            && unit.home_city.is_some()
                    })
            });
            self.results.push(("bribery", transferred,
                format!("Bribed a foreign Warrior through the unit control and paid the displayed price: {transferred}.")));
        }
        if frame == 226 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.technology_theft_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 228 && self.act_passed {
            let diplomat = game.selected_unit;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::StealTechnology(_)))
            {
                game.perform_action(*action);
            }
            let stolen = game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY);
            let captured =
                diplomat.is_some_and(|id| game.sim.units().iter().all(|unit| unit.id != id));
            self.results.push(("technology_theft", stolen && captured,
                format!("Stole Pottery through the Diplomat control: {stolen}. Diplomat captured after mission: {captured}.")));
        }
        if frame == 231 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let diplomat = game.sim.sabotage_playtest_setup();
            game.perform_action(UiAction::SelectUnit(diplomat));
        }
        if frame == 233 && self.act_passed {
            let diplomat = game.selected_unit;
            let action = buttons.iter().find_map(|(_, action)| match action {
                UiAction::SabotageCity(city) => Some((*action, *city)),
                _ => None,
            });
            let stock_destroyed = action.is_some_and(|(action, city)| {
                game.perform_action(action);
                game.sim
                    .cities()
                    .iter()
                    .any(|target| target.id == city && target.shields == 0)
            });
            let captured =
                diplomat.is_some_and(|id| game.sim.units().iter().all(|unit| unit.id != id));
            self.results.push(("sabotage", stock_destroyed && captured,
                format!("Destroyed a foreign city's shield stock through the Diplomat control: {stock_destroyed}. Diplomat captured after mission: {captured}.")));
        }
        if frame == 236 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let spy = game.sim.spy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(spy));
        }
        if frame == 238 && self.act_passed {
            let spy = game.selected_unit;
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::StealTechnology(_)))
            {
                game.perform_action(*action);
            }
            let stolen = game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY);
            let escaped =
                spy.is_some_and(|id| {
                    game.sim.units().iter().any(|unit| {
                        unit.id == id
                            && unit.kind == UnitKind::Spy
                            && unit.moves_left == 0
                            && game.sim.cities().iter().any(|city| {
                                city.owner == HUMAN && (city.x, city.y) == (unit.x, unit.y)
                            })
                    })
                });
            self.results.push(("spy_escape", stolen && escaped,
                format!("Spy stole Pottery from a previously targeted city: {stolen}. Escaped to a friendly city: {escaped}.")));
        }
        if frame == 241 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let spy = game.sim.spy_playtest_setup();
            game.perform_action(UiAction::SelectUnit(spy));
        }
        if frame == 243
            && self.act_passed
            && let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::SpyTargets(Some(_))))
        {
            game.perform_action(*action);
        }
        if frame == 245 && self.act_passed {
            let action = buttons.iter().find(|(_, action)| {
                matches!(
                    action,
                    UiAction::TargetedTheft(_, crate::rules::Tech::POTTERY)
                )
            });
            let completed = action.is_some_and(|(_, action)| {
                game.perform_action(*action);
                game.sim.has_tech(HUMAN, crate::rules::Tech::POTTERY)
            });
            self.results.push((
                "targeted_espionage",
                completed,
                format!(
                    "Opened the Spy target picker and selected Pottery for theft: {completed}."
                ),
            ));
        }
        if frame == 248 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (explorer, spy) = game.sim.stealth_playtest_setup();
            game.perform_action(UiAction::SelectUnit(explorer));
            let visible = |game: &Game| {
                game.sim
                    .units()
                    .iter()
                    .find(|unit| unit.id == spy)
                    .is_some_and(|unit| game.sim.unit_visible_to_human(unit))
            };
            let hidden = !visible(game);
            let revealed = game.sim.apply(Command::Move(explorer, 1, 0)) && visible(game);
            self.results.push(("spy_stealth", hidden && revealed,
                format!("Foreign Spy hidden outside detection range: {hidden}. Revealed by an adjacent Explorer: {revealed}.")));
        }
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
        if frame == 319 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleSpaceship))
            {
                game.perform_action(*action);
            }
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
        if frame == 322 && self.act_passed {
            if let Some((_, action)) = buttons
                .iter()
                .find(|(_, action)| matches!(action, UiAction::ToggleManualAssembly))
            {
                game.perform_action(*action);
                game.sim.manual_spaceship_playtest_build_structural();
            }
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
            let (city, target) = game.sim.hunter_production_playtest_setup();
            let selected = |game: &Game, kind| {
                game.sim
                    .cities()
                    .iter()
                    .any(|port| port.id == city && port.production == Production::Unit(kind))
            };
            let picked_hunter = selected(game, UnitKind::Submarine);
            game.perform_action(UiAction::EndTurn);
            let picked_missile = selected(game, UnitKind::CruiseMissile);
            game.perform_action(UiAction::EndTurn);
            let built_missile = game.sim.units().iter().any(|unit| {
                unit.owner == 1
                    && unit.kind == UnitKind::CruiseMissile
                    && unit.home_city == Some(city)
            });
            game.perform_action(UiAction::EndTurn);
            let target_destroyed = game.sim.units().iter().all(|unit| unit.id != target)
                && game
                    .sim
                    .units()
                    .iter()
                    .any(|unit| unit.owner == 1 && unit.kind == UnitKind::Submarine);
            self.results.push(("ai_hunter_production", picked_hunter && picked_missile && built_missile && target_destroyed,
                format!("AI selected a Submarine: {picked_hunter}. It selected and built a Cruise Missile: {}. The strike destroyed the target: {target_destroyed}.", picked_missile && built_missile)));
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
            *game = Game::new(crate::mapgen::generate(42));
            game.perform_action(UiAction::FoundCity);
            let (home, destination) = game.sim.ai_merchant_production_playtest_setup();
            let picked = game.sim.cities().iter().any(|city| {
                city.id == home && city.production == Production::Unit(UnitKind::Freight)
            });
            game.perform_action(UiAction::EndTurn);
            let built = game
                .sim
                .units()
                .iter()
                .any(|unit| unit.home_city == Some(home) && unit.kind == UnitKind::Freight);
            for _ in 0..3 {
                game.perform_action(UiAction::EndTurn);
                if game.sim.merchant_playtest_route_exists(home, destination) {
                    break;
                }
            }
            let delivered = game.sim.merchant_playtest_route_exists(home, destination);
            self.results.push(("ai_merchant_production", picked && built && delivered,
                format!("AI picked Freight production: {picked}. Freight was built: {built}. It traveled to establish a route: {delivered}.")));
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
                format!("An AI Transport ferried Freight between islands and established a trade route: {delivered}.")));
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
            let (stacked, healed, obsolete) = game.sim.barracks_playtest();
            self.results.push(("barracks", stacked && healed && obsolete,
                format!("Barracks and Sun Tzu granted two ranks: {stacked}. Barracks fully healed a stationary unit: {healed}. Gunpowder removed the obsolete Barracks bonus: {obsolete}.")));
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
                format!("Theatre respected local, empire and enduring specialist effects: {theatre}. Crusade offset military unhappiness and gold upkeep: {crusade}. Communism ended both effects: {obsolete}.")));
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
        if matches!(frame, 406 | 411) && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let spy = game.sim.spy_revolt_playtest_setup(frame == 411);
            game.perform_action(UiAction::SelectUnit(spy));
        }
        if matches!(frame, 408 | 413) && self.act_passed {
            let failed = frame == 413;
            let spy = game.selected_unit;
            let revolt = buttons.iter().find_map(|(_, action)| match action {
                UiAction::InciteCity(city, price) => Some((*action, *city, *price)),
                _ => None,
            });
            let passed = revolt.is_some_and(|(action, target, price)| {
                let gold = game.sim.players()[HUMAN].gold;
                game.perform_action(action);
                let city = game
                    .sim
                    .cities()
                    .iter()
                    .find(|city| city.id == target)
                    .unwrap();
                let actor = game.sim.units().iter().find(|unit| Some(unit.id) == spy);
                if failed {
                    city.owner != HUMAN
                        && city.size == 2
                        && city.shields == 7
                        && game.sim.players()[HUMAN].gold == gold
                        && actor.is_none()
                } else {
                    city.owner == HUMAN
                        && city.size == 1
                        && game.sim.players()[HUMAN].gold == gold - price
                        && actor.is_some_and(|spy| {
                            (spy.x, spy.y) == (city.x, city.y) && spy.moves_left == 0
                        })
                }
            });
            let (name, detail) = if failed {
                (
                    "spy_revolt_failure",
                    "Failed revolt removed the Spy and preserved the treasury and target city.",
                )
            } else {
                (
                    "spy_revolt_escape",
                    "The Spy escaped to the city after it joined the player's empire.",
                )
            };
            self.results.push((name, passed, detail.to_owned()));
        }
        if frame == 416 && self.act_passed {
            let mut land = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let mut sea = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (land, sea) = (
                land.uprising_playtest(false, false),
                sea.uprising_playtest(true, false),
            );
            self.results.push(("barbarian_uprisings", land && sea,
                format!("Recurring uprising spawned land raiders: {land}. Sea uprising loaded a leader and raiders: {sea}. Both respected the turn-120 onset.")));
        }
        if frame == 419 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (marines, infantry, guard, unlocked) = game.sim.marines_playtest_setup();
            let blocked = !game.sim.apply(Command::Move(infantry, 1, 0));
            let fought = game.sim.apply(Command::Move(marines, 1, 0))
                && game.sim.units().iter().all(|unit| unit.id != guard);
            let landed = game
                .sim
                .units()
                .iter()
                .find(|unit| unit.id == marines)
                .is_some_and(|unit| {
                    unit.transport.is_none()
                        && game
                            .sim
                            .cities()
                            .iter()
                            .filter(|city| city.owner == HUMAN)
                            .any(|city| (city.x, city.y) == (unit.x, unit.y))
                });
            let mut uprising = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let raiders = uprising.uprising_playtest(true, true);
            self.results.push(("marines", unlocked && blocked && fought && landed && raiders,
                format!("Amphibious Warfare unlocked production: {unlocked}. The ordinary passenger was blocked: {blocked}. Marines fought and captured the city from a transport: {}. Advanced sea raiders included Marines: {raiders}.", fought && landed)));
        }
        if frame == 422 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (production, site, founded) = sim.ai_expansion_playtest();
            self.results.push(("ai_expansion", production && site && founded,
                format!("An AI with three cities chose to build a Settler: {production}. Its plan stayed on reachable land: {site}. It completed the trip before founding: {founded}.")));
        }
        if frame == 425 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (production, embarked, founded) = sim.overseas_settlement_playtest(false);
            self.results.push(("overseas_settlement", production && embarked && founded,
                format!("A viable ferry route justified Settler production: {production}. The ferry picked up the Settler and sailed: {embarked}. The Settler founded a city on the island: {founded}.")));
        }
        if frame == 428 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (boat, founder, settled) = sim.settlement_ferry_production_playtest();
            self.results.push(("settlement_ferry_production", boat && founder && settled,
                format!("Map Making let the AI queue a settlement Trireme: {boat}. Completing the ship queued a Settler: {founder}. The pair founded a city overseas: {settled}.")));
        }
        if frame == 431 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (production, embarked, founded) = sim.overseas_settlement_playtest(true);
            self.results.push(("inland_overseas_settlement", production && embarked && founded,
                format!("An inland site justified production: {production}. The ferry sailed: {embarked}. The Settler landed before founding inland: {founded}.")));
        }
        if frame == 434 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (inland, coastal, resumed) = sim.ferry_research_playtest();
            self.results.push(("ferry_research", inland && coastal && resumed,
                format!("Inland city kept its research goal: {inland}. Port prioritized Map Making and its prerequisites: {coastal}. The AI resumed its original goal once it could build a ferry: {resumed}.")));
        }
        if frame == 437 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (waiting, embarked, founded) = sim.offshore_pickup_playtest(false);
            self.results.push(("offshore_settlement_pickup", waiting && embarked && founded,
                format!("Settler waited on open shore as the ferry approached: {waiting}. It boarded the offshore ferry: {embarked}. It founded on the destination island: {founded}.")));
        }
        if frame == 440 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (walked, embarked, founded) = sim.offshore_pickup_playtest(true);
            self.results.push(("inland_ferry_pickup", walked && embarked && founded,
                format!("Settler and ferry traveled toward a rendezvous: {walked}. Settler boarded: {embarked}. It founded a city overseas: {founded}.")));
        }
        if frame == 443 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "rendezvous_route_selection",
                sim.rendezvous_route_playtest(),
                "A later pickup won because it shortened the full route to the island.".to_owned(),
            ));
        }
        if frame == 446 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (tie, strength, radius) = sim.city_borders_playtest();
            self.results.push(("city_border_claims", tie && strength && radius,
                format!("Equal strength preserved the existing claim: {tie}. Stronger claims took outer tiles and respected permanent claims: {strength}. Borders expanded and contracted with population: {radius}.")));
        }
        if frame == 449 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (water, fog) = sim.fogged_border_playtest();
            self.results.push(("fogged_borders", water && fog,
                format!("City claimed adjacent coast and the enclosed lake but left open sea unclaimed: {water}. Ownership stayed last-seen under fog and updated on return: {fog}.")));
        }
        if frame == 452 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (budget, quiet, threatened) = sim.ai_city_defense_playtest();
            self.results.push(("ai_city_defense", budget && quiet && threatened,
                format!("Defender choice respected shield stock: {budget}. A safe city kept its build: {quiet}. A weak garrison facing an enemy Cannon switched to a defender: {threatened}.")));
        }
        if frame == 455 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (empty, walls, naval) = sim.ai_fortifications_playtest();
            self.results.push(("ai_fortifications", empty && walls && naval,
                format!("Empty city chose a defender: {empty}. Garrisoned city chose walls against a Cannon: {walls}. Battleship threat did not justify City Walls: {naval}.")));
        }
        if frame == 458 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (sent, held, reserve) = sim.ai_reinforcements_playtest();
            self.results.push(("ai_reinforcements", sent && held && reserve,
                format!("A reserve Marine marched toward the threatened city: {sent}. The Marine arrived and held: {held}. The source city kept its Warrior: {reserve}.")));
        }
        if frame == 461 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (held, attack, refused) = sim.ai_counterattack_playtest();
            self.results.push(("ai_counterattacks", held && attack && refused,
                format!("Lone defender stayed: {held}. Spare Marine destroyed the adjacent Cannon while a guard stayed: {attack}. Warriors avoided an unfavorable fight: {refused}.")));
        }
        if frame == 464 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (bombed, returned, skipped) = sim.ai_bomber_playtest();
            self.results.push(("ai_bomber_sorties", bombed && returned && skipped,
                format!("Bomber damaged a reachable enemy: {bombed}. It returned to refuel before running dry: {returned}. It skipped another strike when no refueling site remained: {skipped}.")));
        }
        if frame == 467 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (launched, held, recovered) = sim.ai_carrier_sortie_playtest(false);
            self.results.push(("ai_carrier_sorties", launched && held && recovered,
                format!("Bomber launched and struck from an offshore carrier: {launched}. Carrier held its recovery position: {held}. Bomber returned and refueled: {recovered}.")));
        }
        if frame == 470 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (intercepted, skipped, refueled) = sim.ai_fighter_playtest();
            self.results.push(("ai_fighter_interception", intercepted && skipped && refueled,
                format!("Fighter intercepted a nearby bomber: {intercepted}. The distant bomber survived outside safe round-trip range: {skipped}. Fighter returned and refueled: {refueled}.")));
        }
        if frame == 473 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (strike, held, recovered) = sim.ai_carrier_sortie_playtest(true);
            self.results.push(("ai_carrier_positioning", strike && held && recovered,
                format!("Carrier sailed into strike range and the bomber hit: {strike}. It held the new station: {held}. The bomber recovered aboard: {recovered}.")));
        }
        if frame == 476 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (passed, detail) = match sim.save_campaign_playtest() {
                Ok((restored, next, rejected)) => (
                    restored && next && rejected,
                    format!(
                        "Campaign restored byte-for-byte: {restored}. Next turn matched: {next}. Save replacement and corrupt-file rejection worked: {rejected}."
                    ),
                ),
                Err(error) => (false, error.to_string()),
            };
            self.results.push(("campaign_save_load", passed, detail));
        }
        if frame == 479 && self.act_passed {
            *game = Game::new(crate::mapgen::generate(42));
            let (actor, _, gates) = game.sim.paradrop_playtest_setup();
            self.results.push((
                "paratrooper_requirements",
                gates,
                "Combined Arms unlocked production, and a tile beyond range was rejected."
                    .to_owned(),
            ));
            game.perform_action(UiAction::SelectUnit(actor));
        }
        if frame == 481 && self.act_passed {
            let button = buttons
                .iter()
                .any(|(_, action)| matches!(action, UiAction::BeginParadrop));
            let actor = game.selected_unit;
            let target = game
                .sim
                .cities()
                .iter()
                .find(|city| city.owner == 1)
                .map(|city| (city.x, city.y))
                .unwrap();
            if button {
                game.perform_action(UiAction::BeginParadrop);
                game.perform_action(UiAction::ParadropTo(target.0, target.1));
            }
            let landed = game.sim.units().iter().any(|unit| {
                Some(unit.id) == actor
                    && (unit.x, unit.y) == target
                    && unit.moves_left == crate::sim::MOVE_FRAGMENTS
                    && !game.sim.can_begin_paradrop(unit)
            });
            let captured = game
                .sim
                .cities()
                .iter()
                .any(|city| city.owner == HUMAN && (city.x, city.y) == target);
            self.results.push(("paradrop", button && landed && captured,
                format!("HUD offered the action: {button}. Paratroopers landed with movement and could not drop again: {landed}. They captured the undefended enemy city: {captured}.")));
        }
        if frame == 484 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (defended, refused, captured) = sim.ai_paradrop_playtest();
            self.results.push(("ai_paradrops", defended && refused && captured,
                format!("AI reinforced an empty threatened city before attacking: {defended}. It refused the defended landing: {refused}. It captured the enemy city once the defender left: {captured}.")));
        }
        if frame == 487 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (declined, struck) = sim.ai_paradrop_stack_playtest();
            self.results.push(("paradrop_follow_up", declined && struck,
                format!("AI passed up a lone Warrior: {declined}. It landed beside three exposed Warriors and eliminated the stack in a follow-up attack: {struck}.")));
        }
        if frame == 490 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (research, locked, chose, finished) = sim.ai_government_playtest();
            self.results.push((
                "government_research",
                research,
                "A stronger locked government raised demand for its first missing prerequisite."
                    .to_owned(),
            ));
            self.results.push(("ai_government", locked && chose && finished,
                format!("Locked government was excluded: {locked}. Trade city favored Republic: {chose}. The AI kept and completed its revolution: {finished}.")));
        }
        if frame == 493 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (improved, preserved) = sim.government_worker_preview_playtest();
            self.results.push(("government_worker_preview", improved && preserved,
                format!("Candidate government score reflected a better workable tile: {improved}. Live worker and specialist assignments were preserved: {preserved}.")));
        }
        if frame == 496 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (science, growth) = sim.domestic_building_playtest();
            self.results.push(("ai_domestic_buildings", science && growth,
                format!("A trade-rich city favored a Library: {science}. The same city at its growth cap favored a river Aqueduct: {growth}.")));
        }
        if frame == 499 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (mobile, advanced, blocked) = sim.offensive_production_playtest();
            self.results.push((
                "ai_ground_target",
                advanced,
                "The field unit marched toward the reachable city past a nearer island city."
                    .to_owned(),
            ));
            self.results.push(("ai_offensive_production", mobile && blocked,
                format!("AI chose a mobile Chariot for a reachable enemy city: {mobile}. The attack evaluation dropped the target after the land route was cut: {blocked}.")));
        }
        if frame == 502 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (embarked, guarded, captured) = sim.overseas_assault_playtest(false);
            self.results.push(("ai_overseas_assault", embarked && guarded && captured,
                format!("Transport picked up a spare attacker: {embarked}. The source city retained its garrison: {guarded}. The landed force captured the island city: {captured}.")));
        }
        if frame == 505 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (embarked, guarded, captured) = sim.overseas_assault_playtest(true);
            self.results.push(("ai_defended_overseas_assault", embarked && guarded && captured,
                format!("The AI rejected a lone attacker and embarked the stronger force: {embarked}. Source city retained a guard: {guarded}. The expedition captured the defended city: {captured}.")));
        }
        if frame == 508 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (embarked, together, captured) = sim.coordinated_fleet_playtest(false);
            self.results.push(("coordinated_assault_fleet", embarked && together && captured,
                format!("Two ships shared the assault force: {embarked}. They unloaded together: {together}. They captured the city while retaining the source guard: {captured}.")));
        }
        if frame == 511 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (assembled, together, captured) = sim.coordinated_fleet_playtest(true);
            self.results.push(("remote_assault_assembly", assembled && together && captured,
                format!("Troops waited for the remote ship and boarded both: {assembled}. Both ships unloaded together: {together}. The fleet captured the city and left a source guard: {captured}.")));
        }
        if frame == 514 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (intercepted, protected) = sim.fleet_escort_playtest();
            self.results.push(("assault_fleet_escort", intercepted && protected,
                format!("A Battleship intercepted a Frigate threatening the transport: {intercepted}. The transported Marines remained aboard: {protected}.")));
        }
        if frame == 517 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "barbarian_ransom",
                sim.ransom_playtest(),
                "Checked stacked leaders, the treasury cap, mixed stacks, and naval attackers."
                    .to_owned(),
            ));
        }
        if frame == 519 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (recovered, outside, unloaded) = sim.disband_playtest();
            self.results.push(("disband_and_recycle", recovered && outside && unloaded,
                format!("Recycled shields survived production switches: {recovered}. Distant recovery was rejected and field disband succeeded: {outside}. Cargo survived in port: {unloaded}.")));
        }
        if frame == 521 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (rescued, drowned, evacuated) = sim.cargo_rescue_playtest();
            self.results.push(("transport_cargo_rescue", rescued && drowned && evacuated,
                format!("Cargo transferred to a friendly ship and woke from sentry: {rescued}. An enemy ship offered no rescue: {drowned}. A Barbarian Leader evacuated to port: {evacuated}.")));
        }
        if frame == 523 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("nested_transport_rescue", sim.nested_cargo_rescue_playtest(),
                "Checked a helicopter aboard a full carrier and rejected a helicopter whose carrier was also lost.".to_owned()));
        }
        if frame == 525 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "movement_cost_routing",
                sim.road_route_playtest(),
                "Go To followed three connected road steps and arrived in one turn.".to_owned(),
            ));
        }
        if frame == 527 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("fog_of_war_routing", sim.fog_route_playtest(),
                "Hidden terrain and enemy positions did not change the route. A revealed enemy caused a detour.".to_owned()));
        }
        if frame == 529 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "remembered_map_routing",
                sim.remembered_route_playtest(),
                "A road lost under fog remained in the route until the tile was seen again."
                    .to_owned(),
            ));
        }
        if frame == 531 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_emergency_defense_purchase", sim.ai_defense_purchase_playtest(),
                "The AI waited when short on gold, then rushed a defender in the exposed city and left the safe city's production alone.".to_owned()));
        }
        if frame == 533 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_emergency_defense_sale",
                sim.ai_defense_sale_playtest(),
                "The exposed city sold its granary to rush a defender and kept its walls."
                    .to_owned(),
            ));
        }
        if frame == 535 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_civilian_upgrade_budget", sim.ai_civilian_upgrade_playtest(),
                "Checked wartime restraint, the gold reserve, and a peacetime Worker upgrade in a city.".to_owned()));
        }
        if frame == 537 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_purchase_reserve_tax", sim.ai_reserve_playtest(),
                "An unaffordable upgrade raised the reserve and tax rate; both returned to normal after purchase.".to_owned()));
        }
        if frame == 539 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_diplomat_bribery", sim.ai_bribery_playtest(),
                "The AI protected its reserve against a passive target and spent it to bribe an immediate threat.".to_owned()));
        }
        if frame == 541 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_city_counterintelligence", sim.ai_counterintelligence_playtest(),
                "A garrisoned city queued a Spy, then an arriving Diplomat stayed to protect it from a rival agent.".to_owned()));
        }
        if frame == 543 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_offensive_city_missions",
                sim.ai_city_mission_playtest(),
                "AI Diplomats approached a rival city, established an embassy, then stole Pottery."
                    .to_owned(),
            ));
        }
        if frame == 545 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_incite_revolt_budget",
                sim.ai_incite_revolt_playtest(),
                "An AI Diplomat declined an unaffordable revolt, then paid to acquire the city."
                    .to_owned(),
            ));
        }
        if frame == 547 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_city_sabotage", sim.ai_city_sabotage_playtest(),
                "An adjacent AI agent left production intact under armistice and destroyed it at war.".to_owned()));
        }
        if frame == 549 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_offensive_agent_recruitment", sim.ai_offensive_recruitment_playtest(),
                "A queued or active agent reserved its target, and the AI recruited another agent when a distinct target became available.".to_owned()));
        }
        if frame == 551 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "ai_persistent_city_mission",
                sim.ai_persistent_mission_playtest(),
                "An agent kept its target across turns and redirected when that city disappeared."
                    .to_owned(),
            ));
        }
        if frame == 553 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_map_exchange", sim.treaty_map_playtest(),
                "Verified payment, sea-only coverage, allied world-map transfer, and no shared live vision.".to_owned()));
        }
        if frame == 555 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_map_treaty_offer", sim.ai_map_offer_playtest(),
                "The AI offered an affordable sea map, and acceptance transferred knowledge and cleared the offer.".to_owned()));
        }
        if frame == 557 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_city_purchase", sim.city_trade_playtest(),
                "A capital and stale price were rejected. A purchased city kept its population, stocks, improvements and eligible units, and relocated an incompatible visitor.".to_owned()));
        }
        if frame == 559 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "diplomacy_city_buyback",
                sim.city_buyback_playtest(),
                "The city's founding civilization bought it back for half the regular price."
                    .to_owned(),
            ));
        }
        if frame == 561 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("diplomacy_city_bundle", sim.city_bundle_playtest(),
                "A city, map, and gold traded together. A city that became a capital blocked the entire deal.".to_owned()));
        }
        if frame == 563 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_defense_route_cost", sim.ai_threat_route_cost_playtest(),
                "A Horseman was out of range over mountains, two turns away on grassland, and one turn away on roads.".to_owned()));
        }
        if frame == 565 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_road_reinforcements", sim.ai_road_reinforcement_playtest(),
                "A Marine reached the threatened city once the road opened and left a guard behind.".to_owned()));
        }
        if frame == 567 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_merchant_road_delivery", sim.ai_merchant_road_playtest(),
                "The same Caravan reached the destination and delivered in one turn only when the road was open.".to_owned()));
        }
        if frame == 569 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push(("ai_map_observation_transfer", sim.map_snapshot_playtest(),
                "A sea map transferred the seller's observed ocean after the tile changed out of sight.".to_owned()));
        }
        if let Some(step) = match frame {
            30 => Some("foundation"),
            65 => Some("economy"),
            90 => Some("government"),
            115 => Some("civil_war"),
            148 => Some("diplomacy"),
            _ => None,
        } {
            let detail = if !self.previous_passed {
                "Skipped because a prerequisite scenario failed".to_owned()
            } else {
                self.act_detail[self.detail_offset..]
                    .trim_start_matches(['.', ' '])
                    .to_owned()
            };
            self.results.push((step, self.act_passed, detail));
            self.previous_passed = self.act_passed;
            self.detail_offset = self.act_detail.len();
        }
    }

    pub(crate) fn finish(&self, hud_count: usize) {
        ensure_parent(&self.path);
        let hud_detail = format!("Drew {hud_count} HUD regions");
        let steps = [
            ("move", self.move_passed, self.move_detail.as_str()),
            ("act", self.act_passed, self.act_detail.as_str()),
            ("hud", hud_count >= 4, hud_detail.as_str()),
        ]
        .into_iter()
        .chain(
            self.results
                .iter()
                .map(|(step, passed, detail)| (*step, *passed, detail.as_str())),
        )
        .map(|(step, passed, detail)| {
            format!(
                "{{\"step\":\"{step}\",\"passed\":{passed},\"detail\":\"{}\"}}",
                json_escape(detail)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
        let json = format!("{{\"version\":1,\"steps\":[{steps}]}}");
        if let Err(error) = fs::write(&self.path, json) {
            eprintln!("Could not write playtest report: {error}");
        }
    }
}
