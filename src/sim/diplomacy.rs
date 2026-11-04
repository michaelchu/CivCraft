use super::*;

#[derive(Default)]
pub(super) struct Relation {
    ceasefire_until: Option<i32>,
    cause_until: [i32; 2],
}

impl Simulation {
    fn relation_key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

    pub(crate) fn at_war(&self, a: usize, b: usize) -> bool {
        a != b && self.ceasefire_turns(a, b) == 0
    }

    pub(crate) fn ceasefire_turns(&self, a: usize, b: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .and_then(|relation| relation.ceasefire_until)
            .map_or(0, |until| (until - self.turn).max(0))
    }

    pub(crate) fn has_senate(&self, owner: usize) -> bool {
        if self.owns_wonder(owner, "Statue of Liberty") {
            return false;
        }
        matches!(
            self.players[owner].government,
            Government::Federation | Government::Democracy
        ) || (self
            .cities
            .iter()
            .any(|city| city.has_building("United Nations"))
            && self
                .completed_wonders
                .iter()
                .any(|building| building.data().name == "Manhattan Project")
            && !self.apollo_completed
            && self.players[owner]
                .known
                .iter()
                .any(|tech| tech.name() == "Nuclear Fission"))
    }

    pub(crate) fn senate_blocks_cancellation(&self, owner: usize, other: usize) -> bool {
        self.has_senate(owner)
            && self
                .diplomacy
                .get(&Self::relation_key(owner, other))
                .is_none_or(|relation| {
                    relation.cause_until[usize::from(owner > other)] <= self.turn
                })
    }

    pub(crate) fn can_negotiate(&self, other: usize) -> bool {
        other != HUMAN
            && other != BARBARIANS
            && other < self.players.len()
            && (self.cities.iter().any(|city| city.owner == other)
                || self.units.iter().any(|unit| unit.owner == other))
            && (self
                .diplomacy
                .contains_key(&Self::relation_key(HUMAN, other))
                || self
                    .cities
                    .iter()
                    .any(|city| city.owner == other && self.tile(city.x, city.y).visible)
                || self
                    .units
                    .iter()
                    .any(|unit| unit.owner == other && self.tile(unit.x, unit.y).visible))
    }

    pub(super) fn agree_ceasefire(&mut self, other: usize) -> bool {
        if !self.can_negotiate(other) || !self.at_war(HUMAN, other) {
            return false;
        }
        self.diplomacy
            .entry(Self::relation_key(HUMAN, other))
            .or_default()
            .ceasefire_until = Some(self.turn + 16);
        self.add_event(format!(
            "{} agreed to a ceasefire.",
            self.players[other].name
        ));
        true
    }

    pub(super) fn cancel_ceasefire(&mut self, owner: usize, other: usize) -> bool {
        if self.ceasefire_turns(owner, other) == 0 {
            return false;
        }
        if self.senate_blocks_cancellation(owner, other) {
            if owner == HUMAN {
                self.add_event(format!(
                    "The Senate refuses to break the ceasefire with {}.",
                    self.players[other].name
                ));
            }
            return false;
        }
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(owner, other))
            .unwrap();
        relation.ceasefire_until = None;
        relation.cause_until[usize::from(owner > other)] = 0;
        self.record_provocation(other, owner);
        self.add_event(format!(
            "{} ended the ceasefire with {}.",
            self.players[owner].name, self.players[other].name
        ));
        true
    }

    pub(super) fn record_provocation(&mut self, victim: usize, offender: usize) {
        if victim != offender && victim != BARBARIANS && offender != BARBARIANS {
            self.diplomacy
                .entry(Self::relation_key(victim, offender))
                .or_default()
                .cause_until[usize::from(victim > offender)] = self.turn + 2;
        }
    }

    pub(super) fn remember_visible_rivals(&mut self) {
        for owner in self
            .units
            .iter()
            .filter(|unit| self.tiles[Self::idx(unit.x, unit.y)].visible)
            .map(|unit| unit.owner)
            .chain(
                self.cities
                    .iter()
                    .filter(|city| self.tiles[Self::idx(city.x, city.y)].visible)
                    .map(|city| city.owner),
            )
        {
            if owner != HUMAN && owner != BARBARIANS {
                self.diplomacy
                    .entry(Self::relation_key(HUMAN, owner))
                    .or_default();
            }
        }
    }

    pub(super) fn advance_diplomacy(&mut self) {
        let mut expired = Vec::new();
        for (&(a, b), relation) in &mut self.diplomacy {
            if relation
                .ceasefire_until
                .is_some_and(|until| until <= self.turn)
            {
                relation.ceasefire_until = None;
                if a == HUMAN || b == HUMAN {
                    expired.push(if a == HUMAN { b } else { a });
                }
            }
        }
        for other in expired {
            self.add_event(format!(
                "The ceasefire with {} expired.",
                self.players[other].name
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senate_enforces_treaties_unless_provoked_or_waived() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        let key = Simulation::relation_key(HUMAN, 1);
        game.diplomacy.entry(key).or_default().ceasefire_until = Some(17);
        game.players[HUMAN].government = Government::Federation;
        assert!(!game.apply(Command::CancelCeasefire(1)));
        game.record_provocation(HUMAN, 1);
        assert!(game.apply(Command::CancelCeasefire(1)));
        assert!(game.at_war(HUMAN, 1));

        let find = |name| {
            Building::all()
                .find(|building| building.data().name == name)
                .unwrap()
        };
        game.diplomacy.get_mut(&key).unwrap().ceasefire_until = Some(17);
        game.cities[0].buildings.push(find("Statue of Liberty"));
        assert!(game.apply(Command::CancelCeasefire(1)));

        game.cities[0]
            .buildings
            .retain(|building| building.data().name != "Statue of Liberty");
        game.players[HUMAN].government = Government::Tribal;
        game.cities[0].buildings.push(find("United Nations"));
        game.completed_wonders.push(find("Manhattan Project"));
        let fission = Tech::all()
            .find(|tech| tech.name() == "Nuclear Fission")
            .unwrap();
        game.players[HUMAN].known.push(fission);
        assert!(game.has_senate(HUMAN));
        game.apollo_completed = true;
        assert!(!game.has_senate(HUMAN));
    }
}
