//! Civ III Great Leaders: a rare unit created when an elite land unit wins a
//! combat by killing. See `docs/superpowers/specs/2026-11-17-great-leader-design.md`.

use super::*;
use crate::rules::engine_rules::GreatLeaderSettings;

impl Simulation {
    /// True when the unit sits at the top level of its veteran system.
    pub(super) fn unit_is_elite(&self, unit: &Unit) -> bool {
        let levels = veterancy::selected_veteran_system(unit.kind).levels.len();
        levels > 1 && unit.veteran_level + 1 == levels
    }

    /// True when any living unit of `owner` is elite. Stands in for Civ III's
    /// "a victorious army" requirement for the Heroic Epic.
    pub(in crate::sim) fn owner_has_elite_unit(&self, owner: usize) -> bool {
        self.units
            .iter()
            .any(|unit| unit.owner == owner && self.unit_is_elite(unit))
    }

    /// Strength of a unit in a fight: its better of attack and defense, using
    /// the member that fights when the unit is an Army.
    fn combat_strength(&self, unit: &Unit) -> i32 {
        let unit = if unit.kind == UnitKind::Army {
            self.army_combat_member(unit, true)
                .unwrap_or_else(|| unit.clone())
        } else {
            unit.clone()
        };
        unit.kind.attack_strength().max(unit.kind.stats().defense)
    }

    /// Reason a victorious unit is ineligible to create a Great Leader, or
    /// `None` when eligible. Cheap checks come first and nothing draws from RNG.
    pub(in crate::sim) fn great_leader_ineligibility_reason(
        &self,
        winner: &Unit,
        loser: &Unit,
    ) -> Option<&'static str> {
        if winner.kind == UnitKind::Army {
            return Some("army_winner");
        }
        if winner.created_leader {
            return Some("winner_already_created_leader");
        }
        if winner.owner == BARBARIANS {
            return Some("barbarian_winner");
        }
        if loser.owner == BARBARIANS {
            return Some("barbarian_loser");
        }
        if winner.kind.is_naval() || winner.kind.is_aircraft() || winner.kind.is_missile() {
            return Some("non_land_winner");
        }
        // Only a fight against a comparable opponent earns a Leader. Beating
        // far weaker units, over and over, is farming rather than a great deed.
        let minimum = GreatLeaderSettings::campaign().min_victory_strength_percent;
        if self.combat_strength(loser) * 100 < self.combat_strength(winner) * minimum {
            return Some("trivial_victory");
        }
        if !self.unit_is_elite(winner) {
            return Some("not_elite");
        }
        // A Leader appears on the winner's tile; never create one at sea.
        if self.tile(winner.x, winner.y).terrain.is_water() {
            return Some("on_water");
        }
        let in_army = winner
            .transport
            .and_then(|id| self.unit_by_id(id))
            .is_some_and(|transport| transport.kind == UnitKind::Army);
        if in_army {
            return Some("in_army");
        }
        // A living Scientific Leader does not count: the two kinds are capped
        // separately.
        if self.units.iter().any(|unit| {
            unit.owner == winner.owner && unit.kind == UnitKind::Leader && !unit.scientific
        }) {
            return Some("already_has_leader");
        }
        None
    }

    /// Whether this kill could create a Great Leader. Cheap checks come first
    /// and nothing here draws from the RNG.
    #[cfg(test)]
    pub(in crate::sim) fn great_leader_eligible(&self, winner: &Unit, loser: &Unit) -> bool {
        self.great_leader_ineligibility_reason(winner, loser)
            .is_none()
    }

    /// The "1 in N" denominator for the owner's next eligible kill.
    pub(in crate::sim) fn great_leader_denominator(&self, owner: usize, attacked: bool) -> i32 {
        let settings = GreatLeaderSettings::campaign();
        let heroic_epic = self
            .cities
            .iter()
            .any(|city| city.owner == owner && city.has_building(Building::HEROIC_EPIC));
        match (attacked, heroic_epic) {
            (true, false) => settings.attack_denominator,
            (true, true) => settings.attack_heroic_epic_denominator,
            (false, false) => settings.defense_denominator,
            (false, true) => settings.defense_heroic_epic_denominator,
        }
    }

    /// Rolls for a Great Leader after `winner_id` killed `loser`. Returns the
    /// new Leader's id. An ineligible kill consumes no random numbers.
    #[cfg(test)]
    pub(in crate::sim) fn roll_great_leader(
        &mut self,
        winner_id: u32,
        loser: &Unit,
        winner_attacked: bool,
    ) -> Option<u32> {
        let winner = self.unit_by_id(winner_id)?.clone();
        if !self.great_leader_eligible(&winner, loser) {
            return None;
        }
        self.draw_great_leader(winner_id, winner_attacked)
    }

    /// Draws the odds for a winner already judged eligible. Combat decides
    /// eligibility from the units as they were before the fight, so a
    /// promotion earned in the same fight does not count.
    pub(in crate::sim) fn draw_great_leader(
        &mut self,
        winner_id: u32,
        winner_attacked: bool,
    ) -> Option<u32> {
        let winner = self.unit_by_id(winner_id)?.clone();
        let heroic_epic = self
            .cities
            .iter()
            .any(|city| city.owner == winner.owner && city.has_building(Building::HEROIC_EPIC));
        let denominator = self.great_leader_denominator(winner.owner, winner_attacked);
        if self.rand_below(denominator) != 0 {
            if self.trace.wants::<military_observation::LeaderEvent>() {
                self.trace_leader_event(military_observation::LeaderEvent {
                    owner: winner.owner,
                    action: "roll_missed",
                    unit_id: Some(winner_id),
                    leader_id: None,
                    army_id: None,
                    city_id: None,
                    kind: Some(winner.kind.name()),
                    building: None,
                    attacked: Some(winner_attacked),
                    denominator: Some(denominator),
                    heroic_epic: Some(heroic_epic),
                    shields_saved: None,
                    can_form_army: None,
                    members_now: None,
                    capacity: None,
                });
            }
            return None;
        }
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == winner_id) {
            unit.created_leader = true;
        }
        if winner.owner == HUMAN {
            self.add_event(format!(
                "A Great Leader has emerged from your victorious {}.",
                winner.kind.name()
            ));
        }
        let leader_id = self.spawn_unit_with_origin(
            winner.owner,
            UnitKind::Leader,
            winner.x,
            winner.y,
            lifecycle::UnitOrigin::GreatLeader { winner_id },
        );
        if self.trace.wants::<military_observation::LeaderEvent>() {
            self.trace_leader_event(military_observation::LeaderEvent {
                owner: winner.owner,
                action: "spawned",
                unit_id: Some(winner_id),
                leader_id: Some(leader_id),
                army_id: None,
                city_id: None,
                kind: Some(winner.kind.name()),
                building: None,
                attacked: Some(winner_attacked),
                denominator: Some(denominator),
                heroic_epic: Some(heroic_epic),
                shields_saved: None,
                can_form_army: None,
                members_now: None,
                capacity: None,
            });
        }
        Some(leader_id)
    }

    /// Rolls for a Scientific Great Leader after `owner` researched `tech`
    /// first in the world. It is born in the capital, at most one lives per
    /// owner, and a nation with the scientific trait has the better chance.
    /// Returns the new Leader's id. A capped or cityless owner draws nothing.
    pub(in crate::sim) fn roll_scientific_leader(
        &mut self,
        owner: usize,
        tech: Tech,
    ) -> Option<u32> {
        if owner == BARBARIANS
            || self
                .units
                .iter()
                .any(|unit| unit.owner == owner && unit.kind == UnitKind::Leader && unit.scientific)
        {
            return None;
        }
        let city = self
            .cities
            .iter()
            .find(|city| city.owner == owner && city.has_building(Building::PALACE))
            .or_else(|| self.cities.iter().find(|city| city.owner == owner))?;
        let (x, y, name) = (city.x, city.y, city.name.clone());
        let settings = GreatLeaderSettings::campaign();
        let percent = if self
            .ai_civilization(owner)
            .is_some_and(|civ| civ.scientific)
        {
            settings.scientific_leader_trait_percent
        } else {
            settings.scientific_leader_percent
        };
        if self.rand_below(100) >= percent {
            return None;
        }
        let leader_id = self.spawn_unit_with_origin(
            owner,
            UnitKind::Leader,
            x,
            y,
            lifecycle::UnitOrigin::ScientificLeader {
                technology: tech.name(),
            },
        );
        if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == leader_id) {
            unit.scientific = true;
        }
        if owner == HUMAN {
            self.add_event(format!(
                "A Scientific Great Leader has emerged in {name} from your discovery of {}.",
                tech.name()
            ));
        }
        Some(leader_id)
    }

    /// Ids and owners of an owner's Leaders, taken before an operation that
    /// may reassign units.
    pub(in crate::sim) fn leaders_of(&self, owner: usize) -> Vec<(u32, usize)> {
        self.units
            .iter()
            .filter(|unit| unit.owner == owner && unit.kind == UnitKind::Leader)
            .map(|unit| (unit.id, unit.owner))
            .collect()
    }

    /// A Leader never changes owner. When a city trade, incitement or civil
    /// war reassigns one, it is lost instead.
    pub(in crate::sim) fn remove_leaders_that_changed_owner(&mut self, before: &[(u32, usize)]) {
        let lost: Vec<u32> = before
            .iter()
            .filter(|&&(id, owner)| self.unit_by_id(id).is_some_and(|unit| unit.owner != owner))
            .map(|&(id, _)| id)
            .collect();
        if !lost.is_empty() {
            self.remove_units_with_cargo_because(lost, lifecycle::UnitRemoval::CityLost);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::veterancy::selected_veteran_system;

    fn elite_level(kind: UnitKind) -> usize {
        selected_veteran_system(kind).levels.len() - 1
    }

    fn game() -> Simulation {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game
    }

    fn spawn_elite(game: &mut Simulation, owner: usize, kind: UnitKind, x: i32, y: i32) -> u32 {
        let id = game.spawn_unit(owner, kind, x, y);
        let level = elite_level(kind);
        game.units
            .iter_mut()
            .find(|unit| unit.id == id)
            .unwrap()
            .veteran_level = level;
        id
    }

    fn unit(game: &Simulation, id: u32) -> Unit {
        game.unit_by_id(id).unwrap().clone()
    }

    #[test]
    fn eligibility_requires_an_elite_land_winner() {
        let mut game = game();
        let loser_id = game.spawn_unit(1, UnitKind::Pikeman, 11, 24);
        let loser = unit(&game, loser_id);
        let winner = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        assert!(game.great_leader_eligible(&unit(&game, winner), &loser));

        let out_of_range = game.spawn_unit(HUMAN, UnitKind::Knight, 13, 24);
        game.units
            .iter_mut()
            .find(|u| u.id == out_of_range)
            .unwrap()
            .veteran_level = u8::MAX as usize;
        assert!(
            !game.great_leader_eligible(&unit(&game, out_of_range), &loser),
            "only the top level is elite, not any level beyond the table"
        );

        let veteran = game.spawn_unit(HUMAN, UnitKind::Knight, 12, 24);
        game.units
            .iter_mut()
            .find(|u| u.id == veteran)
            .unwrap()
            .veteran_level = elite_level(UnitKind::Knight) - 1;
        assert!(!game.great_leader_eligible(&unit(&game, veteran), &loser));

        for kind in [UnitKind::AegisCruiser, UnitKind::Fighter] {
            let id = spawn_elite(&mut game, HUMAN, kind, 14, 24);
            assert!(
                !game.great_leader_eligible(&unit(&game, id), &loser),
                "{kind:?} winners never create Leaders"
            );
        }

        let barbarian = game.spawn_unit(BARBARIANS, UnitKind::Warrior, 11, 26);
        assert!(!game.great_leader_eligible(&unit(&game, winner), &unit(&game, barbarian)));

        let army = game.spawn_unit(HUMAN, UnitKind::Army, 16, 24);
        let loaded = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 16, 24);
        game.units
            .iter_mut()
            .find(|u| u.id == loaded)
            .unwrap()
            .transport = Some(army);
        assert!(!game.great_leader_eligible(&unit(&game, loaded), &loser));

        game.units
            .iter_mut()
            .find(|u| u.id == winner)
            .unwrap()
            .created_leader = true;
        assert!(!game.great_leader_eligible(&unit(&game, winner), &loser));
        game.units
            .iter_mut()
            .find(|u| u.id == winner)
            .unwrap()
            .created_leader = false;

        game.spawn_unit(HUMAN, UnitKind::Leader, 18, 24);
        assert!(
            !game.great_leader_eligible(&unit(&game, winner), &loser),
            "an owner with a living Leader cannot make another"
        );
    }

    #[test]
    fn trivial_victories_cannot_create_leaders() {
        let mut game = game();
        let winner = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        let winner = unit(&game, winner);
        let weak = game.spawn_unit(1, UnitKind::Warrior, 11, 24);
        assert_eq!(
            game.great_leader_ineligibility_reason(&winner, &unit(&game, weak)),
            Some("trivial_victory"),
            "an elite Knight beating a Warrior is not a deed worth a Leader"
        );
        for kind in [UnitKind::Pikeman, UnitKind::Knight] {
            let peer = game.spawn_unit(1, kind, 12, 24);
            assert_eq!(
                game.great_leader_ineligibility_reason(&winner, &unit(&game, peer)),
                None,
                "{kind:?} is a comparable opponent"
            );
        }
        let modern = spawn_elite(&mut game, HUMAN, UnitKind::Tank, 14, 24);
        let musketeer = game.spawn_unit(1, UnitKind::Musketeer, 15, 24);
        assert_eq!(
            game.great_leader_ineligibility_reason(&unit(&game, modern), &unit(&game, musketeer)),
            Some("trivial_victory"),
            "the rule scales with the winner's era"
        );
    }

    fn scientific_civilization() -> String {
        crate::rules::native::civilizations()
            .civilizations
            .iter()
            .find(|civ| civ.scientific)
            .expect("the catalog has a scientific civilization")
            .id
            .clone()
    }

    /// Fresh games over many seeds: how many first discoveries yield a Leader.
    fn scientific_leaders_in(trials: u32, civilization: Option<&str>) -> u32 {
        let mut created = 0;
        for seed in 0..trials {
            let mut game = game();
            game.set_random_seed(seed);
            game.players[HUMAN].civilization_id = civilization.map(str::to_owned);
            game.create_city(HUMAN, 20, 24);
            if let Some(id) = game.roll_scientific_leader(HUMAN, Tech::POTTERY) {
                let leader = unit(&game, id);
                assert!(leader.scientific && leader.kind == UnitKind::Leader);
                let capital = game
                    .cities
                    .iter()
                    .find(|city| city.owner == HUMAN && city.has_building(Building::PALACE))
                    .map_or((20, 24), |city| (city.x, city.y));
                assert_eq!((leader.x, leader.y), capital, "born in the capital");
                created += 1;
            }
        }
        created
    }

    #[test]
    fn a_first_discovery_creates_a_scientific_leader_at_about_three_percent() {
        let created = scientific_leaders_in(4000, None);
        assert!(
            (70..=170).contains(&created),
            "3% of 4000 is about 120, got {created}"
        );
    }

    #[test]
    fn scientific_civilizations_get_the_higher_five_percent_chance() {
        let id = scientific_civilization();
        let created = scientific_leaders_in(4000, Some(&id));
        assert!(
            (140..=260).contains(&created),
            "5% of 4000 is about 200, got {created}"
        );
    }

    #[test]
    fn an_owner_keeps_one_scientific_leader_and_the_limits_are_independent() {
        let mut game = game();
        game.create_city(HUMAN, 20, 24);
        let existing = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
        game.units
            .iter_mut()
            .find(|u| u.id == existing)
            .unwrap()
            .scientific = true;
        let before = game.random_snapshot();
        assert_eq!(game.roll_scientific_leader(HUMAN, Tech::POTTERY), None);
        assert_eq!(game.random_snapshot(), before, "no draw when capped");

        // A living Scientific Leader does not block a military Leader...
        let winner = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        let loser = game.spawn_unit(1, UnitKind::Knight, 11, 24);
        assert_eq!(
            game.great_leader_ineligibility_reason(&unit(&game, winner), &unit(&game, loser)),
            None
        );
        // ...and a living military Leader does not block a Scientific one.
        let mut game = self::game();
        game.create_city(HUMAN, 20, 24);
        game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
        let before = game.random_snapshot();
        for seed in 0..4000 {
            game.set_random_seed(seed);
            if game.roll_scientific_leader(HUMAN, Tech::POTTERY).is_some() {
                return;
            }
        }
        panic!("a military Leader must not stop Scientific Leader creation, {before:?}");
    }

    #[test]
    fn ineligible_combat_draws_no_rng() {
        let mut game = game();
        let loser = game.spawn_unit(1, UnitKind::Pikeman, 11, 24);
        let loser = unit(&game, loser);
        let green = game.spawn_unit(HUMAN, UnitKind::Knight, 10, 24);
        let before = game.random_snapshot();
        assert_eq!(game.roll_great_leader(green, &loser, true), None);
        assert_eq!(game.random_snapshot(), before);

        let elite = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 12, 24);
        let before = game.random_snapshot();
        game.roll_great_leader(elite, &loser, true);
        assert_ne!(game.random_snapshot(), before, "an eligible roll draws");
    }

    #[test]
    fn roll_eventually_creates_one_leader_on_the_winners_tile() {
        let mut game = game();
        let loser = game.spawn_unit(1, UnitKind::Pikeman, 11, 24);
        let loser = unit(&game, loser);
        let winner = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        let mut created = None;
        for _ in 0..2000 {
            created = game.roll_great_leader(winner, &loser, true);
            if created.is_some() {
                break;
            }
        }
        let id = created.expect("1 in 16 succeeds within 2000 rolls");
        let leader = unit(&game, id);
        assert_eq!(leader.kind, UnitKind::Leader);
        assert_eq!((leader.owner, leader.x, leader.y), (HUMAN, 10, 24));
        assert!(unit(&game, winner).created_leader);
        for _ in 0..200 {
            assert_eq!(game.roll_great_leader(winner, &loser, true), None);
        }
        let leaders = game
            .units
            .iter()
            .filter(|u| u.kind == UnitKind::Leader)
            .count();
        assert_eq!(leaders, 1);
    }

    #[test]
    fn heroic_epic_uses_the_better_odds() {
        let mut game = game();
        assert_eq!(game.great_leader_denominator(HUMAN, true), 16);
        assert_eq!(game.great_leader_denominator(HUMAN, false), 32);
        game.create_city(1, 30, 24);
        game.cities[0].buildings.push(Building::HEROIC_EPIC);
        assert_eq!(
            game.great_leader_denominator(HUMAN, true),
            16,
            "another civilization's Heroic Epic does not help"
        );
        game.create_city(HUMAN, 20, 24);
        let ours = game.cities.iter().position(|c| c.owner == HUMAN).unwrap();
        game.cities[ours].buildings.push(Building::HEROIC_EPIC);
        assert_eq!(game.great_leader_denominator(HUMAN, true), 12);
        assert_eq!(game.great_leader_denominator(HUMAN, false), 24);
    }

    #[test]
    fn created_leader_is_recorded_in_lifecycle() {
        let mut game = game();
        game.trace = trace::Collector::enabled(std::collections::BTreeSet::from(["*".into()]), 64);
        game.start_lifecycle_observation();
        let loser = game.spawn_unit(1, UnitKind::Pikeman, 11, 24);
        let loser = unit(&game, loser);
        let winner = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        while game.roll_great_leader(winner, &loser, true).is_none() {}
        let changes = serde_json::to_string(&game.observe_lifecycle().changes).unwrap();
        assert!(changes.contains("\"source\":\"great_leader\""), "{changes}");
    }

    #[test]
    fn heroic_epic_needs_an_elite_unit() {
        let mut game = game();
        game.create_city(HUMAN, 20, 24);
        let city = game.cities[0].clone();
        let blocker = game.building_blocker(&city, Building::HEROIC_EPIC);
        assert!(
            blocker
                .as_deref()
                .is_some_and(|text| text.contains("elite unit")),
            "{blocker:?}"
        );
        assert!(!game.can_produce(&city, Production::Building(Building::HEROIC_EPIC)));
        spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        assert_eq!(game.building_blocker(&city, Building::HEROIC_EPIC), None);
        assert!(game.can_produce(&city, Production::Building(Building::HEROIC_EPIC)));
    }

    #[test]
    fn owner_has_elite_unit_ignores_other_owners_and_civilians() {
        let mut game = game();
        spawn_elite(&mut game, 1, UnitKind::Knight, 10, 24);
        game.spawn_unit(HUMAN, UnitKind::Worker, 12, 24);
        game.spawn_unit(HUMAN, UnitKind::Knight, 14, 24);
        assert!(!game.owner_has_elite_unit(HUMAN));
        assert!(game.owner_has_elite_unit(1));
        spawn_elite(&mut game, HUMAN, UnitKind::Knight, 16, 24);
        assert!(game.owner_has_elite_unit(HUMAN));
    }

    #[test]
    fn a_winner_at_sea_creates_no_leader() {
        let mut game = game();
        let loser = game.spawn_unit(1, UnitKind::Marine, 11, 24);
        let loser = unit(&game, loser);
        game.tile_mut(10, 24).terrain = Terrain::Ocean;
        let marine = spawn_elite(&mut game, HUMAN, UnitKind::Marine, 10, 24);
        assert!(
            !game.great_leader_eligible(&unit(&game, marine), &loser),
            "a Leader would appear on an ocean tile"
        );
        game.tile_mut(10, 24).terrain = Terrain::Grassland;
        assert!(game.great_leader_eligible(&unit(&game, marine), &loser));
    }

    #[test]
    fn creating_a_leader_tells_the_human_player() {
        let mut game = game();
        let loser = game.spawn_unit(1, UnitKind::Pikeman, 11, 24);
        let loser = unit(&game, loser);
        let winner = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
        while game.roll_great_leader(winner, &loser, true).is_none() {}
        assert!(
            game.events
                .iter()
                .any(|event| event.contains("Great Leader")),
            "{:?}",
            game.events
        );
    }

    #[test]
    fn promotion_in_the_same_fight_does_not_make_the_winner_eligible() {
        for seed in 0..4000u32 {
            let mut game = game();
            game.set_random_seed(seed);
            let (nx, ny) = game.map_wrap.offset(10, 24, 1, 0).unwrap();
            for (x, y) in [(10, 24), (nx, ny)] {
                game.tile_mut(x, y).terrain = Terrain::Grassland;
            }
            game.sign_pact(HUMAN, 1, Pact::War);
            let attacker = game.spawn_unit(HUMAN, UnitKind::Knight, 10, 24);
            let top = elite_level(UnitKind::Knight);
            game.units
                .iter_mut()
                .find(|u| u.id == attacker)
                .unwrap()
                .veteran_level = top - 1;
            game.spawn_unit(1, UnitKind::Pikeman, nx, ny);
            let direction = game.direction_for_unit_offset(attacker, 1, 0).unwrap();
            game.apply(Command::MoveByDirection(attacker, direction));
            let promoted = game
                .unit_by_id(attacker)
                .is_some_and(|unit| unit.veteran_level == top);
            let leader = game.units.iter().any(|u| u.kind == UnitKind::Leader);
            assert!(
                !(promoted && leader),
                "seed {seed}: a unit promoted to elite in this fight created a Leader"
            );
        }
    }

    fn attack_until_leader(attacker_is_elite: bool) -> bool {
        for seed in 0..3000u32 {
            let mut game = game();
            game.set_random_seed(seed);
            let (nx, ny) = game.map_wrap.offset(10, 24, 1, 0).unwrap();
            for (x, y) in [(10, 24), (nx, ny)] {
                game.tile_mut(x, y).terrain = Terrain::Grassland;
            }
            game.sign_pact(HUMAN, 1, Pact::War);
            let attacker = if attacker_is_elite {
                let id = spawn_elite(&mut game, HUMAN, UnitKind::Knight, 10, 24);
                game.spawn_unit(1, UnitKind::Pikeman, nx, ny);
                id
            } else {
                // Commands only drive the human player's units, so the human
                // attacks and the other civilization's elite defender wins.
                spawn_elite(&mut game, 1, UnitKind::Pikeman, nx, ny);
                game.spawn_unit(HUMAN, UnitKind::Horseman, 10, 24)
            };
            let direction = game.direction_for_unit_offset(attacker, 1, 0).unwrap();
            game.apply(Command::MoveByDirection(attacker, direction));
            if game.units.iter().any(|u| u.kind == UnitKind::Leader) {
                return true;
            }
        }
        false
    }

    #[test]
    fn combat_victory_by_an_elite_attacker_can_create_a_leader() {
        assert!(attack_until_leader(true));
    }

    #[test]
    fn combat_victory_by_an_elite_defender_can_create_a_leader() {
        assert!(attack_until_leader(false));
    }
}
