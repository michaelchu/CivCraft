//! Automated gameplay scenarios covering espionage behavior.

use super::*;

impl Simulation {
    pub(crate) fn incite_playtest_setup(&mut self) -> (u32, u32, u32) {
        let actor = self.technology_theft_playtest_setup();
        self.players[HUMAN].gold = 5000;
        let (x, y) = (self.cities[0].x, self.cities[0].y);
        self.cities[0].size = 2;
        self.cities[0].shields = 7;
        self.cities[0].food = 5;
        self.cities[0]
            .buildings
            .retain(|&building| building != Building::PALACE);
        self.cities[0].buildings.push(Building::GRANARY);
        let city = self.cities[0].id;
        let guard = self.spawn_unit(1, UnitKind::Warrior, x, y);
        self.units.last_mut().unwrap().home_city = Some(city);
        self.refresh_visibility();
        (actor, city, guard)
    }

    pub(crate) fn spy_revolt_playtest_setup(&mut self, fail: bool) -> u32 {
        let (actor, _, _) = self.incite_playtest_setup();
        let spy = self.units.iter_mut().find(|unit| unit.id == actor).unwrap();
        spy.kind = UnitKind::Spy;
        self.set_random_seed(if fail { 3 } else { 2 });
        actor
    }

    pub(crate) fn sabotage_unit_playtest_setup(&mut self) -> (u32, u32) {
        let (actor, target) = self.bribe_playtest_setup();
        self.units
            .iter_mut()
            .find(|unit| unit.id == actor)
            .unwrap()
            .kind = UnitKind::Spy;
        self.units
            .iter_mut()
            .find(|unit| unit.id == target)
            .unwrap()
            .hp = 3;
        self.set_random_seed(1);
        (actor, target)
    }

    pub(crate) fn poison_playtest_setup(&mut self) -> (u32, u32) {
        let spy = self.spy_playtest_setup();
        let city = &mut self.cities[0];
        city.size = 2;
        city.food = 5;
        (spy, city.id)
    }

    pub(crate) fn spy_playtest_setup(&mut self) -> u32 {
        let diplomat = self.technology_theft_playtest_setup();
        self.units
            .iter_mut()
            .find(|unit| unit.id == diplomat)
            .unwrap()
            .kind = UnitKind::Spy;
        let (x, y) = offset_native_by_map(10, 20, 3, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.create_city(HUMAN, x, y);
        self.players[HUMAN].known.push(Tech::ESPIONAGE);
        self.cities[0].technology_thefts = 1;
        self.set_random_seed(2);
        diplomat
    }

    pub(crate) fn sabotage_playtest_setup(&mut self) -> u32 {
        let diplomat = self.embassy_playtest_setup();
        let city = self.cities.last_mut().unwrap();
        city.shields = 9;
        city.protected_shields = 2;
        city.production_before_switch = Some((city.production, 12));
        self.set_random_seed(1);
        diplomat
    }

    pub(crate) fn technology_theft_playtest_setup(&mut self) -> u32 {
        let diplomat = self.embassy_playtest_setup();
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::POTTERY];
        self.set_random_seed(1);
        diplomat
    }

    pub(crate) fn bribe_playtest_setup(&mut self) -> (u32, u32) {
        let (x, y) = (10, 20);
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        self.tile_mut(x, y).terrain = Terrain::Grassland;
        self.tile_mut(nx, ny).terrain = Terrain::Grassland;
        self.create_city(HUMAN, x, y);
        let home = self.cities.last().unwrap().id;
        let diplomat = self.spawn_unit(HUMAN, UnitKind::Diplomat, x, y);
        self.units.last_mut().unwrap().home_city = Some(home);
        let target = self.spawn_unit(1, UnitKind::Warrior, nx, ny);
        self.players[HUMAN].gold = 200;
        self.refresh_visibility();
        (diplomat, target)
    }
}
