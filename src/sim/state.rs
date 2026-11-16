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

/// Bookkeeping for fog-of-war refreshes deferred while a batch of changes is applied.
#[derive(Clone, Default)]
pub(super) struct VisibilityRefresh {
    pub(in crate::sim) refresh_deferred: bool,
    pub(in crate::sim) refresh_pending: bool,
    pub(in crate::sim) pending_render_tiles: Vec<bool>,
    pub(in crate::sim) render_scope: Option<Vec<usize>>,
}
