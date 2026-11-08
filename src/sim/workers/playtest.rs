//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    /// Desktop regression: remote cleanup selection, a closer worker taking
    /// the reservation, and deterministic shelter once no jobs remain.
    pub(crate) fn ai_worker_planning_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.owner = None;
            tile.hut = false;
            tile.river = false;
            tile.road = true;
            tile.railroad = true;
            tile.irrigation = true;
            tile.farmland = true;
            tile.mine = false;
            tile.oil_well = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.base = None;
        }
        self.create_city(1, 20, 20);
        self.tile_mut(20, 20).pollution = true;
        self.climate_current[0] = 1000;
        let far = offset_native_by_map(20, 20, -4, 0);
        let near = offset_native_by_map(20, 20, 1, 0);
        // Road-only approach makes the far worker arrive next phase.
        for tile in &mut self.tiles {
            tile.railroad = false;
        }
        let near_id = self.spawn_unit(1, UnitKind::Engineer, near.0, near.1);
        // Native player lists prepend: create the far worker last so it
        // reserves first and the nearer worker must actually displace it.
        let far_id = self.spawn_unit(1, UnitKind::Settlers, far.0, far.1);
        let actor = self.unit_by_id(far_id).unwrap().clone();
        let remote = self
            .worker_plan(&actor, &WorkMap::default(), &[])
            .is_some_and(|p| {
                p.destination == (20, 20) && p.activity == WorkKind::Clean(Contamination::Pollution)
            });
        self.run_ai_worker_phase(1);
        let reserved = self.units.iter().any(|u| {
            u.id == near_id
                && (u.x, u.y) == (20, 20)
                && u.work_kind == Some(WorkKind::Clean(Contamination::Pollution))
        }) && self
            .units
            .iter()
            .find(|u| u.id == far_id)
            .is_some_and(|u| u.work_kind != Some(WorkKind::Clean(Contamination::Pollution)));
        self.units.clear();
        self.tile_mut(20, 20).pollution = false;
        self.climate_current[0] = 0;
        self.cities[0].owner = HUMAN;
        self.players[HUMAN].known.clear();
        for tile in &mut self.tiles {
            tile.owner = None;
        }
        let shelter = self.spawn_unit(HUMAN, UnitKind::Worker, near.0, near.1);
        self.units.last_mut().unwrap().automated = true;
        // Generic Human automation (also the displaced-worker callback)
        // seeks shelter. Default AI phase-entry workers deliberately do not.
        self.run_ai_worker_phase(HUMAN);
        let sheltered = self
            .units
            .iter()
            .any(|u| u.id == shelter && (u.x, u.y) == (20, 20));
        (remote, reserved, sheltered)
    }
}
