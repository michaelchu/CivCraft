//! Spaceship part production, assembly, and launch outcomes.

use super::*;

// Attachment indices from Freeciv common/spaceship.c, in placement order.
pub(crate) const STRUCTURE_PARENT: [usize; 32] = [
    0, 0, 0, 1, 2, 3, 4, 5, 2, 3, 6, 7, 8, 9, 10, 11, 14, 15, 16, 17, 18, 19, 4, 5, 22, 23, 24, 25,
    26, 27, 20, 21,
];
const COMPONENT_SUPPORT: [usize; 16] = [0, 12, 1, 13, 8, 12, 9, 13, 22, 28, 23, 29, 26, 28, 27, 29];
const MODULE_SUPPORT: [usize; 12] = [0, 1, 10, 15, 14, 11, 18, 19, 20, 31, 30, 21];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShipPart {
    Structural(usize),
    Component(usize),
    Module(usize),
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Spaceship {
    pub(super) built: [usize; 3],
    structure: [bool; 32],
    modules: [usize; 3],
    components: [usize; 2],
    manual: bool,
    arrival: Option<f64>,
    arrived: bool,
}

impl Spaceship {
    pub(super) fn arrival_year(&self) -> Option<f64> {
        self.arrival
    }

    pub(super) fn launched(&self) -> bool {
        self.arrival.is_some()
    }

    pub(super) fn add_part(&mut self, part: usize) {
        self.built[part] += 1;
        if !self.manual {
            self.autoplace();
        }
    }

    fn unplaced(&self) -> [usize; 3] {
        [
            self.built[0] - self.structure.iter().filter(|&&placed| placed).count(),
            self.built[1] - self.components.iter().sum::<usize>(),
            self.built[2] - self.modules.iter().sum::<usize>(),
        ]
    }

    fn can_place(&self, part: ShipPart) -> bool {
        if self.launched() {
            return false;
        }
        let [structure, components, modules] = self.unplaced();
        match part {
            ShipPart::Structural(i) => {
                structure > 0
                    && i < 32
                    && !self.structure[i]
                    && (i == 0 || self.structure[STRUCTURE_PARENT[i]])
            }
            ShipPart::Component(i) => components > 0 && i < 2 && self.components[i] < 8,
            ShipPart::Module(i) => modules > 0 && i < 3 && self.modules[i] < 4,
        }
    }

    fn place(&mut self, part: ShipPart) -> bool {
        if !self.can_place(part) {
            return false;
        }
        match part {
            ShipPart::Structural(i) => self.structure[i] = true,
            ShipPart::Component(i) => self.components[i] += 1,
            ShipPart::Module(i) => self.modules[i] += 1,
        }
        true
    }

    fn autoplace(&mut self) {
        loop {
            let [structure, components, modules] = self.unplaced();
            let part = if modules > 0 {
                let [habitation, support, solar] = self.modules;
                let preferred = if habitation == 0
                    || (habitation < support && solar * 2 > habitation + support)
                {
                    0
                } else if support == 0 {
                    1
                } else if solar == 0 || solar * 2 < habitation + support {
                    2
                } else if support < habitation || solar * 2 > habitation + support {
                    1
                } else {
                    2
                };
                let available = (0..3).find(|&i| self.modules[i] < 4).unwrap();
                ShipPart::Module(if self.modules[preferred] < 4 {
                    preferred
                } else {
                    available
                })
            } else if components > 0 {
                ShipPart::Component(usize::from(self.components[0] > self.components[1]))
            } else if structure > 0 {
                ShipPart::Structural(self.next_structural())
            } else {
                break;
            };
            assert!(self.place(part));
        }
    }

    fn next_structural(&self) -> usize {
        if !self.structure[0] {
            return 0;
        }
        let components = self.components;
        let mut required = MODULE_SUPPORT[..3]
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, _)| self.modules[*i] > 0)
            .map(|(_, socket)| socket)
            .chain(
                COMPONENT_SUPPORT
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(i, _)| components[i % 2] > i / 2)
                    .map(|(_, socket)| socket),
            )
            .chain(
                MODULE_SUPPORT
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(i, _)| self.modules[i % 3] > i / 3)
                    .map(|(_, socket)| socket),
            )
            .chain(0..32)
            .find(|&socket| !self.structure[socket])
            .unwrap();
        while !self.structure[STRUCTURE_PARENT[required]] {
            required = STRUCTURE_PARENT[required];
        }
        required
    }

    pub(super) fn score(&self) -> i32 {
        if !self.arrived {
            return 0;
        }
        let habitation = (0..self.modules[0])
            .filter(|&i| self.structure[MODULE_SUPPORT[i * 3]])
            .count();
        (habitation as f64 * 100.0 * self.flight().unwrap().1) as i32
    }

    fn flight(&self) -> Option<(f64, f64)> {
        let components = self.components;
        let mut connected_components = [0; 2];
        for (i, &socket) in COMPONENT_SUPPORT.iter().enumerate() {
            connected_components[i % 2] +=
                usize::from(components[i % 2] > i / 2 && self.structure[socket]);
        }
        let mut connected_modules = [0; 3];
        for (i, &socket) in MODULE_SUPPORT.iter().enumerate() {
            connected_modules[i % 3] +=
                usize::from(self.modules[i % 3] > i / 3 && self.structure[socket]);
        }
        let [habitation, support, solar] = connected_modules;
        let [fuel, propulsion] = connected_components;
        if [habitation, support, solar, fuel, propulsion].contains(&0) {
            return None;
        }
        let mass: usize = self
            .structure
            .iter()
            .enumerate()
            .filter(|(_, installed)| **installed)
            .map(|(i, _)| if i < 6 { 200 } else { 100 })
            .sum::<usize>()
            + 1600 * (habitation + support)
            + 400 * (solar + fuel + propulsion);
        let success = (support as f64 / habitation as f64).min(1.0)
            * (2.0 * solar as f64 / (habitation + support) as f64).min(1.0);
        Some((
            mass as f64 / (200 * fuel.min(propulsion) + 20) as f64,
            success,
        ))
    }
}

impl Simulation {
    pub(crate) fn ship_assembly(&self) -> ([usize; 3], [bool; 32], [usize; 2], [usize; 3], bool) {
        let ship = &self.spaceships[HUMAN];
        (
            ship.unplaced(),
            ship.structure,
            ship.components,
            ship.modules,
            ship.manual,
        )
    }

    pub(crate) fn can_place_ship_part(&self, part: ShipPart) -> bool {
        self.spaceships[HUMAN].can_place(part)
    }

    pub(super) fn place_ship_part(&mut self, part: ShipPart) -> bool {
        self.spaceships[HUMAN].place(part)
    }

    pub(super) fn toggle_manual_assembly(&mut self) -> bool {
        let ship = &mut self.spaceships[HUMAN];
        if !self.apollo_completed || ship.launched() {
            return false;
        }
        ship.manual = !ship.manual;
        if !ship.manual {
            ship.autoplace();
        }
        true
    }

    pub(crate) fn spaceship_status(&self, owner: usize) -> Option<String> {
        let ship = &self.spaceships[owner];
        if let Some(year) = ship.arrival {
            Some(format!("Spaceship arrives {}", year_label(year as i32)))
        } else if self.can_launch_spaceship(owner) {
            let (years, success) = ship.flight().unwrap();
            Some(format!(
                "Launch spaceship ({years:.1} yr, {:.0}%)",
                success * 100.0
            ))
        } else if self.apollo_completed {
            let [structure, components, modules] = ship.built;
            Some(format!(
                "Spaceship parts: {structure} / {components} / {modules}"
            ))
        } else {
            None
        }
    }

    pub(crate) fn can_launch_spaceship(&self, owner: usize) -> bool {
        let ship = &self.spaceships[owner];
        !ship.launched()
            && ship.flight().is_some()
            && self
                .cities
                .iter()
                .any(|city| city.owner == owner && city.has_building(Building::PALACE))
    }

    pub(super) fn launch_spaceship(&mut self, owner: usize) -> bool {
        if !self.can_launch_spaceship(owner) {
            return false;
        }
        let years = self.spaceships[owner].flight().unwrap().0;
        let arrival = f64::from(self.year) + years;
        self.spaceships[owner].arrival = Some(arrival);
        self.award_achievement(owner, super::achievements::Achievement::Spaceship);
        self.add_event(format!(
            "{} launched a spaceship. Arrival expected in {}.",
            self.players[owner].name,
            year_label(arrival as i32)
        ));
        true
    }

    pub(super) fn lose_spaceship(&mut self, owner: usize) {
        if self.spaceships[owner].built != [0; 3] {
            self.spaceships[owner] = Spaceship::default();
            self.add_event(format!(
                "{} lost its spaceship with its capital.",
                self.players[owner].name
            ));
        }
    }

    pub(super) fn advance_space_race(&mut self) {
        for owner in 1..self.players.len() {
            if self.players[owner].ai && self.spaceships[owner].built == [32, 16, 12] {
                self.launch_spaceship(owner);
            }
        }
        let arrivals: Vec<_> = self
            .spaceships
            .iter()
            .enumerate()
            .filter_map(|(owner, ship)| {
                ship.arrival
                    .filter(|&year| self.year >= year as i32 && self.player_alive(owner))
                    .map(|year| (owner, year))
            })
            .collect();
        if let Some((_, first_year)) = arrivals.iter().min_by(|a, b| a.1.total_cmp(&b.1)) {
            let tied: Vec<_> = arrivals
                .iter()
                .filter(|(_, year)| year == first_year)
                .map(|(owner, _)| *owner)
                .collect();
            let winner = tied[self.rand_below(tied.len() as i32) as usize];
            self.spaceships[winner].arrived = true;
            self.game_over = Some(format!(
                "{} reached Alpha Centauri and won the space race.",
                self.players[winner].name
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_placement_enforces_dependencies_inventory_and_launch_lock() {
        let mut ship = Spaceship {
            manual: true,
            ..Spaceship::default()
        };
        ship.add_part(0);
        ship.add_part(1);
        ship.add_part(2);
        assert!(!ship.place(ShipPart::Structural(1)));
        assert!(ship.place(ShipPart::Structural(0)));
        assert!(!ship.place(ShipPart::Structural(1)));
        assert!(ship.place(ShipPart::Component(1)));
        assert!(!ship.place(ShipPart::Component(0)));
        assert!(ship.place(ShipPart::Module(2)));
        assert_eq!(ship.components, [0, 1]);
        assert_eq!(ship.modules, [0, 0, 1]);
        ship.add_part(0);
        ship.arrival = Some(2000.0);
        assert!(!ship.place(ShipPart::Structural(1)));
    }

    #[test]
    fn flight_requires_connected_parts_and_uses_reference_mass() {
        let mut ship = Spaceship::default();
        for (kind, count) in [(2, 3), (1, 2), (0, 7)] {
            for _ in 0..count {
                ship.add_part(kind);
            }
        }
        assert_eq!(ship.flight(), None);
        ship.add_part(0);
        assert_eq!(ship.flight(), Some((5600.0 / 220.0, 1.0)));
    }

    #[test]
    fn full_spaceship_connects_every_component_and_module() {
        let mut ship = Spaceship::default();
        for (kind, count) in [(2, 12), (1, 16), (0, 32)] {
            for _ in 0..count {
                ship.add_part(kind);
            }
        }
        assert_eq!(ship.modules, [4; 3]);
        assert_eq!(ship.flight(), Some((24600.0 / 1620.0, 1.0)));
    }
}
