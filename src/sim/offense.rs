//! Default AI offensive production and its research side effects.
//! daimilitary.c:kill_something_with/process_attacker_want; target selection
//! follows daiunit.c:find_something_to_kill (Normal's H_TARGETS policy).
use super::advisors::AiBuildChoice;

mod targeting;
use super::normal_paths::NormalPaths;
use super::unit_catalog::{UnitAiStrategy, UnitFlag};
use super::*;

#[derive(Clone)]
pub(in crate::sim) struct OffensiveTarget {
    pub(in crate::sim) x: i32,
    pub(in crate::sim) y: i32,
    pub(in crate::sim) city: Option<u32>,
    pub(in crate::sim) path: Vec<(i32, i32)>,
    turns: i32,
    want: f32,
    pub(in crate::sim) ferry: Option<Unit>,
    new_ferry: bool,
}

struct OffensivePlan {
    target: OffensiveTarget,
    ferry_map: Option<NormalPaths>,
    invasion: (i32, i32),
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
        Self::amortize_attack(value, delay, cost, self.cached_offensive_output(city))
    }

    fn amortize_attack(value: f32, delay: i32, cost: i32, output: i32) -> f32 {
        (f64::from(value.max(0.0)) * (23.0_f64 / 24.0).powi(delay + cost / output)) as f32
    }

    fn cached_offensive_output(&self, city: &City) -> i32 {
        if self.advisor_cache.active() {
            self.advisor_cache.memos().offensive_output(city.id)
        } else {
            None
        }
        .unwrap_or_else(|| {
            let output = self.offensive_production_output(city);
            if self.advisor_cache.active() {
                self.advisor_cache
                    .memos()
                    .remember_offensive_output(city.id, output);
            }
            output
        })
    }

    fn offensive_production_output(&self, city: &City) -> i32 {
        let cached = self.advisor_cache.memos().average_production(city.owner);
        let average = cached.unwrap_or_else(|| {
            let mut count = 0;
            let mut total = 0;
            for other in self.cities_owned_by(city.owner) {
                count += 1;
                total += self.city_forecast(other).shield_surplus();
            }
            let average = total / count.max(1);
            if self.advisor_cache.active() {
                self.advisor_cache
                    .memos()
                    .remember_average_production(city.owner, average);
            }
            average
        });
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
            want *= (s.moves * campaign_move_fragments()) as f32;
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

    pub(in crate::sim) fn assigned_attack_strength(
        &self,
        unit: &Unit,
    ) -> std::collections::HashMap<u32, (i32, i32)> {
        let mut totals = std::collections::HashMap::new();
        for (&id, &target) in &self.ai_ground_targets {
            if id == unit.id && unit.id != 0 {
                continue;
            }
            let Some(helper) = self.unit_by_id(id).filter(|u| u.owner == unit.owner) else {
                continue;
            };
            let rating = Self::offensive_attack_rating(helper);
            if rating != 0 {
                let entry = totals.entry(target).or_insert((0, 0));
                entry.0 += rating;
                entry.1 += helper.kind.stats().cost;
            }
        }
        totals
    }

    pub(in crate::sim) fn offensive_helpers(
        &self,
        unit: &Unit,
        city: &City,
        assigned: &std::collections::HashMap<u32, (i32, i32)>,
    ) -> (i32, i32) {
        let (mut attack, mut cost) = assigned.get(&city.id).copied().unwrap_or_default();
        let mut nearby_ids = std::collections::HashSet::new();
        // Freeciv's reinforcements_cost_and_value checks the local radius
        // around a target. Query those tiles directly instead of scanning the
        // entire unit roster for every target city.
        for (x, y, _) in self.map_wrap.tiles_within_steps(city.x, city.y, 1) {
            for helper in self.tile_units_ordered(x, y) {
                if helper.id == unit.id && unit.id != 0
                    || !self.allied(unit.owner, helper.owner)
                    || !nearby_ids.insert(helper.id)
                {
                    continue;
                }
                let rating = Self::offensive_attack_rating(helper);
                if rating == 0 {
                    continue;
                }
                attack += rating;
                cost += helper.kind.stats().cost;
            }
        }
        (attack, cost)
    }

    fn offensive_city_worth(&self, city: &City) -> f64 {
        0.9 * f64::from(self.cached_advisor_city_output(city)) + (f64::from(city.size) - 0.5) * 10.0
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
                && (successor.unit_class().buildable_in_any_city()
                    || successor.is_aircraft()
                    || self.naval_city_access(city, successor))
        })
    }

    fn offensive_native_near(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        self.pf_native_tile(kind, x, y)
            || self
                .map_wrap
                .eight_way_neighbors(x, y)
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
        for kind in UnitKind::all()
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
                let path = self.shared_research_path(city.owner, tech);
                if path.is_empty() {
                    continue;
                }
                let cost = path
                    .iter()
                    .copied()
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
        self.offensive_advice_for_wonder_city(city, self.ai_wonder_city(city.owner))
    }

    pub(in crate::sim) fn offensive_advice_for_wonder_city(
        &self,
        city: &City,
        wonder_city: Option<u32>,
    ) -> (Option<AiBuildChoice>, Vec<(Tech, f32)>) {
        let (prior, early, severe) =
            self.pre_offensive_military_advice_with_wonder_city(city, wonder_city);
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
        let cache_enabled = self.advisor_cache.active();
        if cache_enabled && let Some(result) = self.advisor_cache.memos().offensive_advice(city.id)
        {
            self.trace.count_operation("offensive_advice.cache_hit", 1);
            return result;
        }
        if cache_enabled {
            self.trace.count_operation("offensive_advice.cache_miss", 1);
        }
        let result = self.offensive_advice_with_prior(city, prior);
        if self.advisor_cache.active() {
            self.advisor_cache
                .memos()
                .remember_offensive_advice(city.id, result.clone());
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
        let profiler = self.trace.profiler();
        let _profile = profiler.scope("offense.candidate_evaluation");
        let mut research = Vec::new();
        let mut choice = prior;
        let mut replaced = false;
        let assessment = self.advisor_guard_city_assessment(city);
        if assessment.danger != 0 && assessment.defense == 0 {
            return (choice, research, replaced);
        }
        let connectivity = self.movement_connectivity(city.owner);
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
            let assigned = self.assigned_attack_strength(&seed);
            let Some(OffensivePlan {
                target,
                ferry_map: mut ferry_path,
                invasion: (enemy_attacks, occupiers),
            }) = self.offensive_target_plan(self.cached_offensive_output(city), &seed, &assigned)
            else {
                continue;
            };
            let enemy_city = target.city.and_then(|id| self.city_by_id(id));
            let (defender, base_benefit) = self.offensive_victim(&seed, &target);
            let (helper, helper_cost) = enemy_city.map_or((0, 0), |enemy| {
                self.offensive_helpers(&seed, enemy, &assigned)
            });
            let enemy_city_worth = enemy_city.map_or(0.0, |enemy| self.offensive_city_worth(enemy));
            let enemy_finishing_factor =
                enemy_city.map_or(0.0, |enemy| self.offensive_finishing_factor(enemy));
            let target_victims = self.offensive_victim_count(target.x, target.y);
            let empire_city_count = self.player_cities_ordered(city.owner).count().max(1) as i32;
            let mut benefit = base_benefit;
            if enemy_city.is_some() && (seed.kind.can_occupy_city() || occupiers > 0) {
                benefit = (f64::from(benefit)
                    + enemy_city_worth * f64::from(enemy_finishing_factor) / 3.0)
                    as f32;
            }
            let mut selected = AiBuildChoice {
                production: Production::Unit(seed_kind),
                want: f64::from(target.want),
            };
            // Deduplicate candidate route searches across unit kinds that share
            // the exact same movement & target-attack profile from this city.
            type CandidateRouteKey = (
                super::unit_catalog::UnitClass,
                i32,
                bool,
                bool,
                bool,
                bool,
                bool,
                bool,
                bool,
                bool,
            );
            let mut route_memo: Vec<(CandidateRouteKey, Option<i32>)> = Vec::new();
            for kind in UnitKind::simple_military() {
                self.trace.count_operation("offense.candidate_units", 1);
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
                let tech = kind.required_tech();
                let missing = tech.map_or_else(Default::default, |tech| {
                    self.shared_research_path(city.owner, tech)
                });
                let tech_cost = missing
                    .iter()
                    .map(|&tech| self.target_cost(city.owner, tech))
                    .sum::<i32>()
                    / 4
                    / empire_city_count;
                let power = (Self::offensive_attack_rating(&candidate) + helper).pow(2);
                if power == 0 {
                    continue;
                }
                let vulnerability = defender.as_ref().map_or(0, |def| {
                    self.offensive_defense_squared(def, kind, target.x, target.y, true)
                });
                let desire = if vulnerability == 0 && (kind.is_naval() || kind.fuel_capacity() > 0)
                {
                    0.0
                } else {
                    let mut desire = kill_desire(
                        benefit,
                        power,
                        kind.stats().cost + helper_cost,
                        vulnerability,
                        target_victims,
                    );
                    if enemy_city.is_some() {
                        let existing = helper * helper;
                        if benefit * existing as f32 > (helper_cost * vulnerability) as f32 {
                            desire -= kill_desire(
                                benefit,
                                existing,
                                helper_cost,
                                vulnerability,
                                target_victims,
                            );
                        }
                        if kind.can_occupy_city() && enemy_attacks > 0 && occupiers == 0 {
                            desire = desire.max(
                                (enemy_city_worth * 10.0 * f64::from(enemy_finishing_factor))
                                    as f32,
                            );
                        }
                    }
                    desire
                };
                let desire_before_route = desire - (tech_cost * 17) as f32;
                let cost = Self::offensive_balanced_cost(kind)
                    + if target.new_ferry {
                        target.ferry.as_ref().unwrap().kind.stats().cost
                    } else {
                        0
                    };
                let best_possible_want =
                    self.offensive_amortize(city, desire_before_route, 1, cost);
                let can_produce = missing.is_empty()
                    && f64::from(best_possible_want) > selected.want
                    && self.can_produce(city, Production::Unit(kind));
                if best_possible_want <= 0.0
                    || missing.is_empty()
                        && (!can_produce || f64::from(best_possible_want) <= selected.want)
                {
                    self.trace
                        .count_operation("offensive.candidate_route.pruned_by_upper_bound", 1);
                    continue;
                }
                let route_key: CandidateRouteKey = (
                    crate::sim::movement_rules::MovementRules::campaign_unit_class(kind),
                    self.unit_move_rate(&candidate),
                    super::action_rules::unit_type_flag(kind, "IgTer")
                        .expect("campaign C2C3 unit-type IgTer binding"),
                    Simulation::unit_ignores_zoc(kind),
                    super::action_rules::unit_type_flag(kind, "CoastStrict")
                        .expect("campaign C2C3 CoastStrict unit flag"),
                    kind.has_marine_attack(),
                    kind.is_naval(),
                    kind.has_flag(UnitFlag::OneAttack),
                    kind.is_naval()
                        && !matches!(kind, UnitKind::Galley | UnitKind::Submarine)
                        && kind.attack_strength() > 0,
                    kind.can_attack_aircraft(),
                );
                let turns = if let Some(&(_, cached_turns)) =
                    route_memo.iter().find(|(key, _)| *key == route_key)
                {
                    cached_turns
                } else {
                    let computed = self.offensive_candidate_turns_with_connectivity(
                        &candidate,
                        &target,
                        ferry_path.as_mut(),
                        &connectivity,
                    );
                    route_memo.push((route_key, computed));
                    computed
                };
                let Some(turns) = turns else {
                    continue;
                };
                let desire = desire_before_route - (turns * 17) as f32;
                let want = self.offensive_amortize(city, desire, turns.max(1), cost);
                if want > 0.0 {
                    if !missing.is_empty() {
                        if let Some(tech) = tech {
                            research.push((tech, want));
                        }
                    } else if f64::from(want) > selected.want && can_produce {
                        selected = AiBuildChoice {
                            production: Production::Unit(kind),
                            want: f64::from(want),
                        };
                    }
                }
            }
            if self.advisor_cache.active()
                && let (Some(boat), Some(fmap)) = (target.ferry.as_ref(), ferry_path.take())
            {
                let key = (
                    boat.owner,
                    boat.kind.index(),
                    boat.x,
                    boat.y,
                    boat.moves_left,
                    boat.hp,
                    boat.veteran,
                );
                self.advisor_cache.memos().return_overlap_map(key, fmap);
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
