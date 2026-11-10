//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    fn civ3_east(&self, point: (i32, i32), tiles: usize) -> (i32, i32) {
        (0..tiles).fold(point, |(x, y), _| {
            self.map_wrap.civ3_neighbor(x, y, 2).unwrap()
        })
    }

    pub(crate) fn ai_zero_attack_carrier_path_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.hut = false;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
        let blocker = self.civ3_east((10, 20), 1);
        self.spawn_unit(BARBARIANS, UnitKind::Caravel, blocker.0, blocker.1);
        let unit = self.unit_by_id(carrier).unwrap().clone();
        let endpoint = self
            .ai_movement_cost_order(&unit)
            .iter()
            .any(|&(cost, x, y)| (x, y) == blocker && cost > 0);
        let target = self.pf_default_action(
            UnitKind::Carrier,
            HUMAN,
            blocker,
            true,
            true,
            false,
        );
        endpoint
            && UnitKind::Carrier.stats().attack > 0
            && target == super::PfAction::Attack
            && self.leader_paths(&unit, None).costs[self.map_wrap.index(blocker.0, blocker.1)]
                < i32::MAX
    }



}
