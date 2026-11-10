//! Advisor scoring of city production choices.

use super::*;

impl Simulation {
    pub(in crate::sim) fn city_attacker_odds(
        &self,
        city: &City,
        enemy: &Unit,
        defender: &Unit,
        building: Option<Building>,
    ) -> f64 {
        let full_strength = percent_of(
            enemy.kind.stats().attack * POWER_FACTOR,
            enemy.veteran_power(),
        );
        let (attack_fp, defense_fp) = self.combat_firepower(enemy, defender);
        combat_win_chance(
            full_strength,
            enemy.hp,
            attack_fp,
            self.defense_power_with_building(defender, city.x, city.y, enemy.kind, building),
            defender.hp,
            defense_fp,
        )
    }

    pub(super) fn city_martial_law_want(&self, city: &City) -> f64 {
        let amount = match self.players[city.owner].government {
            Government::Anarchy | Government::Despotism | Government::Monarchy => 1,
            Government::Communism => 2,
            _ => 0,
        };
        if amount == 0
            || city.has_building(Building::SHAKESPEARES_THEATER)
            || self.tile_units_ordered(city.x, city.y).count() >= 3
        {
            return 0.0;
        }
        // FEELING_FINAL is the sole happiness step used by the military
        // advisor. An entertainer also justifies a police unit even when the
        // present assignment has already removed the city's unhappiness.
        let (_, unhappy, _) = self.city_feelings(city, self.city_forecast(city).luxury);
        f64::from(amount.min(unhappy + city.specialists[2]) * 35)
    }

    pub(super) fn native_city_defense_danger(assessment: GuardCityAssessment, surplus: i32) -> i32 {
        // Literal source positive feedback: assess_danger has squared each
        // attack rating already, and military_advisor squares its sum again.
        // Wider intermediates avoid undefined native signed overflow.
        let qdanger = i64::from(assessment.danger).pow(2);
        let defense = i64::from(assessment.defense);
        if qdanger == 0 || surplus <= 0 && defense != 0 {
            return 0;
        }
        if qdanger >= defense {
            if assessment.urgency == 0 {
                100
            } else if defense == 0 {
                200 + assessment.urgency
            } else {
                (200.min(100 * qdanger / defense)) as i32 + assessment.urgency
            }
        } else {
            (100 * qdanger / defense) as i32
        }
    }

    pub(super) fn defense_building_provider(
        &self,
        city: &City,
        attacker: Option<UnitKind>,
    ) -> Option<Building> {
        // dai_find_source_building picks the greatest single effect, with
        // strict greater-than preserving ruleset order on ties. All three
        // Defend_Bonus providers have value100. A NULL type deliberately
        // skips class requirements, matching the native advisor's FIXME.
        [
            Building::CITY_WALLS,
            Building::COASTAL_DEFENSE,
            Building::SAM_BATTERY,
        ]
        .into_iter()
        .find(|&building| {
            self.building_blocker(city, building).is_none()
                && attacker.is_none_or(|kind| match building {
                    Building::CITY_WALLS => !kind.is_aircraft() && !kind.is_naval(),
                    Building::COASTAL_DEFENSE => kind.is_naval(),
                    Building::SAM_BATTERY => kind.is_aircraft() && !kind.is_missile(),
                    _ => false,
                })
        })
    }

    pub(super) fn native_defense_building_want(
        &self,
        city: &City,
        building: Building,
        assessment: GuardCityAssessment,
    ) -> f64 {
        let initial = self.domestic_improvement_want(city, building) as f32;
        if initial == 0.0 {
            return 0.0;
        }
        let reduced: i64 = self
            .city_threat_arrivals(city)
            .filter(|(enemy, _, _)| {
                self.defense_building_provider(city, Some(enemy.kind)) == Some(building)
            })
            .map(|(enemy, _, turns)| {
                let vulnerability = i64::from(self.advisor_threat_vulnerability(city, enemy));
                // Native danger_reduced discounts the already time-discounted
                // vulnerability once more, independently of total danger.
                vulnerability * vulnerability / i64::from(turns.max(1)).pow(2)
            })
            .sum();
        if reduced <= 0 {
            return f64::from(initial);
        }
        let mut want = initial.max((100 + assessment.urgency) as f32);
        if assessment.urgency > 0 && reduced > i64::from(assessment.defense_igwall) * 2 {
            want += 100.0;
        } else if assessment.defense_igwall != 0 && reduced > i64::from(assessment.defense_igwall) {
            want = want.max((reduced * 100 / i64::from(assessment.defense_igwall)) as f32);
        }
        f64::from(want)
    }

    pub(in crate::sim) fn native_city_defense_choice(
        &self,
        city: &City,
    ) -> (Option<AiBuildChoice>, f64) {
        let assessment = self.advisor_guard_city_assessment(city);
        let martial = self.city_martial_law_want(city);
        if assessment.danger == 0 && martial <= 0.0 {
            return (None, martial);
        }
        let danger =
            Self::native_city_defense_danger(assessment, self.city_forecast(city).shield_surplus());
        let count = self.tile_units_ordered(city.x, city.y).count() as i32;
        let Some(kind) = self.production_city_defender_with_assessment(city, danger, assessment)
        else {
            return (None, martial);
        };
        if assessment.defense == 0 && danger > 0 {
            return (
                Some(AiBuildChoice {
                    production: Production::Unit(kind),
                    want: f64::from((1000 + danger) as f32),
                }),
                martial,
            );
        }
        let mut choice = None;
        if assessment.danger > 0
            && assessment.defense != 0
            && let Some(building) = self.defense_building_provider(city, None)
        {
            // This is the pure impr_buy_gold_cost quote, not the action's
            // founded-this-turn/disorder/bought-production legality gate.
            let price = 2
                * (building.data().cost - city.shields).max(0)
                * if city.shields == 0 { 2 } else { 1 };
            let affordable = self.players[city.owner].gold > price;
            let want = self.native_defense_building_want(city, building, assessment);
            if want > 0.0 && (danger < 101 || count > 1 || assessment.grave == 0 && affordable) {
                let want = f64::from((want * 0.5) as f32);
                choice = Some(AiBuildChoice {
                    production: Production::Building(building),
                    want: if assessment.urgency == 0 {
                        want.min(1000.0)
                    } else {
                        want
                    },
                });
            }
        }
        if danger > 0 && count <= assessment.urgency || martial > 0.0 {
            let want = if assessment.urgency == 0 && kind.stats().defense == 1 {
                // get_city_bonus(HP_Regen) has NULL type, so class-specific
                // Barracks/Port/Airport effects do not satisfy CERTAIN.
                danger.min(25)
            } else {
                danger
            };
            let candidate = AiBuildChoice {
                production: Production::Unit(kind),
                want: f64::from(want as f32 + martial as f32),
            };
            if choice.is_none_or(|old| candidate.want > old.want) {
                choice = Some(candidate);
            }
        }
        (choice, martial)
    }

    pub(in crate::sim) fn city_defense_production(&self, city: &City) -> Option<Production> {
        self.native_city_defense_choice(city)
            .0
            .map(|choice| choice.production)
    }

    pub(in crate::sim) fn bodyguard_production_choice(
        &self,
        city: &City,
        sea: bool,
    ) -> Option<(UnitKind, f64)> {
        // daimilitary.c:dai_choose_bodyguard deliberately chooses the best
        // defensive type BEFORE checking whether it can serve any charge.
        // The source computes allow_gold from tax and ACTUAL homed upkeep,
        // then queries utype_upkeep_cost(type,NULL,player,Gold). With no unit
        // context HasHomeCity is MAYBE, so every provided Gold Upkeep_Pct
        // row fails CERTAIN and all candidate type gold costs are zero.
        let mut best = None;
        let mut best_desire = 0.0_f32;
        for kind in UnitKind::simple_military() {
            if !Self::guard_defender_role(kind)
                || kind.is_civilian()
                || kind.is_naval() != sea
                || !self.unit_buildable(city, kind)
            {
                continue;
            }
            let stats = kind.stats();
            // Bodyguards use full type firepower; city defense separately
            // applies the bad-city-defender adjustment before this shared score.
            let desire = Self::base_defender_desire(kind, stats.firepower);
            let tied = (f64::from(desire - best_desire)).abs()
                < 0.001 * (f64::from(desire.abs()) + f64::from(best_desire.abs()));
            if desire > best_desire
                || (tied && best.is_none_or(|old: UnitKind| stats.cost <= old.stats().cost))
            {
                best = Some(kind);
                best_desire = desire;
            }
        }
        let kind = best?;
        let mut virtual_unit = self.make_unit(0, city.owner, kind, city.x, city.y);
        virtual_unit.veteran = self.trained_rank(city, kind);
        virtual_unit.hp = virtual_unit.max_hit_points();
        virtual_unit.home_city = Some(city.id);
        self.look_for_ai_charge(&virtual_unit)
            .and_then(|(_, want)| (want > 0.0).then_some((kind, f64::from(want))))
    }

    pub(in crate::sim) fn consider_bodyguard_production(
        &self,
        city: &City,
        sea: bool,
        choice: &mut Option<AiBuildChoice>,
    ) -> bool {
        // dai_unit_consider_bodyguard never overrides a military emergency.
        if choice.is_some_and(|old| old.want >= MILITARY_EMERGENCY_WANT) {
            return false;
        }
        if let Some((kind, want)) = self.bodyguard_production_choice(city, sea)
            && choice.is_none_or(|old| want > old.want)
        {
            *choice = Some(AiBuildChoice {
                production: Production::Unit(kind),
                want,
            });
            true
        } else {
            false
        }
    }

    pub(super) fn military_training_building(
        &self,
        city: &City,
        kind: UnitKind,
    ) -> Option<Building> {
        // dai_find_source_building(EFT_VETERAN_BUILD): effects are examined
        // in ruleset order; only buildable ordinary improvements qualify.
        // All supported provider effects have value1. Wonders/governments
        // are not infrastructure providers, though existing bonuses train.
        let candidates: &[Building] = if kind.is_aircraft() || kind.is_missile() {
            &[Building::AIRPORT]
        } else if kind.is_naval() {
            &[Building::PORT_FACILITY]
        } else {
            &[
                Building::BARRACKS_III,
                Building::BARRACKS_II,
                Building::BARRACKS,
            ]
        };
        candidates.iter().copied().find(|&building| {
            building.data().genus == Genus::Improvement
                && self.building_blocker(city, building).is_none()
        })
    }

    pub(in crate::sim) fn adjust_military_training_choice(
        &self,
        city: &City,
        choice: &mut AiBuildChoice,
    ) {
        let Production::Unit(kind) = choice.production else {
            return;
        };
        // Civ III's default rank is Regular (rank 1). Training buildings
        // should still replace routine unit advice when they would promote
        // that unit to Veteran; only an existing rank above the default
        // means the city already grants the training effect.
        let default_rank = crate::sim::civ3_unit_rules::default_experience_rank();
        if kind.is_civilian() || kind.no_veteran() || self.trained_rank(city, kind) > default_rank {
            return;
        }
        if let Some(building) = self.military_training_building(city, kind) {
            choice.production = Production::Building(building);
            // All six civilizations use the default Builder50. Native int divisions
            // 50/TRAIT_DEFAULT_VALUE(50)/2 yield0, before adding double0.5.
            choice.want = (f64::from(choice.want as f32) * 0.5) as f32 as f64;
        }
    }

    pub(in crate::sim) fn routine_military_advice_allowed(&self, city: &City) -> bool {
        let forecast = self.city_forecast(city);
        let (_, final_unhappy, _) = self.city_feelings(city, forecast.luxury);
        let (_, effect_unhappy, _) = self.city_feelings_after_effects(city, forecast.luxury);
        forecast.shield_surplus() > 0
            && final_unhappy <= effect_unhappy
            && self.ai_wonder_city(city.owner) != Some(city.id)
    }

    /// Shared native stages preceding the sea/land offensive callbacks.
    /// Returns (current choice, immediate return, severe-defense return).
    pub(in crate::sim) fn pre_offensive_military_advice(
        &self,
        city: &City,
    ) -> (Option<AiBuildChoice>, bool, bool) {
        let (mut military, martial) = self.native_city_defense_choice(city);
        // Source returns its life-saving defense advice before all routine
        // candidates and training when production/happiness/wonder gates fail.
        if !self.routine_military_advice_allowed(city) {
            return (military, true, false);
        }
        let mut defensive = military.is_some();
        if self.consider_bodyguard_production(city, false, &mut military) {
            defensive = true;
        }
        // Strict wants preserve source ordering among eligible unit choices.
        let consider_unit = |choice: &mut Option<AiBuildChoice>,
                             candidate: Option<(UnitKind, f64)>| {
            if let Some((kind, want)) = candidate
                && want > 0.0
                && choice.is_none_or(|old| want > old.want)
            {
                *choice = Some(AiBuildChoice {
                    production: Production::Unit(kind),
                    want,
                });
                true
            } else {
                false
            }
        };
        let severe = military.is_some_and(|old| old.want - martial >= MILITARY_EMERGENCY_WANT)
            && self.city_grave_danger(city) > 0;
        if !severe {
            if self.consider_bodyguard_production(city, true, &mut military) {
                defensive = true;
            }
            if !(defensive && military.is_some_and(|old| old.want >= MILITARY_EMERGENCY_WANT)) {
                consider_unit(&mut military, self.aircraft_production_choice(city));
            }
        }
        (military, false, severe)
    }

    pub(in crate::sim) fn military_build_advice(
        &self,
        city: &City,
    ) -> (Option<AiBuildChoice>, Option<AiBuildChoice>) {
        let (mut military, early, severe) = self.pre_offensive_military_advice(city);
        if early {
            return (military, military);
        }
        if !severe {
            // kill_something_with performs its comparison BEFORE a ferry
            // replacement truncates want. Accept its final choice directly.
            military = self.offensive_advice_with_prior(city, military).0;
            if let Some((kind, want)) = self.hunter_production_choice(city)
                && want > 0.0
                && military.is_none_or(|old| want > old.want)
            {
                military = Some(AiBuildChoice {
                    production: Production::Unit(kind),
                    want,
                });
            }
        }
        let before = military;
        if !severe && let Some(choice) = military.as_mut() {
            self.adjust_military_training_choice(city, choice);
        }
        (before, military)
    }

    pub(in crate::sim) fn ai_build_choice(&self, city: &City) -> Option<AiBuildChoice> {
        let (_, military) = self.military_build_advice(city);
        if military.is_some() && self.ai_on_war_footing(city.owner) {
            return military;
        }

        let mut best = military;
        let mut consider = |production, want: f64| {
            let want = want.min(DOMESTIC_WANT_MAX);
            if want > 0.0 && best.is_none_or(|choice| want > choice.want) {
                best = Some(AiBuildChoice { production, want });
            }
        };
        if let Some(choice) = self.domestic_build_choice(city) {
            consider(choice.production, choice.want);
        }
        if let Some(choice) = self.wonder_build_choice(city) {
            consider(choice.production, choice.want);
        }
        if let Some((kind, want)) = self.settlement_production_choice(city) {
            let want = if self.ai_wonder_city(city.owner) == Some(city.id) {
                want / if kind.stats().pop_cost > 0 { 5.0 } else { 2.0 }
            } else {
                want
            };
            consider(Production::Unit(kind), want);
        }
        for building in [
            Building::SPACE_MODULE,
            Building::SPACE_COMPONENT,
            Building::SPACE_STRUCTURAL,
        ] {
            if self.building_blocker(city, building).is_none() {
                consider(
                    Production::Building(building),
                    self.space_production_want(city, building),
                );
                break;
            }
        }
        best
    }

    pub(in crate::sim) fn prepare_ai_city_defenses(&mut self, owner: usize) {
        let ids: Vec<_> = self
            .player_cities_ordered(owner)
            .map(|city| city.id)
            .collect();
        for id in ids {
            let Some(city) = self.city_by_id(id).cloned() else {
                continue;
            };
            self.record_ai_military_tech_wants(&city);
            if let Some(choice) = self.ai_build_choice(&city)
                && city.production != choice.production
            {
                self.change_production(id, choice.production);
            }
        }
    }
}
