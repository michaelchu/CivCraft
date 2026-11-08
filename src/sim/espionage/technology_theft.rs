//! Technology theft mission resolution.

use super::*;

impl Simulation {
    pub(crate) fn stealable_technology(&self, owner: usize, other: usize, tech: Tech) -> bool {
        if tech == Tech::FUTURE {
            self.players[other].future_techs > self.players[owner].future_techs
        } else {
            self.has_tech(other, tech) && self.tech_available(owner, tech)
        }
    }

    pub(crate) fn can_steal_technology(&self, unit: &Unit, city: &City) -> bool {
        city.owner != BARBARIANS && self.can_target_city_covertly(unit, city)
    }

    pub(super) fn random_stealable_technology(
        &mut self,
        owner: usize,
        other: usize,
    ) -> Option<Tech> {
        let available: Vec<_> = Tech::all()
            .filter(|&tech| self.stealable_technology(owner, other, tech))
            .collect();
        if available.is_empty() {
            self.stealable_technology(owner, other, Tech::FUTURE)
                .then_some(Tech::FUTURE)
        } else {
            Some(available[self.rand_below(available.len() as i32) as usize])
        }
    }

    pub(in crate::sim) fn steal_technology(
        &mut self,
        actor: u32,
        city_id: u32,
        target: Option<Tech>,
    ) -> bool {
        let Some(unit) = self.unit_by_id(actor).cloned() else {
            return false;
        };
        let Some(index) = self.city_index_by_id(city_id) else {
            return false;
        };
        let city = self.cities[index].clone();
        if !self.can_steal_technology(&unit, &city)
            || target.is_some_and(|tech| {
                unit.kind != UnitKind::Spy
                    || !self.stealable_technology(unit.owner, city.owner, tech)
            })
        {
            return false;
        }
        if self.resolve_diplomatic_defender(
            &unit,
            &city,
            Some(diplomacy::DiplomaticIncident::Theft),
        ) {
            self.refresh_visibility();
            return true;
        }

        let repeated = city.technology_thefts > 0 && unit.kind == UnitKind::Diplomat;
        let failed = repeated
            || (0..=city.technology_thefts + u32::from(target.is_some()))
                .any(|_| self.rand_below(100) >= 80);
        if failed {
            self.units.retain(|other| other.id != actor);
            self.discard_unit_ai(actor);
            if !self.at_war(unit.owner, city.owner) {
                self.record_provocation(city.owner, unit.owner);
                self.record_diplomatic_incident(
                    unit.owner,
                    Some(city.owner),
                    diplomacy::DiplomaticIncident::Theft,
                    false,
                );
            }
            if unit.owner == HUMAN {
                self.add_event(if repeated {
                    format!(
                        "{} was prepared for another theft. Your Diplomat was caught.",
                        city.name
                    )
                } else {
                    format!(
                        "Your {} was caught stealing technology from {}.",
                        unit.kind.name(),
                        city.name
                    )
                });
            }
            self.refresh_visibility();
            return true;
        }

        let technology =
            target.or_else(|| self.random_stealable_technology(unit.owner, city.owner));
        let Some(tech) = technology else {
            self.charge_covert_movement(&unit, &city);
            if unit.owner == HUMAN {
                self.add_event(format!("No new technology was found in {}.", city.name));
            }
            return true;
        };

        self.discover(unit.owner, tech);
        if self.players[unit.owner].researching == tech {
            self.players[unit.owner].researching = self.research_target(unit.owner);
        }
        self.cities[index].technology_thefts += 1;
        if !self.at_war(unit.owner, city.owner) {
            self.record_provocation(city.owner, unit.owner);
            self.record_diplomatic_incident(
                unit.owner,
                Some(city.owner),
                diplomacy::DiplomaticIncident::Theft,
                false,
            );
        }
        if unit.owner == HUMAN {
            self.add_event(format!(
                "Your {} stole {} from {}.",
                unit.kind.name(),
                tech.name(),
                city.name
            ));
        } else if city.owner == HUMAN {
            self.add_event(format!(
                "{} stole {} from {}.",
                self.players[unit.owner].name,
                tech.name(),
                city.name
            ));
        }
        self.finish_covert_mission(&unit, (city.x, city.y), true);
        self.refresh_visibility();
        true
    }
}
