//! Default AI offensive production and its research side effects.
//! daimilitary.c:kill_something_with/process_attacker_want; target selection
//! follows daiunit.c:find_something_to_kill (Normal's H_TARGETS policy).
use super::advisors::AiBuildChoice;
use super::normal_paths::NormalPaths;
use super::unit_catalog::{UnitFlag, UnitRole};
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
        let cities: Vec<_> = self.player_cities_ordered(city.owner).collect();
        let average = cities
            .iter()
            .map(|c| self.city_forecast(c).shield_surplus())
            .sum::<i32>()
            / cities.len().max(1) as i32;
        let output = self
            .city_forecast(city)
            .shield_surplus()
            .max(average)
            .max(1);
        (f64::from(value) * (23.0_f64 / 24.0).powi(delay + cost / output)) as f32
    }

    fn offensive_seed(&self, city: &City, sea: bool) -> Option<UnitKind> {
        let mut best = None;
        let mut best_want = -1.0_f32;
        for kind in UnitKind::SIMPLE_MILITARY {
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
            defender.kind.stats().hp
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
            let nearby = i32::from(self.map_distance(helper.x, helper.y, city.x, city.y) <= 1);
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
        kind.obsolete_by().iter().any(|&successor| {
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
                .neighbors(x, y)
                .any(|(x, y)| self.pf_native_tile(kind, x, y))
    }

    fn offensive_can_attack(&self, kind: UnitKind, x: i32, y: i32) -> bool {
        kind.stats().attack > 0
            && (self.pf_native_tile(kind, x, y)
                || kind.is_naval() && !matches!(kind, UnitKind::Trireme | UnitKind::Submarine))
    }

    /// daiunit.c:find_beachhead; strict lower movement cost retains the
    /// first native adjacent-tile candidate, including overlap land endpoints.
    fn offensive_beachhead(
        &self,
        map: &mut NormalPaths,
        cargo: UnitKind,
        owner: usize,
        x: i32,
        y: i32,
    ) -> Option<(i32, i32, bool)> {
        let mut best = None;
        let mut best_cost = i32::MAX;
        if cargo.has_marine_attack() {
            for (x, y) in self.map_wrap.neighbors(x, y) {
                if let Some(cost) = map.movement_cost(self, x, y)
                    && cost < best_cost
                {
                    best_cost = cost;
                    best = Some((x, y, true));
                }
            }
        } else {
            let mut checked = std::collections::HashSet::from([(x, y)]);
            for (bx, by) in self.map_wrap.neighbors(x, y) {
                if !self.pf_native_tile(cargo, bx, by) {
                    continue;
                }
                for (sx, sy) in self.map_wrap.neighbors(bx, by) {
                    if !checked.insert((sx, sy))
                        || self
                            .tile_units_ordered(sx, sy)
                            .any(|u| !self.allied(owner, u.owner))
                    {
                        continue;
                    }
                    if let Some(cost) = map.movement_cost(self, sx, sy)
                        && cost < best_cost
                    {
                        best_cost = cost;
                        best = Some((sx, sy, (sx, sy) != (bx, by)));
                    }
                }
            }
        }
        best
    }

    fn offensive_find_ferry(&self, passenger: &Unit) -> Option<Unit> {
        // aiferry_find_boat's combined PF and sea_move EC. The source
        // intentionally cannot cross sea back onto a second continent.
        let mut map = NormalPaths::ferry_search_map(self, passenger);
        let mut best_turns = i32::MAX;
        let mut best = None;
        for (x, y, turn, extra) in map.positions(self) {
            if turn + (extra / 65536) as i32 > best_turns {
                break;
            }
            let tiles: Vec<_> = if self.tile(x, y).terrain.is_water() {
                std::iter::once((x, y))
                    .chain(self.map_wrap.neighbors(x, y))
                    .collect()
            } else {
                vec![(x, y)]
            };
            for (x, y) in tiles {
                for boat in self.tile_units_ordered(x, y) {
                    if !self.ferry_can_take_unit(boat, passenger)
                        || self.cargo_count(boat.id) >= boat.kind.transport_capacity()
                        || boat.kind.fuel_capacity() > 0
                        || boat.kind.hp_loss_percent() > 0
                    {
                        continue;
                    }
                    let ferry_turns = (((extra / 65536) as i32 * 16) - boat.moves_left)
                        / (boat.kind.stats().moves * MOVE_FRAGMENTS);
                    let turns = turn.max(ferry_turns);
                    if turns < best_turns {
                        best_turns = turns;
                        best = Some(boat.clone());
                    }
                }
            }
        }
        best
    }

    fn offensive_target(&self, city: &City, seed: &Unit) -> Option<OffensiveTarget> {
        let mut map = NormalPaths::attack_map(self, seed);
        let mut best: Option<OffensiveTarget> = None;
        let mut fallback = 0.0_f32;
        let rating = Self::offensive_attack_rating(seed);
        let unhap = self.offensive_military_unhappiness(city);
        let ferry = if !seed.kind.is_naval() {
            self.offensive_find_ferry(seed).or_else(|| {
                UnitKind::ALL_NATIVE
                    .into_iter()
                    .rev()
                    .find(|kind| {
                        kind.has_role(UnitRole::Ferryboat) && self.unit_available(seed.owner, *kind)
                    })
                    .or_else(|| {
                        UnitKind::ALL_NATIVE
                            .into_iter()
                            .find(|kind| kind.has_role(UnitRole::Ferryboat))
                    })
                    .filter(|_| {
                        self.map_wrap
                            .neighbors(seed.x, seed.y)
                            .any(|(x, y)| self.tile(x, y).terrain.is_water())
                    })
                    .map(|kind| self.make_unit(0, seed.owner, kind, seed.x, seed.y))
            })
        } else {
            None
        };
        let mut ferry_map = ferry
            .as_ref()
            .map(|ferry| NormalPaths::overlap_map(self, ferry));
        for owner in 0..self.players.len() {
            if !self.ai_potentially_hostile(seed.owner, owner) {
                continue;
            }
            for enemy in self.player_cities_ordered(owner) {
                if !self.tile(enemy.x, enemy.y).known_by[seed.owner]
                    || !self.offensive_can_attack(seed.kind, enemy.x, enemy.y)
                {
                    continue;
                }
                let mut by_boat = false;
                let turns = if let Some(turns) = map.position_turn(self, enemy.x, enemy.y) {
                    turns
                } else if let Some(fmap) = ferry_map.as_mut() {
                    let Some((x, y, land)) =
                        self.offensive_beachhead(fmap, seed.kind, seed.owner, enemy.x, enemy.y)
                    else {
                        continue;
                    };
                    by_boat = true;
                    let mut turns = fmap.position_turn(self, x, y).unwrap() + i32::from(land);
                    if let Some(boat) = ferry.as_ref()
                        && boat.id != 0
                        && (boat.x, boat.y) != (seed.x, seed.y)
                    {
                        let Some(walk) = map.position_turn(self, boat.x, boat.y) else {
                            continue;
                        };
                        turns += walk;
                    }
                    turns
                } else {
                    continue;
                };
                let defender = self.anticipated_city_defender(enemy, seed, turns, true);
                let vulnerability = defender.as_ref().map_or(0, |d| {
                    self.offensive_defense_squared(d, seed.kind, enemy.x, enemy.y, true)
                });
                let mut benefit = defender
                    .as_ref()
                    .map_or(0.0, |d| d.kind.stats().cost as f32);
                let (helper, helper_cost) = self.offensive_helpers(seed, enemy);
                let (invaders, occupiers) = self.invasion_support(seed.owner, enemy, 0);
                let victims = self.tile_units_ordered(enemy.x, enemy.y).count() as i32;
                let reserves = invaders - victims
                    + if seed.kind.has_flag(UnitFlag::OneAttack) {
                        1
                    } else {
                        seed.kind.stats().moves * MOVE_FRAGMENTS
                    };
                if reserves > 0 && (seed.kind.can_occupy_city() || occupiers > 0) {
                    benefit += (self.advisor_city_output(enemy, None) * reserves / 5) as f32;
                }
                let power = (rating + helper).pow(2);
                let mut want = if !seed.kind.can_occupy_city() && defender.is_none() || turns > 10 {
                    0.0
                } else if seed.kind.can_occupy_city()
                    && occupiers == 0
                    && (invaders > 0 || victims == 0)
                {
                    (seed.kind.stats().cost * 17) as f32
                } else {
                    kill_desire(
                        benefit,
                        power,
                        seed.kind.stats().cost + helper_cost,
                        vulnerability,
                        victims + 1,
                    )
                };
                want -= (turns * if unhap { 53 } else { 17 }) as f32;
                let new_ferry = by_boat && ferry.as_ref().is_some_and(|f| f.id == 0);
                let cost = Self::offensive_balanced_cost(seed.kind)
                    + if new_ferry {
                        ferry.as_ref().unwrap().kind.stats().cost
                    } else {
                        0
                    };
                want = self.offensive_amortize(city, want, turns.max(1), cost);
                let choose_fallback =
                    want <= 0.0 && best.as_ref().is_none_or(|old| old.want <= 0.0) && {
                        let bk = self.offensive_amortize(city, benefit * 17.0, turns.max(1), cost);
                        if bk > fallback {
                            fallback = bk;
                            true
                        } else {
                            false
                        }
                    };
                if choose_fallback || want > best.as_ref().map_or(0.0, |old| old.want) {
                    best = Some(OffensiveTarget {
                        x: enemy.x,
                        y: enemy.y,
                        city: Some(enemy.id),
                        turns,
                        want,
                        ferry: by_boat.then(|| ferry.clone().unwrap()),
                        new_ferry,
                    });
                }
            }
            for enemy in self.player_units_ordered(owner) {
                if !self.tile(enemy.x, enemy.y).known_by[seed.owner]
                    || self.cities.iter().any(|c| (c.x, c.y) == (enemy.x, enemy.y))
                    || !Self::offense_type_hostile(enemy.kind)
                    || !self.offensive_can_attack(seed.kind, enemy.x, enemy.y)
                    || self.best_defender_id(seed, enemy.x, enemy.y) != Some(enemy.id)
                {
                    continue;
                }
                let Some(turns) = map.position_turn(self, enemy.x, enemy.y) else {
                    continue;
                };
                let vulnerability =
                    self.offensive_defense_squared(enemy, seed.kind, enemy.x, enemy.y, true);
                let mut want = if turns > 10 {
                    0.0
                } else {
                    kill_desire(
                        enemy.kind.stats().cost as f32,
                        rating * rating,
                        seed.kind.stats().cost,
                        vulnerability,
                        1,
                    ) - (turns * if unhap { 53 } else { 17 }) as f32
                };
                want = self.offensive_amortize(
                    city,
                    want,
                    turns.max(1),
                    Self::offensive_balanced_cost(seed.kind),
                );
                if want > best.as_ref().map_or(0.0, |old| old.want) {
                    best = Some(OffensiveTarget {
                        x: enemy.x,
                        y: enemy.y,
                        city: None,
                        turns,
                        want,
                        ferry: None,
                        new_ferry: false,
                    });
                }
            }
        }
        best
    }

    fn offense_type_hostile(kind: UnitKind) -> bool {
        !kind.has_flag(UnitFlag::NonMil)
            || kind.has_flag(UnitFlag::Diplomat)
            || kind.has_flag(UnitFlag::Spy)
            || kind.has_flag(UnitFlag::Capturer)
    }

    fn offensive_victim_count(&self, x: i32, y: i32) -> i32 {
        if self.stack_vulnerable(x, y) {
            1
        } else {
            1 + self.tile_units_ordered(x, y).count() as i32
        }
    }

    fn offensive_victim(&self, seed: &Unit, target: &OffensiveTarget) -> (Option<Unit>, f32) {
        if let Some(city) = target
            .city
            .and_then(|id| self.cities.iter().find(|city| city.id == id))
        {
            // kill_something_with deliberately retains def_type on the
            // <=1-turn branch while clearing benefit and def_vet. The later
            // type loop therefore still rates an unbuilt rank-zero defender.
            let mut defender = self.defender_versus(city, seed);
            if target.turns <= 1 {
                if let Some(unit) = defender.as_mut() {
                    unit.veteran = 0;
                }
            }
            let vulnerability = if target.turns > 1 {
                defender.as_ref().map_or(0, |d| {
                    self.offensive_defense_squared(d, seed.kind, target.x, target.y, true)
                })
            } else {
                0
            };
            let mut benefit = if target.turns > 1 {
                defender
                    .as_ref()
                    .map_or(0.0, |d| d.kind.stats().cost as f32)
            } else {
                0.0
            };
            if let Some(current) = self
                .best_defender_id(seed, target.x, target.y)
                .and_then(|id| self.units.iter().find(|u| u.id == id))
            {
                let mut current = current.clone();
                current.hp = current.kind.stats().hp;
                current.state = UnitState::Idle;
                let power =
                    self.offensive_defense_squared(&current, seed.kind, target.x, target.y, true);
                if power > vulnerability {
                    benefit = current.kind.stats().cost as f32;
                    defender = Some(current);
                }
            }
            (defender, benefit)
        } else {
            let defender = self
                .best_defender_id(seed, target.x, target.y)
                .and_then(|id| self.units.iter().find(|u| u.id == id))
                .map(|unit| {
                    let mut unit = unit.clone();
                    unit.hp = unit.kind.stats().hp;
                    unit.state = UnitState::Idle;
                    unit
                });
            let benefit = defender
                .as_ref()
                .map_or(0.0, |d| d.kind.stats().cost as f32);
            (defender, benefit)
        }
    }

    fn offensive_candidate_turns(&self, candidate: &Unit, target: &OffensiveTarget) -> Option<i32> {
        let mut pf = NormalPaths::enemy_map(self, candidate);
        let mut turns = None;
        if let Some(boat) = target.ferry.as_ref() {
            let mut fmap = NormalPaths::overlap_map(self, boat);
            if let Some((x, y, _)) = self.offensive_beachhead(
                &mut fmap,
                candidate.kind,
                candidate.owner,
                target.x,
                target.y,
            ) {
                turns = fmap.position_turn(self, x, y);
                let first = pf.positions(self).into_iter().find(|(x, y, _, _)| {
                    (*x, *y) == (boat.x, boat.y) || (*x, *y) == (target.x, target.y)
                });
                if let Some((x, y, travel, _)) = first {
                    if (x, y) == (target.x, target.y) {
                        turns = Some(travel);
                    } else {
                        turns = turns.map(|sea| sea + travel);
                    }
                }
            }
        }
        turns.or_else(|| pf.position_turn(self, target.x, target.y))
    }

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
            .filter(|kind| kind.has_role(UnitRole::Ferryboat))
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
        let (choice, research, replaced) = self.offensive_advice_with_prior(city, prior);
        (if replaced { choice } else { None }, research)
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
            let Some(target) = self.offensive_target(city, &seed) else {
                continue;
            };
            let enemy_city = target
                .city
                .and_then(|id| self.cities.iter().find(|c| c.id == id));
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
            for kind in UnitKind::SIMPLE_MILITARY {
                if kind.stats().attack <= 0
                    || !Self::guard_can_follow(kind, seed_kind)
                    || !self.offensive_native_near(kind, target.x, target.y)
                    || self.offensive_obsolete(city, kind)
                {
                    continue;
                }
                let mut candidate = self.make_unit(0, city.owner, kind, city.x, city.y);
                candidate.veteran = self.trained_rank(city, kind);
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
                desire -= (turns
                    * if self.offensive_military_unhappiness(city) {
                        53
                    } else {
                        17
                    }) as f32;
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

impl Simulation {
    pub(crate) fn offensive_research_playtest(&self) -> (bool, bool, bool, bool) {
        let mut game = self.clone();
        game.units.clear();
        game.cities.clear();
        game.diplomacy.clear();
        game.ai_ground_targets.clear();
        game.ai_guard_targets.clear();
        game.ai_unit_guards.clear();
        game.unit_goto_tiles.clear();
        game.ai_ferries = Default::default();
        game.players[1].known = vec![Tech::BRONZE_WORKING, Tech::THE_WHEEL];
        game.players[1].government = Government::Despotism;
        game.players[1].rates = [40, 60, 0];
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.road = false;
            tile.railroad = false;
            tile.river = false;
            tile.mine = false;
            tile.irrigation = false;
            tile.resource = None;
            tile.base = None;
            tile.hut = false;
            tile.owner = None;
            tile.known_by.fill(true);
        }
        let home = (10, 24);
        let enemy = offset_native_by_map(home.0, home.1, 3, 0);
        game.create_city(1, home.0, home.1);
        game.create_city(HUMAN, enemy.0, enemy.1);
        game.cities[1].size = 8;
        game.cities[0].buildings.push(Building::BARRACKS);
        game.spawn_unit(1, UnitKind::Phalanx, home.0, home.1);
        game.spawn_unit(HUMAN, UnitKind::Warrior, enemy.0, enemy.1);
        game.sign_pact(1, HUMAN, Pact::War);
        let city = game.cities[0].clone();
        let (choice, feedback) = game.offensive_advice(&city);
        let iron = feedback
            .iter()
            .filter(|(tech, _)| *tech == Tech::IRON_WORKING)
            .map(|(_, want)| *want)
            .sum::<f32>();
        let future = iron > 0.0;
        let current=choice.is_some_and(|choice|matches!(choice.production,Production::Unit(kind) if game.can_produce(&city,Production::Unit(kind))));
        game.tile_mut(enemy.0, enemy.1).known_by[1] = false;
        let unknown = game.offensive_advice(&city);
        let hidden = unknown.0.is_none() && unknown.1.is_empty();
        game.tile_mut(enemy.0, enemy.1).known_by[1] = true;
        game.players[1].known.push(Tech::IRON_WORKING);
        let learned = game.offensive_advice(&city);
        let known = learned
            .1
            .iter()
            .all(|(tech, _)| *tech != Tech::IRON_WORKING);
        (future, current, hidden, known)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn island_fixture() -> (Simulation, City, Unit, (i32, i32)) {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        game.prune_unit_ai_state();
        game.players[1].known = vec![Tech::WARRIOR_CODE];
        game.players[1].government = Government::Despotism;
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Ocean;
            tile.known_by.fill(true);
            tile.owner = None;
            tile.hut = false;
            tile.resource = None;
            tile.river = false;
            tile.base = None;
        }
        let home = (10, 24);
        let enemy = offset_native_by_map(home.0, home.1, 8, 0);
        let beach = offset_native_by_map(enemy.0, enemy.1, -1, 0);
        for (x, y) in [home, enemy, beach] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
        }
        game.create_city(1, home.0, home.1);
        game.create_city(HUMAN, enemy.0, enemy.1);
        game.sign_pact(1, HUMAN, Pact::War);
        let city = game.cities[0].clone();
        game.spawn_unit(1, UnitKind::Warrior, home.0, home.1);
        let mut seed = game.make_unit(0, 1, UnitKind::Archer, home.0, home.1);
        seed.home_city = Some(city.id);
        seed.veteran = 1;
        (game, city, seed, enemy)
    }

    #[test]
    fn bundled_bases_do_not_get_unconfigured_stack_protection_in_offensive_advice() {
        let (mut game, _, seed, _) = island_fixture();
        let tile = offset_native_by_map(seed.x, seed.y, 1, 0);
        game.tile_mut(tile.0, tile.1).terrain = Terrain::Grassland;
        game.spawn_unit(HUMAN, UnitKind::Warrior, tile.0, tile.1);
        game.spawn_unit(HUMAN, UnitKind::Warrior, tile.0, tile.1);
        for base in [Base::Fortress, Base::Airbase] {
            game.tile_mut(tile.0, tile.1).base = Some(base);
            assert_eq!(game.offensive_victim_count(tile.0, tile.1), 1);
        }
        game.create_city(HUMAN, tile.0, tile.1);
        assert_eq!(game.offensive_victim_count(tile.0, tile.1), 3);
    }

    #[test]
    fn immediate_successor_direct_requirements_exclude_old_types_even_if_successor_is_obsolete() {
        let (mut game, city, _, _) = island_fixture();
        game.players[1].known.extend([
            Tech::IRON_WORKING,
            Tech::GUNPOWDER,
            UnitKind::Riflemen.required_tech().unwrap(),
        ]);
        assert!(!game.unit_buildable(&city, UnitKind::Musketeers));
        assert!(game.unit_available(1, UnitKind::Musketeers));
        assert!(game.offensive_obsolete(&city, UnitKind::Legion));
    }

    #[test]
    fn nearby_empty_city_retains_unbuilt_rank_zero_defender_type_with_zero_unit_benefit() {
        let (mut game, _, seed, enemy) = island_fixture();
        game.players[HUMAN].known.push(Tech::BRONZE_WORKING);
        let target = OffensiveTarget {
            x: enemy.0,
            y: enemy.1,
            city: Some(game.cities[1].id),
            turns: 1,
            want: 1.0,
            ferry: None,
            new_ferry: false,
        };
        let (defender, benefit) = game.offensive_victim(&seed, &target);
        let defender = defender.expect("native retains def_type despite arrival<=1");
        assert_eq!(defender.kind, UnitKind::Phalanx);
        assert_eq!(defender.veteran, 0);
        assert_eq!(benefit, 0.0);
        assert!(game.offensive_defense_squared(&defender, seed.kind, enemy.0, enemy.1, true) > 0);
        let current = game.spawn_unit(HUMAN, UnitKind::Marines, enemy.0, enemy.1);
        let marine = game.units.iter_mut().find(|u| u.id == current).unwrap();
        marine.hp = 1;
        marine.state = UnitState::Fortified;
        let (defender, benefit) = game.offensive_victim(&seed, &target);
        let defender = defender.unwrap();
        assert_eq!(defender.kind, UnitKind::Marines);
        assert_eq!(defender.hp, UnitKind::Marines.stats().hp);
        assert_eq!(defender.state, UnitState::Idle);
        assert_eq!(benefit, UnitKind::Marines.stats().cost as f32);
    }

    #[test]
    fn fractional_ferry_override_survives_integer_conversion_below_the_discarded_prior_bid() {
        let (mut game, city, _, _) = island_fixture();
        // With no buildable boat the very same virtual Trireme target keeps
        // its unrounded attacker want. Learning Map Making changes only the
        // role callback's ability to substitute that boat in this fixture.
        let raw = game
            .offensive_advice(&city)
            .0
            .expect("real offensive sea job");
        assert_eq!(raw.production, Production::Unit(UnitKind::Archer));
        let fraction = raw.want.fract();
        assert!(fraction > 0.0 && fraction < 1.0);
        game.players[1].known.push(Tech::MAP_MAKING);
        assert!(game.can_produce(&city, Production::Unit(UnitKind::Trireme)));
        let prior = AiBuildChoice {
            production: Production::Unit(UnitKind::Archer),
            want: raw.want.floor() + fraction / 2.0,
        };
        assert!(raw.want > prior.want && raw.want - prior.want < 1.0);
        let (final_choice, _, replaced) = game.offensive_advice_with_prior(&city, Some(prior));
        let final_choice = final_choice.unwrap();
        assert!(replaced);
        assert_eq!(final_choice.production, Production::Unit(UnitKind::Trireme));
        assert_eq!(final_choice.want, raw.want.floor());
        assert!(
            final_choice.want < prior.want,
            "native already discarded prior before int conversion"
        );
        // A genuinely higher pre-conversion prior keeps its own provenance.
        let stronger = AiBuildChoice {
            want: raw.want + 0.25,
            ..prior
        };
        let (kept, _, replaced) = game.offensive_advice_with_prior(&city, Some(stronger));
        assert!(!replaced);
        assert_eq!(kept.unwrap().production, stronger.production);
        assert_eq!(kept.unwrap().want, stronger.want);
    }

    #[test]
    fn virtual_ferry_stimulates_its_missing_technology_after_a_real_offensive_target() {
        let (game, city, seed, _) = island_fixture();
        assert!(game.offensive_find_ferry(&seed).is_none());
        let target = game
            .offensive_target(&city, &seed)
            .expect("virtual boat finds beachhead");
        assert!(target.new_ferry);
        assert_eq!(target.ferry.as_ref().unwrap().kind, UnitKind::Trireme);
        let (choice, research) = game.offensive_advice(&city);
        assert!(choice.is_some());
        assert!(
            research
                .iter()
                .any(|(tech, want)| *tech == Tech::MAP_MAKING && *want > 0.0)
        );
    }

    #[test]
    fn combined_ferry_search_finds_remote_boat_but_seed_rejects_an_unreachable_rendezvous() {
        let (mut game, city, seed, enemy) = island_fixture();
        let sea = offset_native_by_map(seed.x, seed.y, 4, 0);
        let boat_id = game.spawn_unit(1, UnitKind::Galleon, sea.0, sea.1);
        let boat = game
            .offensive_find_ferry(&seed)
            .expect("sea EC search reaches remote ferry");
        assert_eq!(boat.id, boat_id);
        let mut walk = NormalPaths::attack_map(&game, &seed);
        assert!(walk.position_turn(&game, boat.x, boat.y).is_none());
        assert!(
            game.offensive_target(&city, &seed).is_none(),
            "seed cannot board remote ship"
        );
        // Literal process_attacker_want retains its initialized sailing time
        // when its later per-type PF reaches neither boat nor target. The
        // stricter seed stage above prevents this from creating a seed job.
        let target = OffensiveTarget {
            x: enemy.0,
            y: enemy.1,
            city: Some(game.cities[1].id),
            turns: 0,
            want: 1.0,
            ferry: Some(boat.clone()),
            new_ferry: false,
        };
        let mut fmap = NormalPaths::overlap_map(&game, &boat);
        let (x, y, _) = game
            .offensive_beachhead(&mut fmap, seed.kind, seed.owner, enemy.0, enemy.1)
            .unwrap();
        assert_eq!(
            game.offensive_candidate_turns(&seed, &target),
            fmap.position_turn(&game, x, y)
        );
    }

    #[test]
    fn literal_c_kill_desire_and_amortization_vectors_preserve_float_assignments() {
        // Literal extracted native expressions compiled as C (float adv_want,
        // double pow, MORT24), not values calculated by the Rust routine.
        assert_eq!(kill_desire(80.25, 1600, 40, 625, 3).to_bits(), 1153282254);
        assert_eq!(kill_desire(30.0, 400, 20, 225, 1).to_bits(), 1129054208);
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(1, 10, 24);
        let city = game.cities.last().unwrap();
        assert_eq!(
            game.offensive_amortize(city, 1234.5, 7, 0).to_bits(),
            1147477144
        );
    }
    #[test]
    fn future_offensive_types_emit_direct_wants_while_normal_known_target_gate_is_shared() {
        let game = Simulation::new(crate::mapgen::generate(42));
        assert_eq!(game.offensive_research_playtest(), (true, true, true, true));
    }
}
