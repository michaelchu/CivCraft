//! Read-only projections for client screens.

use super::*;
use crate::rules::TechEra;

/// The values needed to draw the selected unit's context card, without
/// exposing player, tile, transport, or movement-rule lookups to that view.
#[derive(Clone)]
pub(crate) struct UnitContextView {
    pub(crate) kind: UnitKind,
    pub(crate) owner: usize,
    pub(crate) era_index: usize,
    pub(crate) owner_name: String,
    pub(crate) terrain_name: &'static str,
    pub(crate) status: String,
    pub(crate) moves_left: i32,
}

/// The computed values needed by the selected city's compact context card.
pub(crate) struct CityContextView {
    pub(crate) name: String,
    pub(crate) size: i32,
    pub(crate) foreign_citizens: i32,
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) trade: i32,
    pub(crate) illness: i32,
    pub(crate) food_store: i32,
    pub(crate) growth_box: i32,
    pub(crate) production_name: &'static str,
    pub(crate) production_stock: i32,
    pub(crate) production_cost: i32,
    pub(crate) culture: i64,
    pub(crate) culture_per_turn: i64,
    pub(crate) next_border_threshold: Option<i64>,
}

/// Research progress and goal for a single player.
pub(crate) struct ResearchProgressView {
    pub(crate) goal: Tech,
    pub(crate) researching: Tech,
    pub(crate) points: i32,
    pub(crate) target_cost: i32,
}

/// Simulation facts the research screen needs for one technology.
pub(crate) struct TechnologyResearchView {
    pub(crate) known: bool,
    pub(crate) available: bool,
    pub(crate) target_cost: i32,
}

/// Display-ready turn, economy and control state for the HUD.
pub(crate) struct TopBarView {
    pub(crate) year: i32,
    pub(crate) gold: i32,
    pub(crate) government_name: &'static str,
    pub(crate) research_name: &'static str,
    pub(crate) research_points: i32,
    pub(crate) research_cost: i32,
    pub(crate) spaceship_status: Option<String>,
}

pub(crate) struct GovernmentView {
    pub(crate) leader_line: String,
    pub(crate) status_line: String,
    pub(crate) options: Vec<GovernmentOptionView>,
    /// Science, tax and luxury percentages as shown; anarchy forces all to luxury.
    pub(crate) rates: [i32; 3],
    /// The government's maximum share for any one rate.
    pub(crate) max_rate: i32,
    /// True during anarchy, when the rates cannot be changed.
    pub(crate) rates_locked: bool,
    pub(crate) can_mobilize: bool,
    pub(crate) mobilization_label: &'static str,
}

pub(crate) struct GovernmentOptionView {
    pub(crate) government: Government,
    pub(crate) name: &'static str,
    pub(crate) enabled: bool,
    pub(crate) current: bool,
    pub(crate) support_description: String,
    pub(crate) requirement_description: Option<String>,
}

pub(crate) struct DiplomacyRivalView {
    pub(crate) owner: usize,
    pub(crate) name: String,
    pub(crate) leader_name: String,
    pub(crate) pact: Pact,
    pub(crate) pact_turns: i32,
    pub(crate) has_embassy: bool,
    pub(crate) gives_shared_vision: bool,
    pub(crate) senate_veto: bool,
    pub(crate) vassal_status: Option<String>,
    pub(crate) can_cancel_pact: bool,
    pub(crate) can_release: bool,
    pub(crate) pact_offer: Option<Pact>,
    pub(crate) can_negotiate: bool,
    pub(crate) treaty_offer: Option<TreatyOffer>,
}

pub(crate) struct DiplomacyMeetingView {
    pub(crate) leader_line: String,
    pub(crate) can_negotiate: bool,
    pub(crate) ai_offer: Option<TreatyOffer>,
    pub(crate) pact: Pact,
    pub(crate) next_pact: Option<Pact>,
    pub(crate) offer: TreatyOffer,
    pub(crate) sides: [TreatySideView; 2],
    pub(crate) vassal_offer: Result<TreatyOffer, String>,
}

pub(crate) struct TreatySideView {
    pub(crate) name: String,
    pub(crate) gold: i32,
    pub(crate) technologies: Vec<(Tech, String)>,
    pub(crate) can_offer_embassy: bool,
    pub(crate) can_offer_vision: bool,
    pub(crate) cities: Vec<(u32, String)>,
}

pub(crate) struct SpaceshipAssemblyView {
    pub(crate) status: String,
    /// The ten Civ III components and whether each has been built.
    pub(crate) parts: Vec<(&'static str, bool)>,
}

#[derive(Clone)]
pub(crate) struct UnitActionView {
    pub(crate) unit_name: &'static str,
    pub(crate) owner: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) work_left: i32,
    pub(crate) upgrade: Option<(UnitKind, i32, bool)>,
    pub(crate) paradrop: bool,
    pub(crate) bombard: bool,
    pub(crate) precision_bombard: bool,
    pub(crate) air_superiority: bool,
    pub(crate) rebase: bool,
    pub(crate) recon: bool,
    pub(crate) has_order: bool,
    pub(crate) capture_units: bool,
    pub(crate) form_army: bool,
    pub(crate) cities: Vec<UnitCityActionView>,
    pub(crate) nuclear_weapon: bool,
    pub(crate) nuclear_strike: bool,
    pub(crate) transport_action: Option<String>,
    pub(crate) unload_all: bool,
    pub(crate) can_connect: bool,
    pub(crate) can_patrol: bool,
    pub(crate) found_city: Option<bool>,
    pub(crate) road_action: Option<(String, bool)>,
    pub(crate) road_work: WorkKind,
    pub(crate) irrigation_action: Option<(String, bool)>,
    pub(crate) irrigation_work: WorkKind,
    pub(crate) mine_action: Option<(String, bool)>,
    pub(crate) mine_work: WorkKind,
    pub(crate) terrain_work: Vec<WorkKind>,
    pub(crate) clean_targets: Vec<(Contamination, String)>,
    pub(crate) base_work: Vec<(Base, String)>,
    pub(crate) pillage_targets: Vec<(PillageTarget, String)>,
    pub(crate) fortify: bool,
    pub(crate) set_home_city: bool,
    pub(crate) automate: bool,
    pub(crate) automate_label: &'static str,
    pub(crate) can_disband: bool,
    pub(crate) disband_cities: Vec<(u32, String)>,
}

#[derive(Clone)]
pub(crate) struct UnitCityActionView {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) can_airlift: bool,
    pub(crate) can_join: bool,
    pub(crate) can_complete_production: bool,
}

/// City identity needed by the minimap, with civilization color already resolved.
pub(crate) struct MinimapCityView {
    pub(crate) id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) owner: usize,
}

pub(crate) struct MinimapTileView {
    pub(crate) terrain: Terrain,
    pub(crate) seen: bool,
    pub(crate) visible: bool,
}

/// Display-ready tile state for the map painter. Strategic resources are
/// filtered here so the renderer does not need to query campaign rules.
#[derive(Clone, Copy)]
pub(crate) struct RenderTileView {
    pub(crate) terrain: Terrain,
    pub(crate) base_terrain: TerrainBase,
    pub(crate) resource: Option<Resource>,
    pub(crate) hut: bool,
    pub(crate) river_edges: u8,
    pub(crate) road: bool,
    pub(crate) railroad: bool,
    pub(crate) irrigation: bool,
    pub(crate) farmland: bool,
    pub(crate) pollution: bool,
    pub(crate) fallout: bool,
    pub(crate) craters: bool,
    pub(crate) base: Option<Base>,
    pub(crate) mine: bool,
    pub(crate) oil_well: bool,
    pub(crate) oil_platform: bool,
    pub(crate) seen: bool,
    pub(crate) visible: bool,
}

pub(crate) struct SelectableUnitView {
    pub(crate) id: u32,
    pub(crate) transported: bool,
}

pub(crate) struct ScoreStandingView {
    pub(crate) name: String,
    pub(crate) score: i32,
}

pub(crate) struct GameOverView {
    pub(crate) message: String,
    pub(crate) standings: Vec<ScoreStandingView>,
}

pub(crate) struct UnitFidgetView {
    pub(crate) id: u32,
    pub(crate) kind: UnitKind,
    pub(crate) era_index: usize,
    pub(crate) state: UnitState,
    pub(crate) working: bool,
}

pub(crate) struct MusicCueView {
    pub(crate) era: TechEra,
    pub(crate) culture_group: i32,
}

/// Render-ready copy of a transient simulation event. It replaces campaign
/// entity records with the same projections used by the normal world painter.
#[derive(Clone)]
pub(crate) enum PresentationEventView {
    Move {
        unit: RenderedUnitView,
        from: (i32, i32),
        to: (i32, i32),
        duration: f64,
    },
    Capture {
        unit: RenderedUnitView,
        duration: f64,
    },
    FoundCity {
        unit: RenderedUnitView,
        duration: f64,
    },
    WorkStarted {
        unit: RenderedUnitView,
        kind: WorkKind,
        duration: f64,
    },
    Fortify {
        unit: RenderedUnitView,
        duration: f64,
    },
    EnterHut {
        unit: RenderedUnitView,
        duration: f64,
    },
    Combat(Box<CombatPresentationView>),
    AirMission(Box<AirMissionPresentationView>),
}

#[derive(Clone)]
pub(crate) enum AirMissionPresentationKind {
    Bombard {
        defender: Option<(RenderedUnitView, i32, i32)>,
        city: Option<RenderedCityView>,
    },
    CruiseMissileStrike {
        defender: Option<(RenderedUnitView, i32, i32)>,
        city: Option<RenderedCityView>,
    },
    NuclearStrike {
        intercepted_by_sdi: bool,
    },
    Recon,
    Rebase,
    SamIntercept {
        shot_down: bool,
    },
    AaUnitIntercept {
        aa_unit: RenderedUnitView,
        shot_down: bool,
    },
}

#[derive(Clone)]
pub(crate) struct AirMissionPresentationView {
    pub(crate) aircraft: RenderedUnitView,
    pub(crate) from: (i32, i32),
    pub(crate) to: (i32, i32),
    pub(crate) outbound_facing: u8,
    pub(crate) return_facing: u8,
    pub(crate) kind: AirMissionPresentationKind,
    /// Duration of each flight leg (`from -> to` and, when applicable, `to -> from`).
    pub(crate) fly_duration: f64,
    /// Duration of the target-tile action (bombing run, SAM strike, AA fire).
    pub(crate) action_duration: f64,
    /// Duration of the post-action crash clip when shot down.
    pub(crate) outcome_duration: f64,
    pub(crate) duration: f64,
}

#[derive(Clone)]
pub(crate) struct CombatPresentationView {
    pub(crate) attacker: RenderedUnitView,
    pub(crate) defender: RenderedUnitView,
    pub(crate) hp: Vec<(i32, i32)>,
    pub(crate) battle_ended: bool,
    pub(crate) city: Option<RenderedCityView>,
    pub(crate) bombard_effects: bool,
    /// Client may replace these defaults with the resolved source clip timings.
    pub(crate) round_durations: Vec<f64>,
    pub(crate) result_duration: f64,
    pub(crate) impact_duration: f64,
    pub(crate) duration: f64,
}

impl PresentationEventView {
    pub(crate) fn duration(&self) -> f64 {
        match self {
            Self::Move { duration, .. }
            | Self::Capture { duration, .. }
            | Self::FoundCity { duration, .. }
            | Self::WorkStarted { duration, .. }
            | Self::Fortify { duration, .. }
            | Self::EnterHut { duration, .. } => *duration,
            Self::Combat(combat) => combat.duration,
            Self::AirMission(mission) => mission.duration,
        }
    }

    pub(crate) fn actors(&self) -> impl Iterator<Item = &RenderedUnitView> {
        match self {
            Self::Move { unit, .. }
            | Self::Capture { unit, .. }
            | Self::FoundCity { unit, .. }
            | Self::WorkStarted { unit, .. }
            | Self::Fortify { unit, .. }
            | Self::EnterHut { unit, .. } => [Some(unit), None],
            Self::Combat(combat) => [Some(&combat.attacker), Some(&combat.defender)],
            Self::AirMission(mission) => match &mission.kind {
                AirMissionPresentationKind::Bombard {
                    defender: Some((defender, _, _)),
                    ..
                }
                | AirMissionPresentationKind::CruiseMissileStrike {
                    defender: Some((defender, _, _)),
                    ..
                } => [Some(&mission.aircraft), Some(defender)],
                AirMissionPresentationKind::AaUnitIntercept { aa_unit, .. } => {
                    [Some(&mission.aircraft), Some(aa_unit)]
                }
                _ => [Some(&mission.aircraft), None],
            },
        }
        .into_iter()
        .flatten()
    }
}

/// City management data with all model lookups and production rules resolved.
pub(crate) struct CityManagerView {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) owner: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) era_index: usize,
    pub(crate) size: i32,
    pub(crate) culture: i64,
    pub(crate) culture_per_turn: i64,
    pub(crate) next_border_threshold: Option<i64>,
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) production_name: String,
    pub(crate) production_cost: i32,
    pub(crate) bought_production: bool,
    pub(crate) specialists: [i32; 3],
    pub(crate) tile_workers: i32,
    pub(crate) governor: CityGovernor,
    pub(crate) worklist: Vec<CityWorklistItemView>,
    pub(crate) food_net: i32,
    pub(crate) shield_net: i32,
    pub(crate) science: i32,
    pub(crate) gold_net: i32,
    pub(crate) luxury: i32,
    pub(crate) disorder: bool,
    pub(crate) growth_box: i32,
    pub(crate) growth_limited: bool,
    pub(crate) rush_cost: Option<i32>,
    pub(crate) treasury: i32,
    pub(crate) luxuries: Vec<(Resource, &'static str)>,
    pub(crate) buildings: Vec<BuildingProductionView>,
    pub(crate) units: Vec<UnitProductionView>,
    pub(crate) map_tiles: Vec<CityMapTileView>,
}

/// Displayable city identity and appearance, derived from the current or last
/// observed city state without exposing a campaign `City` record.
#[derive(Clone)]
pub(crate) struct RenderedCityView {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) owner: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) size: i32,
    pub(crate) civilization_id: Option<String>,
    pub(crate) art_style: u8,
    pub(crate) art_era: u8,
    pub(crate) occupied: bool,
    pub(crate) disorder: bool,
    pub(crate) capital: bool,
    pub(crate) growth_label: Option<String>,
    pub(crate) production_label: Option<String>,
}

/// Unit data consumed by world rendering and animation selection.
#[derive(Clone)]
pub(crate) struct RenderedUnitView {
    pub(crate) id: u32,
    pub(crate) kind: UnitKind,
    pub(crate) owner: usize,
    pub(crate) era_index: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) facing: u8,
    pub(crate) transport: Option<u32>,
    pub(crate) is_transporter: bool,
    pub(crate) work_kind: Option<WorkKind>,
    pub(crate) state: UnitState,
    pub(crate) moves_left: i32,
    pub(crate) movement_rate: i32,
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
    pub(crate) visible: bool,
    pub(crate) context: Option<UnitContextView>,
}

pub(crate) struct CityWorklistItemView {
    pub(crate) production: Production,
    pub(crate) name: String,
    pub(crate) target: Production,
    pub(crate) target_name: String,
    pub(crate) available: bool,
}

pub(crate) struct BuildingProductionView {
    pub(crate) building: Building,
    pub(crate) name: &'static str,
    pub(crate) cost_label: String,
    pub(crate) blocker: Option<String>,
    pub(crate) built: bool,
    pub(crate) current: bool,
    pub(crate) can_change: bool,
    pub(crate) can_queue: bool,
    pub(crate) sale_price: Option<i32>,
}

pub(crate) struct UnitProductionView {
    pub(crate) kind: UnitKind,
    pub(crate) cost: i32,
    pub(crate) available: bool,
    pub(crate) current: bool,
    pub(crate) can_change: bool,
    pub(crate) can_queue: bool,
}

pub(crate) struct CityMapTileView {
    pub(crate) dx: i32,
    pub(crate) dy: i32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) is_city: bool,
    pub(crate) seen: bool,
    pub(crate) worked: bool,
    pub(crate) can_assign: bool,
    pub(crate) yields: (i32, i32, i32),
}

mod city;
mod diplomacy;
mod map;
mod navigation;
mod panels;
mod units;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_context_projection_includes_rule_and_map_values() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let unit = sim.units_owned_by(HUMAN).next().unwrap();
        let view = sim.unit_context_view(unit);
        assert_eq!(view.kind, unit.kind);
        assert_eq!(view.owner, unit.owner);
        assert_eq!(view.moves_left, unit.moves_left);
        assert_eq!(view.terrain_name, sim.tile(unit.x, unit.y).terrain.name());
    }

    #[test]
    fn selectable_units_skip_automated_sentry_and_fortifying_or_fortified_units() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        for unit in &mut sim.units {
            if unit.owner == HUMAN {
                unit.moves_left = 0;
            }
        }

        let automated = sim.spawn_unit(HUMAN, UnitKind::Warrior, 10, 20);
        sim.units
            .iter_mut()
            .find(|unit| unit.id == automated)
            .unwrap()
            .automated = true;

        let sentry = sim.spawn_unit(HUMAN, UnitKind::Warrior, 11, 20);
        sim.units
            .iter_mut()
            .find(|unit| unit.id == sentry)
            .unwrap()
            .state = UnitState::Sentry;

        let fortified_military = sim.spawn_unit(HUMAN, UnitKind::Warrior, 12, 20);
        sim.units
            .iter_mut()
            .find(|unit| unit.id == fortified_military)
            .unwrap()
            .state = UnitState::Fortified;

        let fortifying_worker = sim.spawn_unit(HUMAN, UnitKind::Worker, 15, 20);
        sim.units
            .iter_mut()
            .find(|unit| unit.id == fortifying_worker)
            .unwrap()
            .state = UnitState::Fortifying;

        let worker = sim.spawn_unit(HUMAN, UnitKind::Worker, 13, 20);

        let ordinary_military = sim.spawn_unit(HUMAN, UnitKind::Warrior, 14, 20);

        let selectable: Vec<u32> = sim
            .selectable_units_view()
            .into_iter()
            .map(|unit| unit.id)
            .collect();

        assert!(!selectable.contains(&automated));
        assert!(!selectable.contains(&sentry));
        assert!(!selectable.contains(&fortified_military));
        assert!(!selectable.contains(&fortifying_worker));
        assert!(selectable.contains(&worker));
        assert!(selectable.contains(&ordinary_military));
    }

    #[test]
    fn selectable_units_skip_units_with_active_goto_orders() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let unit_id = sim.units_owned_by(HUMAN).next().unwrap().id;
        let unit = sim
            .units
            .iter_mut()
            .find(|unit| unit.id == unit_id)
            .unwrap();
        unit.moves_left = 1;
        unit.work_left = 0;
        unit.automated = false;
        unit.state = UnitState::Idle;
        sim.unit_goto_tiles.insert(unit_id, (0, 0));

        assert!(
            !sim.selectable_units_view()
                .iter()
                .any(|unit| unit.id == unit_id)
        );

        sim.unit_goto_tiles.remove(&unit_id);
        sim.units
            .iter_mut()
            .find(|unit| unit.id == unit_id)
            .unwrap()
            .order = Some(UnitOrder::Patrol {
            origin: (1, 1),
            destination: (2, 2),
            returning: false,
        });
        assert!(
            !sim.selectable_units_view()
                .iter()
                .any(|unit| unit.id == unit_id)
        );
    }

    #[test]
    fn city_context_projection_uses_the_current_city_forecast() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.create_city(HUMAN, 20, 24);
        let id = sim.city_at(20, 24).unwrap().id;
        let city = sim.city_by_id(id).unwrap();
        let forecast = sim.city_forecast(city);
        let view = sim.city_context_view(id).unwrap();
        assert_eq!(
            (view.food, view.shields, view.trade),
            (forecast.food, forecast.shields, forecast.trade)
        );
        assert_eq!(view.food_store, city.food);
        assert_eq!(view.production_name, city.production.name());
        assert_eq!(view.production_cost, city.production.cost());
        assert!(sim.city_context_view(u32::MAX).is_none());
    }

    #[test]
    fn settlers_found_and_explore_while_workers_keep_terrain_work() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.cities.clear();
        for tile in &mut sim.tiles {
            tile.terrain = Terrain::Grassland;
            tile.visible = true;
        }
        let settler = sim.spawn_unit(HUMAN, UnitKind::Settler, 12, 24);
        let worker = sim.spawn_unit(HUMAN, UnitKind::Worker, 14, 24);

        let view = sim.unit_action_view(settler).unwrap();
        assert!(view.found_city.is_some());
        assert!(
            view.automate && view.automate_label == "Explore",
            "Settlers explore, not work"
        );
        assert!(view.road_action.is_none());
        assert!(view.irrigation_action.is_none());
        assert!(view.mine_action.is_none());
        assert!(view.terrain_work.is_empty());

        let view = sim.unit_action_view(worker).unwrap();
        assert!(view.found_city.is_none());
        assert_eq!(view.automate_label, "Automate");
        assert!(view.road_action.is_some());
        assert!(view.irrigation_action.is_some());
        assert!(view.mine_action.is_some());

        // Exploring never founds a city.
        sim.units
            .iter_mut()
            .find(|u| u.id == settler)
            .unwrap()
            .automated = true;
        sim.run_automated_unit(settler);
        assert!(sim.cities.is_empty());
    }

    #[test]
    fn government_projection_carries_rates_cap_and_lock() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let player = &sim.players[HUMAN];
        let max = player.government.max_rate();
        let view = sim.government_view(HUMAN).unwrap();
        assert_eq!(view.rates, player.rates);
        assert_eq!(view.max_rate, max);
        assert!(!view.rates_locked);

        sim.players[HUMAN].government = Government::during_revolution();
        let view = sim.government_view(HUMAN).unwrap();
        assert_eq!(view.rates, [0, 0, 100]);
        assert!(view.rates_locked);
    }

    #[test]
    fn top_bar_projection_contains_display_values_and_control_state() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let player = &sim.players[HUMAN];
        let view = sim.top_bar_view(HUMAN).unwrap();
        assert_eq!(view.year, sim.year);
        assert_eq!(view.gold, player.gold);
        assert_eq!(view.government_name, player.government.name());
        assert_eq!(view.research_name, player.researching.name());
        assert_eq!(view.research_points, player.research);
        assert_eq!(
            view.research_cost,
            sim.target_cost(HUMAN, player.researching)
        );
        assert_eq!(view.spaceship_status, sim.spaceship_status(HUMAN));
        assert!(sim.top_bar_view(usize::MAX).is_none());
    }

    #[test]
    fn leader_action_view_offers_complete_production_only_where_legal() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        game.cities.clear();
        game.players[HUMAN].known = Tech::all().collect();
        game.create_city(HUMAN, 20, 24);
        let city = game.cities[0].id;
        let leader = game.spawn_unit(HUMAN, UnitKind::Leader, 20, 24);
        game.units
            .iter_mut()
            .find(|unit| unit.id == leader)
            .unwrap()
            .scientific = true;
        game.refresh_visibility();
        let offered = |game: &Simulation| {
            game.unit_action_view(leader)
                .unwrap()
                .cities
                .iter()
                .any(|entry| entry.id == city && entry.can_complete_production)
        };

        game.cities[0].production = Production::Building(Building::PYRAMIDS);
        game.cities[0].shields = 0;
        assert!(offered(&game), "a wonder in the Leader's own city");

        game.units
            .iter_mut()
            .find(|unit| unit.id == leader)
            .unwrap()
            .scientific = false;
        assert!(
            !offered(&game),
            "a military Leader is not offered a Great Wonder"
        );
        game.cities[0].production = Production::Building(Building::TEMPLE);
        assert!(
            offered(&game),
            "a military Leader is offered an improvement"
        );
        game.units
            .iter_mut()
            .find(|unit| unit.id == leader)
            .unwrap()
            .scientific = true;
        game.cities[0].production = Production::Building(Building::PYRAMIDS);

        game.cities[0].production = Production::Building(Building::COINAGE);
        assert!(!offered(&game), "Coinage is never offered");

        game.cities[0].production = Production::Unit(UnitKind::Warrior);
        assert!(!offered(&game), "units are never offered");

        game.cities[0].production = Production::Building(Building::PYRAMIDS);
        game.tile_mut(20, 24).visible = false;
        assert!(!offered(&game), "a city hidden by fog is not offered");
        game.tile_mut(20, 24).visible = true;

        assert!(game.apply(Command::CompleteProduction {
            unit_id: leader,
            city_id: city,
        }));
        assert!(
            game.unit_action_view(leader).is_none(),
            "the Leader is used up"
        );
    }

    #[test]
    fn minimap_city_projection_resolves_owner_civilization_identity() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.create_city(HUMAN, 20, 24);
        let city = sim.cities.last().unwrap();
        let view = sim
            .minimap_city_views()
            .into_iter()
            .find(|view| view.id == city.id)
            .unwrap();
        assert_eq!((view.x, view.y, view.owner), (city.x, city.y, city.owner));
        assert_eq!(sim.player_team_palette_view(view.owner), Some(1));
    }

    #[test]
    fn city_manager_projection_resolves_forecast_and_production_controls() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.create_city(HUMAN, 20, 24);
        let city = sim.cities.last().unwrap();
        let city_id = city.id;
        let forecast = sim.city_forecast(city);
        let view = sim.city_manager_view(city_id).unwrap();
        assert_eq!(view.name, city.name);
        assert_eq!(view.size, city.size);
        assert_eq!(view.food, city.food);
        assert_eq!(view.shields, city.shields);
        assert_eq!(view.production_name, city.production.name());
        assert_eq!(view.production_cost, city.production.cost());
        assert_eq!(
            view.food_net,
            forecast.food_surplus(city.size, forecast.support.food)
        );
        assert_eq!(view.shield_net, forecast.shield_surplus());
        assert_eq!(view.science, forecast.science);
        assert_eq!(view.luxury, forecast.luxury);
        assert_eq!(view.growth_box, Simulation::growth_box(city.size));
        assert_eq!(view.growth_limited, city.size >= sim.city_size_limit(city));
        assert_eq!(view.map_tiles.len(), sim.map_city_radius(city).count());
        assert!(view.units.iter().any(|unit| unit.kind == UnitKind::Settler));
        assert!(sim.city_manager_view(u32::MAX).is_none());
    }

    #[test]
    fn city_map_labels_use_the_worked_tile_rules() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.create_city(HUMAN, 20, 24);
        sim.players[HUMAN].government = Government::DESPOTISM;
        let city_id = sim.cities.last().unwrap().id;
        let positions: Vec<_> = sim
            .map_city_radius(sim.city_by_id(city_id).unwrap())
            .filter(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
            .take(4)
            .map(|(_, _, x, y)| (x, y))
            .collect();
        for (index, &(x, y)) in positions.iter().enumerate() {
            let tile = sim.tile_mut(x, y);
            tile.seen = index != 3;
            tile.visible = tile.seen;
            tile.terrain = if index == 0 {
                Terrain::Coast
            } else {
                Terrain::Plains
            };
            tile.resource = if index == 1 {
                Some(Resource::Wheat)
            } else {
                None
            };
            tile.road = index == 2;
            tile.irrigation = index == 2;
        }
        let city = sim.city_by_id(city_id).unwrap();
        let view = sim.city_manager_view(city_id).unwrap();
        let shown_yields = |(x, y)| {
            view.map_tiles
                .iter()
                .find(|tile| (tile.x, tile.y) == (x, y))
                .unwrap()
                .yields
        };
        for &(x, y) in &positions[..3] {
            assert_eq!(
                shown_yields((x, y)),
                sim.city_tile_yields(city, x, y, false, None)
            );
        }
        let (x, y) = positions[0];
        assert_ne!(shown_yields((x, y)), sim.tile(x, y).yields());
        assert_eq!(shown_yields(positions[3]), (0, 0, 0));
    }

    #[test]
    fn movement_order_buttons_and_commands_share_actor_eligibility() {
        let mut game = Simulation::new(crate::test_support::seeded_map());
        game.units.clear();
        let start = (10, 24);
        let target = game
            .map_wrap
            .eight_way_neighbor(start.0, start.1, 1)
            .unwrap();
        for (x, y) in [start, target] {
            let tile = game.tile_mut(x, y);
            tile.terrain = Terrain::Grassland;
            tile.road = false;
        }
        let id = game.spawn_unit(HUMAN, UnitKind::Worker, start.0, start.1);
        game.refresh_visibility();

        assert!(!game.unit_action_view(id).unwrap().can_patrol);
        assert!(!game.apply(Command::Patrol(id, target.0, target.1)));
        assert!(game.unit_action_view(id).unwrap().can_connect);
        assert!(game.apply(Command::Work(id, WorkKind::Road)));
        assert!(!game.unit_action_view(id).unwrap().can_connect);
        assert!(!game.apply(Command::Connect(id, target.0, target.1)));
        assert_eq!(game.unit_by_id(id).unwrap().work_kind, Some(WorkKind::Road));

        game.units.clear();
        let guard = game.spawn_unit(HUMAN, UnitKind::Warrior, start.0, start.1);
        game.refresh_visibility();
        assert!(game.unit_action_view(guard).unwrap().can_patrol);
        assert!(game.apply(Command::Patrol(guard, target.0, target.1)));
        assert!(!game.unit_action_view(guard).unwrap().can_patrol);
    }

    #[test]
    fn found_city_button_and_command_share_movement_and_treaty_checks() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.units.clear();
        sim.tile_mut(20, 24).terrain = Terrain::Grassland;
        let id = sim.spawn_unit(HUMAN, UnitKind::Settler, 20, 24);
        let moves = sim.units[0].moves_left;
        sim.units[0].moves_left = 0;
        assert_eq!(sim.unit_action_view(id).unwrap().found_city, Some(false));
        assert!(!sim.apply(Command::FoundCity(id)));

        sim.units[0].moves_left = moves;
        sim.sign_pact(HUMAN, 1, Pact::Peace);
        sim.tile_mut(20, 24).owner = Some(1);
        assert_eq!(sim.unit_action_view(id).unwrap().found_city, Some(false));
        assert!(!sim.apply(Command::FoundCity(id)));

        sim.tile_mut(20, 24).owner = None;
        assert_eq!(sim.unit_action_view(id).unwrap().found_city, Some(true));
        assert!(sim.apply(Command::FoundCity(id)));
    }

    #[test]
    fn city_map_updates_on_discovery_and_remembers_tiles_until_seen_again() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.create_city(HUMAN, 20, 24);
        let id = sim.cities.last().unwrap().id;
        let (_, _, x, y) = sim
            .map_city_radius(sim.city_by_id(id).unwrap())
            .find(|&(dx, dy, _, _)| (dx, dy) != (0, 0))
            .unwrap();
        let tile = sim.tile_mut(x, y);
        tile.terrain = Terrain::Grassland;
        tile.resource = None;
        tile.river = false;
        tile.seen = false;
        tile.visible = false;
        let city_tile = |sim: &Simulation| {
            sim.city_manager_view(id)
                .unwrap()
                .map_tiles
                .into_iter()
                .find(|tile| (tile.x, tile.y) == (x, y))
                .unwrap()
        };
        assert!(!city_tile(&sim).seen);
        assert!(!sim.render_tile_view(x, y).seen);
        let tile = sim.tile_mut(x, y);
        tile.seen = true;
        tile.visible = true;
        sim.remember_visible_human_tiles();
        let discovered = city_tile(&sim);
        assert!(discovered.seen);
        assert!(sim.render_tile_view(x, y).visible);
        let tile = sim.tile_mut(x, y);
        tile.visible = false;
        tile.terrain = Terrain::Mountains;
        assert_eq!(sim.render_tile_view(x, y).terrain, Terrain::Grassland);
        let remembered = city_tile(&sim);
        assert!(remembered.seen);
        assert_eq!(remembered.yields, discovered.yields);
        sim.tile_mut(x, y).visible = true;
        assert_eq!(sim.render_tile_view(x, y).terrain, Terrain::Mountains);
        assert_ne!(city_tile(&sim).yields, discovered.yields);
    }

    #[test]
    fn map_painter_projection_filters_undiscovered_strategic_resources() {
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        sim.players[HUMAN].known.clear();
        let tile = sim.tile_mut(20, 20);
        tile.terrain = Terrain::Plains;
        tile.resource = Some(Resource::Iron);
        tile.road = true;
        tile.seen = true;
        tile.visible = true;

        let hidden = sim.render_tile_view(20, 20);
        assert_eq!(hidden.resource, None);
        assert!(hidden.road);
        assert_eq!(hidden.terrain, Terrain::Plains);

        sim.players[HUMAN].known.push(Tech::IRON_WORKING);
        assert_eq!(sim.render_tile_view(20, 20).resource, Some(Resource::Iron));
    }

    /// Manual timing probe: `cargo test --release -- --ignored --nocapture projection_timing`.
    #[test]
    #[ignore]
    fn projection_timing_probe() {
        use std::time::Instant;
        let mut sim = Simulation::new(crate::mapgen::generate(42));
        let w = sim.map_wrap.width();
        let h = sim.map_wrap.height();
        let players = sim.players.len().max(1);
        let mut n = 0;
        for y in (2..h - 2).step_by(3) {
            for x in (0..w).step_by(3) {
                if sim.tile(x, y).terrain.is_water() {
                    continue;
                }
                sim.create_city(n % players, x, y);
                sim.spawn_unit(n % players, UnitKind::all_slice()[n % 20], x, y);
                sim.spawn_unit(n % players, UnitKind::all_slice()[(n + 7) % 20], x + 1, y);
                n += 1;
            }
        }
        crate::sim::ui_review::ui_review_reveal_all_tiles(&mut sim);
        println!(
            "map {w}x{h}, cities {}, units {}",
            sim.cities.len(),
            sim.units.len()
        );
        let reps = 200u32;
        let time = |name: &str, f: &dyn Fn()| {
            let t = Instant::now();
            for _ in 0..reps {
                f();
            }
            println!(
                "{name:32} {:>9.1} us/call",
                t.elapsed().as_secs_f64() * 1e6 / reps as f64
            );
        };
        let id = sim.cities[0].id;
        time("city_manager_view", &|| {
            std::hint::black_box(sim.city_manager_view(id));
        });
        time("minimap_city_views", &|| {
            std::hint::black_box(sim.minimap_city_views());
        });
        time("render_cities", &|| {
            std::hint::black_box(sim.render_cities());
        });
        let tiles: std::collections::HashSet<(i32, i32)> = (0..40)
            .flat_map(|y| (0..60).map(move |x| (x % w, y % h)))
            .collect();
        let view: std::collections::HashSet<(i32, i32)> = (10..24)
            .flat_map(|y| (4..24).map(move |x| (x, y)))
            .collect();
        time("render_city_markers (sources)", &|| {
            std::hint::black_box(sim.render_city_markers());
        });
        time("rendered_city_views_in (20x14)", &|| {
            std::hint::black_box(sim.rendered_city_views_in(&view));
        });
        time("rendered_unit_views_in (20x14)", &|| {
            std::hint::black_box(sim.rendered_unit_views_in(&view));
        });
        time("rendered_unit_views_in (60x40)", &|| {
            std::hint::black_box(sim.rendered_unit_views_in(&tiles));
        });
        time("render_tile_view x(60x40)", &|| {
            for &(x, y) in &tiles {
                std::hint::black_box(sim.render_tile_view(x, y));
            }
        });
    }
}
