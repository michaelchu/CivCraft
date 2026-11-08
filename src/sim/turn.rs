//! Implements turn behavior within the sim subsystem.

use super::*;

impl Simulation {
    fn advance_calendar(&mut self) {
        let step = if self.year < -1000 {
            50
        } else if self.year < 0 {
            25
        } else if self.year < 1000 {
            20
        } else if self.year < 1500 {
            10
        } else if self.year < 1750 {
            5
        } else if self.year < 1900 {
            2
        } else {
            1
        };
        self.year += step;
        if self.year == 0 {
            self.year = 1;
        }
    }

    pub(in crate::sim) fn end_turn(&mut self) {
        if self.game_over.is_some() {
            return;
        }
        // srv_main:end_phase also enacts changes made during this phase.
        self.finish_revolutions();
        self.run_ai_worker_phase(HUMAN);
        let automated: Vec<u32> = self
            .player_units_ordered(HUMAN)
            .filter(|u| u.automated && !u.kind.stats().can_work)
            .map(|u| u.id)
            .collect();
        for id in automated {
            self.run_automated_unit(id);
        }
        for owner in 1..self.players.len() {
            if owner == BARBARIANS {
                continue;
            }
            self.run_ai_player(owner);
            self.run_ai_last_activities(owner);
        }
        self.summon_barbarians();
        self.run_ai_player(BARBARIANS);
        self.ai_free_bulbs.clear();
        for owner in 0..self.players.len() {
            self.restore_player_units(owner);
            let income = self.process_player_cities(owner);
            self.finish_player_national_activities(owner, income);
        }
        self.check_disasters();
        self.update_climate();
        self.refresh_diplomatic_contact_effects();
        self.turn += 1;
        self.advance_diplomacy();
        self.prune_trade_routes();
        self.advance_calendar();
        // srv_main:begin_phase progresses activities with the previous
        // movement budget, then refills MP. Revolution completion is also
        // checked here, after activities (update_revolution at line 1381).
        for owner in 0..self.players.len() {
            self.progress_player_unit_activities(owner);
        }
        self.finish_revolutions();
        for city in &mut self.cities {
            city.airlift_used = false;
        }
        self.refresh_visibility();
        self.rebuild_city_ownership();
        // Historical field name is retained for save compatibility; hunter
        // interception needs previous positions of land and sea units alike.
        self.previous_naval_positions = self
            .units
            .iter()
            .map(|unit| (unit.id, (unit.x, unit.y)))
            .collect();
        self.hunter_targets.retain(|hunter, target| {
            self.previous_naval_positions.contains_key(hunter)
                && self.previous_naval_positions.contains_key(target)
        });
        self.check_achievements();
        self.add_event(format!(
            "Turn {} begins - {}.",
            self.turn,
            year_label(self.year)
        ));
        self.check_game_over();
        if self.game_over.is_none() && !self.check_ranked_endgame() {
            self.advance_space_race();
        }
    }

    pub(in crate::sim) fn check_game_over(&mut self) {
        let human_survives = self.cities.iter().any(|c| c.owner == HUMAN)
            || self
                .units
                .iter()
                .any(|u| u.owner == HUMAN && u.kind == UnitKind::Settlers);
        if !human_survives {
            self.game_over = Some("Your civilization has passed into history.".to_owned());
            return;
        }
        let rival_survives =
            self.cities
                .iter()
                .any(|c| c.owner != HUMAN && c.owner != BARBARIANS)
                || self.units.iter().any(|u| {
                    u.owner != HUMAN && u.owner != BARBARIANS && u.kind == UnitKind::Settlers
                });
        if !rival_survives {
            self.game_over = Some("The world is united beneath your banner.".to_owned());
        }
    }
}
