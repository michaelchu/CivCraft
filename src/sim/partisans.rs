//! Freeciv's default ruleset callback for partisans after conquest.
//!
//! The inherited `data/default/default.lua::_deflua_make_partisans_callback`
//! calls `city:inspire_partisans(loser)` from the `city_transferred` signal.
//! The city must already belong to the winner and have its post-conquest
//! population. The caller is responsible for invoking this only for the
//! `"conquest"` transfer reason and only if the city survived conquest.

use super::*;
use crate::rules::freeciv::{RuleParameter, RulesetData, effect_value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PartisanCitizenSettings {
    nationality: bool,
    partisans_pct: i32,
}

fn city_citizen_parameter<'a>(rules: &'a RulesetData, key: &str) -> Option<&'a RuleParameter> {
    let mut matching = rules.parameters.iter().filter(|parameter| {
        parameter.file.eq_ignore_ascii_case("cities.ruleset")
            && parameter.section.eq_ignore_ascii_case("citizen")
            && parameter.key.eq_ignore_ascii_case(key)
    });
    let parameter = matching.next();
    assert!(
        matching.next().is_none(),
        "selected cities.ruleset repeats [citizen].{key}"
    );
    parameter
}

fn selected_partisan_citizen_settings() -> PartisanCitizenSettings {
    let rules = RulesetData::campaign_default();
    // These are the pinned engine defaults from server/ruleset/ruleload.h and
    // ruleload.c. C2C3 overrides both values in cities.ruleset.
    let nationality = city_citizen_parameter(rules, "nationality")
        .map(|parameter| {
            parameter
                .boolean_value()
                .expect("selected [citizen].nationality is a boolean")
                .expect("selected [citizen].nationality is present")
        })
        .unwrap_or(false);
    let partisans_pct = city_citizen_parameter(rules, "partisans_pct")
        .map(|parameter| {
            parameter
                .integer_value()
                .expect("selected [citizen].partisans_pct is an integer")
                .expect("selected [citizen].partisans_pct is present")
        })
        .unwrap_or(0);
    let partisans_pct =
        i32::try_from(partisans_pct).expect("selected [citizen].partisans_pct fits an i32");
    assert!(
        (0..=100).contains(&partisans_pct),
        "selected [citizen].partisans_pct must be between 0 and 100"
    );
    PartisanCitizenSettings {
        nationality,
        partisans_pct,
    }
}

fn city_has_partisan_local_support(
    city: &City,
    original_owner: Option<usize>,
    inspirer: usize,
    settings: PartisanCitizenSettings,
) -> bool {
    if !settings.nationality || settings.partisans_pct == 0 {
        return original_owner == Some(inspirer);
    }

    let own_nationality = city
        .foreign_citizens
        .iter()
        .find(|&&(nation, _)| nation == inspirer)
        .map_or(0, |&(_, count)| count);
    city.size > 0
        && i64::from(own_nationality) * 100 / i64::from(city.size)
            >= i64::from(settings.partisans_pct)
}

fn selected_partisan_unit() -> UnitKind {
    static UNIT: std::sync::OnceLock<UnitKind> = std::sync::OnceLock::new();
    *UNIT.get_or_init(|| {
        // The selected `unit_economics.tsv` explicitly maps Civ III Guerilla
        // to C2C3 `unit_partisan`. Confirm that this is the one selected unit
        // carrying Freeciv's Partisan role before using the Civ III identity.
        let candidates = UnitKind::ALL_NATIVE
            .into_iter()
            .filter(|&kind| {
                matches!(super::action_rules::freeciv_unit_economics_record(kind),
                super::action_rules::UnitEconomicsRecordBinding::Mapped {
                    record,
                    kind: mapping,
                } if mapping == super::action_rules::UnitEconomicsBindingKind::RoleAlias
                    && record
                        .string_list_field("roles")
                        .expect("selected partisan role list is valid")
                        .is_some_and(|roles| {
                            roles
                                .iter()
                                .any(|role| role.eq_ignore_ascii_case("Partisan"))
                        }))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            candidates.len(),
            1,
            "selected ruleset must map one Civ III unit to its Partisan role"
        );
        candidates[0]
    })
}

impl Simulation {
    /// Evaluate and apply the inherited Freeciv post-conquest partisan
    /// callback for an already transferred city.
    ///
    /// Hook contract: call after the city owner and citizen nationalities have
    /// been transferred, after conquest population loss, and after civil-war
    /// resolution; pass the defeated owner. Call only for a surviving city
    /// whose transfer reason is `conquest`. Returns the number of units placed.
    pub(super) fn inspire_partisans(&mut self, index: usize, defender: usize) -> usize {
        if index >= self.cities.len() || defender >= self.players.len() {
            return 0;
        }
        let city = self.cities[index].clone();
        let settings = selected_partisan_citizen_settings();
        let original_owner = self.city_founders.get(&city.id).copied();
        if !city_has_partisan_local_support(&city, original_owner, defender, settings) {
            return 0;
        }

        // `api_methods_city_inspire_partisans()` evaluates this effect for the
        // defeated owner, not the new city owner. Its selected C2C3 effects
        // supply government, Communism, Gunpowder, world Guerilla Warfare, and
        // non-barbarian requirements through the shared evaluator.
        let rules = RulesetData::campaign_default();
        let effect_context = super::action_rules::SimulationRequirementContext::for_player_in_city(
            self, defender, &city,
        );
        let inspiration = effect_value(&rules.effects, "Inspire_Partisans", &effect_context)
            .expect("campaign C2C3 Inspire_Partisans requirements are supported");
        if inspiration <= 0 {
            return 0;
        }

        let partisan_unit = selected_partisan_unit();
        let partisans = (self.rand_below(2 + (city.size + 1) / 2) + 1).min(8);
        let center = self.map_wrap.index(city.x, city.y);
        let labels = self.continent_labels();
        let mut sites: Vec<_> = self
            .map_city_radius(&city)
            .filter(|&(_, _, x, y)| {
                self.pf_native_tile(partisan_unit, x, y)
                    && !self.cities.iter().any(|city| (city.x, city.y) == (x, y))
                    && !self.units.iter().any(|unit| (unit.x, unit.y) == (x, y))
            })
            .collect();
        sites.sort_by_key(|&(dx, dy, _, _)| city_radius_order_key(dx, dy));
        let mut placed = 0;
        for _ in 0..partisans {
            let mut best = None;
            for (site_index, &(_, _, x, y)) in sites.iter().enumerate() {
                let mut value = self.virtual_defense_power(defender, partisan_unit, x, y) * 10;
                if labels[self.map_wrap.index(x, y)] != labels[center] {
                    value /= 2;
                }
                value -= self.rand_below(value / 3);
                if best.is_none_or(|(_, previous)| value > previous) {
                    best = Some((site_index, value));
                }
            }
            let Some((site_index, _)) = best else {
                break;
            };
            let (_, _, x, y) = sites.remove(site_index);
            self.spawn_unit(defender, partisan_unit, x, y);
            self.units.last_mut().unwrap().state = UnitState::Fortified;
            placed += 1;
        }
        if defender == HUMAN || city.owner == HUMAN {
            self.add_event(format!("The loss of {} has inspired partisans.", city.name));
        }
        placed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captured_city_fixture() -> (Simulation, usize) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.base = None;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
        }
        let (x, y) = (20, 24);
        game.create_city(HUMAN, x, y);
        let city = game.cities.len() - 1;
        game.cities[city].size = 4;
        game.cities[city].foreign_citizens = vec![(1, 3)];
        game.city_founders.insert(game.cities[city].id, 1);
        (game, city)
    }

    #[test]
    fn partisan_eligibility_uses_selected_nationality_threshold_or_original_owner() {
        let rules = selected_partisan_citizen_settings();
        assert_eq!(
            rules,
            PartisanCitizenSettings {
                nationality: true,
                partisans_pct: 75
            }
        );
        let mut city = City::new(7, HUMAN, 10, 10, "Test City".to_owned(), 0);
        city.size = 4;
        city.foreign_citizens = vec![(1, 2)];
        assert!(!city_has_partisan_local_support(&city, Some(1), 1, rules));
        city.foreign_citizens[0].1 = 3;
        assert!(city_has_partisan_local_support(&city, Some(2), 1, rules));

        let no_nationality = PartisanCitizenSettings {
            nationality: false,
            partisans_pct: 75,
        };
        assert!(city_has_partisan_local_support(
            &city,
            Some(1),
            1,
            no_nationality
        ));
        assert!(!city_has_partisan_local_support(
            &city,
            Some(2),
            1,
            no_nationality
        ));
        let zero_threshold = PartisanCitizenSettings {
            nationality: true,
            partisans_pct: 0,
        };
        assert!(city_has_partisan_local_support(
            &city,
            Some(1),
            1,
            zero_threshold
        ));
        assert!(!city_has_partisan_local_support(
            &city,
            Some(2),
            1,
            zero_threshold
        ));
    }

    #[test]
    fn selected_partisan_role_uses_the_explicit_civ3_guerilla_alias() {
        let kind = selected_partisan_unit();
        assert_eq!(kind, UnitKind::Guerilla);
        let super::super::action_rules::UnitEconomicsRecordBinding::Mapped {
            record,
            kind: super::super::action_rules::UnitEconomicsBindingKind::RoleAlias,
        } = super::super::action_rules::freeciv_unit_economics_record(kind)
        else {
            panic!("C2C3 Partisan must use an explicit Civ III role alias");
        };
        assert_eq!(record.section, "unit_partisan");
    }

    #[test]
    fn partisan_site_score_uses_virtual_defense_without_an_attacker() {
        let (mut game, _) = captured_city_fixture();
        let (x, y) = (21, 24);
        let kind = selected_partisan_unit();
        let base_score = game.virtual_defense_power(1, kind, x, y);

        // Freeciv passes a null attacker to get_virtual_defense_power. The
        // shared resolver must therefore omit attacker-dependent Defend_Bonus
        // effects such as C2C3's Airbase bonus while scoring a spawn tile.
        let tile = game.map_wrap.index(x, y);
        game.tiles[tile].base = Some(Base::Airbase);
        let virtual_score = game.virtual_defense_power(1, kind, x, y);
        let attacker = game.make_unit(0, 1, kind, x, y);
        let combat_score = game.defense_power_against(&attacker, x, y, kind);

        assert_eq!(virtual_score, base_score);
        assert!(combat_score > virtual_score);
    }

    #[test]
    fn c2c3_partisan_callback_spawns_only_when_selected_effects_match() {
        let (mut game, city) = captured_city_fixture();
        game.players[1].government = Government::Communism;
        game.players[1]
            .known
            .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
        assert_eq!(game.inspire_partisans(city, 1), 0);

        game.players[HUMAN].known.push(Tech::NATIONALISM);
        let placed = game.inspire_partisans(city, 1);
        assert!(placed > 0);
        assert!(placed <= 8);
        assert!(game.units.iter().all(|unit| {
            unit.owner == 1
                && unit.kind == selected_partisan_unit()
                && unit.state == UnitState::Fortified
        }));
    }

    #[test]
    fn partisan_callback_uses_effect_value_for_the_loser_and_rejects_barbarians() {
        let (mut game, city) = captured_city_fixture();
        let rules = RulesetData::campaign_default();
        let city_record = game.cities[city].clone();
        let context = super::super::action_rules::SimulationRequirementContext::for_player_in_city(
            &game,
            1,
            &city_record,
        );
        assert_eq!(
            effect_value(&rules.effects, "Inspire_Partisans", &context).unwrap(),
            0
        );
        game.players[1].government = Government::Democracy;
        game.players[1]
            .known
            .extend([Tech::COMMUNISM, Tech::GUNPOWDER]);
        game.players[HUMAN].known.push(Tech::NATIONALISM);
        let city_record = game.cities[city].clone();
        let context = super::super::action_rules::SimulationRequirementContext::for_player_in_city(
            &game,
            1,
            &city_record,
        );
        assert_eq!(
            effect_value(&rules.effects, "Inspire_Partisans", &context).unwrap(),
            1
        );
        assert_eq!(game.inspire_partisans(city, BARBARIANS), 0);
    }
}
