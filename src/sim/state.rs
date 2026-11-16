//! Transient parts of [`Simulation`](super::Simulation), grouped by purpose.
//!
//! None of these is serialized with the campaign struct. The save module
//! writes the AI memories that must survive a reload beside it; everything
//! else is rebuilt or starts empty.

/// Plans, reservations and memories the AI players keep between phases.
#[derive(Clone, Default)]
pub(super) struct AiState {
    pub(in crate::sim) map_memory: super::AiMapMemory,
    pub(in crate::sim) purchase_reserve: std::collections::BTreeMap<usize, i32>,
    pub(in crate::sim) war_plans:
        std::collections::BTreeMap<(usize, usize), super::diplomacy::WarPlan>,
    pub(in crate::sim) space_warnings: std::collections::BTreeMap<(usize, usize), i32>,
    pub(in crate::sim) free_bulbs: std::collections::BTreeMap<usize, i32>,
    pub(in crate::sim) tech_wants: std::collections::BTreeMap<usize, Vec<f32>>,
    pub(in crate::sim) ground_targets: std::collections::BTreeMap<u32, u32>,
    pub(in crate::sim) settlement_targets:
        std::collections::BTreeMap<u32, super::settlement::SettlementReservation>,
    pub(in crate::sim) diplomatic_memory:
        std::collections::BTreeMap<(usize, usize), super::diplomacy::DiplomaticMemory>,
    pub(in crate::sim) diplomatic_intel:
        std::collections::BTreeMap<(usize, usize), super::diplomacy::DiplomaticIntel>,
    pub(in crate::sim) pending_war_targets: std::collections::BTreeMap<usize, usize>,
    pub(in crate::sim) guard_targets: std::collections::BTreeMap<u32, super::guards::GuardTarget>,
    pub(in crate::sim) unit_guards: std::collections::BTreeMap<u32, super::guards::GuardState>,
    pub(in crate::sim) worker_tasks: std::collections::BTreeSet<u32>,
    pub(in crate::sim) ferries: super::ferries::FerryRegistry,
    pub(in crate::sim) government_cache:
        std::collections::BTreeMap<usize, super::government::GovernmentAdvisorCache>,
}

/// Derived lookups and memoized results. Interior mutability lets read-only rule code fill them.
#[derive(Clone, Default)]
pub(super) struct Caches {
    pub(in crate::sim) unit_ids: std::cell::RefCell<std::collections::HashMap<u32, usize>>,
    pub(in crate::sim) city_ids: std::cell::RefCell<std::collections::HashMap<u32, usize>>,
    pub(in crate::sim) advisor_enabled: std::cell::Cell<bool>,
    pub(in crate::sim) threat_routes_enabled: std::cell::Cell<bool>,
    pub(in crate::sim) threat_arrivals:
        std::cell::RefCell<std::collections::BTreeMap<(u32, u32), Option<i32>>>,
    pub(in crate::sim) danger_routes: std::cell::RefCell<std::collections::BTreeMap<u32, Vec<i32>>>,
    pub(in crate::sim) step_costs:
        std::cell::RefCell<std::collections::HashMap<super::movement_rules::StepCostCacheKey, i32>>,
    pub(in crate::sim) native_tiles: std::cell::RefCell<Vec<i8>>,
    pub(in crate::sim) spatial: std::cell::RefCell<super::AdvisorSpatialIndex>,
}

impl Caches {
    pub(super) fn advisor_cache_enabled(&self) -> bool {
        self.advisor_enabled.get()
    }

    pub(super) fn threat_route_cache_enabled(&self) -> bool {
        self.threat_routes_enabled.get()
    }

    pub(super) fn clear_advisor_threats(&self) {
        self.threat_arrivals.borrow_mut().clear();
        self.danger_routes.borrow_mut().clear();
    }

    pub(super) fn end_advisor_cache(&self) {
        self.advisor_enabled.set(false);
        self.threat_routes_enabled.set(false);
        self.clear_advisor_threats();
        *self.spatial.borrow_mut() = Default::default();
    }

    pub(super) fn end_advisor_cache_preserving_threat_routes(&self) {
        self.advisor_enabled.set(false);
        *self.spatial.borrow_mut() = Default::default();
        self.threat_routes_enabled.set(true);
    }

    pub(super) fn clear_ai_threat_route_cache(&self) {
        self.threat_routes_enabled.set(false);
        self.clear_advisor_threats();
    }

    pub(super) fn unit_index_by_id(&self, id: u32, units: &[super::Unit]) -> Option<usize> {
        if let Some(index) = self.unit_ids.borrow().get(&id).copied()
            && units.get(index).is_some_and(|unit| unit.id == id)
        {
            return Some(index);
        }
        let index = units.iter().position(|unit| unit.id == id);
        let mut cache = self.unit_ids.borrow_mut();
        if let Some(index) = index {
            cache.insert(id, index);
        } else {
            cache.remove(&id);
        }
        index
    }

    pub(super) fn city_index_by_id(&self, id: u32, cities: &[super::City]) -> Option<usize> {
        if let Some(index) = self.city_ids.borrow().get(&id).copied()
            && cities.get(index).is_some_and(|city| city.id == id)
        {
            return Some(index);
        }
        let index = cities.iter().position(|city| city.id == id);
        let mut cache = self.city_ids.borrow_mut();
        if let Some(index) = index {
            cache.insert(id, index);
        } else {
            cache.remove(&id);
        }
        index
    }

    pub(super) fn native_tile(&self, index: usize, cache_len: usize) -> Option<bool> {
        let mut cache = self.native_tiles.borrow_mut();
        if cache.is_empty() {
            cache.resize(cache_len, -1);
        }
        (cache[index] >= 0).then(|| cache[index] != 0)
    }

    pub(super) fn store_native_tile(&self, index: usize, native: bool) {
        self.native_tiles.borrow_mut()[index] = i8::from(native);
    }

    pub(super) fn step_cost(&self, key: &super::movement_rules::StepCostCacheKey) -> Option<i32> {
        self.step_costs.borrow().get(key).copied()
    }

    pub(super) fn store_step_cost(&self, key: super::movement_rules::StepCostCacheKey, cost: i32) {
        self.step_costs.borrow_mut().insert(key, cost);
    }

    pub(super) fn danger_route(&self, unit_id: u32) -> Option<Vec<i32>> {
        self.danger_routes.borrow().get(&unit_id).cloned()
    }

    pub(super) fn store_danger_route(&self, unit_id: u32, costs: Vec<i32>) {
        self.danger_routes.borrow_mut().insert(unit_id, costs);
    }

    pub(super) fn threat_arrival(&self, key: (u32, u32)) -> Option<Option<i32>> {
        self.threat_arrivals.borrow().get(&key).copied()
    }

    pub(super) fn store_threat_arrival(&self, key: (u32, u32), turns: Option<i32>) {
        self.threat_arrivals.borrow_mut().insert(key, turns);
    }
}

/// Bookkeeping for fog-of-war refreshes deferred while a batch of changes is applied.
#[derive(Clone, Default)]
pub(super) struct VisibilityRefresh {
    pub(in crate::sim) refresh_deferred: bool,
    pub(in crate::sim) refresh_pending: bool,
    pub(in crate::sim) pending_render_tiles: Vec<bool>,
    pub(in crate::sim) render_scope: Option<Vec<usize>>,
}
