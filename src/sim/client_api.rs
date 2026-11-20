//! Narrow simulation access for the interactive client.
//!
//! Screen code receives read-only projections and submits gameplay commands
//! through this type. The underlying campaign model stays private to `sim`.

use super::ui_review::{ReviewFixture, ReviewState};
use super::{
    CityContextView, CityManagerView, Command, DiplomacyMeetingView, DiplomacyRivalView,
    EspionageRivalView, GameOverView, GotoPreview, GovernmentView, MinimapCityView,
    MinimapTileView, MusicCueView, Pact, PresentationEventView, RenderTileView, RenderedCityView,
    RenderedUnitView, ResearchProgressView, SelectableUnitView, Simulation, SpaceshipAssemblyView,
    Tech, TechnologyResearchView, TopBarView, TreatyOffer, UnitActionView, UnitContextView,
    UnitFidgetView,
};
use crate::world::topology::MapWrap;
use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::{cell::RefCell, rc::Rc};

/// A bounded cache of the last requested projection, including missing results.
struct CachedProjection<K, V> {
    key: K,
    view: Option<Rc<V>>,
}

type ProjectionCache<K, V> = RefCell<Option<CachedProjection<K, V>>>;

fn cached_projection<K: Eq, V>(
    cache: &ProjectionCache<K, V>,
    key: K,
    build: impl FnOnce() -> Option<V>,
) -> Option<Rc<V>> {
    let mut entry = cache.borrow_mut();
    if let Some(cached) = entry.as_ref()
        && cached.key == key
    {
        return cached.view.clone();
    }
    let view = build().map(Rc::new);
    *entry = Some(CachedProjection {
        key,
        view: view.clone(),
    });
    view
}

/// Completed worker-owned campaign. Presentation may be prepared before commit.
pub(crate) struct CompletedTurn {
    simulation: Simulation,
    events: VecDeque<PresentationEventView>,
    autosave: Option<Result<Vec<u8>, String>>,
}
impl CompletedTurn {
    pub(crate) fn take_events(&mut self) -> VecDeque<PresentationEventView> {
        std::mem::take(&mut self.events)
    }
    pub(crate) fn commit(self, client: &mut ClientSimulation) -> Option<Result<Vec<u8>, String>> {
        client.simulation = self.simulation;
        client.invalidate_views();
        self.autosave
    }
}

/// Client-facing simulation session. It exposes projections and commands,
/// keeping screen code independent of campaign entity storage.
pub(crate) struct ClientSimulation {
    simulation: Simulation,
    view_revision: u64,
    unit_actions: std::cell::RefCell<Option<(u32, UnitActionView)>>,
    city_manager: ProjectionCache<u32, CityManagerView>,
    goto_preview: ProjectionCache<(u32, i32, i32), GotoPreview>,
}

impl ClientSimulation {
    pub(crate) fn religion_management_view(&self) -> super::religion::ReligionManagementView {
        self.simulation.religion_management_view()
    }
    pub(crate) fn city_religion_summary(&self, city: u32) -> String {
        self.simulation.city_religion_summary(city)
    }
    pub(crate) fn public_faith_summary(&self, owner: usize) -> String {
        self.simulation.public_faith_summary(owner)
    }

    pub(crate) fn new(mut simulation: Simulation) -> Self {
        simulation.enable_visual_events();
        Self {
            simulation,
            view_revision: 0,
            unit_actions: Default::default(),
            city_manager: Default::default(),
            goto_preview: Default::default(),
        }
    }

    pub(crate) fn load_campaign(path: &Path) -> io::Result<Self> {
        Simulation::load_campaign(path).map(Self::new)
    }

    pub(crate) fn save_campaign(&self, path: &Path) -> io::Result<()> {
        self.simulation.save_campaign(path)
    }

    pub(crate) fn religion_research_unlocks(tech: Tech) -> Vec<&'static str> {
        Simulation::religion_research_unlocks(tech)
    }

    pub(crate) fn research_unit_unlocks(tech: Tech) -> Vec<&'static str> {
        Simulation::research_unit_unlocks(tech)
    }

    pub(crate) fn map_wrap(&self) -> MapWrap {
        self.simulation.map_wrap
    }

    pub(crate) fn turn_view(&self) -> i32 {
        self.simulation.turn()
    }

    pub(crate) fn world_seed(&self) -> u64 {
        self.simulation.world_seed
    }

    fn invalidate_views(&mut self) {
        self.view_revision = self.view_revision.wrapping_add(1);
        *self.unit_actions.get_mut() = None;
        *self.city_manager.get_mut() = None;
        *self.goto_preview.get_mut() = None;
    }

    pub(crate) fn view_revision(&self) -> u64 {
        self.view_revision
    }

    pub(crate) fn start_background_turn(
        &mut self,
        autosave: bool,
    ) -> Result<crate::background::Task<CompletedTurn>, String> {
        // The UI copy is frozen for the job's lifetime. Only the worker owns and
        // mutates the authoritative simulation; no graphics objects cross threads.
        let mut frozen = self.simulation.clone();
        frozen.take_visual_events();
        let mut simulation = std::mem::replace(&mut self.simulation, frozen);
        crate::background::Task::spawn("civcraft-turn", move || {
            if !simulation.apply(Command::EndTurn) {
                return Err("The turn could not be advanced.".to_owned());
            }
            let autosave = autosave.then(|| {
                simulation
                    .encode_campaign()
                    .map_err(|error| error.to_string())
            });
            let events = simulation.take_presentation_event_views();
            Ok(CompletedTurn {
                simulation,
                events,
                autosave,
            })
        })
    }

    pub(crate) fn apply(&mut self, command: Command) -> bool {
        // Failed commands can also emit feedback or partially update client state.
        self.invalidate_views();
        self.simulation.apply(command)
    }

    pub(crate) fn is_human_unit(&self, id: u32) -> bool {
        self.simulation
            .unit_by_id(id)
            .is_some_and(|unit| unit.owner == super::HUMAN)
    }

    pub(crate) fn unit_kind_view(&self, id: u32) -> Option<super::UnitKind> {
        self.simulation.unit_by_id(id).map(|unit| unit.kind)
    }

    pub(crate) fn unit_moves_left_view(&self, id: u32) -> Option<i32> {
        self.simulation.unit_by_id(id).map(|unit| unit.moves_left)
    }

    pub(crate) fn city_exists(&self, id: u32) -> bool {
        self.simulation.city_by_id(id).is_some()
    }

    pub(crate) fn take_presentation_event_views(&mut self) -> VecDeque<PresentationEventView> {
        self.simulation.take_presentation_event_views()
    }

    pub(crate) fn initial_client_focus_view(&self) -> (Option<u32>, Option<(i32, i32)>) {
        self.simulation.initial_client_focus_view()
    }

    pub(crate) fn campaign_notifications_view(&self) -> &[super::Notification] {
        self.simulation.campaign_notifications_view()
    }

    pub(crate) fn unread_notifications_view(&self) -> usize {
        self.simulation.unread_notifications_view()
    }

    pub(crate) fn campaign_messages_view(&self) -> Vec<String> {
        self.simulation.campaign_messages_view()
    }

    pub(crate) fn mark_notifications_read(&mut self) {
        self.simulation.mark_notifications_read();
    }

    pub(crate) fn notify(&mut self, text: impl Into<String>) {
        self.simulation.add_event(text);
    }

    pub(crate) fn notify_important(&mut self, text: impl Into<String>) {
        self.simulation.add_important_event(text);
    }

    pub(crate) fn unit_action_view(&self, id: u32) -> Option<UnitActionView> {
        let mut cached = self.unit_actions.borrow_mut();
        if let Some((cached_id, view)) = cached.as_ref()
            && *cached_id == id
        {
            return Some(view.clone());
        }
        let view = self.simulation.unit_action_view(id)?;
        *cached = Some((id, view.clone()));
        Some(view)
    }

    pub(crate) fn bombardable_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        self.simulation.bombardable_targets_in_range(id)
    }

    pub(crate) fn precision_bombardable_targets_in_range(&self, id: u32) -> Vec<(i32, i32)> {
        self.simulation.precision_bombardable_targets_in_range(id)
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

    pub(crate) fn city_governor_preview(&self, city_id: u32) -> Option<[i32; 6]> {
        self.simulation.city_governor_preview(city_id)
    }

    pub(crate) fn city_manager_view(&self, city_id: u32) -> Option<Rc<CityManagerView>> {
        cached_projection(&self.city_manager, city_id, || {
            self.simulation.city_manager_view(city_id)
        })
    }

    pub(crate) fn diplomacy_overview_view(&self) -> Vec<DiplomacyRivalView> {
        self.simulation.diplomacy_overview_view()
    }

    pub(crate) fn espionage_rivals_view(&self) -> Vec<EspionageRivalView> {
        self.simulation.espionage_rivals_view()
    }

    pub(crate) fn diplomacy_meeting_view(
        &self,
        owner: usize,
        draft: Option<TreatyOffer>,
    ) -> Option<DiplomacyMeetingView> {
        self.simulation.diplomacy_meeting_view(owner, draft)
    }

    pub(crate) fn vassal_offer_label(&self, terms: super::VassalTerms) -> String {
        self.simulation.vassal_offer_label(terms)
    }

    pub(crate) fn vassal_war_preview(
        &self,
        a: usize,
        b: usize,
        terms: super::VassalTerms,
    ) -> String {
        self.simulation.vassal_war_preview(a, b, terms)
    }

    pub(crate) fn treaty_offer_current_view(&self, owner: usize, offer: &TreatyOffer) -> bool {
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

    pub(crate) fn goto_preview_view(
        &self,
        unit_id: u32,
        x: i32,
        y: i32,
    ) -> Option<Rc<GotoPreview>> {
        cached_projection(&self.goto_preview, (unit_id, x, y), || {
            self.simulation.goto_preview_view(unit_id, x, y)
        })
    }

    pub(crate) fn territory_border_owner_view(&self, x: i32, y: i32) -> Option<usize> {
        self.simulation.territory_border_owner_view(x, y)
    }

    pub(crate) fn territory_geometry_owner_view(&self, x: i32, y: i32) -> Option<usize> {
        self.simulation.territory_geometry_owner_view(x, y)
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

    pub(crate) fn selectable_unit_ids_view(&self) -> impl Iterator<Item = u32> + '_ {
        self.simulation.selectable_unit_ids_view()
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

    pub(crate) fn rendered_city_views_in(
        &self,
        tiles: &std::collections::HashSet<(i32, i32)>,
    ) -> Vec<RenderedCityView> {
        self.simulation.rendered_city_views_in(tiles)
    }

    pub(crate) fn rendered_unit_views_in(
        &self,
        tiles: &std::collections::HashSet<(i32, i32)>,
    ) -> Vec<RenderedUnitView> {
        self.simulation.rendered_unit_views_in(tiles)
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

    pub(crate) fn fork_for_review(&self) -> Self {
        Self::new(self.simulation.clone())
    }

    pub(crate) fn review_fixture(&mut self) -> ReviewFixture<'_> {
        self.invalidate_views();
        ReviewFixture(&mut self.simulation)
    }

    pub(crate) fn review_state(&self) -> ReviewState<'_> {
        ReviewState(&self.simulation)
    }

    #[cfg(test)]
    pub(crate) fn review_simulation(&self) -> &Simulation {
        &self.simulation
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn background_turn_preserves_frozen_state_events_rng_and_autosave() {
        let mut client = ClientSimulation::new(Simulation::new(crate::test_support::seeded_map()));
        let before = client.simulation.encode_campaign().unwrap();
        let mut expected = client.simulation.clone();
        let foreground_started = std::time::Instant::now();
        assert!(expected.apply(Command::EndTurn));
        let expected_events: Vec<_> = expected
            .take_presentation_event_views()
            .iter()
            .flat_map(|event| {
                event
                    .actors()
                    .map(|unit| (unit.id, unit.x, unit.y))
                    .collect::<Vec<_>>()
            })
            .collect();
        let expected_bytes = expected.encode_campaign().unwrap();
        let foreground_ms = foreground_started.elapsed().as_secs_f64() * 1000.0;
        let started = std::time::Instant::now();
        let job = client.start_background_turn(true).unwrap();
        let handoff_ms = started.elapsed().as_secs_f64() * 1000.0;
        let mut outcome = job.wait().unwrap();
        println!(
            "Turn pipeline {}x{}: foreground={foreground_ms:.3} ms, UI handoff={handoff_ms:.3} ms, background total={:.3} ms",
            client.map_wrap().width(),
            client.map_wrap().height(),
            started.elapsed().as_secs_f64() * 1000.0
        );
        assert!(
            client.simulation.encode_campaign().unwrap() == before,
            "background turn changed campaign bytes"
        );
        let actual_events: Vec<_> = outcome
            .take_events()
            .iter()
            .flat_map(|event| {
                event
                    .actors()
                    .map(|unit| (unit.id, unit.x, unit.y))
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(actual_events, expected_events);
        let payload = outcome.commit(&mut client).unwrap().unwrap();
        assert!(
            payload == expected_bytes,
            "background turn changed campaign bytes"
        );
        assert!(
            client.simulation.encode_campaign().unwrap() == expected_bytes,
            "background turn changed campaign bytes"
        );
        let path = std::env::temp_dir().join(format!(
            "civcraft-background-turn-{}.save",
            std::process::id()
        ));
        Simulation::write_campaign_payload(&path, &payload).unwrap();
        let loaded = Simulation::load_campaign(&path).unwrap();
        assert!(std::fs::read(&path).unwrap().ends_with(&payload));
        assert_eq!(loaded.turn, expected.turn);
        assert_eq!(loaded.rng, expected.rng);
        assert!(
            bincode::serialize(&loaded.units).unwrap()
                == bincode::serialize(&expected.units).unwrap()
        );
        assert!(
            bincode::serialize(&loaded.cities).unwrap()
                == bincode::serialize(&expected.cities).unwrap()
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn background_turn_without_autosave_returns_no_save_payload() {
        let mut client = ClientSimulation::new(Simulation::new(crate::test_support::seeded_map()));
        let outcome = client.start_background_turn(false).unwrap().wait().unwrap();
        assert!(outcome.commit(&mut client).is_none());
    }

    #[test]
    fn city_manager_cache_reuses_views_and_refreshes_treasury_and_removed_cities() {
        let mut simulation = Simulation::new(crate::mapgen::generate(42));
        simulation.create_city(super::super::HUMAN, 20, 24);
        let id = simulation.cities.last().unwrap().id;
        let mut client = ClientSimulation::new(simulation);
        let original = client.city_manager_view(id).unwrap();
        assert!(Rc::ptr_eq(
            &original,
            &client.city_manager_view(id).unwrap()
        ));
        let old_gold = original.treasury;
        client.review_fixture().0.players[super::super::HUMAN].gold += 123;
        let changed = client.city_manager_view(id).unwrap();
        assert!(!Rc::ptr_eq(&original, &changed));
        assert_eq!(changed.treasury, old_gold + 123);
        client
            .review_fixture()
            .0
            .cities
            .retain(|city| city.id != id);
        assert!(client.city_manager_view(id).is_none());
    }

    #[test]
    fn goto_cache_tracks_actor_destination_and_movement_changes() {
        let mut simulation = Simulation::new(crate::mapgen::generate(42));
        simulation.units.clear();
        for tile in &mut simulation.tiles {
            tile.terrain = super::super::Terrain::Grassland;
            tile.visible = true;
            tile.seen = true;
            tile.road = false;
            tile.railroad = false;
        }
        let id =
            simulation.spawn_unit(super::super::HUMAN, super::super::UnitKind::Warrior, 10, 24);
        simulation.remember_visible_human_tiles();
        let mut client = ClientSimulation::new(simulation);
        let original = client.goto_preview_view(id, 12, 24).unwrap();
        assert!(Rc::ptr_eq(
            &original,
            &client.goto_preview_view(id, 12, 24).unwrap()
        ));
        let other = client.goto_preview_view(id, 13, 24).unwrap();
        assert_eq!(
            (
                other.points.last().unwrap().x,
                other.points.last().unwrap().y
            ),
            (13, 24)
        );
        assert!(client.goto_preview_view(u32::MAX, 12, 24).is_none());
        assert!(
            client
                .goto_preview_view(id, 12, client.map_wrap().height())
                .is_none()
        );
        client.review_fixture().0.units[0].moves_left = 0;
        let changed = client.goto_preview_view(id, 12, 24).unwrap();
        assert_eq!(changed.points[0].moves_left, 0);
        assert_eq!(
            changed.as_ref(),
            &client.simulation.goto_preview_view(id, 12, 24).unwrap()
        );
        client.apply(Command::EndTurn);
        let changed = client.goto_preview_view(id, 12, 24).unwrap();
        assert_eq!(
            changed.as_ref(),
            &client.simulation.goto_preview_view(id, 12, 24).unwrap()
        );
    }

    #[test]
    fn missing_projections_are_cached_until_the_key_changes() {
        let cache = RefCell::new(None);
        let calls = std::cell::Cell::new(0);
        let build = || {
            calls.set(calls.get() + 1);
            None::<u32>
        };
        assert!(cached_projection(&cache, 1, build).is_none());
        assert!(cached_projection(&cache, 1, build).is_none());
        assert_eq!(calls.get(), 1);
        assert!(cached_projection(&cache, 2, build).is_none());
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn cached_actions_refresh_after_commands_and_fixture_mutations() {
        let mut client = ClientSimulation::new(Simulation::new(crate::mapgen::generate(42)));
        let id = client
            .simulation
            .units
            .iter()
            .find(|unit| unit.owner == super::super::HUMAN)
            .unwrap()
            .id;
        let first = client.unit_action_view(id).unwrap();
        assert_eq!(client.unit_action_view(id).unwrap().fortify, first.fortify);
        let revision = client.view_revision();
        assert!(client.apply(Command::EndTurn));
        assert_ne!(client.view_revision(), revision);
        assert!(client.unit_actions.borrow().is_none());
        let current = client.unit_action_view(id).unwrap();
        assert_eq!(
            current.work_left,
            client.simulation.unit_by_id(id).unwrap().work_left
        );
        assert!(client.unit_actions.borrow().is_some());
        let revision = client.view_revision();
        client.review_fixture().0.units.retain(|unit| unit.id != id);
        assert_ne!(client.view_revision(), revision);
        assert!(client.unit_action_view(id).is_none());
        assert!(!client.is_human_unit(id));
    }
}
