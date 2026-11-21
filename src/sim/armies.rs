//! Owner-authorized Army combat: members share a battle and a persistent HP pool.
//! Member identities and HP remain saved as ordinary cargo; max HP is derived.
use super::*;

impl Simulation {
    pub(crate) fn unit_transport_capacity(&self, carrier: &Unit) -> usize {
        carrier.kind.transport_capacity()
            + usize::from(
                carrier.kind == UnitKind::Army
                    && self.has_active_wonder(carrier.owner, Building::PENTAGON),
            )
    }

    pub(in crate::sim) fn is_army_member(&self, unit: &Unit) -> bool {
        unit.transport
            .and_then(|id| self.unit_by_id(id))
            .is_some_and(|carrier| carrier.kind == UnitKind::Army)
    }

    pub(in crate::sim) fn rebuild_armies(&mut self) {
        let ids: Vec<_> = self
            .units
            .iter()
            .filter(|u| u.kind == UnitKind::Army)
            .map(|u| u.id)
            .collect();
        for id in ids {
            self.rebuild_army(id);
        }
    }

    pub(in crate::sim) fn rebuild_army(&mut self, id: u32) {
        let members: Vec<_> = self
            .units
            .iter()
            .filter(|u| u.transport == Some(id))
            .collect();
        let max_hp: i32 = members.iter().map(|u| u.max_hit_points()).sum();
        let hp: i32 = members.iter().map(|u| u.hp).sum();
        if let Some(army) = self
            .units
            .iter_mut()
            .find(|u| u.id == id && u.kind == UnitKind::Army)
        {
            if max_hp == 0 && army.army_max_hp > 0 {
                army.hp = army.kind.hit_points_at_rank(army.veteran);
            }
            army.army_max_hp = max_hp;
            if max_hp > 0 {
                army.hp = hp;
            }
        }
    }

    /// Strongest healthy member fights first; ties are stable across save/load.
    pub(in crate::sim) fn army_combat_member(&self, army: &Unit, attacking: bool) -> Option<Unit> {
        if army.kind != UnitKind::Army {
            return None;
        }
        let mut member = self
            .units
            .iter()
            .filter(|u| u.transport == Some(army.id) && u.hp > 0)
            .max_by_key(|u| {
                (
                    if attacking {
                        u.kind.attack_strength()
                    } else {
                        u.kind.stats().defense
                    },
                    u.hp,
                    std::cmp::Reverse(u.id),
                )
            })?
            .clone();
        member.transport = None;
        member.moves_left = army.moves_left;
        member.state = army.state;
        member.moved = army.moved;
        Some(member)
    }

    pub(in crate::sim) fn unit_attack_strength(&self, unit: &Unit) -> i32 {
        if unit.kind == UnitKind::Army {
            self.army_combat_member(unit, true)
                .map_or(0, |member| member.kind.attack_strength())
        } else {
            unit.kind.attack_strength()
        }
    }

    pub(in crate::sim) fn army_move_rate(&self, army: &Unit) -> i32 {
        // Conquests readme: one extra move above the slowest loaded member.
        let slowest = self
            .units
            .iter()
            .filter(|u| u.transport == Some(army.id))
            .map(|u| u.kind.stats().moves)
            .min();
        slowest.map_or(army.kind.stats().moves, |moves| moves + 1)
            * movement_rules::campaign_move_fragments()
    }

    /// Apply each round to its active member. Exhausted members stay in the
    /// formation and recover with it; defeat destroys the whole Army.
    pub(in crate::sim) fn damage_army_member(
        &mut self,
        army: &Unit,
        damage: i32,
        attacking: bool,
    ) -> i32 {
        let mut remaining = damage;
        while remaining > 0 {
            let Some(member) = self.army_combat_member(army, attacking) else {
                break;
            };
            let loss = remaining.min(member.hp);
            self.units
                .iter_mut()
                .find(|u| u.id == member.id)
                .unwrap()
                .hp -= loss;
            remaining -= loss;
        }
        self.units
            .iter()
            .filter(|u| u.transport == Some(army.id))
            .map(|u| u.hp.max(0))
            .sum()
    }

    /// Bombardment writes the carrier HP through the ordinary damage path.
    /// Mirror that loss into the member pool before any later combat or save.
    pub(in crate::sim) fn sync_army_damage(&mut self, id: u32) {
        let Some(army) = self
            .unit_by_id(id)
            .filter(|u| u.kind == UnitKind::Army)
            .cloned()
        else {
            return;
        };
        let sum: i32 = self
            .units
            .iter()
            .filter(|u| u.transport == Some(id))
            .map(|u| u.hp)
            .sum();
        if sum > army.hp {
            self.damage_army_member(&army, sum - army.hp, false);
        }
    }

    pub(in crate::sim) fn heal_army_members(&mut self, id: u32, mut gain: i32) {
        let mut ids: Vec<_> = self
            .units
            .iter()
            .filter(|u| u.transport == Some(id))
            .map(|u| u.id)
            .collect();
        ids.sort_unstable();
        for member in ids {
            let unit = self.units.iter_mut().find(|u| u.id == member).unwrap();
            let healed = gain.min((unit.max_hit_points() - unit.hp).max(0));
            unit.hp += healed;
            gain -= healed;
        }
        self.rebuild_army(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Simulation {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(true);
        }
        game.sign_pact(HUMAN, 1, Pact::War);
        game.create_city(HUMAN, 10, 20);
        game.create_city(1, 24, 20);
        game
    }

    fn loaded_army(game: &mut Simulation, owner: usize, kinds: &[UnitKind]) -> (u32, Vec<u32>) {
        let army = game.spawn_unit(owner, UnitKind::Army, 10, 20);
        let members = kinds
            .iter()
            .map(|&kind| {
                let member = game.spawn_unit(owner, kind, 10, 20);
                assert!(game.board_specific_ship(member, army));
                member
            })
            .collect();
        (army, members)
    }

    #[test]
    fn army_loading_uses_owner_pentagon_and_preserves_four_members_after_bonus_loss() {
        let mut game = world();
        let (army, members) = loaded_army(&mut game, HUMAN, &[UnitKind::Warrior; 3]);
        let fourth = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        assert!(!game.board_specific_ship(fourth, army));
        game.cities
            .iter_mut()
            .find(|c| c.owner == 1)
            .unwrap()
            .buildings
            .push(Building::PENTAGON);
        assert_eq!(
            game.unit_transport_capacity(game.unit_by_id(army).unwrap()),
            3
        );
        game.cities
            .iter_mut()
            .find(|c| c.owner == HUMAN)
            .unwrap()
            .buildings
            .push(Building::PENTAGON);
        assert!(game.apply(Command::BoardOrDeboard(fourth)));
        assert_eq!(game.cargo_count(army), 4);
        let max_hp: i32 = game
            .units
            .iter()
            .filter(|u| u.transport == Some(army))
            .map(Unit::max_hit_points)
            .sum();
        assert_eq!(game.unit_by_id(army).unwrap().max_hit_points(), max_hp);
        assert!(!game.apply(Command::BoardOrDeboard(members[0])));
        assert!(!game.apply(Command::UnloadAll(army)));
        game.cities
            .iter_mut()
            .find(|c| c.owner == HUMAN)
            .unwrap()
            .buildings
            .retain(|&b| b != Building::PENTAGON);
        assert_eq!(
            game.unit_transport_capacity(game.unit_by_id(army).unwrap()),
            3
        );
        assert_eq!(game.cargo_count(army), 4);
        let fifth = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        assert!(!game.board_specific_ship(fifth, army));
    }

    #[test]
    fn army_members_are_owned_combat_troops_and_cannot_act_or_defend_separately() {
        let mut game = world();
        let (army, members) = loaded_army(&mut game, HUMAN, &[UnitKind::Warrior]);
        for kind in [
            UnitKind::Worker,
            UnitKind::Leader,
            UnitKind::Catapult,
            UnitKind::Army,
            UnitKind::TacticalNuke,
        ] {
            let id = game.spawn_unit(HUMAN, kind, 10, 20);
            assert!(
                !game.can_board_ship(game.unit_by_id(id).unwrap(), game.unit_by_id(army).unwrap()),
                "{kind:?}"
            );
        }
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        let allied = game.spawn_unit(1, UnitKind::Warrior, 10, 20);
        assert!(!game.can_board_ship(
            game.unit_by_id(allied).unwrap(),
            game.unit_by_id(army).unwrap()
        ));
        assert!(!game.combat_unit_can_defend_here(game.unit_by_id(members[0]).unwrap()));
        assert!(!game.apply(Command::MoveCiv3(members[0], 2)));
        assert_eq!(
            game.unit_move_rate(game.unit_by_id(army).unwrap()),
            2 * MOVE_FRAGMENTS
        );
    }

    #[test]
    fn army_damage_switches_members_and_heals_the_pool_without_resurrecting_an_independent_unit() {
        let mut game = world();
        let (army, members) =
            loaded_army(&mut game, HUMAN, &[UnitKind::Swordsman, UnitKind::Warrior]);
        let before = game.unit_by_id(army).unwrap().clone();
        let sword_hp = game.unit_by_id(members[0]).unwrap().hp;
        let remaining = game.damage_army_member(&before, sword_hp, true);
        game.units.iter_mut().find(|u| u.id == army).unwrap().hp = remaining;
        assert_eq!(
            game.army_combat_member(&before, true).unwrap().id,
            members[1]
        );
        assert_eq!(game.unit_by_id(members[0]).unwrap().hp, 0);
        game.restore_player_units(HUMAN);
        assert!(game.unit_by_id(members[0]).unwrap().hp > 0);
        let live = game.unit_by_id(army).unwrap();
        assert_eq!(
            live.hp,
            members
                .iter()
                .map(|&id| game.unit_by_id(id).unwrap().hp)
                .sum::<i32>()
        );
        assert!(live.hp > remaining && live.hp <= live.max_hit_points());
    }

    #[test]
    fn army_fights_through_move_command_and_carries_members_into_captured_city() {
        let mut game = world();
        let (army, members) = loaded_army(&mut game, HUMAN, &[UnitKind::ModernArmor; 3]);
        let target = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        game.create_city(1, target.0, target.1);
        let city = game.city_at(target.0, target.1).unwrap().id;
        game.cities.iter_mut().find(|c| c.id == city).unwrap().size = 3;
        let defender = game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        assert!(game.apply(Command::MoveCiv3(army, 2)));
        assert!(game.unit_by_id(defender).is_none());
        assert_eq!(game.city_by_id(city).unwrap().owner, HUMAN);
        let unit = game.unit_by_id(army).unwrap();
        assert_eq!((unit.x, unit.y), target);
        assert_eq!(
            unit.hp,
            members
                .iter()
                .map(|&id| game.unit_by_id(id).unwrap().hp)
                .sum::<i32>()
        );
        for id in members {
            let member = game.unit_by_id(id).unwrap();
            assert_eq!((member.x, member.y), target);
        }
    }

    #[test]
    fn defeated_army_loses_all_members_in_a_protected_city() {
        let mut game = world();
        let (army, members) = loaded_army(&mut game, HUMAN, &[UnitKind::Warrior; 3]);
        let target = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        let enemy = game.spawn_unit(1, UnitKind::ModernArmor, target.0, target.1);
        assert_eq!(
            game.best_defender_id(game.unit_by_id(enemy).unwrap(), 10, 20),
            Some(army)
        );
        game.resolve_combat(enemy, army, 10, 20, None);
        assert!(game.unit_by_id(army).is_none());
        assert!(members.iter().all(|&id| game.unit_by_id(id).is_none()));
        assert!(game.unit_by_id(enemy).is_some());
    }

    #[test]
    fn damaged_army_members_and_derived_health_round_trip_through_campaign_save() {
        let mut game = world();
        let (army, members) =
            loaded_army(&mut game, HUMAN, &[UnitKind::Swordsman, UnitKind::Warrior]);
        let snapshot = game.unit_by_id(army).unwrap().clone();
        let hp = game.damage_army_member(&snapshot, game.unit_by_id(members[0]).unwrap().hp, true);
        game.units.iter_mut().find(|u| u.id == army).unwrap().hp = hp;
        let path =
            std::env::temp_dir().join(format!("civcraft-army-combat-{}.save", std::process::id()));
        game.save_campaign(&path).unwrap();
        let loaded = Simulation::load_campaign(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(loaded.unit_by_id(army).unwrap().hp, hp);
        assert_eq!(
            loaded.unit_by_id(army).unwrap().max_hit_points(),
            snapshot.max_hit_points()
        );
        assert_eq!(loaded.unit_by_id(members[0]).unwrap().hp, 0);
        assert_eq!(
            loaded
                .army_combat_member(loaded.unit_by_id(army).unwrap(), true)
                .unwrap()
                .id,
            members[1]
        );
    }

    #[test]
    fn bombardment_damages_the_army_pool_once_and_keeps_members_in_sync() {
        let mut game = world();
        let (army, members) = loaded_army(&mut game, 1, &[UnitKind::Warrior; 3]);
        let at = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        let gun = game.spawn_unit(HUMAN, UnitKind::Artillery, at.0, at.1);
        game.refresh_visibility();
        let before = game.unit_by_id(army).unwrap().hp;
        assert!(game.apply(Command::Bombard(gun, 10, 20)));
        let after = game.unit_by_id(army).unwrap().hp;
        assert!(after < before && after > 0);
        assert_eq!(
            after,
            members
                .iter()
                .map(|&id| game.unit_by_id(id).unwrap().hp)
                .sum::<i32>()
        );
    }

    #[test]
    fn empty_armies_cannot_attack_or_conquer_and_loaded_armies_can_ride_ships() {
        let mut game = world();
        let army = game.spawn_unit(HUMAN, UnitKind::Army, 10, 20);
        let target = game.map_wrap.civ3_neighbor(10, 20, 2).unwrap();
        game.create_city(1, target.0, target.1);
        assert!(!game.action_available_for_unit_fail_closed(
            game.unit_by_id(army).unwrap(),
            "Conquer City",
            target
        ));
        assert!(!game.action_available_for_unit_fail_closed(
            game.unit_by_id(army).unwrap(),
            "Attack",
            target
        ));
        let member = game.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        assert!(game.board_specific_ship(member, army));
        let ship = game.spawn_unit(HUMAN, UnitKind::Transport, 10, 20);
        assert!(game.board_specific_ship(army, ship));
        game.move_cargo(ship, 12, 20);
        assert_eq!(
            (
                game.unit_by_id(army).unwrap().x,
                game.unit_by_id(army).unwrap().y
            ),
            (12, 20)
        );
        assert_eq!(
            (
                game.unit_by_id(member).unwrap().x,
                game.unit_by_id(member).unwrap().y
            ),
            (12, 20)
        );
    }
}
