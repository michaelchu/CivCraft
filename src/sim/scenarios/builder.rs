//! Small typed helpers for constructing deterministic campaign fixtures.

use crate::{
    sim::{Pact, Simulation, UnitKind},
    world::{GeneratedMap, GeneratedTile, Terrain, TerrainBase},
};

pub(crate) struct WorldBuilder<'a> {
    game: &'a mut Simulation,
}

impl<'a> WorldBuilder<'a> {
    pub(crate) fn new(game: &'a mut Simulation) -> Self {
        Self { game }
    }

    /// Keep the seed, topology and roster, but discard generated geography
    /// and starting positions before the simulation initializes its state.
    pub(crate) fn ocean_campaign(mut map: GeneratedMap) -> Simulation {
        map.starts.clear();
        map.tiles.fill(GeneratedTile {
            terrain: Terrain::Sea,
            base_terrain: TerrainBase::Sea,
            altitude: 0,
            hut: false,
            river: false,
            river_edges: 0,
            resource: None,
        });
        Simulation::new(map)
    }

    pub(crate) fn start_at(&mut self, turn: i32, year: i32) {
        self.game.turn = turn;
        self.game.year = year;
    }

    pub(crate) fn grassland(&mut self, position: (i32, i32)) {
        let tile = self.game.tile_mut(position.0, position.1);
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
    }

    pub(crate) fn mark_known(&mut self, owner: usize, position: (i32, i32)) {
        self.game
            .tile_mut(position.0, position.1)
            .known_by
            .set(owner, true);
    }

    pub(crate) fn city(&mut self, owner: usize, position: (i32, i32), size: i32) -> u32 {
        self.game.create_city(owner, position.0, position.1);
        let city = self.game.cities.last_mut().expect("city was just created");
        city.size = size;
        city.id
    }

    pub(crate) fn unit(&mut self, owner: usize, kind: UnitKind, position: (i32, i32)) -> u32 {
        self.game.spawn_unit(owner, kind, position.0, position.1)
    }

    pub(crate) fn unit_experience_rank(&mut self, id: u32, rank: usize) {
        let unit = self.unit_mut(id);
        assert!(rank < crate::rules::native::experience().levels.len());
        unit.veteran = rank;
        unit.hp = unit.max_hit_points();
    }

    pub(crate) fn unit_hit_points(&mut self, id: u32, hit_points: i32) {
        let unit = self.unit_mut(id);
        assert!((1..=unit.max_hit_points()).contains(&hit_points));
        unit.hp = hit_points;
    }

    fn unit_mut(&mut self, id: u32) -> &mut crate::sim::Unit {
        let index = self
            .game
            .unit_index_by_id(id)
            .expect("fixture unit must exist");
        &mut self.game.units[index]
    }

    /// Seed the previous turn's position for AI policies that track movement.
    pub(crate) fn previous_unit_position(&mut self, id: u32, position: (i32, i32)) {
        assert!(
            self.game.unit_by_id(id).is_some(),
            "fixture unit must exist"
        );
        self.game.previous_naval_positions.insert(id, position);
    }

    pub(crate) fn war(&mut self, first: usize, second: usize, first_contact: i32, trust: [i32; 2]) {
        self.game.sign_pact(first, second, Pact::War);
        self.game
            .set_scenario_diplomatic_history(first, second, first_contact, trust);
    }

    pub(crate) fn refresh_visibility(&mut self) {
        self.game.refresh_visibility();
    }
}
