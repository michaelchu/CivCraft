//! Shared default PF action policy from common/aicore/pf_tools.c. Planning
//! capabilities deliberately differ from legal action execution.
use super::movement_rules::{MovementRules, MovementTile};
use super::*;

impl UnitKind {
    // IgZOC and absence of UCF_ZOC are separate native exemptions.
    pub(super) fn ignores_zoc(self) -> bool {
        super::action_rules::freeciv_unit_type_flag(self, "IgZOC")
            .expect("campaign C2C3 unit-type IgZOC binding")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PfAction {
    None,
    Attack,
    Impossible,
}

impl Simulation {
    fn pf_class_can_occupy_city(kind: UnitKind) -> bool {
        MovementRules::campaign()
            .unit_class_movement_data_for_kind(kind)
            .expect("campaign C2C3 unit-class movement data")
            .can_occupy_city
    }

    // tile_move_cost_ptrs: cost only; scopes/action callbacks decide legality.
    pub(super) fn raw_move_cost_for(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        let kind = unit.kind;
        let rules = MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class binding");
        let ignores = super::action_rules::freeciv_unit_type_flag(kind, "IgTer")
            .expect("campaign C2C3 unit-type IgTer binding");
        let source = self.tile(unit.x, unit.y);
        let dest = self.tile(x, y);
        let restricted = [source.owner, dest.owner]
            .into_iter()
            .flatten()
            .any(|owner| self.at_war(unit.owner, owner));
        rules
            .step_cost(
                MovementTile::new(
                    source.terrain,
                    source.road,
                    source.railroad,
                    source.river,
                    source.base,
                ),
                MovementTile::new(
                    dest.terrain,
                    dest.road,
                    dest.railroad,
                    dest.river,
                    dest.base,
                ),
                class,
                self.pf_move_is_cardinal((unit.x, unit.y), (x, y)),
                restricted,
                ignores,
            )
            .expect("campaign C2C3 step movement data")
    }
    pub(super) fn pf_native_tile(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        let rules = MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class binding");
        match rules.tile_native_to_class(
            tile.terrain,
            tile.road,
            tile.railroad,
            tile.river,
            tile.base,
            class,
        ) {
            Ok(native) => native,
            // Volcano has no C2C3 terrain identity. Its Mountains adaptation
            // applies only to movement cost, not native/action requirements.
            Err(super::movement_rules::MovementRulesError::UnsupportedTerrain(_)) => false,
            Err(error) => panic!("campaign C2C3 native-tile data is invalid: {error:?}"),
        }
    }

    /// Movement scope uses its explicit Volcano→Mountains adapter; generic PF
    /// action/native requirements continue to use `pf_native_tile` above.
    pub(super) fn pf_movement_native_tile(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        let rules = MovementRules::campaign();
        let class = rules
            .unit_class_for_kind(kind)
            .expect("campaign C2C3 unit class binding");
        rules
            .movement_tile_native_to_class(
                tile.terrain,
                tile.road,
                tile.railroad,
                tile.river,
                tile.base,
                class,
            )
            .expect("campaign C2C3 movement native_to data")
    }

    /// Freeciv's C2C3 ISO|HEX topology marks six of CivCraft's eight steps as
    /// cardinal. Delegate to movement's shared topology adapter so direct,
    /// observed, and PF path costs classify X-wrap and bounded-Y steps alike.
    fn pf_move_is_cardinal(&self, from: (i32, i32), to: (i32, i32)) -> bool {
        self.movement_is_cardinal(from, to)
    }

    pub(super) fn pf_main_sight(&self, observer: usize, x: i32, y: i32) -> bool {
        self.has_main_sight_at(observer, x, y)
    }

    fn pf_attack_possible(&self, kind: UnitKind, owner: usize, x: i32, y: i32, seen: bool) -> bool {
        // can_attack_non_native checks actual action capabilities as well
        // as Sea's flag. Carrier attack0 does NOT permit shore attacks,
        // even though the PF default flags can attack native sea targets.
        let shore = kind.is_naval()
            && !matches!(kind, UnitKind::Galley | UnitKind::Submarine)
            && kind.attack_strength() > 0;
        if !shore && !self.pf_native_tile(kind, x, y) {
            return false;
        }
        if !seen {
            return true;
        }
        let city = self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner));
        let mut any = false;
        for unit in self.units.iter().filter(|unit| (unit.x, unit.y) == (x, y)) {
            if !self.at_war(owner, unit.owner) {
                return false;
            }
            let native_base = unit.kind.is_aircraft()
                && matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase));
            if !unit.kind.unit_class().is_unreachable()
                || kind.can_attack_aircraft()
                || native_base
                || city
            {
                any = true;
            }
            // unreachableprotects is disabled by Civ2Civ3: unreachable
            // aircraft do not protect a reachable ship in the same stack.
        }
        any
    }

    pub(super) fn pf_default_action(
        &self,
        kind: UnitKind,
        owner: usize,
        position: (i32, i32),
        known: bool,
        seen: bool,
        city_attack: bool,
    ) -> PfAction {
        let (x, y) = position;
        if !known {
            return PfAction::None;
        }
        let city = self
            .cities
            .iter()
            .any(|city| (city.x, city.y) == (x, y) && !self.allied(owner, city.owner));
        if city {
            if kind == UnitKind::Explorer {
                return PfAction::None;
            }
        }
        let units = self
            .units
            .iter()
            .any(|unit| (unit.x, unit.y) == (x, y) && !self.allied(owner, unit.owner));
        if units {
            if !kind.is_civilian() {
                return if self.pf_attack_possible(kind, owner, x, y, seen) {
                    PfAction::Attack
                } else {
                    PfAction::Impossible
                };
            }
        }
        if city && city_attack && !kind.is_civilian() {
            return if Self::pf_class_can_occupy_city(kind)
                || self.pf_attack_possible(kind, owner, x, y, false)
            {
                PfAction::Attack
            } else {
                PfAction::Impossible
            };
        }
        PfAction::None
    }

    pub(super) fn pf_action_step_possible(&self, actor: &Unit, action: PfAction) -> bool {
        if action == PfAction::Attack {
            self.pf_native_tile(actor.kind, actor.x, actor.y)
                || actor.kind.has_marine_attack()
                || (actor.kind.is_naval() && actor.kind.attack_strength() > 0)
        } else {
            // Units can act from transport scope; native/city scope also permits actions.
            self.pf_native_tile(actor.kind, actor.x, actor.y)
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (actor.x, actor.y)
                        && (self.allied(actor.owner, city.owner)
                            || Self::pf_class_can_occupy_city(actor.kind))
                })
                || actor.transport.is_some()
                || self.boarding_target(actor, actor.x, actor.y).is_some()
        }
    }

    pub(super) fn pf_extra_class_defense(&self, kind: UnitKind, x: i32, y: i32) -> i32 {
        // Forts/bases use Defend_Bonus effects, NOT extra.defense_bonus.
        // River is the only provided extra with that attribute.
        if self.tile(x, y).river
            && ((!kind.is_naval() && !kind.is_aircraft() && !kind.is_big_land())
                || kind == UnitKind::Galley)
        {
            25
        } else {
            0
        }
    }

    pub(super) fn pf_physical_attack(
        &self,
        enemy: &Unit,
        target: UnitKind,
        x: i32,
        y: i32,
    ) -> bool {
        if enemy.kind.attack_strength() <= 0
            || !self.pf_action_step_possible(enemy, PfAction::Attack)
        {
            return false;
        }
        let shore =
            enemy.kind.is_naval() && !matches!(enemy.kind, UnitKind::Submarine | UnitKind::Galley);
        if !shore && !self.pf_native_tile(enemy.kind, x, y) {
            return false;
        }
        !target.is_aircraft()
            || enemy.kind.can_attack_aircraft()
            || self.cities.iter().any(|city| (city.x, city.y) == (x, y))
            || matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
    }

    pub(super) fn pf_action_move_cost(kind: UnitKind, action: PfAction, rate: i32) -> i32 {
        let rules = MovementRules::campaign();
        let fragments = rules
            .move_fragments()
            .expect("campaign C2C3 movement fragments");
        let class = rules
            .unit_class_movement_data_for_kind(kind)
            .expect("campaign C2C3 unit-class movement data");
        let one_attack = super::action_rules::freeciv_unit_type_flag(kind, "OneAttack")
            .expect("campaign C2C3 unit-type OneAttack binding");
        // C2C3's Attack consumes the full move rate for OneAttack units and
        // Missile-class units (the latter can use Suicide Attack). Bombard is
        // a separate C2C3 action with its own move-cost effect; CivCraft PF
        // does not model that action yet.
        if action == PfAction::Attack && (one_attack || class.missile) {
            rate
        } else {
            fragments
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::movement_rules::{RoadMoveMode, RoadType};
    use super::*;

    #[test]
    fn unit_zoc_exemptions_use_c2c3_flags_and_native_extras() {
        for kind in [
            UnitKind::Chariot,
            UnitKind::Catapult,
            UnitKind::Cannon,
            UnitKind::Galley,
        ] {
            assert!(!kind.ignores_zoc(), "{kind:?}");
        }
        assert!(UnitKind::Explorer.ignores_zoc());
        // Civ III's similarly named Guerilla ID does not bind to the C2C3
        // Partisan record, so it uses the selected default (no IgZOC).
        assert!(!UnitKind::Guerilla.ignores_zoc());
        assert!(UnitKind::Leader.ignores_zoc());
        assert!(!UnitKind::Carrier.ignores_zoc());
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        for terrain in [Terrain::Jungle, Terrain::Mountains, Terrain::Marsh] {
            let tile = sim.tile_mut(10, 20);
            tile.terrain = terrain;
            tile.road = false;
            tile.railroad = false;
            tile.base = None;
            tile.river = true;
            assert!(!sim.pf_native_tile(UnitKind::Chariot, 10, 20));
            assert!(sim.pf_native_tile(UnitKind::Warrior, 10, 20));
            sim.tile_mut(10, 20).base = Some(Base::Fort);
            assert!(sim.pf_native_tile(UnitKind::Chariot, 10, 20));
            sim.tile_mut(10, 20).base = None;
            sim.tile_mut(10, 20).road = true;
            assert!(sim.pf_native_tile(UnitKind::Chariot, 10, 20));
        }
    }

    #[test]
    fn pf_flags_use_civilian_and_military_campaign_actions() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        sim.tile_mut(10, 20).terrain = Terrain::Grassland;
        sim.spawn_unit(1, UnitKind::Warrior, 10, 20);
        for kind in [UnitKind::Settler] {
            assert_eq!(
                sim.pf_default_action(kind, HUMAN, (10, 20), true, true, true),
                PfAction::None
            );
        }
        assert_eq!(
            sim.pf_default_action(UnitKind::Explorer, HUMAN, (10, 20), true, true, true),
            PfAction::None
        );
        assert_eq!(
            sim.pf_default_action(UnitKind::Carrier, HUMAN, (10, 20), true, true, true),
            PfAction::Attack
        );
        sim.tile_mut(10, 20).terrain = Terrain::Sea;
        sim.units.clear();
        sim.spawn_unit(1, UnitKind::Caravel, 10, 20);
        assert_eq!(
            sim.pf_default_action(UnitKind::Carrier, HUMAN, (10, 20), true, true, true),
            PfAction::Attack
        );
        sim.tile_mut(10, 20).terrain = Terrain::Grassland;
        sim.create_city(1, 10, 20);
    }

    fn clear_movement_tile(sim: &mut Simulation, position: (i32, i32), terrain: Terrain) {
        let tile = sim.tile_mut(position.0, position.1);
        tile.terrain = terrain;
        tile.owner = None;
        tile.road = false;
        tile.railroad = false;
        tile.river = false;
        tile.base = None;
    }

    #[test]
    fn pf_terrain_costs_use_campaign_terrain_records_and_move_fragments() {
        let rules = MovementRules::campaign();
        let fragments = rules.move_fragments().unwrap();
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        let from = (10, 20);
        let to = sim.map_wrap.civ3_neighbor(from.0, from.1, 1).unwrap();
        sim.spawn_unit(HUMAN, UnitKind::Warrior, from.0, from.1);
        let actor = sim.units[0].clone();

        for terrain in [
            Terrain::Grassland,
            Terrain::Hills,
            Terrain::Mountains,
            Terrain::Forest,
        ] {
            clear_movement_tile(&mut sim, from, Terrain::Grassland);
            clear_movement_tile(&mut sim, to, terrain);
            let expected = rules.terrain_movement_cost(terrain).unwrap() * fragments;
            assert_eq!(
                sim.raw_move_cost_for(&actor, to.0, to.1),
                expected,
                "{terrain:?}"
            );
        }
        assert_eq!(fragments, 6);
        // The explicit Volcano movement adapter borrows Mountain movement
        // cost/nativity, while generic native/action requirements stay unsupported.
        assert_eq!(rules.terrain_movement_cost(Terrain::Volcano).unwrap(), 3);
        clear_movement_tile(&mut sim, to, Terrain::Volcano);
        assert!(!sim.pf_native_tile(UnitKind::Warrior, to.0, to.1));
        assert!(sim.pf_movement_native_tile(UnitKind::Warrior, to.0, to.1));
        assert_eq!(
            sim.raw_move_cost_for(&actor, to.0, to.1),
            rules.terrain_movement_cost(Terrain::Volcano).unwrap() * fragments
        );
    }

    #[test]
    fn river_road_mode_is_cardinal_while_regular_roads_are_fast_always() {
        let rules = MovementRules::campaign();
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        let from = (10, 20);
        let cardinal = sim.map_wrap.civ3_neighbor(from.0, from.1, 1).unwrap();
        let diagonal = sim.map_wrap.civ3_neighbor(from.0, from.1, 2).unwrap();
        sim.spawn_unit(HUMAN, UnitKind::Warrior, from.0, from.1);
        let actor = sim.units[0].clone();
        let base_cost = rules.terrain_movement_cost(Terrain::Grassland).unwrap()
            * rules.move_fragments().unwrap();
        let river_cost = rules.road_move_cost(RoadType::River).unwrap();
        let road_cost = rules.road_move_cost(RoadType::Road).unwrap();
        assert_eq!(
            rules.road_move_mode(RoadType::River).unwrap(),
            RoadMoveMode::Cardinal
        );
        assert_eq!(
            rules.road_move_mode(RoadType::Road).unwrap(),
            RoadMoveMode::FastAlways
        );

        clear_movement_tile(&mut sim, from, Terrain::Grassland);
        clear_movement_tile(&mut sim, cardinal, Terrain::Grassland);
        sim.tile_mut(from.0, from.1).river = true;
        sim.tile_mut(cardinal.0, cardinal.1).river = true;
        assert!(sim.pf_move_is_cardinal(from, cardinal));
        assert_eq!(
            sim.raw_move_cost_for(&actor, cardinal.0, cardinal.1),
            base_cost.min(river_cost)
        );
        sim.spawn_unit(HUMAN, UnitKind::Chariot, from.0, from.1);
        let big_land_actor = sim.units[1].clone();
        assert_eq!(
            sim.raw_move_cost_for(&big_land_actor, cardinal.0, cardinal.1),
            base_cost,
            "C2C3 River native_to excludes Big Land units"
        );

        clear_movement_tile(&mut sim, diagonal, Terrain::Grassland);
        sim.tile_mut(diagonal.0, diagonal.1).river = true;
        assert!(!sim.pf_move_is_cardinal(from, diagonal));
        assert_eq!(
            sim.raw_move_cost_for(&actor, diagonal.0, diagonal.1),
            base_cost
        );

        sim.tile_mut(from.0, from.1).river = false;
        sim.tile_mut(diagonal.0, diagonal.1).river = false;
        sim.tile_mut(from.0, from.1).road = true;
        sim.tile_mut(diagonal.0, diagonal.1).road = true;
        assert_eq!(
            sim.raw_move_cost_for(&actor, diagonal.0, diagonal.1),
            base_cost.min(road_cost)
        );

        // X-wrap seam adjacency retains the intended six-cardinal/two-diagonal
        // direction classification; the north/south edges remain bounded.
        let seam_from = (0, 20);
        let seam_cardinal = sim
            .map_wrap
            .civ3_neighbor(seam_from.0, seam_from.1, 7)
            .unwrap();
        let seam_diagonal = sim
            .map_wrap
            .civ3_neighbor(seam_from.0, seam_from.1, 6)
            .unwrap();
        assert_eq!(seam_cardinal.0, sim.map_wrap.width() - 1);
        assert_eq!(seam_diagonal.0, sim.map_wrap.width() - 1);
        assert!(sim.pf_move_is_cardinal(seam_from, seam_cardinal));
        assert!(!sim.pf_move_is_cardinal(seam_from, seam_diagonal));
        assert!(sim.map_wrap.civ3_neighbor(10, 0, 0).is_none());
    }

    #[test]
    fn attack_move_cost_uses_c2c3_one_attack_and_missile_rules() {
        let fragments = MovementRules::campaign().move_fragments().unwrap();
        let rate = 3 * fragments;
        assert_eq!(
            Simulation::pf_action_move_cost(UnitKind::Warrior, PfAction::Attack, rate),
            fragments
        );
        assert_eq!(
            Simulation::pf_action_move_cost(UnitKind::Catapult, PfAction::Attack, rate),
            fragments,
            "Bombarder is a separate action and does not change PF Attack cost"
        );
        assert_eq!(
            Simulation::pf_action_move_cost(UnitKind::CruiseMissile, PfAction::Attack, rate),
            rate,
            "the selected Missile class can perform Suicide Attack"
        );
        assert_eq!(
            Simulation::pf_action_move_cost(UnitKind::Warrior, PfAction::None, rate),
            fragments
        );
    }
}
