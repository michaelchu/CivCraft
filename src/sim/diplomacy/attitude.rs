//! Diplomatic attitude calculation between civilizations.

use super::*;

fn campaign_ai_effect_value(
    context: &impl crate::rules::freeciv::EffectContext,
    kind: &str,
    scope: &str,
) -> i32 {
    let rules = crate::rules::freeciv::RulesetData::campaign_default();
    i32::try_from(
        crate::rules::freeciv::effect_value(&rules.effects, kind, context).unwrap_or_else(
            |error| panic!("could not evaluate C2C3 {kind} for AI {scope}: {error:?}"),
        ),
    )
    .unwrap_or_else(|_| panic!("C2C3 {kind} exceeds AI {scope} range"))
}

impl Simulation {
    pub(in crate::sim) fn refresh_diplomatic_intelligence(&mut self) {
        let mut cache = std::collections::BTreeMap::new();
        for owner in 0..self.players.len() {
            if owner == BARBARIANS {
                continue;
            }
            for other in 0..self.players.len() {
                if other == owner || other == BARBARIANS {
                    continue;
                }
                let mut intel = DiplomaticIntel::default();
                for third in 0..self.players.len() {
                    if third == owner || third == other || !self.player_alive(third) {
                        continue;
                    }
                    if self.allied(other, third) && self.diplomatic_at_war(owner, third) {
                        intel.allied_with_enemy = Some(third);
                    }
                    if self.allied(owner, third) && self.diplomatic_at_war(other, third) {
                        intel.at_war_with_ally = Some(third);
                    }
                    if self.allied(owner, third) && self.allied(other, third) {
                        intel.allied_with_ally = Some(third);
                    }
                }
                cache.insert((owner, other), intel);
            }
        }
        self.ai_diplomatic_intel = cache;
    }

    pub(super) fn diplomatic_intel(&self, owner: usize, other: usize) -> DiplomaticIntel {
        self.ai_diplomatic_intel
            .get(&(owner, other))
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn diplomatic_at_war(&self, owner: usize, other: usize) -> bool {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_some_and(|relation| relation.first_contact.is_some() && relation.pact == Pact::War)
    }

    pub(super) fn campaign_city_effect(&self, city: &City, kind: &str) -> i32 {
        let context = super::super::action_rules::SimulationRequirementContext::for_city_at(
            self,
            city,
            Some((city.x, city.y)),
        );
        campaign_ai_effect_value(&context, kind, "city value")
    }

    pub(super) fn campaign_player_effect(&self, owner: usize, kind: &str) -> i32 {
        let context = super::super::action_rules::SimulationRequirementContext::for_player_at(
            self, owner, None,
        );
        campaign_ai_effect_value(&context, kind, "player value")
    }

    pub(super) fn ai_war_desire(&self, owner: usize, target: usize) -> i32 {
        // daidiplomacy.c:dai_war_desire. This intentionally uses type power,
        // not current HP, veteran level or battle odds.
        let mut want = 0i32;
        let mut fear = 0i32;
        for city in self.cities_owned_by(target) {
            let output = self.city_forecast(city);
            want += 100 + city.size * 20 + output.shield_surplus() * 8 + output.trade * 6;
            // daidiplomacy.c uses get_city_bonus(EFT_DEFEND_BONUS) without a
            // unit context, matching Freeciv's certain-only effect query.
            fear += self.campaign_city_effect(city, "Defend_Bonus");
            let mut appraisal = city.clone();
            appraisal.owner = owner;
            for &building in &city.buildings {
                let data = building.data();
                want += data.cost;
                if !self.building_is_obsolete(&appraisal, building, None) {
                    want += data.cost
                        * match data.genus {
                            crate::rules::Genus::GreatWonder => 2,
                            crate::rules::Genus::SmallWonder => 1,
                            _ => 0,
                        };
                }
            }
        }
        for unit in self.units_owned_by(target) {
            let stats = unit.kind.stats();
            fear += stats.attack * stats.hp * stats.firepower;
            if stats.can_found {
                want += 100;
            }
        }
        let mut settlers = 0;
        for unit in self.units_owned_by(owner) {
            let stats = unit.kind.stats();
            fear -= stats.attack * stats.hp * stats.firepower / 2;
            if stats.can_found {
                want -= 200;
                settlers += 1;
            }
        }
        let mut cities = 0;
        for city in self.cities_owned_by(owner) {
            if matches!(city.production,Production::Unit(kind) if kind.stats().can_found) {
                want -= 150;
                settlers += 1;
            }
            cities += 1;
        }
        want -= want.abs() / (cities - settlers).max(1);
        fear += self.players[target].gold / 5000
            * self
                .cities
                .iter()
                .filter(|city| city.owner == target)
                .count() as i32;
        let advances = |owner: usize| {
            self.players[owner].known.len() as i32 + self.players[owner].future_techs
        };
        fear += (advances(target) - advances(owner)).max(0) * 100;
        if self.spaceships[target].built.iter().any(|&n| n > 0) {
            want *= 2;
        }
        if self.ai_space_race_leader() == Some(target) {
            return 1_000_000;
        }
        for third in 0..self.players.len() {
            if third == owner || third == BARBARIANS || !self.player_alive(third) {
                continue;
            }
            let excuse = self
                .diplomacy
                .get(&Self::relation_key(owner, third))
                .is_some_and(|relation| {
                    relation.cause_until[usize::from(owner > third)] > self.turn
                });
            if !excuse && (third == target || self.allied(target, third)) {
                let divisor = match self.pact(owner, third) {
                    Pact::Armistice => 10,
                    Pact::Ceasefire => 7,
                    Pact::Peace => 5,
                    Pact::Alliance => 3,
                    _ => 0,
                };
                if divisor != 0 {
                    want -= want.abs() / divisor;
                }
            }
        }
        let love = self
            .diplomacy
            .get(&Self::relation_key(owner, target))
            .map_or(0, |relation| {
                relation.trust[usize::from(owner > target)] / 10
            });
        want -= (want * love / 2000).max(0);
        let mut distances = 0;
        let mut own_cities = 0;
        for city in self.cities_owned_by(owner) {
            if let Some(distance) = self
                .cities
                .iter()
                .filter(|city| city.owner == target)
                .map(|other| self.map_distance(city.x, city.y, other.x, other.y))
                .min()
            {
                distances += distance;
            }
            own_cities += 1;
        }
        let distance = if own_cities == 0 {
            1
        } else {
            (distances / own_cities).max(1)
        };
        want = (f64::from(want) * (23.0f64 / 24.0).powi(distance)) as i32;
        if self.allied(owner, target) {
            want /= 4;
        }
        want - fear
    }

    pub(super) fn advance_ai_love(&mut self) {
        // dai_diplomacy_begin_new_phase runs before diplomatic actions.
        // Native love is integer [-1000,1000]; UI trust is ten times love.
        let pairs: Vec<_> = self.diplomacy.keys().copied().collect();
        let desires: std::collections::BTreeMap<_, _> = (0..self.players.len())
            .map(|owner| {
                let mut best = None;
                for other in 0..self.players.len() {
                    if owner == other
                        || other == BARBARIANS
                        || !self.player_alive(other)
                        || self
                            .diplomacy
                            .get(&Self::relation_key(owner, other))
                            .is_none_or(|relation| relation.first_contact.is_none())
                    {
                        continue;
                    }
                    let desire = self.ai_war_desire(owner, other);
                    if desire > best.map_or(0, |(_, score)| score) {
                        best = Some((other, desire));
                    }
                }
                (owner, best)
            })
            .collect();
        for (a, b) in pairs {
            if !self.player_alive(a) || !self.player_alive(b) {
                continue;
            }
            for (owner, other) in [(a, b), (b, a)] {
                if !self.players[owner].ai || owner == BARBARIANS || other == BARBARIANS {
                    continue;
                }
                let side = usize::from(owner > other);
                let relation = &self.diplomacy[&(a, b)];
                let mut love = relation.trust[side] / 10;
                let provoked = relation.cause_until[side] > self.turn;
                let contacted = relation.first_contact.is_some();
                let intel = self.diplomatic_intel(owner, other);
                let allied_with_enemy = intel.allied_with_enemy.is_some();
                let at_war_with_ally = intel.at_war_with_ally.is_some();
                if matches!(
                    relation.pact,
                    Pact::Ceasefire | Pact::Armistice | Pact::Peace | Pact::Alliance
                ) && !provoked
                    && self.ai_war_countdown(owner, other) == -1
                    && !allied_with_enemy
                    && !at_war_with_ally
                    && desires[&owner].is_none_or(|(target, _)| target != other)
                {
                    love += 15;
                    if self.allied(owner, other) {
                        love += 10;
                    }
                    love += 7
                        * (0..self.players.len())
                            .filter(|&third| {
                                third != owner
                                    && third != other
                                    && third != BARBARIANS
                                    && self.player_alive(third)
                                    && self.diplomatic_at_war(owner, third)
                                    && self.diplomatic_at_war(other, third)
                            })
                            .count() as i32;
                } else if self.diplomatic_at_war(owner, other) {
                    love -= 15;
                } else if provoked {
                    if love > 0 {
                        love -= love / 2;
                    }
                    love -= 180;
                }
                if love > 800 && !self.allied(owner, other) {
                    love -= 10;
                }
                let trespassers = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == other
                            && self.tile(unit.x, unit.y).owner == Some(owner)
                            && self.unit_visible_to(owner, unit)
                    })
                    .count() as i32;
                love -= (trespassers * 5).min(30 * (i32::from(allied_with_enemy) + 1));
                if contacted {
                    // daidiplomacy.c:get_player_bonus(EFT_GAIN_AI_LOVE) lets
                    // C2C3's selected effects decide which wonders apply and
                    // when (including the Apollo Program requirement).
                    love += self.campaign_player_effect(other, "Gain_AI_Love");
                }
                if let Some((target, desire)) = desires[&owner]
                    && target == other
                {
                    love -= desire.min(50);
                }
                love = (f64::from(love) * 0.96) as i32;
                self.diplomacy.get_mut(&(a, b)).unwrap().trust[side] = love.clamp(-1000, 1000) * 10;
            }
        }
    }
}
