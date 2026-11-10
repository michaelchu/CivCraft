//! Citizen happiness, disorder, and city mood calculations.

use super::*;

impl Simulation {
    pub(crate) fn city_in_disorder(&self, city: &City, luxury: i32) -> bool {
        self.city_mood(city, luxury).0
    }

    pub(in crate::sim) fn building_content(&self, city: &City) -> i32 {
        let has = |building| city.has_building(building);
        let owns = |wonder| self.has_active_wonder(city.owner, wonder);
        let communist = self.players[city.owner].government == Government::Communism;
        let mut content = 2 * i32::from(has(Building::POLICE_STATION))
            + i32::from(has(Building::COURTHOUSE))
            + i32::from(owns(Building::HANGING_GARDENS));
        if has(Building::TEMPLE) {
            content += (1
                + i32::from(!communist && self.has_tech(city.owner, Tech::MYSTICISM))
                + 2 * i32::from(owns(Building::TEMPLE_OF_ARTEMIS)))
                * (1 + i32::from(owns(Building::ORACLE)));
        }
        if has(Building::CATHEDRAL) {
            content += 3
                + i32::from(!communist && self.has_tech(city.owner, Tech::THEOLOGY))
                + 3 * i32::from(owns(Building::MICHELANGELOS_CHAPEL));
        }
        if has(Building::AMPHITHEATER) {
            content += 3 + i32::from(self.has_tech(city.owner, Tech::ELECTRICITY));
        }
        if has(Building::COLOSSEUM) {
            content += 2;
        }
        if owns(Building::MAUSOLEUM_OF_MAUSOLOS) {
            content += i32::from(has(Building::CITY_WALLS)) + i32::from(has(Building::COURTHOUSE));
        }
        content
    }

    pub(in crate::sim) fn city_mood(&self, city: &City, luxury: i32) -> (bool, bool) {
        let (happy, unhappy, angry) = self.city_feelings(city, luxury);
        (
            happy < unhappy + 2 * angry,
            city.size >= 3 && unhappy == 0 && angry == 0 && happy >= (city.size + 1) / 2,
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
        // Apply luxury before Make_Content.
        let cities = self
            .cities
            .iter()
            .filter(|other| other.owner == city.owner)
            .count() as i32;
        let basis = government.empire_size();
        let empire_penalty = if basis > 0 {
            (cities - 1).max(0) / basis
        } else {
            0
        };
        let base = 4 - empire_penalty;
        let workers = city.tile_workers();
        let specialists = city.size - workers;
        let content = (base.clamp(0, city.size) - specialists).max(0);
        let mut angry = (-base).clamp(0, workers);
        let mut unhappy = workers - content - angry;
        let mut upgrades = luxury / 2;
        let calmed = angry.min(upgrades);
        angry -= calmed;
        unhappy += calmed;
        upgrades -= calmed;
        let happy_content = content.min(upgrades);
        upgrades -= happy_content;
        let happy_unhappy = unhappy.min(upgrades / 2);
        unhappy -= happy_unhappy;
        upgrades -= happy_unhappy * 2;
        if upgrades >= 1 {
            let promoted = unhappy.min(1);
            unhappy -= promoted;
        }

        let martial_limit = match government {
            Government::Despotism => 2,
            Government::Monarchy | Government::Feudalism => 3,
            Government::Communism | Government::Fascism => 4,
            Government::Anarchy | Government::Republic | Government::Democracy => 0,
        };
        let garrison = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == city.owner
                    && unit.kind.is_military()
                    && (unit.x, unit.y) == (city.x, city.y)
            })
            .count()
            .min(martial_limit) as i32;
        let pacification = (if martial_enabled { garrison } else { 0 })
            + self.building_content(city);
        let calming_angry = pacification.min(angry);
        angry -= calming_angry;
        unhappy += calming_angry;
        let promoted = unhappy.min(pacification - calming_angry);
        unhappy -= promoted;

        let happy = happy_content + happy_unhappy;
        (happy, unhappy, angry)
    }
}
