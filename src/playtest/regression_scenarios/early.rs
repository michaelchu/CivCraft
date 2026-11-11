//! Implements early behavior within the regression scenarios subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_early_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        _buttons: &[(Rect, UiAction)],
    ) {
        if frame == 416 {
            let mut land = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let mut sea = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (land, sea) = (
                land.uprising_playtest(false, false),
                sea.uprising_playtest(true, false),
            );
            self.results.push(("barbarian_uprisings", land && sea,
                format!("Recurring uprising spawned land raiders: {land}. Sea uprising loaded raiders aboard a barbarian ship: {sea}. Both respected the turn-120 onset.")));
        }

        if frame == 419 && self.act_passed {
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (marines, infantry, guard, unlocked) =
                game.sim.playtest_harness().marines_playtest_setup();
            let blocked = !game
                .sim
                .playtest_harness()
                .apply(Command::Move(infantry, 1, 0));
            let fought = game
                .sim
                .playtest_harness()
                .apply(Command::Move(marines, 1, 0))
                && game
                    .sim
                    .playtest_state()
                    .units()
                    .iter()
                    .all(|unit| unit.id != guard);
            let landed = game
                .sim
                .playtest_state()
                .units()
                .iter()
                .find(|unit| unit.id == marines)
                .is_some_and(|unit| {
                    unit.transport.is_none()
                        && game
                            .sim
                            .playtest_state()
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

        if frame == 422 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (production, site, founded) = sim.ai_expansion_playtest();
            self.results.push(("ai_expansion", production && site && founded,
                format!("An AI with three cities chose to build a Settler: {production}. Its plan stayed on reachable land: {site}. It completed the trip before founding: {founded}.")));
        }

        if frame == 425 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (production, embarked, founded) = sim.overseas_settlement_playtest(false);
            self.results.push(("overseas_settlement", production && embarked && founded,
                format!("A viable ferry route justified Settler production: {production}. The ferry picked up the Settler and sailed: {embarked}. The Settler founded a city on the island: {founded}.")));
        }

        if frame == 428 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (boat, founder, settled) = sim.settlement_ferry_production_playtest();
            self.results.push(("settlement_ferry_production", boat && founder && settled,
                format!("Map Making let the AI queue a settlement Trireme: {boat}. Completing the ship queued a Settler: {founder}. The pair founded a city overseas: {settled}.")));
        }

        if frame == 431 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (production, embarked, founded) = sim.overseas_settlement_playtest(true);
            self.results.push(("inland_overseas_settlement", production && embarked && founded,
                format!("An inland site justified production: {production}. The ferry sailed: {embarked}. The Settler landed before founding inland: {founded}.")));
        }

        if frame == 434 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (inland, coastal, resumed) = sim.ferry_research_playtest();
            self.results.push(("ferry_research", inland && coastal && resumed,
                format!("Inland city kept its research goal: {inland}. Port prioritized Map Making and its prerequisites: {coastal}. The AI selected an available advance toward its newly scored goal after unlocking ferries: {resumed}.")));
        }

        if frame == 437 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (waiting, embarked, founded) = sim.offshore_pickup_playtest(false);
            self.results.push(("offshore_settlement_pickup", waiting && embarked && founded,
                format!("Settler waited on open shore as the ferry approached: {waiting}. It boarded the offshore ferry: {embarked}. It founded on the destination island: {founded}.")));
        }

        if frame == 440 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (walked, embarked, founded) = sim.offshore_pickup_playtest(true);
            self.results.push(("inland_ferry_pickup", walked && embarked && founded,
                format!("Settler and ferry traveled toward a rendezvous: {walked}. Settler boarded: {embarked}. It founded a city overseas: {founded}.")));
        }

        if frame == 443 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            self.results.push((
                "rendezvous_route_selection",
                sim.rendezvous_route_playtest(),
                "A later pickup won because it shortened the full route to the island.".to_owned(),
            ));
        }

        if frame == 446 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (tie, strength, radius) = sim.city_borders_playtest();
            self.results.push(("city_border_claims", tie && strength && radius,
                format!("Equal culture preserved the existing claim: {tie}. Greater civilization culture won rival claims even at a farther rank: {strength}. The existing population-based expansion approximation still contracts: {radius}.")));
        }

        if frame == 449 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (water, fog) = sim.fogged_border_playtest();
            self.results.push(("fogged_borders", water && fog,
                format!("City claimed adjacent coast and the enclosed lake but left open sea unclaimed: {water}. Ownership stayed last-seen under fog and updated on return: {fog}.")));
        }

        if frame == 452 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (budget, quiet, threatened) = sim.ai_city_defense_playtest();
            self.results.push(("ai_city_defense", budget && quiet && threatened,
                format!("Native defender priorities held across shield stocks: {budget}. A safe city kept its build: {quiet}. A weak garrison facing an enemy Cannon switched to a defender: {threatened}.")));
        }

        if frame == 455 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (empty, walls, naval) = sim.ai_fortifications_playtest();
            self.results.push(("ai_fortifications", empty && walls && naval,
                format!("Empty city chose a defender: {empty}. Garrisoned city chose walls against a Cannon: {walls}. Native generic naval-defense choice and actual land/sea effect distinctions matched: {naval}.")));
        }

        if frame == 458 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (sent, held, reserve) = sim.ai_reinforcements_playtest();
            self.results.push(("ai_reinforcements", sent && held && reserve,
                format!("A reserve Marine marched toward the threatened city: {sent}. The Marine arrived and held: {held}. The source city kept its Warrior: {reserve}.")));
        }

        if frame == 461 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (held, attack, refused) = sim.ai_counterattack_playtest();
            self.results.push(("ai_counterattacks", held && attack && refused,
                format!("Lone defender stayed: {held}. Spare Marine destroyed the adjacent Cannon while a guard stayed: {attack}. Warriors avoided an unfavorable fight: {refused}.")));
        }

        if frame == 470 {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (intercepted, skipped, refueled) = sim.ai_fighter_playtest();
            self.results.push(("ai_fighter_interception", intercepted && skipped && refueled,
                format!("Fighter intercepted a nearby bomber: {intercepted}. The fighter waited for full fuel before another sortie: {skipped}. Fighter returned and refueled: {refueled}.")));
        }

        if frame == 476 {
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
            *game = Game::new(crate::sim::Simulation::new(crate::mapgen::generate(42)));
            let (actor, _, gates) = game.sim.playtest_harness().paradrop_playtest_setup();
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
