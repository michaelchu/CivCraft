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
    pub(crate) senate_veto: bool,
    pub(crate) pact_offer: Option<Pact>,
    pub(crate) next_pact: Option<Pact>,
    pub(crate) can_negotiate: bool,
    pub(crate) treaty_offer: Option<TreatyOffer>,
    pub(crate) world_map_price: i32,
    pub(crate) sea_map_price: i32,
    pub(crate) has_city_deals: bool,
}

pub(crate) struct CityDealView {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) price: i32,
    pub(crate) affordable: bool,
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
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) moves_left: i32,
    pub(crate) work_left: i32,
    pub(crate) upgrade: Option<(UnitKind, i32, bool)>,
    pub(crate) air_superiority: bool,
    pub(crate) rebase: bool,
    pub(crate) paradrop: bool,
    pub(crate) bombard: bool,
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

impl Simulation {
    pub(crate) fn diplomacy_meeting_view(
        &self,
        owner: usize,
        draft: Option<TreatyOffer>,
    ) -> Option<DiplomacyMeetingView> {
        if owner == HUMAN {
            return None;
        }
        let leader = self.players.get(owner)?;
        let pact = self.pact(HUMAN, owner);
        let ai_offer = self.treaty_offer(owner);
        let next_pact = self.next_pact(HUMAN, owner);
        let offer = draft.or(ai_offer).unwrap_or(TreatyOffer {
            pact: next_pact,
            ..TreatyOffer::default()
        });
        let after = offer.pact.unwrap_or(pact);
        let sides = [HUMAN, owner].map(|side| {
            let other = if side == HUMAN { owner } else { HUMAN };
            TreatySideView {
                name: self.players[side].name.clone(),
                gold: self.players[side].gold,
                technologies: self
                    .treaty_trade_technologies(side, other)
                    .into_iter()
                    .map(|tech| (tech, tech.name().to_owned()))
                    .collect(),
                can_offer_embassy: self.treaty_can_offer_embassy(side, other),
                can_offer_vision: self.treaty_can_offer_vision(side, other, after),
                cities: self
                    .treaty_trade_cities(side, after)
                    .into_iter()
                    .filter_map(|id| self.city_by_id(id).map(|city| (id, city.name.clone())))
                    .collect(),
            }
        });
        Some(DiplomacyMeetingView {
            leader_line: format!("{} · {}", leader.name, leader.leader_name()),
            can_negotiate: self.can_negotiate(owner),
            ai_offer,
            pact,
            next_pact,
            offer,
            sides,
        })
    }

    pub(crate) fn treaty_offer_current_view(&self, owner: usize, offer: TreatyOffer) -> bool {
        self.treaty_offer(owner) == Some(offer)
    }

    pub(crate) fn diplomacy_overview_view(&self) -> Vec<DiplomacyRivalView> {
        (1..self.players.len())
            .filter_map(|owner| {
                let can_negotiate = self.can_negotiate(owner);
                let treaty_offer = self.treaty_offer(owner);
                if !can_negotiate && treaty_offer.is_none() {
                    return None;
                }
                let pact = self.pact(HUMAN, owner);
                let player = &self.players[owner];
                Some(DiplomacyRivalView {
                    owner,
                    name: player.name.clone(),
                    leader_name: player.leader_name().to_owned(),
                    pact,
                    pact_turns: self.pact_turns(HUMAN, owner),
                    has_embassy: self.has_embassy(HUMAN, owner),
                    senate_veto: pact != Pact::War && self.senate_blocks_cancellation(HUMAN, owner),
                    pact_offer: self.pact_offer(owner),
                    next_pact: self.next_pact(HUMAN, owner),
                    can_negotiate,
                    treaty_offer,
                    world_map_price: self.map_trade_price(owner, HUMAN, TreatyMap::World, pact),
                    sea_map_price: self.map_trade_price(owner, HUMAN, TreatyMap::Sea, pact),
                    has_city_deals: self
                        .cities
                        .iter()
                        .any(|city| city.owner == owner && self.city_trade_price(city).is_some()),
                })
            })
            .collect()
    }

    pub(crate) fn city_deals_view(&self, owner: usize) -> Vec<CityDealView> {
        self.cities
            .iter()
            .filter(|city| city.owner == owner)
            .filter_map(|city| {
                let price = self.city_trade_price(city)?;
                Some(CityDealView {
                    id: city.id,
                    name: city.name.clone(),
                    price,
                    affordable: self.players[HUMAN].gold >= price,
                })
            })
            .collect()
    }

    pub(crate) fn treasury_view(&self, owner: usize) -> Option<i32> {
        self.players.get(owner).map(|player| player.gold)
    }

    pub(crate) fn player_name_view(&self, owner: usize) -> Option<String> {
        self.players.get(owner).map(|player| player.name.clone())
    }

    pub(crate) fn city_name_view(&self, city_id: u32) -> Option<String> {
        self.city_by_id(city_id).map(|city| city.name.clone())
    }

    pub(crate) fn goto_preview_view(&self, unit_id: u32, x: i32, y: i32) -> Option<GotoPreview> {
        self.preview_goto(unit_id, x, y)
    }

    pub(crate) fn displayed_tile_owner_view(&self, x: i32, y: i32) -> Option<usize> {
        self.displayed_tile_owner(x, y)
    }

    pub(crate) fn unit_action_view(&self, id: u32) -> Option<UnitActionView> {
        let unit = self.unit_by_id(id)?;
        let owner_gold = self.players[unit.owner].gold;
        let stats = unit.kind.stats();
        let tile = self.tile(unit.x, unit.y);
        let work_left = unit.work_left;
        let cities = self
            .cities
            .iter()
            .map(|city| UnitCityActionView {
                id: city.id,
                name: city.name.clone(),
                can_airlift: self.can_airlift(unit, city),
                can_join: self.can_join_city(unit, city),
                can_help_wonder: self.can_help_wonder(unit, city)
                    && self.tile(city.x, city.y).visible,
            })
            .collect();
        let upgrade = self
            .paid_upgrade(unit)
            .map(|(target, price)| (target, price, owner_gold >= price));
        let road_action = if stats.can_work && unit.transport.is_none() {
            if tile.road && self.has_tech(unit.owner, Tech::STEAM_POWER) {
                Some((
                    "Build railroad [R]".to_owned(),
                    self.can_railroad_at(unit.owner, unit.x, unit.y),
                ))
            } else {
                Some((
                    "Build road [R]".to_owned(),
                    !tile.road
                        && tile.terrain.road_time() > 0
                        && (!tile.river || self.has_tech(unit.owner, Tech::ENGINEERING)),
                ))
            }
        } else {
            None
        };
        let irrigation_action =
            if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
                if tile.irrigation {
                    Some((
                        "Build farmland [I]".to_owned(),
                        self.can_farm_at(unit.owner, unit.x, unit.y),
                    ))
                } else {
                    Some((
                        "Irrigate [I]".to_owned(),
                        self.can_irrigate_at(unit.owner, unit.x, unit.y),
                    ))
                }
            } else {
                None
            };
        let mine_action = if let Some(kind) = self.oil_work(unit) {
            Some((format!("Build {} [M]", kind.name()), work_left == 0))
        } else if stats.can_work && unit.transport.is_none() && !unit.kind.is_naval() {
            Some((
                "Mine [M]".to_owned(),
                !tile.mine && tile.terrain.mine_time() > 0,
            ))
        } else {
            None
        };
        let terrain_work = [WorkKind::Cultivate, WorkKind::Plant]
            .into_iter()
            .filter(|&kind| self.terrain_work(unit, kind).is_some())
            .collect();
        let clean_targets = if unit.kind.can_clean() && unit.transport.is_none() {
            [
                Contamination::Pollution,
                Contamination::Fallout,
                Contamination::Craters,
            ]
            .into_iter()
            .filter(|&target| tile.has_contamination(target))
            .map(|target| (target, target.name().to_owned()))
            .collect()
        } else {
            Vec::new()
        };
        let base_work = [Base::Fort, Base::Fortress, Base::Airstrip, Base::Airbase]
            .into_iter()
            .filter(|&base| self.base_work_time(unit, base).is_some())
            .map(|base| (base, base.name().to_owned()))
            .collect();
        let base_target = tile.base.map(PillageTarget::Base);
        let pillage_targets = PillageTarget::ALL
            .into_iter()
            .chain(base_target)
            .filter(|&target| self.can_pillage(unit, target))
            .map(|target| (target, target.name().to_owned()))
            .collect();
        let disband_cities = self
            .cities
            .iter()
            .filter(|city| self.can_disband(unit, Some(city)))
            .map(|city| (city.id, city.name.clone()))
            .collect();
        Some(UnitActionView {
            unit_name: unit.kind.name(),
            owner: unit.owner,
            x: unit.x,
            y: unit.y,
            moves_left: unit.moves_left,
            work_left,
            upgrade,
            air_superiority: self.can_assign_air_superiority(unit),
            rebase: self.can_begin_rebase(unit),
            paradrop: self.can_begin_paradrop(unit),
            bombard: self.can_begin_bombard(unit),
            cities,
            nuclear_weapon: unit.kind.is_nuclear_weapon(),
            nuclear_strike: self.can_begin_nuclear_strike(unit),
            transport_action: self.transport_action(unit).map(str::to_owned),
            found_city: (stats.can_found && unit.transport.is_none())
                .then(|| self.can_found_at(unit.x, unit.y)),
            road_action,
            road_work: if tile.road {
                WorkKind::Railroad
            } else {
                WorkKind::Road
            },
            irrigation_action,
            irrigation_work: if tile.irrigation {
                WorkKind::Farmland
            } else {
                WorkKind::Irrigation
            },
            mine_action,
            mine_work: self.oil_work(unit).unwrap_or(WorkKind::Mine),
            terrain_work,
            clean_targets,
            base_work,
            pillage_targets,
            fortify: unit.kind.is_military() && unit.transport.is_none(),
            set_home_city: self.home_city_target(unit).is_some(),
            automate: unit.automated
                || (unit.kind.fuel_capacity() == 0 && unit.kind != UnitKind::Helicopter),
            can_disband: self.can_disband(unit, None),
            disband_cities,
        })
    }

    pub(crate) fn campaign_messages_view(&self) -> Vec<String> {
        self.events.clone()
    }

    pub(crate) fn minimap_tile_view(&self, x: i32, y: i32) -> MinimapTileView {
        let tile = self.render_tile(x, y);
        MinimapTileView {
            terrain: tile.terrain,
            seen: tile.seen,
            visible: tile.visible,
        }
    }

    pub(crate) fn render_tile_view(&self, x: i32, y: i32) -> RenderTileView {
        let tile = self.render_tile(x, y);
        RenderTileView {
            terrain: tile.terrain,
            base_terrain: tile.base_terrain,
            resource: tile
                .active_resource()
                .filter(|&resource| self.resource_visible_to(HUMAN, resource)),
            hut: tile.hut,
            river_edges: tile.river_edges,
            road: tile.road,
            railroad: tile.railroad,
            irrigation: tile.irrigation,
            farmland: tile.farmland,
            pollution: tile.pollution,
            fallout: tile.fallout,
            craters: tile.craters,
            base: tile.base,
            mine: tile.mine,
            oil_well: tile.oil_well,
            oil_platform: tile.oil_platform,
            seen: tile.seen,
            visible: tile.visible,
        }
    }

    pub(crate) fn human_city_at_view(&self, x: i32, y: i32) -> Option<u32> {
        self.cities
            .iter()
            .find(|city| city.owner == HUMAN && (city.x, city.y) == (x, y))
            .map(|city| city.id)
    }

    pub(crate) fn human_units_at_view(&self, x: i32, y: i32) -> Vec<SelectableUnitView> {
        let mut units = self
            .units
            .iter()
            .rev()
            .filter(|unit| unit.owner == HUMAN && (unit.x, unit.y) == (x, y))
            .map(|unit| SelectableUnitView {
                id: unit.id,
                transported: unit.transport.is_some(),
            })
            .collect::<Vec<_>>();
        units.sort_by_key(|unit| unit.transported);
        units
    }

    pub(crate) fn selectable_units_view(&self) -> Vec<SelectableUnitView> {
        self.units
            .iter()
            .filter(|unit| {
                unit.owner == HUMAN
                    && unit.moves_left > 0
                    && unit.work_left == 0
                    && unit.state != UnitState::Sentry
            })
            .map(|unit| SelectableUnitView {
                id: unit.id,
                transported: unit.transport.is_some(),
            })
            .collect()
    }

    pub(crate) fn unit_position_view(&self, id: u32) -> Option<(i32, i32)> {
        self.unit_by_id(id).map(|unit| (unit.x, unit.y))
    }

    pub(crate) fn newest_city_id_view(&self) -> Option<u32> {
        self.cities.last().map(|city| city.id)
    }

    pub(crate) fn unit_fidget_view(&self, id: u32) -> Option<UnitFidgetView> {
        let unit = self.unit_by_id(id)?;
        Some(UnitFidgetView {
            id: unit.id,
            kind: unit.kind,
            era_index: self.research_era(unit.owner).index(),
            state: unit.state,
            working: unit.work_kind.is_some(),
        })
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

    pub(crate) fn player_civilization_id_view(&self, owner: usize) -> Option<String> {
        self.players.get(owner)?.civilization_id.clone()
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
        let (unplaced, structure, components, modules, manual) = self.ship_assembly();
        let status = self.spaceship_status(owner).unwrap_or_default();
        let structural_enabled =
            std::array::from_fn(|index| self.can_place_ship_part(ShipPart::Structural(index)));
        let component_enabled =
            std::array::from_fn(|index| self.can_place_ship_part(ShipPart::Component(index)));
        let module_enabled =
            std::array::from_fn(|index| self.can_place_ship_part(ShipPart::Module(index)));
        SpaceshipAssemblyView {
            unplaced,
            structure,
            components,
            modules,
            manual,
            can_launch: self.can_launch_spaceship(owner),
            can_toggle_manual: !status.starts_with("Spaceship arrives"),
            status,
            structural_parents: STRUCTURE_PARENT,
            structural_enabled,
            component_enabled,
            module_enabled,
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
        let options = Government::TARGETS
            .into_iter()
            .map(|government| {
                let support_description = government.unit_support().map_or_else(
                    || "Unit support: unresolved legacy government".to_owned(),
                    |policy| {
                        format!(
                            "Free units: {}/{}/{} · {} gold per excess unit",
                            policy.town, policy.city, policy.metropolis, policy.cost_per_unit
                        )
                    },
                );
                let requirement_description = government.requirement().and_then(|tech| {
                    (!player.known.contains(&tech)).then(|| {
                        if self.can_change_government(owner, government) {
                            "Statue of Liberty".to_owned()
                        } else {
                            format!("Requires {}", tech.name())
                        }
                    })
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
        Some(GovernmentView {
            leader_line: format!("{} · {}", player.name, player.leader_name()),
            status_line,
            options,
        })
    }

    pub(crate) fn top_bar_view(&self, owner: usize) -> Option<TopBarView> {
        let player = self.players.get(owner)?;
        let research = player.researching;
        Some(TopBarView {
            turn: self.turn,
            year: self.year,
            gold: player.gold,
            government_name: player.government.name(),
            anarchy: player.government == Government::Anarchy,
            rates: player.rates,
            max_rate: player.government.max_rate(),
            research_name: research.name(),
            research_points: player.research,
            research_cost: self.target_cost(owner, research),
            spaceship_status: self.spaceship_status(owner),
            can_launch_spaceship: self.can_launch_spaceship(owner),
        })
    }

    pub(crate) fn minimap_city_views(&self) -> Vec<MinimapCityView> {
        self.render_city_markers()
            .into_iter()
            .map(|city| self.minimap_city_view(city.id, city.x, city.y, city.owner))
            .collect()
    }

    pub(crate) fn minimap_city_override_view(&self, city: &RenderedCityView) -> MinimapCityView {
        MinimapCityView {
            id: city.id,
            x: city.x,
            y: city.y,
            owner: city.owner,
            civilization_id: city.civilization_id.clone(),
        }
    }

    fn minimap_city_view(&self, id: u32, x: i32, y: i32, owner: usize) -> MinimapCityView {
        MinimapCityView {
            id,
            x,
            y,
            owner,
            civilization_id: self.players[owner].civilization_id.clone(),
        }
    }

    pub(crate) fn city_manager_view(&self, city_id: u32) -> Option<CityManagerView> {
        let city = self.city_by_id(city_id)?;
        let observed = self.observed_city(city);
        let city_sprite = self.rendered_city_view_from_observed(&observed);
        let forecast = self.city_forecast(city);
        let support = &forecast.support;
        let units = UnitKind::ALL_NATIVE
            .into_iter()
            .map(|kind| {
                let production = Production::Unit(kind);
                UnitProductionView {
                    kind,
                    cost: kind.stats().cost,
                    available: self.can_produce(city, production),
                    current: city.production == production,
                    can_change: self.can_change_production(city, production),
                    can_queue: city.worklist.len() < WORKLIST_LIMIT
                        && self.can_produce(city, production),
                }
            })
            .collect();
        let buildings = Building::all()
            .map(|building| {
                let data = building.data();
                let production = Production::Building(building);
                let blocker = self.building_blocker(city, building);
                let cost_label = if data.genus == crate::rules::Genus::Convert {
                    "Converts shields to gold".to_owned()
                } else {
                    format!("{} shields  /  ref. upkeep {}", data.cost, data.upkeep)
                };
                BuildingProductionView {
                    building,
                    name: data.name,
                    cost_label,
                    built: city.buildings.contains(&building),
                    current: city.production == production,
                    can_change: blocker.is_none() && self.can_change_production(city, production),
                    can_queue: city.worklist.len() < WORKLIST_LIMIT
                        && self.can_produce(city, production),
                    sale_price: Self::building_sale_price(city, building),
                    blocker,
                }
            })
            .collect();
        let worklist = city
            .worklist
            .iter()
            .copied()
            .map(|production| {
                let target = self.worklist_replacement(city, production);
                CityWorklistItemView {
                    production,
                    name: production.name().to_owned(),
                    target,
                    target_name: target.name().to_owned(),
                    available: self.can_produce(city, target),
                }
            })
            .collect();
        let city_map_tiles = self
            .map_city_radius(city)
            .into_iter()
            .map(|(dx, dy, x, y)| {
                let tile = self.tile(x, y);
                let is_city = (dx, dy) == (0, 0);
                let worked = city.worked_tiles.contains(&(x, y));
                CityMapTileView {
                    dx,
                    dy,
                    x,
                    y,
                    is_city,
                    seen: tile.seen,
                    worked,
                    can_assign: !is_city
                        && (worked
                            || (city.specialists.iter().any(|&n| n > 0)
                                && self.can_assign_city_tile(city, x, y))),
                    yields: tile.yields(),
                }
            })
            .collect();
        let garrison = self
            .units
            .iter()
            .filter(|unit| unit.owner == HUMAN && unit.x == city.x && unit.y == city.y)
            .map(|unit| GarrisonUnitView {
                id: unit.id,
                kind: unit.kind,
            })
            .collect();
        Some(CityManagerView {
            id: city.id,
            name: city.name.clone(),
            owner: city.owner,
            x: city.x,
            y: city.y,
            civilization_id: self.players[city.owner].civilization_id.clone(),
            era_index: self.research_era(city.owner).index(),
            city_sprite,
            size: city.size,
            food: city.food,
            shields: city.shields,
            production_name: city.production.name().to_owned(),
            production_cost: city.production.cost(),
            bought_production: city.bought_production,
            specialists: city.specialists,
            tile_workers: city.tile_workers(),
            worklist,
            food_net: forecast.food_surplus(city.size, support.food),
            shield_net: forecast.shield_surplus(),
            science: forecast.science,
            gold_net: forecast.gold_surplus(support.gold),
            luxury: forecast.luxury,
            disorder: forecast.disorder,
            growth_box: Self::growth_box(city.size),
            growth_limited: city.size >= Self::city_size_limit(city),
            support_food: support.food,
            support_shields: support.shields,
            support_gold: support.gold,
            rush_cost: self.rush_cost(city),
            treasury: self.players[city.owner].gold,
            buildings,
            units,
            garrison,
            map_tiles: city_map_tiles,
        })
    }

    pub(crate) fn rendered_city_views(&self) -> Vec<RenderedCityView> {
        self.render_cities()
            .iter()
            .map(|observed| self.rendered_city_view_from_observed(observed))
            .collect()
    }

    pub(super) fn rendered_city_override_view(&self, city: &City) -> RenderedCityView {
        let observed = self.observed_city(city);
        self.rendered_city_view_from_observed(&observed)
    }

    pub(crate) fn rendered_unit_views(&self) -> Vec<RenderedUnitView> {
        self.units
            .iter()
            .map(|unit| self.rendered_unit_view(unit))
            .filter(|unit| unit.visible)
            .collect()
    }

    pub(super) fn rendered_unit_override_view(&self, unit: &Unit) -> RenderedUnitView {
        let mut view = self.rendered_unit_view(unit);
        view.context = Some(self.unit_context_view(unit));
        view
    }

    fn rendered_unit_view(&self, unit: &Unit) -> RenderedUnitView {
        let stats = unit.kind.stats();
        RenderedUnitView {
            id: unit.id,
            kind: unit.kind,
            owner: unit.owner,
            civilization_id: self.players[unit.owner].civilization_id.clone(),
            era_index: self.research_era(unit.owner).index(),
            x: unit.x,
            y: unit.y,
            facing: unit.facing,
            transport: unit.transport,
            is_transporter: unit.kind.transport_capacity() > 0,
            work_kind: unit.work_kind,
            state: unit.state,
            moves_left: unit.moves_left,
            movement_rate: self.unit_move_rate(unit),
            hp: unit.hp,
            max_hp: unit.max_hit_points(),
            attack: stats.attack,
            defense: stats.defense,
            visible: self.unit_visible_to_human(unit),
            context: None,
        }
    }

    fn rendered_city_view_from_observed(&self, observed: &ObservedCity) -> RenderedCityView {
        let city = &observed.city;
        let (growth_label, production_label) = if city.owner == HUMAN {
            let forecast = self.city_forecast(city);
            let capacity = Self::growth_box(city.size);
            let surplus = forecast.food_surplus(city.size, forecast.support.food);
            let can_grow = city.size < Self::city_size_limit(city);
            let growth = if city.food + surplus >= capacity && !can_grow {
                "blocked".to_owned()
            } else if city.food + surplus >= capacity {
                "1".to_owned()
            } else if surplus > 0 {
                ((capacity - city.food + surplus - 1) / surplus).to_string()
            } else if surplus < 0 {
                format!("famine {}", 1 - city.food / surplus)
            } else {
                "-".to_owned()
            };
            let production = match city.production {
                Production::Building(building)
                    if building.data().genus == crate::rules::Genus::Convert =>
                {
                    city.production.name().to_owned()
                }
                _ => {
                    let available = match city.production {
                        Production::Building(building)
                            if building.data().genus == crate::rules::Genus::GreatWonder =>
                        {
                            self.great_wonder_available(building)
                        }
                        _ => true,
                    };
                    let turns = if available {
                        let cost = city.production.cost();
                        let surplus = forecast.shield_surplus();
                        if city.shields >= cost {
                            Some(1)
                        } else if surplus > 0 {
                            Some((cost - city.shields - 1) / surplus + 1)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                    .filter(|turns| *turns <= 999);
                    format!(
                        "{} : {}",
                        city.production.name(),
                        turns.map_or_else(|| "-".to_owned(), |turns| turns.to_string())
                    )
                }
            };
            (Some(growth), Some(production))
        } else {
            (None, None)
        };
        RenderedCityView {
            id: city.id,
            name: city.name.clone(),
            owner: city.owner,
            x: city.x,
            y: city.y,
            size: city.size,
            civilization_id: self.players[city.owner].civilization_id.clone(),
            civ3_style: observed.civ3_style,
            civ3_era: observed.civ3_era,
            occupied: observed.occupied,
            disorder: observed.disorder,
            capital: city.buildings.contains(&Building::PALACE)
                || city.buildings.contains(&Building::ECCLESIASTICAL_PALACE),
            growth_label,
            production_label,
        }
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

    pub(crate) fn city_context_view(&self, city_id: u32) -> Option<CityContextView> {
        let city = self.city_by_id(city_id)?;
        let forecast = self.city_forecast(city);
        Some(CityContextView {
            name: city.name.clone(),
            size: city.size,
            foreign_citizens: city.size - city.domestic_citizens(),
            food: forecast.food,
            shields: forecast.shields,
            trade: forecast.trade,
            illness: self.city_illness(city, forecast.shields),
            food_store: city.food,
            growth_box: Self::growth_box(city.size),
            production_name: city.production.name(),
            production_stock: city.shields,
            production_cost: city.production.cost(),
        })
    }

    /// Projects a unit and its adjacent campaign state for the context card.
    /// `unit` may be the presentation playback snapshot for an active animation.
    pub(crate) fn unit_context_view(&self, unit: &Unit) -> UnitContextView {
        let status = if unit.work_left > 0 {
            format!(
                "Building {} - {} turns",
                unit.work_kind
                    .map(|work| work.name())
                    .unwrap_or("improvement"),
                self.project_turns_left(unit)
            )
        } else if let Some(carrier) = unit.transport {
            self.units
                .iter()
                .find(|ship| ship.id == carrier)
                .map_or_else(
                    || "Aboard transport".to_owned(),
                    |ship| format!("Aboard {}", ship.kind.name()),
                )
        } else if unit.kind.transport_capacity() > 0 {
            format!(
                "Cargo {}/{}",
                self.cargo_count(unit.id),
                unit.kind.transport_capacity()
            )
        } else if unit.automated {
            "Automated".to_owned()
        } else {
            match unit.state {
                UnitState::Idle => "Ready".to_owned(),
                UnitState::Fortifying => "Fortifying".to_owned(),
                UnitState::Fortified => "Fortified (+50% defense)".to_owned(),
                UnitState::Sentry => "Sentry".to_owned(),
                UnitState::AirSuperiority => "Air Superiority".to_owned(),
            }
        };
        let status = if unit.kind.is_aircraft() {
            format!(
                "{status} · Fuel {}/{}",
                unit.fuel,
                unit.kind.fuel_capacity()
            )
        } else {
            status
        };
        let status = if unit.veteran > 0 {
            format!("{status} · {}", unit.rank_name())
        } else {
            status
        };
        let player = &self.players[unit.owner];
        let stats = unit.kind.stats();
        UnitContextView {
            kind: unit.kind,
            owner: unit.owner,
            civilization_id: player.civilization_id.clone(),
            era_index: self.research_era(unit.owner).index(),
            owner_name: player.name.clone(),
            terrain_name: self.tile(unit.x, unit.y).terrain.name(),
            status,
            hp: unit.hp,
            max_hp: unit.max_hit_points(),
            moves_left: unit.moves_left,
            moves_allowance: self.unit_move_allowance(unit.owner, unit.kind),
            attack: stats.attack,
            defense: stats.defense,
        }
    }

    pub(crate) fn unit_context_view_for_rendered(
        &self,
        unit: &RenderedUnitView,
    ) -> Option<UnitContextView> {
        unit.context.clone()
    }

    pub(crate) fn unit_context_view_by_id(&self, id: u32) -> Option<UnitContextView> {
        self.unit_by_id(id).map(|unit| self.unit_context_view(unit))
    }
}

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
        assert_eq!(
            view.growth_limited,
            city.size >= Simulation::city_size_limit(city)
        );
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
