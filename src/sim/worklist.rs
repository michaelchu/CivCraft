//! City production queues and their editing rules.

use super::*;
use crate::rules::{Genus, Requirement};

pub(crate) const WORKLIST_LIMIT: usize = 64;

#[derive(Clone, Copy)]
pub(crate) enum WorklistEdit {
    Append(Production),
    Remove(usize),
    Move(usize, usize),
}

impl Simulation {
    pub(super) fn edit_worklist(&mut self, city_id: u32, edit: WorklistEdit) -> bool {
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == city_id && city.owner == HUMAN)
        else {
            return false;
        };
        let city = &self.cities[index];
        let count = city.worklist.len();
        match edit {
            WorklistEdit::Append(target)
                if count < WORKLIST_LIMIT && self.can_produce(city, target) =>
            {
                self.cities[index].worklist.push(target);
            }
            WorklistEdit::Remove(from) if from < count => {
                self.cities[index].worklist.remove(from);
            }
            WorklistEdit::Move(from, to) if from < count && to < count && from != to => {
                let worklist = &mut self.cities[index].worklist;
                let target = worklist.remove(from);
                worklist.insert(to, target);
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn worklist_replacement(&self, city: &City, target: Production) -> Production {
        if let Production::Unit(kind) = target
            && let Some(successor) = self.production_successor(city, kind)
        {
            return Production::Unit(successor);
        }
        target
    }

    fn worklist_item_expired(&self, city: &City, target: Production) -> bool {
        let Production::Building(building) = target else {
            return false;
        };
        let data = building.data();
        (data.genus == Genus::GreatWonder && self.completed_wonders.contains(&building))
            || (data.genus == Genus::SmallWonder && city.has_building(building))
            || data.obsolete.iter().any(|&(req, present)| {
                matches!(req, Requirement::Tech(id) if self.has_tech(city.owner, Tech(id)) == present)
            })
    }

    pub(super) fn advance_city_worklist(&mut self, index: usize) -> bool {
        let city = &self.cities[index];
        if city.worklist.is_empty() {
            return false;
        }
        let mut remaining = Vec::with_capacity(city.worklist.len());
        let mut next = None;
        for &entry in &city.worklist {
            let target = self.worklist_replacement(city, entry);
            if next.is_none() && self.can_produce(city, target) {
                next = Some(target);
            } else if next.is_some() || !self.worklist_item_expired(city, entry) {
                remaining.push(entry);
            }
        }
        let city = &mut self.cities[index];
        city.worklist = remaining;
        let Some(target) = next else {
            return false;
        };
        city.production = target;
        if city.owner == HUMAN {
            let message = format!("{} began {} from its worklist.", city.name, target.name());
            self.add_event(message);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worklist_skips_unavailable_items_and_preserves_order_and_shields() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 20);
        let id = game.cities[0].id;
        let granary = Production::Building(Building::GRANARY);
        let worker = Production::Unit(UnitKind::Worker);
        let warrior = Production::Unit(UnitKind::Warrior);
        game.players[HUMAN].known.push(Tech::POTTERY);
        for target in [warrior, worker, granary] {
            assert!(game.apply(Command::EditWorklist(id, WorklistEdit::Append(target))));
        }
        assert!(game.apply(Command::EditWorklist(id, WorklistEdit::Move(2, 0))));
        assert!(game.apply(Command::EditWorklist(id, WorklistEdit::Remove(1))));
        game.players[HUMAN]
            .known
            .retain(|&tech| tech != Tech::POTTERY);
        game.cities[0].shields = warrior.cost() + 1;
        game.process_cities();
        assert_eq!(game.cities[0].production, worker);
        assert_eq!(game.cities[0].worklist, [granary]);
        assert!(game.cities[0].shields >= 1);

        game.players[HUMAN].known.push(Tech::POTTERY);
        game.cities[0].shields = worker.cost();
        game.cities[0].size = 2;
        game.cities[0].food = 10;
        game.process_cities();
        assert_eq!(game.cities[0].production, granary);
        assert!(game.cities[0].worklist.is_empty());
    }
}
