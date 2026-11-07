use super::*;

impl Playtest {
    pub(super) fn advance_early_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
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
                format!("Inland city kept its research goal: {inland}. Port prioritized Map Making and its prerequisites: {coastal}. The AI selected an available advance toward its newly scored goal after unlocking ferries: {resumed}.")));
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
                format!("Native defender priorities held across shield stocks: {budget}. A safe city kept its build: {quiet}. A weak garrison facing an enemy Cannon switched to a defender: {threatened}.")));
        }

        if frame == 455 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (empty, walls, naval) = sim.ai_fortifications_playtest();
            self.results.push(("ai_fortifications", empty && walls && naval,
                format!("Empty city chose a defender: {empty}. Garrisoned city chose walls against a Cannon: {walls}. Native generic naval-defense choice and actual land/sea effect distinctions matched: {naval}.")));
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
            self.results.push(("ai_airbase_fighter_sorties", bombed && returned && skipped,
                format!("Fighter struck a reachable enemy: {bombed}. It returned and refueled: {returned}. It skipped another strike when no refueling site remained: {skipped}.")));
        }

        if frame == 467 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (launched, held, recovered) = sim.ai_carrier_sortie_playtest(false);
            self.results.push(("ai_carrier_sorties", launched && held && recovered,
                format!("Fighter launched and struck from an offshore carrier: {launched}. Carrier held its recovery position: {held}. Fighter returned and refueled: {recovered}.")));
        }

        if frame == 470 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (intercepted, skipped, refueled) = sim.ai_fighter_playtest();
            self.results.push(("ai_fighter_interception", intercepted && skipped && refueled,
                format!("Fighter intercepted a nearby bomber: {intercepted}. The fighter waited for full fuel before another sortie: {skipped}. Fighter returned and refueled: {refueled}.")));
        }

        if frame == 473 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (strike, held, recovered) = sim.ai_carrier_sortie_playtest(true);
            self.results.push(("native_carrier_sortie_range", strike && held && recovered,
                format!("Carrier held its station with a target beyond the boss's sortie range: {strike}. It remained at that station: {held}. Fighter remained aboard and refueled: {recovered}.")));
        }

        if frame == 476 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (passed, detail) = match sim.save_campaign_playtest() {
                Ok((restored, next, rejected)) => (
                    restored && next && rejected,
                    format!(
                        "Campaign restored preserved state with native task resets: {restored}. Two loaded replays continued identically: {next}. Save replacement, legacy migrations, and corrupt-file rejection worked: {rejected}."
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
    }
}
