use super::*;

mod observer;
mod offers;
mod vision;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Pact {
    #[default]
    War,
    Ceasefire,
    Armistice,
    Peace,
    Alliance,
}

impl Pact {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::War => "War",
            Self::Ceasefire => "Ceasefire",
            Self::Armistice => "Armistice",
            Self::Peace => "Peace",
            Self::Alliance => "Alliance",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TreatyMap {
    World,
    Sea,
}

impl TreatyMap {
    fn flag(self) -> u64 {
        match self {
            Self::World => 2,
            Self::Sea => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Concessions {
    pub(crate) gold: i32,
    pub(crate) technology: Option<Tech>,
    // Preserve the legacy u64 wire: flags0..2, city ID3..34. Embassy35
    // extends it without changing legacy fields or city identifiers.
    access: u64,
}

impl Concessions {
    const CITY_SHIFT: u32 = 3;
    const CITY_MASK: u64 = (u32::MAX as u64) << Self::CITY_SHIFT;
    const EMBASSY_FLAG: u64 = 1 << 35;
    const VALID_MASK: u64 = 7 | Self::CITY_MASK | Self::EMBASSY_FLAG;

    pub(crate) fn shared_vision(self) -> bool {
        self.access & 1 != 0
    }

    pub(crate) fn embassy(self) -> bool {
        self.access & Self::EMBASSY_FLAG != 0
    }

    fn with_embassy(mut self) -> Self {
        self.access |= Self::EMBASSY_FLAG;
        self
    }

    pub(crate) fn city(self) -> Option<u32> {
        let id = ((self.access & Self::CITY_MASK) >> Self::CITY_SHIFT) as u32;
        (id != 0).then_some(id)
    }

    fn with_city(mut self, id: u32) -> Self {
        self.access = (self.access & !Self::CITY_MASK) | (u64::from(id) << Self::CITY_SHIFT);
        self
    }

    fn valid_access(self) -> bool {
        self.access & 7 <= 5 && self.access & !Self::VALID_MASK == 0
    }

    pub(crate) fn map(self) -> Option<TreatyMap> {
        [TreatyMap::World, TreatyMap::Sea]
            .into_iter()
            .find(|map| self.access & map.flag() != 0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TreatyOffer {
    pub(crate) pact: Option<Pact>,
    // Terms are ordered by player ID.
    pub(crate) terms: [Concessions; 2],
}

impl TreatyOffer {
    pub(crate) fn bundled(self) -> bool {
        self.terms != [Concessions::default(); 2]
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) enum WarReason {
    Spoils,
    Hatred,
    Excuse,
    Alliance,
    Space,
    Behavior,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct WarPlan {
    pub(super) countdown: i32,
    pub(super) reason: WarReason,
}

// ai/aitraits.c clips the effective trait before daidiplomacy.c computes
// its threshold in love units. Our diplomatic trust uses ten units per love.
fn aggression_war_threshold(value: i32) -> i32 {
    let aggression = value.clamp(1, 2500) as f32;
    ((1000.0 * (0.70 + aggression.sqrt() / 50.0 / 2.0) - 1000.0) as i32) * 10
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct DiplomaticIntel {
    allied_with_enemy: Option<usize>,
    at_war_with_ally: Option<usize>,
    allied_with_ally: Option<usize>,
}

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct DiplomaticMemory {
    pub(super) spam: i32,
    pub(super) asked_peace: i32,
    pub(super) asked_alliance: i32,
    pub(super) asked_ceasefire: i32,
    pub(super) strongest_pact: Pact,
}

impl DiplomaticMemory {
    fn for_pair(a: usize, b: usize) -> Self {
        // daidata.c:dai_diplomacy_defaults starts spam deterministically.
        Self {
            spam: ((a + b) % 5) as i32,
            ..Self::default()
        }
    }
}

// Retain source incident categories even where the bundled rules do not
// currently grant a casus belli for that action.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(super) enum DiplomaticIncident {
    Snooping,
    Theft,
    UnitWeakening,
    UnitLoss,
    CityLoss,
    BuildingLoss,
    PopulationLoss,
    TerrainLoss,
    ExtraLoss,
    Misc,
    Nuclear,
}

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Relation {
    pact: Pact,
    until: i32,
    first_contact: Option<i32>,
    cause_until: [i32; 2],
    trust: [i32; 2],
    offer: Option<TreatyOffer>,
    vision: [bool; 2],
    embassy: [bool; 2],
    offer_after: i32,
}

impl Simulation {
    pub(super) fn relation_key(a: usize, b: usize) -> (usize, usize) {
        (a.min(b), a.max(b))
    }

    fn has_real_embassy(&self, owner: usize, other: usize) -> bool {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_some_and(|relation| relation.embassy[usize::from(owner > other)])
    }

    pub(crate) fn has_embassy(&self, owner: usize, other: usize) -> bool {
        owner != other
            && other != BARBARIANS
            && (self.has_real_embassy(owner, other)
                || self.has_active_wonder(owner, Building::MARCO_POLOS_EMBASSY))
    }

    pub(super) fn clear_outgoing_embassies(&mut self, owner: usize) {
        for (&(a, b), relation) in &mut self.diplomacy {
            if owner == a {
                relation.embassy[0] = false;
            } else if owner == b {
                relation.embassy[1] = false;
            }
        }
    }

    pub(crate) fn can_establish_embassy(&self, unit: &Unit, city: &City) -> bool {
        (unit.kind.is_diplomatic() || unit.kind == UnitKind::Explorer)
            && unit.moves_left > 0
            && unit.transport.is_none()
            && city.owner != unit.owner
            && city.owner != BARBARIANS
            && self.map_distance(unit.x, unit.y, city.x, city.y) <= 1
            && !self.has_real_embassy(unit.owner, city.owner)
    }

    pub(super) fn establish_embassy(&mut self, unit_id: u32, city_id: u32) -> bool {
        let Some(unit) = self.units.iter().find(|unit| unit.id == unit_id) else {
            return false;
        };
        let Some(city) = self.cities.iter().find(|city| city.id == city_id) else {
            return false;
        };
        if !self.can_establish_embassy(unit, city) {
            return false;
        }
        let (owner, other) = (unit.owner, city.owner);
        let spy = unit.kind == UnitKind::Spy;
        let message = format!("An embassy was established in {}.", city.name);
        self.make_diplomatic_contact(owner, other);
        let relation = self
            .diplomacy
            .entry(Self::relation_key(owner, other))
            .or_default();
        relation.embassy[usize::from(owner > other)] = true;
        relation.first_contact.get_or_insert(self.turn);
        if spy {
            self.units
                .iter_mut()
                .find(|unit| unit.id == unit_id)
                .unwrap()
                .moves_left -= 1;
        } else {
            self.units.retain(|unit| unit.id != unit_id);
            self.prune_unit_ai_state();
        }
        if owner == HUMAN {
            self.add_event(message);
        }
        self.refresh_visibility();
        true
    }

    pub(crate) fn pact(&self, a: usize, b: usize) -> Pact {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(Pact::War, |relation| relation.pact)
    }

    pub(crate) fn at_war(&self, a: usize, b: usize) -> bool {
        a != b && self.pact(a, b) == Pact::War
    }

    pub(crate) fn allied(&self, a: usize, b: usize) -> bool {
        a == b || self.pact(a, b) == Pact::Alliance
    }

    pub(crate) fn pact_turns(&self, a: usize, b: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(a, b))
            .map_or(0, |relation| (relation.until - self.turn).max(0))
    }

    pub(crate) fn next_pact(&self, a: usize, b: usize) -> Option<Pact> {
        match self.pact(a, b) {
            Pact::War => Some(Pact::Ceasefire),
            Pact::Ceasefire if self.pact_turns(a, b) <= 4 => Some(Pact::Peace),
            Pact::Peace => Some(Pact::Alliance),
            _ => None,
        }
    }

    pub(crate) fn has_senate(&self, owner: usize) -> bool {
        if self.has_active_wonder(owner, Building::STATUE_OF_LIBERTY) {
            return false;
        }
        matches!(
            self.players[owner].government,
            Government::Federation | Government::Democracy
        ) || (self
            .cities
            .iter()
            .any(|city| city.has_building(Building::UNITED_NATIONS))
            && self
                .completed_wonders
                .iter()
                .any(|building| *building == Building::MANHATTAN_PROJECT)
            && !self.apollo_completed
            && self.players[owner]
                .known
                .iter()
                .any(|tech| *tech == Tech::NUCLEAR_FISSION))
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

    pub(super) fn ai_on_war_footing(&self, owner: usize) -> bool {
        self.ai_war_plans.iter().any(|(&(planner, target), plan)| {
            planner == owner && plan.countdown >= 0 && self.player_alive(target)
        })
    }

    pub(super) fn ai_potentially_hostile(&self, owner: usize, other: usize) -> bool {
        owner != other
            && (self.at_war(owner, other)
                || self.ai_war_countdown(owner, other) >= 0
                || self
                    .diplomacy
                    .get(&Self::relation_key(owner, other))
                    .is_none_or(|relation| relation.first_contact.is_none()))
    }

    pub(super) fn advisor_player_dangerous(&self, owner: usize, other: usize) -> bool {
        // advdata.c:adv_is_player_dangerous and the default-AI override.
        // Do not substitute the stricter military target predicate or the
        // alive/contact-filtered default-AI diplomatic intelligence cache.
        if self.players[owner].ai && self.ai_war_countdown(owner, other) >= 0 {
            return true;
        }
        if owner == other {
            return false;
        }
        if self.diplomatic_at_war(owner, other) || self.pact(owner, other) == Pact::Ceasefire {
            return true;
        }
        let allied_with_enemy = (0..self.players.len())
            .any(|third| self.allied(other, third) && self.diplomatic_at_war(owner, third));
        if allied_with_enemy {
            return true;
        }
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_none_or(|relation| {
                let direction = usize::from(owner > other);
                relation.cause_until[direction] > self.turn || relation.trust[direction] < 1000
            })
    }

    fn ai_war_countdown(&self, owner: usize, target: usize) -> i32 {
        self.ai_war_plans
            .get(&(owner, target))
            .map_or(-1, |plan| plan.countdown)
    }

    fn begin_ai_war_plan(&mut self, owner: usize, target: usize, delay: i32, reason: WarReason) {
        if self.ai_war_countdown(owner, target) != -1 {
            return;
        }
        self.ai_war_plans.insert(
            (owner, target),
            WarPlan {
                countdown: delay,
                reason,
            },
        );
        if target == HUMAN {
            self.add_event(format!(
                "{} is preparing for war with you in {delay} turns.",
                self.players[owner].name
            ));
        }
    }

    fn ai_space_race_leader(&self) -> Option<usize> {
        (0..self.players.len())
            .filter(|&owner| owner != BARBARIANS && self.player_alive(owner))
            .filter(|&owner| self.spaceships[owner].built.iter().any(|&n| n > 0))
            .min_by_key(|&owner| match self.spaceships[owner].arrival_year() {
                Some(year) => (0, year as i32),
                None => (1, 0),
            })
    }

    fn advance_ai_war_plans(&mut self) {
        let map_size = ((self.map_wrap.width() * self.map_wrap.height()) / 1000).max(1);
        let space_leader = self.ai_space_race_leader();
        let pairs: Vec<_> = self.diplomacy.keys().copied().collect();
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.players[owner].ai || !self.player_alive(owner) {
                continue;
            }
            let contacts: Vec<_> = pairs
                .iter()
                .filter_map(|&(a, b)| {
                    let target = if a == owner {
                        b
                    } else if b == owner {
                        a
                    } else {
                        return None;
                    };
                    (target != BARBARIANS
                        && self.player_alive(target)
                        && self.diplomacy[&(a, b)].first_contact.is_some())
                    .then_some(target)
                })
                .collect();

            for &target in &contacts {
                let relation = &self.diplomacy[&Self::relation_key(owner, target)];
                let side = usize::from(owner > target);
                if !self.at_war(owner, target)
                    && relation.trust[side] < 0
                    && relation.cause_until[side] - self.turn >= 2
                {
                    self.begin_ai_war_plan(owner, target, map_size, WarReason::Behavior);
                }
            }
            if space_leader != Some(owner) {
                for &target in &contacts {
                    if self.at_war(owner, target)
                        || self.ai_war_countdown(owner, target) != -1
                        || !self.spaceships[target].built.iter().any(|&n| n > 0)
                    {
                        continue;
                    }
                    if !self.spaceships[target].launched() {
                        if self
                            .ai_space_warnings
                            .get(&(owner, target))
                            .copied()
                            .unwrap_or(0)
                            == 0
                        {
                            let side = usize::from(owner > target);
                            let relation = self
                                .diplomacy
                                .get_mut(&Self::relation_key(owner, target))
                                .unwrap();
                            relation.trust[side] = (relation.trust[side] - 1000).max(-10_000);
                            let delay = 10 + self.rand_below(6) as i32;
                            self.ai_space_warnings.insert((owner, target), delay);
                            if target == HUMAN {
                                self.add_event(format!("{} warns that continued spaceship construction may lead to war.", self.players[owner].name));
                            }
                        }
                    } else if space_leader == Some(target) {
                        let allied = self.allied(owner, target);
                        let side = usize::from(owner > target);
                        let relation = self
                            .diplomacy
                            .get_mut(&Self::relation_key(owner, target))
                            .unwrap();
                        if allied {
                            relation.trust[side] = -10_000;
                            relation.vision[side] = false;
                        }
                        relation.trust[side] = (relation.trust[side] - 5000).max(-10_000);
                        if allied && !self.senate_blocks_cancellation(owner, target) {
                            self.cancel_pact(owner, target);
                        }
                        self.begin_ai_war_plan(owner, target, 2, WarReason::Space);
                    }
                }
            }

            // classic/nations.ruleset supplies a fixed default of 50; current
            // fixed civilization identities have no trait overrides or modifiers.
            let war_threshold = aggression_war_threshold(50);
            let target = contacts
                .iter()
                .copied()
                .filter(|&target| {
                    let relation = &self.diplomacy[&Self::relation_key(owner, target)];
                    !self.at_war(owner, target) && self.turn - relation.first_contact.unwrap() > 15
                })
                .min_by_key(|&target| {
                    self.diplomacy[&Self::relation_key(owner, target)].trust
                        [usize::from(owner > target)]
                });
            let already_at_war = contacts.iter().any(|&target| self.at_war(owner, target));
            if !already_at_war
                && let Some(target) = target
                && self.diplomacy[&Self::relation_key(owner, target)].trust
                    [usize::from(owner > target)]
                    < war_threshold
            {
                let relation = &self.diplomacy[&Self::relation_key(owner, target)];
                let reason = if relation.cause_until[usize::from(owner > target)] > self.turn {
                    WarReason::Excuse
                } else {
                    WarReason::Hatred
                };
                self.begin_ai_war_plan(owner, target, 4 + map_size, reason);
            }

            // Unlike a shared enemy, an enemy of an ally supplies a specific
            // obligation. Freeciv gives this a shorter preparation countdown.
            for &target in &contacts {
                if self.at_war(owner, target)
                    || self.allied(owner, target)
                    || self.ai_war_countdown(owner, target) != -1
                {
                    continue;
                }
                let intel = self.diplomatic_intel(owner, target);
                let defended_ally = intel.at_war_with_ally;
                let shared_ally = intel.allied_with_ally.is_some();
                if defended_ally.is_some()
                    && !shared_ally
                    && (self.pact(owner, target) != Pact::Ceasefire || self.rand_below(5) == 0)
                {
                    self.begin_ai_war_plan(owner, target, 2 + map_size, WarReason::Alliance);
                }
            }
        }

        let plans: Vec<_> = self
            .ai_war_plans
            .iter()
            .map(|(&pair, &plan)| (pair, plan))
            .collect();
        for ((owner, target), plan) in plans {
            if !self.player_alive(owner) || !self.player_alive(target) || !self.players[owner].ai {
                self.ai_war_plans.remove(&(owner, target));
            } else if plan.countdown > 0 {
                self.ai_war_plans
                    .get_mut(&(owner, target))
                    .unwrap()
                    .countdown -= 1;
            } else if plan.countdown < -1 {
                self.ai_war_plans
                    .get_mut(&(owner, target))
                    .unwrap()
                    .countdown += 1;
            } else if plan.countdown == 0 && !self.at_war(owner, target) {
                self.declare_prepared_ai_war(owner, target, plan.reason);
            }
        }
    }

    fn declare_prepared_ai_war(&mut self, owner: usize, target: usize, reason: WarReason) {
        if matches!(reason, WarReason::Alliance)
            && self
                .diplomatic_intel(owner, target)
                .at_war_with_ally
                .is_none()
        {
            self.ai_war_plans.remove(&(owner, target));
            return;
        }
        let stubbornness = match reason {
            WarReason::Spoils | WarReason::Space => -10,
            WarReason::Hatred | WarReason::Excuse | WarReason::Behavior => -20,
            WarReason::Alliance => -3,
        };
        // dai_go_to_war commits the diplomatic cooldown before consulting
        // the Senate, and revokes only this AI's outgoing shared vision.
        self.ai_war_plans.insert(
            (owner, target),
            WarPlan {
                countdown: stubbornness,
                reason,
            },
        );
        if let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(owner, target)) {
            relation.vision[usize::from(owner > target)] = false;
        }
        self.refresh_visibility();
        if !self.at_war(owner, target) && self.senate_blocks_cancellation(owner, target) {
            let government = self.players[owner].government;
            self.players[owner].government = Government::Anarchy;
            let revolution_helps = !self.senate_blocks_cancellation(owner, target);
            self.players[owner].government = government;
            if revolution_helps && self.begin_ai_war_revolution(owner) {
                // revolution_start runs inside the request above, before
                // native dai_go_to_war assigns its transient war_target.
                self.ai_pending_war_targets.insert(owner, target);
            } else {
                self.ai_war_plans.remove(&(owner, target));
            }
            return;
        }
        self.declare_ai_war_now(owner, target);
    }

    pub(super) fn resume_ai_war_after_revolution(&mut self, owner: usize) {
        if let Some(target) = self.ai_pending_war_targets.remove(&owner) {
            if self.player_alive(target) {
                if let Some(relation) = self.diplomacy.get_mut(&Self::relation_key(owner, target)) {
                    relation.vision[usize::from(owner > target)] = false;
                }
                self.declare_ai_war_now(owner, target);
            }
        }
    }

    fn declare_ai_war_now(&mut self, owner: usize, target: usize) {
        while !self.at_war(owner, target) {
            if !self.cancel_pact(owner, target) {
                self.ai_war_plans.remove(&(owner, target));
                return;
            }
        }
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(owner, target))
            .unwrap();
        let trust = &mut relation.trust[usize::from(owner > target)];
        let love = *trust / 10;
        *trust = (if love > 0 { -1 } else { love } - 125) * 10;
        relation.offer = None;
        if target == HUMAN {
            self.add_event(format!("{} declared war on you.", self.players[owner].name));
        }
        self.refresh_visibility();
    }

    fn pact_refusal(
        &self,
        receiver: usize,
        proposer: usize,
        proposed: Pact,
    ) -> Option<&'static str> {
        if self.players[receiver].ai
            && proposed != Pact::Ceasefire
            && self.ai_war_countdown(receiver, proposer) != -1
        {
            return Some("We are preparing for war or remain committed to the current war.");
        }
        if self.next_pact(receiver, proposer) != Some(proposed)
            && !(proposed == Pact::Alliance && self.pact(receiver, proposer) != Pact::Alliance)
        {
            return Some("We must honor the current treaty before negotiating the next one.");
        }
        if proposed != Pact::Ceasefire {
            let live_enemy = (0..self.players.len()).any(|third| {
                third != receiver
                    && third != proposer
                    && third != BARBARIANS
                    && self.player_alive(third)
                    && self.allied(proposer, third)
                    && self.diplomatic_at_war(receiver, third)
            });
            if live_enemy {
                return Some("Your ties to an enemy make this treaty impossible.");
            }
            if self
                .diplomatic_intel(receiver, proposer)
                .allied_with_enemy
                .is_some()
            {
                return Some("Keep your distance from our enemy for a while first.");
            }
            if proposed == Pact::Alliance
                && (0..self.players.len()).any(|third| {
                    third != receiver
                        && third != proposer
                        && third != BARBARIANS
                        && self.player_alive(third)
                        && self.allied(receiver, third)
                        && self.diplomatic_at_war(proposer, third)
                })
            {
                return Some("You are at war with our ally.");
            }
        }
        let empty = Relation::default();
        let relation = self
            .diplomacy
            .get(&Self::relation_key(receiver, proposer))
            .unwrap_or(&empty);
        if relation.first_contact.is_none() && !self.diplomatic_meeting_possible(receiver, proposer)
        {
            return Some("We have not made contact.");
        }
        if proposed == Pact::Alliance && relation.trust[usize::from(receiver > proposer)] < 1000 {
            return Some("We simply do not trust you with an alliance yet.");
        }
        None
    }

    fn pact_clause_value(&self, owner: usize, other: usize, proposed: Pact) -> i64 {
        let empty = Relation::default();
        let relation = self
            .diplomacy
            .get(&Self::relation_key(owner, other))
            .unwrap_or(&empty);
        let love = relation.trust[usize::from(owner > other)] / 10;
        let missing = match proposed {
            Pact::Peace => love - 125,
            Pact::Alliance => love - 250,
            Pact::Ceasefire if self.turn - relation.first_contact.unwrap_or(0) < 15 => {
                return 0;
            }
            Pact::Ceasefire => love,
            _ => return 0,
        };
        // daidiplomacy.c:greed converts missing love into negative gold value.
        let deficit = i64::from(missing.min(0));
        let mut value = -(deficit * deficit / 10);
        if proposed == Pact::Alliance
            && !matches!(
                self.pact(owner, other),
                Pact::Peace | Pact::Armistice | Pact::Alliance
            )
        {
            let peace_deficit = i64::from((love - 125).min(0));
            value -= peace_deficit * peace_deficit / 10;
        }
        let shared_ally = self
            .diplomatic_intel(owner, other)
            .allied_with_ally
            .is_some();
        if shared_ally {
            value /= 2;
        }
        value
    }

    pub(super) fn player_alive(&self, owner: usize) -> bool {
        self.cities.iter().any(|city| city.owner == owner)
            || self.units.iter().any(|unit| unit.owner == owner)
    }

    pub(super) fn negotiate(&mut self, other: usize, proposed: Pact) -> bool {
        if !self.can_negotiate(other) {
            return false;
        }
        if let Some(reason) = self.pact_refusal(other, HUMAN, proposed) {
            self.add_event(format!("{} declined. {reason}", self.players[other].name));
            return false;
        }
        let offer = TreatyOffer {
            pact: Some(proposed),
            ..TreatyOffer::default()
        };
        if !self.execute_offer(HUMAN, other, offer) {
            self.add_event(format!(
                "{} requires compensation for that treaty.",
                self.players[other].name
            ));
            return false;
        }
        true
    }

    pub(super) fn sign_pact(&mut self, a: usize, b: usize, proposed: Pact) {
        let relation = self.diplomacy.entry(Self::relation_key(a, b)).or_default();
        relation.pact = if proposed == Pact::Peace {
            Pact::Armistice
        } else {
            proposed
        };
        relation.until = if matches!(proposed, Pact::Peace | Pact::Ceasefire) {
            self.turn + 16
        } else {
            0
        };
        relation.offer = None;
        // Native diplomacy has a permanent state independently of physical
        // contact; a treaty through an embassy can change NO_CONTACT without
        // calling first_contact. Its untouched first_contact_turn is zero.
        relation.first_contact.get_or_insert(0);
        for key in [(a, b), (b, a)] {
            if key.0 == BARBARIANS || key.1 == BARBARIANS {
                continue;
            }
            let memory = self
                .ai_diplomatic_memory
                .entry(key)
                .or_insert_with(|| DiplomaticMemory::for_pair(key.0, key.1));
            if Self::pact_rank(proposed) > Self::pact_rank(memory.strongest_pact) {
                memory.strongest_pact = proposed;
            }
        }
        self.ai_war_plans.remove(&(a, b));
        self.ai_war_plans.remove(&(b, a));
        if a == HUMAN || b == HUMAN {
            let other = if a == HUMAN { b } else { a };
            let detail = if proposed == Pact::Peace {
                " Withdraw from their territory during the 16-turn armistice."
            } else {
                ""
            };
            self.add_event(format!(
                "{} agreed to {}.{detail}",
                self.players[other].name,
                proposed.name().to_lowercase()
            ));
        }
    }

    pub(super) fn cancel_pact(&mut self, owner: usize, other: usize) -> bool {
        let pact = self.pact(owner, other);
        if pact == Pact::War || !self.player_alive(other) {
            return false;
        }
        if self.senate_blocks_cancellation(owner, other) {
            if owner == HUMAN {
                self.add_event(format!(
                    "The Senate refuses to break the {} with {}.",
                    pact.name().to_lowercase(),
                    self.players[other].name
                ));
            }
            return false;
        }
        if pact != Pact::Alliance {
            self.record_war_incident(owner, other, pact);
        }
        let relation = self
            .diplomacy
            .get_mut(&Self::relation_key(owner, other))
            .unwrap();
        relation.pact = if pact == Pact::Alliance {
            Pact::Armistice
        } else {
            Pact::War
        };
        relation.until = if pact == Pact::Alliance {
            self.turn + 16
        } else {
            0
        };
        relation.offer = None;
        relation.cause_until[usize::from(owner > other)] = 0;
        if pact == Pact::Alliance {
            self.resolve_split_stacks(owner, other);
            self.refresh_visibility();
        } else {
            self.record_provocation(other, owner);
            let mutual_allies: Vec<_> = (0..self.players.len())
                .filter(|&ally| {
                    ally != owner
                        && ally != other
                        && self.player_alive(ally)
                        && self.allied(ally, owner)
                        && self.allied(ally, other)
                })
                .collect();
            for ally in mutual_allies {
                self.record_provocation(ally, owner);
                self.cancel_pact(ally, owner);
            }
        }
        self.prune_trade_routes();
        self.add_event(format!(
            "{} ended the {} with {}.",
            self.players[owner].name,
            pact.name().to_lowercase(),
            self.players[other].name
        ));
        true
    }

    pub(super) fn record_diplomatic_incident(
        &mut self,
        violator: usize,
        victim: Option<usize>,
        incident: DiplomaticIncident,
        international: bool,
    ) {
        if violator >= self.players.len() {
            return;
        }
        let badness = match incident {
            DiplomaticIncident::Snooping | DiplomaticIncident::ExtraLoss => 2,
            DiplomaticIncident::Theft
            | DiplomaticIncident::UnitLoss
            | DiplomaticIncident::BuildingLoss
            | DiplomaticIncident::PopulationLoss => 5,
            DiplomaticIncident::UnitWeakening => 3,
            DiplomaticIncident::CityLoss => 10,
            DiplomaticIncident::TerrainLoss => 4,
            DiplomaticIncident::Misc => 1,
            DiplomaticIncident::Nuclear => 0,
        };
        for receiver in 0..self.players.len() {
            if receiver == violator
                || receiver == BARBARIANS
                || !self.players[receiver].ai
                || (!international && victim != Some(receiver))
            {
                continue;
            }
            let loss = if matches!(incident, DiplomaticIncident::Nuclear) {
                if victim == Some(receiver) {
                    300
                } else if victim == Some(violator) {
                    50
                } else {
                    100
                }
            } else if victim == Some(receiver) {
                badness * 1000 * if international { 2 } else { 1 } / 35
            } else if victim == Some(violator) {
                badness
            } else {
                badness * 2
            };
            let relation = self
                .diplomacy
                .entry(Self::relation_key(receiver, violator))
                .or_default();
            let love = &mut relation.trust[usize::from(receiver > violator)];
            *love = (love.saturating_div(10).saturating_sub(loss)).saturating_mul(10);
        }
    }

    fn pact_rank(pact: Pact) -> i32 {
        match pact {
            Pact::War => 0,
            Pact::Ceasefire => 1,
            Pact::Armistice | Pact::Peace => 2,
            Pact::Alliance => 3,
        }
    }

    fn record_war_incident(&mut self, violator: usize, victim: usize, prior: Pact) {
        // call_incident(VICTIM_ONLY) dispatches through the victim's callback;
        // dai_incident returns immediately when that victim is human.
        if !self.players[victim].ai {
            return;
        }
        let strongest = [(violator, victim), (victim, violator)]
            .iter()
            .filter_map(|key| {
                self.ai_diplomatic_memory
                    .get(key)
                    .map(|memory| memory.strongest_pact)
            })
            .chain([prior])
            .max_by_key(|&pact| Self::pact_rank(pact))
            .unwrap_or(Pact::War);
        for receiver in 0..self.players.len() {
            if receiver == violator || receiver == BARBARIANS || !self.players[receiver].ai {
                continue;
            }
            let loss = 33
                + match strongest {
                    Pact::Peace | Pact::Armistice => 33,
                    Pact::Alliance => 100,
                    _ => 0,
                };
            let relation = self
                .diplomacy
                .entry(Self::relation_key(receiver, violator))
                .or_default();
            let trust = &mut relation.trust[usize::from(receiver > violator)];
            let mut love = trust.saturating_div(10).saturating_sub(loss);
            if receiver == victim {
                love = love.saturating_sub(333).min(-1);
            }
            *trust = love.saturating_mul(10);
        }
    }

    pub(super) fn diplomatic_love(&self, owner: usize, other: usize) -> i32 {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .map_or(0, |relation| {
                relation.trust[usize::from(owner > other)] / 10
            })
    }

    pub(crate) fn contacted_civilizations(&self, owner: usize) -> Vec<usize> {
        let mut contacts: Vec<_> = self
            .diplomacy
            .iter()
            .filter_map(|(&(a, b), relation)| {
                let other = if a == owner {
                    b
                } else if b == owner {
                    a
                } else {
                    return None;
                };
                (other != owner
                    && other != BARBARIANS
                    && self.player_alive(other)
                    && relation.first_contact.is_some())
                .then_some((relation.first_contact.unwrap_or_default(), other))
            })
            .collect();
        contacts.sort_unstable();
        contacts.into_iter().map(|(_, other)| other).collect()
    }

    pub(crate) fn diplomatic_attitude(&self, owner: usize, other: usize) -> &'static str {
        // UI bands summarize the reference AI's [-1000, 1000] love scale.
        // They are display labels; the underlying diplomacy score is unchanged.
        match self.diplomatic_love(owner, other) {
            love if love <= -600 => "Furious",
            love if love <= -200 => "Annoyed",
            love if love < 200 => "Cautious",
            love if love < 600 => "Polite",
            _ => "Gracious",
        }
    }

    pub(super) fn diplomatic_meeting_possible(&self, owner: usize, other: usize) -> bool {
        owner != other
            && owner < self.players.len()
            && other < self.players.len()
            && owner != BARBARIANS
            && other != BARBARIANS
            && self.player_alive(owner)
            && self.player_alive(other)
            && (self.has_embassy(owner, other)
                || self.has_embassy(other, owner)
                || self
                    .diplomatic_contact_until
                    .get(&Self::relation_key(owner, other))
                    .copied()
                    .is_some_and(|until| self.turn < until))
    }

    pub(super) fn seed_legacy_diplomatic_contacts(&mut self) {
        // V1–V8 recorded first contact but no last-contact counter. Give known
        // living factions one bounded migration window without replaying
        // physical contact, first-contact callbacks, or random draws.
        let until = self.turn.saturating_add(20);
        for (&(a, b), relation) in &self.diplomacy {
            if a != BARBARIANS
                && b != BARBARIANS
                && relation.first_contact.is_some()
                && self.player_alive(a)
                && self.player_alive(b)
            {
                self.diplomatic_contact_until.entry((a, b)).or_insert(until);
            }
        }
    }

    pub(super) fn refresh_diplomatic_contact_effects(&mut self) {
        // srv_main.c:do_have_contacts_effect runs after city refresh at phase
        // end, before the turn advances. Civ2Civ3 grants this via Marco Polo.
        for owner in 0..self.players.len() {
            if owner == BARBARIANS
                || !self.player_alive(owner)
                || !self.has_active_wonder(owner, Building::MARCO_POLOS_EMBASSY)
            {
                continue;
            }
            for other in 0..self.players.len() {
                if other != owner && other != BARBARIANS && self.player_alive(other) {
                    self.make_diplomatic_contact(owner, other);
                }
            }
        }
    }

    pub(crate) fn refresh_diplomatic_contact_at(&mut self, owner: usize, x: i32, y: i32) {
        if owner == BARBARIANS || owner >= self.players.len() || !self.player_alive(owner) {
            return;
        }
        // plrhand.c:maybe_make_contact examines adjacent units/cities, not
        // every visible tile or territorial ownership. Current unit roster
        // has no Flagless type (native skips those units).
        let contacts: std::collections::BTreeSet<_> = self
            .units
            .iter()
            .map(|unit| (unit.owner, unit.x, unit.y))
            .chain(self.cities.iter().map(|city| (city.owner, city.x, city.y)))
            .filter_map(|(other, tx, ty)| {
                (other != owner && other != BARBARIANS && self.map_distance(x, y, tx, ty) <= 1)
                    .then_some(other)
            })
            .collect();
        for other in contacts {
            self.make_diplomatic_contact(owner, other);
        }
    }

    pub(super) fn make_diplomatic_contact(&mut self, a: usize, b: usize) {
        if a == b
            || a == BARBARIANS
            || b == BARBARIANS
            || !self.player_alive(a)
            || !self.player_alive(b)
        {
            return;
        }
        let first_contact = self
            .diplomacy
            .get(&Self::relation_key(a, b))
            .is_none_or(|relation| relation.first_contact.is_none());
        self.initialize_diplomatic_contact(a, b);
        if first_contact && (a == HUMAN || b == HUMAN) {
            let other = if a == HUMAN { b } else { a };
            self.add_event(format!(
                "You have made contact with the {}, ruled by {}.",
                self.players[other].name,
                self.players[other].leader_name()
            ));
        }
        // Native decrements before advancing the turn and exempts the first
        // contact turn. Later physical contacts refresh the ordinary20 turns.
        let first_turn = self.diplomacy[&Self::relation_key(a, b)].first_contact == Some(self.turn);
        self.diplomatic_contact_until.insert(
            Self::relation_key(a, b),
            self.turn.saturating_add(20 + i32::from(first_turn)),
        );
    }

    fn initialize_diplomatic_contact(&mut self, a: usize, b: usize) {
        let key = Self::relation_key(a, b);
        if self
            .diplomacy
            .get(&key)
            .is_some_and(|relation| relation.first_contact.is_some())
        {
            return;
        }
        let common_ally = (0..self.players.len()).any(|third| {
            third != a
                && third != b
                && third != BARBARIANS
                && self.player_alive(third)
                && self.allied(a, third)
                && self.allied(b, third)
        });
        let relation = self.diplomacy.entry(key).or_default();
        relation.first_contact = Some(self.turn);
        if common_ally && relation.pact == Pact::War {
            // plrhand.c:get_default_diplstate sets immediate Peace, without
            // an armistice, when both newly met players share an ally.
            relation.pact = Pact::Peace;
            relation.until = 0;
            for pair in [(a, b), (b, a)] {
                let memory = self
                    .ai_diplomatic_memory
                    .entry(pair)
                    .or_insert_with(|| DiplomaticMemory::for_pair(pair.0, pair.1));
                if Self::pact_rank(memory.strongest_pact) < Self::pact_rank(Pact::Peace) {
                    memory.strongest_pact = Pact::Peace;
                }
            }
        }
        // dai_diplomacy_first_contact initializes each AI's own attitude once.
        for owner in [a, b] {
            if owner != BARBARIANS && self.players[owner].ai {
                let adjustment = (2 - self.rand_below(5)) * 10;
                self.diplomacy.get_mut(&key).unwrap().trust[usize::from(owner == key.1)] +=
                    adjustment;
            }
        }
    }

    pub(super) fn record_provocation(&mut self, victim: usize, offender: usize) {
        if victim != offender && victim != BARBARIANS && offender != BARBARIANS {
            self.initialize_diplomatic_contact(victim, offender);
            let relation = self
                .diplomacy
                .entry(Self::relation_key(victim, offender))
                .or_default();
            relation.first_contact.get_or_insert(self.turn);
            relation.cause_until[usize::from(victim > offender)] = self.turn + 2;
        }
    }

    pub(super) fn refresh_diplomatic_intelligence(&mut self) {
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

    fn diplomatic_intel(&self, owner: usize, other: usize) -> DiplomaticIntel {
        self.ai_diplomatic_intel
            .get(&(owner, other))
            .copied()
            .unwrap_or_default()
    }

    fn diplomatic_at_war(&self, owner: usize, other: usize) -> bool {
        self.diplomacy
            .get(&Self::relation_key(owner, other))
            .is_some_and(|relation| relation.first_contact.is_some() && relation.pact == Pact::War)
    }

    fn ai_war_desire(&self, owner: usize, target: usize) -> i32 {
        // daidiplomacy.c:dai_war_desire. This intentionally uses type power,
        // not current HP, veteran level or battle odds.
        let mut want = 0i32;
        let mut fear = 0i32;
        for city in self.cities.iter().filter(|city| city.owner == target) {
            let output = self.city_forecast(city);
            want += 100 + city.size * 20 + output.shield_surplus() * 8 + output.trade * 6;
            // get_city_bonus supplies no unit context, so UnitClass/Flag
            // defense requirements fail (including walls and city bonuses).
            fear += match self.tile(city.x, city.y).base {
                Some(Base::Fortress | Base::Airbase) => 50,
                _ => 0,
            };
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
        for unit in self.units.iter().filter(|unit| unit.owner == target) {
            let stats = unit.kind.stats();
            fear += stats.attack * stats.hp * stats.firepower;
            if stats.can_found {
                want += 100;
            }
        }
        let mut settlers = 0;
        for unit in self.units.iter().filter(|unit| unit.owner == owner) {
            let stats = unit.kind.stats();
            fear -= stats.attack * stats.hp * stats.firepower / 2;
            if stats.can_found {
                want -= 200;
                settlers += 1;
            }
        }
        let mut cities = 0;
        for city in self.cities.iter().filter(|city| city.owner == owner) {
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
        for city in self.cities.iter().filter(|city| city.owner == owner) {
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

    fn advance_ai_love(&mut self) {
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
                if contacted && !self.completed_wonders.contains(&Building::APOLLO_PROGRAM) {
                    for wonder in [Building::EIFFEL_TOWER, Building::UNITED_NATIONS] {
                        if self.has_active_wonder(other, wonder) {
                            love += 10;
                        }
                    }
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

    pub(super) fn advance_diplomacy(&mut self) {
        let mut transitions = Vec::new();
        for (&(a, b), relation) in &mut self.diplomacy {
            if relation.until > 0 && relation.until <= self.turn {
                relation.pact = if relation.pact == Pact::Armistice {
                    Pact::Peace
                } else {
                    Pact::War
                };
                relation.until = 0;
                relation.offer = None;
                transitions.push((a, b, relation.pact));
            }
        }
        for (a, b, pact) in transitions {
            if pact == Pact::Peace {
                let expelled: Vec<_> = self
                    .units
                    .iter()
                    .filter(|unit| {
                        unit.owner == a && self.tile(unit.x, unit.y).owner == Some(b)
                            || unit.owner == b && self.tile(unit.x, unit.y).owner == Some(a)
                    })
                    .map(|unit| unit.id)
                    .collect();
                self.units.retain(|unit| {
                    !expelled.contains(&unit.id)
                        && unit.transport.is_none_or(|id| !expelled.contains(&id))
                });
                self.prune_unit_ai_state();
            }
            if a == HUMAN || b == HUMAN {
                let other = if a == HUMAN { b } else { a };
                self.add_event(format!(
                    "{} with {}.",
                    if pact == Pact::Peace {
                        "Peace entered into force"
                    } else {
                        "Ceasefire expired"
                    },
                    self.players[other].name
                ));
            }
        }

        // daidata.c applies new-phase love against the preceding intelligence,
        // then refreshes the relation pointers used by diplomatic actions.
        self.advance_ai_love();
        self.refresh_diplomatic_intelligence();
        self.advance_ai_war_plans();
        self.advance_diplomatic_offers();
    }

    fn advance_diplomatic_offers(&mut self) {
        let mut vision_changed = false;
        for owner in 0..self.players.len() {
            if owner == BARBARIANS || !self.players[owner].ai || !self.player_alive(owner) {
                continue;
            }
            // This precedes target/countdown/contact spam exclusions in the
            // source: an existing unsafe grant must still be withdrawn.
            let revoke: Vec<_> = self
                .diplomacy
                .iter()
                .filter_map(|(&(a, b), relation)| {
                    let other = if a == owner {
                        b
                    } else if b == owner {
                        a
                    } else {
                        return None;
                    };
                    (relation.vision[usize::from(owner > other)]
                        && (!self.allied(owner, other) || !self.vision_safe(owner, other)))
                    .then_some(other)
                })
                .collect();
            for other in revoke {
                self.diplomacy
                    .get_mut(&Self::relation_key(owner, other))
                    .unwrap()
                    .vision[usize::from(owner > other)] = false;
                vision_changed = true;
            }
            let contacts: Vec<_> = (0..self.players.len())
                .filter(|&other| {
                    other != owner
                        && other != BARBARIANS
                        && self.player_alive(other)
                        && (self
                            .diplomacy
                            .get(&Self::relation_key(owner, other))
                            .is_some_and(|relation| relation.first_contact.is_some())
                            || self.has_embassy(owner, other)
                            || self.has_embassy(other, owner))
                })
                .collect();
            // dai_diplomacy_actions retains this target even if its love was
            // above the war threshold. Its offer loop still skips that player.
            let mut target = contacts
                .iter()
                .copied()
                .filter(|&other| {
                    let Some(relation) = self.diplomacy.get(&Self::relation_key(owner, other))
                    else {
                        return false;
                    };
                    relation.first_contact.is_some()
                        && !self.diplomatic_at_war(owner, other)
                        && self.turn - relation.first_contact.unwrap() > 15
                        && relation.trust[usize::from(owner > other)] < 10_000
                })
                .min_by_key(|&other| {
                    self.diplomacy[&Self::relation_key(owner, other)].trust
                        [usize::from(owner > other)]
                });
            for other in contacts {
                if Some(other) == target
                    || self.ai_war_countdown(owner, other) >= 0
                    || !self.diplomatic_meeting_possible(owner, other)
                {
                    continue;
                }
                self.diplomacy
                    .entry(Self::relation_key(owner, other))
                    .or_default();
                let memory = self
                    .ai_diplomatic_memory
                    .entry((owner, other))
                    .or_insert_with(|| DiplomaticMemory::for_pair(owner, other));
                memory.asked_peace = memory.asked_peace.saturating_sub(1).max(0);
                memory.asked_alliance = memory.asked_alliance.saturating_sub(1).max(0);
                memory.asked_ceasefire = memory.asked_ceasefire.saturating_sub(1).max(0);
                memory.spam = memory.spam.saturating_sub(1).max(0);
                if let Some(warning) = self.ai_space_warnings.get_mut(&(owner, other)) {
                    *warning = warning.saturating_sub(1).max(0);
                }
                if memory.spam > 0 && self.players[other].ai {
                    continue;
                }
                if memory.spam == 0 {
                    let spam = if self.allied(owner, other) {
                        6 + self.rand_below(8)
                    } else {
                        3 + self.rand_below(4)
                    };
                    self.ai_diplomatic_memory
                        .get_mut(&(owner, other))
                        .unwrap()
                        .spam = spam;
                }
                let key = Self::relation_key(owner, other);
                let (proposed, offer) = if self.allied(owner, other) {
                    let at_war_with_ally = self
                        .diplomatic_intel(owner, other)
                        .at_war_with_ally
                        .is_some();
                    if at_war_with_ally {
                        continue;
                    }
                    // The original reuses `target` while checking an ally's
                    // failure to fight our enemies. Preserve that loop effect.
                    target = (0..self.players.len()).find(|&third| {
                        third != owner
                            && third != BARBARIANS
                            && self.player_alive(third)
                            && self.diplomatic_at_war(owner, third)
                            && !self.diplomatic_at_war(other, third)
                    });
                    if self.diplomacy[&key].trust[usize::from(owner > other)] <= 5000 {
                        continue;
                    }
                    (None, self.source_allied_offer(owner, other))
                } else {
                    if self.pact(owner, other) != Pact::War
                        && self
                            .diplomatic_intel(owner, other)
                            .at_war_with_ally
                            .is_some()
                    {
                        continue;
                    }
                    let Some(pact) = self.next_pact(owner, other).filter(|&pact| {
                        self.pact_refusal(owner, other, pact).is_none()
                            && self.pact_clause_value(owner, other, pact) >= 0
                    }) else {
                        continue;
                    };
                    let memory = &self.ai_diplomatic_memory[&(owner, other)];
                    let asked = match pact {
                        Pact::Peace => memory.asked_peace,
                        Pact::Alliance => memory.asked_alliance,
                        _ => memory.asked_ceasefire,
                    };
                    if other == HUMAN && asked > 0 {
                        continue;
                    }
                    (
                        Some(pact),
                        TreatyOffer {
                            pact: Some(pact),
                            ..TreatyOffer::default()
                        },
                    )
                };
                if offer.pact.is_none() && !offer.bundled() {
                    continue;
                }
                if let Some(pact) = proposed {
                    let memory = self.ai_diplomatic_memory.get_mut(&(owner, other)).unwrap();
                    if other == HUMAN {
                        match pact {
                            Pact::Peace => memory.asked_peace = 12,
                            Pact::Alliance => memory.asked_alliance = 13,
                            _ => memory.asked_ceasefire = 9,
                        }
                    }
                }
                if other == HUMAN {
                    self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
                    let what = if offer.bundled() {
                        "a treaty bundle".to_owned()
                    } else {
                        proposed.unwrap().name().to_lowercase()
                    };
                    self.add_event(format!("{} offers {what}.", self.players[owner].name));
                } else {
                    self.execute_offer(key.0, key.1, offer);
                }
            }
        }
        if vision_changed {
            self.refresh_visibility();
        }
    }

    pub(super) fn treaty_blocks_move(&self, unit: &Unit, x: i32, y: i32) -> bool {
        let blocked = |owner| !self.allied(unit.owner, owner) && !self.at_war(unit.owner, owner);
        let occupied = self
            .units
            .iter()
            .any(|other| (other.x, other.y) == (x, y) && blocked(other.owner))
            || self
                .cities
                .iter()
                .any(|city| (city.x, city.y) == (x, y) && blocked(city.owner));
        occupied
            || !unit.kind.is_civilian()
                && self
                    .tile(x, y)
                    .owner
                    .is_some_and(|owner| self.pact(unit.owner, owner) == Pact::Peace)
    }
}

impl Simulation {
    pub(crate) fn ai_war_planning_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(1);
        relation.trust[1] = -5000;
        self.advance_ai_war_plans();
        let preparing = self.ai_on_war_footing(1) && self.pact(HUMAN, 1) == Pact::Peace;
        for _ in 0..4 + (self.map_wrap.width() * self.map_wrap.height() / 1000).max(1) {
            self.advance_ai_war_plans();
        }
        preparing
            && self.at_war(HUMAN, 1)
            && !self.ai_on_war_footing(1)
            && self.ai_war_countdown(1, HUMAN) == -20
            && self.pact_clause_value(1, HUMAN, Pact::Ceasefire) < 0
    }

    pub(crate) fn diplomacy_playtest_friendship(&mut self) {
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.trust[1] = 2500;
        relation.cause_until[1] = 0;
        self.ai_war_plans.retain(|&(owner, _), _| owner != 1);
        self.units
            .retain(|unit| unit.owner != 1 || unit.kind.stats().attack == 0);
        for city in self.cities.iter_mut().filter(|city| city.owner == 1) {
            city.production = Production::Unit(UnitKind::Settlers);
        }
        self.make_diplomatic_contact(HUMAN, 1);
        self.diplomacy_playtest_alternate_contact();
    }

    fn diplomacy_playtest_alternate_contact(&mut self) {
        if !self.player_alive(2) {
            if let Some((x, y)) = (0..self.tiles.len())
                .map(|index| self.map_wrap.position(index))
                .find(|&(x, y)| {
                    self.cities
                        .iter()
                        .all(|city| self.map_distance(city.x, city.y, x, y) > 5)
                        && self.units.iter().all(|unit| (unit.x, unit.y) != (x, y))
                })
            {
                self.tile_mut(x, y).terrain = Terrain::Grassland;
                self.create_city(2, x, y);
            }
        }
        self.make_diplomatic_contact(1, 2);
        self.sign_pact(1, 2, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(1, 2)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(self.turn - 20);
        relation.trust = [-2000, 6500];
        relation.cause_until = [0; 2];
        self.ai_war_plans.remove(&(1, 2));
        self.ai_war_plans.remove(&(2, 1));
    }

    pub(crate) fn diplomacy_playtest_allied_trade_baseline(&self) -> Option<i32> {
        let currency = Tech::all().find(|tech| tech.name() == "Currency")?;
        Some(
            self.tech_sell_price_with_advisor(HUMAN, 1, currency, false)?
                - self.tech_sell_price_with_advisor(1, HUMAN, Tech::WARRIOR_CODE, false)?,
        )
    }

    pub(crate) fn diplomacy_playtest_allied_sharing(&mut self) {
        // Arrange a trusted established alliance, preserving the technologies
        // and treasuries whose actual exchange is exercised by the UI.
        if !self.allied(HUMAN, 1) {
            return;
        }
        self.make_diplomatic_contact(HUMAN, 1);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.trust[1] = 6500;
        relation.cause_until = [0; 2];
        self.ai_war_plans.remove(&(1, HUMAN));
        self.ai_war_plans.remove(&(HUMAN, 1));
        self.diplomacy_playtest_alternate_contact();
        self.diplomacy_playtest_tick(1);
    }

    pub(crate) fn ai_treaty_policy_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.ai_war_plans.clear();
        self.make_diplomatic_contact(HUMAN, 1);
        self.sign_pact(HUMAN, 1, Pact::Peace);
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        self.sign_pact(1, 2, Pact::Alliance);
        self.diplomacy.entry((HUMAN, 2)).or_default().pact = Pact::War;
        self.players[HUMAN].known = vec![Tech::POTTERY];
        self.players[1].known.clear();
        self.players[2].known.clear();
        let quote = self.tech_trade_quote(HUMAN, 1, Tech::POTTERY, true);
        let mut gift = TreatyOffer::default();
        gift.terms[0].technology = Some(Tech::POTTERY);
        let received = quote.is_some_and(|(_, danger)| danger)
            && self.execute_offer(HUMAN, 1, gift)
            && self.has_tech(1, Tech::POTTERY);

        // A human can knowingly give vision that leaks to a third party.
        self.diplomacy.get_mut(&(HUMAN, 2)).unwrap().pact = Pact::Peace;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.diplomacy.get_mut(&(1, 2)).unwrap().vision[0] = true;
        let mut vision = TreatyOffer::default();
        vision.terms[0].access = 1;
        let shared = !self.vision_safe(HUMAN, 1) && self.execute_offer(HUMAN, 1, vision);
        self.cancel_pact(HUMAN, 1);
        let preserved = self.diplomacy[&(HUMAN, 1)].vision[0];

        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Ceasefire;
        relation.until = self.turn + 16;
        relation.trust[1] = 1000;
        self.ai_diplomatic_intel.clear();
        self.players[HUMAN].gold = 2312;
        let mut alliance = TreatyOffer {
            pact: Some(Pact::Alliance),
            ..TreatyOffer::default()
        };
        alliance.terms[0].gold = 2311;
        let compensated = !self.execute_offer(HUMAN, 1, alliance) && {
            alliance.terms[0].gold = 2312;
            self.execute_offer(HUMAN, 1, alliance)
        };
        self.players[1].government = Government::Democracy;
        self.players[1].revolution_target = None;
        self.players[1].revolution_turn = Some(self.turn - 10);
        self.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
        let pending = !self.at_war(1, HUMAN)
            && !self.ai_on_war_footing(1)
            && self.ai_war_countdown(1, HUMAN) == -20
            && self.ai_pending_war_targets.get(&1) == Some(&HUMAN)
            && self.players[1]
                .revolution_turn
                .is_some_and(|finish| finish > self.turn);
        let resumed = self.change_government(1, Government::Despotism)
            && self.at_war(1, HUMAN)
            && !self.ai_pending_war_targets.contains_key(&1);
        received && shared && preserved && compensated && pending && resumed
    }

    pub(crate) fn ai_diplomatic_intelligence_playtest(&mut self) -> bool {
        self.units.clear();
        self.turn = 30;
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        self.make_diplomatic_contact(HUMAN, 1);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.until = self.turn + 4;
        relation.trust[1] = 4000;
        self.sign_pact(HUMAN, 2, Pact::Alliance);
        self.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
        self.refresh_diplomatic_intelligence();
        let remembered = self.diplomatic_intel(1, HUMAN).allied_with_enemy == Some(2);
        let broken = self.apply(Command::CancelPact(2));
        let held = !self.apply(Command::Negotiate(1, Pact::Peace))
            && self.pact(HUMAN, 1) == Pact::Ceasefire;
        self.refresh_diplomatic_intelligence();
        let agreed = self.apply(Command::Negotiate(1, Pact::Peace))
            && self.pact(HUMAN, 1) == Pact::Armistice;
        remembered && broken && held && agreed
    }

    pub(crate) fn embassy_contact_playtest(&mut self) -> bool {
        self.units.clear();
        self.turn = 30;
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (18, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.diplomacy.remove(&(HUMAN, 1));
        self.diplomatic_contact_until.remove(&(HUMAN, 1));
        let adjacent = offset_native_by_map(18, 24, 1, 0);
        self.tile_mut(adjacent.0, adjacent.1).terrain = Terrain::Grassland;
        let visitor = self.spawn_unit(HUMAN, UnitKind::Diplomat, adjacent.0, adjacent.1);
        let met =
            self.can_negotiate(1) && self.diplomatic_contact_until[&(HUMAN, 1)] == self.turn + 21;
        self.units.retain(|unit| unit.id != visitor);
        self.diplomacy_playtest_tick(21);
        self.decline_offer(1);
        let expired = !self.can_negotiate(1) && !self.negotiate(1, Pact::Ceasefire);
        self.spawn_unit(HUMAN, UnitKind::Diplomat, adjacent.0, adjacent.1);
        let renewed = self.can_negotiate(1);
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        self.players[HUMAN].known = vec![Tech::ALPHABET];
        self.players[1].known = vec![Tech::ALPHABET];
        let offer = self.source_allied_offer(1, HUMAN);
        let clauses = offer.terms.iter().all(|terms| terms.embassy());
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
        let accepted = self.apply(Command::AcceptOffer(1, offer));
        self.diplomatic_contact_until.clear();
        met && expired
            && renewed
            && clauses
            && accepted
            && self.has_real_embassy(HUMAN, 1)
            && self.has_real_embassy(1, HUMAN)
            && self.can_negotiate(1)
    }

    pub(crate) fn ai_incident_action_playtest(&mut self) -> bool {
        self.units.clear();
        let (actor, target) = self.bribe_playtest_setup();
        self.make_diplomatic_contact(HUMAN, 1);
        self.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.trust[1] = 0;
        let actor_unit = self.units.iter().find(|unit| unit.id == actor).unwrap();
        let target_unit = self.units.iter().find(|unit| unit.id == target).unwrap();
        let Some(price) = self.bribe_price(actor_unit, target_unit) else {
            return false;
        };
        self.players[HUMAN].gold = price;
        let stale = !self.apply(Command::BribeUnit(actor, target, price + 1))
            && self.diplomacy[&(HUMAN, 1)].trust[1] == 0;
        let bribed = self.apply(Command::BribeUnit(actor, target, price));
        stale
            && bribed
            && self
                .units
                .iter()
                .any(|unit| unit.kind == UnitKind::Warrior && unit.owner == HUMAN)
            && self.diplomacy[&(HUMAN, 1)].trust[1] == -1420
            && self.diplomacy[&(HUMAN, 1)].cause_until[1] == self.turn + 2
    }

    pub(crate) fn ai_observer_incident_cadence_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (18, 24)), (2, (28, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        let distant = self.spawn_unit(HUMAN, UnitKind::Warrior, 26, 24);
        self.tile_mut(26, 24).known_by[1] = true;
        self.tile_mut(26, 24).owner = Some(1);
        let fogged = !self.unit_visible_to(1, self.units.iter().find(|u| u.id == distant).unwrap());
        let hidden = self.spawn_unit(HUMAN, UnitKind::Submarine, 20, 24);
        let stealth = !self.unit_visible_to(1, self.units.iter().find(|u| u.id == hidden).unwrap());
        self.spawn_unit(1, UnitKind::Destroyer, 19, 23);
        let detected = self.unit_visible_to(1, self.units.iter().find(|u| u.id == hidden).unwrap());
        self.make_diplomatic_contact(HUMAN, 1);
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        self.record_diplomatic_incident(HUMAN, Some(1), DiplomaticIncident::Theft, false);
        let offended = self.diplomacy[&(HUMAN, 1)].trust[1] == -1420;
        self.sign_pact(HUMAN, 1, Pact::Ceasefire);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.until = self.turn + 4;
        relation.trust[1] = 3000;
        self.advance_diplomatic_offers();
        let offered = self.pact_offer(1) == Some(Pact::Peace);
        self.decline_offer(1);
        self.advance_diplomatic_offers();
        let paced = self.pact_offer(1).is_none()
            && self.ai_diplomatic_memory[&(1, HUMAN)].asked_peace == 11;
        fogged && stealth && detected && offended && offered && paced
    }

    pub(crate) fn ai_diplomatic_love_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(1);
        relation.trust[1] = 1000;
        self.advance_ai_love();
        let growth = self.diplomacy[&(HUMAN, 1)].trust[1] == 1100;
        self.record_provocation(1, HUMAN);
        self.advance_ai_love();
        let resentment = self.diplomacy[&(HUMAN, 1)].trust[1] == -1200;
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().cause_until[1] = 0;
        self.players[HUMAN].gold = 1000;
        let mut gift = TreatyOffer::default();
        gift.terms[0].gold = 110;
        let accepted = self.execute_offer(HUMAN, 1, gift);
        growth && resentment && accepted && self.diplomacy[&(HUMAN, 1)].trust[1] == -200
    }

    pub(crate) fn ai_space_strategy_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.turn = 30;
        self.sign_pact(HUMAN, 1, Pact::Alliance);
        let relation = self.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.first_contact = Some(self.turn);
        relation.trust[1] = 5000;
        self.spaceship_playtest_setup();
        self.advance_ai_war_plans();
        let warned = self.diplomacy[&(HUMAN, 1)].trust[1] == 4000
            && self.ai_space_warnings[&(1, HUMAN)] >= 9
            && !self.ai_on_war_footing(1)
            && self.allied(HUMAN, 1);
        self.advance_ai_war_plans();
        let quiet = self.diplomacy[&(HUMAN, 1)].trust[1] == 4000;
        if !self.launch_spaceship(HUMAN) {
            return false;
        }
        self.advance_ai_war_plans();
        warned
            && quiet
            && self.ai_war_countdown(1, HUMAN) == 1
            && self.pact(HUMAN, 1) == Pact::Armistice
    }

    pub(crate) fn city_bundle_playtest(&mut self) -> bool {
        self.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 24)), (1, (14, 24)), (HUMAN, (10, 24))] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(owner, x, y);
        }
        self.transfer_traded_city(2, 1);
        for (x, y) in [(16, 24), (20, 24)] {
            self.tile_mut(x, y).terrain = Terrain::Grassland;
            self.create_city(1, x, y);
        }

        let point = (23, 36);
        let tile = self.tile_mut(point.0, point.1);
        tile.known_by[1] = true;
        tile.known_by[HUMAN] = false;
        tile.seen = false;
        let key = (HUMAN, 1);
        self.diplomacy.entry(key).or_default().pact = Pact::Peace;
        self.players[HUMAN].known.clear();
        self.players[1].known.clear();
        self.players[HUMAN].gold = 20_000;
        self.make_diplomatic_contact(HUMAN, 1);
        let offer = self.build_offer(HUMAN, 1, None);
        if offer.terms[1].city() != Some(self.cities[2].id)
            || offer.terms[1].map() != Some(TreatyMap::World)
        {
            return false;
        }

        self.cities[2].buildings.push(Building::PALACE);
        self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        let rejected = !self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 20_000
            && self.cities[2].owner == 1
            && !self.tile(point.0, point.1).seen;
        self.cities[2]
            .buildings
            .retain(|building| *building != Building::PALACE);
        self.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        rejected
            && self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 20_000 - offer.terms[0].gold
            && self.cities[2].owner == HUMAN
            && self.tile(point.0, point.1).seen
    }

    pub(crate) fn ai_map_offer_playtest(&mut self) -> bool {
        self.create_city(HUMAN, 3, 3);
        self.create_city(1, 18, 32);
        let point = (11, 24);
        let tile = self.tile_mut(point.0, point.1);
        tile.terrain = Terrain::Ocean;
        tile.known_by[1] = true;
        tile.known_by[HUMAN] = false;
        tile.visible = false;
        tile.seen = false;
        let relation = self.diplomacy.entry((HUMAN, 1)).or_default();
        relation.pact = Pact::Peace;
        relation.trust = [0; 2];
        self.players[HUMAN].gold = 15;
        let offer = self.build_offer(HUMAN, 1, None);
        if offer.terms[1].map() != Some(TreatyMap::Sea) || offer.terms[0].gold != 15 {
            return false;
        }
        self.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
        self.apply(Command::AcceptOffer(1, offer))
            && self.players[HUMAN].gold == 0
            && self.treaty_offer(1).is_none()
            && self.tile(point.0, point.1).seen
            && !self.tile(point.0, point.1).visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisor_danger_distinguishes_love_pacts_excuses_and_allocated_enemy_allies() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.river = false;
            tile.road = false;
            tile.railroad = false;
        }
        game.create_city(1, 10, 24);
        let nearby = offset_native_by_map(10, 24, 1, 0);
        let threat = game.spawn_unit(HUMAN, UnitKind::Warrior, nearby.0, nearby.1);
        let assessed = |sim: &Simulation| {
            sim.city_threat_arrivals(&sim.cities[0])
                .any(|(unit, _, _)| unit.id == threat)
        };
        game.sign_pact(1, HUMAN, Pact::Peace);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 999;
        assert!(game.advisor_player_dangerous(1, HUMAN));
        assert!(assessed(&game));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
        assert!(!game.advisor_player_dangerous(1, HUMAN));
        assert!(!assessed(&game));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Ceasefire;
        assert!(game.advisor_player_dangerous(1, HUMAN));
        assert!(assessed(&game));
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.cause_until[1] = game.turn + 1;
        assert!(game.advisor_player_dangerous(1, HUMAN));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().cause_until[1] = game.turn;
        assert!(!game.advisor_player_dangerous(1, HUMAN));
        game.ai_war_plans.insert(
            (1, HUMAN),
            WarPlan {
                countdown: 0,
                reason: WarReason::Spoils,
            },
        );
        assert!(game.advisor_player_dangerous(1, HUMAN));
        assert!(assessed(&game));
        game.ai_war_plans.clear();
        game.spawn_unit(2, UnitKind::Warrior, 24, 40);
        game.make_diplomatic_contact(1, 2);
        game.diplomacy.get_mut(&(1, 2)).unwrap().pact = Pact::War;
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.units.retain(|unit| unit.owner != 2);
        game.cities.retain(|city| city.owner != 2);
        assert!(!game.player_alive(2));
        assert!(game.advisor_player_dangerous(1, HUMAN));
        assert!(!game.advisor_player_dangerous(1, 1));
    }

    #[test]
    fn defense_building_baseline_respects_native_peace_love_and_ceasefire_danger() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.cities.clear();
        for tile in &mut game.tiles {
            tile.terrain = Terrain::Grassland;
            tile.owner = None;
            tile.river = false;
            tile.base = None;
        }
        game.create_city(1, 10, 24);
        game.cities[0].buildings.clear();
        game.create_city(HUMAN, 22, 40);
        game.sign_pact(1, HUMAN, Pact::Peace);
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.trust[1] = 999;
        assert_eq!(
            game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
            83.0
        );
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
        assert_eq!(
            game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
            3.0
        );
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Ceasefire;
        assert_eq!(
            game.defensive_building_effect_want(&game.cities[0], Building::CITY_WALLS),
            83.0
        );
    }

    fn war_planning_game() -> Simulation {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        for (owner, (x, y)) in [(HUMAN, (8, 20)), (1, (12, 24)), (2, (18, 30))] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
            game.create_city(owner, x, y);
        }
        game.turn = 30;
        game.sign_pact(HUMAN, 1, Pact::Peace);
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Peace;
        relation.until = 0;
        relation.first_contact = Some(1);
        relation.trust[1] = -5000;
        game.make_diplomatic_contact(HUMAN, 1);
        game
    }

    #[test]
    fn physical_contact_and_treaty_embassy_work_through_commands() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.embassy_contact_playtest());
    }

    #[test]
    fn embassy_access_extends_the_legacy_wire_without_aliasing_city_ids() {
        let legacy_access = (u64::from(u32::MAX) << 3) | 3;
        let legacy = Concessions {
            gold: 77,
            technology: Some(Tech::POTTERY),
            access: legacy_access,
        };
        let bytes = bincode::serialize(&(77i32, Some(Tech::POTTERY), legacy_access)).unwrap();
        assert_eq!(bincode::serialize(&legacy).unwrap(), bytes);
        let decoded: Concessions = bincode::deserialize(&bytes).unwrap();
        assert_eq!(decoded.city(), Some(u32::MAX));
        assert!(!decoded.embassy());
        let extended = decoded.with_embassy();
        assert!(extended.valid_access());
        assert_eq!(extended.city(), Some(u32::MAX));
        assert!(extended.shared_vision() && extended.embassy());
        assert_eq!(extended.map(), Some(TreatyMap::World));
        let cleared = extended.with_city(0);
        assert_eq!(cleared.city(), None);
        assert!(cleared.embassy() && cleared.shared_vision());
        for access in [6, 1u64 << 36, u64::MAX] {
            assert!(
                !Concessions {
                    access,
                    ..Concessions::default()
                }
                .valid_access()
            );
        }
    }

    #[test]
    fn finite_contact_expires_and_only_physical_events_refresh_it() {
        let mut game = war_planning_game();
        let first = game.diplomacy[&(HUMAN, 1)].first_contact;
        assert!(game.can_negotiate(1));
        game.diplomacy_playtest_tick(20);
        assert!(!game.can_negotiate(1));
        assert!(!game.negotiate(1, Pact::Alliance));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].first_contact, first);
        let city = game.cities.iter().find(|city| city.owner == 1).unwrap();
        let (x, y) = (city.x, city.y);
        game.spawn_unit(HUMAN, UnitKind::Diplomat, x, y);
        game.refresh_diplomatic_contact_at(HUMAN, x, y);
        assert!(game.can_negotiate(1));
        assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], game.turn + 20);
    }

    #[test]
    fn embassy_clause_gold_values_follow_pact_love_and_embassy_leakage_rules() {
        let game = war_planning_game();
        let embassy = Concessions::default().with_embassy();
        let empty = Concessions::default();
        assert_eq!(
            game.ai_treaty_clause_balance(1, HUMAN, embassy, empty, Pact::Alliance),
            Some((0, true))
        );
        assert_eq!(
            game.ai_treaty_clause_balance(1, HUMAN, embassy, empty, Pact::Peace),
            Some((-150, false))
        );
        assert_eq!(
            game.ai_treaty_clause_balance(1, HUMAN, embassy, empty, Pact::Ceasefire),
            Some((-2000, false))
        );
        assert_eq!(
            game.ai_treaty_clause_balance(1, HUMAN, empty, embassy, Pact::Peace),
            Some((10, true))
        );
        assert!(
            game.ai_treaty_clause_balance(1, HUMAN, empty, embassy, Pact::War)
                .is_none()
        );
    }

    #[test]
    fn embassy_clauses_are_directional_permanent_and_reject_stale_duplicates() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        let mut offer = TreatyOffer::default();
        offer.terms[1] = Concessions::default().with_embassy();
        assert!(game.execute_offer(HUMAN, 1, offer));
        assert!(game.has_real_embassy(HUMAN, 1));
        assert!(!game.has_real_embassy(1, HUMAN));
        game.diplomatic_contact_until.clear();
        assert!(game.diplomatic_meeting_possible(HUMAN, 1));
        assert!(!game.execute_offer(HUMAN, 1, offer));
        offer.terms[1] = Concessions::default();
        offer.terms[0] = Concessions::default().with_embassy();
        assert!(game.execute_offer(HUMAN, 1, offer));
        assert!(game.has_real_embassy(1, HUMAN));
    }

    #[test]
    fn an_existing_offer_can_be_accepted_after_physical_contact_expires() {
        let mut game = war_planning_game();
        let mut offer = TreatyOffer::default();
        offer.terms[0].gold = 11;
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().offer = Some(offer);
        game.diplomatic_contact_until.clear();
        assert!(!game.can_negotiate(1));
        assert!(game.accept_offer(1, offer));
        assert_eq!(game.players[HUMAN].gold, 39);
    }

    #[test]
    fn legacy_contact_migration_gives_one_bounded_window_without_callbacks() {
        let mut game = war_planning_game();
        game.diplomacy.entry((HUMAN, 2)).or_default();
        game.diplomatic_contact_until.clear();
        let rng = game.random_snapshot();
        let trust = game.diplomacy[&(HUMAN, 1)].trust;
        game.seed_legacy_diplomatic_contacts();
        assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], game.turn + 20);
        assert!(!game.diplomatic_contact_until.contains_key(&(HUMAN, 2)));
        assert_eq!(game.random_snapshot(), rng);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust, trust);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].first_contact, Some(1));
        game.turn += 20;
        game.seed_legacy_diplomatic_contacts();
        assert!(!game.can_negotiate(1));
    }

    #[test]
    fn ordinary_combat_and_city_conquest_do_not_fabricate_incidents() {
        let mut game = war_planning_game();
        let city = game.cities.iter_mut().find(|city| city.owner == 1).unwrap();
        city.size = 3;
        city.buildings
            .retain(|&building| building != Building::PALACE);
        let target = (city.x, city.y);
        let city_id = city.id;
        let origin = offset_native_by_map(target.0, target.1, -1, 0);
        let neighbor = offset_native_by_map(target.0, target.1, 1, 0);
        game.tile_mut(origin.0, origin.1).terrain = Terrain::Grassland;
        game.tile_mut(neighbor.0, neighbor.1).terrain = Terrain::Grassland;
        let defender = game.spawn_unit(1, UnitKind::Warrior, target.0, target.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == defender)
            .unwrap()
            .hp = 1;
        game.spawn_unit(2, UnitKind::Warrior, neighbor.0, neighbor.1);
        let actor = game.spawn_unit(HUMAN, UnitKind::Legion, origin.0, origin.1);
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::War;
        relation.trust[1] = 0;
        relation.cause_until[1] = 0;
        game.diplomatic_contact_until.clear();
        assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
        game.set_random_seed(1);
        assert!(game.apply(Command::Move(actor, 1, 0)));
        assert_eq!(
            game.cities
                .iter()
                .find(|city| city.id == city_id)
                .unwrap()
                .owner,
            HUMAN
        );
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 0);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].cause_until[1], 0);
        // Actual occupation still makes contact with a neighboring third party.
        assert!(game.diplomatic_contact_until[&(HUMAN, 2)] > game.turn);
    }

    #[test]
    fn a_lost_attack_does_not_refresh_contact_without_actual_movement() {
        let mut game = war_planning_game();
        let city = game.cities.iter().find(|city| city.owner == 1).unwrap();
        let target = (city.x, city.y);
        let origin = offset_native_by_map(target.0, target.1, -1, 0);
        game.tile_mut(origin.0, origin.1).terrain = Terrain::Grassland;
        game.spawn_unit(1, UnitKind::Phalanx, target.0, target.1);
        let actor = game.spawn_unit(HUMAN, UnitKind::Warrior, origin.0, origin.1);
        game.units
            .iter_mut()
            .find(|unit| unit.id == actor)
            .unwrap()
            .hp = 1;
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::War;
        relation.trust[1] = 0;
        relation.cause_until[1] = 0;
        game.diplomatic_contact_until.clear();
        game.set_random_seed(1);
        assert!(game.apply(Command::Move(actor, 1, 0)));
        assert!(game.units.iter().all(|unit| unit.id != actor));
        assert!(!game.diplomatic_contact_until.contains_key(&(HUMAN, 1)));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 0);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].cause_until[1], 0);
    }

    #[test]
    fn capturing_a_merchant_renews_ownership_contact_without_an_incident() {
        let mut game = war_planning_game();
        let (actor, merchant, _) = game.merchant_capture_playtest_setup();
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::War;
        relation.trust[1] = 0;
        relation.cause_until[1] = 0;
        game.diplomatic_contact_until.clear();
        assert!(game.apply(Command::Move(actor, 1, 0)));
        assert!(game.units.iter().all(|unit| unit.id != merchant));
        assert!(
            game.units
                .iter()
                .any(|unit| unit.owner == HUMAN && unit.kind == UnitKind::Freight)
        );
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 0);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].cause_until[1], 0);
        assert!(game.diplomatic_contact_until[&(HUMAN, 1)] > game.turn);
    }

    #[test]
    fn intercepted_nuclear_action_has_no_incident_but_a_successful_strike_does() {
        for intercepted in [true, false] {
            let mut game = Simulation::new(crate::mapgen::generate(42));
            let (missile, _, city_id) = game.nuclear_playtest_setup(intercepted);
            let city = game.cities.iter().find(|city| city.id == city_id).unwrap();
            let target = (city.x, city.y);
            let relation = game.diplomacy.entry((HUMAN, 1)).or_default();
            relation.trust[1] = 0;
            relation.cause_until[1] = 0;
            let weapon = game
                .units
                .iter()
                .find(|unit| unit.id == missile)
                .unwrap()
                .clone();
            game.nuclear_strike(&weapon, target.0, target.1);
            let relation = &game.diplomacy[&(HUMAN, 1)];
            if intercepted {
                assert_eq!(relation.trust[1], 0);
                assert_eq!(relation.cause_until[1], 0);
                assert_eq!(
                    game.cities
                        .iter()
                        .find(|city| city.id == city_id)
                        .unwrap()
                        .size,
                    4
                );
            } else {
                assert_eq!(relation.trust[1], -3000);
                assert_eq!(relation.cause_until[1], game.turn + 2);
                assert_eq!(
                    game.cities
                        .iter()
                        .find(|city| city.id == city_id)
                        .unwrap()
                        .size,
                    2
                );
            }
            assert!(game.units.iter().all(|unit| unit.id != missile));
        }
    }

    #[test]
    fn marco_polo_refreshes_global_contacts_before_turn_advance_and_leaves_grace() {
        let mut game = war_planning_game();
        let home = game
            .cities
            .iter()
            .position(|city| city.owner == HUMAN)
            .unwrap();
        game.cities[home]
            .buildings
            .push(Building::MARCO_POLOS_EMBASSY);
        game.turn = 50;
        assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
        game.refresh_diplomatic_contact_effects();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].first_contact, Some(1));
        assert_eq!(game.diplomacy[&(HUMAN, 2)].first_contact, Some(50));
        assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], 70);
        assert_eq!(game.diplomatic_contact_until[&(HUMAN, 2)], 71);
        game.cities[home]
            .buildings
            .retain(|&building| building != Building::MARCO_POLOS_EMBASSY);
        game.turn = 69;
        assert!(game.can_negotiate(1));
        game.turn = 70;
        assert!(!game.can_negotiate(1));
        assert!(game.can_negotiate(2));
        game.turn = 71;
        assert!(!game.can_negotiate(2));
    }

    #[test]
    fn virtual_embassies_allow_meetings_without_manufacturing_physical_contact() {
        let mut game = war_planning_game();
        game.cities
            .iter_mut()
            .find(|city| city.owner == HUMAN)
            .unwrap()
            .buildings
            .push(Building::MARCO_POLOS_EMBASSY);
        assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
        assert!(game.can_negotiate(2));
        assert!(game.negotiate(2, Pact::Ceasefire));
        assert_eq!(game.pact(HUMAN, 2), Pact::Ceasefire);
        assert!(!game.diplomatic_contact_until.contains_key(&(HUMAN, 2)));
        assert_eq!(game.diplomacy[&(HUMAN, 2)].first_contact, Some(0));
    }

    #[test]
    fn first_contact_skips_the_first_decrement_and_common_allies_start_at_peace() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.sign_pact(1, 2, Pact::Alliance);
        game.diplomacy.remove(&(HUMAN, 1));
        game.diplomatic_contact_until.remove(&(HUMAN, 1));
        game.make_diplomatic_contact(HUMAN, 1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert_eq!(game.diplomatic_contact_until[&(HUMAN, 1)], game.turn + 21);
    }

    #[test]
    fn a_successful_bribe_applies_its_incident_after_validation() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_incident_action_playtest());
    }

    #[test]
    fn observer_incidents_and_offer_cadence_work_together() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_observer_incident_cadence_playtest());
    }

    #[test]
    fn incident_categories_and_scope_use_native_integer_love() {
        let mut game = war_planning_game();
        for (incident, badness) in [
            (DiplomaticIncident::Snooping, 2),
            (DiplomaticIncident::Theft, 5),
            (DiplomaticIncident::UnitWeakening, 3),
            (DiplomaticIncident::UnitLoss, 5),
            (DiplomaticIncident::CityLoss, 10),
            (DiplomaticIncident::BuildingLoss, 5),
            (DiplomaticIncident::PopulationLoss, 5),
            (DiplomaticIncident::TerrainLoss, 4),
            (DiplomaticIncident::ExtraLoss, 2),
            (DiplomaticIncident::Misc, 1),
        ] {
            game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
            game.record_diplomatic_incident(HUMAN, Some(1), incident, false);
            assert_eq!(
                game.diplomacy[&(HUMAN, 1)].trust[1],
                -(badness * 1000 / 35) * 10
            );
        }
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        game.record_diplomatic_incident(HUMAN, Some(1), DiplomaticIncident::Theft, true);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -2850);
        assert_eq!(game.diplomacy[&(HUMAN, 2)].trust[1], -100);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        game.record_diplomatic_incident(HUMAN, Some(1), DiplomaticIncident::Nuclear, false);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -3000);
    }

    #[test]
    fn historical_alliance_increases_betrayal_loss_for_every_ai() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert!(game.cancel_pact(HUMAN, 1));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        assert!(game.cancel_pact(HUMAN, 1));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -4660);
        assert_eq!(game.diplomacy[&(HUMAN, 2)].trust[1], -1330);
    }

    #[test]
    fn friendly_ui_fixture_has_a_real_alternate_target_for_peace_then_alliance() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.diplomacy_playtest_friendship();
        game.diplomacy_playtest_tick(12);
        assert_eq!(game.pact_offer(1), Some(Pact::Peace));
        assert!(game.negotiate(1, Pact::Peace));
        game.diplomacy_playtest_tick(16);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert_eq!(game.pact_offer(1), None);
        game.diplomacy_playtest_unit(1).unwrap();
        game.diplomacy_playtest_tick(1);
        assert_eq!(game.pact_offer(1), Some(Pact::Alliance));
    }

    #[test]
    fn native_pair_spam_delays_first_ai_offer_without_delaying_human_offers() {
        let mut game = war_planning_game();
        game.sign_pact(1, 2, Pact::Ceasefire);
        game.make_diplomatic_contact(1, 2);
        let relation = game.diplomacy.get_mut(&(1, 2)).unwrap();
        relation.until = game.turn + 4;
        relation.trust = [2000; 2];
        assert_eq!(game.ai_diplomatic_memory[&(1, 2)].spam, 3);
        for left in [2, 1] {
            game.advance_diplomatic_offers();
            assert_eq!(game.ai_diplomatic_memory[&(1, 2)].spam, left);
            assert_eq!(game.pact(1, 2), Pact::Ceasefire);
        }
        game.advance_diplomatic_offers();
        assert_eq!(game.pact(1, 2), Pact::Armistice);
    }

    #[test]
    fn cached_diplomatic_intelligence_changes_real_negotiation_commands() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_diplomatic_intelligence_playtest());
    }

    #[test]
    fn new_phase_love_uses_previous_intelligence_before_refresh() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1100);
        game.refresh_diplomatic_intelligence();
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1050);
        game.sign_pact(HUMAN, 2, Pact::Peace);
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1000);
        game.refresh_diplomatic_intelligence();
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1100);
    }

    #[test]
    fn cached_enemy_alliance_holds_a_refusal_until_refresh_and_reloads_live() {
        let mut game = war_planning_game();
        game.tile_mut(24, 40).terrain = Terrain::Grassland;
        game.create_city(3, 24, 40);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 4000;
        for third in [2, 3] {
            game.sign_pact(HUMAN, third, Pact::Alliance);
            game.diplomacy.entry((1, third)).or_default().first_contact = Some(1);
        }
        game.refresh_diplomatic_intelligence();
        // The source retains the last matching example, not the first.
        assert_eq!(game.diplomatic_intel(1, HUMAN).allied_with_enemy, Some(3));
        for third in [2, 3] {
            game.sign_pact(HUMAN, third, Pact::Peace);
        }
        assert_eq!(
            game.pact_refusal(1, HUMAN, Pact::Alliance),
            Some("Keep your distance from our enemy for a while first.")
        );
        let rng = game.random_snapshot();
        let trust = game.diplomacy[&(HUMAN, 1)].trust;
        let path = std::env::temp_dir().join(format!("civcraft-intel-{}.save", std::process::id()));
        game.save_campaign(&path).unwrap();
        let resumed = Simulation::load_campaign(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(resumed.random_snapshot(), rng);
        assert_eq!(resumed.diplomacy[&(HUMAN, 1)].trust, trust);
        assert_eq!(resumed.diplomatic_intel(1, HUMAN).allied_with_enemy, None);
        assert!(resumed.pact_refusal(1, HUMAN, Pact::Alliance).is_none());
        // Cached pointers are intentionally transient, as native save_relations
        // does not serialize them and phase_begin(FALSE) recomputes on load.
    }

    #[test]
    fn shared_ally_greed_discount_is_a_phase_snapshot() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
        assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -2250);
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.sign_pact(1, 2, Pact::Alliance);
        assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -2250);
        game.refresh_diplomatic_intelligence();
        assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -1125);
        assert!(game.cancel_pact(1, 2));
        assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -1125);
        game.refresh_diplomatic_intelligence();
        assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Alliance), -2250);
    }

    #[test]
    fn explicit_map_and_city_deal_fixtures_establish_finite_contact() {
        let fresh = || Simulation::new(crate::mapgen::generate(42));
        assert!(fresh().treaty_map_playtest());
        assert!(fresh().city_trade_playtest());
        assert!(fresh().city_buyback_playtest());
    }

    #[test]
    fn allied_exchange_ui_fixture_consumes_a_retained_snapshot_and_accepts_the_real_offer() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        game.players[1].gold = 300;
        game.diplomacy_playtest_technologies(true);
        game.diplomacy_playtest_allied_sharing();
        let sharing = game
            .treaty_offer(1)
            .expect("actual established-alliance sharing offer");
        assert_eq!(sharing.terms[0].technology, Some(Tech::POTTERY));
        assert_eq!(sharing.terms[1].technology.unwrap().name(), "Writing");
        assert!(game.accept_offer(1, sharing));
        game.diplomacy_playtest_allied_exchange();
        let currency = Tech::all().find(|tech| tech.name() == "Currency").unwrap();
        let offer = game
            .treaty_offer(1)
            .expect("actual subsequent allied trade offer");
        let baseline = game.diplomacy_playtest_allied_trade_baseline().unwrap();
        assert_eq!(offer.pact, None);
        assert_eq!(offer.terms[0].technology, Some(currency));
        assert_eq!(offer.terms[1].technology, Some(Tech::WARRIOR_CODE));
        assert_eq!(offer.terms[1].gold, baseline + 1);
        let retained = game.ai_tech_wants.clone();
        let quoted = game.tech_sell_price(HUMAN, 1, currency).unwrap();
        let mut cold = game.clone();
        cold.ai_tech_wants.clear();
        assert_eq!(
            cold.tech_sell_price(HUMAN, 1, currency).unwrap() + 1,
            quoted
        );
        // Live economic conditions change, but no last-activities refresh
        // occurred. Quote evaluation must consume the same retained vector.
        game.players[1].rates = [0, 100, 0];
        for city in game.cities.iter_mut().filter(|city| city.owner == 1) {
            city.size = 6;
            city.specialists = [6, 0, 0];
        }
        assert_eq!(game.tech_sell_price(HUMAN, 1, currency), Some(quoted));
        assert_eq!(game.ai_tech_wants, retained);
        let human_gold = game.players[HUMAN].gold;
        assert!(game.accept_offer(1, offer));
        assert_eq!(game.players[HUMAN].gold, human_gold + baseline + 1);
        assert!(game.has_tech(HUMAN, Tech::WARRIOR_CODE));
        assert!(game.has_tech(1, currency));
        assert_eq!(game.ai_tech_wants, retained);
    }

    #[test]
    fn allied_trade_baseline_recomputes_embassy_leakage_before_advisor_premium() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        let currency = Tech::all().find(|tech| tech.name() == "Currency").unwrap();
        game.players[HUMAN].known = vec![Tech::ALPHABET, Tech::BRONZE_WORKING, currency];
        game.players[1].known = vec![Tech::ALPHABET, Tech::BRONZE_WORKING, Tech::WARRIOR_CODE];
        let before = game.diplomacy_playtest_allied_trade_baseline().unwrap();
        let mut embassy = TreatyOffer::default();
        embassy.terms = [Concessions::default().with_embassy(); 2];
        assert!(game.execute_offer(HUMAN, 1, embassy));
        let after = game.diplomacy_playtest_allied_trade_baseline().unwrap();
        assert!(after < before);
        let expected = (3 * game.target_cost(1, currency) / 2) / 2
            - (3 * game.target_cost(HUMAN, Tech::WARRIOR_CODE) / 2) / 2;
        assert_eq!(after, expected);
    }

    #[test]
    fn established_sharing_fixture_keeps_inventory_for_the_real_exchange() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        game.diplomacy_playtest_technologies(true);
        let before = [
            game.players[HUMAN].known.clone(),
            game.players[1].known.clone(),
        ];
        game.diplomacy_playtest_allied_sharing();
        assert_eq!(game.players[HUMAN].known, before[0]);
        assert_eq!(game.players[1].known, before[1]);
        let offer = game.treaty_offer(1).unwrap();
        assert_eq!(offer.pact, None);
        assert_eq!(offer.terms[0].technology, Some(Tech::POTTERY));
        assert_eq!(offer.terms[1].technology.unwrap().name(), "Writing");
        assert!(offer.terms[0].gold > 0);
        assert!(offer.terms.iter().all(|terms| terms.shared_vision()));
    }

    #[test]
    fn automatic_alliance_origination_is_a_pact_then_trusted_allied_sharing() {
        let mut game = war_planning_game();
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.first_contact = Some(game.turn);
        relation.trust[1] = 3000;
        game.advance_diplomatic_offers();
        let offer = game.treaty_offer(1).unwrap();
        assert_eq!(offer.pact, Some(Pact::Alliance));
        assert_eq!(offer.terms, [Concessions::default(); 2]);
        assert!(game.execute_offer(HUMAN, 1, offer));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
        game.advance_diplomatic_offers();
        assert!(game.treaty_offer(1).is_none());
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5001;
        game.advance_diplomatic_offers();
        let sharing = game.treaty_offer(1).unwrap();
        assert_eq!(sharing.pact, None);
        assert!(sharing.terms[1].shared_vision());
        assert!(
            sharing
                .terms
                .iter()
                .all(|terms| terms.map().is_none() && terms.city().is_none())
        );
    }

    #[test]
    fn unsafe_vision_is_revoked_even_when_target_spam_processing_is_skipped() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().vision[1] = true;
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.diplomacy.get_mut(&(HUMAN, 2)).unwrap().vision[0] = true;
        game.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
        assert!(!game.vision_safe(1, HUMAN));
        game.advance_diplomatic_offers();
        assert!(!game.diplomacy[&(HUMAN, 1)].vision[1]);
    }

    #[test]
    fn positive_most_hated_target_still_suppresses_source_offer_cadence() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
        let spam = game.ai_diplomatic_memory[&(1, HUMAN)].spam;
        game.advance_diplomatic_offers();
        assert_eq!(game.pact_offer(1), None);
        assert_eq!(game.ai_diplomatic_memory[&(1, HUMAN)].spam, spam);
    }

    #[test]
    fn phase_love_and_gift_response_match_reference_integer_order() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_diplomatic_love_playtest());
    }

    #[test]
    fn war_preparation_and_hostile_alliances_block_peacetime_love() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
        game.begin_ai_war_plan(1, HUMAN, 3, WarReason::Hatred);
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 960);
        game.ai_war_plans.clear();
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.diplomacy.entry((1, 2)).or_default().first_contact = Some(1);
        game.refresh_diplomatic_intelligence();
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 920);
    }

    #[test]
    fn visible_trespass_penalty_is_capped_in_native_love_units() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        game.tile_mut(12, 24).owner = Some(1);
        for _ in 0..9 {
            game.spawn_unit(HUMAN, UnitKind::Warrior, 12, 24);
        }
        game.advance_ai_love();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -140);
    }

    #[test]
    fn gold_can_compensate_for_missing_love_but_not_a_hard_alliance_refusal() {
        let mut game = war_planning_game();
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.pact = Pact::Ceasefire;
        relation.until = game.turn + 4;
        relation.trust[1] = 1000;
        game.players[HUMAN].gold = 1000;
        let mut offer = TreatyOffer {
            pact: Some(Pact::Peace),
            ..TreatyOffer::default()
        };
        assert_eq!(game.pact_clause_value(1, HUMAN, Pact::Peace), -62);
        assert!(!game.execute_offer(HUMAN, 1, offer));
        offer.terms[0].gold = 62;
        assert!(game.execute_offer(HUMAN, 1, offer));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 990;
        offer.pact = Some(Pact::Alliance);
        offer.terms[0].gold = 900;
        assert!(!game.execute_offer(HUMAN, 1, offer));
    }

    #[test]
    fn profitable_war_target_loses_love_while_expansion_suppresses_war_desire() {
        let mut game = war_planning_game();
        game.tile_mut(14, 26).terrain = Terrain::Grassland;
        game.create_city(1, 14, 26);
        for city in &mut game.cities {
            city.production = Production::Unit(UnitKind::Warrior);
        }
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 0;
        let desire = game.ai_war_desire(1, HUMAN);
        assert!(desire > 0);
        game.advance_ai_love();
        assert_eq!(
            game.diplomacy[&(HUMAN, 1)].trust[1],
            ((-f64::from(desire.min(50)) * 0.96) as i32) * 10
        );
        game.spawn_unit(1, UnitKind::Settlers, 12, 24);
        game.spawn_unit(1, UnitKind::Settlers, 14, 26);
        assert!(game.ai_war_desire(1, HUMAN) < desire);
    }

    #[test]
    fn an_ai_city_concession_must_leave_more_than_two_cities() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        for (x, y) in [(14, 24), (16, 24)] {
            game.tile_mut(x, y).terrain = Terrain::Grassland;
            game.create_city(1, x, y);
        }
        let city = game.cities.last().unwrap();
        let mut give = Concessions::default().with_city(city.id);
        let mut receive = Concessions::default();
        receive.gold = game.city_sale_value(city, HUMAN);
        assert!(
            game.ai_treaty_clause_balance(1, HUMAN, give, receive, Pact::Alliance)
                .is_none()
        );
        game.tile_mut(20, 24).terrain = Terrain::Grassland;
        game.create_city(1, 20, 24);
        assert!(
            game.ai_treaty_clause_balance(1, HUMAN, give, receive, Pact::Alliance)
                .is_some()
        );
        give.gold = 0;
        assert!(
            game.ai_treaty_clause_balance(1, HUMAN, give, receive, Pact::War)
                .is_none()
        );
    }

    #[test]
    fn first_contact_randomizes_each_ai_once_without_changing_human_attitude() {
        let mut game = war_planning_game();
        game.diplomacy.remove(&(1, 2));
        game.make_diplomatic_contact(1, 2);
        let relation = &game.diplomacy[&(1, 2)];
        assert_eq!(relation.first_contact, Some(game.turn));
        assert!(
            relation
                .trust
                .iter()
                .all(|&trust| (-20..=20).contains(&trust) && trust % 10 == 0)
        );
        let before = relation.trust;
        let rng = game.random_snapshot();
        game.make_diplomatic_contact(2, 1);
        assert_eq!(game.diplomacy[&(1, 2)].trust, before);
        assert_eq!(game.random_snapshot(), rng);
    }

    #[test]
    fn first_contact_notifies_the_human_once_and_diplomacy_exposes_war_status() {
        let mut game = war_planning_game();
        game.events.clear();
        game.diplomacy.remove(&(HUMAN, 2));

        game.make_diplomatic_contact(HUMAN, 2);

        assert_eq!(game.pact(HUMAN, 2), Pact::War);
        assert_eq!(game.events.len(), 1);
        assert!(game.events[0].contains("You have made contact with the"));
        assert!(game.events[0].contains(&game.players[2].name));
        assert!(game.events[0].contains(game.players[2].leader_name()));

        game.make_diplomatic_contact(2, HUMAN);
        assert_eq!(game.events.len(), 1);
    }

    #[test]
    fn a_capital_attack_does_not_guarantee_free_peace_after_twelve_turns() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        game.tile_mut(8, 24).terrain = Terrain::Grassland;
        game.create_city(HUMAN, 8, 24);
        let id = game.civil_war_playtest().unwrap();
        assert!(game.negotiate(1, Pact::Ceasefire));
        game.players[HUMAN].government = Government::Tribal;
        assert!(game.cancel_pact(HUMAN, 1));
        assert!(game.apply(Command::Move(id, 1, 0)));
        game.end_turn();
        assert!(game.negotiate(1, Pact::Ceasefire));
        game.diplomacy_playtest_technologies(false);
        // Exercise an established rival ready for another war. Native
        // dai_war_desire deliberately removes its expansion-era appetite
        // when every surviving city builds Settlers (as quiet cities now do).
        // A capital incident alone must not override that source policy.
        let garrisons: Vec<_> = game
            .cities
            .iter_mut()
            .filter(|city| city.owner == 1)
            .map(|city| {
                city.production = Production::Unit(UnitKind::Warrior);
                (city.x, city.y)
            })
            .collect();
        for (x, y) in garrisons {
            game.spawn_unit(1, UnitKind::Warrior, x, y);
        }
        assert!(game.ai_war_desire(1, HUMAN) > 0);
        game.diplomacy_playtest_tick(12);
        let key = (HUMAN, 1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Ceasefire);
        assert_eq!(game.pact_offer(1), None);
        assert!(game.diplomacy[&key].trust[1] < 1250);
        assert!(game.ai_war_desire(1, HUMAN) > 0);
    }

    #[test]
    fn balanced_trades_do_not_count_as_all_beneficial_gifts() {
        let mut game = war_planning_game();
        game.players[1].gold = 1000;
        game.players[HUMAN].gold = 1000;
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1000;
        let mut terms = TreatyOffer::default();
        terms.terms[0].gold = 110;
        terms.terms[1].gold = 110;
        assert!(game.execute_offer(HUMAN, 1, terms));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 1000);
    }

    #[test]
    fn aggression_threshold_matches_reference_clipping_and_love_rounding() {
        assert_eq!(aggression_war_threshold(50), -2290);
        assert_eq!(aggression_war_threshold(0), aggression_war_threshold(1));
        assert_eq!(aggression_war_threshold(2501), 2000);
        assert!(aggression_war_threshold(30) < aggression_war_threshold(90));
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = -2291;
        game.advance_ai_war_plans();
        assert!(game.ai_on_war_footing(1));
    }

    #[test]
    fn spaceship_construction_warns_before_launch_without_breaking_alliance() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_space_strategy_playtest());
    }

    #[test]
    fn space_race_leader_keeps_its_own_victory_strategy() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
        // AI 1 is the first player to start a spaceship (human has none).
        game.spaceships[1].add_part(0);
        game.spaceships[2].add_part(0);
        game.sign_pact(1, 2, Pact::Peace);
        game.diplomacy.get_mut(&(1, 2)).unwrap().first_contact = Some(game.turn);
        game.advance_ai_war_plans();
        assert!(!game.ai_space_warnings.contains_key(&(1, 2)));
        assert_eq!(game.ai_war_countdown(1, 2), -1);
        assert!(game.ai_space_warnings.contains_key(&(2, 1)));
    }

    #[test]
    fn spaceship_warnings_repeat_only_after_the_reference_cooldown() {
        let mut game = war_planning_game();
        let relation = game.diplomacy.get_mut(&(HUMAN, 1)).unwrap();
        relation.first_contact = Some(game.turn);
        relation.trust[1] = 9000;
        game.spaceships[HUMAN].add_part(0);
        game.advance_ai_war_plans();
        game.advance_diplomatic_offers();
        let cooldown = game.ai_space_warnings[&(1, HUMAN)];
        assert!((9..=14).contains(&cooldown));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 8000);
        for _ in 0..cooldown {
            game.advance_ai_war_plans();
            game.advance_diplomatic_offers();
        }
        assert_eq!(game.ai_space_warnings[&(1, HUMAN)], 0);
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 8000);
        game.advance_ai_war_plans();
        game.advance_diplomatic_offers();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], 7000);
    }

    #[test]
    fn fresh_casus_belli_starts_the_short_revenge_countdown() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = -1;
        game.record_provocation(1, HUMAN);
        game.advance_ai_war_plans();
        let plan = game.ai_war_plans[&(1, HUMAN)];
        assert!(matches!(plan.reason, WarReason::Behavior));
        assert_eq!(plan.countdown, (MAP_W * MAP_H / 1000).max(1) - 1);
        assert!(game.ai_on_war_footing(1));
        for _ in 0..(MAP_W * MAP_H / 1000).max(1) {
            game.advance_ai_war_plans();
        }
        assert!(game.at_war(1, HUMAN));
    }

    #[test]
    fn launched_space_leader_triggers_a_two_turn_war_plan() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
        game.spaceship_playtest_setup();
        assert!(game.launch_spaceship(HUMAN));
        game.advance_ai_war_plans();
        let plan = game.ai_war_plans[&(1, HUMAN)];
        assert!(matches!(plan.reason, WarReason::Space));
        assert_eq!(plan.countdown, 1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        game.advance_ai_war_plans();
        assert_eq!(game.ai_war_countdown(1, HUMAN), 0);
        game.advance_ai_war_plans();
        assert!(game.at_war(HUMAN, 1));
        assert_eq!(game.ai_war_countdown(1, HUMAN), -10);
    }

    #[test]
    fn hatred_prepares_war_and_stubbornness_expires_separately() {
        let mut game = war_planning_game();
        assert!(!game.ai_on_war_footing(1));
        game.advance_ai_war_plans();
        let delay = 4 + (MAP_W * MAP_H / 1000).max(1);
        assert_eq!(game.ai_war_countdown(1, HUMAN), delay - 1);
        assert!(game.ai_on_war_footing(1));
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        for _ in 0..delay {
            game.advance_ai_war_plans();
        }
        assert!(game.at_war(1, HUMAN));
        assert_eq!(game.ai_war_countdown(1, HUMAN), -20);
        assert!(!game.ai_on_war_footing(1));
        assert!(game.pact_clause_value(1, HUMAN, Pact::Ceasefire) < 0);
        for _ in 0..19 {
            game.advance_ai_war_plans();
        }
        assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 5000;
        assert!(game.pact_refusal(1, HUMAN, Pact::Ceasefire).is_none());
    }

    #[test]
    fn an_unmet_faction_is_not_an_allys_enemy_or_an_obligation_to_declare_war() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 3000;
        game.sign_pact(1, 2, Pact::Alliance);
        assert!(!game.diplomacy.contains_key(&(HUMAN, 2)));
        game.refresh_diplomatic_intelligence();
        game.advance_ai_war_plans();
        assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
        game.diplomacy.entry((HUMAN, 2)).or_default().first_contact = Some(1);
        game.refresh_diplomatic_intelligence();
        game.advance_ai_war_plans();
        assert!(matches!(
            game.ai_war_plans[&(1, HUMAN)].reason,
            WarReason::Alliance
        ));
        game.diplomacy.remove(&(HUMAN, 2));
        game.ai_war_plans.get_mut(&(1, HUMAN)).unwrap().countdown = 0;
        game.refresh_diplomatic_intelligence();
        game.advance_ai_war_plans();
        assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    }

    #[test]
    fn war_against_an_allys_enemy_is_aborted_when_the_obligation_ends() {
        let mut game = war_planning_game();
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 3000;
        game.sign_pact(1, 2, Pact::Alliance);
        game.diplomacy.entry((HUMAN, 2)).or_default().first_contact = Some(1);
        game.refresh_diplomatic_intelligence();
        game.advance_ai_war_plans();
        assert!(matches!(
            game.ai_war_plans[&(1, HUMAN)].reason,
            WarReason::Alliance
        ));
        assert!(game.ai_on_war_footing(1));
        game.sign_pact(HUMAN, 2, Pact::Peace);
        game.ai_war_plans.get_mut(&(1, HUMAN)).unwrap().countdown = 0;
        game.refresh_diplomatic_intelligence();
        game.advance_ai_war_plans();
        assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
    }

    #[test]
    fn treaty_giver_policies_alliance_jump_and_senate_callback_match_source() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(game.ai_treaty_policy_playtest());
    }

    #[test]
    fn dangerous_technology_is_an_ai_giver_veto_not_a_receiving_veto() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.diplomacy.entry((1, 2)).or_default().pact = Pact::War;
        game.players[1].known = vec![Tech::POTTERY];
        game.players[HUMAN].known.clear();
        game.players[2].known.clear();
        let mut gift = TreatyOffer::default();
        gift.terms[1].technology = Some(Tech::POTTERY);
        assert!(
            game.tech_trade_quote(1, HUMAN, Tech::POTTERY, true)
                .unwrap()
                .1
        );
        assert!(!game.execute_offer(HUMAN, 1, gift));
        assert!(!game.has_tech(HUMAN, Tech::POTTERY));
    }

    #[test]
    fn only_ai_givers_reject_unsafe_vision_and_alliance_cancellation_preserves_grants() {
        let mut game = war_planning_game();
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        game.diplomacy.get_mut(&(HUMAN, 2)).unwrap().vision[0] = true;
        game.diplomacy.entry((1, 2)).or_default().pact = Pact::Peace;
        let mut gift = TreatyOffer::default();
        gift.terms[1].access = 1;
        assert!(!game.vision_safe(1, HUMAN));
        assert!(!game.execute_offer(HUMAN, 1, gift));
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().vision = [true; 2];
        assert!(game.cancel_pact(HUMAN, 1));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].vision, [true; 2]);
        game.advance_diplomatic_offers();
        assert_eq!(game.diplomacy[&(HUMAN, 1)].vision, [true, false]);
    }

    #[test]
    fn senate_in_anarchy_aborts_revolution_and_war_tantrum_uses_native_love() {
        let mut game = war_planning_game();
        game.players[1].government = Government::Democracy;
        game.players[1].known.push(Tech::NUCLEAR_FISSION);
        game.cities[0].buildings.push(Building::UNITED_NATIONS);
        game.completed_wonders.push(Building::MANHATTAN_PROJECT);
        game.apollo_completed = false;
        game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
        assert_eq!(game.players[1].government, Government::Democracy);
        assert_eq!(game.ai_war_countdown(1, HUMAN), -1);
        assert!(!game.ai_pending_war_targets.contains_key(&1));
        assert_eq!(game.pact(1, HUMAN), Pact::Peace);
        game.cities[0].buildings.clear();
        game.players[1].government = Government::Despotism;
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().trust[1] = 1234;
        game.declare_prepared_ai_war(1, HUMAN, WarReason::Hatred);
        assert!(game.at_war(1, HUMAN));
        assert_eq!(game.diplomacy[&(HUMAN, 1)].trust[1], -1260);
    }

    #[test]
    fn senate_requires_a_revolution_before_the_planned_declaration() {
        let mut game = war_planning_game();
        game.players[1].government = Government::Democracy;
        game.ai_war_plans.insert(
            (1, HUMAN),
            WarPlan {
                countdown: 0,
                reason: WarReason::Hatred,
            },
        );
        game.advance_ai_war_plans();
        assert_eq!(game.players[1].government, Government::Anarchy);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert!(!game.ai_on_war_footing(1));
        assert_eq!(game.ai_war_countdown(1, HUMAN), -20);
        assert_eq!(game.ai_pending_war_targets.get(&1), Some(&HUMAN));
        assert_eq!(game.players[1].revolution_target, Some(Government::Anarchy));
        game.advance_ai_war_plans();
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert!(game.change_government(1, Government::Despotism));
        assert!(game.at_war(HUMAN, 1));
        assert_eq!(game.ai_war_countdown(1, HUMAN), -19);
        assert!(!game.ai_pending_war_targets.contains_key(&1));
    }

    #[test]
    fn directional_embassy_consumes_explorer_and_reduces_research_cost() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.units.clear();
        let (x, y) = (10, 20);
        game.create_city(1, x, y);
        let city = game.cities[0].id;
        let (nx, ny) = offset_native_by_map(x, y, 1, 0);
        let explorer = game.spawn_unit(HUMAN, UnitKind::Explorer, nx, ny);
        let diplomat = game.spawn_unit(HUMAN, UnitKind::Diplomat, nx, ny);
        game.players[1].known.push(Tech::POTTERY);
        assert_eq!(game.target_cost(HUMAN, Tech::POTTERY), Tech::POTTERY.cost());
        assert!(game.apply(Command::EstablishEmbassy(explorer, city)));
        assert!(game.has_embassy(HUMAN, 1));
        assert!(!game.has_embassy(1, HUMAN));
        assert!(!game.apply(Command::EstablishEmbassy(diplomat, city)));
        assert!(game.units.iter().all(|unit| unit.id != explorer));
        assert_eq!(
            game.target_cost(HUMAN, Tech::POTTERY),
            Tech::POTTERY.cost() / 2
        );
    }

    #[test]
    fn technology_price_discounts_allies_before_adding_leakage() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.players[1].known = vec![Tech::POTTERY];
        game.players[HUMAN].known.clear();
        game.players[2].known.clear();
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(45));
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(22));
        game.sign_pact(HUMAN, 2, Pact::Alliance);
        let other = game.diplomacy.entry((1, 2)).or_default();
        other.pact = Pact::Peace;
        other.trust[0] = 1000;
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), Some(33));
        game.sign_pact(1, 2, Pact::Ceasefire);
        assert_eq!(game.tech_sell_price(1, HUMAN, Tech::POTTERY), None);
    }

    #[test]
    fn low_science_can_make_a_balanced_offer_unacceptable_to_the_ai() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.diplomacy_playtest_technologies(true);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy.get_mut(&(HUMAN, 1)).unwrap().pact = Pact::Peace;
        let offer = game.build_offer(HUMAN, 1, None);
        assert!(offer.terms.iter().all(|term| term.technology.is_some()));
        assert!(game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
        game.players[1].last_science = 0;
        assert!(!game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
        game.players[1].research = -1;
        assert!(!game.ai_accepts_treaty_exchange(HUMAN, 1, &offer.terms, Pact::Peace));
    }

    #[test]
    fn stale_bundle_applies_no_gold_technology_pact_or_vision() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        game.diplomacy_playtest_technologies(true);
        let key = Simulation::relation_key(HUMAN, 1);
        let relation = game.diplomacy.entry(key).or_default();
        relation.pact = Pact::Peace;
        relation.trust = [3000; 2];
        let offer = game.build_offer(HUMAN, 1, Some(Pact::Alliance));
        assert!(offer.terms[0].gold > 0 && offer.terms[1].technology.is_some());
        game.diplomacy.get_mut(&key).unwrap().offer = Some(offer);
        game.players[HUMAN].gold = offer.terms[0].gold - 1;
        let balances = [game.players[HUMAN].gold, game.players[1].gold];
        assert!(!game.apply(Command::AcceptOffer(1, offer)));
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert_eq!([game.players[HUMAN].gold, game.players[1].gold], balances);
        assert!(!game.has_tech(HUMAN, offer.terms[1].technology.unwrap()));
        assert_eq!(game.vision_recipients(1), vec![1]);
    }

    #[test]
    fn an_armistice_disbands_units_that_fail_to_withdraw() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
        game.sign_pact(HUMAN, 1, Pact::Peace);
        game.diplomacy_playtest_tick(15);
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(game.units.iter().any(|unit| unit.id == soldier));
        game.diplomacy_playtest_tick(1);
        assert_eq!(game.pact(HUMAN, 1), Pact::Peace);
        assert!(!game.units.iter().any(|unit| unit.id == soldier));
    }

    #[test]
    fn breaking_an_alliance_removes_visitors_from_former_allied_cities() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.create_city(1, 20, 20);
        let soldier = game.spawn_unit(HUMAN, UnitKind::Warrior, 20, 20);
        game.sign_pact(HUMAN, 1, Pact::Alliance);
        assert!(game.apply(Command::CancelPact(1)));
        assert_eq!(game.pact(HUMAN, 1), Pact::Armistice);
        assert!(
            game.units
                .iter()
                .any(|unit| unit.id == soldier && (unit.x, unit.y) != (20, 20))
        );
    }

    #[test]
    fn senate_enforces_treaties_unless_provoked_or_waived() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.players[HUMAN].government = Government::Federation;
        assert!(!game.apply(Command::CancelPact(1)));
        game.record_provocation(HUMAN, 1);
        assert!(game.apply(Command::CancelPact(1)));
        assert!(game.at_war(HUMAN, 1));

        let find = |name| {
            Building::all()
                .find(|building| building.data().name == name)
                .unwrap()
        };
        game.sign_pact(HUMAN, 1, Pact::Ceasefire);
        game.cities[0].buildings.push(find("Statue of Liberty"));
        assert!(game.apply(Command::CancelPact(1)));

        game.cities[0]
            .buildings
            .retain(|building| building.data().name != "Statue of Liberty");
        game.players[HUMAN].government = Government::Tribal;
        game.cities[0].buildings.push(find("United Nations"));
        game.completed_wonders.push(find("Manhattan Project"));
        let fission = Tech::all()
            .find(|tech| *tech == Tech::NUCLEAR_FISSION)
            .unwrap();
        game.players[HUMAN].known.push(fission);
        assert!(game.has_senate(HUMAN));
        game.apollo_completed = true;
        assert!(!game.has_senate(HUMAN));
    }
}

impl Simulation {
    /// Worst-case concessions for the isolated, opt-in screenshot gallery.
    pub(crate) fn ui_review_offer(&mut self) -> TreatyOffer {
        let mut terms = Concessions {
            gold: 12345,
            technology: Some(Tech::AMPHIBIOUS_WARFARE),
            access: 3,
        };
        terms = terms
            .with_embassy()
            .with_city(self.cities.iter().find(|c| c.owner == 1).unwrap().id);
        let offer = TreatyOffer {
            pact: Some(Pact::Alliance),
            terms: [terms; 2],
        };
        self.diplomacy
            .get_mut(&Self::relation_key(HUMAN, 1))
            .unwrap()
            .offer = Some(offer);
        offer
    }
}

impl Simulation {
    pub(crate) fn ui_review_contacts(&mut self) {
        for other in 1..PLAYER_COUNT {
            self.make_diplomatic_contact(HUMAN, other);
            // Established peace lets the gallery exercise city purchase controls.
            let relation = self
                .diplomacy
                .get_mut(&Self::relation_key(HUMAN, other))
                .unwrap();
            relation.pact = Pact::Peace;
            relation.until = 0;
        }
    }
}
