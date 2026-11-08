//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    pub(crate) fn ai_normal_path_actions_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.known_by.fill(true);
        }
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let blocker = offset_native_by_map(10, 20, 1, 0);
        let refuge = offset_native_by_map(10, 20, 2, 0);
        self.spawn_unit(HUMAN, UnitKind::Warrior, blocker.0, blocker.1);
        self.spawn_unit(BARBARIANS, UnitKind::Worker, refuge.0, refuge.1);
        let actor = self.unit_by_id(leader).unwrap();
        let optimistic = self.leader_paths(actor, None).costs
            [self.map_wrap.index(refuge.0, refuge.1)]
            == 2 * MOVE_FRAGMENTS;
        self.run_ai_player(BARBARIANS);
        let stopped = self.units.iter().any(|unit| {
            unit.id == leader
                && (unit.x, unit.y) == (10, 20)
                && unit.moves_left == 2 * MOVE_FRAGMENTS
        });
        self.units.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Ocean;
        }
        let carrier = self.spawn_unit(HUMAN, UnitKind::Carrier, 10, 20);
        self.spawn_unit(BARBARIANS, UnitKind::Caravel, blocker.0, blocker.1);
        let actor = self.unit_by_id(carrier).unwrap();
        let paths = self.leader_paths(actor, None);
        let endpoint = paths.costs[self.map_wrap.index(blocker.0, blocker.1)] == MOVE_FRAGMENTS
            && paths.costs[self.map_wrap.index(refuge.0, refuge.1)] == 3 * MOVE_FRAGMENTS;
        let execution = !self.apply(Command::Move(carrier, 1, 0))
            && self
                .units
                .iter()
                .any(|unit| unit.id == carrier && (unit.x, unit.y) == (10, 20));
        optimistic && stopped && endpoint && execution
    }

    pub(crate) fn ai_barbarian_leader_playtest(&mut self) -> bool {
        self.units.clear();
        self.cities.clear();
        self.prune_unit_ai_state();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.owner = None;
            tile.hut = false;
            tile.known_by.fill(true);
        }
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let refuge = offset_native_by_map(10, 20, 2, 0);
        self.spawn_unit(BARBARIANS, UnitKind::Worker, refuge.0, refuge.1);
        self.run_ai_player(BARBARIANS);
        let sheltered = self
            .units
            .iter()
            .any(|unit| unit.id == leader && (unit.x, unit.y) == refuge);
        self.units.clear();
        self.prune_unit_ai_state();
        let leader = self.spawn_unit(BARBARIANS, UnitKind::BarbarianLeader, 10, 20);
        let enemy = offset_native_by_map(10, 20, -1, 0);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, enemy.0, enemy.1);
        self.run_ai_player(BARBARIANS);
        sheltered
            && self.units.iter().any(|unit| {
                unit.id == leader
                    && unit.moves_left == 0
                    && self.map_distance(unit.x, unit.y, enemy.0, enemy.1) > 1
            })
    }
}
