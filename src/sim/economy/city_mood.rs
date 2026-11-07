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
            content += 1
                + i32::from(!communist && self.has_tech(city.owner, Tech::MYSTICISM))
                + 2 * i32::from(owns(Building::TEMPLE_OF_ARTEMIS));
        }
        if has(Building::CATHEDRAL) {
            content += 3
                + i32::from(!communist && self.has_tech(city.owner, Tech::THEOLOGY))
                + 3 * i32::from(owns(Building::MICHELANGELOS_CHAPEL));
        }
        if has(Building::AMPHITHEATER) {
            content += 3 + i32::from(self.has_tech(city.owner, Tech::ELECTRICITY));
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
        self.city_feelings_stages(city, luxury, true, true)
    }

    pub(in crate::sim) fn city_feelings_before_military(
        &self,
        city: &City,
        luxury: i32,
    ) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, false, true)
    }

    pub(in crate::sim) fn city_feelings_after_effects(
        &self,
        city: &City,
        luxury: i32,
    ) -> (i32, i32, i32) {
        self.city_feelings_stages(city, luxury, false, false)
    }

    fn city_feelings_stages(
        &self,
        city: &City,
        luxury: i32,
        military: bool,
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
        let mut content = content - happy_content;
        upgrades -= happy_unhappy * 2;
        if upgrades >= 1 {
            let promoted = unhappy.min(1);
            unhappy -= promoted;
            content += promoted;
        }

        let martial = match government {
            Government::Anarchy | Government::Despotism | Government::Monarchy => 1,
            Government::Communism => 2,
            _ => 0,
        };
        let garrison = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == city.owner
                    && !unit.kind.is_civilian()
                    && (unit.x, unit.y) == (city.x, city.y)
            })
            .count()
            .min(3) as i32;
        let pacification = i32::from(government == Government::Tribal)
            + if martial_enabled {
                martial * garrison
            } else {
                0
            }
            + self.building_content(city);
        let calming_angry = pacification.min(angry);
        angry -= calming_angry;
        unhappy += calming_angry;
        let promoted = unhappy.min(pacification - calming_angry);
        unhappy -= promoted;
        content += promoted;

        let mut happy = happy_content + happy_unhappy;
        if !military {
            return (happy, unhappy, angry);
        }
        let pressure = self.city_military_unhappiness(city);
        let from_content = content.min(pressure);
        unhappy += from_content;
        let pressure = pressure - from_content;
        let from_happy = happy.min(pressure / 2);
        unhappy += from_happy;
        happy = (happy - from_happy - pressure % 2).max(0);
        (happy, unhappy, angry)
    }

    /// city_unit_unhappiness summed over the native supported-unit list.
    pub(in crate::sim) fn city_military_unhappiness(&self, city: &City) -> i32 {
        use super::unit_catalog::{UnitClass, UnitFlag};
        let government = self.players[city.owner].government;
        let suffrage = self.has_active_wonder(city.owner, Building::WOMENS_SUFFRAGE);
        let factor = match government {
            Government::Anarchy | Government::Tribal | Government::Despotism => 0,
            Government::Democracy => 2 - i32::from(suffrage),
            _ => 1,
        };
        if factor == 0 {
            return 0;
        }
        // The provided ruleset has Make_Content_Mil allowances, and NO
        // Make_Content_Mil_Per effects. Police relieve a fixed city total.
        let mut free = match government {
            Government::Republic => 1,
            Government::Monarchy | Government::Communism => 3,
            _ => 2,
        };
        free += i32::from(suffrage) - i32::from(suffrage && government == Government::Democracy);
        free += i32::from(city.has_building(Building::POLICE_STATION))
            * if government == Government::Democracy && !suffrage {
                2
            } else {
                1
            };
        free += i32::from(self.has_active_wonder(city.owner, Building::KING_RICHARDS_CRUSADE));
        free += i32::from(self.has_active_wonder(city.owner, Building::STATUE_OF_ZEUS));
        let mut supported: Vec<_> = self
            .units
            .iter()
            .filter(|unit| unit.home_city == Some(city.id) && unit.owner == city.owner)
            .collect();
        supported.sort_by_key(|unit| std::cmp::Reverse(unit.home_order));
        let mut pressure = 0;
        for unit in supported {
            let happy_cost = unit.kind.base_upkeep().2 * factor;
            if happy_cost <= 0 {
                continue;
            }
            let class = unit.kind.unit_class();
            let friendly_extra = match self.tile(unit.x, unit.y).base {
                Some(Base::Fortress) => matches!(
                    class,
                    UnitClass::Land
                        | UnitClass::SmallLand
                        | UnitClass::BigLand
                        | UnitClass::Merchant
                ),
                Some(Base::Airbase) => matches!(
                    class,
                    UnitClass::Air | UnitClass::Helicopter | UnitClass::Missile
                ),
                _ => false,
            } && self.cities.iter().any(|nearby| {
                self.allied(nearby.owner, unit.owner)
                    && self.map_distance(unit.x, unit.y, nearby.x, nearby.y) <= 3
            });
            let aggressive = unit.kind.stats().attack > 0
                && !self
                    .cities
                    .iter()
                    .any(|other| (other.x, other.y) == (unit.x, unit.y))
                && self.tile(unit.x, unit.y).owner != Some(unit.owner)
                && !friendly_extra;
            if !aggressive && !unit.kind.has_flag(UnitFlag::FieldUnit) {
                continue;
            }
            let relieved = free.min(happy_cost);
            free -= relieved;
            pressure += happy_cost - relieved;
        }
        pressure
    }

    pub(in crate::sim) fn offensive_military_unhappiness(&self, city: &City) -> bool {
        self.city_military_unhappiness(city) > 0
    }
}
