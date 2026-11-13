//! Read-only projections for client screens.

use super::*;
use crate::rules::TechEra;

/// The values needed to draw the selected unit's context card, without
/// exposing player, tile, transport, or movement-rule lookups to that view.
#[derive(Clone)]
pub(crate) struct UnitContextView {
    pub(crate) kind: UnitKind,
    pub(crate) owner: usize,
    pub(crate) civilization_id: Option<String>,
    pub(crate) era_index: usize,
    pub(crate) owner_name: String,
    pub(crate) terrain_name: &'static str,
    pub(crate) status: String,
    pub(crate) hp: i32,
    pub(crate) max_hp: i32,
    pub(crate) moves_left: i32,
    pub(crate) moves_allowance: i32,
    pub(crate) attack: i32,
    pub(crate) defense: i32,
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

/// Display-ready status and controls for the top bar.
pub(crate) struct TopBarView {
    pub(crate) turn: i32,
    pub(crate) year: i32,
    pub(crate) gold: i32,
    pub(crate) government_name: &'static str,
    pub(crate) anarchy: bool,
    pub(crate) rates: [i32; 3],
    pub(crate) max_rate: i32,
    pub(crate) research_name: &'static str,
    pub(crate) research_points: i32,
    pub(crate) research_cost: i32,
    pub(crate) spaceship_status: Option<String>,
    pub(crate) can_launch_spaceship: bool,
}

pub(crate) struct GovernmentView {
    pub(crate) leader_line: String,
    pub(crate) status_line: String,
    pub(crate) options: Vec<GovernmentOptionView>,
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
    pub(crate) pact_offer: Option<Pact>,
    pub(crate) next_pact: Option<Pact>,
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
    pub(crate) unplaced: [usize; 3],
    pub(crate) structure: [bool; 32],
    pub(crate) components: [usize; 2],
    pub(crate) modules: [usize; 3],
    pub(crate) manual: bool,
    pub(crate) status: String,
    pub(crate) can_launch: bool,
    pub(crate) can_toggle_manual: bool,
    pub(crate) structural_parents: [usize; 32],
    pub(crate) structural_enabled: [bool; 32],
    pub(crate) component_enabled: [bool; 2],
    pub(crate) module_enabled: [bool; 3],
}

pub(crate) struct UnitActionView {
    pub(crate) unit_name: &'static str,
    pub(crate) owner: usize,
    pub(crate) worker: bool,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) moves_left: i32,
    pub(crate) work_left: i32,
    pub(crate) upgrade: Option<(UnitKind, i32, bool)>,
    pub(crate) paradrop: bool,
    pub(crate) bombard: bool,
    pub(crate) capture_units: bool,
    pub(crate) cities: Vec<UnitCityActionView>,
    pub(crate) nuclear_weapon: bool,
    pub(crate) nuclear_strike: bool,
    pub(crate) transport_action: Option<String>,
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
    pub(crate) can_disband: bool,
    pub(crate) disband_cities: Vec<(u32, String)>,
}

pub(crate) struct UnitCityActionView {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) can_airlift: bool,
    pub(crate) can_join: bool,
    pub(crate) can_help_wonder: bool,
}

/// City identity needed by the minimap, with civilization color already resolved.
pub(crate) struct MinimapCityView {
    pub(crate) id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) owner: usize,
    pub(crate) civilization_id: Option<String>,
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
    Combat(Box<CombatPresentationView>),
}

#[derive(Clone)]
pub(crate) struct CombatPresentationView {
    pub(crate) attacker: RenderedUnitView,
    pub(crate) defender: RenderedUnitView,
    pub(crate) hp: Vec<(i32, i32)>,
    pub(crate) city: Option<RenderedCityView>,
    pub(crate) bombard_effects: bool,
    pub(crate) territory: Vec<Option<usize>>,
    pub(crate) duration: f64,
}

impl PresentationEventView {
    pub(crate) fn duration(&self) -> f64 {
        match self {
            Self::Move { duration, .. }
            | Self::Capture { duration, .. }
            | Self::FoundCity { duration, .. }
            | Self::WorkStarted { duration, .. }
            | Self::Fortify { duration, .. } => *duration,
            Self::Combat(combat) => combat.duration,
        }
    }

    pub(crate) fn actors(&self) -> Vec<&RenderedUnitView> {
        match self {
            Self::Move { unit, .. }
            | Self::Capture { unit, .. }
            | Self::FoundCity { unit, .. }
            | Self::WorkStarted { unit, .. }
            | Self::Fortify { unit, .. } => vec![unit],
            Self::Combat(combat) => vec![&combat.attacker, &combat.defender],
        }
    }
}

/// City management data with all model lookups and production rules resolved.
pub(crate) struct CityManagerView {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) owner: usize,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) civilization_id: Option<String>,
    pub(crate) era_index: usize,
    pub(crate) city_sprite: RenderedCityView,
    pub(crate) size: i32,
    pub(crate) food: i32,
    pub(crate) shields: i32,
    pub(crate) production_name: String,
    pub(crate) production_cost: i32,
    pub(crate) bought_production: bool,
    pub(crate) specialists: [i32; 3],
    pub(crate) tile_workers: i32,
    pub(crate) worklist: Vec<CityWorklistItemView>,
    pub(crate) food_net: i32,
    pub(crate) shield_net: i32,
    pub(crate) science: i32,
    pub(crate) gold_net: i32,
    pub(crate) luxury: i32,
    pub(crate) disorder: bool,
    pub(crate) growth_box: i32,
    pub(crate) growth_limited: bool,
    pub(crate) support_food: i32,
    pub(crate) support_shields: i32,
    pub(crate) support_gold: i32,
    pub(crate) rush_cost: Option<i32>,
    pub(crate) treasury: i32,
    pub(crate) buildings: Vec<BuildingProductionView>,
    pub(crate) units: Vec<UnitProductionView>,
    pub(crate) garrison: Vec<GarrisonUnitView>,
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
    pub(crate) civ3_style: u8,
    pub(crate) civ3_era: u8,
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
    pub(crate) civilization_id: Option<String>,
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

pub(crate) struct GarrisonUnitView {
    pub(crate) id: u32,
    pub(crate) kind: UnitKind,
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
        assert_eq!(view.hp, unit.hp);
        assert_eq!(view.max_hp, unit.max_hit_points());
        assert_eq!(view.moves_left, unit.moves_left);
        assert_eq!(view.attack, unit.kind.stats().attack);
        assert_eq!(view.defense, unit.kind.stats().defense);
        assert_eq!(view.terrain_name, sim.tile(unit.x, unit.y).terrain.name());
    }

    #[test]
    fn selectable_units_skip_automated_sentry_and_fortified_military_units() {
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
        assert!(selectable.contains(&worker));
        assert!(selectable.contains(&ordinary_military));
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
    fn top_bar_projection_contains_display_values_and_control_state() {
        let sim = Simulation::new(crate::mapgen::generate(42));
        let player = &sim.players[HUMAN];
        let view = sim.top_bar_view(HUMAN).unwrap();
        assert_eq!(view.turn, sim.turn);
        assert_eq!(view.year, sim.year);
        assert_eq!(view.gold, player.gold);
        assert_eq!(view.government_name, player.government.name());
        assert_eq!(view.rates, player.rates);
        assert_eq!(view.research_name, player.researching.name());
        assert_eq!(view.research_points, player.research);
        assert_eq!(
            view.research_cost,
            sim.target_cost(HUMAN, player.researching)
        );
        assert_eq!(view.spaceship_status, sim.spaceship_status(HUMAN));
        assert_eq!(view.can_launch_spaceship, sim.can_launch_spaceship(HUMAN));
        assert!(sim.top_bar_view(usize::MAX).is_none());
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
        assert_eq!(
            view.civilization_id.as_deref(),
            sim.players[HUMAN].civilization_id.as_deref()
        );
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
}
