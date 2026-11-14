//! Default AI offensive production and its research side effects.
//! daimilitary.c:kill_something_with/process_attacker_want; target selection
//! follows daiunit.c:find_something_to_kill (Normal's H_TARGETS policy).
use super::advisors::AiBuildChoice;

mod targeting;
use super::normal_paths::NormalPaths;
use super::unit_catalog::{UnitAiStrategy, UnitFlag};
use super::*;

#[derive(Clone)]
struct OffensiveTarget {
    x: i32,
    y: i32,
    city: Option<u32>,
    turns: i32,
    want: f32,
    ferry: Option<Unit>,
    new_ferry: bool,
}

fn kill_desire(benefit: f32, attack: i32, loss: i32, vulnerability: i32, victims: i32) -> f32 {
    let denominator = attack + vulnerability * victims;
    if denominator == 0 {
        return 0.0;
    }
    (benefit * attack as f32 - (loss * vulnerability) as f32) * victims as f32 * 17.0
        / denominator as f32
}

impl Simulation {
    fn offensive_attack_rating(unit: &Unit) -> i32 {
        Self::attack_power(unit) * unit.hp * unit.kind.stats().firepower / (POWER_FACTOR * 3)
    }

    fn offensive_balanced_cost(kind: UnitKind) -> i32 {
        let s = kind.stats();
        2 * s.cost * s.attack / (s.attack + s.defense)
    }

    fn offensive_amortize(&self, city: &City, value: f32, delay: i32, cost: i32) -> f32 {
        if value <= 0.0 {
            return 0.0;
        }
        let output = if self.ai_advisor_cache_enabled.get() {
            self.ai_spatial_index
                .borrow()
                .offensive_outputs
                .get(&city.id)
                .copied()
        } else {
            None
        }
        .unwrap_or_else(|| {
            let output = self.offensive_production_output(city);
            if self.ai_advisor_cache_enabled.get() {
                self.ai_spatial_index
                    .borrow_mut()
                    .offensive_outputs
                    .insert(city.id, output);
            }
            output
        });
        (f64::from(value) * (23.0_f64 / 24.0).powi(delay + cost / output)) as f32
    }

    fn offensive_production_output(&self, city: &City) -> i32 {
        let cities: Vec<_> = self.player_cities_ordered(city.owner).collect();
        let average = cities
            .iter()
            .map(|c| self.city_forecast(c).shield_surplus())
            .sum::<i32>()
            / cities.len().max(1) as i32;
        self.city_forecast(city)
            .shield_surplus()
            .max(average)
            .max(1)
    }

    fn offensive_seed(&self, city: &City, sea: bool) -> Option<UnitKind> {
        let mut best = None;
        let mut best_want = -1.0_f32;
        for kind in UnitKind::simple_military() {
            if !self.can_produce(city, Production::Unit(kind))
                || (sea && !kind.is_naval() && !kind.is_aircraft() && !kind.is_missile())
                || (!sea && kind.is_naval())
            {
                continue;
            }
            let s = kind.stats();
            let mut want = s.hp as f32;
            want *= (s.moves * MOVE_FRAGMENTS) as f32;
            want *= s.firepower as f32;
            want *= s.attack as f32;
            want += s.defense as f32;
            if s.ignores_terrain {
                want += want / 2.0;
            }
            if kind.has_flag(UnitFlag::GameLoss) {
                want /= 10.0;
            }
            if kind.has_flag(UnitFlag::CityBuster) {
                want += want / 2.0;
            }
            if kind.has_marine_attack() || kind.is_naval() {
                want += want / 4.0;
            }
            // The supplied ruleset's class-specific Defend_Bonus rows make
            // the native igwall cache false for every supported type.
            if want > best_want
                || want == best_want && best.is_some_and(|old: UnitKind| s.cost <= old.stats().cost)
            {
                best = Some(kind);
                best_want = want;
            }
        }
        best
    }

    fn offensive_defense_squared(
        &self,
        defender: &Unit,
        attacker: UnitKind,
        x: i32,
        y: i32,
        full_hp: bool,
    ) -> i32 {
        let hp = if full_hp {
            defender.max_hit_points()
        } else {
            defender.hp
        };
        let rating = self.defense_power_against(defender, x, y, attacker)
            * hp
            * defender.kind.stats().firepower
            / (POWER_FACTOR * 3);
        rating * rating
    }

    fn offensive_helpers(&self, unit: &Unit, city: &City) -> (i32, i32) {
        let mut attack = 0;
        let mut cost = 0;
        for helper in &self.units {
            if helper.id == unit.id && unit.id != 0 || !self.allied(unit.owner, helper.owner) {
                continue;
            }
            let rating = Self::offensive_attack_rating(helper);
            if rating == 0 {
                continue;
            }
            let nearby = i32::from(
                self.map_wrap
                    .civ3_distance(helper.x, helper.y, city.x, city.y)
                    <= 1,
            );
            let going = i32::from(
                helper.owner == unit.owner
                    && self.ai_ground_targets.get(&helper.id) == Some(&city.id),
            );
            attack += rating * (nearby + going);
            cost += helper.kind.stats().cost * (nearby + going);
        }
        (attack, cost)
    }

    fn offensive_city_worth(&self, city: &City) -> f64 {
        0.9 * f64::from(self.advisor_city_output(city, None)) + (f64::from(city.size) - 0.5) * 10.0
    }

    fn offensive_finishing_factor(&self, city: &City) -> f32 {
        let count = self.player_cities_ordered(city.owner).count();
        if count <= 5 {
            2.0 - count as f32 / 5.0
        } else {
            1.0
        }
    }

    fn offensive_obsolete(&self, city: &City, kind: UnitKind) -> bool {
        // process_attacker_want asks about the IMMEDIATE successor's
        // direct requirements, deliberately ignoring its own obsolescence.
        kind.obsolete_by().is_some_and(|successor| {
            self.unit_available(city.owner, successor)
                && (successor.unit_class().build_anywhere()
                    || successor.is_aircraft()
                    || self.naval_city_access(city, successor))
        })
    }

    fn offensive_native_near(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        self.pf_native_tile(kind, x, y)
            || self
                .map_wrap
                .civ3_neighbors(x, y)
                .any(|(x, y)| self.pf_native_tile(kind, x, y))
    }

    fn offensive_can_attack(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        kind.stats().attack > 0
            && (self.pf_native_tile(kind, x, y)
                || kind.is_naval() && !matches!(kind, UnitKind::Galley | UnitKind::Submarine))
    }

    /// daiunit.c:find_beachhead; strict lower movement cost retains the
    /// first native adjacent-tile candidate, including overlap land endpoints.
    fn offensive_ferry_role(
        &self,
        city: &City,
        want: i32,
        research: &mut Vec<(Tech, f32)>,
    ) -> Option<UnitKind> {
        // daitech.c:dai_wants_role_unit walks the role backwards, stops at
        // the first buildable unit and stimulates the cheapest future role.
        let mut build = None;
        let mut best_cost = i32::MAX;
        let mut best_tech = None;
        for kind in UnitKind::ALL_NATIVE
            .into_iter()
            .rev()
            .filter(|kind| kind.has_ai_strategy(UnitAiStrategy::NavalTransport))
        {
            if self.can_produce(city, Production::Unit(kind)) {
                build = Some(kind);
                break;
            }
            if !self.unit_buildable_later(city, kind) {
                continue;
            }
            if let Some(tech) = kind.required_tech() {
                let path = self.research_path(city.owner, tech);
                if path.is_empty() {
                    continue;
                }
                let cost = path
                    .into_iter()
                    .map(|tech| self.target_cost(city.owner, tech))
                    .sum::<i32>();
                if cost < best_cost {
                    best_cost = cost;
                    best_tech = Some(tech);
                }
            }
        }
        if let Some(tech) = best_tech {
            research.push((tech, if build.is_some() { want / 2 } else { want } as f32));
        }
        build
    }

    pub(super) fn offensive_advice(
        &self,
        city: &City,
    ) -> (Option<AiBuildChoice>, Vec<(Tech, f32)>) {
        let (prior, early, severe) = self.pre_offensive_military_advice(city);
        if early || severe {
            return (None, Vec::new());
        }
        let (choice, research, replaced) = self.cached_offensive_advice(city, prior);
        (if replaced { choice } else { None }, research)
    }

    pub(super) fn cached_offensive_advice(
        &self,
        city: &City,
        prior: Option<AiBuildChoice>,
    ) -> (Option<AiBuildChoice>, Vec<(Tech, f32)>, bool) {
        self.trace.count_operation("offensive_advice.calls", 1);
        let cache_enabled = self.ai_advisor_cache_enabled.get();
        if cache_enabled
            && let Some(result) = self
                .ai_spatial_index
                .borrow()
                .offensive_advice
                .get(&city.id)
                .cloned()
        {
            self.trace.count_operation("offensive_advice.cache_hit", 1);
            return result;
        }
        if cache_enabled {
            self.trace.count_operation("offensive_advice.cache_miss", 1);
        }
        let result = self.offensive_advice_with_prior(city, prior);
        if self.ai_advisor_cache_enabled.get() {
            self.ai_spatial_index
                .borrow_mut()
                .offensive_advice
                .insert(city.id, result.clone());
        }
        result
    }

    /// Caller has passed the shared routine/severe-defense stages. Selection
    /// provenance survives the source's later integer ferry-role conversion.
    pub(super) fn offensive_advice_with_prior(
        &self,
        city: &City,
        prior: Option<AiBuildChoice>,
    ) -> (Option<AiBuildChoice>, Vec<(Tech, f32)>, bool) {
        let mut research = Vec::new();
        let mut choice = prior;
        let mut replaced = false;
        let assessment = self.advisor_guard_city_assessment(city);
        if assessment.danger != 0 && assessment.defense == 0 {
            return (choice, research, replaced);
        }
        for sea in [true, false] {
            let Some(seed_kind) = self.offensive_seed(city, sea) else {
                continue;
            };
            if seed_kind.fuel_capacity() > 0 {
                continue;
            }
            let mut seed = self.make_unit(0, city.owner, seed_kind, city.x, city.y);
            seed.home_city = Some(city.id);
            seed.veteran = if sea {
                self.trained_rank(city, seed_kind)
            } else {
                1
            };
            seed.hp = seed.max_hit_points();
            let Some(target) = self.offensive_target(city, &seed) else {
                continue;
            };
            let enemy_city = target.city.and_then(|id| self.city_by_id(id));
            let (defender, base_benefit) = self.offensive_victim(&seed, &target);
            let (helper, helper_cost) =
                enemy_city.map_or((0, 0), |enemy| self.offensive_helpers(&seed, enemy));
            let (_, occupiers) =
                enemy_city.map_or((0, 0), |enemy| self.invasion_support(city.owner, enemy, 0));
            let mut benefit = base_benefit;
            if let Some(enemy) = enemy_city
                && (seed.kind.can_occupy_city() || occupiers > 0)
            {
                benefit = (f64::from(benefit)
                    + self.offensive_city_worth(enemy)
                        * f64::from(self.offensive_finishing_factor(enemy))
                        / 3.0) as f32;
            }
            let mut selected = AiBuildChoice {
                production: Production::Unit(seed_kind),
                want: f64::from(target.want),
            };
            for kind in UnitKind::simple_military() {
                if kind.stats().attack <= 0
                    || !Self::guard_can_follow(kind, seed_kind)
                    || !self.offensive_native_near(kind, target.x, target.y)
                    || self.offensive_obsolete(city, kind)
                {
                    continue;
                }
                let mut candidate = self.make_unit(0, city.owner, kind, city.x, city.y);
                candidate.veteran = self.trained_rank(city, kind);
                candidate.hp = candidate.max_hit_points();
                let Some(turns) = self.offensive_candidate_turns(&candidate, &target) else {
                    continue;
                };
                let tech = kind.required_tech();
                let missing =
                    tech.map_or_else(Vec::new, |tech| self.research_path(city.owner, tech));
                let tech_cost = missing
                    .iter()
                    .map(|&tech| self.target_cost(city.owner, tech))
                    .sum::<i32>()
                    / 4
                    / self.player_cities_ordered(city.owner).count().max(1) as i32;
                let power = (Self::offensive_attack_rating(&candidate) + helper).pow(2);
                if power == 0 {
                    continue;
                }
                let vulnerability = defender.as_ref().map_or(0, |def| {
                    self.offensive_defense_squared(def, kind, target.x, target.y, true)
                });
                let victims = self.offensive_victim_count(target.x, target.y);
                let mut desire = if vulnerability == 0
                    && (kind.is_naval() || kind.fuel_capacity() > 0)
                {
                    0.0
                } else {
                    let mut desire = kill_desire(
                        benefit,
                        power,
                        kind.stats().cost + helper_cost,
                        vulnerability,
                        victims,
                    );
                    if let Some(enemy) = enemy_city {
                        let existing = helper * helper;
                        if benefit * existing as f32 > (helper_cost * vulnerability) as f32 {
                            desire -=
                                kill_desire(benefit, existing, helper_cost, vulnerability, victims);
                        }
                        let (attacks, occupied) = self.invasion_support(city.owner, enemy, 0);
                        if kind.can_occupy_city() && attacks > 0 && occupied == 0 {
                            desire = desire.max(
                                (self.offensive_city_worth(enemy)
                                    * 10.0
                                    * f64::from(self.offensive_finishing_factor(enemy)))
                                    as f32,
                            );
                        }
                    }
                    desire
                };
                desire -= (tech_cost * 17) as f32;
                desire -= (turns * 17) as f32;
                let cost = Self::offensive_balanced_cost(kind)
                    + if target.new_ferry {
                        target.ferry.as_ref().unwrap().kind.stats().cost
                    } else {
                        0
                    };
                let want = self.offensive_amortize(city, desire, turns.max(1), cost);
                if want > 0.0 {
                    if !missing.is_empty() {
                        if let Some(tech) = tech {
                            research.push((tech, want));
                        }
                    } else if f64::from(want) > selected.want
                        && self.can_produce(city, Production::Unit(kind))
                    {
                        selected = AiBuildChoice {
                            production: Production::Unit(kind),
                            want: f64::from(want),
                        };
                    }
                }
            }
            if selected.want > choice.map_or(0.0, |old| old.want) {
                if target.new_ferry {
                    // Selection already happened. The source's int role
                    // callback may lower this below the discarded old bid.
                    let want = selected.want as i32;
                    if let Some(kind) = self.offensive_ferry_role(city, want, &mut research) {
                        selected.production = Production::Unit(kind);
                        selected.want = f64::from(want);
                    }
                }
                choice = Some(selected);
                replaced = true;
            }
        }
        (choice, research, replaced)
    }
}

#[cfg(test)]
#[path = "offense/tests.rs"]
mod tests;
