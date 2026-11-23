//! Conquests-style diplomacy and espionage missions.
//!
//! Civ III represents an embassy and planted spy as player-to-player assets,
//! not map units. Mission calculations are kept here so the UI never changes
//! campaign state directly.

use super::*;

const STANDARD_ESPIONAGE_TECH_RATE: i64 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EspionageMission {
    EstablishEmbassy,
    InvestigateCity,
    StealTechnology,
    PlantSpy,
    StealWorldMap,
    StealMilitaryPlans,
    SabotageProduction,
    InitiatePropaganda,
    ExposeEnemySpy,
}

impl EspionageMission {
    pub(crate) const ALL: [Self; 9] = [
        Self::EstablishEmbassy,
        Self::InvestigateCity,
        Self::StealTechnology,
        Self::PlantSpy,
        Self::StealWorldMap,
        Self::StealMilitaryPlans,
        Self::SabotageProduction,
        Self::InitiatePropaganda,
        Self::ExposeEnemySpy,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::EstablishEmbassy => "Establish Embassy",
            Self::InvestigateCity => "Investigate City",
            Self::StealTechnology => "Steal Technology",
            Self::PlantSpy => "Plant Spy",
            Self::StealWorldMap => "Steal World Map",
            Self::StealMilitaryPlans => "Steal Military Plans",
            Self::SabotageProduction => "Sabotage Production",
            Self::InitiatePropaganda => "Initiate Propaganda",
            Self::ExposeEnemySpy => "Expose Enemy Spy",
        }
    }

    pub(crate) fn needs_city(self) -> bool {
        matches!(
            self,
            Self::InvestigateCity | Self::SabotageProduction | Self::InitiatePropaganda
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EspionageRisk {
    #[default]
    Immediate,
    Carefully,
    Safely,
}

impl EspionageRisk {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Immediate => "Immediate",
            Self::Carefully => "Carefully",
            Self::Safely => "Safely",
        }
    }

    fn cost_multiplier(self) -> (i64, i64) {
        match self {
            Self::Immediate => (1, 1),
            Self::Carefully => (3, 2),
            Self::Safely => (2, 1),
        }
    }

    fn escape_chance(self, spy: bool, veteran: bool) -> i32 {
        match (spy, veteran, self) {
            (false, _, Self::Immediate) => 40,
            (false, _, Self::Carefully) => 55,
            (false, _, Self::Safely) => 65,
            (true, false, Self::Immediate) => 60,
            (true, false, Self::Carefully) => 75,
            (true, false, Self::Safely) => 85,
            (true, true, Self::Immediate) => 70,
            (true, true, Self::Carefully) => 85,
            (true, true, Self::Safely) => 95,
        }
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct SpyAsset {
    pub(crate) veteran: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct EspionageRivalView {
    pub(crate) owner: usize,
    pub(crate) name: String,
    pub(crate) contacted: bool,
    pub(crate) at_war: bool,
    pub(crate) embassy: bool,
    pub(crate) spy: bool,
    pub(crate) espionage_ready: bool,
    pub(crate) can_establish_embassy: bool,
    pub(crate) can_plant_spy: bool,
    pub(crate) cities: Vec<(u32, String)>,
    pub(crate) military_units: Vec<(String, i32, i32)>,
}

impl Simulation {
    pub(crate) fn espionage_available(&self) -> bool {
        self.has_tech(
            HUMAN,
            Tech::all().find(|tech| tech.name() == "Writing").unwrap(),
        )
    }

    pub(crate) fn espionage_rivals_view(&self) -> Vec<EspionageRivalView> {
        if !self.espionage_available() {
            return Vec::new();
        }
        (1..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .map(|owner| {
                let contacted = self.contacted_pact(HUMAN, owner).is_some();
                let at_war = self.at_war(HUMAN, owner);
                let relation = self.diplomacy.get(&Self::relation_key(HUMAN, owner));
                let direction = 0;
                let has_spy = relation.is_some_and(|relation| relation.spy[direction].is_some());
                let military_plans_active = relation
                    .is_some_and(|relation| relation.military_plans_until[direction] > self.turn);
                let military_units = if military_plans_active {
                    self.units
                        .iter()
                        .filter(|unit| unit.owner == owner && unit.kind.is_military())
                        .map(|unit| (unit.kind.name().to_owned(), unit.x, unit.y))
                        .collect()
                } else {
                    Vec::new()
                };
                let cities = self
                    .cities
                    .iter()
                    .filter(|city| city.owner == owner)
                    .map(|city| (city.id, city.name.clone()))
                    .collect();
                EspionageRivalView {
                    owner,
                    name: self.players[owner].name.clone(),
                    contacted,
                    at_war,
                    embassy: !at_war && self.has_embassy(HUMAN, owner),
                    spy: has_spy,
                    espionage_ready: self.has_tech(HUMAN, Tech::ESPIONAGE)
                        && self.has_active_intelligence_agency(HUMAN),
                    can_establish_embassy: contacted && !at_war && !self.has_embassy(HUMAN, owner),
                    can_plant_spy: self.has_tech(HUMAN, Tech::ESPIONAGE)
                        && self.has_active_intelligence_agency(HUMAN)
                        && self.has_embassy(HUMAN, owner)
                        && !has_spy
                        && !at_war
                        && relation.is_none_or(|relation| {
                            relation.failed_spy_plant_turn[direction]
                                .is_none_or(|failed_turn| self.turn > failed_turn)
                        }),
                    cities,
                    military_units,
                }
            })
            .collect()
    }

    fn has_active_intelligence_agency(&self, owner: usize) -> bool {
        self.cities
            .iter()
            .any(|city| city.owner == owner && city.has_building(Building::INTELLIGENCE_AGENCY))
    }

    pub(in crate::sim) fn run_ai_espionage(&mut self, actor: usize) {
        if actor >= self.civilization_count
            || !self.player_alive(actor)
            || !self.players[actor].ai
            || !self.has_tech(
                actor,
                Tech::all().find(|tech| tech.name() == "Writing").unwrap(),
            )
        {
            return;
        }
        let opponents: Vec<_> = (0..self.civilization_count)
            .filter(|&owner| owner != actor && self.player_alive(owner))
            .collect();
        for opponent in opponents {
            if self.contacted_pact(actor, opponent).is_none() {
                continue;
            }
            let at_war = self.at_war(actor, opponent);
            let relation_key = Self::relation_key(actor, opponent);
            let direction = usize::from(actor > opponent);
            let relation = self.diplomacy.get(&relation_key);
            let spy = relation.is_some_and(|relation| relation.spy[direction].is_some());
            let failed_plant = relation
                .is_some_and(|relation| relation.failed_spy_plant_turn[direction].is_some());
            let embassy = !at_war && self.has_embassy(actor, opponent);

            if !at_war && !embassy {
                let capital = self
                    .cities
                    .iter()
                    .find(|city| city.owner == opponent && city.has_building(Building::PALACE))
                    .or_else(|| self.cities.iter().find(|city| city.owner == opponent));
                if let Some(capital) = capital {
                    let cost = self.espionage_cost(
                        actor,
                        opponent,
                        capital,
                        EspionageMission::EstablishEmbassy,
                        EspionageRisk::Immediate,
                    );
                    if self.players[actor].gold >= cost.saturating_add(100)
                        && self.rand_below(8) == 0
                    {
                        self.perform_espionage_for(
                            actor,
                            opponent,
                            EspionageMission::EstablishEmbassy,
                            None,
                            EspionageRisk::Immediate,
                        );
                        continue;
                    }
                }
            }

            if !at_war
                && embassy
                && !spy
                && !failed_plant
                && self.has_tech(actor, Tech::ESPIONAGE)
                && self.has_active_intelligence_agency(actor)
            {
                let capital = self
                    .cities
                    .iter()
                    .find(|city| city.owner == opponent && city.has_building(Building::PALACE))
                    .or_else(|| self.cities.iter().find(|city| city.owner == opponent));
                let cost = capital.map_or(60, |city| {
                    self.espionage_cost(
                        actor,
                        opponent,
                        city,
                        EspionageMission::PlantSpy,
                        EspionageRisk::Immediate,
                    )
                });
                if self.players[actor].gold >= cost.saturating_add(100) && self.rand_below(8) == 0 {
                    self.perform_espionage_for(
                        actor,
                        opponent,
                        EspionageMission::PlantSpy,
                        None,
                        EspionageRisk::Immediate,
                    );
                    continue;
                }
            }

            if !spy
                || !self.has_tech(actor, Tech::ESPIONAGE)
                || !self.has_active_intelligence_agency(actor)
                || self.players[actor].gold < 150
                || self.rand_below(4) != 0
            {
                continue;
            }
            let mut missions = Vec::new();
            if Tech::all().any(|tech| {
                tech.is_researchable_advance()
                    && self.has_tech(opponent, tech)
                    && !self.has_tech(actor, tech)
            }) {
                missions.push((EspionageMission::StealTechnology, None));
            }
            if self
                .units
                .iter()
                .any(|unit| unit.owner == opponent && unit.kind.is_military())
            {
                missions.push((EspionageMission::StealMilitaryPlans, None));
            }
            if self
                .tiles
                .iter()
                .any(|tile| tile.known_by[opponent] && !tile.known_by[actor])
            {
                missions.push((EspionageMission::StealWorldMap, None));
            }
            let target_city = self
                .cities
                .iter()
                .filter(|city| city.owner == opponent)
                .max_by_key(|city| (city.shields, city.size));
            if let Some(city) = target_city {
                if city.shields > 0 {
                    missions.push((EspionageMission::SabotageProduction, Some(city.id)));
                }
                if self.players[opponent].government != Government::DEMOCRACY {
                    missions.push((EspionageMission::InitiatePropaganda, Some(city.id)));
                }
            }
            let reverse = usize::from(opponent > actor);
            if self
                .diplomacy
                .get(&relation_key)
                .is_some_and(|relation| relation.spy[reverse].is_some())
            {
                missions.push((EspionageMission::ExposeEnemySpy, None));
            }
            if !missions.is_empty() {
                let (mission, city) = missions[self.rand_below(missions.len() as i32) as usize];
                self.perform_espionage_for(
                    actor,
                    opponent,
                    mission,
                    city,
                    EspionageRisk::Immediate,
                );
            }
        }
    }

    pub(crate) fn perform_espionage(
        &mut self,
        opponent: usize,
        mission: EspionageMission,
        city_id: Option<u32>,
        risk: EspionageRisk,
    ) -> bool {
        self.perform_espionage_for(HUMAN, opponent, mission, city_id, risk)
    }

    fn perform_espionage_for(
        &mut self,
        actor: usize,
        opponent: usize,
        mission: EspionageMission,
        city_id: Option<u32>,
        risk: EspionageRisk,
    ) -> bool {
        let fail = |game: &mut Self, message: &str| {
            game.add_espionage_event(actor, opponent, message.to_owned());
            false
        };
        if actor >= self.players.len()
            || actor == BARBARIANS
            || opponent >= self.players.len()
            || opponent == actor
            || opponent == BARBARIANS
            || !self.player_alive(opponent)
            || !self.player_alive(actor)
            || !self.has_tech(
                actor,
                Tech::all().find(|tech| tech.name() == "Writing").unwrap(),
            )
        {
            return fail(self, "That espionage action is not available.");
        }
        let Some(pact) = self.contacted_pact(actor, opponent) else {
            return fail(
                self,
                "You must meet a civilization before taking diplomatic action.",
            );
        };
        let at_war = pact == Pact::War;
        let direction = usize::from(actor > opponent);
        let relation_key = Self::relation_key(actor, opponent);
        // Conquests embassies close during war and become available again at
        // peace; keep the stored asset so a peace treaty reopens it.
        let embassy = !at_war && self.has_embassy(actor, opponent);
        let spy = self
            .diplomacy
            .get(&relation_key)
            .and_then(|relation| relation.spy[direction]);
        let target = city_id.and_then(|id| self.cities.iter().find(|city| city.id == id));
        if mission.needs_city() && target.is_none_or(|city| city.owner != opponent) {
            return fail(self, "Select a city belonging to the chosen civilization.");
        }

        if mission == EspionageMission::EstablishEmbassy {
            if at_war || embassy {
                return fail(
                    self,
                    "An embassy can only be established with a contacted civilization at peace.",
                );
            }
            let capital = self
                .cities
                .iter()
                .find(|city| city.owner == opponent && city.has_building(Building::PALACE))
                .or_else(|| self.cities.iter().find(|city| city.owner == opponent))
                .cloned();
            let cost = capital.as_ref().map_or(20, |city| {
                self.espionage_cost(actor, opponent, city, mission, risk)
            });
            if !self.spend_espionage_gold(actor, cost) {
                return fail(self, "Your treasury cannot cover the embassy cost.");
            }
            self.diplomacy.entry(relation_key).or_default().embassy[direction] = true;
            self.add_espionage_event(
                actor,
                opponent,
                format!(
                    "{} embassy established for {cost} gold.",
                    self.players[opponent].name
                ),
            );
            if let Some(capital) = capital.as_ref() {
                self.report_city_intelligence(actor, capital, None);
            }
            return true;
        }

        if mission == EspionageMission::PlantSpy {
            if !self.has_tech(actor, Tech::ESPIONAGE)
                || !self.has_active_intelligence_agency(actor)
                || !embassy
                || at_war
                || spy.is_some()
            {
                return fail(
                    self,
                    "Planting a Spy requires an embassy, Espionage, an Intelligence Agency, peace, and no existing Spy in that civilization.",
                );
            }
            if self
                .diplomacy
                .get(&relation_key)
                .and_then(|relation| relation.failed_spy_plant_turn[direction])
                .is_some()
            {
                return fail(
                    self,
                    "A failed planting attempt is still being investigated.",
                );
            }
            let capital = self
                .cities
                .iter()
                .find(|city| city.owner == opponent && city.has_building(Building::PALACE))
                .or_else(|| self.cities.iter().find(|city| city.owner == opponent));
            let cost = capital.map_or(60, |city| {
                self.espionage_cost(actor, opponent, city, mission, risk)
            });
            if !self.spend_espionage_gold(actor, cost) {
                return fail(self, "Your treasury cannot cover the Spy planting cost.");
            }
            let veteran = matches!(
                self.players[actor].government,
                Government::COMMUNISM | Government::FASCISM
            );
            let success = self.rand_below(100) < if veteran { 60 } else { 50 };
            let relation = self.diplomacy.entry(relation_key).or_default();
            if success {
                relation.spy[direction] = Some(SpyAsset { veteran });
                relation.failed_spy_plant_turn[direction] = None;
                self.add_espionage_event(
                    actor,
                    opponent,
                    format!(
                        "A Spy was planted in {} for {cost} gold.",
                        self.players[opponent].name
                    ),
                );
            } else {
                relation.spy[direction] = None;
                relation.failed_spy_plant_turn[direction] = Some(self.turn);
                self.record_espionage_discovery(opponent, actor);
                self.add_espionage_event(
                    actor,
                    opponent,
                    format!(
                        "The attempt to plant a Spy in {} failed; the operation was discovered.",
                        self.players[opponent].name
                    ),
                );
            }
            return true;
        }

        let requires_espionage = !matches!(
            mission,
            EspionageMission::EstablishEmbassy | EspionageMission::InvestigateCity
        );
        if requires_espionage
            && (!self.has_tech(actor, Tech::ESPIONAGE)
                || !self.has_active_intelligence_agency(actor))
        {
            return fail(
                self,
                "This mission requires Espionage and an Intelligence Agency.",
            );
        }
        if mission == EspionageMission::InvestigateCity {
            if !embassy || at_war {
                return fail(
                    self,
                    "Investigating a city requires an open embassy and peace.",
                );
            }
        } else if matches!(
            mission,
            EspionageMission::StealTechnology
                | EspionageMission::StealWorldMap
                | EspionageMission::StealMilitaryPlans
                | EspionageMission::SabotageProduction
        ) {
            if !embassy && spy.is_none() {
                return fail(self, "This mission requires an embassy or a planted Spy.");
            }
        } else if spy.is_none() {
            return fail(self, "This mission requires a planted Spy.");
        }

        let city = target.cloned();
        let cost = city.as_ref().map_or_else(
            || self.espionage_cost_for_opponent(actor, opponent, mission, risk),
            |city| self.espionage_cost(actor, opponent, city, mission, risk),
        );
        if !self.spend_espionage_gold(actor, cost) {
            return fail(self, "Your treasury cannot cover that mission.");
        }

        if mission == EspionageMission::InvestigateCity {
            let city = city.unwrap();
            self.report_city_intelligence(actor, &city, Some(cost));
            return true;
        }

        if mission == EspionageMission::InitiatePropaganda {
            let city_id = city.as_ref().unwrap().id;
            return self.execute_propaganda(
                opponent,
                actor,
                city_id,
                cost,
                spy.is_some_and(|asset| asset.veteran),
            );
        }

        let escape_chance =
            risk.escape_chance(spy.is_some(), spy.is_some_and(|asset| asset.veteran));
        let escaped = self.rand_below(100) < escape_chance;
        let succeeded = escaped && self.rand_below(100) < 80;
        if !escaped {
            self.diplomacy.entry(relation_key).or_default().spy[direction] = None;
            self.record_espionage_discovery(opponent, actor);
            let loss = if spy.is_some() {
                "Your Spy was lost."
            } else {
                "No Spy was lost."
            };
            self.add_espionage_event(
                actor,
                opponent,
                format!(
                    "The {} mission was discovered in {}. {loss}",
                    mission.name(),
                    self.players[opponent].name
                ),
            );
            return true;
        }
        if !succeeded {
            self.add_espionage_event(
                actor,
                opponent,
                format!(
                    "The {} mission failed in {} without discovery.",
                    mission.name(),
                    self.players[opponent].name
                ),
            );
            return true;
        }

        match mission {
            EspionageMission::StealTechnology => {
                let eligible: Vec<_> = Tech::all()
                    .filter(|&tech| {
                        tech.is_researchable_advance()
                            && self.has_tech(opponent, tech)
                            && !self.has_tech(actor, tech)
                    })
                    .collect();
                if eligible.is_empty() {
                    self.add_espionage_event(
                        actor,
                        opponent,
                        "The intelligence mission found no available technology to steal."
                            .to_owned(),
                    );
                    return true;
                }
                let tech = eligible[self.rand_below(eligible.len() as i32) as usize];
                self.discover_with_source(
                    actor,
                    tech,
                    research_observation::TechnologySource::Espionage,
                    Some(opponent),
                );
                self.add_espionage_event(
                    actor,
                    opponent,
                    format!(
                        "Espionage acquired {} from {} for {cost} gold.",
                        tech.name(),
                        self.players[opponent].name
                    ),
                );
            }
            EspionageMission::StealWorldMap => {
                self.transfer_treaty_map(opponent, actor, TreatyMap::World);
                self.refresh_visibility();
                self.add_espionage_event(
                    actor,
                    opponent,
                    format!(
                        "Your Spy stole {}'s world map for {cost} gold.",
                        self.players[opponent].name
                    ),
                );
            }
            EspionageMission::StealMilitaryPlans => {
                self.diplomacy
                    .entry(relation_key)
                    .or_default()
                    .military_plans_until[direction] = self.turn.saturating_add(1);
                self.add_espionage_event(actor, opponent, format!("Your Spy stole {}'s military plans. Enemy unit locations are available for this turn.", self.players[opponent].name));
            }
            EspionageMission::SabotageProduction => {
                let target = city.unwrap();
                let Some(index) = self
                    .cities
                    .iter()
                    .position(|candidate| candidate.id == target.id)
                else {
                    return false;
                };
                self.cities[index].shields = 0;
                self.add_espionage_event(
                    actor,
                    opponent,
                    format!(
                        "Your Spy sabotaged production in {} for {cost} gold.",
                        target.name
                    ),
                );
            }
            EspionageMission::ExposeEnemySpy => {
                let reverse = usize::from(opponent > actor);
                if let Some(relation) = self.diplomacy.get_mut(&relation_key) {
                    if relation.spy[reverse].is_some() {
                        relation.spy[reverse] = None;
                        self.add_espionage_event(
                            actor,
                            opponent,
                            format!(
                                "Your counterespionage operation exposed {}'s Spy.",
                                self.players[opponent].name
                            ),
                        );
                    } else {
                        self.add_espionage_event(
                            actor,
                            opponent,
                            "The counterespionage operation found no enemy Spy.".to_owned(),
                        );
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn execute_propaganda(
        &mut self,
        opponent: usize,
        actor: usize,
        city_id: u32,
        cost: i32,
        veteran: bool,
    ) -> bool {
        let Some(index) = self
            .cities
            .iter()
            .position(|city| city.id == city_id && city.owner == opponent)
        else {
            return false;
        };
        let city = self.cities[index].clone();
        if self.players[opponent].government == Government::DEMOCRACY {
            self.add_espionage_event(
                actor,
                opponent,
                format!(
                    "Propaganda cannot persuade citizens of democratic {}.",
                    city.name
                ),
            );
            return true;
        }
        let culture_modifier = propaganda_culture_modifier(
            self.civilization_culture(actor),
            self.civilization_culture(opponent),
        );
        let government_modifier = propaganda_government_modifier(
            self.players[opponent].government,
            self.players[actor].government,
        );
        let garrison = self
            .units
            .iter()
            .filter(|unit| {
                unit.owner == opponent
                    && unit.x == city.x
                    && unit.y == city.y
                    && unit.kind.is_military()
            })
            .count() as i32;
        let modifier = government_modifier + culture_modifier + if veteran { 10 } else { 0 }
            - 5 * garrison
            - if city.has_building(Building::COURTHOUSE) {
                20
            } else {
                0
            }
            - if city.has_building(Building::PALACE) {
                40
            } else {
                0
            };
        let human_nationals = city
            .foreign_citizens
            .iter()
            .find(|(nation, _)| *nation == actor)
            .map_or(0, |(_, count)| *count);
        let influenced = (0..city.size)
            .filter(|citizen| {
                let nationality_bonus = if *citizen < human_nationals { 20 } else { 0 };
                self.rand_below(100) < (modifier + nationality_bonus).clamp(0, 100)
            })
            .count() as i32;
        if influenced * 4 >= city.size * 3 {
            self.transfer_city_because(index, actor, lifecycle::TransferReason::Propaganda);
            self.add_espionage_event(
                actor,
                opponent,
                format!(
                    "Propaganda succeeded. {} joined your civilization for {cost} gold.",
                    city.name
                ),
            );
        } else {
            let index = self
                .cities
                .iter()
                .position(|candidate| candidate.id == city_id)
                .unwrap();
            self.cities[index].confused_citizens =
                self.cities[index].confused_citizens.max(influenced);
            self.add_espionage_event(actor, opponent, format!("Propaganda failed to turn {}. {influenced} citizen(s) are confused for the coming turn.", city.name));
        }
        true
    }

    fn report_city_intelligence(&mut self, actor: usize, city: &City, cost: Option<i32>) {
        let garrison = self
            .units
            .iter()
            .filter(|unit| unit.owner == city.owner && unit.x == city.x && unit.y == city.y)
            .count();
        let buildings = city
            .buildings
            .iter()
            .map(|building| building.data().name)
            .collect::<Vec<_>>()
            .join(", ");
        let cost = cost.map_or_else(String::new, |gold| format!(" Cost: {gold} gold."));
        if actor == HUMAN {
            self.add_event(format!(
                "Intelligence report: {} (size {}) produces {}; {} shield(s) invested; {garrison} unit(s); buildings: {}.{cost}",
                city.name, city.size, city.production.name(), city.shields, if buildings.is_empty() { "none" } else { &buildings }
            ));
        }
    }

    fn add_espionage_event(&mut self, actor: usize, _opponent: usize, message: String) {
        if actor == HUMAN {
            self.add_event(message);
        }
    }

    fn record_espionage_discovery(&mut self, victim: usize, offender: usize) {
        self.record_provocation(victim, offender);
        self.record_diplomatic_incident(
            offender,
            Some(victim),
            super::DiplomaticIncident::Snooping,
            false,
        );
        if victim == HUMAN && offender != HUMAN {
            self.add_event(format!(
                "Espionage alert: {} was caught conducting a covert operation.",
                self.players[offender].name
            ));
        }
    }

    fn spend_espionage_gold(&mut self, owner: usize, cost: i32) -> bool {
        if cost < 0 || self.players[owner].gold < cost {
            return false;
        }
        self.players[owner].gold -= cost;
        true
    }

    fn espionage_cost_for_opponent(
        &self,
        actor: usize,
        opponent: usize,
        mission: EspionageMission,
        risk: EspionageRisk,
    ) -> i32 {
        let capital = self
            .cities
            .iter()
            .find(|city| city.owner == opponent && city.has_building(Building::PALACE))
            .or_else(|| self.cities.iter().find(|city| city.owner == opponent));
        capital.map_or(10, |city| {
            self.espionage_cost(actor, opponent, city, mission, risk)
        })
    }

    fn espionage_cost(
        &self,
        actor: usize,
        opponent: usize,
        city: &City,
        mission: EspionageMission,
        risk: EspionageRisk,
    ) -> i32 {
        let distance = self
            .cities
            .iter()
            .filter(|own| own.owner == actor)
            .map(|own| self.map_distance(own.x, own.y, city.x, city.y))
            .min()
            .unwrap_or(0) as i64;
        let level = if city.size <= 6 {
            1
        } else if city.size <= 12 {
            2
        } else {
            3
        };
        let pop = i64::from(city.size);
        let base = match mission {
            EspionageMission::EstablishEmbassy => 20,
            EspionageMission::InvestigateCity => 10,
            EspionageMission::StealTechnology => 10,
            EspionageMission::PlantSpy => 60,
            EspionageMission::StealWorldMap => 1,
            EspionageMission::StealMilitaryPlans => 10,
            EspionageMission::SabotageProduction => 10,
            EspionageMission::InitiatePropaganda => 100,
            EspionageMission::ExposeEnemySpy => 80,
        };
        let scale = (distance * i64::from(level)).saturating_add(base);
        let factor = match mission {
            EspionageMission::EstablishEmbassy => scale.saturating_add(pop),
            EspionageMission::InvestigateCity => distance * i64::from(level) + base * pop,
            EspionageMission::StealTechnology => {
                distance * i64::from(level)
                    + base
                        * self.players[opponent].known.len() as i64
                        * STANDARD_ESPIONAGE_TECH_RATE
                        / 100
            }
            EspionageMission::PlantSpy | EspionageMission::ExposeEnemySpy => scale,
            EspionageMission::StealWorldMap => {
                distance * i64::from(level)
                    + base * ((self.map_wrap.width() + self.map_wrap.height()) / 2) as i64
            }
            EspionageMission::StealMilitaryPlans => {
                distance * i64::from(level)
                    + base
                        * self
                            .units
                            .iter()
                            .filter(|unit| unit.owner == opponent)
                            .count() as i64
            }
            EspionageMission::SabotageProduction => {
                distance * i64::from(level) + base * i64::from(city.shields.max(0))
            }
            EspionageMission::InitiatePropaganda => {
                let own_culture = i128::from(self.civilization_culture(actor).max(1));
                let rival_culture = i128::from(self.civilization_culture(opponent).max(0));
                // The price follows what the city holds, not just its size, so a
                // small wonder city is not a bargain. Wonders count double.
                let holdings: i64 =
                    city.buildings
                        .iter()
                        .filter(|building| {
                            !matches!(building.data().genus, crate::rules::Genus::Special)
                                && **building != Building::PALACE
                        })
                        .map(|building| {
                            let data = building.data();
                            let weight = match data.genus {
                                crate::rules::Genus::SmallWonder
                                | crate::rules::Genus::GreatWonder => 2,
                                _ => 1,
                            };
                            i64::from(data.cost.max(0)) * weight
                        })
                        .sum();
                distance * i64::from(level)
                    + ((i128::from(base * pop + holdings) * (own_culture + rival_culture)
                        / own_culture)
                        .min(i128::from(i64::MAX)) as i64)
            }
        };
        let (num, den) = risk.cost_multiplier();
        ((factor.saturating_mul(num) / den).clamp(1, i64::from(i32::MAX))) as i32
    }
}

fn propaganda_government_modifier(attacker: Government, defender: Government) -> i32 {
    let row = match attacker {
        Government::DESPOTISM => [15, 0, -10, -15, -20, -30, 0, 0],
        Government::MONARCHY => [20, 15, 0, -5, -10, -25, 0, 0],
        Government::COMMUNISM => [25, 20, 5, 0, -10, -20, 0, 0],
        Government::REPUBLIC => [30, 25, 10, 8, 0, -5, 0, 0],
        Government::DEMOCRACY => [35, 30, 20, 10, 5, 0, 0, 0],
        Government::FASCISM => [0, 0, -5, -20, -10, -10, 0, 0],
        Government::FEUDALISM => [0, 15, 0, -10, -10, -20, -25, 0],
        _ => return 0,
    };
    row.get(defender.index()).copied().unwrap_or(0)
}

fn propaganda_culture_modifier(attacker_culture: i64, defender_culture: i64) -> i32 {
    let attacker = i128::from(attacker_culture.max(0));
    let defender = i128::from(defender_culture.max(0));
    if attacker == 0 {
        3
    } else if defender == 0 || attacker >= defender * 3 {
        30
    } else if attacker >= defender * 2 {
        25
    } else if attacker >= defender {
        20
    } else if attacker * 4 >= defender * 3 {
        10
    } else if attacker * 2 >= defender {
        5
    } else {
        3
    }
}

#[cfg(test)]
#[path = "espionage/tests.rs"]
mod tests;
