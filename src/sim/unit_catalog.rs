//! Unit catalog accessors over the static Civ2Civ3 metadata.
pub(super) use super::unit_catalog_data::{UnitAiStrategy, UnitClass, UnitFlag};
use super::*;

impl UnitKind {
    pub(super) fn unit_class(self) -> UnitClass {
        self.metadata().class
    }
    pub(super) fn has_flag(self, flag: UnitFlag) -> bool {
        self.metadata().flags & (1u64 << flag as u32) != 0
    }
    pub(super) fn has_ai_strategy(self, strategy: UnitAiStrategy) -> bool {
        self.metadata().ai_strategies & (1u32 << strategy as u8) != 0
    }
    pub(super) fn is_worker(self) -> bool {
        self.has_flag(UnitFlag::Workers)
    }
    pub(super) fn has_marine_attack(self) -> bool {
        self.has_flag(UnitFlag::Marines)
    }
    #[cfg(test)]
    pub(super) fn has_zone_of_control(self) -> bool {
        self.conquests_rule().zone_of_control != 0
    }
    #[cfg(test)]
    pub(super) fn can_build_fortress(self) -> bool {
        self.is_worker() || self.has_flag(UnitFlag::Fortress)
    }
    pub(super) fn has_no_build(self) -> bool {
        self.has_flag(UnitFlag::NoBuild)
    }
    pub(super) fn has_no_home(self) -> bool {
        self.has_flag(UnitFlag::NoHome)
    }
    pub(super) fn no_veteran(self) -> bool {
        self.has_flag(UnitFlag::NoVeteran)
    }
    pub(super) fn is_big_land(self) -> bool {
        self.unit_class() == UnitClass::BigLand
    }
    pub(super) fn required_government(self) -> Option<Government> {
        self.metadata().government
    }
    pub(super) fn max_veteran_levels(self) -> usize {
        self.metadata().veteran_levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_fixture_checks_source_stats_capabilities_and_independent_native_order() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.native_unit_catalog_playtest(), (true, true, true));
    }

    #[test]
    fn overlapping_units_read_combat_and_production_values_from_conquests_table() {
        let knight = UnitKind::Knight.stats();
        assert_eq!(
            (knight.cost, knight.attack, knight.defense, knight.moves),
            (70, 4, 3, 2)
        );
        let galley = UnitKind::Galley.stats();
        assert_eq!(
            (galley.cost, galley.attack, galley.defense, galley.moves),
            (30, 1, 1, 3)
        );
    }

    #[test]
    fn civ_availability_comes_from_conquests_and_wonder_units_are_not_buildable() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.players[0].name, "Romans");
        assert!(game.unit_available(0, UnitKind::Warrior));
        game.players[0].known.push(Tech::BRONZE_WORKING);
        assert!(game.unit_available(0, UnitKind::Spearman));
        assert!(!game.unit_available(0, UnitKind::Crusader));
        game.players[0].name = "Aztecs".to_owned();
        assert!(!game.unit_available(0, UnitKind::Warrior));
        assert!(game.unit_available(0, UnitKind::JaguarWarrior));
    }

    #[test]
    fn active_catalog_is_the_source_order_and_upgrades_use_conquests_links() {
        let rules = crate::sim::civ3_unit_rules::all();
        assert_eq!(UnitKind::ALL_NATIVE.len(), 92);
        assert_eq!(rules.len(), UnitKind::ALL_NATIVE.len());
        for (kind, rule) in UnitKind::ALL_NATIVE.iter().zip(rules) {
            assert_eq!(kind.conquests_id(), Some(rule.id));
            assert_eq!(kind.name(), rule.name);
            assert_eq!(kind.stats().cost, rule.cost);
            assert_eq!(kind.stats().attack, rule.attack);
            assert_eq!(kind.stats().defense, rule.defense);
        }
        assert_eq!(UnitKind::Warrior.obsolete_by(), Some(UnitKind::Swordsman));
        assert_eq!(
            UnitKind::from_conquests_id("tank").unwrap().obsolete_by(),
            Some(UnitKind::ModernArmor)
        );
        assert_eq!(UnitKind::Curragh.obsolete_by(), Some(UnitKind::Galley));
        assert_eq!(UnitKind::Scout.obsolete_by(), Some(UnitKind::Conquistador));
    }

    #[test]
    fn source_technology_and_resource_requirements_resolve_for_the_full_roster() {
        for kind in UnitKind::ALL_NATIVE {
            let rule = crate::sim::civ3_unit_rules::get(kind.conquests_id().unwrap()).unwrap();
            assert_eq!(kind.required_resources(), rule.resources(), "{kind:?}");
            assert_eq!(kind.required_tech(), rule.required_tech(), "{kind:?}");
        }
        let marine = UnitKind::from_conquests_id("marine").unwrap();
        let tank = UnitKind::from_conquests_id("tank").unwrap();
        assert_eq!(marine.required_tech().unwrap().name(), "Amphibious Warfare");
        assert_eq!(marine.required_resources(), vec![Resource::Rubber]);
        assert_eq!(
            tank.required_resources(),
            vec![Resource::Oil, Resource::Rubber]
        );
        assert!(marine.has_marine_attack());
        assert!(UnitKind::Settler.stats().can_found);
        assert!(UnitKind::Worker.stats().can_work);
        assert_eq!(UnitKind::Transport.transport_capacity(), 6);
        assert!(UnitKind::Fighter.is_aircraft());
        assert_eq!(UnitKind::CruiseMissile.bombard_rate(), 3);
    }

    #[test]
    fn appended_civ3_units_keep_their_technology_and_resource_gates() {
        use UnitKind::*;
        assert_eq!(UnitKind::ALL_NATIVE.len(), 92);
        for (kind, name) in [
            (Longbowman, "Invention"),
            (Infantry, "Replaceable Parts"),
            (ModernArmor, "Synthetic Fibers"),
            (Privateer, "Magnetism"),
            (JetFighter, "Rocketry"),
            (NuclearSubmarine, "Fission"),
            (ICBM, "Satellites"),
            (TacticalNuke, "Space Flight"),
        ] {
            assert_eq!(kind.required_tech().unwrap().name(), name, "{kind:?}");
        }
        assert!(Scout.required_tech().is_none());
        assert_eq!(Privateer.required_resources().len(), 2);
        assert_eq!(Infantry.required_resources(), vec![Resource::Rubber]);
        assert_eq!(ModernArmor.required_resources().len(), 3);
        assert_eq!(
            NuclearSubmarine.required_resources(),
            vec![Resource::Uranium]
        );
    }

    #[test]
    fn source_type_and_ability_flags_drive_campaign_unit_metadata() {
        assert_eq!(UnitKind::Spearman.unit_class(), UnitClass::SmallLand);
        assert!(UnitKind::Chariot.is_big_land());
        assert_eq!(UnitKind::Transport.unit_class(), UnitClass::Sea);
        assert_eq!(UnitKind::Fighter.unit_class(), UnitClass::Air);
        assert_eq!(UnitKind::Helicopter.unit_class(), UnitClass::Helicopter);
        assert_eq!(UnitKind::CruiseMissile.unit_class(), UnitClass::Missile);
        assert!(UnitKind::JaguarWarrior.has_flag(UnitFlag::Unique));
        assert!(UnitKind::JaguarWarrior.has_ai_strategy(UnitAiStrategy::Offense));
        assert!(UnitKind::Worker.is_worker());
        assert!(UnitKind::Worker.can_build_fortress());
        assert!(UnitKind::CruiseMissile.has_flag(UnitFlag::OneAttack));
    }

    #[test]
    fn ai_strategies_come_from_prto_profiles_and_merge_alternate_strategy_rows() {
        assert!(UnitKind::Warrior.has_ai_strategy(UnitAiStrategy::Offense));
        assert!(!UnitKind::Warrior.has_ai_strategy(UnitAiStrategy::Defense));
        assert!(UnitKind::Spearman.has_ai_strategy(UnitAiStrategy::Defense));
        assert!(!UnitKind::Spearman.has_ai_strategy(UnitAiStrategy::Offense));

        // Rifleman has separate source prototypes for its Offense and
        // Defense strategies; F-15 has separate Air Bombard/Air Defense rows.
        assert!(UnitKind::Rifleman.has_ai_strategy(UnitAiStrategy::Offense));
        assert!(UnitKind::Rifleman.has_ai_strategy(UnitAiStrategy::Defense));
        assert!(UnitKind::F15.has_ai_strategy(UnitAiStrategy::AirBombard));
        assert!(UnitKind::F15.has_ai_strategy(UnitAiStrategy::AirDefense));

        // Nuclear Submarine's alternate PRTO profile selects naval missile
        // transport while its primary profile is Naval Power.
        assert!(UnitKind::NuclearSubmarine.has_ai_strategy(UnitAiStrategy::NavalPower));
        assert!(UnitKind::NuclearSubmarine.has_ai_strategy(UnitAiStrategy::NavalMissileTransport));
    }

    // These are catalog contracts, not tests of advisor training/mission policy.
    // Keep literal expectations here rather than under misleading advisor names.
    #[test]
    fn land_production_candidates_keep_their_conquests_prerequisites_and_roles() {
        assert_eq!(
            UnitKind::Spearman.required_tech(),
            Some(Tech::BRONZE_WORKING)
        );
        assert_eq!(UnitKind::Spearman.stats().defense, 2);
        assert_eq!(
            UnitKind::Marine.required_tech(),
            Some(Tech::AMPHIBIOUS_WARFARE)
        );
        assert_eq!(
            UnitKind::Marine.required_resources(),
            vec![Resource::Rubber]
        );
        assert!(UnitKind::Marine.has_marine_attack());
        assert_eq!(UnitKind::Marine.stats().moves, 1);
        assert_eq!(UnitKind::Catapult.required_tech(), Some(Tech::MATHEMATICS));
        assert_eq!(UnitKind::Catapult.stats().attack, 0);
        assert!(UnitKind::Catapult.has_ai_strategy(UnitAiStrategy::Artillery));
    }

    #[test]
    fn air_and_sea_candidates_keep_their_conquests_prerequisites_and_combat_values() {
        assert_eq!(UnitKind::Fighter.required_tech(), Some(Tech::FLIGHT));
        assert_eq!(UnitKind::Fighter.required_resources(), vec![Resource::Oil]);
        assert_eq!(UnitKind::Fighter.stats().attack, 4);
        assert_eq!(UnitKind::Fighter.stats().defense, 2);
        assert_eq!(UnitKind::Fighter.hp_loss_percent(), 10);
        assert!(UnitKind::Fighter.has_ai_strategy(UnitAiStrategy::AirDefense));
        assert_eq!(UnitKind::Destroyer.required_tech(), Some(Tech::COMBUSTION));
        assert_eq!(
            UnitKind::Destroyer.required_resources(),
            vec![Resource::Oil]
        );
        assert_eq!(UnitKind::AegisCruiser.required_tech(), Some(Tech::ROBOTICS));
        assert_eq!(
            UnitKind::AegisCruiser.required_resources(),
            vec![Resource::Aluminum, Resource::Uranium]
        );
        assert!(UnitKind::AegisCruiser.is_naval());
        assert_eq!(UnitKind::AegisCruiser.stats().attack, 15);
        assert_eq!(UnitKind::AegisCruiser.stats().defense, 10);
        assert_eq!(UnitKind::Carrier.stats().attack, 1);
        assert!(UnitKind::Transport.is_naval());
        // Previously embedded in an unrelated advisor scenario aggregate.
        assert_eq!(
            UnitKind::TacticalNuke.required_tech(),
            Some(Tech::SPACE_FLIGHT)
        );
        assert_eq!(
            UnitKind::TacticalNuke.required_resources(),
            vec![Resource::Aluminum, Resource::Uranium]
        );
    }

    #[test]
    fn worker_and_missile_capabilities_do_not_imply_military_training_policy() {
        assert!(UnitKind::Worker.stats().can_work);
        assert!(UnitKind::TacticalNuke.no_veteran());
        assert!(UnitKind::CruiseMissile.no_veteran());
    }

    #[test]
    fn campaign_zone_of_control_comes_from_the_conquests_prto_field() {
        assert!(UnitKind::ModernArmor.has_zone_of_control());
        assert!(UnitKind::Cavalry.has_zone_of_control());
        assert!(!UnitKind::Warrior.has_zone_of_control());
        assert!(!UnitKind::Spearman.has_zone_of_control());
    }
}
