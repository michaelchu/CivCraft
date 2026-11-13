//! Random and rules-driven disasters affecting cities and terrain.

#[cfg(test)]
use super::terrain_work::neighbors;
use super::*;
use crate::rules::Genus;
use crate::rules::freeciv::{ContentKind, RuleRecord, RulesetData, requirements_met};

impl Simulation {
    pub(super) fn check_disasters(&mut self) {
        let rules = RulesetData::campaign_default();
        let frequency = rules
            .integer_setting_value("disasters")
            .unwrap_or_else(|error| panic!("invalid C2C3 disasters setting: {error}"));
        assert!(
            frequency >= 0,
            "C2C3 disasters setting must not be negative"
        );
        if frequency == 0 {
            return;
        }
        let disasters: Vec<_> = rules
            .rule_records
            .iter()
            .filter(|record| {
                record.file.eq_ignore_ascii_case("game.ruleset")
                    && record.namespace.eq_ignore_ascii_case("disaster")
            })
            .collect();
        // Freeciv iterates players and each player's live city list, not the
        // global city vector. Preserve that order and re-resolve ids so a
        // disaster that removes a city cannot invalidate later indices.
        let city_ids: Vec<_> = (0..self.players.len())
            .flat_map(|owner| self.player_cities_ordered(owner).map(|city| city.id))
            .collect();
        for city_id in city_ids {
            for disaster in &disasters {
                let Some(index) = self.city_index_by_id(city_id) else {
                    break;
                };
                let disaster_frequency = disaster
                    .integer_field("frequency")
                    .unwrap_or_else(|error| {
                        panic!("invalid {}.frequency: {error}", disaster.section)
                    })
                    .unwrap_or_else(|| {
                        panic!("{} is missing its required frequency", disaster.section)
                    });
                assert!(
                    disaster_frequency >= 0,
                    "{} has a negative disaster frequency",
                    disaster.section
                );
                let probability = i64::from(frequency) * disaster_frequency;
                // Freeciv consumes the frequency roll before checking the
                // disaster's requirements, even when the city cannot qualify.
                if i64::from(self.rand_below(1_000_000)) < probability
                    && self.disaster_possible(index, disaster)
                {
                    self.apply_disaster(index, disaster);
                }
            }
        }
    }

    fn disaster_possible(&self, index: usize, disaster: &RuleRecord) -> bool {
        let city = &self.cities[index];
        let context = super::action_rules::SimulationRequirementContext::for_city_at(
            self,
            city,
            Some((city.x, city.y)),
        );
        requirements_met(&disaster.requirements, &context).unwrap_or_else(|error| {
            panic!(
                "could not evaluate {} disaster requirements: {error:?}",
                disaster.section
            )
        })
    }

    fn apply_disaster(&mut self, index: usize, disaster: &RuleRecord) {
        let city = self.cities[index].clone();
        let effects = disaster
            .string_list_field("effects")
            .unwrap_or_else(|error| panic!("invalid {}.effects: {error}", disaster.section))
            .unwrap_or_default();
        for effect in &effects {
            assert!(
                [
                    "Robbery",
                    "Pollution",
                    "Fallout",
                    "ReducePopulation",
                    "ReducePopDestroy",
                    "DestroyBuilding",
                    "EmptyFoodStock",
                    "EmptyProdStock",
                ]
                .iter()
                .any(|supported| effect.eq_ignore_ascii_case(supported)),
                "unsupported {} disaster effect `{effect}`",
                disaster.section
            );
        }
        let has_effect = |name: &str| {
            effects
                .iter()
                .any(|effect| effect.eq_ignore_ascii_case(name))
        };
        let mut details = Vec::new();

        // Keep Freeciv cityturn.c::apply_disaster's effect order. The selected
        // ruleset supplies the effect set; this adapter implements each native
        // effect against the preserved Civ III city and tile state.
        if has_effect("Robbery") {
            let gold = self.players[city.owner].gold;
            let trade = self.city_forecast(&city).trade;
            if gold > 0 && trade > 0 {
                let loss = gold.min(trade * 5);
                self.players[city.owner].gold -= loss;
                details.push(format!("{loss} gold stolen."));
            }
        }
        for (effect, contamination) in [
            ("Pollution", Contamination::Pollution),
            ("Fallout", Contamination::Fallout),
        ] {
            if has_effect(effect) && self.place_contamination(&city, contamination) {
                details.push(format!("{} appeared nearby.", contamination.name()));
            }
        }

        if has_effect("ReducePopulation") || has_effect("ReducePopDestroy") {
            if city.size > 1 {
                self.reduce_city_population(index);
                self.reconcile_city_workers();
                details.push("One citizen lost.".to_owned());
            } else if has_effect("ReducePopDestroy") {
                self.remove_city_for_population_loss(city.id);
                details.push("City destroyed.".to_owned());
            }
        }

        if let Some(index) = self.city_index_by_id(city.id) {
            if has_effect("DestroyBuilding") {
                let targets = self.disaster_building_candidates(&city);
                if !targets.is_empty() {
                    let building = targets[self.rand_below(targets.len() as i32) as usize];
                    self.cities[index]
                        .buildings
                        .retain(|&built| built != building);
                    details.push(format!("{} destroyed.", building.data().name));
                }
            }

            if has_effect("EmptyFoodStock") && city.food > 0 {
                self.cities[index].food = 0;
                details.push(format!("{} stored food lost.", city.food));
            }

            if has_effect("EmptyProdStock") && city.shields > 0 {
                let city = &mut self.cities[index];
                let shields = city.shields;
                city.shields = 0;
                city.protected_shields = 0;
                city.production_before_switch = None;
                details.push(format!("{shields} shields lost."));
            }
        }

        if city.owner == HUMAN {
            let name = disaster
                .display_name
                .as_deref()
                .unwrap_or(&disaster.rule_name);
            let detail = details.join(" ");
            self.add_event(if detail.is_empty() {
                format!("{name} struck {}.", city.name)
            } else {
                format!("{name} struck {}. {detail}", city.name)
            });
        }
    }

    fn disaster_building_candidates(&self, city: &City) -> Vec<Building> {
        let rules = RulesetData::campaign_default();
        rules
            .content
            .iter()
            .filter(|record| record.kind == ContentKind::Building)
            .filter_map(|record| {
                let building = Building::all().find(|building| {
                    let name = building.data().name;
                    name.eq_ignore_ascii_case(&record.rule_name)
                        || record
                            .display_name
                            .as_deref()
                            .is_some_and(|display| name.eq_ignore_ascii_case(display))
                        || (name.eq_ignore_ascii_case("Harbor")
                            && (record.rule_name.eq_ignore_ascii_case("Harbour")
                                || record.display_name.as_deref().is_some_and(|display| {
                                    display.eq_ignore_ascii_case("Harbour")
                                })))
                })?;
                if building.data().genus != Genus::Improvement
                    || !city.buildings.contains(&building)
                {
                    return None;
                }
                let disaster_proof = record
                    .string_list_field("flags")
                    .unwrap_or_else(|error| panic!("invalid {}.flags: {error}", record.section))
                    .unwrap_or_default()
                    .iter()
                    .any(|flag| flag.eq_ignore_ascii_case("DisasterProof"));
                (!disaster_proof).then_some(building)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disaster(name: &str) -> &'static RuleRecord {
        RulesetData::campaign_default()
            .rule_records
            .iter()
            .find(|record| {
                record.file.eq_ignore_ascii_case("game.ruleset")
                    && record
                        .section
                        .eq_ignore_ascii_case(&format!("disaster_{name}"))
            })
            .unwrap_or_else(|| panic!("missing campaign disaster `{name}`"))
    }

    #[test]
    fn disaster_records_supply_the_selected_types_frequencies_effects_and_requirements() {
        let records: Vec<_> = RulesetData::campaign_default()
            .rule_records
            .iter()
            .filter(|record| record.namespace.eq_ignore_ascii_case("disaster"))
            .collect();
        assert_eq!(records.len(), 6);
        for record in records {
            assert_eq!(record.integer_field("frequency").unwrap(), Some(10));
            assert!(record.string_list_field("effects").unwrap().is_some());
        }
        assert!(disaster("earthquake").requirements.iter().any(|req| {
            req.kind.eq_ignore_ascii_case("Terrain")
                && matches!(&req.name, crate::rules::freeciv::RequirementValue::String(name)
                    if name.eq_ignore_ascii_case("Hills"))
        }));
    }

    #[test]
    fn disasters_check_local_conditions_and_protect_wonders() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let city = &mut game.cities[0];
        city.size = 9;
        city.food = 12;
        city.shields = 17;
        city.protected_shields = 10;
        city.buildings
            .extend([Building::PYRAMIDS, Building::GRANARY]);
        game.tile_mut(10, 20).terrain = Terrain::Hills;
        let (nx, ny) = neighbors(10, 20).next().unwrap();
        game.tile_mut(nx, ny).river = true;
        assert!(game.disaster_possible(0, disaster("earthquake")));
        assert!(game.disaster_possible(0, disaster("flood")));
        assert!(game.disaster_possible(0, disaster("fire")));
        assert!(!game.disaster_possible(0, disaster("industrial_accident")));
        assert!(!game.disaster_possible(0, disaster("nuclear_accident")));
        game.apply_disaster(0, disaster("earthquake"));
        game.apply_disaster(0, disaster("flood"));
        game.apply_disaster(0, disaster("fire"));
        assert_eq!(game.cities[0].shields, 0);
        assert_eq!(game.cities[0].protected_shields, 0);
        assert_eq!(game.cities[0].food, 0);
        assert!(game.cities[0].has_building(Building::PYRAMIDS));
        assert!(!game.cities[0].has_building(Building::GRANARY));
        game.players[HUMAN].gold = 1000;
        let loss = game.city_forecast(&game.cities[0]).trade * 5;
        game.apply_disaster(0, disaster("robbery"));
        assert_eq!(game.players[HUMAN].gold, 1000 - loss);
    }
}
