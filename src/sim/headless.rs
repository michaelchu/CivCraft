//! Small control surface for deterministic, headless campaign runs.

use super::*;

impl Simulation {
    /// Treat every active civilization as computer controlled for simulation runs.
    pub(crate) fn enable_all_civilizations_as_ai(&mut self) {
        for player in self.players.iter_mut().take(self.civilization_count) {
            player.ai = true;
        }
    }

    /// Advance one full campaign turn through the normal turn orchestrator.
    pub(crate) fn simulate_turn(&mut self) {
        self.end_turn();
    }

    /// Return a worked map tile's current food, shield, and trade output.
    pub(crate) fn diagnostic_tile_yields(&self, city: &City, x: i32, y: i32) -> (i32, i32, i32) {
        self.city_tile_yields(city, x, y, false, None)
    }

    pub(crate) fn diagnostic_player_alive(&self, owner: usize) -> bool {
        self.player_alive(owner)
    }

    /// Return structured snapshots for units owned by a civilization.
    pub(crate) fn diagnostic_units_json(&self, owner: usize) -> String {
        self.units_owned_by(owner)
            .map(|unit| {
                let goto = self
                    .unit_goto_tiles
                    .get(&unit.id)
                    .map(|(x, y)| format!("[{x},{y}]"))
                    .unwrap_or_else(|| "null".to_owned());
                let settlement = self
                    .ai_settlement_targets
                    .get(&unit.id)
                    .map(|reservation| {
                        let (x, y) = reservation.destination;
                        format!("[{x},{y}]")
                    })
                    .unwrap_or_else(|| "null".to_owned());
                format!(
                    "{{\"id\":{},\"kind\":{},\"x\":{},\"y\":{},\"hp\":{},\"moves_left\":{},\"state\":{},\"can_found\":{},\"work_kind\":{},\"work_left\":{},\"goto\":{goto},\"settlement_target\":{settlement}}}",
                    unit.id,
                    json_string(unit.kind.name()),
                    unit.x,
                    unit.y,
                    unit.hp,
                    unit.moves_left,
                    json_string(&format!("{:?}", unit.state)),
                    unit.kind.stats().can_found,
                    unit.work_kind.map_or_else(|| "null".to_owned(), |work| json_string(work.name())),
                    unit.work_left,
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn json_string(text: &str) -> String {
    let mut output = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c <= '\u{1f}' => output.push(' '),
            c => output.push(c),
        }
    }
    output.push('"');
    output
}
