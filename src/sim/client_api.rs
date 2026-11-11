//! Narrow simulation access for the interactive client.
//!
//! Screen code receives read-only projections and submits gameplay commands
//! through this type. The underlying campaign model stays private to `sim`.

use super::{
    CityContextView, CityDealView, CityManagerView, Command, DiplomacyMeetingView,
    DiplomacyRivalView, GameOverView, GotoPreview, GovernmentView, MinimapCityView,
    MinimapTileView, MusicCueView, Pact, PresentationEventView, RenderTileView, RenderedCityView,
    RenderedUnitView, ResearchProgressView, SelectableUnitView, Simulation, SpaceshipAssemblyView,
    Tech, TechnologyResearchView, TopBarView, TreatyOffer, UnitActionView, UnitContextView,
    UnitFidgetView,
};
use crate::world::topology::MapWrap;
use std::collections::VecDeque;
use std::io;
use std::path::Path;

/// Client-facing simulation session. It exposes projections and commands,
/// keeping screen code independent of campaign entity storage.
pub(crate) struct ClientSimulation {
    simulation: Simulation,
}

impl ClientSimulation {
    pub(crate) fn new(mut simulation: Simulation) -> Self {
        simulation.enable_visual_events();
        Self { simulation }
    }

    pub(crate) fn load_campaign(path: &Path) -> io::Result<Self> {
        Simulation::load_campaign(path).map(Self::new)
    }

    pub(crate) fn save_campaign(&self, path: &Path) -> io::Result<()> {
        self.simulation.save_campaign(path)
    }

    /// Raw mutable access is reserved for deterministic playtest scenarios.
    pub(crate) fn playtest_simulation_mut(&mut self) -> &mut Simulation {
        &mut self.simulation
    }

    /// Read access for deterministic playtest assertions.
    pub(crate) fn playtest_simulation(&self) -> &Simulation {
        &self.simulation
    }

    pub(crate) fn research_unit_unlocks(tech: Tech) -> Vec<&'static str> {
        Simulation::research_unit_unlocks(tech)
    }

    pub(crate) fn map_wrap(&self) -> MapWrap {
        self.simulation.map_wrap
    }

    pub(crate) fn world_seed(&self) -> u64 {
        self.simulation.world_seed
    }

    pub(crate) fn apply(&mut self, command: Command) -> bool {
        self.simulation.apply(command)
    }

    pub(crate) fn take_presentation_event_views(&mut self) -> VecDeque<PresentationEventView> {
        self.simulation.take_presentation_event_views()
    }

    pub(crate) fn initial_client_focus_view(&self) -> (Option<u32>, Option<(i32, i32)>) {
        self.simulation.initial_client_focus_view()
    }

    pub(crate) fn campaign_messages_view(&self) -> Vec<String> {
        self.simulation.campaign_messages_view()
    }

    pub(crate) fn unit_action_view(&self, id: u32) -> Option<UnitActionView> {
        self.simulation.unit_action_view(id)
    }

    pub(crate) fn unit_context_view_by_id(&self, id: u32) -> Option<UnitContextView> {
        self.simulation.unit_context_view_by_id(id)
    }

    pub(crate) fn unit_context_view_for_rendered(
        &self,
        unit: &RenderedUnitView,
    ) -> Option<UnitContextView> {
        self.simulation.unit_context_view_for_rendered(unit)
    }

    pub(crate) fn city_context_view(&self, city_id: u32) -> Option<CityContextView> {
        self.simulation.city_context_view(city_id)
    }

    pub(crate) fn city_manager_view(&self, city_id: u32) -> Option<CityManagerView> {
        self.simulation.city_manager_view(city_id)
    }

    pub(crate) fn city_deals_view(&self, owner: usize) -> Vec<CityDealView> {
        self.simulation.city_deals_view(owner)
    }

    pub(crate) fn diplomacy_overview_view(&self) -> Vec<DiplomacyRivalView> {
        self.simulation.diplomacy_overview_view()
    }

    pub(crate) fn diplomacy_meeting_view(
        &self,
        owner: usize,
        draft: Option<TreatyOffer>,
    ) -> Option<DiplomacyMeetingView> {
        self.simulation.diplomacy_meeting_view(owner, draft)
    }

    pub(crate) fn treaty_offer_current_view(&self, owner: usize, offer: TreatyOffer) -> bool {
        self.simulation.treaty_offer_current_view(owner, offer)
    }

    pub(crate) fn treasury_view(&self, owner: usize) -> Option<i32> {
        self.simulation.treasury_view(owner)
    }

    pub(crate) fn player_name_view(&self, owner: usize) -> Option<String> {
        self.simulation.player_name_view(owner)
    }

    pub(crate) fn city_name_view(&self, city_id: u32) -> Option<String> {
        self.simulation.city_name_view(city_id)
    }

    pub(crate) fn goto_preview_view(&self, unit_id: u32, x: i32, y: i32) -> Option<GotoPreview> {
        self.simulation.goto_preview_view(unit_id, x, y)
    }

    pub(crate) fn displayed_tile_owner_view(&self, x: i32, y: i32) -> Option<usize> {
        self.simulation.displayed_tile_owner_view(x, y)
    }

    pub(crate) fn render_tile_view(&self, x: i32, y: i32) -> RenderTileView {
        self.simulation.render_tile_view(x, y)
    }

    pub(crate) fn minimap_tile_view(&self, x: i32, y: i32) -> MinimapTileView {
        self.simulation.minimap_tile_view(x, y)
    }

    pub(crate) fn human_city_at_view(&self, x: i32, y: i32) -> Option<u32> {
        self.simulation.human_city_at_view(x, y)
    }

    pub(crate) fn human_units_at_view(&self, x: i32, y: i32) -> Vec<SelectableUnitView> {
        self.simulation.human_units_at_view(x, y)
    }

    pub(crate) fn selectable_units_view(&self) -> Vec<SelectableUnitView> {
        self.simulation.selectable_units_view()
    }

    pub(crate) fn unit_position_view(&self, id: u32) -> Option<(i32, i32)> {
        self.simulation.unit_position_view(id)
    }

    pub(crate) fn newest_city_id_view(&self) -> Option<u32> {
        self.simulation.newest_city_id_view()
    }

    pub(crate) fn unit_fidget_view(&self, id: u32) -> Option<UnitFidgetView> {
        self.simulation.unit_fidget_view(id)
    }

    pub(crate) fn music_cue_view(&self, owner: usize) -> Option<MusicCueView> {
        self.simulation.music_cue_view(owner)
    }

    pub(crate) fn game_over_view(&self) -> Option<GameOverView> {
        self.simulation.game_over_view()
    }

    pub(crate) fn player_civilization_id_view(&self, owner: usize) -> Option<String> {
        self.simulation.player_civilization_id_view(owner)
    }

    pub(crate) fn spaceship_assembly_view(&self, owner: usize) -> SpaceshipAssemblyView {
        self.simulation.spaceship_assembly_view(owner)
    }

    pub(crate) fn government_view(&self, owner: usize) -> Option<GovernmentView> {
        self.simulation.government_view(owner)
    }

    pub(crate) fn top_bar_view(&self, owner: usize) -> Option<TopBarView> {
        self.simulation.top_bar_view(owner)
    }

    pub(crate) fn minimap_city_views(&self) -> Vec<MinimapCityView> {
        self.simulation.minimap_city_views()
    }

    pub(crate) fn minimap_city_override_view(&self, city: &RenderedCityView) -> MinimapCityView {
        self.simulation.minimap_city_override_view(city)
    }

    pub(crate) fn rendered_city_views(&self) -> Vec<RenderedCityView> {
        self.simulation.rendered_city_views()
    }

    pub(crate) fn rendered_unit_views(&self) -> Vec<RenderedUnitView> {
        self.simulation.rendered_unit_views()
    }

    pub(crate) fn research_progress_view(&self, owner: usize) -> Option<ResearchProgressView> {
        self.simulation.research_progress_view(owner)
    }

    pub(crate) fn technology_research_view(
        &self,
        owner: usize,
        tech: Tech,
    ) -> Option<TechnologyResearchView> {
        self.simulation.technology_research_view(owner, tech)
    }

    pub(crate) fn research_technology_known(&self, owner: usize, tech: Tech) -> Option<bool> {
        self.simulation.research_technology_known(owner, tech)
    }

    pub(crate) fn pact(&self, first: usize, second: usize) -> Pact {
        self.simulation.pact(first, second)
    }

    /// Raw simulation access is reserved for the UI review fixture harness.
    pub(crate) fn review_simulation(&self) -> &Simulation {
        &self.simulation
    }

    /// Mutable raw access is reserved for UI review fixture setup.
    pub(crate) fn review_simulation_mut(&mut self) -> &mut Simulation {
        &mut self.simulation
    }
}
