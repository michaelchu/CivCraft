use super::*;

impl Simulation {
    pub(in crate::sim) fn defender_versus(&self, city: &City, attacker: &Unit) -> Option<Unit> {
        // daimilitary.c:dai_choose_defender_versus: expected damage survived
        // per shield, rewarding surviving the complete attacking unit.
        let mut best = None;
        let mut best_want = 0.0;
        let mut best_cost = i32::MAX;
        for &kind in SIMPLE_MILITARY
            .iter()
            .filter(|&&kind| self.unit_buildable(city, kind))
        {
            let mut defender = self.make_unit(0, city.owner, kind, city.x, city.y);
            defender.home_city = Some(city.id);
            defender.veteran = self.trained_rank(city, kind);
            let defense = self.defense_power_against(&defender, city.x, city.y, attacker.kind);
            let (attack_fp, defense_fp) = self.combat_firepower(attacker, &defender);
            let denominator = Self::attack_power(attacker) * attack_fp;
            if denominator <= 0 || kind.stats().cost <= 0 {
                continue;
            }
            let loss = f64::from(defense * defender.hp * defense_fp) / f64::from(denominator);
            let want =
                (loss + (loss - f64::from(attacker.hp)).max(0.0)) / f64::from(kind.stats().cost);
            let tied = (want - best_want).abs() < 0.001 * (want.abs() + best_want.abs());
            if want > best_want || (tied && kind.stats().cost <= best_cost) {
                best_want = want;
                best_cost = kind.stats().cost;
                best = Some(defender);
            }
        }
        best
    }

    pub(in crate::sim) fn anticipated_city_defender(
        &self,
        city: &City,
        attacker: &Unit,
        arrival: i32,
        production: bool,
    ) -> Option<Unit> {
        let mut defender = self
            .best_defender_id(attacker, city.x, city.y)
            .and_then(|id| self.unit_by_id(id))
            .cloned();
        if production && let Some(unit) = defender.as_mut() {
            unit.hp = unit.kind.stats().hp;
        }
        if arrival <= 1 {
            return defender;
        }
        let rating = |u: &Unit| {
            self.defense_power_against(u, city.x, city.y, attacker.kind)
                * u.hp
                * u.kind.stats().firepower
                / ADVISOR_POWER_DIVIDER
        };
        // The reference assumes a buildable counter-unit beyond one turn;
        // it does not gate this conservative forecast on stock or treasury.
        if let Some(future) = self.defender_versus(city, attacker)
            && defender
                .as_ref()
                .is_none_or(|current| rating(&future) > rating(current))
        {
            defender = Some(future);
        }
        defender
    }

    pub(super) fn production_defender_types() -> impl Iterator<Item = UnitKind> {
        UnitKind::SIMPLE_MILITARY
            .into_iter()
            .filter(|kind| kind.has_role(UnitRole::DefendGood) || kind.has_role(UnitRole::DefendOk))
    }

    pub(super) fn base_defender_desire(kind: UnitKind, firepower: i32) -> f32 {
        let stats = kind.stats();
        let mut desire = stats.hp as f32;
        desire *= firepower as f32;
        desire *= stats.defense as f32;
        desire += stats.moves as f32;
        desire += stats.attack as f32;
        let bonus = if kind == UnitKind::AegisCruiser {
            250
        } else {
            0
        };
        desire + desire * bonus as f32 / 100.0
    }

    pub(super) fn production_defender_desire(kind: UnitKind, danger: i32) -> f32 {
        let good = Self::guard_defender_role(kind);
        let stats = kind.stats();
        // dai_unit_defense_desirability uses type statistics, not the
        // veteran level or the particular enemy's combat odds.
        let firepower = if kind.has_flag(UnitFlag::BadCityDefender) {
            stats.firepower.min(1)
        } else {
            stats.firepower
        };
        let mut desire = Self::base_defender_desire(kind, firepower);
        if kind.has_flag(UnitFlag::GameLoss) {
            desire /= 10.0;
        }
        if !good {
            desire /= 2.0;
        }
        if kind.has_flag(UnitFlag::FieldUnit) {
            if danger == 0 {
                return -50.0;
            }
            desire /= 2.0;
        }
        desire /= (ADVISOR_POWER_DIVIDER / 2) as f32;
        desire *= desire;
        desire
    }

    pub(super) fn production_city_defender(&self, city: &City, danger: i32) -> Option<UnitKind> {
        self.production_city_defender_with_assessment(
            city,
            danger,
            self.advisor_guard_city_assessment(city),
        )
    }

    pub(super) fn production_city_defender_with_assessment(
        &self,
        city: &City,
        danger: i32,
        assessment: GuardCityAssessment,
    ) -> Option<UnitKind> {
        // Native simple_ai_types in ruleset order, intersected with the
        // supported DefendGood/DefendOk roles. Fuelled fighters and Carrier
        // (capacity8) are excluded even though their roles qualify.
        let wallvalue = if assessment.danger > 0 { 90 } else { 5 };
        let walls = matches!(
            self.tile(city.x, city.y).base,
            Some(Base::Fortress | Base::Airbase)
        );
        let mut best = None;
        let mut best_desire = -1.0_f32;
        let mut best_cost = 1;
        let limit = city.shields + 40;
        for kind in Self::production_defender_types() {
            if !self.unit_buildable(city, kind) {
                continue;
            }
            let mut desire = Self::production_defender_desire(kind, danger);
            if desire <= 0.0 {
                continue;
            }
            if walls && !kind.has_flag(UnitFlag::BadCityDefender) {
                desire *= wallvalue as f32;
                desire /= POWER_FACTOR as f32;
            }
            let cost = kind.stats().cost;
            let tied = f64::from(desire - best_desire).abs()
                < 0.001 * (f64::from(desire.abs()) + f64::from(best_desire.abs()));
            // Keep the literal source condition. Its final limit comparison
            // is tautological (limit == stock+40); this is NOT a hard budget.
            // A higher-desire later unit can win even when it costs over40.
            if best_cost > limit && cost < best_cost
                || desire > best_desire
                || tied && cost <= best_cost
            {
                best = Some(kind);
                best_desire = desire;
                best_cost = cost;
            }
        }
        best
    }

    pub(in crate::sim) fn preferred_city_defender(&self, city: &City) -> UnitKind {
        self.production_city_defender(city, 1)
            .unwrap_or(UnitKind::Warrior)
    }

    pub(in crate::sim) fn production_defender_tech_want(&self, city: &City, tech: Tech) -> f32 {
        if !Self::production_defender_types().any(|kind| kind.required_tech() == Some(tech)) {
            return 0.0;
        }
        // The side effect belongs to dai_process_defender_want, not every
        // tactical call to dai_choose_defender_versus. Mirror the production
        // caller's gates before evaluating future defenders.
        let assessment = self.advisor_guard_city_assessment(city);
        let martial = self.city_martial_law_want(city) as f32;
        if assessment.danger == 0 && martial <= 0.0 {
            return 0.0;
        }
        let danger =
            Self::native_city_defense_danger(assessment, self.city_forecast(city).shield_surplus());
        let count = self.tile_units_ordered(city.x, city.y).count() as i32;
        if (danger <= 0 || assessment.defense != 0 && count > assessment.urgency) && martial <= 0.0
        {
            return 0.0;
        }
        let wallvalue = if assessment.danger > 0 {
            90.0_f32
        } else {
            5.0_f32
        };
        let selected = self.production_city_defender_with_assessment(city, danger, assessment);
        let (mut best, best_cost) = selected.map_or((-1.0_f32, 100), |kind| {
            let mut desire = Self::production_defender_desire(kind, danger);
            if !kind.has_flag(UnitFlag::BadCityDefender) {
                desire *= wallvalue;
                desire /= POWER_FACTOR as f32;
            }
            (desire, kind.stats().cost)
        });
        if best <= 0.0 {
            best = 1.0;
        }
        let total = danger as f32 + martial;
        let cities = self.player_cities_ordered(city.owner).count().max(1) as i32;
        let mut want = 0.0_f32;
        for kind in Self::production_defender_types() {
            if self.unit_buildable(city, kind)
                || !self.unit_buildable_later(city, kind)
                || kind.required_tech() != Some(tech)
            {
                continue;
            }
            let mut desire = Self::production_defender_desire(kind, danger);
            if desire <= 0.0 {
                continue;
            }
            // Supported unit requirements are a single reachable advance;
            // goal cost includes its still-unknown prerequisites. The source
            // overwrites (not sums) costs for multi-tech requirements.
            let tech_cost = self
                .research_path(city.owner, tech)
                .into_iter()
                .map(|required| self.target_cost(city.owner, required))
                .sum::<i32>()
                / 4
                / cities;
            if !kind.has_flag(UnitFlag::BadCityDefender) {
                desire *= wallvalue;
                desire /= POWER_FACTOR as f32;
            }
            let tech_desire = desire * total / (kind.stats().cost + tech_cost) as f32;
            if tech_desire > 0.0 {
                want += tech_desire * best_cost as f32 / best;
            }
        }
        want
    }
}
