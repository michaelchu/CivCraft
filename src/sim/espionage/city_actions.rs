use super::*;

impl Simulation {
    pub(crate) fn sabotageable_building(building: Building) -> bool {
        building.data().genus == crate::rules::Genus::Improvement
    }

    pub(super) fn can_target_city_covertly(&self, unit: &Unit, city: &City) -> bool {
        unit.kind.is_diplomatic()
            && unit.moves_left > 0
            && !self.tile(unit.x, unit.y).terrain.is_water()
            && city.owner != unit.owner
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
    }

    pub(crate) fn can_sabotage_city(&self, unit: &Unit, city: &City) -> bool {
        self.at_war(unit.owner, city.owner) && self.can_target_city_covertly(unit, city)
    }

    pub(crate) fn can_poison_city(&self, unit: &Unit, city: &City) -> bool {
        unit.kind == UnitKind::Spy
            && unit.transport.is_none()
            && city.size >= 2
            && self.can_sabotage_city(unit, city)
    }

    pub(in crate::sim) fn poison_city(&mut self, actor: u32, city_id: u32) -> bool {
        let Some(unit) = self.unit_by_id(actor).cloned() else {
            return false;
        };
        let Some(index) = self.city_index_by_id(city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if !self.can_poison_city(&unit, &city) {
            return false;
        }
        if !self.resolve_diplomatic_defender(&unit, &city, None) {
            // Freeciv rolls at 100% after infiltration. The escape roll is separate.
            self.rand_below(100);
            self.reduce_city_population(index);
            if unit.owner == HUMAN || city.owner == HUMAN {
                self.add_event(format!("A Spy poisoned the water supply of {}.", city.name));
            }
            self.finish_covert_mission(&unit, (city.x, city.y), true);
        }
        self.refresh_visibility();
        true
    }

    pub(crate) fn can_sabotage_unit(&self, spy: &Unit, target: &Unit) -> bool {
        spy.kind == UnitKind::Spy
            && spy.moves_left > 0
            && spy.transport.is_none()
            && !self.tile(spy.x, spy.y).terrain.is_water()
            && self.at_war(spy.owner, target.owner)
            && target.hp >= 2
            && self.map_distance(spy.x, spy.y, target.x, target.y) <= 1
            && !self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (target.x, target.y))
            && !self
                .units
                .iter()
                .any(|unit| unit.id != target.id && (unit.x, unit.y) == (target.x, target.y))
    }

    pub(in crate::sim) fn sabotage_unit(&mut self, actor: u32, target: u32) -> bool {
        let Some(spy) = self.unit_by_id(actor).cloned() else {
            return false;
        };
        let Some(index) = self.unit_index_by_id(target) else {
            return false;
        };
        let victim = &self.units[index];
        if !self.can_sabotage_unit(&spy, victim)
            || (spy.owner == HUMAN && !self.unit_visible_to_human(victim))
        {
            return false;
        }

        let name = victim.kind.name();
        let notify = spy.owner == HUMAN || victim.owner == HUMAN;
        self.units[index].hp /= 2;
        if notify {
            self.add_event(format!("A Spy sabotaged a {name}."));
        }
        // Unit sabotage escapes from the Spy's tile, unlike city missions.
        self.finish_covert_mission(&spy, (spy.x, spy.y), true);
        self.refresh_visibility();
        true
    }

    pub(crate) fn incite_price(&self, unit: &Unit, city: &City) -> Option<i32> {
        if !self.can_target_city_covertly(unit, city)
            || unit.transport.is_some()
            || self.allied(unit.owner, city.owner)
            || [
                Building::COURTHOUSE,
                Building::PALACE,
                Building::ECCLESIASTICAL_PALACE,
            ]
            .into_iter()
            .any(|building| city.has_building(building))
            || self.has_active_wonder(city.owner, Building::MAUSOLEUM_OF_MAUSOLOS)
        {
            return None;
        }
        let garrison: Vec<_> = self
            .units
            .iter()
            .filter(|unit| (unit.x, unit.y) == (city.x, city.y))
            .collect();
        let units: i64 = garrison
            .iter()
            .map(|unit| i64::from(unit.kind.stats().cost))
            .sum();
        let buildings: i64 = city
            .buildings
            .iter()
            .map(|building| i64::from(building.data().cost))
            .sum();
        let forecast = self.city_forecast(city);
        let (happy, unhappy, angry) = self.city_feelings(city, forecast.luxury);
        let effective_size = (city.size + happy - unhappy - 3 * angry).max(1);
        let distance = self
            .cities
            .iter()
            .filter(|capital| {
                capital.owner == city.owner
                    && (capital.has_building(Building::PALACE)
                        || capital.has_building(Building::ECCLESIASTICAL_PALACE))
            })
            .map(|capital| self.map_distance(city.x, city.y, capital.x, capital.y))
            .min()
            .unwrap_or(32)
            .min(32);
        let stability = (if forecast.disorder { 1 } else { 2 })
            * (if city.was_happy && forecast.happy {
                2
            } else {
                1
            });
        let base = (1000 + i64::from(self.players[city.owner].gold) + 2 * units + buildings)
            * stability
            * i64::from(effective_size);
        let cost_per_citizen = base * 100 / (i64::from(distance + 3) * i64::from(city.size));
        let foreign: i32 = city.foreign_citizens.iter().map(|&(_, count)| count).sum();
        let domestic = city.size - foreign;
        let weighted_citizens = 10 * domestic
            + city
                .foreign_citizens
                .iter()
                .map(|&(owner, count)| {
                    if owner == unit.owner {
                        5 * count
                    } else {
                        7 * count
                    }
                })
                .sum::<i32>();
        let culture = self.nation_scores[city.owner].achievement_culture
            + self
                .cities
                .iter()
                .filter(|other| other.owner == city.owner)
                .map(|other| other.history)
                .sum::<i32>();
        let bonus = 100
            + 100 * i64::from(!garrison.is_empty())
            + 50 * i64::from((culture / 1000).clamp(0, 10));
        Some(
            (cost_per_citizen * i64::from(weighted_citizens) * bonus / 100_000)
                .clamp(0, i64::from(i32::MAX)) as i32,
        )
    }

    pub(in crate::sim) fn incite_city(
        &mut self,
        actor: u32,
        city_id: u32,
        quoted_price: i32,
    ) -> bool {
        let Some(unit) = self.unit_by_id(actor).cloned() else {
            return false;
        };
        let Some(index) = self.city_index_by_id(city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        let owner = unit.owner;
        if self.incite_price(&unit, &city) != Some(quoted_price)
            || self.players[owner].gold < quoted_price
        {
            return false;
        }
        if self.resolve_diplomatic_defender(
            &unit,
            &city,
            Some(diplomacy::DiplomaticIncident::CityLoss),
        ) {
            self.refresh_visibility();
            return true;
        }
        let location = (city.x, city.y);
        if !self.at_war(owner, city.owner) {
            self.record_provocation(city.owner, owner);
            self.record_diplomatic_incident(
                owner,
                Some(city.owner),
                diplomacy::DiplomaticIncident::CityLoss,
                false,
            );
        }
        if self.rand_below(100) >= 80 {
            self.finish_covert_mission(&unit, location, false);
            if owner == HUMAN || city.owner == HUMAN {
                self.add_event(format!(
                    "A {} was caught inciting revolt in {}.",
                    unit.kind.name(),
                    city.name
                ));
            }
            self.refresh_visibility();
            return true;
        }

        self.players[owner].gold -= quoted_price;
        if city.size > 1 {
            self.reduce_city_population(index);
        }
        if let Some(tech) = self.random_stealable_technology(owner, city.owner) {
            self.discover(owner, tech);
            if self.players[owner].researching == tech {
                self.players[owner].researching = self.research_target(owner);
            }
        }
        let first_city = !self.cities.iter().any(|other| other.owner == owner);
        let city_after = &mut self.cities[index];
        city_after.transfer_citizens(owner);
        self.list_order.city(city_after);
        city_after.shields = 0;
        city_after.protected_shields = 0;
        city_after.caravan_shields = 0;
        city_after.production_before_switch = None;
        city_after.bought_production = false;
        city_after.was_happy = false;
        city_after
            .buildings
            .retain(|building| building.data().genus != crate::rules::Genus::SmallWonder);
        if first_city {
            city_after.buildings.push(Building::PALACE);
        }
        let transfer_units = self.city_transfer_unit_ids(city_id, city.owner);
        for id in transfer_units {
            let Some(other) = self.units.iter_mut().find(|unit| unit.id == id) else {
                continue;
            };
            let stationed = (other.x, other.y) == location;
            let (dx, dy) = self.map_wrap.vector(other.x, other.y, city.x, city.y);
            let in_other_friendly_city = self
                .cities
                .iter()
                .any(|home| home.owner == city.owner && (home.x, home.y) == (other.x, other.y));
            if stationed
                || (other.home_city == Some(city_id)
                    && isohex_vector_distance(dx, dy) <= 1
                    && !in_other_friendly_city)
            {
                other.owner = owner;
                self.list_order.unit(other);
                other.home_city = other.home_city.map(|_| city_id);
                other.automated = false;
                other.clear_work();
            }
        }
        self.release_city_support_except(city_id, Some(owner));
        if !self.can_produce(&self.cities[index], self.cities[index].production) {
            self.cities[index].production = self.choose_ai_production(owner, city_id);
        }
        self.prune_trade_routes();
        self.rebuild_city_ownership();
        self.refresh_diplomatic_contact_at(owner, city.x, city.y);
        if owner == HUMAN || city.owner == HUMAN {
            self.add_event(format!(
                "{} revolted and joined {}.",
                city.name, self.players[owner].name
            ));
        }
        self.finish_covert_mission(&unit, location, true);
        self.refresh_visibility();
        self.check_game_over();
        true
    }

    pub(super) fn charge_covert_movement(&mut self, unit: &Unit, city: &City) {
        let cost = self
            .movement_cost_for(unit, city.x, city.y)
            .unwrap_or(MOVE_FRAGMENTS);
        if let Some(mover) = self.units.iter_mut().find(|other| other.id == unit.id) {
            mover.moves_left = (mover.moves_left - cost).max(0);
        }
    }

    pub(super) fn finish_covert_mission(&mut self, unit: &Unit, origin: (i32, i32), success: bool) {
        let escape_city = if success && unit.kind == UnitKind::Spy {
            self.cities
                .iter()
                .filter(|home| home.owner == unit.owner)
                .min_by_key(|home| self.map_distance(origin.0, origin.1, home.x, home.y))
                .map(|home| (home.x, home.y, home.name.clone()))
        } else {
            None
        };
        if let Some((x, y, home)) = escape_city
            && self.rand_below(100) < (80 + unit.veteran_power() - 100)
        {
            let spy = self
                .units
                .iter_mut()
                .find(|other| other.id == unit.id)
                .unwrap();
            (spy.x, spy.y) = (x, y);
            spy.transport = None;
            spy.moves_left = 0;
            spy.moved = true;
            spy.state = UnitState::Idle;
            self.promote_unit(unit.id, 100);
            self.release_ai_ferry(unit.id);
            self.refresh_diplomatic_contact_at(unit.owner, x, y);
            if unit.owner == HUMAN {
                self.add_event(format!("Your Spy escaped to {home}."));
            }
        } else {
            self.units.retain(|other| other.id != unit.id);
            self.discard_unit_ai(unit.id);
            if success && unit.owner == HUMAN {
                self.add_event(format!(
                    "Your {} was captured after the mission.",
                    unit.kind.name()
                ));
            }
        }
    }

    pub(in crate::sim) fn sabotage_city(
        &mut self,
        actor: u32,
        city_id: u32,
        target: Option<SabotageTarget>,
    ) -> bool {
        let Some(unit) = self.unit_by_id(actor).cloned() else {
            return false;
        };
        let Some(index) = self.city_index_by_id(city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if !self.can_sabotage_city(&unit, &city)
            || (target.is_some() && unit.kind != UnitKind::Spy)
            || matches!(target, Some(SabotageTarget::Building(building))
                if !city.has_building(building) || !Self::sabotageable_building(building))
        {
            return false;
        }
        if self.resolve_diplomatic_defender(&unit, &city, None) {
            self.refresh_visibility();
            return true;
        }

        let mut success = false;
        let name = unit.kind.name();
        let subject = if unit.owner == HUMAN {
            "Your"
        } else {
            "An enemy"
        };
        let mission_chance = if target.is_some() { 40 } else { 80 };
        let outcome = if self.rand_below(100) >= mission_chance {
            format!(
                "{subject} {name} was caught attempting sabotage in {}.",
                city.name
            )
        } else {
            let mut targets: Vec<_> = city
                .buildings
                .iter()
                .copied()
                .filter(|&building| Self::sabotageable_building(building))
                .collect();
            targets.sort_by_key(|building| building.0);
            if target.is_none() && targets.is_empty() && city.shields == 0 {
                self.charge_covert_movement(&unit, &city);
                if unit.owner == HUMAN {
                    self.add_event(format!(
                        "{subject} {name} found nothing to sabotage in {}.",
                        city.name
                    ));
                }
                return true;
            }
            let building = match target {
                Some(SabotageTarget::Production) => None,
                Some(SabotageTarget::Building(building)) => Some(building),
                None if targets.is_empty() || self.rand_below(2) == 1 => None,
                None => Some(targets[self.rand_below(targets.len() as i32) as usize]),
            };
            if let Some(building) = building {
                let vulnerability = if building == Building::CITY_WALLS {
                    50
                } else {
                    100
                };
                let capital = city.has_building(Building::PALACE)
                    || city.has_building(Building::ECCLESIASTICAL_PALACE);
                if self.rand_below(100)
                    < vulnerability - percent_of(vulnerability, 50 * i32::from(capital))
                {
                    success = true;
                    self.cities[index]
                        .buildings
                        .retain(|&built| built != building);
                    format!(
                        "{subject} {name} destroyed {} in {}.",
                        building.data().name,
                        city.name
                    )
                } else {
                    format!(
                        "{subject} {name} was caught sabotaging {} in {}.",
                        building.data().name,
                        city.name
                    )
                }
            } else {
                let target = &mut self.cities[index];
                success = true;
                target.shields = 0;
                target.protected_shields = 0;
                target.caravan_shields = 0;
                target.production_before_switch = None;
                format!(
                    "{subject} {name} destroyed the production stockpile in {}.",
                    city.name
                )
            }
        };
        if unit.owner == HUMAN || city.owner == HUMAN {
            self.add_event(outcome);
        }
        self.finish_covert_mission(&unit, (city.x, city.y), success);
        self.refresh_visibility();
        true
    }

    pub(super) fn resolve_diplomatic_defender(
        &mut self,
        actor: &Unit,
        city: &City,
        incident: Option<diplomacy::DiplomaticIncident>,
    ) -> bool {
        let defender = self
            .units
            .iter()
            .find(|unit| {
                unit.owner != actor.owner
                    && (unit.x, unit.y) == (city.x, city.y)
                    && unit.kind.is_diplomatic()
            })
            .map(|unit| (unit.id, unit.kind, unit.veteran_power()));
        let Some((defender, defender_kind, defender_power)) = defender else {
            return false;
        };
        let resistance = 50
            * i32::from(
                city.has_building(Building::PALACE)
                    || city.has_building(Building::ECCLESIASTICAL_PALACE),
            )
            + 25 * i32::from(matches!(
                self.tile(city.x, city.y).base,
                Some(Base::Fort | Base::Fortress)
            ));
        let chance = 50
            + 25 * (i32::from(actor.kind == UnitKind::Spy)
                - i32::from(defender_kind == UnitKind::Spy))
            + actor.veteran_power()
            - defender_power;
        let chance = (chance - percent_of(chance, resistance)).clamp(0, 100);
        let attacker_won = self.rand_below(100) < chance;
        let loser = if attacker_won { defender } else { actor.id };
        self.units.retain(|unit| unit.id != loser);
        self.discard_unit_ai(loser);
        if attacker_won {
            let unit = self
                .units
                .iter_mut()
                .find(|unit| unit.id == actor.id)
                .unwrap();
            unit.moves_left = (unit.moves_left - MOVE_FRAGMENTS).max(0);
        } else if !self.at_war(actor.owner, city.owner)
            && let Some(incident) = incident
        {
            self.record_provocation(city.owner, actor.owner);
            self.record_diplomatic_incident(actor.owner, Some(city.owner), incident, false);
        }
        let (winner, modifier) = if attacker_won {
            (actor.id, (100 - chance) * 2)
        } else {
            (defender, chance * 2)
        };
        self.promote_unit(winner, modifier);
        if actor.owner == HUMAN {
            let detail = if attacker_won {
                format!(
                    "Your {} eliminated a defending {}.",
                    actor.kind.name(),
                    defender_kind.name()
                )
            } else {
                format!(
                    "A defending {} eliminated your {}.",
                    defender_kind.name(),
                    actor.kind.name()
                )
            };
            self.add_event(detail);
        }
        true
    }
}
