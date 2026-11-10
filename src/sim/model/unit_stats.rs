//! Combat, movement, and capability statistics associated with unit kinds.

use super::units::UnitKind;

#[derive(Clone, Copy)]
pub(crate) struct UnitStats {
    pub(crate) cost: i32,
    pub(crate) pop_cost: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
    pub(crate) hp: i32,
    pub(crate) firepower: i32,
    pub(crate) moves: i32,
    pub(crate) vision_sq: i32,
    pub(crate) ignores_terrain: bool,
    pub(crate) can_work: bool,
    pub(crate) can_found: bool,
}

impl UnitKind {
    pub(crate) fn hit_points_at_rank(self, rank: usize) -> i32 {
        let rule = self.conquests_rule();
        crate::sim::civ3_unit_rules::experience_rank(rank).base_hit_points
            + rule.hit_point_bonus
    }

    pub(crate) fn stats(self) -> UnitStats {
        let rule = self.conquests_rule();
        let metadata = rule.metadata();
        UnitStats {
            cost: rule.cost,
            pop_cost: rule.population_cost,
            attack: rule.attack,
            defense: rule.defense,
            hp: self.hit_points_at_rank(crate::sim::civ3_unit_rules::default_experience_rank()),
            // Conquests defines regular combat firepower as one hit point per
            // round; bombardment uses the separately imported rate-of-fire.
            firepower: 1,
            moves: rule.moves,
            // Vision radius is an engine default, not a PRTO field.
            vision_sq: 2,
            ignores_terrain: rule.has_flag_1(0, 4),
            can_work: metadata.flags
                & (1u64 << crate::sim::unit_catalog::UnitFlag::Workers as u32)
                != 0,
            can_found: rule.has_flag_3(8, 1),
        }
    }
}
