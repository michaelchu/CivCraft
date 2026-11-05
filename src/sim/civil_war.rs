use super::*;

impl Government {
    pub(super) fn civil_war_chance(self) -> i32 {
        match self {
            Government::Anarchy => 99,
            Government::Tribal => 90,
            Government::Despotism => 80,
            Government::Monarchy => 70,
            Government::Communism => 50,
            Government::Fundamentalism => 60,
            Government::Federation => 20,
            Government::Republic => 40,
            Government::Democracy => 30,
        }
    }
}

impl Simulation {
    pub(super) fn capital_loss_triggers_civil_war(&mut self, owner: usize) -> bool {
        if owner == BARBARIANS || self.players.len() >= PLAYER_SLOTS {
            return false;
        }
        let cities: Vec<_> = self
            .cities
            .iter()
            .filter(|city| city.owner == owner)
            .collect();
        if cities.len() < 10 {
            return false;
        }
        let base = self.players[owner].government.civil_war_chance();
        let modifier: i32 = cities
            .into_iter()
            .map(|city| {
                let luxury = self.city_trade(city, self.city_yields(city).2)[2];
                let (disorder, happy) = self.city_mood(city, luxury);
                5 * (i32::from(disorder) - i32::from(city.was_happy && happy))
            })
            .sum();
        self.rand_below(100) < base + modifier
    }

    pub(super) fn split_civil_war(&mut self, owner: usize) {
        let candidates: Vec<_> = self
            .player_cities_ordered(owner)
            .filter(|city| !city.buildings.contains(&Building::PALACE))
            .filter_map(|city| {
                self.cities
                    .iter()
                    .position(|candidate| candidate.id == city.id)
            })
            .collect();
        if candidates.is_empty() {
            return;
        }
        self.clear_outgoing_embassies(owner);
        let rebel = self.players.len();
        let original = &mut self.players[owner];
        let gold = original.gold / 2;
        original.gold -= gold;
        original.research = 0;
        if original.government != Government::Anarchy {
            original.revolution_target = Some(original.government);
            original.revolution_turn = Some(self.turn + 1);
            original.government = Government::Anarchy;
        }
        const REBEL_NAMES: [&str; 5] = [
            "Free Cities",
            "Separatists",
            "Breakaway League",
            "Rebel Coalition",
            "Independent Union",
        ];
        let rebel_player = Player {
            name: REBEL_NAMES[(rebel - PLAYER_COUNT - 1) % REBEL_NAMES.len()].to_owned(),
            gold,
            rates: [60, 40, 0],
            government: Government::Tribal,
            revolution_target: None,
            revolution_turn: None,
            known: original.known.clone(),
            researching: original.researching,
            research_goal: original.research_goal,
            future_techs: original.future_techs,
            research: 0,
            last_science: 0,
            ai: true,
        };
        self.players.push(rebel_player);
        // daiplayer.c:dai_player_copy copies retained advisor wants during
        // player splits; they are nevertheless transient across save/load.
        let wants = self
            .ai_tech_wants
            .get(&owner)
            .cloned()
            .unwrap_or_else(|| vec![0.0; Tech::all().count()]);
        self.ai_tech_wants.insert(rebel, wants);
        self.spaceships.push(Spaceship::default());
        self.nation_scores.push(NationScore::default());
        for tile in &mut self.tiles {
            tile.known_by[rebel] = tile.known_by[owner];
        }

        let mut remaining = (candidates.len() / 2).max(1);
        let mut defectors = Vec::new();
        for (position, &index) in candidates.iter().enumerate() {
            if remaining >= candidates.len() - position
                || (remaining > 0 && self.rand_below(2) == 1)
            {
                self.cities[index].transfer_citizens(rebel);
                self.list_order.city(&self.cities[index]);
                defectors.push(index);
                remaining -= 1;
            }
        }
        let capital = defectors[self.rand_below(defectors.len() as i32) as usize];
        self.cities[capital].buildings.push(Building::PALACE);
        let unit_ids: Vec<_> = self
            .player_units_ordered(owner)
            .map(|unit| unit.id)
            .collect();
        for id in unit_ids {
            let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) else {
                continue;
            };
            let rebel_home = self
                .cities
                .iter()
                .any(|city| Some(city.id) == unit.home_city && city.owner == rebel);
            if let Some(city) = self.cities.iter().find(|city| {
                matches!(city.owner, id if id == owner || id == rebel)
                    && (city.x, city.y) == (unit.x, unit.y)
            }) {
                if unit.home_city.is_some() && (city.owner != owner || rebel_home) {
                    unit.home_city = Some(city.id);
                }
                unit.owner = city.owner;
            } else if rebel_home {
                unit.owner = rebel;
            }
            self.list_order.unit(unit);
        }
        self.resolve_split_stacks(owner, rebel);
        self.rebuild_city_ownership();
        self.refresh_visibility();
        self.add_event(format!(
            "Civil war partitions {}. {} now hold {} cities.",
            self.players[owner].name,
            self.players[rebel].name,
            defectors.len()
        ));
    }

    pub(super) fn resolve_split_stacks(&mut self, original: usize, rebel: usize) {
        let mut relocations = Vec::new();
        for unit in &self.units {
            if unit.owner != rebel && unit.owner != original {
                continue;
            }
            let wrong_transport = unit.transport.is_some_and(|id| {
                self.units
                    .iter()
                    .any(|boat| boat.id == id && boat.owner != unit.owner)
            });
            let mixed = self.units.iter().any(|other| {
                other.owner != unit.owner
                    && (other.owner == original || other.owner == rebel)
                    && (other.x, other.y) == (unit.x, unit.y)
            });
            let foreign_city = self.cities.iter().any(|city| {
                (city.x, city.y) == (unit.x, unit.y) && !self.allied(city.owner, unit.owner)
            });
            if !wrong_transport && !mixed && !foreign_city {
                continue;
            }
            if !wrong_transport && unit.transport.is_some() {
                continue;
            }
            let city = self
                .cities
                .iter()
                .filter(|city| city.owner == unit.owner)
                .filter(|city| !unit.kind.is_naval() || self.naval_city_access(city, unit.kind))
                .min_by_key(|city| self.map_distance(unit.x, unit.y, city.x, city.y));
            let rival_distance = self
                .cities
                .iter()
                .filter(|city| {
                    (city.owner == original || city.owner == rebel) && city.owner != unit.owner
                })
                .map(|city| self.map_distance(unit.x, unit.y, city.x, city.y))
                .min()
                .unwrap_or(i32::MAX);
            let distance = city.map_or(i32::MAX, |city| {
                self.map_distance(unit.x, unit.y, city.x, city.y)
            });
            if wrong_transport
                || foreign_city
                || distance < rival_distance
                || (distance == rival_distance && unit.owner == rebel)
            {
                relocations.push((unit.id, city.map(|city| (city.x, city.y))));
            }
        }
        for (id, destination) in relocations {
            if let Some((x, y)) = destination {
                if let Some(unit) = self.units.iter_mut().find(|unit| unit.id == id) {
                    (unit.x, unit.y) = (x, y);
                    unit.transport = None;
                    unit.moves_left = 0;
                }
                self.move_cargo(id, x, y);
            } else {
                self.remove_units_with_cargo(vec![id]);
            }
        }
    }
}
