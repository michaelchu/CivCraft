//! Implements military behavior within the regression scenarios subsystem.

use super::*;

impl Playtest {
    pub(super) fn advance_military_scenarios(
        &mut self,
        frame: u32,
        game: &mut Game,
        buttons: &[(Rect, UiAction)],
    ) {
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
                "Monarchy at the research frontier became the goal and current subject using cached government values."
                    .to_owned(),
            ));
            self.results.push(("ai_government", locked && chose && finished,
                format!("Locked government was excluded: {locked}. The AI chose unlocked Monarchy: {chose}. The AI kept and completed its revolution: {finished}.")));
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

        if frame == 519 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (recovered, outside, unloaded) = sim.disband_playtest();
            self.results.push(("disband_and_recycle", recovered && outside && unloaded,
                format!("Recycled shields survived production switches: {recovered}. Distant recovery was rejected and field disband succeeded: {outside}. Cargo survived in port: {unloaded}.")));
        }

        if frame == 521 && self.act_passed {
            let mut sim = crate::sim::Simulation::new(crate::mapgen::generate(42));
            let (rescued, drowned) = sim.cargo_rescue_playtest();
            self.results.push(("transport_cargo_rescue", rescued && drowned,
                format!("Cargo transferred to a friendly ship and woke from sentry: {rescued}. An enemy ship offered no rescue: {drowned}.")));
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
    }
}
