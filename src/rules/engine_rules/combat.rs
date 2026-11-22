//! Typed selected-ruleset combat settings.

/// C2C3 combat-bonus operations supported by Freeciv's combat calculations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CombatBonusKind {
    DefenseMultiplier,
    DefenseMultiplierPct,
    DefenseDivider,
    DefenseDividerPct,
    LowFirepower,
}

/// Selected-ruleset policy for defending while carried by a transporter.
#[allow(
    dead_code,
    reason = "complete Freeciv option set; the campaign selects a subset"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransportDefense {
    Blocked,
    Alight,
    Always,
}

/// A unit's bonus against a target unit-type flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnitCombatBonus {
    pub(crate) flag: String,
    pub(crate) kind: CombatBonusKind,
    pub(crate) value: i32,
}

/// Combat parameters and game settings that affect Freeciv's military rules.
/// Every field is read from the selected C2C3 derivative; `killstack` and
/// `killcitizen` were not written there upstream, so CivCraft records the
/// pinned engine defaults explicitly in its `[settings]` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CombatRulesetSettings {
    pub(crate) tired_attack: bool,
    pub(crate) only_killing_makes_veteran: bool,
    pub(crate) only_real_fight_makes_veteran: bool,
    pub(crate) combat_odds_scaled_veterancy: bool,
    pub(crate) damage_reduces_bombard_rate: bool,
    pub(crate) low_firepower_badwallattacker: i32,
    pub(crate) low_firepower_pearl_harbor: i32,
    pub(crate) low_firepower_combat_bonus: i32,
    pub(crate) low_firepower_nonnat_bombard: i32,
    pub(crate) nuke_pop_loss_pct: i32,
    pub(crate) nuke_defender_survival_chance_pct: i32,
    pub(crate) killstack: bool,
    pub(crate) killcitizen: bool,
    pub(crate) unreachable_protects: bool,
    pub(crate) occupychance: i32,
    pub(crate) razechance: i32,
    pub(crate) savepalace: bool,
    pub(crate) homecaughtunits: bool,
}
