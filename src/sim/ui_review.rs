//! Deterministic data for the opt-in UI screenshot gallery; never used in play.
use super::*;
mod diplomacy;

#[cfg(test)]
#[path = "ui_review/tests.rs"]
mod tests;

/// The mutations available to the visual review harness. Screens have no
/// mutable access to the underlying campaign.
pub(crate) struct ReviewFixture<'a>(pub(in crate::sim) &'a mut Simulation);

impl ReviewFixture<'_> {
    pub(crate) fn leader_army(&mut self) -> u32 {
        let sim = &mut self.0;
        sim.units.clear();
        sim.cities.clear();
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.river = false;
            tile.hut = false;
            tile.base = None;
        }
        for (x, y) in [(10, 24), (16, 24), (22, 24), (28, 24)] {
            sim.create_city(HUMAN, x, y);
        }
        let leader = sim.spawn_unit(HUMAN, UnitKind::Leader, 10, 24);
        sim.refresh_visibility();
        sim.refresh_score_snapshot();
        ui_review_record_knowledge(sim);
        leader
    }

    pub(crate) fn loaded_army(&mut self) -> u32 {
        self.leader_army();
        let sim = &mut self.0;
        let year = sim.year;
        let city = sim
            .cities
            .iter_mut()
            .find(|city| city.x == 10 && city.y == 24)
            .unwrap();
        if !city.buildings.contains(&Building::PENTAGON) {
            city.buildings.push(Building::PENTAGON);
        }
        city.record_culture_building(Building::PENTAGON, HUMAN, year);
        let army = sim.spawn_unit(HUMAN, UnitKind::Army, 10, 24);
        for kind in [UnitKind::Swordsman, UnitKind::Spearman, UnitKind::Warrior] {
            let member = sim.spawn_unit(HUMAN, kind, 10, 24);
            assert!(sim.board_specific_ship(member, army));
        }
        ui_review_record_knowledge(sim);
        army
    }

    pub(crate) fn ai_city_garrisons(&mut self) {
        let sim = &mut self.0;
        ui_review_fog(sim);
        sim.units.clear();
        sim.observed_map = Default::default();
        for (x, y, name, state) in [
            (17, 24, "Idle Defender", UnitState::Idle),
            (21, 24, "Fortified Defender", UnitState::Fortified),
            (25, 24, "Sentry Defender", UnitState::Sentry),
            (21, 30, "Remembered City", UnitState::Fortified),
        ] {
            sim.create_city(1, x, y);
            sim.cities.last_mut().unwrap().name = name.into();
            let id = sim.spawn_unit(1, UnitKind::Spearman, x, y);
            sim.units.iter_mut().find(|u| u.id == id).unwrap().state = state;
        }
        for y in 0..sim.map_wrap.height() {
            for x in 0..sim.map_wrap.width() {
                let known = sim.map_wrap.step_distance(x, y, 21, 25) <= 7;
                let tile = sim.tile_mut(x, y);
                tile.seen = known;
                tile.visible = known && y <= 26;
                tile.known_by.set(HUMAN, known);
            }
        }
        ui_review_record_knowledge(sim);
    }

    pub(crate) fn borders_fog_cities(&mut self) {
        ui_review_borders_fog_cities(self.0)
    }

    pub(crate) fn religion(&mut self, city: u32) {
        if !self.0.has_tech(HUMAN, Tech::MEDITATION) {
            self.0.players[HUMAN].known.push(Tech::MEDITATION);
        }
        assert!(self.0.found_religion(HUMAN, city, 1));
    }

    pub(crate) fn fixture(&mut self) -> (u32, u32) {
        ui_review_fixture(self.0)
    }

    pub(crate) fn reveal_all_tiles(&mut self) {
        ui_review_reveal_all_tiles(self.0)
    }

    /// UI review scene using Civ III city art.
    pub(crate) fn city_sprites(&mut self) {
        ui_review_city_sprites(self.0)
    }

    pub(crate) fn first_contact(&mut self) {
        ui_review_first_contact(self.0)
    }

    pub(crate) fn compensation_negotiation(&mut self, other: usize) {
        self::diplomacy::ui_review_compensation_negotiation(self.0, other)
    }

    pub(crate) fn resource_shadows(&mut self) {
        ui_review_resource_shadows(self.0)
    }

    pub(crate) fn offer(&mut self) -> TreatyOffer {
        self::diplomacy::ui_review_offer(self.0)
    }

    pub(crate) fn spaceship(&mut self) {
        manual_spaceship_fixture(self.0)
    }

    pub(crate) fn touching_borders(&mut self) {
        ui_review_touching_borders(self.0)
    }

    pub(crate) fn rivers(&mut self, fogged: bool) {
        ui_review_rivers(self.0, fogged)
    }

    pub(crate) fn terrain_layers(&mut self, layers: bool) -> Option<u32> {
        ui_review_terrain_layers(self.0, layers)
    }

    pub(crate) fn ice_transitions(&mut self) {
        ui_review_ice_transitions(self.0)
    }

    /// UI review scene using Civ III water terrain art.
    pub(crate) fn water_depth_scene(&mut self) {
        ui_review_water_depths(self.0)
    }

    pub(crate) fn terrain_biomes(&mut self) {
        ui_review_terrain_biomes(self.0);
    }

    pub(crate) fn terrain_seams(&mut self) {
        ui_review_terrain_seams(self.0)
    }

    pub(crate) fn wrap_seam(&mut self) -> u32 {
        ui_review_wrap_seam(self.0)
    }

    pub(crate) fn observed_map(&mut self) -> (u32, (i32, i32)) {
        ui_review_observed_map(self.0)
    }

    pub(crate) fn route_setup(&mut self) -> (u32, (i32, i32)) {
        ui_review_route_setup(self.0)
    }

    pub(crate) fn fog(&mut self) -> u32 {
        ui_review_fog(self.0)
    }

    pub(crate) fn army_combat_setup(&mut self, attacking: bool) -> (u32, (i32, i32), u32) {
        ui_review_army_combat_setup(self.0, attacking)
    }

    pub(crate) fn air_combat_setup(&mut self, with_sam: bool) -> (u32, (i32, i32)) {
        ui_review_air_combat_setup(self.0, with_sam)
    }

    pub(crate) fn nuclear_setup(&mut self, with_sdi: bool) -> (u32, (i32, i32)) {
        ui_review_nuclear_setup(self.0, with_sdi)
    }

    pub(crate) fn animation_setup(&mut self, combat: bool) -> (u32, (i32, i32)) {
        ui_review_animation_setup(self.0, combat)
    }

    pub(crate) fn city_appearance(&mut self) {
        ui_review_city_appearance(self.0)
    }

    pub(crate) fn unit_status(&mut self) {
        ui_review_unit_status(self.0)
    }

    pub(crate) fn generated_river_focus(&mut self) -> Option<(i32, i32)> {
        ui_review_generated_river_focus(self.0)
    }

    pub(crate) fn record_knowledge(&mut self) {
        ui_review_record_knowledge(self.0)
    }

    pub(crate) fn reobserve_map(&mut self) {
        ui_review_reobserve_map(self.0)
    }

    pub(crate) fn clear_animation_events(&mut self) {
        self.0.take_visual_events();
    }
}

/// Read-only inspection for assertions made by the visual review harness.
pub(crate) struct ReviewState<'a>(pub(in crate::sim) &'a Simulation);

impl<'a> ReviewState<'a> {
    pub(crate) fn units(self) -> &'a [Unit] {
        self.0.units()
    }

    pub(crate) fn unit_by_id(self, id: u32) -> Option<&'a Unit> {
        self.0.unit_by_id(id)
    }

    pub(crate) fn cities(self) -> &'a [City] {
        self.0.cities()
    }

    pub(crate) fn city_by_id(self, id: u32) -> Option<&'a City> {
        self.0.city_by_id(id)
    }

    pub(crate) fn cities_owned_by(self, owner: usize) -> impl Iterator<Item = &'a City> {
        self.0.cities_owned_by(owner)
    }

    pub(crate) fn human_rates(self) -> [i32; 3] {
        self.0.players()[HUMAN].rates
    }

    pub(crate) fn tile_visibility(self, x: i32, y: i32) -> (bool, bool) {
        let tile = self.0.tile(x, y);
        (tile.seen, tile.visible)
    }

    pub(crate) fn observed_map_valid(self, reobserved: bool) -> bool {
        ui_review_observed_map_valid(self.0, reobserved)
    }

    pub(crate) fn move_command(self, unit: u32, dx: i32, dy: i32) -> Command {
        Command::move_by_offset(self.0, unit, dx, dy)
    }
}

/// Gallery scenario: a city and its road are remembered while changes to
/// the live tile and city remain hidden until the scout returns.
pub(crate) fn ui_review_observed_map(sim: &mut Simulation) -> (u32, (i32, i32)) {
    sim.units.clear();
    sim.cities.clear();
    sim.observed_map = Default::default();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.road = false;
        tile.railroad = false;
        tile.irrigation = false;
        tile.farmland = false;
        tile.mine = false;
        tile.hut = false;
        tile.resource = None;
        tile.river = false;
        tile.base = None;
        tile.owner = None;
        tile.seen = false;
        tile.visible = false;
        tile.known_by.fill(false);
    }
    sim.players[1].known.clear();
    sim.create_city(1, 20, 24);
    let c = sim.cities.last_mut().unwrap();
    c.name = "Remembered Athens".into();
    c.size = 5;
    c.buildings.push(Building::CITY_WALLS);
    let scout = sim.spawn_unit(HUMAN, UnitKind::Explorer, 19, 24);
    sim.tile_mut(20, 24).road = true;
    sim.tile_mut(19, 23).terrain = Terrain::Forest;
    sim.refresh_visibility();
    let unit = sim.units.iter_mut().find(|u| u.id == scout).unwrap();
    unit.x = 4;
    unit.y = 24;
    sim.refresh_visibility();
    let c = sim.cities.last_mut().unwrap();
    c.name = "Rebuilt Athens".into();
    c.size = 13;
    c.buildings.retain(|b| *b != Building::CITY_WALLS);
    sim.players[1].known.push(Tech::ROCKETRY);
    sim.tile_mut(20, 24).road = false;
    sim.tile_mut(20, 24).railroad = false;
    sim.tile_mut(20, 24).terrain = Terrain::Desert;
    sim.tile_mut(20, 24).irrigation = true;
    sim.tile_mut(19, 23).terrain = Terrain::Hills;
    sim.refresh_visibility();
    (scout, (20, 24))
}

pub(crate) fn ui_review_reobserve_map(sim: &mut Simulation) {
    let unit = sim.units.iter_mut().find(|u| u.owner == HUMAN).unwrap();
    unit.x = 19;
    unit.y = 24;
    sim.refresh_visibility();
}

pub(crate) fn ui_review_observed_map_valid(sim: &Simulation, reobserved: bool) -> bool {
    let city = sim
        .render_cities()
        .into_iter()
        .find(|c| c.city.x == 20 && c.city.y == 24);
    if reobserved {
        sim.render_tile(20, 24).terrain == Terrain::Desert
            && !sim.render_tile(20, 24).road
            && city.is_some_and(|c| {
                c.city.name == "Rebuilt Athens" && c.city.size == 13 && !c.remembered
            })
    } else {
        sim.render_tile(20, 24).terrain == Terrain::Grassland
            && sim.render_tile(20, 24).road
            && city.is_some_and(|c| {
                c.city.name == "Remembered Athens" && c.city.size == 5 && c.remembered
            })
    }
}

pub(crate) fn ui_review_first_contact(sim: &mut Simulation) {
    sim.clear_notifications();
    sim.diplomacy.remove(&(HUMAN, 1));
    sim.make_diplomatic_contact(HUMAN, 1);
}

pub(crate) fn ui_review_fixture(sim: &mut Simulation) -> (u32, u32) {
    sim.units.clear();
    sim.cities.clear();
    sim.players[HUMAN].known = Tech::all().collect();
    sim.players[HUMAN].gold = 12_345_678;
    sim.players[HUMAN].researching = Tech::NUCLEAR_FISSION;
    sim.players[HUMAN].research_goal = Tech::SUPERCONDUCTORS;
    for tile in &mut sim.tiles {
        tile.seen = true;
        tile.known_by.fill(true);
    }
    sim.tile_mut(20, 24).terrain = Terrain::Grassland;
    sim.create_city(HUMAN, 20, 24);
    let city = sim.cities[0].id;
    sim.cities[0].name = "Rome on the Northern Coast".into();
    sim.cities[0].size = 12;
    sim.cities[0].specialists = [2, 2, 1];
    sim.cities[0]
        .buildings
        .extend([Building::LIBRARY, Building::MARKETPLACE, Building::TEMPLE]);
    sim.cities[0].worklist = vec![
        Production::Unit(UnitKind::Worker),
        Production::Building(Building::CITY_WALLS),
        Production::Unit(UnitKind::Marine),
    ];
    for kind in [
        UnitKind::Worker,
        UnitKind::Warrior,
        UnitKind::Marine,
        UnitKind::Rifleman,
        UnitKind::Guerilla,
    ] {
        sim.spawn_unit(HUMAN, kind, 20, 24);
    }
    for owner in 1..PLAYER_COUNT {
        let p = (10 + owner as i32 * 2, 32);
        sim.tile_mut(p.0, p.1).terrain = Terrain::Grassland;
        sim.create_city(owner, p.0, p.1);
        sim.sign_pact(HUMAN, owner, Pact::Peace);
    }
    self::diplomacy::ui_review_contacts(sim);
    for x in [16, 20, 24] {
        sim.tile_mut(x, 40).terrain = Terrain::Grassland;
        sim.create_city(1, x, 40);
    }
    sim.world_seed = 18_446_744_073_709_551_615;
    let rival = sim.cities_owned_by(1).next().unwrap().id;
    let c = sim.cities.iter_mut().find(|c| c.id == rival).unwrap();
    c.name = "Athens of the Eastern Mediterranean".into();
    c.size = 8;
    c.buildings.extend([
        Building::LIBRARY,
        Building::MARKETPLACE,
        Building::TEMPLE,
        Building::CITY_WALLS,
    ]);
    sim.reconcile_city_workers();
    sim.refresh_visibility();
    for tile in &mut sim.tiles {
        tile.known_by.set(HUMAN, true);
    }
    self::diplomacy::ui_review_contacts(sim);
    (city, rival)
}

/// Isolated visual fixture for the two sides of a shared national boundary.
pub(crate) fn ui_review_touching_borders(sim: &mut Simulation) {
    sim.units.clear();
    sim.cities.clear();
    for (owner, dx, name) in [(HUMAN, -2, "Rome"), (1, 2, "Athens")] {
        let (x, y) = offset_native_by_map(20, 24, dx, 0);
        sim.tile_mut(x, y).terrain = Terrain::Grassland;
        sim.create_city(owner, x, y);
        sim.cities.last_mut().unwrap().name = name.into();
    }
    for tile in &mut sim.tiles {
        tile.owner = None;
        tile.last_seen_owner = None;
    }
    for dy in -4..=4 {
        for dx in -4..=4 {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            let tile = sim.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.river = false;
            tile.owner = Some(if dx <= 0 { HUMAN } else { 1 });
            tile.last_seen_owner = tile.owner;
            tile.seen = true;
            tile.visible = true;
            tile.known_by.fill(true);
        }
    }
}

/// Controlled artwork cases: diagonal desert course, bends, forks and a
/// lake receiving rivers from each of the six isohex cardinal directions.
pub(crate) fn ui_review_rivers(sim: &mut Simulation, fogged: bool) {
    sim.units.clear();
    sim.cities.clear();
    for tile in &mut sim.tiles {
        tile.owner = None;
        tile.last_seen_owner = None;
        tile.river = false;
        tile.river_edges = 0;
    }
    for dy in -7..=7 {
        for dx in -7..=7 {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            let tile = sim.tile_mut(x, y);
            tile.terrain = Terrain::Desert;
            tile.river = false;
            tile.river_edges = 0;
            tile.resource = None;
            tile.hut = false;
            tile.road = false;
            tile.railroad = false;
            tile.irrigation = false;
            tile.farmland = false;
            tile.mine = false;
            tile.oil_well = false;
            tile.oil_platform = false;
            tile.pollution = false;
            tile.fallout = false;
            tile.base = None;
            tile.seen = true;
            tile.visible = true;
            tile.known_by.fill(true);
        }
    }
    let mut rivers = vec![
        (-4, -4),
        (-3, -3),
        (-2, -2),
        (2, -3),
        (2, -2),
        (3, -1),
        (3, 0),
        (3, 4),
        (3, 3),
        (4, 4),
        (2, 4),
    ];
    rivers.extend(ISOHEX_DIRS.map(|(dx, dy)| (-3 + dx, 2 + dy)));
    for (dx, dy) in rivers {
        let (x, y) = offset_native_by_map(20, 24, dx, dy);
        sim.tile_mut(x, y).river = true;
    }
    for (dx, dy) in [(-1, -1), (-3, 2), (4, 0), (5, 4)] {
        let (x, y) = offset_native_by_map(20, 24, dx, dy);
        sim.tile_mut(x, y).terrain = Terrain::Lake;
    }
    super::topology::rebuild_river_edges(sim.map_wrap, &mut sim.tiles);
    if fogged {
        let (x, y) = offset_native_by_map(20, 24, -1, -1);
        let tile = sim.tile_mut(x, y);
        tile.seen = false;
        tile.visible = false;
        tile.known_by.set(HUMAN, false);
        let (x, y) = offset_native_by_map(20, 24, 3, -1);
        sim.tile_mut(x, y).visible = false;
    }
    sim.clear_notifications();
    sim.add_event(
        "River artwork fixture: diagonal desert course, bends, forks and six lake outlets.",
    );
}
/// Reveals the generated map for visual inspection without changing any
/// terrain, river flags, extras, RNG or starting positions.
pub(crate) fn ui_review_generated_river_focus(sim: &mut Simulation) -> Option<(i32, i32)> {
    let target = (0..sim.map_wrap.height())
        .flat_map(|y| (0..sim.map_wrap.width()).map(move |x| (x, y)))
        .filter(|&(x, y)| sim.tile(x, y).river)
        .min_by_key(|&(x, y)| {
            let diagonal: Vec<_> = [(1, 1), (-1, -1)]
                .into_iter()
                .filter_map(|(dx, dy)| {
                    sim.map_wrap
                        .offset(x, y, dx, dy)
                        .map(|p| sim.tile(p.0, p.1))
                })
                .collect();
            let lake = diagonal.iter().any(|t| t.terrain == Terrain::Lake);
            let link = diagonal.iter().any(|t| t.terrain.is_water() || t.river);
            (
                !link,
                sim.tile(x, y).terrain != Terrain::Desert,
                !lake,
                y,
                x,
            )
        });
    for tile in &mut sim.tiles {
        tile.seen = true;
        tile.visible = true;
        tile.known_by.set(HUMAN, true);
        tile.last_seen_owner = tile.owner;
    }
    target
}

pub(crate) fn ui_review_unit_status(sim: &mut Simulation) {
    sim.units.clear();
    sim.cities.clear();
    for y in 18..=33 {
        for x in 15..=25 {
            let tile = sim.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.hut = false;
            tile.base = None;
            tile.owner = None;
            tile.seen = true;
            tile.visible = true;
        }
    }
    for (i, (hp, veteran, state, work)) in [
        (20, 0, UnitState::Idle, None),
        (10, 1, UnitState::Fortified, None),
        (2, 2, UnitState::Sentry, None),
        (20, 0, UnitState::Idle, Some(WorkKind::Road)),
        (20, 0, UnitState::Idle, Some(WorkKind::Mine)),
        (20, 0, UnitState::Idle, Some(WorkKind::Irrigation)),
    ]
    .into_iter()
    .enumerate()
    {
        let kind = if work.is_some() {
            UnitKind::Worker
        } else {
            UnitKind::Rifleman
        };
        let id = sim.spawn_unit(
            HUMAN,
            kind,
            17 + (i % 3) as i32 * 3,
            21 + (i / 3) as i32 * 5,
        );
        let u = sim.units.iter_mut().find(|u| u.id == id).unwrap();
        u.veteran = veteran;
        u.hp = hp.min(u.max_hit_points());
        u.state = state;
        u.work_kind = work;
        u.work_left = i32::from(work.is_some()) * 3;
    }
    for (kind, x, automated) in [
        (UnitKind::Worker, 17, true),
        (UnitKind::Warrior, 20, true),
        (UnitKind::Rifleman, 23, false),
    ] {
        let id = sim.spawn_unit(HUMAN, kind, x, 31);
        let unit = sim.units.iter_mut().find(|u| u.id == id).unwrap();
        unit.automated = automated;
        if !automated {
            unit.state = UnitState::Fortifying;
            unit.veteran = 1;
        }
        if !automated {
            sim.spawn_unit(HUMAN, UnitKind::Warrior, x, 31);
        }
    }
}

/// Four cultural sheets currently represented by the playable roster,
/// with one city in each supported population band per style.
///
/// UI review scene using Civ III city art.
pub(crate) fn ui_review_city_sprites(sim: &mut Simulation) {
    sim.units.clear();
    sim.cities.clear();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
        tile.resource = None;
        tile.hut = false;
        tile.base = None;
        tile.owner = None;
        tile.seen = true;
        tile.visible = true;
    }
    let styles = [
        (0, "Roman"),
        (1, "European"),
        (2, "Middle Eastern"),
        (4, "Asian"),
    ];
    for (row, (owner, style)) in styles.into_iter().enumerate() {
        sim.players[owner].known.clear();
        for (col, (size, band)) in [(3, "Small"), (9, "Medium"), (15, "Large")]
            .into_iter()
            .enumerate()
        {
            let x = 15 + col as i32 * 5;
            let y = 17 + row as i32 * 6;
            sim.create_city(owner, x, y);
            let city = sim.cities.last_mut().unwrap();
            city.size = size;
            city.name = format!("{style} {band}");
        }
    }
    sim.reconcile_city_workers();
    sim.refresh_visibility();
    for tile in &mut sim.tiles {
        tile.seen = true;
        tile.visible = true;
    }
}

pub(crate) fn ui_review_city_appearance(sim: &mut Simulation) {
    sim.units.clear();
    sim.cities.clear();
    for y in 18..=29 {
        for x in 15..=25 {
            let tile = sim.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.resource = None;
            tile.hut = false;
            tile.base = None;
            tile.owner = None;
            tile.seen = true;
            tile.visible = true;
        }
    }
    for (owner, x, y, size, name, tech, walls) in [
        (0, 17, 21, 4, "Classical", None, false),
        (1, 22, 21, 5, "Walled Classical", None, true),
        (2, 17, 27, 9, "Industrial", Some(Tech::STEAM_POWER), false),
        (
            3,
            22,
            27,
            17,
            "Modern disorder",
            Some(Tech::SUPERCONDUCTORS),
            true,
        ),
    ] {
        sim.players[owner].known.clear();
        if let Some(tech) = tech {
            sim.players[owner].known.push(tech);
        }
        sim.create_city(owner, x, y);
        let c = sim.cities.last_mut().unwrap();
        c.size = size;
        c.name = name.into();
        if walls {
            c.buildings.push(Building::CITY_WALLS);
        }
        if owner != 3 {
            c.buildings.push(Building::SHAKESPEARES_THEATER);
        }
        sim.spawn_unit(owner, UnitKind::Warrior, x, y);
    }
    sim.reconcile_city_workers();
    sim.refresh_visibility();
    for tile in &mut sim.tiles {
        tile.seen = true;
        tile.visible = true;
    }
}

/// Real commands drive both cases; only their initial battle/map is staged.
pub(crate) fn ui_review_animation_setup(sim: &mut Simulation, combat: bool) -> (u32, (i32, i32)) {
    sim.units.clear();
    sim.cities.clear();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.owner = None;
        tile.base = None;
        tile.river = false;
        tile.resource = None;
        tile.hut = false;
        tile.pollution = false;
        tile.fallout = false;
        tile.road = false;
        tile.railroad = false;
        tile.visible = true;
        tile.seen = true;
    }
    let target = offset_native_by_map(20, 24, 1, 0);
    let id = sim.spawn_unit(
        HUMAN,
        if combat {
            UnitKind::Rifleman
        } else {
            UnitKind::Warrior
        },
        20,
        24,
    );
    if combat {
        sim.create_city(1, target.0, target.1);
        sim.cities.last_mut().unwrap().name = "Battlefield".into();
        sim.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        sim.cancel_pact(HUMAN, 1);
    }
    // Keep an independent rival settlement so this visual battle does
    // not trigger the game's victory modal before it can be inspected.
    sim.create_city(1, 40, 40);
    sim.spawn_unit(1, UnitKind::Warrior, 40, 40);
    sim.enable_visual_events();
    (id, target)
}

pub(crate) fn ui_review_fog(sim: &mut Simulation) -> u32 {
    sim.units.clear();
    sim.cities.clear();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
        tile.altitude = 0;
        tile.hut = false;
        tile.river = false;
        tile.river_edges = 0;
        tile.resource = None;
        tile.road = false;
        tile.railroad = false;
        tile.irrigation = false;
        tile.farmland = false;
        tile.pollution = false;
        tile.fallout = false;
        tile.base = None;
        tile.mine = false;
        tile.oil_well = false;
        tile.oil_platform = false;
        tile.seen = false;
        tile.visible = false;
        tile.owner = None;
        tile.border_city = None;
        tile.last_seen_owner = None;
        tile.stealth_visible = false;
    }
    let id = sim.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
    sim.refresh_visibility();
    for y in 0..sim.map_wrap.height() {
        for x in 0..sim.map_wrap.width() {
            let distance = sim.map_wrap.step_distance(x, y, 20, 24);
            let tile = sim.tile_mut(x, y);
            tile.seen = distance <= 6;
            tile.visible = distance <= 3;
            tile.known_by.set(HUMAN, tile.seen);
        }
    }
    id
}

/// Staged gallery knowledge is explicit and isolated from campaign sight.
/// Freeze the staged art for remembered cells as if the fixture had been seen.
pub(crate) fn ui_review_record_knowledge(sim: &mut Simulation) {
    let visible: Vec<Vec<bool>> = (0..sim.players.len())
        .map(|owner| {
            sim.tiles
                .iter()
                .map(|tile| owner == HUMAN && tile.seen)
                .collect()
        })
        .collect();
    sim.remember_render_map(&visible);
}

#[cfg(test)]
pub(crate) fn ui_review_remember_all_tiles(sim: &mut Simulation) {
    for tile in &mut sim.tiles {
        tile.seen = true;
        tile.visible = false;
        tile.known_by.set(HUMAN, true);
    }
    ui_review_record_knowledge(sim);
}

/// Controlled combinations through the normal shared world/city renderer.
pub(crate) fn ui_review_terrain_layers(sim: &mut Simulation, layers: bool) -> Option<u32> {
    sim.units.clear();
    sim.cities.clear();
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
        tile.base_terrain = TerrainBase::Grassland;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.river_edges = 0;
        tile.mine = false;
        tile.irrigation = false;
        tile.farmland = false;
        tile.oil_well = false;
        tile.oil_platform = false;
        tile.hut = false;
        tile.resource = None;
        tile.base = None;
        tile.pollution = false;
        tile.fallout = false;
        tile.owner = None;
        tile.last_seen_owner = None;
        tile.seen = true;
        tile.visible = true;
        tile.known_by.fill(true);
    }
    if layers {
        for dx in -4..=4 {
            let (x, y) = offset_native_by_map(20, 24, dx, 0);
            let tile = sim.tile_mut(x, y);
            tile.river = true;
            tile.road = true;
            tile.irrigation = true;
            if dx < 0 {
                tile.railroad = true;
            }
            if dx == 2 {
                tile.base = Some(Base::Fortress);
            }
            if dx == -3 {
                tile.mine = true;
            }
            if dx == -4 {
                tile.hut = true;
            }
            if dx == -1 {
                tile.fallout = true;
            }
        }
        sim.create_city(HUMAN, 20, 24);
        let city = sim.cities.last_mut().unwrap();
        city.name = "River Crossing".into();
        city.buildings.push(Building::CITY_WALLS);
        let id = city.id;
        sim.tile_mut(20, 24).pollution = true;
        sim.tile_mut(20, 24).irrigation = true;
        sim.tile_mut(20, 24).river = true;
        sim.tile_mut(20, 24).road = true;
        super::topology::rebuild_river_edges(sim.map_wrap, &mut sim.tiles);
        sim.spawn_unit(HUMAN, UnitKind::Warrior, 20, 24);
        Some(id)
    } else {
        for dy in -4..=4 {
            for dx in -4..=4 {
                let (x, y) = offset_native_by_map(20, 24, dx, dy);
                let terrain = match (dx, dy) {
                    (_, -4..=-3) => {
                        if dx < 0 {
                            Terrain::Sea
                        } else {
                            Terrain::Glacier
                        }
                    }
                    (-4..=-2, -2..=0) => {
                        if (dx + dy) % 2 == 0 {
                            Terrain::Forest
                        } else {
                            Terrain::Jungle
                        }
                    }
                    (1..=4, -2..=0) => {
                        if dx % 2 == 0 {
                            Terrain::Mountains
                        } else {
                            Terrain::Hills
                        }
                    }
                    (_, 1..=4) => match dx {
                        -4..=-3 => Terrain::Desert,
                        -2..=-1 => Terrain::Plains,
                        0 => Terrain::Grassland,
                        1..=2 => Terrain::Marsh,
                        _ => Terrain::Tundra,
                    },
                    _ => Terrain::Grassland,
                };
                let tile = sim.tile_mut(x, y);
                tile.terrain = terrain;
                tile.base_terrain = match terrain {
                    Terrain::Glacier => TerrainBase::Ice,
                    Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
                    Terrain::Sea => TerrainBase::Sea,
                    Terrain::Ocean => TerrainBase::Ocean,
                    Terrain::Desert | Terrain::FloodPlains => TerrainBase::Desert,
                    Terrain::Plains | Terrain::Hills | Terrain::Mountains => TerrainBase::Plains,
                    Terrain::Tundra => TerrainBase::Tundra,
                    Terrain::Grassland | Terrain::GrasslandShield => TerrainBase::Grassland,
                    Terrain::Forest | Terrain::Jungle | Terrain::Marsh | Terrain::Volcano => {
                        TerrainBase::Grassland
                    }
                };
            }
        }
        super::topology::rebuild_river_edges(sim.map_wrap, &mut sim.tiles);
        None
    }
}

fn ui_review_terrain_biomes(sim: &mut Simulation) {
    // Rows of matching biome/feature pairs, including cold relief, for
    // selection QA. No campaign generation or simulation rule changes.
    for (dy, base) in [
        (-1, TerrainBase::Grassland),
        (1, TerrainBase::Plains),
        (3, TerrainBase::Tundra),
    ] {
        for (dx, terrain) in [
            (-3, Terrain::Forest),
            (-2, Terrain::Forest),
            (0, Terrain::Mountains),
            (2, Terrain::Volcano),
        ] {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            let tile = sim.tile_mut(x, y);
            tile.terrain = terrain;
            tile.base_terrain = base;
        }
    }
}

/// Place every known resource in view for sprite review.
pub(crate) fn ui_review_resource_shadows(sim: &mut Simulation) {
    use Resource::*;

    ui_review_terrain_layers(sim, false);
    let land_resources = [
        (Terrain::Mountains, Aluminum),
        (Terrain::Hills, Coal),
        (Terrain::Grassland, Horses),
        (Terrain::Mountains, Iron),
        (Terrain::Desert, Oil),
        (Terrain::Jungle, Rubber),
        (Terrain::Hills, Saltpeter),
        (Terrain::Mountains, Uranium),
        (Terrain::Jungle, Dyes),
        (Terrain::Forest, Furs),
        (Terrain::Jungle, Gems),
        (Terrain::Desert, Incense),
        (Terrain::Plains, Ivory),
        (Terrain::Forest, Silk),
        (Terrain::Marsh, Spice),
        (Terrain::Grassland, Wine),
        (Terrain::Grassland, Cattle),
        (Terrain::Mountains, Gold),
        (Terrain::Tundra, Game),
        (Terrain::Desert, Oasis),
        (Terrain::Jungle, Sugar),
        (Terrain::Grassland, Tobacco),
        (Terrain::Jungle, TropicalFruit),
        (Terrain::Plains, Wheat),
    ];
    for (index, (terrain, resource)) in land_resources.into_iter().enumerate() {
        let dx = index as i32 % 6 - 3;
        let dy = index as i32 / 6 - 2;
        let (x, y) = offset_native_by_map(20, 24, dx, dy);
        let tile = sim.tile_mut(x, y);
        tile.terrain = terrain;
        tile.base_terrain = match terrain {
            Terrain::Glacier => TerrainBase::Ice,
            Terrain::Coast | Terrain::Lake => TerrainBase::Coast,
            Terrain::Sea => TerrainBase::Sea,
            Terrain::Ocean => TerrainBase::Ocean,
            Terrain::Desert | Terrain::FloodPlains => TerrainBase::Desert,
            Terrain::Plains | Terrain::Hills | Terrain::Mountains => TerrainBase::Plains,
            Terrain::Tundra => TerrainBase::Tundra,
            Terrain::Grassland | Terrain::GrasslandShield => TerrainBase::Grassland,
            Terrain::Forest | Terrain::Jungle | Terrain::Marsh | Terrain::Volcano => {
                TerrainBase::Grassland
            }
        };
        tile.resource = Some(resource);
    }

    // Give the ocean resources a broad, connected coastal sea so the
    // Whales and Fish do not look like they are sitting in a tiny pond.
    for dy in 1..=3 {
        for dx in 3..=6 {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            sim.tile_mut(x, y).terrain = Terrain::Sea;
        }
    }
    for (dx, resource) in [(3, Fish), (4, Whales)] {
        let (x, y) = offset_native_by_map(20, 24, dx, 2);
        sim.tile_mut(x, y).resource = Some(resource);
    }
}

pub(crate) fn ui_review_terrain_seams(sim: &mut Simulation) {
    ui_review_terrain_layers(sim, false);
    let kinds = [
        Terrain::Coast,
        Terrain::Sea,
        Terrain::Ocean,
        Terrain::Lake,
        Terrain::Glacier,
        Terrain::Desert,
        Terrain::Plains,
        Terrain::Grassland,
        Terrain::GrasslandShield,
        Terrain::Forest,
        Terrain::Jungle,
        Terrain::Hills,
        Terrain::Mountains,
        Terrain::Marsh,
        Terrain::Tundra,
        Terrain::FloodPlains,
        Terrain::Volcano,
    ];
    for tile in 0..sim.tiles.len() {
        let (x, y) = (
            tile as i32 % sim.map_wrap.width(),
            tile as i32 / sim.map_wrap.width(),
        );
        sim.tiles[tile].terrain = kinds[((x / 4 + (y / 4) * 3) as usize) % kinds.len()];
    }
}

/// A controlled island with evenly spaced Civ III water-depth rings.
/// This isolates atlas transitions from procedural coastline generation.
///
/// UI review scene using Civ III water terrain art.
pub(crate) fn ui_review_water_depths(sim: &mut Simulation) {
    ui_review_terrain_layers(sim, false);
    for tile in 0..sim.tiles.len() {
        sim.tiles[tile].terrain = Terrain::Ocean;
    }
    for dy in -8i32..=8 {
        for dx in -8i32..=8 {
            let distance = crate::sim::topology::isohex_vector_distance(dx, dy);
            let terrain = match distance {
                0..=3 => Terrain::Grassland,
                4..=5 => Terrain::Coast,
                6..=8 => Terrain::Sea,
                _ => Terrain::Ocean,
            };
            let (x, y) = sim
                .map_wrap
                .offset(20, 24, dx, dy)
                .expect("the water-depth bands stay inside the bounded map");
            sim.tile_mut(x, y).terrain = terrain;
        }
    }
}

pub(crate) fn ui_review_ice_transitions(sim: &mut Simulation) {
    ui_review_terrain_layers(sim, false);
    for dy in -8i32..=8 {
        for dx in -8i32..=8 {
            let (x, y) = offset_native_by_map(20, 24, dx, dy);
            let ground = if dx.abs() <= 2 && dy.abs() <= 2 {
                Terrain::Glacier
            } else if dy < -2 {
                Terrain::Desert
            } else if dx > 2 {
                Terrain::Grassland
            } else if dy > 2 {
                Terrain::Plains
            } else {
                Terrain::Marsh
            };
            sim.tile_mut(x, y).terrain = ground;
        }
    }
}

pub(crate) fn ui_review_route_setup(sim: &mut Simulation) -> (u32, (i32, i32)) {
    ui_review_terrain_layers(sim, false);
    for tile in &mut sim.tiles {
        tile.terrain = Terrain::Grassland;
    }
    let (x, y) = offset_native_by_map(20, 24, -2, 0);
    let id = sim.spawn_unit(HUMAN, UnitKind::Warrior, x, y);
    (id, offset_native_by_map(20, 24, 3, 0))
}

pub(crate) fn ui_review_reveal_all_tiles(sim: &mut Simulation) {
    for tile in &mut sim.tiles {
        tile.seen = true;
        tile.visible = true;
        tile.known_by.set(HUMAN, true);
    }
    ui_review_record_knowledge(sim);
}
pub(crate) fn ui_review_wrap_seam(sim: &mut Simulation) -> u32 {
    ui_review_terrain_layers(sim, false);
    for (index, tile) in sim.tiles.iter_mut().enumerate() {
        let y = index as i32 / sim.map_wrap.width();
        tile.terrain = if y == 0 {
            Terrain::Tundra
        } else if y == sim.map_wrap.height() - 1 {
            Terrain::Desert
        } else {
            Terrain::Grassland
        };
        tile.visible = true;
        tile.seen = true;
        tile.owner = None;
        tile.last_seen_owner = None;
    }
    ui_review_record_knowledge(sim);
    sim.spawn_unit(HUMAN, UnitKind::Warrior, 12, 0)
}

/// Prepares the spaceship review scene without adding setup to campaign APIs.
pub(crate) fn manual_spaceship_fixture(sim: &mut Simulation) {
    let city = sim
        .cities
        .iter()
        .find(|city| city.owner == HUMAN)
        .unwrap()
        .id;
    sim.finish_building(city, HUMAN, Building::APOLLO_PROGRAM, 600);
}

/// Three city knowledge states and two territories crossing the fog fringe.
fn ui_review_borders_fog_cities(sim: &mut Simulation) {
    ui_review_fog(sim);
    sim.units.clear();
    sim.observed_map = Default::default();
    for (owner, x, name) in [
        (HUMAN, 18, "Visible Rome"),
        (1, 23, "Remembered Babylon"),
        (1, 28, "Unknown City"),
    ] {
        sim.create_city(owner, x, 24);
        sim.cities.last_mut().unwrap().name = name.into();
    }
    for y in 0..sim.map_wrap.height() {
        for x in 0..sim.map_wrap.width() {
            let distance = sim.map_wrap.step_distance(x, y, 20, 24);
            let tile = sim.tile_mut(x, y);
            tile.seen = distance <= 6;
            tile.visible = distance <= 2;
            tile.known_by.fill(false);
            tile.known_by.set(HUMAN, tile.seen);
            tile.owner = ((16..=29).contains(&x) && (20..=28).contains(&y)).then_some(if x <= 20 {
                HUMAN
            } else {
                1
            });
            tile.last_seen_owner = if tile.seen { tile.owner } else { None };
        }
    }
    ui_review_record_knowledge(sim);
}

/// A mixed Army loses its three wounded members in turn. Both sides retain an
/// independent city so the battle can be reviewed without a victory overlay.
pub(crate) fn ui_review_army_combat_setup(
    sim: &mut Simulation,
    attacking: bool,
) -> (u32, (i32, i32), u32) {
    let (_, target) = ui_review_animation_setup(sim, true);
    sim.units.clear();
    sim.create_city(HUMAN, 10, 40);
    sim.sign_pact(HUMAN, 1, Pact::War);
    let owner = if attacking { HUMAN } else { 1 };
    let at = if attacking { (20, 24) } else { target };
    let army = sim.spawn_unit(owner, UnitKind::Army, at.0, at.1);
    for kind in [UnitKind::Swordsman, UnitKind::Horseman, UnitKind::Spearman] {
        let id = sim.spawn_unit(owner, kind, at.0, at.1);
        sim.units.iter_mut().find(|unit| unit.id == id).unwrap().hp = 1;
        assert!(sim.board_specific_ship(id, army));
    }
    let opponent_at = if attacking { target } else { (20, 24) };
    let opponent = sim.spawn_unit(
        if attacking { 1 } else { HUMAN },
        UnitKind::ModernArmor,
        opponent_at.0,
        opponent_at.1,
    );
    let unit = sim
        .units
        .iter_mut()
        .find(|unit| unit.id == opponent)
        .unwrap();
    unit.veteran = 3;
    unit.hp = unit.max_hit_points();
    sim.native_random = None;
    sim.rng = 42;
    sim.refresh_visibility();
    ui_review_record_knowledge(sim);
    sim.enable_visual_events();
    (if attacking { army } else { opponent }, target, army)
}

/// A human Bomber strikes a nearby enemy city, either landing a bombing run or
/// being intercepted by the city's SAM Battery.
pub(crate) fn ui_review_air_combat_setup(
    sim: &mut Simulation,
    with_sam: bool,
) -> (u32, (i32, i32)) {
    let (_, _) = ui_review_animation_setup(sim, false);
    sim.units.clear();
    sim.cities.clear();
    let base = (20, 24);
    let target = (22, 24);
    for dx in -2..=4 {
        for dy in -2..=2 {
            if let Some((x, y)) = sim.map_wrap.offset(base.0, base.1, dx, dy) {
                let tile = sim.tile_mut(x, y);
                tile.terrain = Terrain::Grassland;
                tile.visible = true;
                tile.seen = true;
            }
        }
    }
    sim.create_city(HUMAN, base.0, base.1);
    sim.create_city(1, target.0, target.1);
    sim.sign_pact(HUMAN, 1, Pact::War);
    if with_sam && let Some(city) = sim.cities.iter_mut().find(|c| (c.x, c.y) == target) {
        city.buildings.push(crate::rules::Building::SAM_BATTERY);
    }
    let bomber = sim.spawn_unit(HUMAN, UnitKind::Bomber, base.0, base.1);
    sim.spawn_unit(1, UnitKind::Infantry, target.0, target.1);
    // Pick a deterministic RNG state that guarantees SAM fire when `with_sam` is true.
    sim.native_random = None;
    if with_sam {
        for seed in 0..100_u64 {
            let mut probe = sim.clone();
            probe.rng = seed;
            probe.enable_visual_events();
            let _ = probe.apply(Command::Bombard(bomber, target.0, target.1));
            if probe.take_visual_events().into_iter().any(|event| {
                matches!(
                    event,
                    VisualEvent::AirMission(m)
                        if matches!(
                            m.kind,
                            super::presentation::AirMissionVisualKind::SamIntercept {
                                shot_down: true
                            }
                        )
                )
            }) {
                sim.rng = seed;
                break;
            }
        }
    } else {
        sim.rng = 42;
    }
    sim.refresh_visibility();
    ui_review_record_knowledge(sim);
    sim.enable_visual_events();
    (bomber, target)
}

/// A human ICBM strikes an enemy city, either detonating a mushroom cloud that
/// leaves nuclear fallout around the target or being intercepted by SDI Defense.
pub(crate) fn ui_review_nuclear_setup(sim: &mut Simulation, with_sdi: bool) -> (u32, (i32, i32)) {
    let (_, _) = ui_review_animation_setup(sim, false);
    sim.units.clear();
    sim.cities.clear();
    let base = (19, 24);
    let target = (22, 24);
    for dx in -3..=5 {
        for dy in -3..=3 {
            if let Some((x, y)) = sim.map_wrap.offset(base.0, base.1, dx, dy) {
                let tile = sim.tile_mut(x, y);
                tile.terrain = Terrain::Grassland;
                tile.fallout = false;
                tile.pollution = false;
                tile.visible = true;
                tile.seen = true;
            }
        }
    }
    sim.create_city(HUMAN, base.0, base.1);
    sim.create_city(1, target.0, target.1);
    sim.sign_pact(HUMAN, 1, Pact::War);
    if let Some(city) = sim.cities.iter_mut().find(|c| (c.x, c.y) == target) {
        city.size = 10;
        if with_sdi {
            city.buildings.push(crate::rules::Building::SDI_DEFENSE);
        }
    }
    let icbm = sim.spawn_unit(HUMAN, UnitKind::ICBM, base.0, base.1);
    // Place friendly observers outside the 3x3 blast radius so the entire blast
    // zone stays brightly visible before and after detonation.
    sim.spawn_unit(HUMAN, UnitKind::Explorer, 24, 24);
    sim.spawn_unit(HUMAN, UnitKind::Explorer, 22, 21);
    sim.spawn_unit(HUMAN, UnitKind::Explorer, 22, 27);
    sim.spawn_unit(1, UnitKind::Infantry, target.0, target.1);
    sim.native_random = None;
    if with_sdi {
        for seed in 0..40_u64 {
            let mut probe = sim.clone();
            probe.rng = seed;
            probe.refresh_visibility();
            probe.enable_visual_events();
            let _ = probe.apply(Command::NuclearStrike(icbm, target.0, target.1));
            if probe.take_visual_events().into_iter().any(|event| {
                matches!(
                    event,
                    VisualEvent::AirMission(m)
                        if matches!(
                            m.kind,
                            super::presentation::AirMissionVisualKind::NuclearStrike {
                                intercepted_by_sdi: true
                            }
                        )
                )
            }) {
                sim.rng = seed;
                break;
            }
        }
    } else {
        sim.rng = 42;
    }
    sim.refresh_visibility();
    ui_review_record_knowledge(sim);
    sim.enable_visual_events();
    (icbm, target)
}
