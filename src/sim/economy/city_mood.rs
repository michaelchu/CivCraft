//! City mood uses C2C3 effects and the common Freeciv citizen conversion order.

use super::*;

impl Simulation {
    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
        self.city_mood(city, luxury).0
    }

    pub(in crate::sim) fn city_mood(&self, city: &City, luxury: i32) -> (bool, bool) {
        let (happy, unhappy, angry) = self.city_feelings(city, luxury);
        (
            happy < unhappy + 2 * angry,
            city.size >= ruleset::CityEconomySettings::campaign().celebrate_size_limit
                && unhappy == 0
                && angry == 0
                && happy >= (city.size + 1) / 2,
        )
    }

    pub(in crate::sim) fn city_feelings(&self, city: &City, luxury: i32) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, true)
    }

    pub(in crate::sim) fn city_feelings_after_effects(
        &self,
        city: &City,
        luxury: i32,
    ) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, false)
    }

    fn city_feelings_stages(
        &self,
        city: &City,
        luxury: i32,
        martial_enabled: bool,
    ) -> (i32, i32, i32) {
        let government = self.players[city.owner].government;
        let effect = |kind: &str| {
            self.city_rule_effect(city, kind, None, None, None, false, government, None)
        };
        let base = self.city_base_content(city, government);

        let specialists = city.specialists.iter().sum::<i32>().clamp(0, city.size);
        let content = (base.min(city.size).max(0) - specialists).max(0);
        let mut angry = (-base).clamp(0, city.size - specialists);
        let mut unhappy = (city.size - specialists - content - angry).max(0);
        let mut happy = 0;

        // city.c:citizen_luxury_happy converts angry, content, unhappy->happy,
        // then one remaining luxury can make an unhappy citizen content.
        let happy_cost = ruleset::CityEconomySettings::campaign().happy_cost.max(1);
        let mut remaining_luxury = luxury.max(0);
        while remaining_luxury >= happy_cost && angry > 0 {
            angry -= 1;
            unhappy += 1;
            remaining_luxury -= happy_cost;
        }
        let mut content = content;
        while remaining_luxury >= happy_cost && content > 0 {
            content -= 1;
            happy += 1;
            remaining_luxury -= happy_cost;
        }
        while remaining_luxury >= 2 * happy_cost && unhappy > 0 {
            unhappy -= 1;
            happy += 1;
            remaining_luxury -= 2 * happy_cost;
        }
        if remaining_luxury >= happy_cost && unhappy > 0 {
            unhappy -= 1;
            content += 1;
        }

        // Make_Content effects first calm angry citizens, then contentify
        // unhappy citizens, matching city.c:citizen_content_buildings.
        let mut content_faces = effect("Make_Content").max(0);
        while content_faces > 0 && angry > 0 {
            angry -= 1;
            unhappy += 1;
            content_faces -= 1;
        }
        while content_faces > 0 && unhappy > 0 {
            unhappy -= 1;
            content += 1;
            content_faces -= 1;
        }

        // C2C3 martial law applies to non-civilian units physically present
        // in the city. Each unit's effect is evaluated through its own type.
        if martial_enabled {
            let max_law = effect("Martial_Law_Max").max(0);
            let mut martial_values: Vec<_> = self
                .units
                .iter()
                .filter(|unit| {
                    unit.owner == city.owner
                        && !unit.kind.is_civilian()
                        && (unit.x, unit.y) == (city.x, city.y)
                })
                .map(|unit| {
                    self.city_rule_effect_for_unit(city, unit, "Martial_Law_By_Unit", government)
                })
                .filter(|value| *value > 0)
                .collect();
            martial_values.sort_unstable_by(|left, right| right.cmp(left));
            let mut martial = martial_values
                .into_iter()
                .take(max_law as usize)
                .sum::<i32>()
                .min(city.size);
            while martial > 0 && angry > 0 {
                angry -= 1;
                unhappy += 1;
                martial -= 1;
            }
            while martial > 0 && unhappy > 0 {
                unhappy -= 1;
                content += 1;
                martial -= 1;
            }
        }

        // city.c:citizen_happy_units applies unit unrest after martial law.
        // The conversion also runs when martial law is disabled by government.
        let mut unit_unhappiness = self.city_unit_upkeep(city).unit_happiness;
        while unit_unhappiness > 0 && content > 0 {
            content -= 1;
            unhappy += 1;
            unit_unhappiness -= 1;
        }
        while unit_unhappiness > 1 && happy > 0 {
            happy -= 1;
            unhappy += 1;
            unit_unhappiness -= 2;
        }
        while unit_unhappiness > 0 && happy > 0 {
            happy -= 1;
            content += 1;
            unit_unhappiness -= 1;
        }
        // Make_Happy precedes No_Unhappy and Force_Content in the final mood
        // stage; these effects are data-driven and also cover wonders.
        let mut happy_faces = effect("Make_Happy").max(0);
        while happy_faces > 0 && content > 0 {
            content -= 1;
            happy += 1;
            happy_faces -= 1;
        }
        while happy_faces > 1 && unhappy > 0 {
            unhappy -= 1;
            happy += 1;
            happy_faces -= 2;
        }
        if effect("No_Unhappy") > 0 {
            unhappy = 0;
            angry = 0;
        } else {
            let mut force_faces = happy_faces + effect("Force_Content").max(0);
            while force_faces > 0 && angry > 0 {
                angry -= 1;
                unhappy += 1;
                force_faces -= 1;
            }
            while force_faces > 0 && unhappy > 0 {
                unhappy -= 1;
                force_faces -= 1;
            }
        }

        (happy, unhappy, angry)
    }

    pub(super) fn city_base_content(&self, city: &City, government: Government) -> i32 {
        let effect = |kind: &str| {
            self.city_rule_effect(city, kind, None, None, None, false, government, None)
        };
        let city_count = self
            .cities
            .iter()
            .filter(|other| other.owner == city.owner)
            .count() as i32;
        let base = effect("City_Unhappy_Size");
        let empire_base = effect("Empire_Size_Base");
        let empire_step = effect("Empire_Size_Step");
        if empire_base + empire_step <= 0 || city_count <= empire_base {
            return base;
        }
        base - 1
            - if empire_step > 0 {
                (city_count - empire_base - 1) / empire_step
            } else {
                0
            }
    }
}
