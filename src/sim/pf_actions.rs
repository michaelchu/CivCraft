//! Shared default PF action policy from common/aicore/pf_tools.c. Planning
//! capabilities deliberately differ from legal action execution.
use super::*;

impl UnitKind {
    // IgZOC and absence of UCF_ZOC are separate native exemptions.
    pub(super) fn ignores_zoc(self) -> bool {
        (self.is_naval() && self != Self::Trireme)
            || self.is_aircraft()
            || matches!(
                self,
                Self::Diplomat
                    | Self::Spy
                    | Self::Explorer
                    | Self::Partisan
                    | Self::BarbarianLeader
                    | Self::Leader
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PfAction {
    None,
    Attack,
    Diplomat,
    Trade,
    Impossible,
}

impl Simulation {
    // tile_move_cost_ptrs: cost only; scopes/action callbacks decide legality.
    pub(super) fn raw_move_cost_for(&self, unit: &Unit, x: i32, y: i32) -> i32 {
        let kind = unit.kind;
        if !kind.unit_class().terrain_speed() {
            return MOVE_FRAGMENTS;
        }
        let ignores = kind.stats().ignores_terrain;
        if !self.pf_native_tile(kind, unit.x, unit.y) || !self.pf_native_tile(kind, x, y) {
            return if ignores { 2 } else { MOVE_FRAGMENTS };
        }
        let source = self.tile(unit.x, unit.y);
        let dest = self.tile(x, y);
        let restricted = [source.owner, dest.owner]
            .into_iter()
            .flatten()
            .any(|owner| self.at_war(unit.owner, owner));
        let mut cost = dest.terrain.movement_cost() * MOVE_FRAGMENTS;
        if !restricted {
            if source.railroad && dest.railroad {
                cost = cost.min(1);
            }
            if source.road && dest.road || source.river && dest.river && !kind.is_big_land() {
                cost = cost.min(2);
            }
        }
        if ignores { cost.min(2) } else { cost }
    }
    pub(super) fn pf_native_tile(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        let tile = self.tile(x, y);
        kind.is_native_on(
            tile.terrain,
            tile.river,
            tile.road,
            tile.railroad,
            tile.base,
        )
    }

    pub(super) fn pf_main_sight(&self, observer: usize, x: i32, y: i32) -> bool {
        self.units
            .iter()
            .map(|unit| {
                let extra = match self.tile(unit.x, unit.y).base {
                    Some(Base::Fortress) if self.has_tech(unit.owner, Tech::ASTRONOMY) => 8,
                    Some(Base::Airbase) => 8,
                    _ => 0,
                };
                (
                    unit.owner,
                    unit.x,
                    unit.y,
                    unit.kind.stats().vision_sq + extra,
                )
            })
            .chain(
                self.cities
                    .iter()
                    .map(|city| (city.owner, city.x, city.y, 8)),
            )
            .any(|(owner, sx, sy, radius)| {
                self.vision_recipients(owner).contains(&observer)
                    && self.map_distance(sx, sy, x, y).pow(2) <= radius
            })
    }

    fn pf_attack_possible(&self, kind: UnitKind, owner: usize, x: i32, y: i32, seen: bool) -> bool {
        // can_attack_non_native checks actual action capabilities as well
        // as Sea's flag. Carrier attack0 does NOT permit shore attacks,
        // even though the PF default flags can attack native sea targets.
        let shore = kind.is_naval()
            && !matches!(kind, UnitKind::Trireme | UnitKind::Submarine)
            && kind.stats().attack > 0;
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
        let trade = matches!(kind, UnitKind::Caravan | UnitKind::Freight);
        // aia_utype_is_considered_spy includes Establish Embassy, so the
        // Explorer has these PF flags despite lacking unit-bribery actions.
        let diplomat = kind.is_diplomatic() || kind == UnitKind::Explorer;
        if city {
            if trade {
                return PfAction::Trade;
            }
            if diplomat {
                return PfAction::Diplomat;
            }
        }
        let units = self
            .units
            .iter()
            .any(|unit| (unit.x, unit.y) == (x, y) && !self.allied(owner, unit.owner));
        if units {
            if diplomat {
                return PfAction::Diplomat;
            }
            if !kind.is_civilian() {
                return if self.pf_attack_possible(kind, owner, x, y, seen) {
                    PfAction::Attack
                } else {
                    PfAction::Impossible
                };
            }
        }
        if city && city_attack && !kind.is_civilian() {
            return if kind.can_occupy_city() || self.pf_attack_possible(kind, owner, x, y, false) {
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
                || (actor.kind.is_naval() && actor.kind.stats().attack > 0)
        } else {
            // The supported diplomatic/merchant types can disembark and act
            // from transport scope; native/city scope also permits actions.
            self.pf_native_tile(actor.kind, actor.x, actor.y)
                || self.cities.iter().any(|city| {
                    (city.x, city.y) == (actor.x, actor.y)
                        && (self.allied(actor.owner, city.owner) || actor.kind.can_occupy_city())
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
                || kind == UnitKind::Trireme)
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
        if enemy.kind.stats().attack <= 0 || !self.pf_action_step_possible(enemy, PfAction::Attack)
        {
            return false;
        }
        let shore =
            enemy.kind.is_naval() && !matches!(enemy.kind, UnitKind::Submarine | UnitKind::Trireme);
        if !shore && !self.pf_native_tile(enemy.kind, x, y) {
            return false;
        }
        !target.is_aircraft()
            || enemy.kind.can_attack_aircraft()
            || self.cities.iter().any(|city| (city.x, city.y) == (x, y))
            || matches!(self.tile(x, y).base, Some(Base::Airstrip | Base::Airbase))
    }

    pub(super) fn pf_action_move_cost(kind: UnitKind, action: PfAction, rate: i32) -> i32 {
        if action == PfAction::Attack && (kind.bombard_rate() > 0 || kind.is_missile()) {
            rate
        } else {
            MOVE_FRAGMENTS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_class_zoc_and_big_land_native_extras() {
        for kind in [
            UnitKind::Caravan,
            UnitKind::Freight,
            UnitKind::Migrants,
            UnitKind::Chariot,
            UnitKind::Catapult,
            UnitKind::Cannon,
            UnitKind::Trireme,
        ] {
            assert!(!kind.ignores_zoc(), "{kind:?}");
        }
        assert!(UnitKind::Explorer.ignores_zoc());
        assert!(UnitKind::Carrier.ignores_zoc());
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        for terrain in [Terrain::Jungle, Terrain::Mountains, Terrain::Swamp] {
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
    fn pf_flags_use_nonmil_and_supported_diplomatic_trade_actions() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        sim.tile_mut(10, 20).terrain = Terrain::Grassland;
        sim.spawn_unit(1, UnitKind::Warrior, 10, 20);
        for kind in [
            UnitKind::Settlers,
            UnitKind::BarbarianLeader,
            UnitKind::Caravan,
        ] {
            assert_eq!(
                sim.pf_default_action(kind, HUMAN, (10, 20), true, true, true),
                PfAction::None
            );
        }
        for kind in [UnitKind::Explorer, UnitKind::Diplomat, UnitKind::Spy] {
            assert_eq!(
                sim.pf_default_action(kind, HUMAN, (10, 20), true, true, true),
                PfAction::Diplomat
            );
        }
        assert_eq!(
            sim.pf_default_action(UnitKind::Carrier, HUMAN, (10, 20), true, true, true),
            PfAction::Impossible
        );
        sim.tile_mut(10, 20).terrain = Terrain::Ocean;
        assert_eq!(
            sim.pf_default_action(UnitKind::Carrier, HUMAN, (10, 20), true, true, true),
            PfAction::Attack
        );
        sim.tile_mut(10, 20).terrain = Terrain::Grassland;
        sim.create_city(1, 10, 20);
        assert_eq!(
            sim.pf_default_action(UnitKind::Caravan, HUMAN, (10, 20), true, true, true),
            PfAction::Trade
        );
    }
}
