//! Focused gameplay scenarios for the parent subsystem.

use super::*;

impl super::Simulation {
    pub(crate) fn ai_guard_ferry_capacity_playtest(&mut self) -> (bool, bool, bool) {
        self.units.clear();
        self.cities.clear();
        self.ai_settlement_targets.clear();
        self.ai_guard_targets.clear();
        self.ai_unit_guards.clear();
        self.ai_ferries = Default::default();
        self.diplomacy.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.owner = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.resource = None;
            tile.hut = false;
        }
        let home = (10, 24);
        let island = offset_native_by_map(home.0, home.1, 8, 0);
        for (x, y) in [home, island] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        self.create_city(1, home.0, home.1);
        let founder = self.spawn_unit(1, UnitKind::Settler, home.0, home.1);
        let small = self.spawn_unit(1, UnitKind::Galley, home.0, home.1);
        let cargo = self.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        self.units
            .iter_mut()
            .find(|u| u.id == cargo)
            .unwrap()
            .transport = Some(small);
        let far = offset_native_by_map(home.0, home.1, -2, 0);
        let roomy = self.spawn_unit(1, UnitKind::Galley, far.0, far.1);
        let actor = self.unit_by_id(founder).unwrap().clone();
        self.reserve_settlement_site(&actor, island);
        let speculative = self
            .best_settlement_ferry(&actor, |_| false)
            .is_some_and(|(id, _)| id == small);
        let guard = self.spawn_unit(1, UnitKind::Musketman, home.0, home.1);
        self.assign_ai_guard(guard, guards::GuardTarget::Unit(founder));
        let no_reserved_defenders = std::collections::HashSet::new();
        let actual = self.dispatch_settlement_ferries(1, &no_reserved_defenders);
        let reselected = actual
            .get(&roomy)
            .is_some_and(|plan| plan.destination == island)
            && !actual.contains_key(&small)
            && self
                .ai_settlement_targets
                .get(&founder)
                .is_some_and(|target| target.destination == island);
        self.remove_units_with_cargo(vec![roomy]);
        let blocked = self
            .dispatch_settlement_ferries(1, &no_reserved_defenders)
            .is_empty()
            && self
                .ai_settlement_targets
                .get(&founder)
                .is_some_and(|target| target.destination == island)
            && self.ferry_boss(small).is_none();
        (speculative, reselected, blocked)
    }

    pub(crate) fn rendezvous_route_playtest(&mut self) -> bool {
        self.units.clear();
        for tile in &mut self.tiles {
            tile.terrain = Terrain::Sea;
            tile.resource = None;
        }
        let early = (10, 24);
        let later = offset_native_by_map(early.0, early.1, 10, 0);
        let island = offset_native_by_map(later.0, later.1, 1, 0);
        self.tile_mut(island.0, island.1).terrain = Terrain::Grassland;
        let labels = self.continent_labels();
        let founder = self.make_unit(0, 1, UnitKind::Settler, early.0, early.1);
        let ship = self.make_unit(0, 1, UnitKind::Galley, early.0, early.1);
        let meetings = [
            Rendezvous {
                tile: early,
                turns: 0,
                walk: Vec::new(),
            },
            Rendezvous {
                tile: later,
                turns: 2,
                walk: vec![(1, 0)],
            },
        ];
        let early_plan = self.settlement_ferry_route(&founder, &ship, &labels, 0, &meetings[..1]);
        let combined = self.settlement_ferry_route(&founder, &ship, &labels, 0, &meetings);
        early_plan.zip(combined).is_some_and(|(early, best)| {
            best.pickup == later
                && best.destination == island
                && best.pickup_walk == meetings[1].walk
                && best.value > early.value
        })
    }
}
