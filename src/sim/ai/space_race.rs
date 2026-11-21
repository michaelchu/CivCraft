//! Assign the empire's space projects once per production-advice pass.

use super::*;
use std::collections::BTreeMap;

const PART_INFRASTRUCTURE: [Building; 4] = [
    Building::FACTORY,
    Building::LIBRARY,
    Building::UNIVERSITY,
    Building::RESEARCH_LAB,
];

impl Simulation {
    pub(in crate::sim) fn space_production_plan(
        &self,
        owner: usize,
    ) -> Option<BTreeMap<u32, Building>> {
        let ship = &self.spaceships[owner];
        if ship.launched() || !self.ai_space_commitment(owner) {
            return None;
        }
        let mut sites: Vec<_> = self
            .cities_owned_by(owner)
            .filter_map(|city| {
                let output = self.city_forecast(city);
                let shields = output.shield_surplus();
                (shields > 0
                    && !output.disorder
                    && self.city_grave_danger(city) == 0
                    && self.advisor_guard_city_assessment(city).urgency <= 1)
                    .then_some((city, shields))
            })
            .collect();
        // A ship already in progress (or after Apollo is completed) can fall
        // back to slower sites if no city reaches the ideal shield threshold.
        if (!self.apollo_completed && ship.built == [0; 3])
            || sites
                .iter()
                .any(|&(_, shields)| shields >= ai_strategy::space_min_city_shields())
        {
            sites.retain(|&(_, shields)| shields >= ai_strategy::space_min_city_shields());
        } else {
            sites.retain(|&(_, shields)| shields >= 4);
        }
        sites.sort_by_key(|&(city, shields)| (std::cmp::Reverse(shields), city.id));
        let mut plan = BTreeMap::new();
        let strategy = self.strategic_priorities(owner);
        if !self.apollo_completed
            && !strategy.economic_crisis
            && let Some(&(city, _)) = sites
                .iter()
                .filter(|(city, _)| {
                    self.can_change_production(city, Production::Building(Building::APOLLO_PROGRAM))
                })
                .min_by_key(|&&(city, shields)| {
                    let stock = if city.production == Production::Building(Building::APOLLO_PROGRAM)
                    {
                        city.shields
                    } else {
                        0
                    };
                    let missing = (Building::APOLLO_PROGRAM.data().cost - stock).max(0);
                    ((missing + shields - 1) / shields, city.id)
                })
        {
            plan.insert(city.id, Building::APOLLO_PROGRAM);
        }

        let mut remaining = std::array::from_fn::<_, 3, _>(|part| {
            spaceship::SPACE_PART_PROJECTS[part]
                .1
                .saturating_sub(ship.built[part])
        });
        let mut available = Vec::new();
        for (city, _) in sites {
            if plan.contains_key(&city.id) {
                continue;
            }
            if let Some(building) = PART_INFRASTRUCTURE
                .into_iter()
                .find(|&required| !city.has_building(required))
            {
                if !strategy.economic_crisis
                    && self.can_change_production(city, Production::Building(building))
                {
                    plan.insert(city.id, building);
                }
                continue;
            }
            if !self.apollo_completed {
                continue;
            }
            // Keep invested shields and count this project against the part
            // limit before assigning new cities.
            if let Production::Building(building) = city.production
                && let Some((part, _)) = buildings::spaceship_part(building)
                && remaining[part] > 0
                && city.shields > 0
                && self.can_change_production(city, city.production)
            {
                remaining[part] -= 1;
                plan.insert(city.id, building);
            } else {
                available.push(city);
            }
        }
        for city in available {
            let project = spaceship::SPACE_PART_PROJECTS
                .iter()
                .enumerate()
                .filter(|&(part, &(building, _))| {
                    remaining[part] > 0
                        && self.can_change_production(city, Production::Building(building))
                })
                .max_by_key(|&(part, &(building, _))| {
                    remaining[part] * building.data().cost as usize
                });
            if let Some((part, &(building, _))) = project {
                remaining[part] -= 1;
                plan.insert(city.id, building);
            }
        }
        Some(plan)
    }
}
