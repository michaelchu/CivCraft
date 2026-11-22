//! Campaign status, government, research, and dialog projections.

use super::*;

impl Simulation {
    pub(crate) fn campaign_notifications_view(&self) -> &[super::notifications::Notification] {
        &self.notifications
    }

    #[cfg(test)]
    pub(crate) fn unread_notifications_view(&self) -> usize {
        self.notifications
            .iter()
            .filter(|notification| !notification.read)
            .count()
    }

    pub(crate) fn campaign_messages_view(&self) -> Vec<String> {
        self.notifications
            .iter()
            .map(|item| item.text.clone())
            .collect()
    }

    pub(crate) fn music_cue_view(&self, owner: usize) -> Option<MusicCueView> {
        let player = self.players.get(owner)?;
        let culture_group = player
            .civilization_id
            .as_deref()
            .and_then(crate::rules::civilizations::by_id)
            .map(|civilization| civilization.culture_group_index)
            .unwrap_or(1);
        Some(MusicCueView {
            era: self.research_era(owner),
            culture_group,
        })
    }

    pub(crate) fn game_over_view(&self) -> Option<GameOverView> {
        let message = self.game_over.clone()?;
        let standings = self
            .score_standings()
            .into_iter()
            .map(|(owner, score)| ScoreStandingView {
                name: self.players[owner].name.clone(),
                score,
            })
            .collect();
        Some(GameOverView { message, standings })
    }

    pub(crate) fn player_team_palette_view(&self, owner: usize) -> Option<u8> {
        crate::map_palette::resolve_team_palette_indices(
            self.players
                .iter()
                .map(|player| player.civilization_id.as_deref()),
        )
        .get(owner)
        .copied()
        .flatten()
    }

    pub(crate) fn initial_client_focus_view(&self) -> (Option<u32>, Option<(i32, i32)>) {
        let focus = self
            .units_owned_by(HUMAN)
            .find(|unit| unit.kind == UnitKind::Settler)
            .or_else(|| self.units_owned_by(HUMAN).next());
        let selected_unit = focus.map(|unit| unit.id);
        let position = focus.map(|unit| (unit.x, unit.y)).or_else(|| {
            self.cities_owned_by(HUMAN)
                .next()
                .map(|city| (city.x, city.y))
        });
        (selected_unit, position)
    }

    pub(crate) fn spaceship_assembly_view(&self, owner: usize) -> SpaceshipAssemblyView {
        SpaceshipAssemblyView {
            status: self.spaceship_status(owner).unwrap_or_default(),
            parts: self.spaceship_parts(owner),
        }
    }

    pub(crate) fn government_view(&self, owner: usize) -> Option<GovernmentView> {
        let player = self.players.get(owner)?;
        let status_line = if let (Some(target), Some(turn)) =
            (player.revolution_target, player.revolution_turn)
        {
            format!(
                "Anarchy  |  {} in {} turns",
                target.name(),
                turn - self.turn
            )
        } else {
            format!(
                "{}  |  Rate limit {}%",
                player.government.name(),
                player.government.max_rate()
            )
        };
        let options = Government::targets()
            .iter()
            .copied()
            .map(|government| {
                let support_description =
                    "Unit upkeep: C2C3 per-unit costs and city allowances".to_owned();
                let requirement_description = government.requirement().and_then(|tech| {
                    (!player.known.contains(&tech)).then(|| format!("Requires {}", tech.name()))
                });
                GovernmentOptionView {
                    government,
                    name: government.name(),
                    enabled: self.can_change_government(owner, government),
                    current: player.government == government,
                    support_description,
                    requirement_description,
                }
            })
            .collect();
        let in_anarchy = player.government == Government::during_revolution();
        let max_rate = player.government.max_rate();
        Some(GovernmentView {
            rates: if in_anarchy {
                [0, 0, 100]
            } else {
                player.rates
            },
            max_rate,
            rates_locked: in_anarchy,
            leader_line: format!("{} · {}", player.name, player.leader_name()),
            status_line,
            options,
            can_mobilize: self.can_mobilize(owner),
            mobilization_label: if player.mobilized {
                "Mobilized · peace treaty required"
            } else if self.can_mobilize(owner) {
                "Mobilize for war"
            } else {
                "Mobilization requires Nationalism"
            },
        })
    }

    pub(crate) fn top_bar_view(&self, owner: usize) -> Option<TopBarView> {
        let player = self.players.get(owner)?;
        let research = player.researching;
        Some(TopBarView {
            year: self.year,
            gold: player.gold,
            government_name: player.government.name(),
            research_name: research.name(),
            research_points: player.research,
            research_cost: self.target_cost(owner, research),
            spaceship_status: self.spaceship_status(owner),
        })
    }

    pub(crate) fn research_progress_view(&self, owner: usize) -> Option<ResearchProgressView> {
        let player = self.players.get(owner)?;
        Some(ResearchProgressView {
            goal: player.research_goal,
            researching: player.researching,
            points: player.research,
            target_cost: self.target_cost(owner, player.researching),
        })
    }

    pub(crate) fn technology_research_view(
        &self,
        owner: usize,
        tech: Tech,
    ) -> Option<TechnologyResearchView> {
        self.players.get(owner)?;
        Some(TechnologyResearchView {
            known: self.has_tech(owner, tech),
            available: self.tech_available(owner, tech),
            target_cost: self.target_cost(owner, tech),
        })
    }

    pub(crate) fn research_technology_known(&self, owner: usize, tech: Tech) -> Option<bool> {
        self.players.get(owner)?;
        Some(self.has_tech(owner, tech))
    }
}
