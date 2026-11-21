//! Typed memoizations shared by advisor systems; storage never escapes this owner.

use crate::{
    rules::Tech,
    sim::{
        Government,
        advisors::{AiBuildChoice, GuardCityAssessment},
        ai::SharedInvasionSupport,
        normal_paths::NormalPaths,
        pf_actions::PfAction,
        settlement::CitySite,
    },
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

pub(in crate::sim) type CachedOffensiveAdvice = (Option<AiBuildChoice>, Vec<(Tech, f32)>, bool);
pub(in crate::sim) type UnitClassTotals = BTreeMap<Option<String>, i32>;
pub(in crate::sim) type ActionKey = (usize, usize, usize, bool, bool, bool);
pub(in crate::sim) type OverlapMapKey = (usize, usize, i32, i32, i32, i32, usize);
pub(in crate::sim) type SightRevision = (u64, usize, usize);
pub(in crate::sim) type SightLayers = (Arc<[bool]>, Arc<[bool]>);
type VirtualCityTileYields = (Government, Vec<Option<(i32, i32, i32)>>);
type RouteZones = [Vec<Option<bool>>; 2];

#[derive(Clone, Default)]
pub(in crate::sim) struct AdvisorMemos(RefCell<MemoData>);

#[derive(Clone, Default)]
struct MemoData {
    unit_class_totals: BTreeMap<usize, Arc<UnitClassTotals>>,
    average_production: BTreeMap<usize, i32>,
    offensive_outputs: BTreeMap<u32, i32>,
    offensive_advice: BTreeMap<u32, CachedOffensiveAdvice>,
    invasion_support: BTreeMap<usize, SharedInvasionSupport>,
    guard_assessments: BTreeMap<u32, GuardCityAssessment>,
    counter_defenders: BTreeMap<(u32, usize), Option<(Tech, bool)>>,
    settlement_base: Option<Vec<i32>>,
    settlement_spacing: Option<Arc<[bool]>>,
    virtual_city_sites: BTreeMap<usize, Vec<Option<Option<CitySite>>>>,
    virtual_city_tile_yields: BTreeMap<usize, VirtualCityTileYields>,
    route_zone_control: BTreeMap<usize, RouteZones>,
    pf_actions: HashMap<ActionKey, PfAction>,
    overlap_maps: HashMap<OverlapMapKey, NormalPaths>,
    sight_layers: HashMap<usize, (SightRevision, SightLayers)>,
}

impl AdvisorMemos {
    pub(super) fn clear(&self) {
        *self.0.borrow_mut() = MemoData::default();
    }
    pub(super) fn clear_invasion_support(&self) {
        self.0.borrow_mut().invasion_support.clear();
    }
    pub(super) fn clear_founder_maps(&mut self) {
        let data = self.0.get_mut();
        data.settlement_base = None;
        data.settlement_spacing = None;
        data.virtual_city_tile_yields.clear();
    }
    pub(in crate::sim) fn clear_unit_class_totals(&self, owner: usize) {
        self.0.borrow_mut().unit_class_totals.remove(&owner);
    }
    pub(in crate::sim) fn unit_class_totals(&self, key: usize) -> Option<Arc<UnitClassTotals>> {
        self.0.borrow().unit_class_totals.get(&key).cloned()
    }
    pub(in crate::sim) fn remember_unit_class_totals(
        &self,
        key: usize,
        value: Arc<UnitClassTotals>,
    ) {
        self.0.borrow_mut().unit_class_totals.insert(key, value);
    }
    pub(in crate::sim) fn average_production(&self, key: usize) -> Option<i32> {
        self.0.borrow().average_production.get(&key).copied()
    }
    pub(in crate::sim) fn remember_average_production(&self, key: usize, value: i32) {
        self.0.borrow_mut().average_production.insert(key, value);
    }
    pub(in crate::sim) fn offensive_output(&self, key: u32) -> Option<i32> {
        self.0.borrow().offensive_outputs.get(&key).copied()
    }
    pub(in crate::sim) fn remember_offensive_output(&self, key: u32, value: i32) {
        self.0.borrow_mut().offensive_outputs.insert(key, value);
    }
    pub(in crate::sim) fn offensive_advice(&self, key: u32) -> Option<CachedOffensiveAdvice> {
        self.0.borrow().offensive_advice.get(&key).cloned()
    }
    pub(in crate::sim) fn remember_offensive_advice(&self, key: u32, value: CachedOffensiveAdvice) {
        self.0.borrow_mut().offensive_advice.insert(key, value);
    }
    pub(in crate::sim) fn take_overlap_map(&self, key: OverlapMapKey) -> Option<NormalPaths> {
        self.0.borrow_mut().overlap_maps.remove(&key)
    }
    pub(in crate::sim) fn return_overlap_map(&self, key: OverlapMapKey, map: NormalPaths) {
        self.0.borrow_mut().overlap_maps.insert(key, map);
    }
    pub(in crate::sim) fn sight_layers(
        &self,
        observer: usize,
        revision: SightRevision,
    ) -> Option<SightLayers> {
        self.0
            .borrow()
            .sight_layers
            .get(&observer)
            .filter(|(rev, _)| *rev == revision)
            .map(|(_, layers)| layers.clone())
    }
    pub(in crate::sim) fn remember_sight_layers(
        &self,
        observer: usize,
        revision: SightRevision,
        layers: SightLayers,
    ) {
        self.0
            .borrow_mut()
            .sight_layers
            .insert(observer, (revision, layers));
    }
    pub(in crate::sim) fn invasion_support(&self, key: usize) -> Option<SharedInvasionSupport> {
        self.0.borrow().invasion_support.get(&key).cloned()
    }
    pub(in crate::sim) fn remember_invasion_support(
        &self,
        key: usize,
        value: SharedInvasionSupport,
    ) {
        self.0.borrow_mut().invasion_support.insert(key, value);
    }
    pub(in crate::sim) fn guard_assessment(&self, key: u32) -> Option<GuardCityAssessment> {
        self.0.borrow().guard_assessments.get(&key).copied()
    }
    pub(in crate::sim) fn remember_guard_assessment(&self, key: u32, value: GuardCityAssessment) {
        self.0.borrow_mut().guard_assessments.insert(key, value);
    }
    pub(in crate::sim) fn counter_defender(
        &self,
        key: (u32, usize),
    ) -> Option<Option<(Tech, bool)>> {
        self.0.borrow().counter_defenders.get(&key).copied()
    }
    pub(in crate::sim) fn remember_counter_defender(
        &self,
        key: (u32, usize),
        value: Option<(Tech, bool)>,
    ) {
        self.0.borrow_mut().counter_defenders.insert(key, value);
    }
    pub(in crate::sim) fn pf_action(&self, key: ActionKey) -> Option<PfAction> {
        self.0.borrow().pf_actions.get(&key).copied()
    }
    pub(in crate::sim) fn remember_pf_action(&self, key: ActionKey, value: PfAction) {
        self.0.borrow_mut().pf_actions.insert(key, value);
    }
    pub(in crate::sim) fn settlement_base(&self) -> Option<Vec<i32>> {
        self.0.borrow().settlement_base.clone()
    }
    pub(in crate::sim) fn remember_settlement_base(&self, map: Vec<i32>) {
        self.0.borrow_mut().settlement_base = Some(map);
    }
    pub(in crate::sim) fn settlement_spacing(&self) -> Option<Arc<[bool]>> {
        self.0.borrow().settlement_spacing.clone()
    }
    pub(in crate::sim) fn remember_settlement_spacing(&self, map: Arc<[bool]>) {
        self.0.borrow_mut().settlement_spacing = Some(map);
    }
    pub(in crate::sim) fn virtual_city_site(
        &self,
        owner: usize,
        index: usize,
    ) -> Option<Option<CitySite>> {
        self.0
            .borrow()
            .virtual_city_sites
            .get(&owner)
            .and_then(|sites| sites[index].clone())
    }
    pub(in crate::sim) fn remember_virtual_city_site(
        &self,
        owner: usize,
        index: usize,
        tiles: usize,
        site: Option<CitySite>,
    ) {
        self.0
            .borrow_mut()
            .virtual_city_sites
            .entry(owner)
            .or_insert_with(|| vec![None; tiles])[index] = Some(site);
    }
    pub(in crate::sim) fn virtual_city_tile_yield(
        &self,
        owner: usize,
        government: Government,
        index: usize,
    ) -> Option<(i32, i32, i32)> {
        self.0
            .borrow()
            .virtual_city_tile_yields
            .get(&owner)
            .filter(|(cached, _)| *cached == government)
            .and_then(|(_, yields)| yields.get(index))
            .copied()
            .flatten()
    }
    pub(in crate::sim) fn remember_virtual_city_tile_yield(
        &self,
        owner: usize,
        government: Government,
        index: usize,
        tiles: usize,
        output: (i32, i32, i32),
    ) {
        let mut data = self.0.borrow_mut();
        let entry = data
            .virtual_city_tile_yields
            .entry(owner)
            .or_insert_with(|| (government, vec![None; tiles]));
        if entry.0 != government {
            *entry = (government, vec![None; tiles]);
        }
        entry.1[index] = Some(output);
    }
    pub(in crate::sim) fn zone_control(
        &self,
        owner: usize,
        visibility: usize,
        index: usize,
    ) -> Option<bool> {
        self.0
            .borrow()
            .route_zone_control
            .get(&owner)
            .and_then(|cache| cache[visibility].get(index))
            .copied()
            .flatten()
    }
    pub(in crate::sim) fn remember_zone_control(
        &self,
        owner: usize,
        visibility: usize,
        index: usize,
        tiles: usize,
        controlled: bool,
    ) {
        let mut data = self.0.borrow_mut();
        let cache = data
            .route_zone_control
            .entry(owner)
            .or_insert_with(|| std::array::from_fn(|_| vec![None; tiles]));
        cache[visibility][index] = Some(controlled);
    }
}
