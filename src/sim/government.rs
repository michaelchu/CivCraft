use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Government {
    Anarchy,
    Tribal,
    Despotism,
    Monarchy,
    Communism,
    Fundamentalism,
    Federation,
    Republic,
    Democracy,
}

impl Government {
    pub(crate) const TARGETS: [Self; 8] = [
        Self::Tribal,
        Self::Despotism,
        Self::Monarchy,
        Self::Communism,
        Self::Fundamentalism,
        Self::Federation,
        Self::Republic,
        Self::Democracy,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Anarchy => "Anarchy",
            Self::Tribal => "Tribal",
            Self::Despotism => "Despotism",
            Self::Monarchy => "Monarchy",
            Self::Communism => "Communism",
            Self::Fundamentalism => "Fundamentalism",
            Self::Federation => "Federation",
            Self::Republic => "Republic",
            Self::Democracy => "Democracy",
        }
    }

    pub(crate) fn requirement(self) -> Option<&'static str> {
        match self {
            Self::Anarchy | Self::Tribal | Self::Despotism => None,
            Self::Monarchy => Some("Monarchy"),
            Self::Communism => Some("Communism"),
            Self::Fundamentalism => Some("Theology"),
            Self::Federation => Some("Economics"),
            Self::Republic => Some("The Republic"),
            Self::Democracy => Some("Democracy"),
        }
    }

    pub(crate) fn max_rate(self) -> i32 {
        match self {
            Self::Anarchy => 100,
            Self::Tribal | Self::Despotism => 60,
            Self::Monarchy | Self::Fundamentalism => 70,
            Self::Communism | Self::Republic => 80,
            Self::Federation | Self::Democracy => 90,
        }
    }

    // Shield cost per eligible unit converted to gold, then city allowances.
    pub(crate) fn unit_support(self) -> (i32, i32, i32) {
        match self {
            Self::Anarchy | Self::Tribal => (0, 2, 0),
            Self::Despotism => (1, 0, 2),
            Self::Monarchy => (1, 0, 3),
            Self::Communism => (0, 3, 0),
            Self::Fundamentalism | Self::Federation => (2, 0, 4),
            Self::Republic => (0, 1, 0),
            Self::Democracy => (2, 0, 2),
        }
    }

    pub(super) fn base_waste(self) -> (i32, i32) {
        match self {
            Self::Anarchy => (30, 0),
            Self::Tribal => (0, 30),
            Self::Despotism => (10, 20),
            Self::Monarchy => (20, 10),
            Self::Communism => (0, 30),
            Self::Fundamentalism => (15, 15),
            Self::Federation => (30, 0),
            Self::Republic => (5, 25),
            Self::Democracy => (25, 5),
        }
    }

    pub(super) fn empire_size(self) -> i32 {
        match self {
            Self::Anarchy => 0,
            Self::Tribal => 12,
            Self::Despotism => 10,
            Self::Monarchy => 14,
            Self::Communism => 28,
            Self::Fundamentalism => 20,
            Self::Federation => 24,
            Self::Republic => 16,
            Self::Democracy => 32,
        }
    }
}

impl Simulation {
    pub(super) fn government_output_waste(
        &self,
        city: &City,
        shields: i32,
        trade: i32,
    ) -> (i32, i32) {
        let player = &self.players[city.owner];
        let government = player.government;
        let (shield_base, trade_base) = government.base_waste();
        let distance = self
            .cities
            .iter()
            .filter(|center| {
                center.owner == city.owner
                    && (center.has_building("Palace")
                        || center.has_building("Ecclesiastical Palace"))
            })
            .map(|center| self.map_distance(city.x, city.y, center.x, center.y))
            .min();
        let has_tech = |name| player.known.iter().any(|tech| tech.name() == name);
        let shield_distance =
            if matches!(government, Government::Communism | Government::Federation) {
                0
            } else {
                1 + i32::from(!has_tech("Trade"))
            };
        let trade_distance = match government {
            Government::Communism => 0,
            Government::Federation => i32::from(!has_tech("The Corporation")),
            _ => 1 + i32::from(!has_tech("The Corporation")),
        };
        let relief = 50
            * (i32::from(city.has_building("Courthouse"))
                + i32::from(
                    city.has_building("Palace") || city.has_building("Ecclesiastical Palace"),
                ));
        let net = |gross, base, per_tile| {
            if distance.is_none() && per_tile > 0 {
                return 0;
            }
            let waste = percent_of(gross, base + distance.unwrap_or(0) * per_tile);
            gross - (waste - percent_of(waste, relief)).clamp(0, gross)
        };
        if city.owner == BARBARIANS {
            return (shields, trade);
        }
        (
            net(shields, shield_base, shield_distance),
            net(trade, trade_base, trade_distance),
        )
    }

    pub(super) fn owns_wonder(&self, owner: usize, name: &str) -> bool {
        self.cities
            .iter()
            .any(|city| city.owner == owner && city.has_building(name))
    }

    pub(super) fn handle_government_tech_loss(&mut self, owner: usize, lost: Tech) {
        let player = &self.players[owner];
        let government = player.revolution_target.unwrap_or(player.government);
        if government.requirement() != Some(lost.name())
            || self.owns_wonder(owner, "Statue of Liberty")
        {
            return;
        }
        let available: Vec<_> = Government::TARGETS
            .into_iter()
            .filter(|candidate| self.can_change_government(owner, *candidate))
            .collect();
        if !available.is_empty() {
            let target = available[self.rand_below(available.len() as i32) as usize];
            self.change_government(owner, target);
        }
    }

    pub(crate) fn can_change_government(&self, owner: usize, target: Government) -> bool {
        let player = &self.players[owner];
        target != Government::Anarchy
            && target != player.government
            && player.revolution_target != Some(target)
            && (player.revolution_turn != Some(self.turn)
                || self.owns_wonder(owner, "Statue of Liberty"))
            && (target
                .requirement()
                .is_none_or(|name| player.known.iter().any(|tech| tech.name() == name))
                || self.owns_wonder(owner, "Statue of Liberty"))
    }

    pub(super) fn change_government(&mut self, owner: usize, target: Government) -> bool {
        if !self.can_change_government(owner, target) {
            return false;
        }
        let player = &self.players[owner];
        let finish = if self.owns_wonder(owner, "Statue of Liberty") {
            self.turn
        } else if player.government == Government::Anarchy {
            player.revolution_turn.unwrap()
        } else {
            // RANDQUICK with the reference default length (5) and six nations.
            let max_turns = (5 - self.government_adoptions[target as usize]).max(1);
            self.turn + 1 + self.rand_below(max_turns)
        };
        let player = &mut self.players[owner];
        player.government = Government::Anarchy;
        player.revolution_target = Some(target);
        player.revolution_turn = Some(finish);
        if owner == HUMAN && finish > self.turn {
            self.add_event(format!(
                "Revolution toward {}. Anarchy for {} turns.",
                target.name(),
                finish - self.turn
            ));
        }
        self.finish_revolutions();
        true
    }

    pub(super) fn finish_revolutions(&mut self) {
        for owner in 0..self.players.len() {
            if owner == BARBARIANS {
                continue;
            }
            let player = &mut self.players[owner];
            if player.revolution_turn.is_some_and(|turn| turn <= self.turn)
                && let Some(target) = player.revolution_target.take()
            {
                player.government = target;
                self.government_adoptions[target as usize] += 1;
                let cap = target.max_rate();
                let mut surplus = 0;
                for rate in &mut player.rates {
                    surplus += (*rate - cap).max(0);
                    *rate = (*rate).min(cap);
                }
                for rate in &mut player.rates {
                    let added = surplus.min(cap - *rate);
                    *rate += added;
                    surplus -= added;
                }
                if owner == HUMAN {
                    self.add_event(format!("{} is now your government.", target.name()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anarchy_waives_normal_technology_upkeep() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        game.players[HUMAN].known = Tech::all().collect();
        let upkeep = game.technology_upkeep(HUMAN);
        assert!(upkeep > 0);
        let known = game.players[HUMAN].known.len();
        game.apply_research_upkeep(HUMAN, 0);
        assert_eq!(game.players[HUMAN].known.len(), known - 1);
        game.players[HUMAN].government = Government::Anarchy;
        assert_eq!(game.technology_upkeep(HUMAN), 0);
    }

    #[test]
    fn government_upkeep_keeps_caravels_on_shields_and_applies_gold_allowances() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        game.create_city(HUMAN, 10, 10);
        let city_id = game.cities[0].id;
        for kind in [UnitKind::Warrior; 5]
            .into_iter()
            .chain([UnitKind::Caravel])
        {
            let id = game.spawn_unit(HUMAN, kind, 10, 10);
            let unit = game.units.last_mut().unwrap();
            unit.home_city = Some(city_id);
            unit.home_order = id;
        }
        for (government, shields, gold) in [
            (Government::Anarchy, 4, 0),
            (Government::Tribal, 4, 0),
            (Government::Despotism, 1, 3),
            (Government::Monarchy, 1, 2),
            (Government::Communism, 3, 0),
            (Government::Fundamentalism, 1, 6),
            (Government::Federation, 1, 6),
            (Government::Republic, 5, 0),
            (Government::Democracy, 1, 8),
        ] {
            game.players[HUMAN].government = government;
            let upkeep = game.city_unit_upkeep(&game.cities[0]);
            assert_eq!(
                (upkeep.food, upkeep.shields, upkeep.gold),
                (2, shields, gold),
                "{government:?}"
            );
        }
        let paid = game.city_gold_upkeep(&game.cities[0]);
        let first = paid[0].0;
        game.players[HUMAN].gold = -8;
        game.balance_unit_gold_upkeep(HUMAN, &paid);
        assert_eq!(game.players[HUMAN].gold, 0);
        assert!(!game.units.iter().any(|unit| unit.id == first));
        assert!(game.units.iter().any(|unit| unit.kind == UnitKind::Caravel));
    }

    #[test]
    fn revolution_respects_unlocks_anarchy_and_rate_caps() {
        let mut game = Simulation::new(crate::mapgen::generate(42));
        assert!(!game.apply(Command::Government(Government::Monarchy)));
        assert!(game.apply(Command::Government(Government::Despotism)));
        let finish = game.players[HUMAN].revolution_turn.unwrap();
        assert!((1..=5).contains(&(finish - game.turn)));
        assert_eq!(game.players[HUMAN].government, Government::Anarchy);
        assert!(!game.apply(Command::RaiseRate(1)));
        game.turn = finish;
        game.finish_revolutions();
        assert_eq!(game.players[HUMAN].government, Government::Despotism);
        assert!(!game.apply(Command::Government(Government::Tribal)));
    }
}
