//! Unit catalog accessors over the static Civ2Civ3 metadata.
pub(super) use super::unit_catalog_data::{UnitClass, UnitFlag, UnitRole};
use super::*;

impl UnitKind {
    pub(super) fn unit_class(self) -> UnitClass {
        self.metadata().class
    }
    pub(super) fn has_flag(self, flag: UnitFlag) -> bool {
        self.metadata().flags & (1u64 << flag as u32) != 0
    }
    pub(super) fn has_role(self, role: UnitRole) -> bool {
        self.metadata().roles & (1u64 << role as u32) != 0
    }
    pub(super) fn is_worker(self) -> bool {
        self.has_flag(UnitFlag::Workers)
    }
    pub(super) fn has_marine_attack(self) -> bool {
        self.has_flag(UnitFlag::Marines)
    }
    pub(super) fn is_unbribable(self) -> bool {
        self.has_flag(UnitFlag::Unbribable)
    }
    pub(super) fn is_city_buster(self) -> bool {
        self.has_flag(UnitFlag::CityBuster)
    }
    pub(super) fn has_no_build(self) -> bool {
        self.has_flag(UnitFlag::NoBuild)
    }
    pub(super) fn has_shield_to_gold(self) -> bool {
        self.has_flag(UnitFlag::ShieldToGold)
    }
    pub(super) fn has_no_zoc(self) -> bool {
        self.has_flag(UnitFlag::HasNoZOC)
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
    pub(super) fn has_terrain_defense(self) -> bool {
        self.unit_class().has_terrain_defense()
    }
    pub(super) fn can_helicopter_field_load(self) -> bool {
        self.metadata().helicopter_field_load
    }
    pub(super) fn can_build_fortress(self) -> bool {
        self.is_worker() || self.has_flag(UnitFlag::Fortress)
    }
    pub(super) fn required_government(self) -> Option<Government> {
        self.metadata().government
    }
    pub(super) fn base_upkeep(self) -> (i32, i32, i32) {
        self.metadata().upkeep
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
    fn existing_42_unit_discriminants_preserve_saved_games() {
        use UnitKind::*;
        let legacy = [
            Settlers,
            Worker,
            Migrants,
            Engineer,
            Diplomat,
            Spy,
            Explorer,
            Warrior,
            Phalanx,
            Archer,
            Legion,
            Horseman,
            Chariot,
            Catapult,
            Musketeers,
            Marines,
            Knights,
            Partisan,
            Cannon,
            Bomber,
            Fighter,
            StealthFighter,
            StealthBomber,
            Helicopter,
            Trireme,
            Caravel,
            Galleon,
            Transport,
            Carrier,
            Frigate,
            Ironclad,
            Destroyer,
            Cruiser,
            Battleship,
            Submarine,
            AegisCruiser,
            CruiseMissile,
            Caravan,
            Freight,
            Nuclear,
            BarbarianLeader,
            Paratroopers,
        ];
        for (index, kind) in legacy.into_iter().enumerate() {
            assert_eq!(
                bincode::serialize(&kind).unwrap(),
                (index as u32).to_le_bytes()
            );
            assert_eq!(
                bincode::deserialize::<UnitKind>(&(index as u32).to_le_bytes()).unwrap(),
                kind
            );
        }
        for kind in UnitKind::ALL_NATIVE {
            assert_eq!(
                bincode::deserialize::<UnitKind>(&bincode::serialize(&kind).unwrap()).unwrap(),
                kind
            );
        }
    }

    #[test]
    fn native_cache_predicate_and_direct_obsolescence_graph_match_rules() {
        let derived: Vec<_> = UnitKind::all()
            .filter(|kind| {
                let class = kind.unit_class();
                !kind.is_civilian()
                    && !kind.is_missile()
                    && kind.fuel_capacity() == 0
                    && kind.transport_capacity() < 8
                    && (class.is_ground()
                        || matches!(
                            class,
                            UnitClass::Air | UnitClass::Helicopter | UnitClass::Trireme
                        )
                        || class == UnitClass::Sea
                            && kind.stats().attack > 0
                            && !kind.has_flag(UnitFlag::OnlyNativeAttack))
            })
            .collect();
        assert_eq!(derived, UnitKind::SIMPLE_MILITARY);
        assert_eq!(UnitKind::Phalanx.obsolete_by(), &[UnitKind::Pikemen]);
        assert_eq!(UnitKind::Musketeers.obsolete_by(), &[UnitKind::Riflemen]);
        assert_eq!(UnitKind::Knights.obsolete_by(), &[UnitKind::Dragoons]);
        assert_eq!(UnitKind::Cannon.obsolete_by(), &[UnitKind::Artillery]);
        assert_eq!(UnitKind::Artillery.obsolete_by(), &[UnitKind::Howitzer]);
        assert_eq!(UnitKind::Frigate.obsolete_by(), &[UnitKind::Ironclad]);
        assert_eq!(UnitKind::Caravel.obsolete_by(), &[UnitKind::Galleon]);
    }

    #[test]
    fn class_and_flags_keep_marines_field_loading_upkeep_and_veterancy_distinct() {
        for kind in [
            UnitKind::MechInf,
            UnitKind::Armor,
            UnitKind::Artillery,
            UnitKind::Howitzer,
        ] {
            assert!(kind.is_big_land());
            assert!(!kind.has_terrain_defense());
            assert!(!UnitKind::Carrier.can_carry(kind));
            assert!(!UnitKind::Trireme.can_carry(kind));
            assert!(UnitKind::Transport.can_carry(kind));
        }
        for kind in [
            UnitKind::Legion,
            UnitKind::Musketeers,
            UnitKind::Riflemen,
            UnitKind::Marines,
        ] {
            assert!(kind.has_marine_attack());
        }
        assert!(UnitKind::AlpineTroops.stats().ignores_terrain);
        assert!(!UnitKind::AlpineTroops.has_flag(UnitFlag::IgZOC));
        assert!(UnitKind::Fanatics.can_helicopter_field_load());
        assert!(!UnitKind::Riflemen.can_helicopter_field_load());
        assert!(UnitKind::Leader.has_no_home());
        assert!(!UnitKind::Leader.has_no_zoc());
        assert_eq!(UnitKind::Leader.max_veteran_levels(), 1);
        assert_eq!(UnitKind::BarbarianLeader.max_veteran_levels(), 1);
        assert!(UnitKind::Storm.no_veteran());
        assert_eq!(UnitKind::Storm.max_veteran_levels(), 4);
        assert_eq!(UnitKind::Crusaders.base_upkeep(), (1, 1, 0));
        assert!(!UnitKind::Fanatics.has_shield_to_gold());
        assert_eq!(UnitKind::Awacs.base_upkeep(), (0, 1, 0));
        assert!(UnitKind::Explorer.has_role(UnitRole::Explorer));
        assert!(!UnitKind::Explorer.has_no_build());
        assert_eq!(
            UnitKind::Explorer
                .required_tech()
                .unwrap()
                .data()
                .unwrap()
                .name,
            "Seafaring"
        );
    }

    #[test]
    fn appended_civ3_units_keep_their_technology_and_resource_gates() {
        use UnitKind::*;
        assert_eq!(UnitKind::ALL_NATIVE.len(), 66);
        for (kind, name) in [
            (Longbowman, "Invention"),
            (Infantry, "Replaceable Parts"),
            (ModernArmor, "Synthetic Fibers"),
            (Privateer, "Magnetism"),
            (JetFighter, "Rocketry"),
            (NuclearSubmarine, "Fission"),
            (Icbm, "Satellites"),
            (TacticalNuke, "Space Flight"),
            (Nuclear, "Fission"),
        ] {
            assert_eq!(kind.required_tech().unwrap().name(), name, "{kind:?}");
        }
        assert!(Scout.required_tech().is_none());
        assert_eq!(Privateer.required_resources().len(), 2);
        assert_eq!(Infantry.required_resources(), &[Resource::Rubber]);
        assert_eq!(ModernArmor.required_resources().len(), 3);
        assert_eq!(NuclearSubmarine.required_resources(), &[Resource::Uranium]);
    }
}
